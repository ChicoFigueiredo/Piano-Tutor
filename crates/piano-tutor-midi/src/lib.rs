//! Entrada MIDI do Piano Tutor (F01, ticket #32): portas, conexão, hot-plug,
//! log NDJSON anonimizado e capacidades observadas. Sem UI.
//!
//! - [`FontePortas`]: origem das portas; real em [`FonteMidir`] (`midir`/ALSA),
//!   simulada em [`FonteFalsa`] (testes e modo simulação).
//! - [`GerenciadorMidi`]: seleção manual, estado observável, desconexão por
//!   varredura (≤ 1 s), limpeza de notas e reconexão automática.
//! - [`RegistroNdjson`]: uma linha JSON por evento, nomes de porta sanitizados.
//! - [`Capacidades`]: velocity variável, CC64, program change e liberação de
//!   nota, só quando observados.
//!
//! A interpretação dos bytes é a do núcleo ([`piano_tutor_core::midi::Parser`]).

pub mod capacidades;
pub mod fonte;
pub mod fonte_falsa;
pub mod fonte_midir;
pub mod gerenciador;
pub mod registro;
pub mod relogio;

pub use capacidades::{Capacidades, Observacao};
pub use fonte::{CallbackBytes, ConexaoAtiva, ErroFonte, FontePortas};
pub use fonte_falsa::FonteFalsa;
pub use fonte_midir::FonteMidir;
pub use gerenciador::{
    Contadores, EstadoConexao, GerenciadorMidi, INTERVALO_VARREDURA_MAXIMO, SaidaMidi,
    aplicar_em_teclas,
};
pub use registro::{MotivoDesconexao, RegistroNdjson, criar_arquivo_sessao};
pub use relogio::{Relogio, RelogioManual, RelogioMonotonico};
