//! Alinhamento de uma partitura ao áudio de uma Aula (M4, etapa 4).
//!
//! Entrada: uma partitura (MusicXML/MXL) e o áudio da Aula (WAV mono, de
//! `ffmpeg -i aula.mp4 -ac 1 -ar 22050 -vn x.wav`). Saída: o instante de cada nota esperada no
//! áudio, com confiança (0–1), e o trecho tocado (recorte). O resultado é uma **Hipótese**: só
//! vira dado confiável depois de revisão humana.
//!
//! Método (`dtw-croma`): croma do áudio (STFT, 12 classes) × croma sintético da partitura, DTW
//! em modo subsequência (o tutor fala e depois toca), refinamento de cada nota para o onset
//! mais próximo (±80 ms) e confiança por nota. Veja `docs/specs/FORMATO-PACOTE-DE-CURSO.md`
//! para o JSON de saída.
//!
//! Núcleo puro: sem GUI, rede nem `unsafe`. **NÃO VERIFICADO com áudio real de piano**; os
//! testes usam fixtures sintéticas.

pub mod croma;
pub mod dtw;
pub mod espectro;
pub mod fft;
pub mod mapeamento;
pub mod onsets;
pub mod saida;
pub mod wav;

use croma::{ConfigCroma, HARMONICOS_PADRAO, croma_da_partitura, croma_do_audio, custo_cosseno};
use dtw::{ConfigDtw, ErroDtw, dtw};
use mapeamento::{ConfigMapeamento, NotaAlinhada, Recorte, calcular_recorte, mapear_notas};
use onsets::{ConfigOnsets, detectar_onsets};
use piano_tutor_core::comparador::{EventoEsperado, esperados_da_partitura};
use piano_tutor_core::partitura::Partitura;
use std::fmt;
use wav::Audio;

/// Andamento (semínimas por minuto) usado quando nem o arquivo nem o usuário informam.
pub const BPM_PADRAO: f64 = 80.0;

/// Configuração completa do alinhamento.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigAlinhamento {
    pub croma: ConfigCroma,
    pub onsets: ConfigOnsets,
    pub dtw: ConfigDtw,
    pub mapeamento: ConfigMapeamento,
    /// Pesos dos harmônicos no croma sintético da partitura.
    pub harmonicos: Vec<f32>,
    /// Andamento informado pelo usuário (sobrepõe o do arquivo); `None` = usar o do arquivo.
    pub bpm: Option<f64>,
    /// Margem somada antes da primeira e depois da última nota no recorte.
    pub margem_s: f64,
    /// Restringe a busca a `[inicio_s, fim_s]` do áudio (tempos do áudio inteiro).
    pub busca: Option<(f64, f64)>,
}

impl Default for ConfigAlinhamento {
    fn default() -> Self {
        Self {
            croma: ConfigCroma::default(),
            onsets: ConfigOnsets::default(),
            dtw: ConfigDtw {
                subsequencia: true,
                ..ConfigDtw::default()
            },
            mapeamento: ConfigMapeamento::default(),
            harmonicos: HARMONICOS_PADRAO.to_vec(),
            bpm: None,
            margem_s: 0.5,
            busca: None,
        }
    }
}

#[derive(Debug)]
pub enum ErroAlinhamento {
    PartituraSemNotas,
    AudioCurto,
    BpmInvalido,
    BuscaInvalida,
    Dtw(ErroDtw),
}

