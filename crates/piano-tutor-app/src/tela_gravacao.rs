//! Tela Gravação (F06): gravar o que o aluno toca, ouvir de novo em outro timbre, salvar e
//! importar arquivos `.mid` e ver uma linha do tempo simples das notas.

use crate::gravacao::{EstadoGravacao, NotaLinha};
use crate::som::{PROGRAMA_ACORDEON, PROGRAMA_PIANO, ServicoSom, Timbre};
use bancada_casca::egui::{
    self, Align2, Button, Color32, ComboBox, CornerRadius, FontId, Key, Rect, RichText, ScrollArea,
    Sense, Slider, Stroke, TextEdit, Ui, pos2, vec2,
};

const VERDE: Color32 = Color32::from_rgb(70, 200, 110);
const AMBAR: Color32 = Color32::from_rgb(255, 196, 70);
const VERMELHO: Color32 = Color32::from_rgb(230, 90, 80);
const AZUL: Color32 = Color32::from_rgb(80, 150, 255);
const ROXO: Color32 = Color32::from_rgb(180, 120, 240);

/// Máximo de notas desenhadas na linha do tempo (arquivos grandes não travam a tela).
const NOTAS_DESENHADAS_MAX: usize = 5000;
const ALTURA_LINHA: f32 = 180.0;
const MARGEM_EIXO: f32 = 16.0;

pub fn desenhar_tela(ui: &mut Ui, g: &mut EstadoGravacao, som: &mut ServicoSom) {
    ScrollArea::vertical()
        .id_salt("gravacao-vertical")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading("Gravação");
            ui.label(
                "Grave o que você toca (teclado, mouse ou teclado do PC), ouça de novo em \
                 outro timbre e guarde como arquivo .mid. A gravação preserva altura, \
                 velocidade, canal e pedal.",
            );
            ui.add_space(6.0);
            desenhar_transporte(ui, g, som);
            desenhar_aviso(ui, g);
            ui.add_space(6.0);
            ui.separator();
            desenhar_timbre(ui, g, som);
            ui.add_space(6.0);
            ui.separator();
            desenhar_lista(ui, g, som);
            ui.add_space(6.0);
            desenhar_arquivos(ui, g);
            if let Some(m) = &g.mensagem {
                ui.label(RichText::new(m.as_str()).weak());
            }
            ui.add_space(6.0);
            ui.separator();
            desenhar_linha_do_tempo(ui, g, som);
        });
}

