//! Tela Timbres (F03, #38): SoundFont, lista de presets, atalhos de timbre, volume, Pânico e
//! diagnóstico da saída. Só desenha e devolve pedidos ([`AcaoTimbres`]); quem executa é o app,
//! porque o Pânico também limpa o Espelho.

use crate::som::{
    ConfigTeclado, EstadoSom, PROGRAMA_ACORDEON, PROGRAMA_ORGAO, PROGRAMA_PIANO, SF2_PADRAO,
    ServicoSom, Timbre,
};
use bancada_casca::egui::{self, Color32, RichText, ScrollArea, Slider, TextEdit, Ui};
use piano_tutor_core::NotaMidi;
use piano_tutor_core::teclado::rotulo;

const VERDE: Color32 = Color32::from_rgb(98, 200, 120);
const AMBAR: Color32 = Color32::from_rgb(255, 214, 102);
const PANICO_FUNDO: Color32 = Color32::from_rgb(178, 34, 34);

/// Estado da tela (campos digitados).
#[derive(Debug, Clone, PartialEq)]
pub struct EstadoTimbres {
    pub campo_caminho: String,
    pub filtro: String,
    /// Último erro de escolha de timbre (preset inexistente etc.).
    pub erro_timbre: Option<String>,
    /// Download do SoundFont padrão: texto e fração (0–1) enquanto baixa, preenchidos pelo app.
    pub download_texto: Option<String>,
    pub download_fracao: Option<f32>,
}

impl Default for EstadoTimbres {
    fn default() -> Self {
        Self {
            campo_caminho: SF2_PADRAO.to_owned(),
            filtro: String::new(),
            erro_timbre: None,
            download_texto: None,
            download_fracao: None,
        }
    }
}

/// O que a tela pediu.
#[derive(Debug, Clone, PartialEq)]
pub enum AcaoTimbres {
    CarregarSf2(String),
    /// Usa o SF2 padrão encontrado no sistema ou baixado; se não houver, baixa.
    UsarPadrao,
    Timbre {
        banco: u16,
        programa: u8,
    },
    Volume(f32),
    Panico,
    ReabrirSaida,
    /// Divisão, camada, transposição e efeitos.
    ConfigTeclado(ConfigTeclado),
}

/// Sons favoritos (programas GM) para troca rápida.
pub const FAVORITOS: [(&str, u8); 12] = [
    ("Piano", 0),
    ("Piano elétrico", 4),
    ("Vibrafone", 11),
    ("Órgão", 19),
    ("Acordeon", 21),
    ("Violão", 24),
    ("Baixo", 32),
    ("Cordas", 48),
    ("Coro", 52),
    ("Sax", 65),
    ("Flauta", 73),
    ("Pad", 89),
];

/// Desenha a tela e devolve os pedidos deste quadro.
pub fn desenhar_tela(ui: &mut Ui, som: &ServicoSom, t: &mut EstadoTimbres) -> Vec<AcaoTimbres> {
    let mut pedidos = Vec::new();
    ScrollArea::vertical()
        .id_salt("timbres-vertical")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading("Timbres");
            desenhar_estado(ui, som, &mut pedidos);
            ui.add_space(8.0);
            ui.separator();
            desenhar_soundfont(ui, som, t, &mut pedidos);
            ui.add_space(8.0);
            ui.separator();
            desenhar_controles(ui, som, t, &mut pedidos);
            ui.add_space(8.0);
            ui.separator();
            desenhar_teclado(ui, som, &mut pedidos);
            ui.add_space(8.0);
            ui.separator();
            desenhar_presets(ui, som, t, &mut pedidos);
            ui.add_space(8.0);
            ui.separator();
            desenhar_diagnostico(ui, som);
        });
    pedidos
}

