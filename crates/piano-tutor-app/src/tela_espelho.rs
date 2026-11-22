//! Tela Espelho (F02, #34): desenha as 61 teclas, o painel "nota atual" e liga o mouse e o
//! teclado do PC ao [`EstadoEspelho`] como fonte de **simulação**.
//!
//! Acessibilidade: a cor nunca é o único sinal. Tecla ligada muda de cor, ganha contorno
//! grosso e uma marca de forma própria (círculo na branca, losango na preta) e o nome da nota.

use crate::espelho::{
    ALTURA_BRANCA, BRANCA, BRANCA_PRESSIONADA, CONTORNO_BRANCA_PRESSIONADA,
    CONTORNO_PRETA_PRESSIONADA, EstadoEspelho, GeometriaTecla, MARGEM, PRETA, PRETA_PRESSIONADA,
    ResumoDesenho, Rgb, ZOOM_MAX, ZOOM_MIN, geometria, largura_total, tecla_em, texto_frequencia,
};
use bancada_casca::egui::containers::scroll_area;
use bancada_casca::egui::{
    self, Align2, Color32, CornerRadius, Event, FontId, Frame, Margin, Painter, Pos2, Rect,
    RichText, ScrollArea, Sense, Shape, Slider, Stroke, StrokeKind, Ui, vec2,
};
use piano_tutor_core::NotaMidi;
use piano_tutor_core::teclado::{
    Acidente, ConfigNomes, ConvencaoOitava, CorTecla, Idioma, nome_sem_oitava, rotulo,
};

/// Largura de uma tecla branca com zoom 1,0, em pixels.
pub const LARGURA_BRANCA_BASE_PX: f32 = 24.0;
/// Espaço abaixo das teclas para a marca do Dó central, em pixels.
const FAIXA_DO_CENTRAL_PX: f32 = 30.0;
/// Largura mínima de tecla para escrever rótulos nela.
const LARGURA_MIN_ROTULO_PX: f32 = 15.0;

const AMBAR: Color32 = Color32::from_rgb(255, 214, 102);
const TINTA_SOBRE_AMBAR: Color32 = Color32::from_rgb(20, 14, 0);

fn cor(c: Rgb) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Retângulo (px) de uma tecla, dada a área das teclas e a largura da branca.
pub fn retangulo_da_tecla(area: Rect, largura_branca: f32, g: &GeometriaTecla) -> Rect {
    let origem = area.min + vec2(MARGEM * largura_branca, 0.0);
    Rect::from_min_size(
        origem + vec2(g.x * largura_branca, 0.0),
        vec2(g.largura * largura_branca, g.altura * largura_branca),
    )
}

/// Desenha a tela inteira (título, avisos, opções, teclado e painel).
pub fn desenhar_tela(ui: &mut Ui, e: &mut EstadoEspelho) {
    processar_teclado_do_pc(ui, e);

    ScrollArea::vertical()
        .id_salt("espelho-vertical")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let aviso_visivel = desenhar_avisos(ui, e);
            ui.add_space(6.0);
            desenhar_opcoes(ui, e);
            ui.add_space(8.0);
            desenhar_teclado(ui, e, aviso_visivel);
            ui.add_space(8.0);
            desenhar_painel(ui, e);
        });
}

// ---------------------------------------------------------------------------------------------
// Entrada do teclado do PC
// ---------------------------------------------------------------------------------------------

