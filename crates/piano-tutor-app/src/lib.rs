//! # piano-tutor-app
//!
//! App "Olá Piano Tutor" (F00): a janela mínima do Piano Tutor sobre a `bancada-casca`
//! (ADR-0002 e ADR-0004). O estado e as ações ficam em módulos puros, testados sem abrir
//! janela; `app` liga tudo à casca.
//!
//! Fora de escopo nesta fase: teclado de 61 teclas (F02), MIDI e áudio.

pub mod acoes;
pub mod app;
pub mod estado;

/// `tokens.json` do app (formato lido pela casca).
pub const TOKENS_JSON: &str = include_str!("../dados/tokens.json");
/// `temas.json` do app: Noite, Claro e Ébano.
pub const TEMAS_JSON: &str = include_str!("../dados/temas.json");
