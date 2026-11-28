//! PDF original ao lado da edição (ticket 20, A17; achado R5 da revisão E01).
//!
//! Na aba Partitura, quando o documento aberto é de um item da Oficina com PDF/páginas, um painel
//! lateral redimensionável (e que se esconde) mostra a página do PDF, levada ao compasso da
//! seleção ou do cursor de entrada. Toda a regra compasso→página/região vem de
//! [`crate::pdf_sincronizado`] (a mesma da aba Revisão); aqui só há o estado do painel e o
//! desenho. O PDF e as páginas são só lidos, nunca escritos.
//!
//! Sem PDF (item sem páginas) ou arquivo solto, o painel não existe: fica uma dica discreta.

use super::EstadoPartitura;
use crate::pdf_sincronizado::{AVISO_SEM_PDF, Destino, MapaCompassos, PedidoDeRolagem};
use crate::revisao::{ItemPartitura, ler_png};
use crate::som::ServicoSom;
use bancada_casca::egui::{
    self, Button, Color32, ColorImage, Image, Panel, Rect, RichText, ScrollArea, Slider,
    TextureHandle, TextureOptions, Ui, load::SizedTexture, vec2,
};
use std::path::PathBuf;
use std::time::SystemTime;

const ZOOM_MIN: f32 = 0.1;
const ZOOM_MAX: f32 = 4.0;
const LARGURA_INICIAL: f32 = 380.0;
const LARGURA_MINIMA: f32 = 160.0;
const AMARELO: Color32 = Color32::from_rgb(230, 190, 70);

/// Dica quando o documento aberto não é de um item da Oficina.
pub const DICA_ARQUIVO_SOLTO: &str =
    "PDF ao lado: só para partituras da Oficina com PDF de origem. Este é um arquivo solto.";

/// Estado do painel do PDF ao lado. Vazio (`Default`) = nada ligado.
pub struct PdfLateral {
    paginas: Vec<PathBuf>,
    mapa: MapaCompassos,
    /// Painel aberto (o usuário pode escondê-lo).
    visivel: bool,
    pagina_atual: usize,
    compasso_visto: Option<usize>,
    compasso_alvo: Option<usize>,
    destino: Option<Destino>,
    rolar: Option<PedidoDeRolagem>,
    y_visivel: f32,
    zoom: f32,
    ajustar: bool,
    chave: Option<(PathBuf, Option<SystemTime>)>,
    textura: Option<TextureHandle>,
    tamanho: [usize; 2],
    erro: Option<String>,
}

impl Default for PdfLateral {
    fn default() -> Self {
        Self {
            paginas: Vec::new(),
            mapa: MapaCompassos::default(),
            visivel: true,
            pagina_atual: 0,
            compasso_visto: None,
            compasso_alvo: None,
            destino: None,
            rolar: None,
            y_visivel: 0.0,
            zoom: 1.0,
            ajustar: true,
            chave: None,
            textura: None,
            tamanho: [0, 0],
            erro: None,
        }
    }
}

impl PdfLateral {
    /// Liga o painel às páginas e ao mapa do item da Oficina.
    pub fn do_item(item: &ItemPartitura) -> Self {
        Self {
            paginas: item.paginas.clone(),
            mapa: item.mapa.clone(),
            ..Self::default()
        }
    }

    /// Há PDF/páginas para mostrar.
    pub fn tem_pdf(&self) -> bool {
        !self.paginas.is_empty() && self.mapa.tem_pdf()
    }

    /// Leitura do estado para os testes de tela (o desenho usa os campos direto).
    #[cfg(test)]
    pub fn visivel(&self) -> bool {
        self.visivel
    }

    #[cfg(test)]
    pub fn alternar(&mut self) {
        self.visivel = !self.visivel;
    }

    #[cfg(test)]
    pub fn pagina(&self) -> usize {
        self.pagina_atual
    }

    #[cfg(test)]
    pub fn destino(&self) -> Option<Destino> {
        self.destino
    }

    #[cfg(test)]
    pub fn compasso_alvo(&self) -> Option<usize> {
        self.compasso_alvo
    }

    #[cfg(test)]
    pub fn rolagem_pendente(&self) -> Option<PedidoDeRolagem> {
        self.rolar
    }

    #[cfg(test)]
    pub fn tamanho_da_pagina(&self) -> Option<[usize; 2]> {
        self.textura.as_ref().map(|_| self.tamanho)
    }

