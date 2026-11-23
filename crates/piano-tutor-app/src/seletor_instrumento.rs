//! Seletor de instrumento do Ribbon: escolhe o timbre do SoundFont por família General MIDI,
//! com busca por nome (sem acento e sem caixa), família ou número do programa.
//!
//! A lógica (famílias, filtro, rótulo) é pura e testada; o desenho só monta o botão do Ribbon e
//! o painel suspenso com a busca e a lista, e devolve o timbre escolhido para o app aplicar.

use crate::icones;
use crate::som::ServicoSom;
use bancada_casca::egui::{self, RichText, ScrollArea, TextEdit, Ui, vec2};
use piano_tutor_audio::rusty::PresetSf2;

/// Banco de bateria do General MIDI (percussão no canal 10).
pub const BANCO_BATERIA: u16 = 128;

/// Uma família do General MIDI: oito programas seguidos, com nome e ícone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Familia {
    pub nome: &'static str,
    pub icone: &'static str,
}

/// As 16 famílias GM (programas 0–127, de 8 em 8) e, por último, a bateria (banco 128).
pub const FAMILIAS: [Familia; 17] = [
    Familia {
        nome: "Pianos",
        icone: "piano",
    },
    Familia {
        nome: "Percussão cromática",
        icone: "nota",
    },
    Familia {
        nome: "Órgãos e acordeons",
        icone: "orgao",
    },
    Familia {
        nome: "Violões e guitarras",
        icone: "cordas",
    },
    Familia {
        nome: "Baixos",
        icone: "cordas",
    },
    Familia {
        nome: "Cordas",
        icone: "cordas",
    },
    Familia {
        nome: "Conjuntos e coros",
        icone: "voz",
    },
    Familia {
        nome: "Metais",
        icone: "sopro",
    },
    Familia {
        nome: "Palhetas",
        icone: "sopro",
    },
    Familia {
        nome: "Flautas",
        icone: "sopro",
    },
    Familia {
        nome: "Sintetizador solo",
        icone: "sintetizador",
    },
    Familia {
        nome: "Sintetizador pad",
        icone: "sintetizador",
    },
    Familia {
        nome: "Efeitos de sintetizador",
        icone: "sintetizador",
    },
    Familia {
        nome: "Étnicos",
        icone: "cordas",
    },
    Familia {
        nome: "Percussivos",
        icone: "bateria",
    },
    Familia {
        nome: "Efeitos sonoros",
        icone: "timbres",
    },
    Familia {
        nome: "Bateria",
        icone: "bateria",
    },
];

/// Índice da família de um preset em [`FAMILIAS`].
pub fn familia_de(banco: u16, programa: u8) -> usize {
    if banco == BANCO_BATERIA {
        FAMILIAS.len() - 1
    } else {
        usize::from(programa.min(127) / 8)
    }
}

/// Texto sem acento e em minúsculas, para a busca.
pub fn normalizar(texto: &str) -> String {
    texto
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            outro => outro,
        })
        .collect()
}

/// O preset combina com a busca? Cada palavra precisa aparecer no nome, na família ou no
/// número do programa (1–128, como no painel de um teclado). Busca vazia combina com todos.
pub fn combina(preset: &PresetSf2, busca: &str) -> bool {
    let familia = FAMILIAS[familia_de(preset.banco, preset.programa)].nome;
    let alvo = normalizar(&format!(
        "{} {} {}",
        preset.nome,
        familia,
        u16::from(preset.programa) + 1
    ));
    normalizar(busca)
        .split_whitespace()
        .all(|palavra| alvo.contains(palavra))
}

/// Os presets que passam no filtro (família opcional e busca), agrupados por família na ordem
/// de [`FAMILIAS`]. Só aparecem famílias com algum preset.
pub fn agrupar<'a>(
    presets: &'a [PresetSf2],
    familia: Option<usize>,
    busca: &str,
) -> Vec<(usize, Vec<&'a PresetSf2>)> {
    let mut grupos: Vec<(usize, Vec<&PresetSf2>)> = Vec::new();
    for f in 0..FAMILIAS.len() {
        if familia.is_some_and(|x| x != f) {
            continue;
        }
        let lista: Vec<&PresetSf2> = presets
            .iter()
            .filter(|p| familia_de(p.banco, p.programa) == f && combina(p, busca))
            .collect();
        if !lista.is_empty() {
            grupos.push((f, lista));
        }
    }
    grupos
}

/// Estado do seletor (texto da busca e família marcada).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EstadoSeletor {
    pub busca: String,
    pub familia: Option<usize>,
}

