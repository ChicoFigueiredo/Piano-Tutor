//! Página inicial: cabeçalho com o estado do teclado e do som e um painel de cartões para as
//! áreas do app (praticar, música, teclado e som). Cada cartão abre a sua rota.

use crate::estado::{
    ROTA_ACORDES, ROTA_CALIBRACAO, ROTA_EXERCICIOS, ROTA_GRAVACAO, ROTA_MONITOR, ROTA_PARTITURA,
    ROTA_RITMOS, ROTA_TECLADO, ROTA_TIMBRES,
};
use crate::icones;
use bancada_casca::egui::{
    self, Color32, CornerRadius, FontId, RichText, Sense, Stroke, StrokeKind, Ui, vec2,
};
use bancada_casca::{TemaAtivo, abrir};

/// Um cartão da página inicial.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cartao {
    pub rota: &'static str,
    pub icone: &'static str,
    pub titulo: &'static str,
    pub texto: &'static str,
}

/// As seções da página, na ordem, com os seus cartões.
pub const SECOES: [(&str, &[Cartao]); 3] = [
    (
        "Praticar",
        &[
            Cartao {
                rota: ROTA_EXERCICIOS,
                icone: "exercicios",
                titulo: "Exercícios",
                texto: "Dez exercícios progressivos, com correção na hora e progresso salvo.",
            },
            Cartao {
                rota: ROTA_ACORDES,
                icone: "acordes",
                titulo: "Acordes e escalas",
                texto: "O que estou tocando, dicionário de acordes, escalas e desafio.",
            },
            Cartao {
                rota: ROTA_RITMOS,
                icone: "ritmos",
                titulo: "Ritmos",
                texto: "Metrônomo, bateria com padrões prontos, pads e exercício de pulso.",
            },
        ],
    ),
    (
        "Música",
        &[
            Cartao {
                rota: ROTA_PARTITURA,
                icone: "partitura",
                titulo: "Partitura",
                texto: "Abra um MusicXML em duas claves e toque com o cursor acompanhando.",
            },
            Cartao {
                rota: ROTA_GRAVACAO,
                icone: "gravar",
                titulo: "Gravar e ouvir",
                texto: "Grave o que você toca, ouça em outro timbre e salve em .mid.",
            },
            Cartao {
                rota: ROTA_TECLADO,
                icone: "teclado",
                titulo: "Teclado",
                texto: "Espelho das 61 teclas, com o teclado físico ou em simulação.",
            },
        ],
    ),
    (
        "Configurar",
        &[
            Cartao {
                rota: ROTA_TIMBRES,
                icone: "timbres",
                titulo: "Sons e timbres",
                texto: "SoundFont, divisão do teclado, camadas, efeitos e saída de áudio.",
            },
            Cartao {
                rota: ROTA_MONITOR,
                icone: "midi",
                titulo: "Monitor MIDI",
                texto: "Porta do teclado, conexão, eventos e log da sessão.",
            },
            Cartao {
                rota: ROTA_CALIBRACAO,
                icone: "calibrar",
                titulo: "Calibrar teclado",
                texto: "Mede as teclas, a força do toque e o pedal do teclado físico.",
            },
        ],
    ),
];

/// Cores da página: as do tema da casca ou, fora dela (testes), as do egui.
#[derive(Debug, Clone, Copy)]
pub struct Cores {
    pub acento: Color32,
    pub tinta: Color32,
    pub tinta_suave: Color32,
    pub superficie_1: Color32,
    pub superficie_3: Color32,
    pub borda: Color32,
    pub sucesso: Color32,
    pub aviso: Color32,
}

impl Cores {
    pub fn do_contexto(ctx: &egui::Context) -> Self {
        match TemaAtivo::paleta_do_contexto(ctx) {
            Some(p) => Self {
                acento: p.destaque,
                tinta: p.tinta,
                tinta_suave: p.tinta_suave,
                superficie_1: p.superficie_1,
                superficie_3: p.superficie_3,
                borda: p.borda,
                sucesso: p.sucesso,
                aviso: p.aviso,
            },
            None => {
                let v = ctx.global_style().visuals.clone();
                Self {
                    acento: v.selection.bg_fill,
                    tinta: v.text_color(),
                    tinta_suave: v.weak_text_color(),
                    superficie_1: v.panel_fill,
                    superficie_3: v.faint_bg_color,
                    borda: v.widgets.noninteractive.bg_stroke.color,
                    sucesso: Color32::from_rgb(98, 200, 120),
                    aviso: v.warn_fg_color,
                }
            }
        }
    }
}