impl fmt::Display for ErroAlinhamento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PartituraSemNotas => write!(f, "a partitura não tem notas"),
            Self::AudioCurto => write!(f, "o trecho de áudio é curto demais para alinhar"),
            Self::BpmInvalido => write!(f, "andamento inválido (use um número positivo)"),
            Self::BuscaInvalida => write!(f, "intervalo de busca inválido (início < fim, ≥ 0)"),
            Self::Dtw(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ErroAlinhamento {}

impl From<ErroDtw> for ErroAlinhamento {
    fn from(e: ErroDtw) -> Self {
        Self::Dtw(e)
    }
}

/// Resultado do alinhamento. Todos os instantes são do áudio **inteiro** recebido.
#[derive(Clone, Debug, PartialEq)]
pub struct Alinhamento {
    /// Uma entrada por nota esperada, na ordem de `esperados_da_partitura` (campo `indice`).
    pub notas: Vec<NotaAlinhada>,
    /// Trecho tocado (primeira nota − margem até última nota + duração + margem).
    pub recorte: Recorte,
    pub confianca_media: f32,
    /// Custo médio por quadro da partitura ao longo do caminho (0 = croma idêntico).
    pub custo_medio: f32,
}

/// Eventos esperados em segundos. Com `bpm`, o andamento do arquivo é reescalado para que o
/// início valha `bpm`; sem andamento no arquivo, `bpm` (ou [`BPM_PADRAO`]) vale em toda parte.
pub fn esperados_com_andamento(p: &Partitura, bpm: Option<f64>) -> Vec<EventoEsperado> {
    match (bpm, p.mapa_de_andamento().inicial()) {
        (Some(b), Some(inicial)) if inicial > 0.0 => esperados_da_partitura(p, b, b / inicial),
        (Some(b), None) => esperados_da_partitura(p, b, 1.0),
        _ => esperados_da_partitura(p, BPM_PADRAO, 1.0),
    }
}

/// Alinha uma partitura ao áudio.
pub fn alinhar(
    partitura: &Partitura,
    audio: &Audio,
    cfg: &ConfigAlinhamento,
) -> Result<Alinhamento, ErroAlinhamento> {
    if let Some(b) = cfg.bpm
        && !(b.is_finite() && b > 0.0)
    {
        return Err(ErroAlinhamento::BpmInvalido);
    }
    let esperados = esperados_com_andamento(partitura, cfg.bpm);
    alinhar_esperados(&esperados, audio, cfg)
}

/// Alinha uma lista de eventos esperados (já em segundos) ao áudio.
pub fn alinhar_esperados(
    esperados: &[EventoEsperado],
    audio: &Audio,
    cfg: &ConfigAlinhamento,
) -> Result<Alinhamento, ErroAlinhamento> {
    if esperados.is_empty() {
        return Err(ErroAlinhamento::PartituraSemNotas);
    }
    let (trecho, deslocamento) = match cfg.busca {
        Some((a, b)) => {
            if !(a.is_finite() && b.is_finite() && a >= 0.0 && b > a) {
                return Err(ErroAlinhamento::BuscaInvalida);
            }
            audio.recortar(a, b)
        }
        None => (audio.clone(), 0.0),
    };
    let croma_audio = croma_do_audio(&trecho, &cfg.croma);
    let Some(croma_part) = croma_da_partitura(esperados, croma_audio.quadro_s, &cfg.harmonicos)
    else {
        return Err(ErroAlinhamento::PartituraSemNotas);
    };
    let n = croma_part.quadros.len();
    let m = croma_audio.quadros.len();
    if m < 2 || (cfg.dtw.subsequencia && m < n / 2) {
        return Err(ErroAlinhamento::AudioCurto);
    }
    let custo = |i: usize, j: usize| custo_cosseno(&croma_part.quadros[i], &croma_audio.quadros[j]);
    let caminho = dtw(n, m, custo, &cfg.dtw)?;
    let onsets = detectar_onsets(&trecho, &cfg.onsets);
    let mut notas = mapear_notas(
        esperados,
        &croma_part,
        &caminho,
        &onsets,
        &custo,
        &cfg.mapeamento,
    );
    let custo_medio = caminho
        .passos
        .iter()
        .map(|&(i, j)| custo(i, j))
        .sum::<f32>()
        / caminho.passos.len() as f32;
    for nota in &mut notas {
        nota.t_audio_s += deslocamento;
    }
    let recorte = calcular_recorte(&notas, audio.duracao_s(), cfg.margem_s)
        .ok_or(ErroAlinhamento::PartituraSemNotas)?;
    let confianca_media = notas.iter().map(|x| x.confianca).sum::<f32>() / notas.len() as f32;
    Ok(Alinhamento {
        notas,
        recorte,
        confianca_media,
        custo_medio,
    })
}
