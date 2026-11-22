//! Tela Calibração (F01): assistente passo a passo para medir o teclado físico.

use crate::calibracao::{Calibracao, Etapa, Suporte};
use bancada_casca::egui::{self, Button, Color32, RichText, ScrollArea, Ui, vec2};

const VERDE: Color32 = Color32::from_rgb(70, 200, 110);
const AMBAR: Color32 = Color32::from_rgb(255, 196, 70);

/// O que a tela pediu ao app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AcaoCalibracao {
    /// Salvar o perfil, gravar o relatório e usar a faixa no Espelho.
    SalvarEUsar,
    /// Voltar à faixa padrão (apaga o perfil salvo).
    Esquecer,
}

/// Informação de contexto para a tela.
pub struct ContextoCalibracao<'a> {
    pub conectado: Option<&'a str>,
    pub perfil_em_uso: Option<String>,
    pub mensagem: Option<&'a str>,
}

pub fn desenhar_tela(
    ui: &mut Ui,
    k: &mut Calibracao,
    ctx: &ContextoCalibracao<'_>,
) -> Vec<AcaoCalibracao> {
    let mut pedidos = Vec::new();
    ScrollArea::vertical()
        .id_salt("calibracao-vertical")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading("Calibração do teclado");
            match ctx.conectado {
                Some(p) => ui.label(RichText::new(format!("Teclado conectado: {p}")).color(VERDE)),
                None => ui.label(
                    RichText::new(
                        "Nenhum teclado conectado. Abra o Monitor MIDI, escolha a porta do \
                         teclado e clique em Conectar; depois volte aqui.",
                    )
                    .color(AMBAR),
                ),
            };
            if let Some(p) = &ctx.perfil_em_uso {
                ui.label(format!("Perfil em uso: {p}"));
            }
            ui.add_space(8.0);
            desenhar_etapas(ui, k);
            ui.add_space(8.0);
            ui.separator();
            desenhar_etapa_atual(ui, k, &mut pedidos);
            if let Some(a) = &k.aviso {
                ui.add_space(6.0);
                ui.label(RichText::new(a).color(AMBAR).strong());
            }
            if let Some(m) = ctx.mensagem {
                ui.add_space(6.0);
                ui.label(RichText::new(m).weak());
            }
            ui.add_space(12.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Recomeçar calibração").clicked() {
                    k.reiniciar();
                }
                if ctx.perfil_em_uso.is_some()
                    && ui
                        .button("Esquecer perfil (voltar à faixa padrão)")
                        .clicked()
                {
                    pedidos.push(AcaoCalibracao::Esquecer);
                }
            });
        });
    pedidos
}

fn desenhar_etapas(ui: &mut Ui, k: &Calibracao) {
    const NOMES: [&str; 6] = [
        "Tecla mais grave",
        "Tecla mais aguda",
        "Conferir as 61 teclas",
        "Força do toque (velocity)",
        "Pedal",
        "Salvar",
    ];
    ui.horizontal_wrapped(|ui| {
        for (i, n) in NOMES.iter().enumerate() {
            let num = i + 1;
            let atual = k.etapa.numero();
            let (marca, cor) = if num < atual {
                ("ok", VERDE)
            } else if num == atual {
                (">", AMBAR)
            } else {
                ("  ", ui.visuals().weak_text_color())
            };
            ui.label(RichText::new(format!("[{marca}] {num}. {n}")).color(cor));
            ui.add_space(8.0);
        }
    });
}

fn instrucao(ui: &mut Ui, t: &str) {
    ui.label(RichText::new(t).size(20.0).strong());
}