    #[cfg(test)]
    pub fn erro(&self) -> Option<&str> {
        self.erro.as_deref()
    }

    /// Aviso discreto: mapa só por página ou sem mapa (nada antes de apontar um compasso).
    pub fn aviso(&self) -> Option<&'static str> {
        self.compasso_alvo?;
        self.mapa.aviso(self.destino)
    }

    /// Compasso (a partir de 0) que a rolagem atual do PDF indica.
    pub fn compasso_indicado(&self) -> Option<usize> {
        self.mapa
            .compasso_na_pagina(self.pagina_atual, self.y_visivel)
    }

    /// Vai à página `n` (a partir de 0), no topo; fora do PDF, não faz nada.
    pub fn ir_para_pagina(&mut self, n: usize) {
        if n < self.paginas.len() {
            self.pagina_atual = n;
            self.rolar = Some(PedidoDeRolagem { pagina: n, y: 0.0 });
            self.y_visivel = 0.0;
        }
    }

    /// Se o compasso em foco mudou, leva o painel a ele. `true` = algo mudou.
    pub fn sincronizar(&mut self, foco: Option<usize>) -> bool {
        if foco == self.compasso_visto {
            return false;
        }
        self.compasso_visto = foco;
        let Some(c) = foco else {
            return false;
        };
        self.compasso_alvo = Some(c);
        self.destino = self.mapa.destino_do_compasso(c);
        if let Some(d) = self.destino {
            self.pagina_atual = d.pagina;
            self.rolar = Some(d.rolagem());
            self.y_visivel = d.rolagem().y;
        }
        true
    }

    /// Decodifica a página atual (PNG) e cria a textura, se ainda não há (limites de `ler_png`).
    fn garantir_pagina(&mut self, ctx: &egui::Context) {
        let Some(caminho) = self.paginas.get(self.pagina_atual).cloned() else {
            return;
        };
        let mtime = std::fs::metadata(&caminho).and_then(|m| m.modified()).ok();
        let chave = (caminho.clone(), mtime);
        if self.chave.as_ref() == Some(&chave) {
            return;
        }
        self.chave = Some(chave);
        self.textura = None;
        self.erro = None;
        self.tamanho = [0, 0];
        match ler_png(&caminho) {
            Ok(img) => {
                let cor = ColorImage::from_rgba_unmultiplied([img.largura, img.altura], &img.rgba);
                self.textura = Some(ctx.load_texture(
                    format!("pdf-lateral-{}", caminho.display()),
                    cor,
                    TextureOptions::LINEAR,
                ));
                self.tamanho = [img.largura, img.altura];
            }
            Err(e) => self.erro = Some(e),
        }
    }

    /// Conteúdo do painel: navegação, zoom e a página com a região destacada.
    fn desenhar_conteudo(&mut self, ui: &mut Ui) {
        self.garantir_pagina(ui.ctx());
        let total = self.paginas.len();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("PDF original").strong());
            if ui.small_button("Esconder").clicked() {
                self.visivel = false;
            }
        });
        ui.horizontal_wrapped(|ui| {
            let atual = self.pagina_atual;
            if ui
                .add_enabled(atual > 0, Button::new("◀"))
                .on_hover_text("Página anterior do PDF")
                .clicked()
            {
                self.ir_para_pagina(atual - 1);
            }
            ui.label(format!("página {} de {total}", atual + 1));
            if ui
                .add_enabled(atual + 1 < total, Button::new("▶"))
                .on_hover_text("Próxima página do PDF")
                .clicked()
            {
                self.ir_para_pagina(atual + 1);
            }
            if let Some(c) = self.compasso_alvo {
                ui.label(format!("compasso {}", c + 1));
            }
        });
        if let Some(c) = self.compasso_indicado() {
            ui.label(RichText::new(format!("PDF indica o compasso {}", c + 1)).weak());
        }
        if let Some(a) = self.aviso() {
            ui.label(RichText::new(a).weak());
        }
        let largura = (ui.available_width() - 16.0).max(100.0);
        let [w, h] = self.tamanho;
        let zoom_auto = if w > 0 {
            (largura / w as f32).clamp(ZOOM_MIN, ZOOM_MAX)
        } else {
            1.0
        };
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().slider_width = 70.0;
            let mut z = if self.ajustar { zoom_auto } else { self.zoom };
            let r = ui.add(
                Slider::new(&mut z, ZOOM_MIN..=ZOOM_MAX)
                    .logarithmic(true)
                    .text("zoom")
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
            );
            if r.changed() {
                self.zoom = z;
                self.ajustar = false;
            }
            if ui.small_button("Ajustar").clicked() {
                self.ajustar = true;
            }
        });
        let Some(textura) = self.textura.clone() else {
            let msg = match (&self.erro, self.paginas.get(self.pagina_atual)) {
                (Some(e), _) => format!("Não consegui ler a página: {e}"),
                (None, Some(c)) => format!("Página não encontrada: {}", c.display()),
                (None, None) => "Esta partitura não tem página de PDF.".to_owned(),
            };
            ui.label(RichText::new(msg).weak());
            return;
        };
        let zoom = if self.ajustar {
            zoom_auto
        } else {
            self.zoom.clamp(ZOOM_MIN, ZOOM_MAX)
        };
        let tam = vec2(w as f32 * zoom, h as f32 * zoom);
        let regiao = self
            .destino
            .filter(|d| d.pagina == self.pagina_atual)
            .and_then(|d| d.regiao);
        let mut area = ScrollArea::both()
            .id_salt("pdf-lateral-pagina")
            .auto_shrink([false, false]);
        // A rolagem só vale quando a textura mostrada já é a da página pedida.
        let textura_e_da_pagina =
            self.paginas.get(self.pagina_atual) == self.chave.as_ref().map(|(c, _)| c);
        if textura_e_da_pagina && let Some(p) = self.rolar.take() {
            area = area.vertical_scroll_offset(p.y * tam.y);
        }
        let saida = area.show(ui, |ui| {
            let r = ui.add(Image::new(SizedTexture::new(textura.id(), tam)));
            if let Some(g) = regiao {
                let caixa = Rect::from_min_size(
                    r.rect.min + vec2(g.x * tam.x, g.y * tam.y),
                    vec2(g.largura * tam.x, g.altura * tam.y),
                );
                ui.painter()
                    .rect_filled(caixa, 0.0, AMARELO.gamma_multiply(0.25));
                ui.painter().rect_stroke(
                    caixa,
                    0.0,
                    egui::Stroke::new(2.0, AMARELO),
                    egui::StrokeKind::Outside,
                );
            }
        });
        if tam.y > 0.0 {
            self.y_visivel = (saida.state.offset.y / tam.y).clamp(0.0, 1.0);
        }
    }
}