fn processar_teclado_do_pc(ui: &Ui, e: &mut EstadoEspelho) {
    let focada = ui.input(|i| i.focused);
    if !focada {
        // Janela sem foco não recebe o "solta": soltar aqui evita nota presa.
        e.liberar_simulacao();
        return;
    }
    let digitando = ui.ctx().egui_wants_keyboard_input();
    let eventos = ui.input(|i| i.events.clone());
    for ev in eventos {
        let Event::Key {
            key,
            pressed,
            repeat,
            modifiers,
            ..
        } = ev
        else {
            continue;
        };
        if !pressed {
            e.pc_soltou(key); // soltar é sempre tratado, mesmo com modificador ou texto
        } else if !repeat && !digitando && !modifiers.command && !modifiers.alt && !modifiers.ctrl {
            e.pc_apertou(key);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Avisos e opções
// ---------------------------------------------------------------------------------------------

/// Desenha os avisos e diz se o de simulação apareceu.
fn desenhar_avisos(ui: &mut Ui, e: &EstadoEspelho) -> bool {
    let mut simulacao = false;
    if let Some(aviso) = e.aviso_simulacao() {
        simulacao = true;
        Frame::new()
            .fill(AMBAR)
            .stroke(Stroke::new(3.0, TINTA_SOBRE_AMBAR))
            .corner_radius(CornerRadius::same(4))
            .inner_margin(Margin::same(8))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(aviso)
                        .strong()
                        .size(18.0)
                        .color(TINTA_SOBRE_AMBAR),
                );
                ui.label(
                    RichText::new(
                        "Os eventos vêm do mouse e do teclado do PC, não de um dispositivo MIDI.",
                    )
                    .color(TINTA_SOBRE_AMBAR),
                );
            });
    }
    if let Some(aviso) = e.aviso_faixa() {
        ui.add_space(4.0);
        Frame::new()
            .stroke(Stroke::new(1.5, AMBAR))
            .corner_radius(CornerRadius::same(4))
            .inner_margin(Margin::same(6))
            .show(ui, |ui| {
                ui.label(RichText::new(aviso).color(AMBAR));
            });
    }
    if let Some(erro) = e.erro() {
        ui.add_space(4.0);
        ui.label(RichText::new(erro).color(AMBAR));
    }
    simulacao
}

fn desenhar_opcoes(ui: &mut Ui, e: &mut EstadoEspelho) {
    let largura_disponivel = ui.available_width();
    ui.horizontal_wrapped(|ui| {
        ui.label("Nomes:");
        ui.selectable_value(&mut e.nomes.idioma, Idioma::PtBr, "PT-BR (Dó, Ré…)");
        ui.selectable_value(
            &mut e.nomes.idioma,
            Idioma::Internacional,
            "Internacional (C, D…)",
        );
        ui.separator();
        ui.label("Acidentes:");
        ui.selectable_value(&mut e.nomes.acidente, Acidente::Sustenido, "Sustenidos (#)");
        ui.selectable_value(&mut e.nomes.acidente, Acidente::Bemol, "Bemóis (b)");
        ui.separator();
        ui.label("Oitava do Dó central:");
        ui.selectable_value(&mut e.nomes.convencao, ConvencaoOitava::Do4, "C4 (MIDI 60)");
        ui.selectable_value(&mut e.nomes.convencao, ConvencaoOitava::Do3, "C3 (MIDI 60)");
    });
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().slider_width = 160.0;
        let mut zoom = e.zoom();
        let r = ui.add(Slider::new(&mut zoom, ZOOM_MIN..=ZOOM_MAX).text("Zoom"));
        if r.changed() {
            e.definir_zoom(zoom);
        }
        if ui.button("Ajustar à largura").clicked() {
            let ideal = largura_disponivel / (largura_total(e.teclado()) * LARGURA_BRANCA_BASE_PX);
            e.definir_zoom(ideal);
        }
        if ui.button("Zoom 100%").clicked() {
            e.definir_zoom(1.0);
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Teclado
// ---------------------------------------------------------------------------------------------

fn desenhar_teclado(ui: &mut Ui, e: &mut EstadoEspelho, aviso_simulacao_visivel: bool) {
    let geo = geometria(e.teclado());
    let w = LARGURA_BRANCA_BASE_PX * e.zoom();
    let tamanho = vec2(
        largura_total(e.teclado()) * w,
        ALTURA_BRANCA * w + FAIXA_DO_CENTRAL_PX,
    );

    ScrollArea::horizontal()
        .scroll_source(
            scroll_area::ScrollSource::SCROLL_BAR | scroll_area::ScrollSource::MOUSE_WHEEL,
        )
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let (rect, resposta) = ui.allocate_exact_size(tamanho, Sense::click_and_drag());
            let area = Rect::from_min_size(rect.min, vec2(tamanho.x, ALTURA_BRANCA * w));

            // Mouse: só vale se o botão foi apertado sobre o teclado e segue apertado.
            let alvo = if resposta.is_pointer_button_down_on() {
                ui.input(|i| i.pointer.interact_pos()).and_then(|p| {
                    let origem = area.min + vec2(MARGEM * w, 0.0);
                    tecla_em(&geo, (p.x - origem.x) / w, (p.y - origem.y) / w)
                })
            } else {
                None
            };
            e.atualizar_mouse(alvo);

            let destacadas: Vec<usize> = e.teclas().indices_ligados().collect();
            if ui.is_rect_visible(rect) {
                pintar(ui.painter(), ui, e, &geo, area, w, &destacadas);
            }
            e.ultimo_desenho = Some(ResumoDesenho {
                area_teclas: area,
                largura_branca: w,
                destacadas,
                desenhadas: geo.len(),
                aviso_simulacao_visivel,
            });
        });
}

