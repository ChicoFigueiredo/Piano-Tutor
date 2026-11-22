//! Tela Ritmos (F08, F09): metrônomo, bateria de 16 passos com padrões prontos, pads e o
//! exercício "toque no pulso".

use crate::ritmo::{EstadoRitmo, ExercicioPulso};
use crate::som::ServicoSom;
use bancada_casca::egui::{
    self, Button, Color32, ComboBox, Key, RichText, ScrollArea, Sense, Slider, Stroke, TextEdit,
    Ui, vec2,
};
use piano_tutor_core::ritmo::{
    BPM_MAX, BPM_MIN, Formula, INSTRUMENTOS, Julgamento, Subdivisao, VEL_ACENTO,
};

const VERDE: Color32 = Color32::from_rgb(70, 200, 110);
const AMBAR: Color32 = Color32::from_rgb(255, 196, 70);
const VERMELHO: Color32 = Color32::from_rgb(230, 90, 80);
const AZUL: Color32 = Color32::from_rgb(80, 150, 255);

/// Teclas do PC que tocam os 8 pads nesta tela.
const TECLAS_PADS: [Key; 8] = [
    Key::Num1,
    Key::Num2,
    Key::Num3,
    Key::Num4,
    Key::Num5,
    Key::Num6,
    Key::Num7,
    Key::Num8,
];

pub fn desenhar_tela(ui: &mut Ui, r: &mut EstadoRitmo, som: &mut ServicoSom) {
    // Pads pelo teclado do PC (sem campo de texto em foco).
    if !ui.ctx().egui_wants_keyboard_input() {
        let apertadas: Vec<usize> = ui.input(|i| {
            TECLAS_PADS
                .iter()
                .enumerate()
                .filter(|(_, k)| i.key_pressed(**k) && i.modifiers.is_none())
                .map(|(n, _)| n)
                .collect()
        });
        for n in apertadas {
            r.pad(som, n);
        }
        if ui.input(|i| i.key_pressed(Key::Space) && i.modifiers.is_none()) {
            alternar(r, som);
        }
    }

    ScrollArea::vertical()
        .id_salt("ritmos-vertical")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading("Ritmos");
            ui.label(
                "Metrônomo e bateria no mesmo relógio do piano. Toque junto no teclado. \
                 Espaço inicia/para; teclas 1 a 8 tocam os pads.",
            );
            ui.add_space(6.0);
            desenhar_transporte(ui, r, som);
            ui.add_space(6.0);
            desenhar_pulsos(ui, r, som);
            ui.add_space(6.0);
            ui.separator();
            desenhar_padroes(ui, r, som);
            ui.add_space(6.0);
            desenhar_grade(ui, r, som);
            ui.add_space(6.0);
            ui.separator();
            desenhar_exercicio(ui, r);
        });
}

fn alternar(r: &mut EstadoRitmo, som: &mut ServicoSom) {
    if r.tocando() {
        r.parar_e_silenciar(som);
    } else {
        r.tocar(som);
    }
}

fn desenhar_transporte(ui: &mut Ui, r: &mut EstadoRitmo, som: &mut ServicoSom) {
    ui.horizontal_wrapped(|ui| {
        let (texto, cor) = if r.tocando() {
            ("Parar", VERMELHO)
        } else {
            ("Tocar", VERDE)
        };
        if ui
            .add(
                Button::new(
                    RichText::new(texto)
                        .size(18.0)
                        .strong()
                        .color(Color32::BLACK),
                )
                .fill(cor)
                .min_size(vec2(96.0, 36.0)),
            )
            .clicked()
        {
            alternar(r, som);
        }
        ui.add_space(12.0);
        ui.label(RichText::new("Andamento").strong());
        if ui.button("-").clicked() {
            r.definir_bpm(r.bpm().saturating_sub(1));
        }
        let mut bpm = r.bpm();
        if ui
            .add(Slider::new(&mut bpm, BPM_MIN..=BPM_MAX).suffix(" bpm"))
            .changed()
        {
            r.definir_bpm(bpm);
        }
        if ui.button("+").clicked() {
            r.definir_bpm(r.bpm().saturating_add(1));
        }
        if ui
            .button("Tap")
            .on_hover_text("Clique no ritmo desejado (2 ou mais vezes)")
            .clicked()
        {
            r.tap(som.agora_ns());
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut r.config.metronomo, "Metrônomo");
        ui.checkbox(&mut r.config.bateria, "Bateria");
        ComboBox::from_id_salt("subdivisao")
            .selected_text(format!("Subdivisão: {}", r.config.subdivisao.texto()))
            .show_ui(ui, |ui| {
                for s in Subdivisao::TODAS {
                    ui.selectable_value(&mut r.config.subdivisao, s, s.texto());
                }
            });
        ComboBox::from_id_salt("contagem")
            .selected_text(match r.config.contagem {
                0 => "Sem contagem".to_owned(),
                1 => "Contagem: 1 compasso".to_owned(),
                n => format!("Contagem: {n} compassos"),
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut r.config.contagem, 0, "Sem contagem");
                ui.selectable_value(&mut r.config.contagem, 1, "1 compasso");
                ui.selectable_value(&mut r.config.contagem, 2, "2 compassos");
            });
        ui.label("Volume");
        ui.add(Slider::new(&mut r.volume, 0..=127).show_value(false));
    });
}

