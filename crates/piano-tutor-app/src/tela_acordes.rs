//! Tela Acordes: "o que toquei?", dicionário de acordes, escalas, campo harmônico e desafio.

use crate::acordes::{
    Conjunto, EstadoAcordes, FonteTreino, Modo, rotulo_da_tonalidade, tonalidades_disponiveis,
};
use crate::espelho::EstadoEspelho;
use crate::ritmo::EstadoRitmo;
use crate::som::ServicoSom;
use crate::tela_espelho::{desenhar_teclado_embutido, texto_tela};
use crate::treino_andamento::{Momento, Ordem};
use bancada_casca::egui::{Button, Color32, ComboBox, RichText, ScrollArea, Slider, Ui, vec2};
use piano_tutor_core::NotaMidi;
use piano_tutor_core::ritmo::{BPM_MAX, BPM_MIN, Formula, Padrao};
use piano_tutor_core::teclado::rotulo;
use piano_tutor_core::teoria::{
    ClasseAltura, GRAUS_ROMANOS, Qualidade, TipoEscala, campo_harmonico_maior,
};
use std::time::Duration;

const VERDE: Color32 = Color32::from_rgb(70, 200, 110);

fn ouvir_juntas(som: &mut ServicoSom, notas: &[NotaMidi]) {
    let t = som.agora_ns() + 30_000_000;
    som.demonstrar(notas, t, Duration::from_millis(1400));
}

fn ouvir_em_sequencia(som: &mut ServicoSom, notas: &[NotaMidi], passo_ms: u64) {
    let t0 = som.agora_ns() + 30_000_000;
    for (i, n) in notas.iter().enumerate() {
        som.demonstrar(
            &[*n],
            t0 + i as u64 * passo_ms * 1_000_000,
            Duration::from_millis(passo_ms.saturating_sub(20)),
        );
    }
}

pub fn desenhar_tela(
    ui: &mut Ui,
    a: &mut EstadoAcordes,
    espelho: &mut EstadoEspelho,
    som: &mut ServicoSom,
    ritmo: &mut EstadoRitmo,
) {
    ScrollArea::vertical()
        .id_salt("acordes-vertical")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading("Acordes e escalas");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut a.modo, Modo::Dicionario, "Dicionário de acordes");
                ui.selectable_value(&mut a.modo, Modo::Escalas, "Escalas e campo harmônico");
                if ui
                    .selectable_value(&mut a.modo, Modo::Desafio, "Desafio: toque o acorde")
                    .clicked()
                    && a.alvo.is_none()
                {
                    a.semear(som.agora_ns());
                    a.sortear(som.agora_ns());
                }
                ui.selectable_value(&mut a.modo, Modo::Treino, "Treino com andamento");
            });
            if a.modo != Modo::Treino && a.treino.ativo() {
                a.treino.parar(ritmo, som);
            }
            ui.add_space(6.0);
            escolher_tonalidade(ui, a);
            ui.add_space(6.0);
            // O que está sendo tocado agora, em qualquer modo: o gesto (nota, acorde, arpejo
            // ou sequência) sobre os ataques recentes. Gesto ambíguo não mostra rótulo.
            let ligadas: Vec<NotaMidi> = espelho.teclas().notas_ligadas().collect();
            a.observar(&ligadas, som.agora_ns());
            let tocando = a.texto_do_gesto().unwrap_or_else(|| {
                "Toque notas para ver o que foi tocado: nota, acorde, arpejo ou sequência."
                    .to_owned()
            });
            ui.label(RichText::new(tocando).size(20.0).strong());
            ui.add_space(6.0);
            match a.modo {
                Modo::Dicionario => desenhar_dicionario(ui, a, som),
                Modo::Escalas => desenhar_escalas(ui, a, som),
                Modo::Desafio => desenhar_desafio(ui, a, som),
                Modo::Treino => desenhar_treino(ui, a, som, ritmo),
            }
            ui.add_space(8.0);
            espelho.definir_guias(a.guias(), Vec::new());
            desenhar_teclado_embutido(ui, espelho);
            ui.label(
                RichText::new("Bolinha verde = tecla a tocar. Azul = tecla que você está tocando.")
                    .weak(),
            );
        });
}