fn desenhar_estado(ui: &mut Ui, som: &ServicoSom, pedidos: &mut Vec<AcaoTimbres>) {
    let cor = if som.estado() == EstadoSom::Pronto {
        VERDE
    } else {
        AMBAR
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(som.texto_status()).strong().color(cor));
        if som.carregando().is_some() {
            ui.spinner();
        }
    });
    if let Some(c) = som.carregando() {
        ui.label(format!("Lendo {} …", c.display()));
    }
    if let Some(d) = som.descricao_saida() {
        ui.label(RichText::new(d).weak());
    }
    if let Some(e) = som.erro_saida() {
        ui.label(RichText::new(e).color(AMBAR));
        if som.caminho_sf2().is_some() && ui.button("Reabrir saída de áudio").clicked() {
            pedidos.push(AcaoTimbres::ReabrirSaida);
        }
    }
    if let Some(e) = som.erro_sf2() {
        ui.label(RichText::new(e).color(AMBAR));
    }
}

fn desenhar_soundfont(
    ui: &mut Ui,
    som: &ServicoSom,
    t: &mut EstadoTimbres,
    pedidos: &mut Vec<AcaoTimbres>,
) {
    ui.label(RichText::new("SoundFont (.sf2)").strong());
    match som.caminho_sf2() {
        Some(c) => ui.label(format!("Em uso: {}", c.display())),
        None => ui.label(RichText::new("Nenhum SoundFont em uso.").color(AMBAR)),
    };
    ui.horizontal_wrapped(|ui| {
        let r = ui.add(
            TextEdit::singleline(&mut t.campo_caminho)
                .hint_text(SF2_PADRAO)
                .desired_width(460.0),
        );
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.button("Carregar").clicked() || enter {
            pedidos.push(AcaoTimbres::CarregarSf2(t.campo_caminho.trim().to_owned()));
        }
        if ui
            .button("Usar o padrão")
            .on_hover_text(
                "FluidR3_GM (MIT): do sistema, ou baixado uma vez para a sua pasta de dados",
            )
            .clicked()
        {
            pedidos.push(AcaoTimbres::UsarPadrao);
        }
    });
    if let Some(texto) = &t.download_texto {
        ui.label(RichText::new(texto).color(AMBAR));
    }
    if let Some(f) = t.download_fracao {
        ui.add(
            egui::ProgressBar::new(f)
                .show_percentage()
                .desired_width(460.0),
        );
    }
}

