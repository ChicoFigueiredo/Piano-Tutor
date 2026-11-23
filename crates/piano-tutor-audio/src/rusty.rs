//! Sintetizador SF2 real com `rustysynth` 1.3.8 (MIT, Rust puro, sem
//! dependências). Carregar o SF2 é lento e aloca: faça-o **fora** do
//! callback, na thread de controle, e só então entregue ao motor.
//!
//! Limite conhecido: o `rustysynth` renderiza em blocos internos
//! (`bloco_interno`, padrão 64 frames). Um evento aplicado no meio de um bloco
//! interno só soa no início do bloco interno seguinte, ou seja, até
//! `bloco_interno - 1` frames depois do frame agendado (≈ 1,3 ms a 48 kHz).
//! Medido no relatório de jitter offline.

use std::fs::File;
use std::io::{BufReader, Read};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::Arc;

use piano_tutor_core::NotaMidi;
use piano_tutor_core::midi::CanalMidi;
use rustysynth::{Synthesizer, SynthesizerSettings};

/// SoundFont carregado (tipo do `rustysynth`), reexportado para o app não
/// depender diretamente da crate.
pub use rustysynth::SoundFont;

use crate::comando::pitch_bend_para_bytes;
use crate::erro::ErroAudio;
use crate::relogio::TaxaAmostragem;
use crate::sintetizador::Sintetizador;

/// Parâmetros do `rustysynth`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfigRusty {
    /// Bloco interno de render (8–1024 frames). Menor = agendamento mais
    /// preciso e mais custo de CPU.
    pub bloco_interno: usize,
    /// Vozes simultâneas (8–256).
    pub polifonia: usize,
    pub reverb_e_chorus: bool,
}

impl Default for ConfigRusty {
    fn default() -> Self {
        Self {
            bloco_interno: 64,
            polifonia: 64,
            reverb_e_chorus: true,
        }
    }
}

/// Carrega e valida um SoundFont. Qualquer falha (arquivo ausente, SF2
/// malformado, SF3 não suportado, ou pânico interno do parser) vira
/// [`ErroAudio::SoundFontInvalido`], sem derrubar o app.
pub fn carregar_soundfont(caminho: &Path) -> Result<Arc<SoundFont>, ErroAudio> {
    let arquivo = File::open(caminho)
        .map_err(|e| ErroAudio::SoundFontInvalido(format!("{}: {e}", caminho.display())))?;
    let mut leitor = BufReader::new(arquivo);
    ler_soundfont(&mut leitor)
}

/// Como [`carregar_soundfont`], a partir de qualquer leitor.
pub fn ler_soundfont<R: Read>(leitor: &mut R) -> Result<Arc<SoundFont>, ErroAudio> {
    match catch_unwind(AssertUnwindSafe(|| SoundFont::new(leitor))) {
        Ok(Ok(sf)) => Ok(Arc::new(sf)),
        Ok(Err(e)) => Err(ErroAudio::SoundFontInvalido(e.to_string())),
        Err(_) => Err(ErroAudio::SoundFontInvalido(
            "o parser do SF2 falhou (arquivo malformado)".to_string(),
        )),
    }
}

/// Um preset do SF2: banco, programa (0–127) e nome como está no arquivo.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PresetSf2 {
    pub banco: u16,
    pub programa: u8,
    pub nome: String,
}

/// Presets do SoundFont, ordenados por banco e programa. Entradas com banco
/// ou programa fora da faixa MIDI são omitidas (nunca inventadas).
pub fn listar_presets(soundfont: &SoundFont) -> Vec<PresetSf2> {
    let mut lista: Vec<PresetSf2> = soundfont
        .get_presets()
        .iter()
        .filter_map(|p| {
            Some(PresetSf2 {
                banco: u16::try_from(p.get_bank_number()).ok()?,
                programa: u8::try_from(p.get_patch_number())
                    .ok()
                    .filter(|v| *v <= 127)?,
                nome: p.get_name().trim().to_string(),
            })
        })
        .collect();
    lista.sort();
    lista.dedup_by(|a, b| a.banco == b.banco && a.programa == b.programa);
    lista
}

/// Sintetizador SF2 atrás do trait [`Sintetizador`].
pub struct SintetizadorRusty {
    sint: Synthesizer,
}

impl std::fmt::Debug for SintetizadorRusty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SintetizadorRusty")
            .field("taxa", &self.sint.get_sample_rate())
            .field("bloco_interno", &self.sint.get_block_size())
            .field("polifonia", &self.sint.get_maximum_polyphony())
            .finish()
    }
}

impl SintetizadorRusty {
    pub fn novo(
        soundfont: &Arc<SoundFont>,
        taxa: TaxaAmostragem,
        config: ConfigRusty,
    ) -> Result<Self, ErroAudio> {
        let hz = i32::try_from(taxa.hz())
            .map_err(|_| ErroAudio::ParametroInvalido("taxa de amostragem grande demais"))?;
        let mut ajustes = SynthesizerSettings::new(hz);
        ajustes.block_size = config.bloco_interno;
        ajustes.maximum_polyphony = config.polifonia;
        ajustes.enable_reverb_and_chorus = config.reverb_e_chorus;
        let sint = Synthesizer::new(soundfont, &ajustes)
            .map_err(|e| ErroAudio::Sintetizador(e.to_string()))?;
        Ok(Self { sint })
    }