/// Seletor de tonalidade (maiores e menores): fixa a grafia enarmônica dos nomes da tela e
/// mostra a armadura.
fn escolher_tonalidade(ui: &mut Ui, a: &mut EstadoAcordes) {
    ui.horizontal_wrapped(|ui| {
        let atual = a
            .tonalidade
            .map_or_else(|| "sem tonalidade".to_owned(), rotulo_da_tonalidade);
        ComboBox::from_id_salt("tonalidade-grafia")
            .selected_text(format!("Tonalidade: {atual}"))
            .height(360.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut a.tonalidade, None, "Sem tonalidade (grafia padrão)");
                for t in tonalidades_disponiveis() {
                    ui.selectable_value(&mut a.tonalidade, Some(t), rotulo_da_tonalidade(t));
                }
            });
        match a.armadura() {
            Some(texto) => {
                ui.label(texto);
            }
            None => {
                ui.label(
                    RichText::new(
                        "Escolha uma tonalidade para ver a armadura e a grafia (Fá# ou Solb).",
                    )
                    .weak(),
                );
            }
        }
    });
}

fn escolher_fundamental(ui: &mut Ui, a: &mut EstadoAcordes) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Tônica:");
        for v in 0..12u8 {
            let c = ClasseAltura::nova(v);
            let texto = format!("{} ({})", a.nome_da_classe(c), a.nome_pt_da_classe(c));
            ui.selectable_value(&mut a.fundamental, c, texto);
        }
    });
}

fn escolher_oitava(ui: &mut Ui, a: &mut EstadoAcordes) {
    ui.horizontal(|ui| {
        ui.label("Região:");
        let mut oitava = i32::from(a.base / 12) - 1;
        if ui
            .add(Slider::new(&mut oitava, 2..=6).prefix("oitava "))
            .changed()
        {
            a.base = u8::try_from((oitava + 1) * 12).unwrap_or(60);
        }
    });
}

fn desenhar_dicionario(ui: &mut Ui, a: &mut EstadoAcordes, som: &mut ServicoSom) {
    escolher_fundamental(ui, a);
    ui.horizontal_wrapped(|ui| {
        ui.label("Tipo:");
        for q in Qualidade::TODAS {
            let texto = format!("{}{}", a.nome_da_classe(a.fundamental), q.sufixo());
            if ui.selectable_value(&mut a.qualidade, q, texto).clicked() {
                a.inversao = 0;
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("Inversão:");
        let n = a.qualidade.intervalos().len();
        for i in 0..n {
            let t = if i == 0 {
                "fundamental".to_owned()
            } else {
                format!("{i}a inversão")
            };
            ui.selectable_value(&mut a.inversao, i, t);
        }
    });
    escolher_oitava(ui, a);
    let acorde = a.acorde();
    let notas = a.notas_do_acorde();
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(a.cifra_do_acorde(&acorde))
                .size(30.0)
                .strong()
                .color(VERDE),
        );
        ui.vertical(|ui| {
            ui.label(RichText::new(a.nome_do_acorde(&acorde)).strong());
            ui.label(format!("Notas: {}", nomes(a, &notas)));
            let formula: Vec<String> = a.qualidade.intervalos().iter().map(|i| grau(*i)).collect();
            ui.label(format!("Fórmula: {}", formula.join(" - ")));
        });
        ui.add_space(12.0);
        if ui
            .add(Button::new("Ouvir").min_size(vec2(80.0, 30.0)))
            .clicked()
        {
            ouvir_juntas(som, &notas);
        }
        if ui.button("Arpejo").clicked() {
            ouvir_em_sequencia(som, &notas, 280);
        }
    });
}