fn pintar(
    p: &Painter,
    ui: &Ui,
    e: &EstadoEspelho,
    geo: &[GeometriaTecla],
    area: Rect,
    w: f32,
    ligadas: &[usize],
) {
    let com_rotulos = w >= LARGURA_MIN_ROTULO_PX;
    let tinta = ui.visuals().text_color();
    // Brancas primeiro, pretas por cima.
    for passo in [CorTecla::Branca, CorTecla::Preta] {
        for g in geo.iter().filter(|g| g.cor == passo) {
            let r = retangulo_da_tecla(area, w, g);
            let ligada = ligadas.contains(&g.indice);
            match g.cor {
                CorTecla::Branca => pintar_branca(p, e, g, r, w, ligada, com_rotulos),
                CorTecla::Preta => pintar_preta(p, e, g, r, w, ligada, com_rotulos),
            }
        }
    }
    // Dó central: triângulo e texto sob a tecla.
    if let Some(i) = e.teclado().indice_do_central()
        && let Some(g) = geo.get(i)
    {
        let r = retangulo_da_tecla(area, w, g);
        let topo = Pos2::new(r.center().x, r.bottom() + 4.0);
        p.add(Shape::convex_polygon(
            vec![topo, topo + vec2(-7.0, 12.0), topo + vec2(7.0, 12.0)],
            AMBAR,
            Stroke::new(1.5, tinta),
        ));
        let texto = match e.nomes.idioma {
            Idioma::PtBr => format!("Dó central ({})", rotulo_tela(g.nota, e.nomes)),
            Idioma::Internacional => format!("Middle C ({})", rotulo_tela(g.nota, e.nomes)),
        };
        p.text(
            Pos2::new(r.center().x, r.bottom() + 17.0),
            Align2::CENTER_TOP,
            texto,
            FontId::proportional(12.0),
            tinta,
        );
    }
}

/// O egui embutido não tem os glifos ♯ e ♭ (aparecem como quadrado): na tela vão como `#` e
/// `b`. O núcleo continua com ♯ e ♭.
pub fn texto_tela(s: &str) -> String {
    s.replace('♯', "#").replace('♭', "b")
}

fn rotulo_tela(nota: NotaMidi, nomes: ConfigNomes) -> String {
    texto_tela(&rotulo(nota, nomes))
}

fn rotulo_de_c(e: &EstadoEspelho, g: &GeometriaTecla) -> Option<String> {
    g.nota
        .numero()
        .is_multiple_of(12)
        .then(|| rotulo_tela(g.nota, e.nomes))
}