fn desenhar_pulsos(ui: &mut Ui, r: &EstadoRitmo, som: &ServicoSom) {
    let f = r.padrao.formula;
    let pos = r.posicao(som.agora_ns());
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{} · {} bpm", f.texto(), r.bpm()))
                .size(20.0)
                .strong(),
        );
        ui.add_space(12.0);
        let n = f.pulsos();
        let (rect, _) = ui.allocate_exact_size(vec2(n as f32 * 44.0, 40.0), Sense::hover());
        let p = ui.painter();
        for i in 0..n {
            let c = rect.left_center() + vec2(22.0 + i as f32 * 44.0, 0.0);
            let atual = pos.is_some_and(|x| x.pulso == i);
            let raio = if i == 0 { 16.0 } else { 12.0 };
            let preench = match (atual, i == 0) {
                (true, true) => AMBAR,
                (true, false) => AZUL,
                _ => ui.visuals().extreme_bg_color,
            };
            p.circle(
                c,
                raio,
                preench,
                Stroke::new(2.0, ui.visuals().text_color()),
            );
            p.text(
                c,
                egui::Align2::CENTER_CENTER,
                (i + 1).to_string(),
                egui::FontId::proportional(13.0),
                if atual {
                    Color32::BLACK
                } else {
                    ui.visuals().text_color()
                },
            );
        }
        if let Some(x) = pos {
            ui.label(format!("compasso {}", x.compasso + 1));
        }
    });
    ui.label(RichText::new(f.explicacao()).weak());
}

fn desenhar_padroes(ui: &mut Ui, r: &mut EstadoRitmo, som: &mut ServicoSom) {
    ui.label(RichText::new("Padrões").strong());
    ui.horizontal_wrapped(|ui| {
        let mut escolhido = None;
        for (i, p) in r.fabrica.iter().enumerate() {
            let sel = r.padrao.nome == p.nome;
            if ui
                .selectable_label(sel, format!("{} ({})", p.nome, p.formula.texto()))
                .clicked()
            {
                escolhido = Some(i);
            }
        }
        if let Some(i) = escolhido {
            let p = r.fabrica[i].clone();
            r.escolher_padrao(p);
            r.config.bateria = true;
        }
    });
    ui.horizontal_wrapped(|ui| {
        let mut formula = r.padrao.formula;
        ComboBox::from_id_salt("formula")
            .selected_text(format!("Compasso {}", formula.texto()))
            .show_ui(ui, |ui| {
                for f in Formula::TODAS {
                    ui.selectable_value(&mut formula, f, f.texto());
                }
            });
        let mut compassos = r.padrao.compassos;
        ComboBox::from_id_salt("compassos")
            .selected_text(format!("{compassos} compasso(s)"))
            .show_ui(ui, |ui| {
                for c in 1..=4u8 {
                    ui.selectable_value(&mut compassos, c, c.to_string());
                }
            });
        if formula != r.padrao.formula || compassos != r.padrao.compassos {
            if formula != r.padrao.formula {
                r.parar_e_silenciar(som);
            }
            r.padrao.redimensionar(formula, compassos);
        }
        if ui.button("Limpar grade").clicked() {
            r.padrao.limpar();
        }
        ui.add_space(12.0);
        ui.add(
            TextEdit::singleline(&mut r.nome_arquivo)
                .hint_text("nome do padrão")
                .desired_width(160.0),
        );
        if ui.button("Salvar").clicked() {
            r.salvar();
        }
        let salvos = r.salvos();
        if !salvos.is_empty() {
            let mut carregar = None;
            ComboBox::from_id_salt("salvos")
                .selected_text("Abrir salvo…")
                .show_ui(ui, |ui| {
                    for s in &salvos {
                        if ui.selectable_label(false, s).clicked() {
                            carregar = Some(s.clone());
                        }
                    }
                });
            if let Some(s) = carregar {
                r.carregar(&s);
            }
        }
    });
    if let Some(m) = &r.mensagem {
        ui.label(RichText::new(m).weak());
    }
}