fn desenhar_escalas(ui: &mut Ui, a: &mut EstadoAcordes, som: &mut ServicoSom) {
    escolher_fundamental(ui, a);
    ui.horizontal_wrapped(|ui| {
        ui.label("Escala:");
        for t in TipoEscala::TODAS {
            ui.selectable_value(&mut a.escala, t, t.nome());
        }
    });
    escolher_oitava(ui, a);
    let notas = a.notas_da_escala();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!(
                "{} {}",
                a.nome_pt_da_classe(a.fundamental),
                a.escala.nome()
            ))
            .size(22.0)
            .strong()
            .color(VERDE),
        );
        if ui.button("Ouvir subindo").clicked() {
            ouvir_em_sequencia(som, &notas, 330);
        }
        if ui.button("Subir e descer").clicked() {
            let mut v = notas.clone();
            v.extend(notas.iter().rev().skip(1));
            ouvir_em_sequencia(som, &v, 300);
        }
    });
    ui.label(format!("Notas: {}", nomes(a, &notas)));
    ui.add_space(8.0);
    ui.label(
        RichText::new(format!(
            "Campo harmônico de {} maior (clique para ouvir e ver no teclado)",
            a.nome_da_classe(a.fundamental)
        ))
        .strong(),
    );
    ui.horizontal_wrapped(|ui| {
        for (i, ac) in campo_harmonico_maior(a.fundamental).iter().enumerate() {
            if ui
                .add(
                    Button::new(format!("{}  {}", GRAUS_ROMANOS[i], a.cifra_do_acorde(ac)))
                        .min_size(vec2(70.0, 30.0)),
                )
                .clicked()
            {
                a.fundamental = ac.fundamental;
                a.qualidade = ac.qualidade;
                a.inversao = 0;
                a.modo = Modo::Dicionario;
                let n = a.notas_do_acorde();
                ouvir_juntas(som, &n);
            }
        }
    });
}

fn desenhar_desafio(ui: &mut Ui, a: &mut EstadoAcordes, som: &mut ServicoSom) {
    ui.horizontal_wrapped(|ui| {
        ComboBox::from_id_salt("conjunto-desafio")
            .selected_text(format!("Acordes: {}", a.conjunto.texto()))
            .show_ui(ui, |ui| {
                for c in Conjunto::TODOS {
                    ui.selectable_value(&mut a.conjunto, c, c.texto());
                }
            });
        if a.conjunto == Conjunto::CampoHarmonico {
            ComboBox::from_id_salt("tonalidade-desafio")
                .selected_text(format!("Tônica: {}", a.nome_da_classe(a.fundamental)))
                .show_ui(ui, |ui| {
                    for v in 0..12u8 {
                        let c = ClasseAltura::nova(v);
                        let texto = a.nome_da_classe(c);
                        ui.selectable_value(&mut a.fundamental, c, texto);
                    }
                });
        }
        ui.checkbox(&mut a.de_ouvido, "De ouvido (esconde o nome)");
        ui.checkbox(&mut a.mostrar_dica, "Mostrar dica no teclado");
    });
    let Some(alvo) = a.alvo else {
        if ui.button("Começar").clicked() {
            a.sortear(som.agora_ns());
        }
        return;
    };
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Toque:").size(22.0));
        let texto = if a.de_ouvido {
            "?".to_owned()
        } else {
            a.cifra_do_acorde(&alvo)
        };
        ui.label(RichText::new(texto).size(40.0).strong().color(VERDE));
        if !a.de_ouvido {
            ui.label(RichText::new(a.nome_do_acorde(&alvo)).size(18.0));
        }
        ui.add_space(12.0);
        if ui
            .add(Button::new("Ouvir").min_size(vec2(80.0, 30.0)))
            .clicked()
        {
            let n = a.notas_do_alvo();
            ouvir_juntas(som, &n);
        }
        if ui.button("Pular").clicked() {
            let agora = som.agora_ns();
            a.pular(agora);
            if a.de_ouvido {
                let n = a.notas_do_alvo();
                ouvir_juntas(som, &n);
            }
        }
    });
    if a.aguardando_soltar {
        ui.label(
            RichText::new("Certo! Solte as teclas para o próximo.")
                .color(VERDE)
                .size(18.0)
                .strong(),
        );
    }
    ui.label(format!(
        "Acertos: {} · pulados: {}{}",
        a.acertos,
        a.pulados,
        a.ultimo
            .as_ref()
            .map(|r| format!(
                " · último: {} em {:.1} s",
                a.cifra_do_acorde(&r.alvo),
                r.segundos
            ))
            .unwrap_or_default()
    ));
    ui.label(
        RichText::new(
            "Vale qualquer inversão e notas dobradas em outra oitava; não pode ter nota a mais.",
        )
        .weak(),
    );
}