fn pintar_branca(
    p: &Painter,
    e: &EstadoEspelho,
    g: &GeometriaTecla,
    r: Rect,
    w: f32,
    ligada: bool,
    com_rotulos: bool,
) {
    let canto = CornerRadius {
        nw: 0,
        ne: 0,
        sw: 3,
        se: 3,
    };
    let (preenchimento, traco, texto) = if ligada {
        (
            cor(BRANCA_PRESSIONADA),
            Stroke::new(3.0, cor(CONTORNO_BRANCA_PRESSIONADA)),
            cor(BRANCA),
        )
    } else {
        (cor(BRANCA), Stroke::new(1.0, cor(PRETA)), cor(PRETA))
    };
    p.rect_filled(r, canto, preenchimento);
    p.rect_stroke(r, canto, traco, StrokeKind::Inside);

    if ligada {
        // Marca de forma: círculo com anel, perto da base.
        let centro = Pos2::new(r.center().x, r.bottom() - 1.1 * w);
        p.circle(
            centro,
            0.34 * w,
            cor(BRANCA),
            Stroke::new(2.0, cor(CONTORNO_BRANCA_PRESSIONADA)),
        );
    }
    if com_rotulos {
        let mut y = r.bottom() - 4.0;
        if ligada {
            p.text(
                Pos2::new(r.center().x, r.bottom() - 1.6 * w),
                Align2::CENTER_BOTTOM,
                texto_tela(nome_sem_oitava(g.nota, e.nomes)),
                FontId::proportional(11.0),
                texto,
            );
            return;
        }
        if let Some(t) = rotulo_de_c(e, g) {
            p.text(
                Pos2::new(r.center().x, y),
                Align2::CENTER_BOTTOM,
                t,
                FontId::proportional(10.0),
                texto,
            );
            y -= 13.0;
        }
        if let Some(l) = e.letra_pc_da_nota(g.nota) {
            p.text(
                Pos2::new(r.center().x, y - 2.0),
                Align2::CENTER_BOTTOM,
                l,
                FontId::monospace(10.0),
                Color32::from_rgb(110, 110, 120),
            );
        }
    }
}

