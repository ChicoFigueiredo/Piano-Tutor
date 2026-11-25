//! # piano-tutor-app
//!
//! App "Olá Piano Tutor" (F00): a janela mínima do Piano Tutor sobre a `bancada-casca`
//! (ADR-0002 e ADR-0004). O estado e as ações ficam em módulos puros, testados sem abrir
//! janela; `app` liga tudo à casca.
//!
//! Marco M1: **teclado físico → MIDI → Espelho + som**.
//!
//! - `monitor` / `tela_monitor` (F01, #32): portas, conexão, capacidades, log NDJSON.
//! - `espelho` / `tela_espelho` (F02, #34): teclado de 61 teclas; recebe eventos físicos por
//!   `EstadoEspelho::aplicar_evento` e, sem dispositivo, a simulação (mouse e teclado do PC).
//! - `som` / `tela_timbres` (F03, #38): SoundFont, timbre, volume, pânico e saída de áudio.
//! - `app`: liga tudo à casca e roteia MIDI → Espelho + som a cada quadro, com um relógio único.
//!
//! Sem calibração real ainda: a faixa do Espelho é a padrão de teste até a F01 medir o REVAS.

pub mod abertura;
pub mod acoes;
pub mod acordes;
pub mod app;
pub mod baixar_sf2;
pub mod calibracao;
pub mod chamada_app;
pub mod cursos;
pub mod dados_usuario;
pub mod dedos;
pub mod espelho;
pub mod estado;
pub mod exercicios;
pub mod gravacao;
pub mod icones;
pub mod licoes;
pub mod monitor;
pub mod mpv;
pub mod opcoes;
pub mod partitura_visual;
pub mod perfis;
pub mod player_aula;
pub mod preferencias;
pub mod professor;
pub mod professor_falante;
pub mod retratos;
pub mod revisao;
pub mod ritmo;
pub mod roda_bend;
pub mod roteiro_aula;
pub mod seletor_instrumento;
pub mod sincronia;
pub mod som;
pub mod teclado_visual;
pub mod tela_acordes;
pub mod tela_calibracao;
pub mod tela_cursos;
pub mod tela_espelho;
pub mod tela_exercicios;
pub mod tela_gravacao;
pub mod tela_inicio;
pub mod tela_monitor;
pub mod tela_partitura;
pub mod tela_player_aula;
pub mod tela_revisao;
pub mod tela_ritmos;
pub mod tela_timbres;
pub mod treino_andamento;
pub mod video_embutido;

/// `tokens.json` do app (formato lido pela casca).
pub const TOKENS_JSON: &str = include_str!("../dados/tokens.json");
/// `temas.json` do app: Noite, Claro e Ébano.
pub const TEMAS_JSON: &str = include_str!("../dados/temas.json");