    /// Atalho: carrega o arquivo e cria o sintetizador.
    pub fn de_arquivo(
        caminho: &Path,
        taxa: TaxaAmostragem,
        config: ConfigRusty,
    ) -> Result<Self, ErroAudio> {
        let sf = carregar_soundfont(caminho)?;
        Self::novo(&sf, taxa, config)
    }

    /// Presets (banco, programa, nome) disponíveis no SF2.
    pub fn presets(&self) -> Vec<(i32, i32, String)> {
        let mut lista: Vec<(i32, i32, String)> = self
            .sint
            .get_sound_font()
            .get_presets()
            .iter()
            .map(|p| {
                (
                    p.get_bank_number(),
                    p.get_patch_number(),
                    p.get_name().to_string(),
                )
            })
            .collect();
        lista.sort();
        lista
    }

    /// Volume mestre interno do `rustysynth` (padrão 0,5).
    pub fn volume_interno(&self) -> f32 {
        self.sint.get_master_volume()
    }
}

impl Sintetizador for SintetizadorRusty {
    fn nota_ligada(&mut self, canal: CanalMidi, nota: NotaMidi, velocidade: u8) {
        self.sint.note_on(
            i32::from(canal.interno()),
            i32::from(nota.numero()),
            i32::from(velocidade.min(127)),
        );
    }

    fn nota_desligada(&mut self, canal: CanalMidi, nota: NotaMidi) {
        self.sint
            .note_off(i32::from(canal.interno()), i32::from(nota.numero()));
    }

    fn controle(&mut self, canal: CanalMidi, controle: u8, valor: u8) {
        self.sint.process_midi_message(
            i32::from(canal.interno()),
            0xB0,
            i32::from(controle & 0x7F),
            i32::from(valor & 0x7F),
        );
    }

    fn programa(&mut self, canal: CanalMidi, programa: u8) {
        self.sint.process_midi_message(
            i32::from(canal.interno()),
            0xC0,
            i32::from(programa & 0x7F),
            0,
        );
    }

    fn pitch_bend(&mut self, canal: CanalMidi, valor: i16) {
        let (lsb, msb) = pitch_bend_para_bytes(valor);
        self.sint.process_midi_message(
            i32::from(canal.interno()),
            0xE0,
            i32::from(lsb),
            i32::from(msb),
        );
    }

    fn todas_notas_desligadas(&mut self) {
        self.sint.note_off_all(false);
    }

    fn silenciar_tudo(&mut self) {
        self.sint.note_off_all(true);
        self.sint.reset_all_controllers();
    }

    fn renderizar(&mut self, esq: &mut [f32], dir: &mut [f32]) {
        let n = esq.len().min(dir.len());
        self.sint.render(&mut esq[..n], &mut dir[..n]);
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::sf2_sintetico;

    fn sintetizador() -> SintetizadorRusty {
        let bytes = sf2_sintetico::gerar();
        let sf = ler_soundfont(&mut bytes.as_slice()).expect("SF2 sintético");
        let taxa = TaxaAmostragem::nova(48_000).expect("taxa");
        SintetizadorRusty::novo(
            &sf,
            taxa,
            ConfigRusty {
                reverb_e_chorus: false,
                ..ConfigRusty::default()
            },
        )
        .expect("sintetizador")
    }

    fn canal0() -> CanalMidi {
        CanalMidi::novo(0).expect("canal")
    }

    /// Toca a nota 69 (senoide) com o bend dado e conta os cruzamentos
    /// ascendentes por zero em ~0,5 s de render offline (proporcional à
    /// frequência).
    fn cruzamentos(bend: Option<i16>) -> usize {
        let mut s = sintetizador();
        s.programa(canal0(), sf2_sintetico::PROGRAMA_SENOIDE);
        if let Some(v) = bend {
            s.pitch_bend(canal0(), v);
        }
        s.nota_ligada(canal0(), NotaMidi::nova(69).expect("nota"), 100);
        let mut esq = vec![0.0_f32; 24_000];
        let mut dir = vec![0.0_f32; 24_000];
        s.renderizar(&mut esq, &mut dir);
        // Ignora o ataque inicial.
        esq[2_000..]
            .windows(2)
            .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
            .count()
    }

    #[test]
    fn pitch_bend_nao_entra_em_panico_com_valores_extremos() {
        let mut s = sintetizador();
        for v in [i16::MIN, -8192, -1, 0, 1, 8191, i16::MAX] {
            s.pitch_bend(canal0(), v);
        }
        let mut esq = vec![0.0_f32; 256];
        let mut dir = vec![0.0_f32; 256];
        s.renderizar(&mut esq, &mut dir);
        assert!(esq.iter().chain(dir.iter()).all(|x| x.is_finite()));
    }

    #[test]
    fn pitch_bend_muda_a_frequencia_no_render_offline() {
        let centro = cruzamentos(None);
        let centro_explicito = cruzamentos(Some(0));
        let acima = cruzamentos(Some(8191));
        let abaixo = cruzamentos(Some(-8192));
        assert!(centro > 100, "esperava um tom audível, veio {centro}");
        assert_eq!(centro, centro_explicito);
        // ±2 semitons: razão 1,122 para cima e 0,891 para baixo (tolerância de 3 %).
        let r_acima = acima as f64 / centro as f64;
        let r_abaixo = abaixo as f64 / centro as f64;
        assert!((r_acima - 1.1225).abs() < 0.03, "razão acima {r_acima}");
        assert!((r_abaixo - 0.8909).abs() < 0.03, "razão abaixo {r_abaixo}");
    }
}