fn pintar_preta(
    p: &Painter,
    e: &EstadoEspelho,
    g: &GeometriaTecla,
    r: Rect,
    w: f32,
    ligada: bool,
    com_rotulos: bool,
) {
    let canto = CornerRadius {
        nw: 0,
        ne: 0,
        sw: 2,
        se: 2,
    };
    let (preenchimento, traco) = if ligada {
        (
            cor(PRETA_PRESSIONADA),
            Stroke::new(3.0, cor(CONTORNO_PRETA_PRESSIONADA)),
        )
    } else {
        (cor(PRETA), Stroke::new(1.0, Color32::from_rgb(90, 90, 104)))
    };
    p.rect_filled(r, canto, preenchimento);
    p.rect_stroke(r, canto, traco, StrokeKind::Inside);

    if ligada {
        // Marca de forma própria: losango escuro.
        let c = Pos2::new(r.center().x, r.bottom() - 0.55 * w);
        let d = 0.3 * w;
        p.add(Shape::convex_polygon(
            vec![
                c + vec2(0.0, -d),
                c + vec2(d, 0.0),
                c + vec2(0.0, d),
                c + vec2(-d, 0.0),
            ],
            cor(CONTORNO_PRETA_PRESSIONADA),
            Stroke::new(1.0, cor(BRANCA)),
        ));
    } else if com_rotulos && let Some(l) = e.letra_pc_da_nota(g.nota) {
        p.text(
            Pos2::new(r.center().x, r.bottom() - 4.0),
            Align2::CENTER_BOTTOM,
            l,
            FontId::monospace(10.0),
            Color32::from_rgb(190, 190, 205),
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Painel "nota atual"
// ---------------------------------------------------------------------------------------------

fn desenhar_painel(ui: &mut Ui, e: &EstadoEspelho) {
    ui.group(|ui| {
        ui.label(RichText::new("Nota atual").strong());
        match e.nota_atual() {
            None => {
                ui.label("Nenhuma nota ainda. Clique numa tecla ou use A S D F G H J K L no teclado do PC.");
            }
            Some(i) => {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(texto_tela(&i.rotulo)).size(34.0).strong());
                        ui.label(if i.pressionada {
                            "pressionada"
                        } else {
                            "solta (última nota tocada)"
                        });
                    });
                    ui.add_space(16.0);
                    egui::Grid::new("nota-atual").num_columns(4).show(ui, |ui| {
                        ui.label("Nome");
                        ui.label(format!(
                            "{} (PT-BR) · {} (internacional)",
                            texto_tela(i.nome_pt),
                            texto_tela(i.nome_internacional)
                        ));
                        ui.label("Número MIDI");
                        ui.label(i.midi.to_string());
                        ui.end_row();
                        ui.label("Oitava");
                        ui.label(format!("{} ({})", i.oitava, convencao_texto(e.nomes)));
                        ui.label("Frequência");
                        ui.label(format!(
                            "{} (Lá4 = 440 Hz)",
                            texto_frequencia(i.frequencia_hz)
                        ));
                        ui.end_row();
                    });
                });
            }
        }
        let ativas: Vec<String> = e.rotulos_ativos().iter().map(|r| texto_tela(r)).collect();
        ui.add_space(4.0);
        if ativas.is_empty() {
            ui.label("Teclas ligadas: nenhuma");
        } else {
            ui.label(format!(
                "Teclas ligadas ({}): {}",
                ativas.len(),
                ativas.join("  ")
            ));
        }
    });
    ui.add_space(6.0);
    ui.label(format!(
        "Mouse: clique ou arraste sobre as teclas. Teclado do PC: A S D F G H J K L ; tocam as brancas, \
         W E T Y U O P as pretas; Z e X descem e sobem a oitava (Dó da base: MIDI {}).",
        e.base_pc()
    ));
}

fn convencao_texto(n: ConfigNomes) -> &'static str {
    match n.convencao {
        ConvencaoOitava::Do4 => "convenção C4 = MIDI 60",
        ConvencaoOitava::Do3 => "convenção C3 = MIDI 60",
    }
}

#[cfg(test)]
mod testes {
    //! Testes "sem janela" da tela: desenho num `egui::Context` com entrada sintética.
    use super::*;
    use crate::espelho::AVISO_SIMULACAO;
    use bancada_casca::egui::{Context, Key, Modifiers, PointerButton, RawInput};

    fn n(x: u8) -> NotaMidi {
        NotaMidi::nova(x).expect("nota válida")
    }

