//! Relógio monotônico injetável.
//!
//! Todo timestamp do pipeline MIDI é "nanossegundos desde a base do relógio".
//! Os tipos vêm de [`piano_tutor_core::tempo`] e são **os mesmos** que o motor
//! de áudio (`piano-tutor-audio`) usa para ancorar frames: o app cria um único
//! relógio e o entrega ao [`crate::GerenciadorMidi`] e à saída de áudio.

pub use piano_tutor_core::tempo::{Relogio, RelogioManual, RelogioMonotonico};