/// Treino com andamento: sequência de acordes no tempo do metrônomo (ou de um ritmo).
fn desenhar_treino(
    ui: &mut Ui,
    a: &mut EstadoAcordes,
    som: &mut ServicoSom,
    ritmo: &mut EstadoRitmo,
) {
    let ativo = a.treino.ativo();
    ui.add_enabled_ui(!ativo, |ui| {
        ui.horizontal_wrapped(|ui| {
            ComboBox::from_id_salt("fonte-treino")
                .selected_text(a.fonte_treino.texto())
                .height(360.0)
                .show_ui(ui, |ui| {
                    for f in FonteTreino::todas() {
                        ui.selectable_value(&mut a.fonte_treino, f, f.texto());
                    }
                });
            if a.fonte_treino.usa_tonica() {
                ComboBox::from_id_salt("tonica-treino")
                    .selected_text(format!("Tônica: {}", a.nome_da_classe(a.fundamental)))
                    .show_ui(ui, |ui| {
                        for v in 0..12u8 {
                            let c = ClasseAltura::nova(v);
                            let texto = a.nome_da_classe(c);
                            ui.selectable_value(&mut a.fundamental, c, texto);
                        }
                    });
            }
            let cfg = &mut a.treino.config;
            ComboBox::from_id_salt("ordem-treino")
                .selected_text(format!("Ordem: {}", cfg.ordem.texto()))
                .show_ui(ui, |ui| {
                    for o in [Ordem::Sequencial, Ordem::Aleatoria] {
                        ui.selectable_value(&mut cfg.ordem, o, o.texto());
                    }
                });
            ComboBox::from_id_salt("compassos-treino")
                .selected_text(format!(
                    "{} compasso{} por acorde",
                    cfg.compassos_por_item,
                    if cfg.compassos_por_item > 1 { "s" } else { "" }
                ))
                .show_ui(ui, |ui| {
                    for c in 1..=4u8 {
                        ui.selectable_value(&mut cfg.compassos_por_item, c, c.to_string());
                    }
                });
            ComboBox::from_id_salt("voltas-treino")
                .selected_text(match cfg.voltas {
                    None => "Repetir sem fim".to_owned(),
                    Some(v) => format!("{v} volta{}", if v > 1 { "s" } else { "" }),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut cfg.voltas, None, "Repetir sem fim");
                    for v in [1u32, 2, 4, 8] {
                        ui.selectable_value(&mut cfg.voltas, Some(v), format!("{v}"));
                    }
                });
            ComboBox::from_id_salt("contagem-treino")
                .selected_text(format!("Contagem: {} compasso(s)", cfg.contagem))
                .show_ui(ui, |ui| {
                    for c in 0..=2u8 {
                        ui.selectable_value(&mut cfg.contagem, c, c.to_string());
                    }
                });
        });
        escolher_oitava(ui, a);
        ui.horizontal_wrapped(|ui| {
            let cfg = &mut a.treino.config;
            ui.checkbox(&mut cfg.app_toca, "O app toca o acorde");
            ui.checkbox(&mut cfg.mostrar_teclas, "Mostrar as teclas no teclado");
            ui.checkbox(&mut ritmo.config.metronomo, "Metrônomo");
            escolher_acompanhamento(ui, ritmo);
        });
    });
    ui.horizontal(|ui| {
        ui.label("Andamento:");
        let mut bpm = ritmo.bpm();
        if ui
            .add(Slider::new(&mut bpm, BPM_MIN..=BPM_MAX).suffix(" BPM"))
            .changed()
        {
            ritmo.definir_bpm(bpm);
        }
        ui.add_space(12.0);
        let (texto, cor) = if ativo {
            ("Parar", Color32::from_rgb(220, 90, 80))
        } else {
            ("Tocar", VERDE)
        };
        if ui
            .add(
                Button::new(
                    RichText::new(texto)
                        .size(20.0)
                        .strong()
                        .color(Color32::BLACK),
                )
                .fill(cor)
                .min_size(vec2(120.0, 36.0)),
            )
            .clicked()
        {
            if ativo {
                a.treino.parar(ritmo, som);
            } else {
                let itens = a.itens_do_treino();
                a.treino.definir_itens(itens);
                a.treino.semear(som.agora_ns());
                a.treino.tocar(ritmo, som);
            }
        }
    });
    ui.add_space(6.0);
    let pulso_agora = ritmo.posicao(som.agora_ns()).map(|p| p.pulso);
    let momento = a.treino.momento();
    if momento == Momento::Parado {
        let lista: Vec<String> = a
            .itens_do_treino()
            .iter()
            .map(|i| i.rotulo.clone())
            .collect();
        ui.label(format!("Sequência: {}", lista.join("  ")));
        ui.label(
            RichText::new(
                "Tocar liga o metrônomo (ou o ritmo escolhido) e troca de acorde no 1 de cada \
                 compasso. Toque cada acorde no tempo: vale qualquer inversão.",
            )
            .weak(),
        );
    } else {
        // Uma linha de altura fixa (pulso, situação, acorde, próximo) para o teclado não pular.
        ui.horizontal(|ui| {
            ui.set_min_height(56.0);
            let total = ritmo.padrao.formula.pulsos();
            for i in 0..total {
                let cor = match pulso_agora {
                    Some(p) if p == i && a.treino.ativo() => VERDE,
                    _ => Color32::from_gray(90),
                };
                let (r, _) =
                    ui.allocate_exact_size(vec2(26.0, 26.0), bancada_casca::egui::Sense::hover());
                ui.painter()
                    .circle_filled(r.center(), if i == 0 { 11.0 } else { 8.0 }, cor);
            }
            ui.add_space(8.0);
            let situacao = match momento {
                Momento::Contagem { pulso, pulsos } => format!("Prepare-se: {pulso} de {pulsos}"),
                Momento::Item { vaga } => match a.treino.total_de_vagas() {
                    Some(t) => format!("Acorde {} de {t}", vaga + 1),
                    None => format!("Acorde {}", vaga + 1),
                },
                _ => "Fim do treino".to_owned(),
            };
            ui.add_sized(
                vec2(190.0, 30.0),
                bancada_casca::egui::Label::new(RichText::new(situacao).size(18.0).strong()),
            );
            if let Some(i) = a.treino.atual() {
                ui.label(RichText::new(&i.rotulo).size(44.0).strong().color(VERDE));
                ui.label(RichText::new(&i.detalhe).size(18.0));
            }
            if let Some(p) = a.treino.proximo() {
                ui.add_space(24.0);
                ui.label(
                    RichText::new(format!("Depois: {}", p.rotulo))
                        .size(20.0)
                        .weak(),
                );
            }
        });
        let pl = a.treino.placar;
        let ultimo = a
            .treino
            .texto_ultimo()
            .map(|t| format!("   ·   {t}"))
            .unwrap_or_default();
        ui.label(format!(
            "No tempo: {} · atrasados: {} · não vieram: {}{}{ultimo}",
            pl.no_tempo,
            pl.atrasados,
            pl.errados,
            pl.aproveitamento()
                .map(|x| format!(" · aproveitamento {x}%"))
                .unwrap_or_default()
        ));
    }
}