fn desenhar_etapa_atual(ui: &mut Ui, k: &mut Calibracao, pedidos: &mut Vec<AcaoCalibracao>) {
    match k.etapa {
        Etapa::Grave => {
            instrucao(
                ui,
                "Toque a tecla MAIS GRAVE do teclado (a da ponta esquerda).",
            );
            ui.label("Deixe a transposição e a oitava do teclado no padrão de fábrica.");
        }
        Etapa::Aguda => {
            instrucao(ui, "Agora toque a tecla MAIS AGUDA (a da ponta direita).");
            if let Some(g) = k.grave {
                ui.label(format!("Mais grave medida: MIDI {g}."));
            }
            if ui.button("Refazer a mais grave").clicked() {
                k.refazer_faixa();
            }
        }
        Etapa::Cobertura => {
            instrucao(
                ui,
                "Toque cada uma das 61 teclas, da esquerda para a direita (brancas e pretas).",
            );
            ui.label(format!(
                "Conferidas: {} de 61 · liberação recebida em {}",
                k.conferidas(),
                k.liberacoes()
            ));
            desenhar_mapa(ui, k);
            ui.horizontal(|ui| {
                let texto = if k.conferidas() == 61 {
                    "Continuar"
                } else {
                    "Continuar sem conferir todas"
                };
                if ui.button(texto).clicked() {
                    k.avancar();
                }
                if ui.button("Refazer a faixa").clicked() {
                    k.refazer_faixa();
                }
            });
        }
        Etapa::Velocidade => {
            instrucao(
                ui,
                "Toque uma tecla bem de leve, depois média, depois bem forte (repita algumas vezes).",
            );
            match k.faixa_velocidade() {
                Some((a, b)) => ui.label(format!(
                    "Velocity observada: de {a} a {b} em {} notas.",
                    k.notas_velocidade()
                )),
                None => ui.label("Nenhuma nota ainda."),
            };
            ui.label(format!("Veredito: {}", k.suporte_velocity().texto()));
            if ui.button("Continuar").clicked() {
                k.avancar();
            }
        }
        Etapa::Pedal => {
            instrucao(ui, "Se houver pedal de sustain ligado, pise e solte.");
            match k.pedal {
                Suporte::Sim => {
                    ui.label(RichText::new("Pedal recebido (CC64).").color(VERDE));
                    if ui.button("Continuar").clicked() {
                        k.avancar();
                    }
                }
                _ => {
                    if ui.button("Não tenho pedal / não respondeu").clicked() {
                        k.sem_pedal();
                    }
                }
            }
        }
        Etapa::Concluida => {
            instrucao(ui, "Calibração concluída.");
            match k.perfil() {
                Some(p) => {
                    egui::Grid::new("resumo-calibracao").show(ui, |ui| {
                        ui.label("Faixa");
                        ui.label(format!("MIDI {} a {} (61 teclas)", p.grave, p.aguda()));
                        ui.end_row();
                        ui.label("Teclas conferidas");
                        ui.label(format!("{} de 61", p.conferidas));
                        ui.end_row();
                        ui.label("Velocity");
                        ui.label(p.velocity.texto());
                        ui.end_row();
                        ui.label("Pedal");
                        ui.label(p.pedal.texto());
                        ui.end_row();
                    });
                    ui.add_space(6.0);
                    if ui
                        .add(
                            Button::new(
                                RichText::new("Salvar e usar este perfil")
                                    .strong()
                                    .color(Color32::BLACK),
                            )
                            .fill(VERDE)
                            .min_size(vec2(200.0, 32.0)),
                        )
                        .clicked()
                    {
                        pedidos.push(AcaoCalibracao::SalvarEUsar);
                    }
                }
                None => {
                    ui.label("A faixa de 61 teclas não foi medida. Recomece a calibração.");
                }
            }
        }
    }
}

fn desenhar_mapa(ui: &mut Ui, k: &Calibracao) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
        for (n, conferida, _) in k.mapa() {
            let preta = matches!(n % 12, 1 | 3 | 6 | 8 | 10);
            let fundo = if conferida {
                VERDE
            } else if preta {
                Color32::from_rgb(40, 40, 48)
            } else {
                Color32::from_rgb(220, 220, 215)
            };
            let tinta = if conferida || !preta {
                Color32::BLACK
            } else {
                Color32::WHITE
            };
            let ultima = k.ultima_nota == Some(n);
            let b = Button::new(RichText::new(n.to_string()).size(10.0).color(tinta))
                .fill(fundo)
                .min_size(vec2(26.0, 26.0))
                .stroke(if ultima {
                    egui::Stroke::new(2.5, AMBAR)
                } else {
                    egui::Stroke::NONE
                });
            ui.add(b);
        }
    });
}