    fn quadro(ctx: &Context, e: &mut EstadoEspelho, eventos: Vec<Event>) {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 800.0))),
            events: eventos,
            ..Default::default()
        };
        ctx.run_ui(entrada, |ui| desenhar_tela(ui, e))
            .drop_without_applying_deltas();
    }

    fn tecla_pc(key: Key, pressed: bool) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    /// Centro (px) da tecla `indice`, pelo último desenho.
    fn centro(e: &EstadoEspelho, indice: usize) -> Pos2 {
        let d = e.ultimo_desenho.as_ref().expect("já desenhou");
        let geo = geometria(e.teclado());
        let r = retangulo_da_tecla(d.area_teclas, d.largura_branca, &geo[indice]);
        match geo[indice].cor {
            CorTecla::Branca => Pos2::new(r.center().x, r.bottom() - 0.4 * d.largura_branca),
            CorTecla::Preta => r.center(),
        }
    }

    fn mouse(pos: Pos2, pressed: bool) -> Vec<Event> {
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            },
        ]
    }

    fn preparar() -> (Context, EstadoEspelho) {
        let ctx = Context::default();
        let mut e = EstadoEspelho::novo();
        for _ in 0..3 {
            quadro(&ctx, &mut e, Vec::new());
        }
        (ctx, e)
    }

    #[test]
    fn desenha_as_61_teclas_sem_panico_em_varios_zooms() {
        let (ctx, mut e) = preparar();
        for z in [ZOOM_MIN, 0.8, 1.0, 1.7, ZOOM_MAX] {
            e.definir_zoom(z);
            quadro(&ctx, &mut e, Vec::new());
            quadro(&ctx, &mut e, Vec::new());
            let d = e.ultimo_desenho.as_ref().expect("desenhou");
            assert_eq!(d.desenhadas, 61);
            assert!((d.largura_branca - LARGURA_BRANCA_BASE_PX * z).abs() < 1e-4);
        }
    }

    #[test]
    fn clicar_numa_tecla_liga_a_nota_certa_e_soltar_apaga_so_ela() {
        let (ctx, mut e) = preparar();
        let i = e.teclado().indice_de(n(60)).expect("Dó central na faixa");
        let pos = centro(&e, i);
        quadro(&ctx, &mut e, vec![Event::PointerMoved(pos)]);
        quadro(&ctx, &mut e, mouse(pos, true));
        quadro(&ctx, &mut e, Vec::new());
        assert!(e.teclas().esta_ligada(n(60)), "{:?}", e.erro());
        assert_eq!(e.teclas().quantidade(), 1);
        assert_eq!(e.ultimo_desenho.as_ref().expect("d").destacadas, vec![i]);
        assert_eq!(e.nota_atual().expect("nota").midi, 60);

        quadro(&ctx, &mut e, mouse(pos, false));
        quadro(&ctx, &mut e, Vec::new());
        assert_eq!(e.teclas().quantidade(), 0);
        assert!(e.ultimo_desenho.as_ref().expect("d").destacadas.is_empty());
    }

    #[test]
    fn clicar_numa_tecla_preta_liga_a_preta_e_nao_a_branca_vizinha() {
        let (ctx, mut e) = preparar();
        let i = e.teclado().indice_de(n(61)).expect("Dó♯4 na faixa");
        let pos = centro(&e, i);
        quadro(&ctx, &mut e, vec![Event::PointerMoved(pos)]);
        quadro(&ctx, &mut e, mouse(pos, true));
        quadro(&ctx, &mut e, Vec::new());
        assert!(e.teclas().esta_ligada(n(61)));
        assert!(!e.teclas().esta_ligada(n(60)) && !e.teclas().esta_ligada(n(62)));
        quadro(&ctx, &mut e, mouse(pos, false));
    }

    #[test]
    fn cada_uma_das_61_teclas_responde_ao_clique_na_nota_correta() {
        let (ctx, mut e) = preparar();
        for i in 0..61 {
            let pos = centro(&e, i);
            quadro(&ctx, &mut e, vec![Event::PointerMoved(pos)]);
            quadro(&ctx, &mut e, mouse(pos, true));
            quadro(&ctx, &mut e, Vec::new());
            let esperada = e.teclado().nota_de(i).expect("índice válido");
            let ligadas: Vec<u8> = e.teclas().notas_ligadas().map(|n| n.numero()).collect();
            assert_eq!(ligadas, vec![esperada.numero()], "tecla {i}");
            quadro(&ctx, &mut e, mouse(pos, false));
            quadro(&ctx, &mut e, Vec::new());
            assert_eq!(e.teclas().quantidade(), 0, "tecla {i} ficou presa");
        }
    }

    #[test]
    fn tecla_do_pc_liga_e_solta() {
        let (ctx, mut e) = preparar();
        quadro(&ctx, &mut e, vec![tecla_pc(Key::A, true)]);
        assert!(e.teclas().esta_ligada(n(60)));
        assert_eq!(e.ultimo_desenho.as_ref().expect("d").destacadas.len(), 1);
        quadro(&ctx, &mut e, vec![tecla_pc(Key::W, true)]);
        assert!(e.teclas().esta_ligada(n(61)) && e.teclas().esta_ligada(n(60)));
        quadro(&ctx, &mut e, vec![tecla_pc(Key::A, false)]);
        assert!(!e.teclas().esta_ligada(n(60)));
        assert!(e.teclas().esta_ligada(n(61)), "só a nota solta apaga");
        quadro(&ctx, &mut e, vec![tecla_pc(Key::W, false)]);
        assert_eq!(e.teclas().quantidade(), 0);
    }

    #[test]
    fn atalho_com_ctrl_nao_toca_nota() {
        let (ctx, mut e) = preparar();
        let ev = Event::Key {
            key: Key::T,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        };
        quadro(&ctx, &mut e, vec![ev]);
        assert_eq!(
            e.teclas().quantidade(),
            0,
            "Ctrl+T é da casca, não uma nota"
        );
    }

    #[test]
    fn dez_notas_simultaneas_ficam_destacadas_e_soltar_uma_preserva_as_outras() {
        let (ctx, mut e) = preparar();
        let dez = [
            Key::A,
            Key::S,
            Key::D,
            Key::F,
            Key::G,
            Key::H,
            Key::J,
            Key::K,
            Key::L,
            Key::Semicolon,
        ];
        quadro(
            &ctx,
            &mut e,
            dez.iter().map(|k| tecla_pc(*k, true)).collect(),
        );
        quadro(&ctx, &mut e, Vec::new());
        assert_eq!(e.teclas().quantidade(), 10);
        let d = e.ultimo_desenho.as_ref().expect("d");
        assert_eq!(d.destacadas.len(), 10);
        assert_eq!(e.rotulos_ativos().len(), 10);

        quadro(&ctx, &mut e, vec![tecla_pc(Key::F, false)]);
        quadro(&ctx, &mut e, Vec::new());
        let d = e.ultimo_desenho.as_ref().expect("d");
        assert_eq!(d.destacadas.len(), 9);
        assert!(!e.teclas().esta_ligada(n(65)));
        for k in dez.iter().filter(|k| **k != Key::F) {
            let nota = e.nota_da_tecla_pc(*k).expect("mapeada");
            assert!(e.teclas().esta_ligada(nota), "{k:?} perdeu o estado");
        }
    }

    #[test]
    fn aviso_de_simulacao_aparece_enquanto_a_fonte_for_simulacao() {
        let (ctx, mut e) = preparar();
        assert_eq!(e.aviso_simulacao(), Some(AVISO_SIMULACAO));
        assert!(
            e.ultimo_desenho
                .as_ref()
                .expect("d")
                .aviso_simulacao_visivel
        );
        quadro(&ctx, &mut e, vec![tecla_pc(Key::A, true)]);
        assert!(
            e.ultimo_desenho
                .as_ref()
                .expect("d")
                .aviso_simulacao_visivel,
            "o aviso é permanente, com ou sem nota tocando"
        );
    }

    #[test]
    fn aviso_some_com_fonte_fisica_e_volta_com_simulacao() {
        use piano_tutor_core::teclado::{AcaoTecla, EventoTecla, OrigemEventos};
        let (ctx, mut e) = preparar();
        e.aplicar_evento(EventoTecla {
            origem: OrigemEventos::DispositivoFisico,
            acao: AcaoTecla::Ligar,
            nota: n(60),
        })
        .expect("na faixa");
        quadro(&ctx, &mut e, Vec::new());
        assert!(
            !e.ultimo_desenho
                .as_ref()
                .expect("d")
                .aviso_simulacao_visivel
        );
        quadro(&ctx, &mut e, vec![tecla_pc(Key::D, true)]);
        assert!(
            e.ultimo_desenho
                .as_ref()
                .expect("d")
                .aviso_simulacao_visivel
        );
    }

    #[test]
    fn perder_o_foco_da_janela_solta_a_simulacao() {
        let (ctx, mut e) = preparar();
        quadro(&ctx, &mut e, vec![tecla_pc(Key::A, true)]);
        assert_eq!(e.teclas().quantidade(), 1);
        // A integração (eframe) marca `focused = false` no RawInput ao perder o foco.
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 800.0))),
            focused: false,
            events: vec![Event::WindowFocused(false)],
            ..Default::default()
        };
        ctx.run_ui(entrada, |ui| desenhar_tela(ui, &mut e))
            .drop_without_applying_deltas();
        assert_eq!(
            e.teclas().quantidade(),
            0,
            "nota presa depois de perder foco"
        );
    }

    #[test]
    fn texto_da_tela_nao_usa_glifos_que_a_fonte_embutida_nao_tem() {
        assert_eq!(texto_tela("Dó♯4"), "Dó#4");
        assert_eq!(texto_tela("Ré♭3"), "Réb3");
        assert_eq!(texto_tela("Sol"), "Sol");
        // O núcleo segue com ♯/♭: só a exibição troca.
        assert_eq!(
            rotulo(n(61), piano_tutor_core::teclado::ConfigNomes::default()),
            "Dó♯4"
        );
        assert_eq!(
            rotulo_tela(n(61), piano_tutor_core::teclado::ConfigNomes::default()),
            "Dó#4"
        );
    }

    #[test]
    fn tecla_ligada_nao_depende_so_da_cor() {
        // Contorno 3 px e marca de forma só existem na tecla ligada; a cor muda também.
        let (ctx, mut e) = preparar();
        let formas = |ctx: &Context, e: &mut EstadoEspelho| {
            let entrada = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 800.0))),
                ..Default::default()
            };
            let saida = ctx.run_ui(entrada, |ui| desenhar_tela(ui, e));
            let total = saida.shapes.len();
            saida.drop_without_applying_deltas();
            total
        };
        let sem = formas(&ctx, &mut e);
        e.pressionar_simulado(n(60));
        let com = formas(&ctx, &mut e);
        assert!(
            com > sem,
            "tecla ligada deve acrescentar formas ({sem} -> {com})"
        );
    }

    #[test]
    fn desenha_em_3_temas_sem_panico() {
        use crate::app::{AppPianoTutor, config};
        use crate::estado::TEMAS;
        use bancada_casca::Casca;

        let mut c = Casca::nova(AppPianoTutor::novo(), config()).expect("tokens e temas");
        let ctx = Context::default();
        let entrada = |eventos: Vec<Event>| RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 800.0))),
            events: eventos,
            ..Default::default()
        };
        // Abre a tela Espelho pela rota e desenha em cada tema, com notas ligadas.
        c.abrir(&ctx, "teclado".into(), bancada_casca::Local::Centro);
        for tema in TEMAS {
            c.definir_aparencia(Some(tema.id), None, None);
            c.app.espelho.pressionar_simulado(n(60));
            c.app.espelho.pressionar_simulado(n(64));
            c.app.espelho.pressionar_simulado(n(67));
            for _ in 0..2 {
                ctx.run_ui(entrada(Vec::new()), |ui| c.desenhar(ui))
                    .drop_without_applying_deltas();
            }
            assert_eq!(
                c.app.espelho.ultimo_desenho.as_ref().expect("d").desenhadas,
                61
            );
            assert_eq!(
                c.app
                    .espelho
                    .ultimo_desenho
                    .as_ref()
                    .expect("d")
                    .destacadas
                    .len(),
                3
            );
            c.app.espelho.limpar_tudo();
        }
        assert!(c.estado.erros.is_empty(), "{:?}", c.estado.erros);
    }
}