fn desenhar_controles(
    ui: &mut Ui,
    som: &ServicoSom,
    t: &mut EstadoTimbres,
    pedidos: &mut Vec<AcaoTimbres>,
) {
    let atual = som.timbre();
    let nome = som.nome_do_timbre().unwrap_or("(sem SoundFont)");
    ui.label(
        RichText::new(format!(
            "Timbre atual: {}:{} {nome}",
            atual.banco, atual.programa
        ))
        .strong(),
    );
    ui.horizontal_wrapped(|ui| {
        for (rotulo, programa) in [
            ("Piano (GM 0)", PROGRAMA_PIANO),
            ("Acordeon (GM 21)", PROGRAMA_ACORDEON),
            ("Órgão (GM 19)", PROGRAMA_ORGAO),
        ] {
            let existe = som.tem_preset(0, programa);
            let marcado = atual.banco == 0 && atual.programa == programa;
            let b = ui
                .add_enabled(existe, egui::Button::new(rotulo).selected(marcado))
                .on_disabled_hover_text("Este preset não existe no SoundFont em uso");
            if b.clicked() {
                pedidos.push(AcaoTimbres::Timbre { banco: 0, programa });
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Favoritos:");
        for (rotulo, programa) in FAVORITOS {
            let existe = som.tem_preset(0, programa);
            let marcado = atual.banco == 0 && atual.programa == programa;
            if ui
                .add_enabled(existe, egui::Button::new(rotulo).selected(marcado))
                .clicked()
            {
                pedidos.push(AcaoTimbres::Timbre { banco: 0, programa });
            }
        }
    });
    if let Some(e) = &t.erro_timbre {
        ui.label(RichText::new(e).color(AMBAR));
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Volume");
        let mut v = som.volume() * 100.0;
        let r = ui.add(Slider::new(&mut v, 0.0..=100.0).suffix(" %").integer());
        if r.changed() {
            pedidos.push(AcaoTimbres::Volume(v / 100.0));
        }
    });
    ui.add_space(6.0);
    let panico = egui::Button::new(
        RichText::new("Pânico — cortar todo o som (Ctrl+.)")
            .strong()
            .color(Color32::WHITE),
    )
    .fill(PANICO_FUNDO)
    .min_size(egui::vec2(320.0, 34.0));
    if ui
        .add(panico)
        .on_hover_text("All-notes-off imediato: corta todas as vozes e o sustain, limpa o Espelho")
        .clicked()
    {
        pedidos.push(AcaoTimbres::Panico);
    }
}

fn combo_timbre(ui: &mut Ui, id: &str, som: &ServicoSom, t: &mut Timbre) {
    let nome = som
        .presets()
        .iter()
        .find(|p| p.banco == t.banco && p.programa == t.programa)
        .map_or_else(|| format!("{}:{}", t.banco, t.programa), |p| p.nome.clone());
    egui::ComboBox::from_id_salt(id)
        .selected_text(nome)
        .width(220.0)
        .show_ui(ui, |ui| {
            for p in som.presets().iter().filter(|p| p.banco <= 127) {
                let v = Timbre {
                    banco: p.banco,
                    programa: p.programa,
                };
                ui.selectable_value(t, v, format!("{:03} {}", p.programa, p.nome));
            }
        });
}

fn nome_nota(n: u8) -> String {
    NotaMidi::nova(n)
        .map(|x| crate::tela_espelho::texto_tela(&rotulo(x, Default::default())))
        .unwrap_or_default()
}

/// Divisão (split), camada (layer), transposição e efeitos.
fn desenhar_teclado(ui: &mut Ui, som: &ServicoSom, pedidos: &mut Vec<AcaoTimbres>) {
    let antes = som.config_teclado();
    let mut c = antes;
    ui.label(RichText::new("Teclado: divisão, camada, transposição e efeitos").strong());
    ui.horizontal_wrapped(|ui| {
        let mut dividir = c.divisao.is_some();
        ui.checkbox(
            &mut dividir,
            "Dividir o teclado (mão esquerda com outro som)",
        );
        let mut ponto = c.divisao.unwrap_or(54);
        if dividir {
            ui.label("a partir de");
            ui.add(Slider::new(&mut ponto, 36..=96).custom_formatter(|v, _| nome_nota(v as u8)));
            ui.label("esquerda:");
            combo_timbre(ui, "timbre-esquerda", som, &mut c.timbre_esquerda);
        }
        c.divisao = dividir.then_some(ponto);
    });
    ui.horizontal_wrapped(|ui| {
        let mut camada = c.camada.is_some();
        ui.checkbox(&mut camada, "Camada (somar um segundo som)");
        let mut t = c.camada.unwrap_or(Timbre {
            banco: 0,
            programa: 48,
        });
        if camada {
            combo_timbre(ui, "timbre-camada", som, &mut t);
        }
        c.camada = camada.then_some(t);
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Transposição");
        ui.add(Slider::new(&mut c.transposicao, -12..=12).suffix(" semitons"));
        ui.label("Reverb");
        ui.add(Slider::new(&mut c.reverb, 0..=127));
        ui.label("Coro");
        ui.add(Slider::new(&mut c.coro, 0..=127));
        if ui.button("Restaurar padrão").clicked() {
            c = ConfigTeclado::default();
        }
    });
    if c != antes {
        pedidos.push(AcaoTimbres::ConfigTeclado(c));
    }
}

fn desenhar_presets(
    ui: &mut Ui,
    som: &ServicoSom,
    t: &mut EstadoTimbres,
    pedidos: &mut Vec<AcaoTimbres>,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Presets do SoundFont").strong());
        ui.label(RichText::new(format!("({})", som.presets().len())).weak());
        ui.add(
            TextEdit::singleline(&mut t.filtro)
                .hint_text("filtrar por nome")
                .desired_width(200.0),
        );
    });
    if som.presets().is_empty() {
        ui.label(RichText::new("Nenhum preset: carregue um SoundFont.").weak());
        return;
    }
    let filtro = t.filtro.trim().to_lowercase();
    let atual = som.timbre();
    ScrollArea::vertical()
        .id_salt("timbres-presets")
        .max_height(300.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for p in som.presets() {
                if !filtro.is_empty() && !p.nome.to_lowercase().contains(&filtro) {
                    continue;
                }
                let texto = format!("{:03}:{:03}  {}", p.banco, p.programa, p.nome);
                if p.banco > 127 {
                    ui.add_enabled(
                        false,
                        egui::Label::new(format!("{texto}  (percussão, canal 10)")),
                    );
                    continue;
                }
                let marcado = p.banco == atual.banco && p.programa == atual.programa;
                if ui
                    .selectable_label(marcado, RichText::new(texto).monospace())
                    .clicked()
                {
                    pedidos.push(AcaoTimbres::Timbre {
                        banco: p.banco,
                        programa: p.programa,
                    });
                }
            }
        });
}