fn desenhar_transporte(ui: &mut Ui, g: &mut EstadoGravacao, som: &mut ServicoSom) {
    let gravando = g.gravando();
    let tocando = g.tocando();
    let pausada = g.pausada();
    let tem_selecao = g.selecionada.is_some_and(|i| i < g.takes.len());
    ui.horizontal_wrapped(|ui| {
        if botao(ui, "Gravar", VERMELHO, !gravando).clicked() {
            g.gravar(som);
        }
        if botao(ui, "Parar", AMBAR, gravando || tocando || pausada).clicked() {
            g.parar(som);
        }
        let rotulo_tocar = if pausada { "Retomar" } else { "Tocar" };
        if botao(
            ui,
            rotulo_tocar,
            VERDE,
            !gravando && !tocando && tem_selecao,
        )
        .clicked()
        {
            g.tocar(som);
        }
        if botao(ui, "Pausa", AZUL, tocando).clicked() {
            g.pausar(som);
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.add_enabled_ui(!gravando, |ui| {
            ui.checkbox(&mut g.contagem, "Contagem de 1 compasso (4 tempos)");
            if g.contagem {
                ui.add(Slider::new(&mut g.bpm_contagem, 40..=200).suffix(" bpm"));
            }
        });
    });
    let agora = som.agora_ns();
    let estado = if gravando {
        match g.contagem_restante_ns(agora) {
            Some(r) => RichText::new(format!(
                "Contagem... gravação começa em {:.1} s",
                ms(r) / 1000.0
            ))
            .color(AMBAR),
            None => RichText::new(format!(
                "Gravando · {} · {} eventos",
                tempo_texto(g.duracao_gravando_ns(agora)),
                g.eventos_gravando()
            ))
            .color(VERMELHO),
        }
    } else if let (Some(pos), Some(i)) = (g.posicao_ns(agora), g.selecionada) {
        let dur = g.takes.get(i).map_or(0, |t| t.gravacao.duracao_ns);
        let verbo = if tocando { "Tocando" } else { "Pausado em" };
        RichText::new(format!(
            "{verbo} {} / {}",
            tempo_texto(pos),
            tempo_texto(dur)
        ))
        .color(VERDE)
    } else {
        RichText::new("Parado").weak()
    };
    ui.label(estado.strong());
}

fn botao(ui: &mut Ui, texto: &str, cor: Color32, ativo: bool) -> egui::Response {
    ui.add_enabled(
        ativo,
        Button::new(
            RichText::new(texto)
                .size(18.0)
                .strong()
                .color(Color32::BLACK),
        )
        .fill(cor)
        .min_size(vec2(96.0, 36.0)),
    )
}

fn desenhar_aviso(ui: &mut Ui, g: &mut EstadoGravacao) {
    if !g.tem_dados_nao_salvos() {
        return;
    }
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new("Atenção: há gravação NÃO SALVA. Salve como .mid antes de fechar o app.")
                .strong()
                .color(AMBAR),
        );
        if ui
            .button("Salvar rascunho de recuperação")
            .on_hover_text("Guarda uma cópia em gravacoes/recuperacao, sem substituir o salvamento")
            .clicked()
        {
            g.salvar_recuperacao();
        }
    });
}

fn desenhar_timbre(ui: &mut Ui, g: &mut EstadoGravacao, som: &mut ServicoSom) {
    ui.label(RichText::new("Timbre da reprodução").strong());
    ui.label(
        RichText::new(
            "É o mesmo timbre das demonstrações do app (acordes, escalas). A reprodução usa \
             as mesmas notas e tempos em qualquer timbre.",
        )
        .weak(),
    );
    let atual = som.config_teclado().timbre_demonstracao;
    let piano = Timbre {
        banco: 0,
        programa: PROGRAMA_PIANO,
    };
    let acordeom = Timbre {
        banco: 0,
        programa: PROGRAMA_ACORDEON,
    };
    let mut escolha: Option<Option<Timbre>> = None;
    ui.horizontal_wrapped(|ui| {
        let tem_piano = som.tem_preset(piano.banco, piano.programa);
        if ui
            .add_enabled_ui(tem_piano, |ui| {
                ui.selectable_label(atual == Some(piano), "Piano")
            })
            .inner
            .clicked()
        {
            escolha = Some(Some(piano));
        }
        let tem_acordeom = som.tem_preset(acordeom.banco, acordeom.programa);
        let r = ui
            .add_enabled_ui(tem_acordeom, |ui| {
                ui.selectable_label(atual == Some(acordeom), "Acordeom")
            })
            .inner;
        if !tem_acordeom {
            r.on_hover_text("Este SoundFont não tem o preset de acordeom");
        } else if r.clicked() {
            escolha = Some(Some(acordeom));
        }
        if ui
            .selectable_label(atual.is_none(), "Igual ao principal")
            .clicked()
        {
            escolha = Some(None);
        }
        let opcoes: Vec<(Timbre, String)> = som
            .presets()
            .iter()
            .filter(|p| p.banco <= 127)
            .map(|p| {
                (
                    Timbre {
                        banco: p.banco,
                        programa: p.programa,
                    },
                    format!("{}:{} {}", p.banco, p.programa, p.nome),
                )
            })
            .collect();
        let texto = atual
            .and_then(|t| opcoes.iter().find(|(x, _)| *x == t))
            .map_or_else(|| "Outro preset...".to_owned(), |(_, n)| n.clone());
        ComboBox::from_id_salt("timbre-reproducao")
            .selected_text(texto)
            .show_ui(ui, |ui| {
                for (t, nome) in &opcoes {
                    if ui.selectable_label(atual == Some(*t), nome).clicked() {
                        escolha = Some(Some(*t));
                    }
                }
            });
    });
    if let Some(novo) = escolha {
        let mut c = som.config_teclado();
        c.timbre_demonstracao = novo;
        if let Err(e) = som.definir_config_teclado(c) {
            g.mensagem = Some(e);
        }
    }
}