/// Desenha o seletor no Ribbon. Devolve o timbre escolhido neste quadro (banco, programa).
pub fn desenhar(
    ui: &mut Ui,
    som: &ServicoSom,
    estado: &mut EstadoSeletor,
    compacto: bool,
) -> Option<(u16, u8)> {
    let timbre = som.timbre();
    let nome = som
        .nome_do_timbre()
        .map_or_else(|| "Sem SoundFont".to_owned(), str::to_owned);
    let familia = FAMILIAS[familia_de(timbre.banco, timbre.programa)];
    let mut escolhido = None;
    let largura = if compacto { 220.0 } else { 236.0 };
    let id = ui.make_persistent_id("piano.seletor-instrumento");
    let botao = ui
        .add_sized(
            vec2(largura, if compacto { 26.0 } else { 30.0 }),
            egui::Button::new(
                RichText::new(format!("{}  {}", icones::glifo(familia.icone), nome)).size(14.0),
            )
            .right_text(icones::glifo("expandir")),
        )
        .on_hover_text("Escolher o instrumento (timbre do SoundFont), com busca");
    if botao.clicked() {
        estado.busca.clear();
    }
    if !compacto {
        ui.label(
            RichText::new(format!(
                "{} · programa {}{}",
                familia.nome,
                u16::from(timbre.programa) + 1,
                if timbre.banco == 0 {
                    String::new()
                } else {
                    format!(" · banco {}", timbre.banco)
                }
            ))
            .small()
            .weak(),
        );
    }
    egui::Popup::from_toggle_button_response(&botao)
        .id(id)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .width(600.0)
        .show(|ui| {
            escolhido = painel(ui, som, estado);
            if escolhido.is_some() {
                ui.close();
            }
        });
    escolhido
}

fn painel(ui: &mut Ui, som: &ServicoSom, estado: &mut EstadoSeletor) -> Option<(u16, u8)> {
    let mut escolhido = None;
    ui.set_min_width(580.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(icones::glifo("buscar")).size(16.0));
        let r = ui.add(
            TextEdit::singleline(&mut estado.busca)
                .hint_text("Buscar instrumento: piano, cordas, 41…")
                .desired_width(f32::INFINITY),
        );
        if !r.has_focus() && estado.busca.is_empty() {
            r.request_focus();
        }
    });
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        if ui
            .selectable_label(estado.familia.is_none(), "Todas")
            .clicked()
        {
            estado.familia = None;
        }
        for (i, f) in FAMILIAS.iter().enumerate() {
            let marcada = estado.familia == Some(i);
            if ui
                .selectable_label(marcada, format!("{} {}", icones::glifo(f.icone), f.nome))
                .clicked()
            {
                estado.familia = if marcada { None } else { Some(i) };
            }
        }
    });
    ui.separator();
    let presets = som.presets();
    if presets.is_empty() {
        ui.label("Nenhum SoundFont carregado. Abra Sons e timbres para escolher um arquivo .sf2.");
        return None;
    }
    let atual = som.timbre();
    let grupos = agrupar(presets, estado.familia, &estado.busca);
    ScrollArea::vertical()
        .max_height(420.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            if grupos.is_empty() {
                ui.label(RichText::new("Nada encontrado.").weak());
            }
            for (f, lista) in &grupos {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "{}  {}",
                        icones::glifo(FAMILIAS[*f].icone),
                        FAMILIAS[*f].nome.to_uppercase()
                    ))
                    .small()
                    .strong()
                    .weak(),
                );
                for p in lista {
                    let marcado = p.banco == atual.banco && p.programa == atual.programa;
                    let r = linha(ui, &p.nome, u16::from(p.programa) + 1, p.banco, marcado);
                    if r.clicked() {
                        escolhido = Some((p.banco, p.programa));
                    }
                }
            }
        });
    escolhido
}