fn desenhar_diagnostico(ui: &mut Ui, som: &ServicoSom) {
    ui.label(RichText::new("Diagnóstico da saída").strong());
    match som.telemetria() {
        Some(t) => {
            ui.label(format!(
                "Frames {} · callbacks {} · xruns {} · erros do dispositivo {} · eventos \
                 atrasados {} · amostras recortadas {} · reancoragens {}",
                t.frames,
                t.callbacks,
                t.xruns,
                t.erros_dispositivo,
                t.comandos_atrasados,
                t.amostras_recortadas,
                t.reancoragens
            ));
        }
        None => {
            ui.label(RichText::new("Sem saída de áudio aberta.").weak());
        }
    }
    ui.label(
        RichText::new(format!(
            "Comandos perdidos (fila cheia) {} · pânicos {} · reaberturas {}",
            som.comandos_perdidos(),
            som.panicos(),
            som.reaberturas()
        ))
        .weak(),
    );
    ui.label(
        RichText::new(
            "Latência MIDI-áudio não é medida aqui: a medição por captura/loopback é feita no \
             PC de referência.",
        )
        .weak(),
    );
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::som::apoio_testes::servico_pronto;
    use bancada_casca::egui::{Context, RawInput, Rect, pos2, vec2};
    use piano_tutor_audio::RelogioManual;
    use std::sync::Arc;

    fn quadro(ctx: &Context, som: &ServicoSom, t: &mut EstadoTimbres) -> Vec<AcaoTimbres> {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
            ..Default::default()
        };
        let mut pedidos = Vec::new();
        ctx.run_ui(entrada, |ui| pedidos = desenhar_tela(ui, som, t))
            .drop_without_applying_deltas();
        pedidos
    }

    #[test]
    fn desenha_inativo_e_pronto_sem_panico_e_sem_pedidos_espontaneos() {
        let ctx = Context::default();
        let mut t = EstadoTimbres::default();
        let inativo = ServicoSom::inativo(Arc::new(RelogioManual::novo()));
        assert!(quadro(&ctx, &inativo, &mut t).is_empty());
        let (pronto, _m) = servico_pronto(Arc::new(RelogioManual::novo()));
        for _ in 0..2 {
            assert!(quadro(&ctx, &pronto, &mut t).is_empty());
        }
        t.filtro = "seno".into();
        assert!(quadro(&ctx, &pronto, &mut t).is_empty());
    }
}
