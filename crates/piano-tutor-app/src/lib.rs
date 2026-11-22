//! # piano-tutor-app
//!
//! App "Olá Piano Tutor" (F00): a janela mínima do Piano Tutor sobre a `bancada-casca`
//! (ADR-0002 e ADR-0004). O estado e as ações ficam em módulos puros, testados sem abrir
//! janela; `app` liga tudo à casca.
//!
//! F02 (#34, parte de UI): tela Espelho do teclado de 61 teclas em modo simulação
//! (`espelho` guarda o estado puro; `tela_espelho` desenha). Ainda sem MIDI real, sem áudio e
//! sem calibração: o gerenciador MIDI entra por `EstadoEspelho::aplicar_evento`.

pub mod acoes;
pub mod app;
pub mod espelho;
pub mod estado;
pub mod tela_espelho;

/// `tokens.json` do app (formato lido pela casca).
pub const TOKENS_JSON: &str = include_str!("../dados/tokens.json");
/// `temas.json` do app: Noite, Claro e Ébano.
pub const TEMAS_JSON: &str = include_str!("../dados/temas.json");
