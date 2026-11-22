//! Motor de áudio do Piano Tutor (F03, ticket #37).
//!
//! - [`relogio`]: relógio mestre em frames de amostra, conversões e âncora
//!   frames ↔ relógio monotônico (fonte única de tempo, ADR-0005).
//! - [`comando`]: comandos e o lado produtor da fila sem lock (`rtrb`).
//! - [`motor`]: o que roda no callback (sem alocação, sem lock, sem E/S).
//! - [`sintetizador`]: trait [`Sintetizador`] e sintetizador falso de teste.
//! - [`rusty`]: sintetizador SF2 com `rustysynth`.
//! - [`saida`]: saída `cpal` no dispositivo padrão.
//! - [`offline`]: render sem dispositivo e WAV de teste.
//! - [`jitter`]: medição offline do erro de posicionamento.
//! - [`sf2_sintetico`]: SF2 mínimo gerado em memória para testes.
//!
//! Kira não é usado: a spec (§3.4) só admite Kira após spike que prove o
//! mesmo relógio para o sintetizador.

pub mod comando;
pub mod erro;
pub mod jitter;
pub mod motor;
pub mod offline;
pub mod relogio;
pub mod rusty;
pub mod saida;
pub mod sf2_sintetico;
pub mod sintetizador;

pub use comando::{Acao, ComandoAudio, ControleAudio, Quando};
pub use erro::ErroAudio;
pub use motor::{ConfigMotor, EstadoMotor, MotorAudio, Telemetria};
pub use relogio::{LeitorRelogio, Relogio, RelogioManual, RelogioMonotonico, TaxaAmostragem};
pub use rusty::{ConfigRusty, PresetSf2, SintetizadorRusty, SoundFont, listar_presets};
pub use saida::{ConfigSaida, InfoSaida, SaidaAudio};
pub use sintetizador::{Sintetizador, SintetizadorFalso};