/// Só metrônomo (escolhe o compasso) ou um dos ritmos de fábrica (o compasso vem dele).
fn escolher_acompanhamento(ui: &mut Ui, ritmo: &mut EstadoRitmo) {
    let atual = if ritmo.config.bateria {
        ritmo.padrao.nome.clone()
    } else {
        "Só metrônomo".to_owned()
    };
    let mut escolha: Option<Option<Padrao>> = None;
    ComboBox::from_id_salt("acompanhamento-treino")
        .selected_text(format!("Ritmo: {atual}"))
        .height(360.0)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(!ritmo.config.bateria, "Só metrônomo")
                .clicked()
            {
                escolha = Some(None);
            }
            for p in &ritmo.fabrica {
                if ui
                    .selectable_label(
                        ritmo.config.bateria && ritmo.padrao.nome == p.nome,
                        format!("{} ({})", p.nome, p.formula.texto()),
                    )
                    .clicked()
                {
                    escolha = Some(Some(p.clone()));
                }
            }
        });
    let bpm = ritmo.bpm();
    match escolha {
        Some(Some(p)) => {
            ritmo.escolher_padrao(p);
            ritmo.definir_bpm(bpm);
            ritmo.config.bateria = true;
        }
        Some(None) => {
            ritmo.config.bateria = false;
        }
        None => {}
    }
    if !ritmo.config.bateria {
        let mut f = ritmo.padrao.formula;
        ComboBox::from_id_salt("compasso-treino")
            .selected_text(format!("Compasso {}", f.texto()))
            .show_ui(ui, |ui| {
                for x in Formula::TODAS {
                    ui.selectable_value(&mut f, x, x.texto());
                }
            });
        if f != ritmo.padrao.formula {
            ritmo.escolher_padrao(Padrao::vazio("Metrônomo", f, 1, bpm));
        }
    }
}