fn desenhar_grade(ui: &mut Ui, r: &mut EstadoRitmo, som: &mut ServicoSom) {
    let passos = r.padrao.passos();
    let por_pulso = r.padrao.formula.passos_por_pulso() as usize;
    let atual = r
        .posicao(som.agora_ns())
        .and_then(|p| p.passo(&r.padrao, r.config.contagem));
    let lado = if passos > 24 { 18.0 } else { 24.0 };
    ScrollArea::horizontal()
        .id_salt("grade-ritmo")
        .show(ui, |ui| {
            egui::Grid::new("grade")
                .spacing(vec2(2.0, 3.0))
                .show(ui, |ui| {
                    for (i, inst) in INSTRUMENTOS.iter().enumerate() {
                        let rotulo = if i < 8 {
                            format!("{} {}", i + 1, inst.nome)
                        } else {
                            inst.nome.to_owned()
                        };
                        if ui
                            .add(Button::new(rotulo).min_size(vec2(130.0, lado)))
                            .on_hover_text("Tocar agora")
                            .clicked()
                        {
                            r.pad(som, i);
                        }
                        let lado_ms = vec2(lado, lado);
                        if ui
                            .add(botao_ms("M", r.mudo[i], AMBAR, lado_ms))
                            .on_hover_text(
                                "Mudo: cala esta peça na bateria (não é salvo no padrão)",
                            )
                            .clicked()
                        {
                            r.mudo[i] = !r.mudo[i];
                        }
                        if ui
                            .add(botao_ms("S", r.solo[i], VERDE, lado_ms))
                            .on_hover_text("Solo: só as peças em solo tocam (vence o mudo)")
                            .clicked()
                        {
                            r.solo[i] = !r.solo[i];
                        }
                        for passo in 0..passos {
                            if passo > 0 && passo % por_pulso == 0 {
                                ui.add_space(4.0);
                            }
                            let v = r.padrao.velocidade(i, passo);
                            let inicio_pulso = passo % por_pulso == 0;
                            let fundo = if v >= VEL_ACENTO {
                                AMBAR
                            } else if v > 0 {
                                AZUL
                            } else if inicio_pulso {
                                ui.visuals().widgets.inactive.weak_bg_fill
                            } else {
                                ui.visuals().extreme_bg_color
                            };
                            let borda = if atual == Some(passo) {
                                Stroke::new(2.5, VERDE)
                            } else {
                                Stroke::new(
                                    1.0,
                                    ui.visuals().widgets.noninteractive.bg_stroke.color,
                                )
                            };
                            let texto = if v >= VEL_ACENTO {
                                "X"
                            } else if v > 0 {
                                "x"
                            } else {
                                ""
                            };
                            if ui
                                .add(
                                    Button::new(RichText::new(texto).color(Color32::BLACK))
                                        .fill(fundo)
                                        .stroke(borda)
                                        .min_size(vec2(lado, lado)),
                                )
                                .clicked()
                            {
                                r.padrao.alternar(i, passo);
                            }
                        }
                        ui.end_row();
                    }
                });
        });
    ui.label(
        RichText::new(
            "Clique num passo: vazio, depois normal (x), depois acentuado (X). \
             M cala a peça, S deixa só as peças em solo (o solo vence o mudo).",
        )
        .weak(),
    );
}

/// Botão pequeno de alternância (M de mudo, S de solo).
fn botao_ms(letra: &str, ligado: bool, cor: Color32, tamanho: egui::Vec2) -> Button<'static> {
    let b = if ligado {
        Button::new(RichText::new(letra).strong().color(Color32::BLACK)).fill(cor)
    } else {
        Button::new(RichText::new(letra).weak())
    };
    b.min_size(tamanho)
}