fn desenhar_lista(ui: &mut Ui, g: &mut EstadoGravacao, som: &mut ServicoSom) {
    ui.label(RichText::new("Gravações desta sessão").strong());
    if g.takes.is_empty() {
        ui.label(RichText::new("Nenhuma ainda. Aperte Gravar e toque.").weak());
        return;
    }
    let mut selecionar = None;
    let mut remover = None;
    for (i, t) in g.takes.iter().enumerate() {
        ui.horizontal(|ui| {
            let sel = g.selecionada == Some(i);
            let mut texto = format!(
                "{} · {} · {} notas",
                t.gravacao.nome,
                tempo_texto(t.gravacao.duracao_ns),
                t.gravacao.total_notas()
            );
            if !t.salvo {
                texto.push_str(" · NÃO SALVA");
            }
            let rt = if t.salvo {
                RichText::new(texto)
            } else {
                RichText::new(texto).color(AMBAR)
            };
            if ui.selectable_label(sel, rt).clicked() {
                selecionar = Some(i);
            }
            if ui
                .small_button("Descartar")
                .on_hover_text("Tira da lista (o arquivo salvo, se houver, continua no disco)")
                .clicked()
            {
                remover = Some(i);
            }
        });
    }
    if let Some(i) = selecionar {
        g.selecionar(i, som);
    }
    if let Some(i) = remover {
        g.remover(i, som);
    }
}

fn desenhar_arquivos(ui: &mut Ui, g: &mut EstadoGravacao) {
    ui.label(RichText::new("Arquivos .mid").strong());
    ui.horizontal_wrapped(|ui| {
        ui.add(
            TextEdit::singleline(&mut g.nome_arquivo)
                .hint_text("nome do arquivo")
                .desired_width(180.0),
        );
        let sel = g.selecionada.filter(|i| *i < g.takes.len());
        if ui
            .add_enabled(sel.is_some(), Button::new("Salvar .mid"))
            .clicked()
            && let Some(i) = sel
        {
            g.salvar(i);
        }
        let salvos = g.salvos();
        let mut abrir = None;
        ComboBox::from_id_salt("gravacoes-salvas")
            .selected_text("Abrir salvo...")
            .show_ui(ui, |ui| {
                if salvos.is_empty() {
                    ui.label(RichText::new("Nenhum arquivo salvo.").weak());
                }
                for s in &salvos {
                    if ui.selectable_label(false, s).clicked() {
                        abrir = Some(s.clone());
                    }
                }
            });
        if let Some(s) = abrir {
            g.abrir_salvo(&s);
        }
    });
    ui.horizontal_wrapped(|ui| {
        let r = ui.add(
            TextEdit::singleline(&mut g.caminho_importar)
                .hint_text("caminho de um arquivo .mid (tipo 0 ou 1)")
                .desired_width(360.0),
        );
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if ui.button("Importar .mid").clicked() || enter {
            g.importar_digitado();
        }
    });
}

fn desenhar_linha_do_tempo(ui: &mut Ui, g: &mut EstadoGravacao, som: &ServicoSom) {
    ui.label(RichText::new("Linha do tempo").strong());
    let Some(i) = g.selecionada.filter(|i| *i < g.takes.len()) else {
        ui.label(RichText::new("Selecione uma gravação para ver as notas.").weak());
        return;
    };
    let dur = g.takes[i].gravacao.duracao_ns.max(1);
    let largura = ui.available_width().max(240.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(largura, ALTURA_LINHA), Sense::click());
    let pos = g.posicao_ns(som.agora_ns());
    desenhar_notas(ui, rect, &g.takes[i].notas, dur, pos);
    if resp.clicked()
        && let Some(p) = resp.interact_pointer_pos()
    {
        let f = ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        g.buscar((f64::from(f) * dur as f64) as u64);
    }
    ui.label(
        RichText::new("Clique na linha do tempo para escolher onde a reprodução começa.").weak(),
    );
}