/// Resumo do que está ligado, mostrado no cabeçalho.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resumo {
    pub midi: String,
    pub midi_ok: bool,
    pub som: String,
    pub som_ok: bool,
    pub instrumento: String,
}

/// Desenha a página.
pub fn desenhar_tela(ui: &mut Ui, resumo: &Resumo) {
    let p = Cores::do_contexto(ui.ctx());
    egui::ScrollArea::vertical()
        .id_salt("inicio")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(icones::glifo("piano"))
                        .size(40.0)
                        .color(p.acento),
                );
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Piano Tutor").size(28.0).strong());
                    ui.label(
                        RichText::new("Professor assistido de piano e teclado, local e sem nuvem.")
                            .color(p.tinta_suave),
                    );
                });
            });
            ui.add_space(14.0);
            ui.horizontal_wrapped(|ui| {
                pilula(ui, "midi", &resumo.midi, resumo.midi_ok, &p);
                pilula(ui, "volume", &resumo.som, resumo.som_ok, &p);
                pilula(ui, "piano", &resumo.instrumento, true, &p);
            });
            for (titulo, cartoes) in SECOES {
                ui.add_space(20.0);
                ui.label(
                    RichText::new(titulo.to_uppercase())
                        .size(12.0)
                        .strong()
                        .color(p.tinta_suave),
                );
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
                    for c in cartoes {
                        if cartao(ui, c, &p) {
                            abrir(ui.ctx(), c.rota);
                        }
                    }
                });
            }
            ui.add_space(20.0);
            ui.label(
                RichText::new(
                    "Atalhos: Ctrl+K paleta de ações · Ctrl+. Pânico · Ctrl+F1 recolhe o Ribbon · \
                     Ctrl+T alterna o tema · Alt mostra as letras do Ribbon",
                )
                .small()
                .color(p.tinta_suave),
            );
        });
}

fn pilula(ui: &mut Ui, icone: &str, texto: &str, ok: bool, p: &Cores) {
    let cor = if ok { p.sucesso } else { p.aviso };
    egui::Frame::new()
        .fill(p.superficie_3)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icones::glifo(icone)).size(14.0).color(cor));
                ui.label(RichText::new(texto).size(13.0));
            });
        });
}

/// Um cartão clicável. Devolve `true` no clique.
fn cartao(ui: &mut Ui, c: &Cartao, p: &Cores) -> bool {
    let tamanho = vec2(260.0, 112.0);
    let (rect, resp) = ui.allocate_exact_size(tamanho, Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if ui.is_rect_visible(rect) {
        let pintor = ui.painter();
        let fundo = if resp.hovered() {
            p.superficie_3
        } else {
            p.superficie_1
        };
        let borda = if resp.hovered() { p.acento } else { p.borda };
        pintor.rect(
            rect,
            CornerRadius::same(8),
            fundo,
            Stroke::new(1.0, borda),
            StrokeKind::Inside,
        );
        let faixa = egui::Rect::from_min_size(rect.min + vec2(0.0, 12.0), vec2(3.0, 28.0));
        if resp.hovered() {
            pintor.rect_filled(faixa, CornerRadius::same(2), p.acento);
        }
        pintor.text(
            rect.min + vec2(18.0, 14.0),
            egui::Align2::LEFT_TOP,
            icones::glifo(c.icone),
            FontId::proportional(26.0),
            p.acento,
        );
        pintor.text(
            rect.min + vec2(56.0, 18.0),
            egui::Align2::LEFT_TOP,
            c.titulo,
            FontId::proportional(16.0),
            p.tinta,
        );
        let galeria = pintor.layout(
            c.texto.to_owned(),
            FontId::proportional(13.0),
            p.tinta_suave,
            tamanho.x - 36.0,
        );
        pintor.galley(rect.min + vec2(18.0, 54.0), galeria, Color32::PLACEHOLDER);
    }
    resp.clicked()
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn cartoes_tem_rotas_unicas_e_icones_existentes() {
        let mut rotas = HashSet::new();
        for (_, cartoes) in SECOES {
            for c in cartoes {
                assert!(rotas.insert(c.rota), "rota repetida {}", c.rota);
                assert!(!icones::glifo(c.icone).is_empty(), "ícone {}", c.icone);
            }
        }
        assert_eq!(rotas.len(), 9);
    }
}
