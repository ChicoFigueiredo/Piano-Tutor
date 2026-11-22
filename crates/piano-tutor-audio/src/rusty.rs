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
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};

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