fn cor_da_nota(n: &NotaLinha) -> Color32 {
    let base = match n.canal {
        0 => AZUL,
        1 => VERDE,
        2 => AMBAR,
        9 => VERMELHO,
        _ => ROXO,
    };
    base.gamma_multiply(0.45 + 0.55 * f32::from(n.velocidade) / 127.0)
}

fn desenhar_notas(ui: &Ui, rect: Rect, notas: &[NotaLinha], dur_ns: u64, pos_ns: Option<u64>) {
    let p = ui.painter_at(rect);
    p.rect_filled(rect, CornerRadius::same(4), ui.visuals().extreme_bg_color);
    let cor_texto = ui.visuals().weak_text_color();
    let util = Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() - MARGEM_EIXO));
    let x_de = |t: u64| util.left() + (t as f64 / dur_ns as f64) as f32 * util.width();

    // Eixo do tempo: marcas a cada 0,5 a 300 s, com pelo menos 70 px entre elas.
    let dur_s = dur_ns as f64 / 1e9;
    let passo = [0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0]
        .into_iter()
        .find(|s| s / dur_s * f64::from(util.width()) >= 70.0)
        .unwrap_or(600.0);
    let mut s = 0.0;
    while s <= dur_s {
        let x = x_de((s * 1e9) as u64);
        p.line_segment(
            [pos2(x, util.top()), pos2(x, util.bottom())],
            Stroke::new(1.0, cor_texto.gamma_multiply(0.25)),
        );
        p.text(
            pos2(x + 2.0, rect.bottom() - 1.0),
            Align2::LEFT_BOTTOM,
            tempo_texto((s * 1e9) as u64),
            FontId::proportional(11.0),
            cor_texto,
        );
        s += passo;
    }

    if notas.is_empty() {
        return;
    }
    let (min, max) = notas
        .iter()
        .fold((127u8, 0u8), |(a, b), n| (a.min(n.nota), b.max(n.nota)));
    let linhas = (u32::from(max) - u32::from(min) + 1).max(12) as f32;
    let alt = util.height() / linhas;
    for n in notas.iter().take(NOTAS_DESENHADAS_MAX) {
        let x0 = x_de(n.inicio_ns);
        let x1 = x_de(n.fim_ns).max(x0 + 2.0);
        let y = util.bottom() - f32::from(n.nota - min + 1) * alt;
        let r = Rect::from_min_max(pos2(x0, y), pos2(x1, y + (alt - 1.0).max(2.0)));
        p.rect_filled(r, CornerRadius::same(1), cor_da_nota(n));
    }
    if notas.len() > NOTAS_DESENHADAS_MAX {
        p.text(
            pos2(rect.right() - 4.0, rect.top() + 2.0),
            Align2::RIGHT_TOP,
            format!("mostrando as primeiras {NOTAS_DESENHADAS_MAX} notas"),
            FontId::proportional(11.0),
            cor_texto,
        );
    }
    p.text(
        pos2(rect.left() + 4.0, rect.top() + 2.0),
        Align2::LEFT_TOP,
        format!("notas MIDI {min} a {max}"),
        FontId::proportional(11.0),
        cor_texto,
    );
    if let Some(t) = pos_ns {
        let x = x_de(t.min(dur_ns));
        p.line_segment(
            [pos2(x, util.top()), pos2(x, util.bottom())],
            Stroke::new(2.0, AMBAR),
        );
    }
}

fn ms(ns: u64) -> f64 {
    ns as f64 / 1e6
}