/// Desenha a aba Partitura com o PDF ao lado, quando existe. `corpo` desenha o resto da tela.
pub(super) fn desenhar_com_pdf(
    ui: &mut Ui,
    e: &mut EstadoPartitura,
    som: &mut ServicoSom,
    corpo: fn(&mut Ui, &mut EstadoPartitura, &mut ServicoSom),
) {
    // O vínculo com a Oficina é `versoes`; sem ele, o item guardado (se houver) não vale.
    let da_oficina = e.versoes.is_some();
    let mut p = std::mem::take(&mut e.pdf_lateral);
    if da_oficina {
        p.sincronizar(e.compasso_em_foco());
    }
    if da_oficina && p.tem_pdf() {
        if p.visivel {
            Panel::right("pdf_lateral")
                .resizable(true)
                .default_size(LARGURA_INICIAL)
                .size_range(LARGURA_MINIMA..=f32::INFINITY)
                .show(ui, |ui| p.desenhar_conteudo(ui));
        } else if ui
            .small_button("Mostrar PDF ao lado")
            .on_hover_text("Reabre o painel com o PDF original")
            .clicked()
        {
            p.visivel = true;
        }
    } else {
        let dica = if da_oficina {
            AVISO_SEM_PDF
        } else {
            DICA_ARQUIVO_SOLTO
        };
        ui.label(RichText::new(dica).weak().small());
    }
    e.pdf_lateral = p;
    corpo(ui, e, som);
    // Um clique ou tecla na partitura leva o PDF no quadro seguinte.
    if e.versoes.is_some() && e.pdf_lateral.sincronizar(e.compasso_em_foco()) {
        ui.ctx().request_repaint();
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::atalhos_partitura::MovimentoDeEntrada;
    use crate::selecao_partitura::Comando;
    use crate::tela_partitura::EMBUTIDAS;
    use bancada_casca::egui::{Context, RawInput, pos2};
    use piano_tutor_audio::RelogioManual;
    use piano_tutor_core::tempo::Relogio;
    use sha2::{Digest, Sha256};
    use std::path::Path;
    use std::sync::Arc;

    fn png() -> Vec<u8> {
        let mut saida = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut saida, 4, 6);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().expect("cabeçalho");
            w.write_image_data(&[200u8; 4 * 6 * 3]).expect("dados");
        }
        saida
    }

    fn hash(caminho: &Path) -> String {
        Sha256::digest(std::fs::read(caminho).expect("lê"))
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// Oficina com `ode` (MusicXML da Hipótese) e o `estado.toml` dado; devolve a raiz.
    fn oficina(extra: &str) -> (tempfile::TempDir, EstadoPartitura, PathBuf) {
        let d = tempfile::tempdir().expect("pasta");
        let raiz = d.path().join("oficina");
        let pasta = raiz.join("partituras");
        std::fs::create_dir_all(&pasta).expect("cria");
        std::fs::create_dir_all(raiz.join("paginas")).expect("cria");
        for n in 1..=2 {
            std::fs::write(raiz.join(format!("paginas/p-{n}.png")), png()).expect("png");
        }
        std::fs::write(raiz.join("original.pdf"), b"%PDF-1.4 falso para o teste\n").expect("pdf");
        let xml = pasta.join("ode.musicxml");
        std::fs::write(&xml, EMBUTIDAS[3].1).expect("hipótese");
        std::fs::write(
            pasta.join("ode.estado.toml"),
            format!("estado = \"hipotese\"\n{extra}"),
        )
        .expect("toml");
        let mut e = EstadoPartitura::novo(None);
        e.abrir_caminho(&xml.to_string_lossy());
        assert!(e.erro.is_none(), "{:?}", e.erro);
        (d, e, raiz)
    }

    const COM_PDF: &str = "pdf = \"original.pdf\"\n\
        paginas = [\"paginas/p-1.png\", \"paginas/p-2.png\"]\n\
        [[mapa_compassos]]\ncompasso = 1\npagina = 1\n\
        [[mapa_compassos]]\ncompasso = 3\npagina = 2\nregiao = [0.1, 0.5, 0.8, 0.1]\n";

    fn banco() -> (ServicoSom, Context) {
        let (som, _m) = crate::som::apoio_testes::servico_pronto(
            Arc::new(RelogioManual::novo()) as Arc<dyn Relogio>
        );
        (som, Context::default())
    }

    fn quadro(ctx: &Context, e: &mut EstadoPartitura, som: &mut ServicoSom, largura: f32) {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(largura, 800.0))),
            ..Default::default()
        };
        ctx.run_ui(entrada, |ui| super::super::desenhar_tela(ui, e, som))
            .drop_without_applying_deltas();
    }

    fn apontar(e: &mut EstadoPartitura, c: usize) {
        for _ in 0..200 {
            if e.compasso_em_foco() == Some(c) {
                return;
            }
            e.navegar(Comando::Seguinte);
        }
        panic!("não achei o compasso {c}");
    }

    #[test]
    fn com_pdf_o_painel_aparece_e_acompanha_a_selecao_de_nota() {
        let (_d, mut e, _) = oficina(COM_PDF);
        let (mut som, ctx) = banco();
        quadro(&ctx, &mut e, &mut som, 1280.0);
        assert!(e.pdf_lateral.tem_pdf() && e.pdf_lateral.visivel());
        assert_eq!(e.pdf_lateral.pagina(), 0);
        apontar(&mut e, 2);
        for _ in 0..3 {
            quadro(&ctx, &mut e, &mut som, 1280.0);
        }
        assert_eq!(e.pdf_lateral.compasso_alvo(), Some(2));
        assert_eq!(e.pdf_lateral.pagina(), 1, "compasso 3 está na página 2");
        let d = e.pdf_lateral.destino().expect("destino");
        assert!(d.regiao.is_some() && !d.por_pagina);
        assert!(
            e.pdf_lateral.rolagem_pendente().is_none(),
            "rolagem aplicada"
        );
        assert_eq!(e.pdf_lateral.tamanho_da_pagina(), Some([4, 6]));
        assert!(e.pdf_lateral.erro().is_none());
    }

    #[test]
    fn cursor_de_entrada_tambem_leva_o_pdf_e_sem_regiao_informa_por_pagina() {
        let (_d, mut e, _) = oficina(COM_PDF);
        let (mut som, ctx) = banco();
        quadro(&ctx, &mut e, &mut som, 1280.0);
        apontar(&mut e, 0);
        quadro(&ctx, &mut e, &mut som, 1280.0);
        assert!(e.pdf_lateral.destino().is_some_and(|d| d.por_pagina));
        assert_eq!(
            e.pdf_lateral.aviso(),
            Some(crate::pdf_sincronizado::AVISO_POR_PAGINA)
        );
        // Anda só o cursor de entrada até o compasso mudar.
        let antes = e.compasso_em_foco();
        for _ in 0..80 {
            e.mover_entrada(MovimentoDeEntrada::Seguinte);
            if e.compasso_em_foco() != antes {
                break;
            }
        }
        assert_ne!(e.compasso_em_foco(), antes, "o cursor mudou de compasso");
        quadro(&ctx, &mut e, &mut som, 1280.0);
        assert_eq!(e.pdf_lateral.compasso_alvo(), e.compasso_em_foco());
    }

    #[test]
    fn esconder_e_mostrar_alterna_o_painel_sem_perder_a_pagina() {
        let (_d, mut e, _) = oficina(COM_PDF);
        let (mut som, ctx) = banco();
        quadro(&ctx, &mut e, &mut som, 1280.0);
        apontar(&mut e, 2);
        quadro(&ctx, &mut e, &mut som, 1280.0);
        e.pdf_lateral.alternar();
        assert!(!e.pdf_lateral.visivel());
        quadro(&ctx, &mut e, &mut som, 1280.0);
        assert_eq!(e.pdf_lateral.pagina(), 1);
        e.pdf_lateral.alternar();
        quadro(&ctx, &mut e, &mut som, 1280.0);
        assert!(e.pdf_lateral.visivel());
    }

    #[test]
    fn painel_aceita_larguras_estreitas_e_largas_sem_panico() {
        let (_d, mut e, _) = oficina(COM_PDF);
        let (mut som, ctx) = banco();
        for l in [300.0, 480.0, 1920.0] {
            quadro(&ctx, &mut e, &mut som, l);
        }
    }

    #[test]
    fn sem_pdf_nao_ha_painel_e_a_edicao_segue() {
        let (_d, mut e, _) = oficina("");
        let (mut som, ctx) = banco();
        quadro(&ctx, &mut e, &mut som, 1280.0);
        assert!(e.versoes.is_some());
        assert!(!e.pdf_lateral.tem_pdf());
        apontar(&mut e, 1);
        quadro(&ctx, &mut e, &mut som, 1280.0);
        assert_eq!(e.pdf_lateral.tamanho_da_pagina(), None);
    }

    #[test]
    fn arquivo_solto_nao_tem_painel_nem_usa_item_antigo() {
        let (_d, mut e, _) = oficina(COM_PDF);
        let (mut som, ctx) = banco();
        quadro(&ctx, &mut e, &mut som, 1280.0);
        apontar(&mut e, 2);
        quadro(&ctx, &mut e, &mut som, 1280.0);
        quadro(&ctx, &mut e, &mut som, 1280.0);
        assert!(e.pdf_lateral.tamanho_da_pagina().is_some());
        // Outro documento (solto): o vínculo some e o painel não é mais desenhado.
        e.abrir_xml("solto", EMBUTIDAS[3].1.to_owned())
            .expect("abre");
        assert!(e.versoes.is_none());
        let pagina = e.pdf_lateral.pagina();
        apontar(&mut e, 1);
        quadro(&ctx, &mut e, &mut som, 1280.0);
        assert_eq!(e.pdf_lateral.pagina(), pagina, "painel inerte");
    }

    #[test]
    fn pdf_e_paginas_nunca_sao_alterados_pelo_painel_nem_pela_edicao() {
        let (_d, mut e, raiz) = oficina(COM_PDF);
        let arquivos = [
            raiz.join("original.pdf"),
            raiz.join("paginas/p-1.png"),
            raiz.join("paginas/p-2.png"),
        ];
        let antes: Vec<String> = arquivos.iter().map(|a| hash(a)).collect();
        let (mut som, ctx) = banco();
        for c in [0, 1, 2] {
            apontar(&mut e, c);
            for _ in 0..3 {
                quadro(&ctx, &mut e, &mut som, 1280.0);
            }
        }
        e.pdf_lateral.ir_para_pagina(0);
        e.pdf_lateral.alternar();
        e.pdf_lateral.alternar();
        quadro(&ctx, &mut e, &mut som, 1280.0);
        let depois: Vec<String> = arquivos.iter().map(|a| hash(a)).collect();
        assert_eq!(antes, depois);
    }
}