fn nomes(a: &EstadoAcordes, notas: &[NotaMidi]) -> String {
    if a.tonalidade.is_some() {
        return a.nomes_das_notas(notas);
    }
    notas
        .iter()
        .map(|n| texto_tela(&rotulo(*n, Default::default())))
        .collect::<Vec<_>>()
        .join(" ")
}

fn grau(semitons: u8) -> String {
    const G: [&str; 12] = [
        "1", "b2", "2", "b3", "3", "4", "b5", "5", "#5", "6", "b7", "7",
    ];
    if semitons >= 12 {
        (match semitons - 12 {
            2 => "9",
            x => G[usize::from(x % 12)],
        })
        .to_string()
    } else {
        G[usize::from(semitons)].to_owned()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::som::apoio_testes::servico_pronto;
    use bancada_casca::egui::{Context, FontId, RawInput, Rect, pos2, vec2};
    use piano_tutor_audio::RelogioManual;
    use std::sync::Arc;

    fn quadro(ctx: &Context, a: &mut EstadoAcordes, e: &mut EstadoEspelho, som: &mut ServicoSom) {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
            ..Default::default()
        };
        ctx.run_ui(entrada, |ui| {
            desenhar_tela(ui, a, e, som, &mut EstadoRitmo::novo(None))
        })
        .drop_without_applying_deltas();
    }

    #[test]
    fn desenha_cada_modo_com_servico_inativo_e_pronto() {
        let ctx = Context::default();
        let mut a = EstadoAcordes::default();
        let mut e = EstadoEspelho::novo();
        let mut inativo = ServicoSom::inativo(Arc::new(RelogioManual::novo()));
        let (mut pronto, _m) = servico_pronto(Arc::new(RelogioManual::novo()));
        for modo in [Modo::Dicionario, Modo::Escalas, Modo::Desafio, Modo::Treino] {
            a.modo = modo;
            for _ in 0..2 {
                quadro(&ctx, &mut a, &mut e, &mut inativo);
                quadro(&ctx, &mut a, &mut e, &mut pronto);
            }
        }
    }

    #[test]
    fn desenha_dicionario_com_cada_qualidade_e_inversao() {
        let ctx = Context::default();
        let mut a = EstadoAcordes::default();
        let mut e = EstadoEspelho::novo();
        let (mut som, _m) = servico_pronto(Arc::new(RelogioManual::novo()));
        a.modo = Modo::Dicionario;
        for q in Qualidade::TODAS {
            a.qualidade = q;
            for i in 0..q.intervalos().len() {
                a.inversao = i;
                quadro(&ctx, &mut a, &mut e, &mut som);
            }
        }
    }

    #[test]
    fn desenha_escalas_e_campo_harmonico_de_cada_tipo() {
        let ctx = Context::default();
        let mut a = EstadoAcordes::default();
        let mut e = EstadoEspelho::novo();
        let (mut som, _m) = servico_pronto(Arc::new(RelogioManual::novo()));
        a.modo = Modo::Escalas;
        for t in TipoEscala::TODAS {
            a.escala = t;
            quadro(&ctx, &mut a, &mut e, &mut som);
        }
    }

    #[test]
    fn desenha_desafio_em_cada_conjunto_com_e_sem_alvo() {
        let ctx = Context::default();
        let mut a = EstadoAcordes::default();
        let mut e = EstadoEspelho::novo();
        let (mut som, _m) = servico_pronto(Arc::new(RelogioManual::novo()));
        a.modo = Modo::Desafio;
        quadro(&ctx, &mut a, &mut e, &mut som);
        for c in Conjunto::TODOS {
            a.conjunto = c;
            a.semear(som.agora_ns());
            a.sortear(som.agora_ns());
            for de_ouvido in [false, true] {
                a.de_ouvido = de_ouvido;
                quadro(&ctx, &mut a, &mut e, &mut som);
            }
        }
    }

    #[test]
    fn desenha_cada_modo_em_cada_tonalidade() {
        let ctx = Context::default();
        let mut a = EstadoAcordes::default();
        let mut e = EstadoEspelho::novo();
        let (mut som, _m) = servico_pronto(Arc::new(RelogioManual::novo()));
        a.semear(som.agora_ns());
        a.sortear(som.agora_ns());
        for t in std::iter::once(None).chain(
            crate::acordes::tonalidades_disponiveis()
                .into_iter()
                .map(Some),
        ) {
            a.tonalidade = t;
            for modo in [Modo::Dicionario, Modo::Escalas, Modo::Desafio, Modo::Treino] {
                a.modo = modo;
                quadro(&ctx, &mut a, &mut e, &mut som);
            }
        }
    }

    #[test]
    fn os_glifos_usados_nos_textos_das_telas_existem_na_fonte() {
        let ctx = Context::default();
        let mut a = EstadoAcordes::default();
        let mut e = EstadoEspelho::novo();
        let mut som = ServicoSom::inativo(Arc::new(RelogioManual::novo()));
        quadro(&ctx, &mut a, &mut e, &mut som);
        quadro(&ctx, &mut a, &mut e, &mut som);
        // Textos das três telas e do núcleo que as telas exibem (♯ e ♭ saem como # e b).
        let usados = format!(
            "·—…ªº°ÁÉÓÚàáâãçéêíóôõú{}{}",
            GRAUS_ROMANOS.concat(),
            "1ª inversão 2ª inversão 3ª inversão Arpejo ascendente descendente sequência bemóis"
        );
        let fonte = FontId::proportional(14.0);
        let faltam: String = usados
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .filter(|ch| !ctx.fonts_mut(|f| f.has_glyph(&fonte, *ch)))
            .collect();
        assert!(faltam.is_empty(), "glifos ausentes: {faltam:?}");
    }
}