fn desenhar_exercicio(ui: &mut Ui, r: &mut EstadoRitmo) {
    ui.label(RichText::new("Exercício: toque no pulso").strong());
    let mut ativo = r.exercicio.is_some();
    if ui
        .checkbox(
            &mut ativo,
            "Ativar (cada nota que você toca é comparada ao pulso mais próximo)",
        )
        .changed()
    {
        r.exercicio = ativo.then(ExercicioPulso::default);
    }
    let Some(e) = r.exercicio.as_mut() else {
        return;
    };
    ui.horizontal_wrapped(|ui| {
        ui.label("Tolerância");
        ui.add(Slider::new(&mut e.tolerancia_ms, 10..=150).suffix(" ms"));
        ui.label("Compensação de latência");
        ui.add(Slider::new(&mut e.compensacao_ms, -100..=100).suffix(" ms"));
        if ui.button("Zerar").clicked() {
            e.toques.clear();
        }
    });
    let (n, certo, adiantado, atrasado, medio) = e.resumo();
    if n == 0 {
        ui.label("Inicie o metrônomo e toque qualquer tecla junto com cada pulso.");
        return;
    }
    ui.label(
        RichText::new(format!(
            "{n} toques: {certo} no tempo, {adiantado} adiantados, {atrasado} atrasados · \
             desvio médio {medio:.0} ms"
        ))
        .strong(),
    );
    ui.horizontal_wrapped(|ui| {
        for (d, j) in e.toques.iter().rev().take(16) {
            let (cor, t) = match j {
                Julgamento::NoTempo => (VERDE, "no tempo"),
                Julgamento::Adiantado => (AMBAR, "adiantado"),
                Julgamento::Atrasado => (VERMELHO, "atrasado"),
            };
            ui.label(RichText::new(format!("{t} {d:+.0} ms")).color(cor));
        }
    });
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::som::apoio_testes::servico_pronto;
    use bancada_casca::egui::{Context, RawInput, Rect, pos2, vec2};
    use piano_tutor_audio::RelogioManual;
    use std::sync::Arc;
    use std::time::Duration;

    fn quadro(ctx: &Context, som: &mut ServicoSom, r: &mut EstadoRitmo) {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
            ..Default::default()
        };
        ctx.run_ui(entrada, |ui| desenhar_tela(ui, r, som))
            .drop_without_applying_deltas();
    }

    #[test]
    fn desenha_inativo_e_pronto_sem_panico() {
        let ctx = Context::default();
        let mut r = EstadoRitmo::novo(None);
        let mut inativo = ServicoSom::inativo(Arc::new(RelogioManual::novo()));
        for _ in 0..2 {
            quadro(&ctx, &mut inativo, &mut r);
        }
        let (mut pronto, _m) = servico_pronto(Arc::new(RelogioManual::novo()));
        for _ in 0..2 {
            quadro(&ctx, &mut pronto, &mut r);
        }
        assert!(!r.tocando(), "desenhar não deve iniciar o metrônomo");
    }

    #[test]
    fn desenha_tocando_com_exercicio_do_pulso() {
        let ctx = Context::default();
        let relogio = Arc::new(RelogioManual::novo());
        let (mut som, _m) = servico_pronto(relogio.clone());
        let mut r = EstadoRitmo::novo(None);
        r.tocar(&mut som);
        assert!(r.tocando());
        for _ in 0..3 {
            relogio.avancar(Duration::from_millis(250));
            quadro(&ctx, &mut som, &mut r);
        }
        r.exercicio = Some(ExercicioPulso::default());
        if let Some(e) = r.exercicio.as_mut() {
            e.toques.push((12.5, Julgamento::NoTempo));
            e.toques.push((-80.0, Julgamento::Atrasado));
        }
        relogio.avancar(Duration::from_millis(250));
        quadro(&ctx, &mut som, &mut r);
        r.parar();
        quadro(&ctx, &mut som, &mut r);
    }

    #[test]
    fn desenha_cada_padrao_de_fabrica_selecionado() {
        let ctx = Context::default();
        let (mut som, _m) = servico_pronto(Arc::new(RelogioManual::novo()));
        let mut r = EstadoRitmo::novo(None);
        let padroes = r.fabrica.clone();
        assert!(!padroes.is_empty());
        for p in padroes {
            r.escolher_padrao(p);
            quadro(&ctx, &mut som, &mut r);
            r.tocar(&mut som);
            quadro(&ctx, &mut som, &mut r);
            r.parar();
        }
    }

    #[test]
    fn desenha_com_mudo_e_solo_ligados_e_para_silenciando() {
        let ctx = Context::default();
        let relogio = Arc::new(RelogioManual::novo());
        let (mut som, _m) = servico_pronto(relogio.clone());
        let mut r = EstadoRitmo::novo(None);
        r.mudo[0] = true;
        r.solo[1] = true;
        r.config.bateria = true;
        r.tocar(&mut som);
        relogio.avancar(Duration::from_millis(250));
        quadro(&ctx, &mut som, &mut r);
        r.parar_e_silenciar(&mut som);
        quadro(&ctx, &mut som, &mut r);
        assert!(!r.tocando());
    }
}