/// Uma linha da lista: nome à esquerda, programa (e banco) à direita, fundo no foco.
fn linha(ui: &mut Ui, nome: &str, programa: u16, banco: u16, marcado: bool) -> egui::Response {
    let largura = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(largura, 26.0), egui::Sense::click());
    let v = ui.visuals();
    let fundo = if marcado {
        Some(v.selection.bg_fill.gamma_multiply(0.35))
    } else if resp.hovered() {
        Some(v.widgets.hovered.weak_bg_fill)
    } else {
        None
    };
    let p = ui.painter();
    if let Some(f) = fundo {
        p.rect_filled(rect, egui::CornerRadius::same(4), f);
    }
    if marcado {
        p.rect_filled(
            egui::Rect::from_min_size(rect.min + vec2(0.0, 5.0), vec2(3.0, 16.0)),
            egui::CornerRadius::same(2),
            v.selection.bg_fill,
        );
    }
    let fonte = egui::FontId::proportional(14.0);
    p.text(
        rect.left_center() + vec2(12.0, 0.0),
        egui::Align2::LEFT_CENTER,
        nome,
        fonte.clone(),
        v.text_color(),
    );
    let direita = if banco == 0 {
        format!("{programa}")
    } else {
        format!("{programa} · banco {banco}")
    };
    p.text(
        rect.right_center() - vec2(10.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        direita,
        egui::FontId::monospace(12.0),
        v.weak_text_color(),
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Volume compacto do Ribbon: trilho fino com a parte cheia na cor de destaque. Devolve o
/// novo valor (0–1) quando o usuário arrasta ou clica, e se soltou.
pub fn volume(ui: &mut Ui, valor: f32, largura: f32) -> (Option<f32>, bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(largura, 18.0), egui::Sense::click_and_drag());
    let resp = resp.on_hover_text(format!("Volume geral: {:.0}%", valor * 100.0));
    let v = ui.visuals();
    let cheio = v.selection.bg_fill;
    let trilho = v.widgets.inactive.bg_fill;
    let meio = rect.center().y;
    let r = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 6.0, meio - 2.0),
        egui::pos2(rect.right() - 6.0, meio + 2.0),
    );
    let x = r.left() + r.width() * valor.clamp(0.0, 1.0);
    let p = ui.painter();
    p.rect_filled(r, egui::CornerRadius::same(2), trilho);
    p.rect_filled(
        egui::Rect::from_min_max(r.min, egui::pos2(x, r.max.y)),
        egui::CornerRadius::same(2),
        cheio,
    );
    let raio = if resp.hovered() || resp.dragged() {
        7.0
    } else {
        6.0
    };
    p.circle(
        egui::pos2(x, meio),
        raio,
        v.widgets.inactive.fg_stroke.color,
        egui::Stroke::new(2.0, cheio),
    );
    let novo = resp
        .interact_pointer_pos()
        .filter(|_| resp.dragged() || resp.clicked())
        .map(|pos| ((pos.x - r.left()) / r.width()).clamp(0.0, 1.0));
    (novo, resp.drag_stopped() || resp.clicked())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn p(banco: u16, programa: u8, nome: &str) -> PresetSf2 {
        PresetSf2 {
            banco,
            programa,
            nome: nome.to_owned(),
        }
    }

    #[test]
    fn familias_seguem_o_general_midi() {
        assert_eq!(FAMILIAS[familia_de(0, 0)].nome, "Pianos");
        assert_eq!(FAMILIAS[familia_de(0, 21)].nome, "Órgãos e acordeons");
        assert_eq!(FAMILIAS[familia_de(0, 40)].nome, "Cordas");
        assert_eq!(FAMILIAS[familia_de(0, 127)].nome, "Efeitos sonoros");
        assert_eq!(FAMILIAS[familia_de(BANCO_BATERIA, 0)].nome, "Bateria");
    }

    #[test]
    fn busca_ignora_acento_caixa_e_aceita_numero_e_familia() {
        let acordeon = p(0, 21, "Accordion");
        assert!(combina(&acordeon, "ACCORD"));
        assert!(combina(&acordeon, "orgaos"), "família sem acento");
        assert!(combina(&acordeon, "22"), "programa 1–128");
        assert!(combina(&acordeon, ""));
        assert!(!combina(&acordeon, "piano"));
        assert!(combina(&p(0, 0, "Yamaha Grand Piano"), "grand piano"));
    }

    #[test]
    fn agrupa_por_familia_e_filtra() {
        let lista = [
            p(0, 0, "Grand Piano"),
            p(0, 4, "Rhodes EP"),
            p(0, 40, "Violin"),
            p(128, 0, "Standard"),
        ];
        let g = agrupar(&lista, None, "");
        assert_eq!(
            g.iter().map(|(f, l)| (*f, l.len())).collect::<Vec<_>>(),
            vec![(0, 2), (5, 1), (16, 1)]
        );
        let so_piano = agrupar(&lista, Some(0), "rhodes");
        assert_eq!(so_piano.len(), 1);
        assert_eq!(so_piano[0].1[0].nome, "Rhodes EP");
        assert!(agrupar(&lista, Some(5), "piano").is_empty());
    }

    #[test]
    fn normaliza_portugues() {
        assert_eq!(normalizar("Órgão Elétrico Ç"), "orgao eletrico c");
    }
}