/// `m:ss.d`.
fn tempo_texto(ns: u64) -> String {
    let decimos = ns / 100_000_000;
    format!(
        "{}:{:02}.{}",
        decimos / 600,
        (decimos / 10) % 60,
        decimos % 10
    )
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::gravacao::{EventoGravado, Gravacao, Mensagem, Take};
    use crate::som::apoio_testes::servico_pronto;
    use bancada_casca::egui::{Context, RawInput};
    use piano_tutor_audio::RelogioManual;
    use piano_tutor_core::tempo::Relogio;
    use std::sync::Arc;
    use std::time::Duration;

    fn quadro(ctx: &Context, som: &mut ServicoSom, g: &mut EstadoGravacao) {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
            ..Default::default()
        };
        ctx.run_ui(entrada, |ui| desenhar_tela(ui, g, som))
            .drop_without_applying_deltas();
    }

    fn take() -> Take {
        let notas = [(60u8, 0u64, 400u64), (64, 500, 900), (67, 1000, 1500)];
        let mut eventos = Vec::new();
        for (n, ini, fim) in notas {
            eventos.push(EventoGravado {
                t_ns: ini * 1_000_000,
                msg: Mensagem::NotaLigada {
                    canal: 0,
                    nota: n,
                    velocidade: 90,
                },
            });
            eventos.push(EventoGravado {
                t_ns: fim * 1_000_000,
                msg: Mensagem::NotaDesligada {
                    canal: 0,
                    nota: n,
                    velocidade: 0,
                },
            });
        }
        let g = Gravacao {
            nome: "Teste".to_owned(),
            eventos,
            tempos: Vec::new(),
            duracao_ns: 1_600_000_000,
        };
        let notas = g.notas();
        Take {
            gravacao: g,
            notas,
            salvo: false,
            caminho: None,
        }
    }

    #[test]
    fn tempo_texto_formata_minutos_segundos_e_decimos() {
        assert_eq!(tempo_texto(0), "0:00.0");
        assert_eq!(tempo_texto(65_300_000_000), "1:05.3");
    }

    #[test]
    fn desenha_sem_panico_em_todos_os_estados_e_nao_inicia_nada() {
        let ctx = Context::default();
        let mut g = EstadoGravacao::novo(None);
        let mut inativo = ServicoSom::inativo(Arc::new(RelogioManual::novo()));
        for _ in 0..2 {
            quadro(&ctx, &mut inativo, &mut g);
        }
        let relogio = RelogioManual::novo();
        let (mut som, _m) = servico_pronto(Arc::new(relogio.clone()) as Arc<dyn Relogio>);
        g.takes.push(take());
        g.selecionada = Some(0);
        for _ in 0..2 {
            quadro(&ctx, &mut som, &mut g);
        }
        assert!(!g.gravando() && !g.tocando(), "desenhar não inicia nada");
        assert!(
            g.tem_dados_nao_salvos(),
            "aviso aparece com gravação não salva"
        );
        g.tocar(&mut som);
        relogio.avancar(Duration::from_millis(400));
        g.atualizar(&mut som);
        quadro(&ctx, &mut som, &mut g);
        g.pausar(&mut som);
        quadro(&ctx, &mut som, &mut g);
        g.parar(&mut som);
        g.contagem = true;
        g.gravar(&mut som);
        quadro(&ctx, &mut som, &mut g);
        g.parar(&mut som);
        quadro(&ctx, &mut som, &mut g);
    }

    #[test]
    fn linha_do_tempo_com_muitas_notas_nao_trava_nem_entra_em_panico() {
        let ctx = Context::default();
        let (mut som, _m) = servico_pronto(Arc::new(RelogioManual::novo()));
        let mut g = EstadoGravacao::novo(None);
        let mut t = take();
        t.notas = (0..(NOTAS_DESENHADAS_MAX + 100))
            .map(|i| NotaLinha {
                nota: (i % 128) as u8,
                canal: (i % 16) as u8,
                velocidade: 127,
                inicio_ns: i as u64 * 1000,
                fim_ns: i as u64 * 1000 + 500,
            })
            .collect();
        g.takes.push(t);
        g.selecionada = Some(0);
        quadro(&ctx, &mut som, &mut g);
    }
}
