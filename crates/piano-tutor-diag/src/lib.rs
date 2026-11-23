//! Diagnóstico de ambiente Linux do Piano Tutor (F00).

pub mod coleta;
pub mod latencia;
pub mod medicao;
pub mod relatorio;
/// Reexportado do núcleo (única implementação).
pub use piano_tutor_core::sanitizar;
