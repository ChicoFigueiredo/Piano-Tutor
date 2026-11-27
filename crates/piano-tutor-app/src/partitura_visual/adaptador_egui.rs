//! Adaptador do layout de partitura para o egui (ticket 06): único ponto em que as primitivas
//! geométricas próprias (`geometria`) viram `Shape`s do egui, mais o estado e o componente de
//! tela (rolagem, cache). O cálculo do layout e o hit test não passam por aqui.

use super::*;
use crate::geometria::Alinha;
use bancada_casca::egui::{
    Align, Align2, Color32, EventFilter, FontFamily, FontId, Id, Painter, Pos2, Rect, ScrollArea,
    Sense, Shape, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, pos2, vec2,
};

/// Cores da partitura (tokens do tema do app: Noite e Claro).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paleta {
    pub papel: Color32,
    pub tinta: Color32,
    pub suave: Color32,
    pub direita: Color32,
    pub esquerda: Color32,
    /// Nota acertada na prática.
    pub acerto: Color32,
    /// Nota errada ou perdida na prática.
    pub erro: Color32,
    /// Nota selecionada e foco do teclado (ticket 12): anel e cor da nota. Contraste mínimo
    /// contra o papel em `selecao_partitura::CONTRASTE_MINIMO` (e `_ALTO` no alto contraste).
    pub selecao: Color32,
}

impl Paleta {
    pub fn para_tema(escuro: bool) -> Self {
        Self::para_tema_com(escuro, false)
    }

    /// Como [`Paleta::para_tema`], com a opção de **alto contraste** da seleção (amarelo sobre o
    /// papel escuro, azul puro sobre o claro).
    pub fn para_tema_com(escuro: bool, alto_contraste: bool) -> Self {
        let mut p = Self::base(escuro);
        if alto_contraste {
            p.selecao = if escuro {
                Color32::from_rgb(0xFF, 0xFF, 0x00)
            } else {
                Color32::from_rgb(0x00, 0x00, 0xFF)
            };
        }
        p
    }

    fn base(escuro: bool) -> Self {
        if escuro {
            Self {
                papel: Color32::from_rgb(0x1E, 0x21, 0x29),
                tinta: Color32::from_rgb(0xE8, 0xE4, 0xDA),
                suave: Color32::from_rgb(0x8C, 0x8A, 0x84),
                direita: Color32::from_rgb(0xE2, 0xA2, 0x4B),
                esquerda: Color32::from_rgb(0x4F, 0xA3, 0xC7),
                acerto: Color32::from_rgb(0x4C, 0xC3, 0x6B),
                erro: Color32::from_rgb(0xE8, 0x5A, 0x50),
                selecao: Color32::from_rgb(0xFF, 0x6F, 0xE0),
            }
        } else {
            Self {
                papel: Color32::from_rgb(0xFF, 0xFF, 0xFF),
                tinta: Color32::from_rgb(0x1D, 0x1B, 0x17),
                suave: Color32::from_rgb(0x6B, 0x67, 0x5F),
                direita: Color32::from_rgb(0xB5, 0x65, 0x1D),
                esquerda: Color32::from_rgb(0x1F, 0x6F, 0x8B),
                acerto: Color32::from_rgb(0x1B, 0x8A, 0x3C),
                erro: Color32::from_rgb(0xC6, 0x28, 0x28),
                selecao: Color32::from_rgb(0x7B, 0x1F, 0xA2),
            }
        }
    }

    pub(super) fn mao(&self, m: Mao) -> Color32 {
        match m {
            Mao::Direita => self.direita,
            Mao::Esquerda => self.esquerda,
        }
    }
}

#[cfg(test)]
pub(super) fn cor_do_item(it: &Item, pal: &Paleta, destaque: Option<Destaque>) -> Color32 {
    cor_do_item_com(it, pal, destaque, None)
}

pub(super) fn cor_do_item_com(
    it: &Item,
    pal: &Paleta,
    destaque: Option<Destaque>,
    resultados: Option<&MapaResultados>,
) -> Color32 {
    if let (Some(a), Some(r)) = (it.alvo.as_ref(), resultados)
        && let Some(res) = r.de(a)
    {
        return match res {
            ResultadoNota::Acertou => pal.acerto,
            ResultadoNota::Errou => pal.erro,
        };
    }
    if let (Some(a), Some(d)) = (it.alvo, destaque)
        && a.compasso == d.compasso
        && d.tempo + 1e-9 >= a.inicio
        && d.tempo < a.fim - 1e-9
    {
        return pal.mao(a.mao);
    }
    match it.cor {
        Cor::Tinta => pal.tinta,
        Cor::Suave => pal.suave,
    }
}

/// Cor com que a primitiva é pintada: a nota selecionada (cabeça, pontos de aumento e acidente)
/// ganha a cor da seleção acima de tudo; as demais seguem destaque, resultado e tinta.
pub(super) fn cor_final(it: &Item, pal: &Paleta, marcas: Marcas<'_>) -> Color32 {
    match marcas.selecao.and_then(|s| s.nota) {
        Some(sel) if it.nota == Some(sel) => pal.selecao,
        _ => cor_do_item_com(it, pal, marcas.destaque, marcas.resultados),
    }
}

fn espessura_px(sp: f32, esp: f32) -> f32 {
    (sp * esp).max(1.0)
}

fn alinha(a: Alinha) -> Align {
    match a {
        Alinha::Min => Align::Min,
        Alinha::Centro => Align::Center,
        Alinha::Max => Align::Max,
    }
}

/// Âncora própria -> `Align2` do egui.
fn align2(a: Ancora) -> Align2 {
    Align2([alinha(a.0[0]), alinha(a.0[1])])
}

/// Elipse em pixels (egui), pelos mesmos pontos do layout.
fn elipse_egui(c: Pos2, rx: f32, ry: f32, rot: f32, n: usize) -> Vec<Pos2> {
    elipse_pontos(ponto(c.x, c.y), rx, ry, rot, n)
        .into_iter()
        .map(|p| pos2(p.x, p.y))
        .collect()
}

/// Pinta as primitivas de um sistema. `origem` é o canto superior esquerdo do layout.
pub fn pintar_itens(
    painter: &Painter,
    origem: Pos2,
    esp: f32,
    itens: &[Item],
    pal: &Paleta,
    marcas: Marcas<'_>,
    com_texto: bool,
) {
    let px = |p: Ponto| pos2(origem.x + p.x * esp, origem.y + p.y * esp);
    // Só pinta o que cruza a região visível (a fita contínua tem milhares de primitivas).
    let corte = painter.clip_rect().expand(MARGEM_CORTE_SP * esp);
    for it in itens {
        let lim = it.forma.limites();
        // Defesa final: forma com coordenada não finita nunca chega ao epaint.
        if !(lim.min.is_finite() && lim.max.is_finite()) || !forma_finita(&it.forma) {
            continue;
        }
        if !corte.intersects(Rect::from_min_max(px(lim.min), px(lim.max))) {
            continue;
        }
        let cor = cor_final(it, pal, marcas);
        match &it.forma {
            Forma::Linha { a, b, espessura } => {
                let (mut pa, mut pb) = (px(*a), px(*b));
                if (pa.y - pb.y).abs() < 1e-3 {
                    pa.y = pa.y.round();
                    pb.y = pb.y.round();
                }
                painter.add(Shape::line_segment(
                    [pa, pb],
                    Stroke::new(espessura_px(*espessura, esp), cor),
                ));
            }
            Forma::Preenchido { pontos } => {
                painter.add(Shape::convex_polygon(
                    pontos.iter().map(|p| px(*p)).collect(),
                    cor,
                    Stroke::NONE,
                ));
            }
            Forma::Caminho { pontos, espessura } => {
                painter.add(Shape::line(
                    pontos.iter().map(|p| px(*p)).collect(),
                    Stroke::new((espessura * esp).max(0.8), cor),
                ));
            }
            Forma::Cabeca {
                centro,
                rx,
                ry,
                rotacao,
                oca,
                rotacao_oco,
            } => {
                let c = px(*centro);
                painter.add(Shape::convex_polygon(
                    elipse_egui(c, rx * esp, ry * esp, *rotacao, 28),
                    cor,
                    Stroke::NONE,
                ));
                if *oca {
                    let (irx, iry) = if rotacao_oco.abs() > 1.0 {
                        (rx * 0.34, ry * 0.8)
                    } else {
                        (rx * 0.72, ry * 0.32)
                    };
                    painter.add(Shape::convex_polygon(
                        elipse_egui(c, irx * esp, iry * esp, *rotacao_oco, 24),
                        pal.papel,
                        Stroke::NONE,
                    ));
                }
            }
            Forma::Disco { centro, raio } => {
                let r = (raio * esp).max(0.7);
                painter.add(Shape::convex_polygon(
                    elipse_egui(px(*centro), r, r, 0.0, 20),
                    cor,
                    Stroke::NONE,
                ));
            }
            Forma::Texto {
                pos,
                texto,
                altura,
                ancora,
            } => {
                if com_texto {
                    // Altura de algarismo ≈ 0,7 do corpo da fonte.
                    let corpo = (altura * esp / 0.72).max(11.0);
                    painter.text(
                        px(*pos),
                        align2(*ancora),
                        texto,
                        FontId::proportional(corpo),
                        cor,
                    );
                }
            }
            Forma::Glifo {
                pos,
                caractere,
                escala,
            } => {
                if com_texto {
                    pintar_glifo(painter, px(*pos), esp, *caractere, *escala, cor);
                }
            }
        }
    }
}

/// Pinta um glifo da Bravura com a origem da linha de base em `alvo`. Sem a família `Bravura`
/// nas fontes do contexto (a casca a registra na partida), cai no algarismo comum.
fn pintar_glifo(painter: &Painter, alvo: Pos2, esp: f32, c: char, escala: f32, cor: Color32) {
    let corpo = bravura::CORPO_EM_SP * esp * escala;
    if !(corpo.is_finite() && corpo > 0.05) {
        return;
    }
    if !bravura::bravura_registrada(painter.ctx()) {
        // O glifo `fingeringN` tem o algarismo `N`: o texto comum o substitui sem enganar.
        let n = (c as u32).saturating_sub(0xED10);
        painter.text(
            pos2(alvo.x, alvo.y),
            Align2::LEFT_BOTTOM,
            n.to_string(),
            FontId::proportional((bravura::ALTURA_DO_ALGARISMO * esp * escala / 0.72).max(11.0)),
            cor,
        );
        return;
    }
    let galley = painter.layout_no_wrap(
        c.to_string(),
        FontId::new(corpo, FontFamily::Name(bravura::FAMILIA.into())),
        cor,
    );
    // A linha de base do glifo no galley: posição da linha mais a posição do glifo.
    let (bx, by) = galley.rows.first().map_or((0.0, 0.0), |r| {
        let g = r.glyphs.first();
        (
            r.pos.x + g.map_or(0.0, |g| g.pos.x),
            r.pos.y + g.map_or(0.0, |g| g.pos.y),
        )
    });
    painter.galley(pos2(alvo.x - bx, alvo.y - by), galley, cor);
}

/// Raio do anel de seleção, em sp (maior que a cabeça de nota, que tem 0,64 sp de meia largura).
const RAIO_ANEL_SP: f32 = 1.25;

type ChaveLayout = (u32, u32, ModoRolagem);

/// Estado persistente do componente (cache do layout e rolagem).
#[derive(Default)]
pub struct EstadoVisual {
    cache: Option<(ChaveLayout, u64, Arc<Layout>)>,
    versao: u64,
    ultimo_sistema: Option<usize>,
    /// Último layout calculado (para testes e consultas).
    pub ultimo_layout: Option<Arc<Layout>>,
}

impl EstadoVisual {
    /// Descarta o layout em cache (chamar ao trocar de partitura).
    pub fn invalidar(&mut self) {
        self.versao += 1;
        self.cache = None;
        self.ultimo_sistema = None;
        self.ultimo_layout = None;
    }
}

/// Como a partitura acompanha a execução.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ModoRolagem {
    /// Sistemas empilhados; ao chegar ao seguinte a vista salta para ele (atual no topo).
    #[default]
    Sistemas,
    /// Um único sistema comprido, rolado na horizontal com o cursor parado a 1/3 da largura.
    Fita,
}

impl ModoRolagem {
    pub const TODOS: [ModoRolagem; 2] = [ModoRolagem::Sistemas, ModoRolagem::Fita];

    pub fn rotulo(self) -> &'static str {
        match self {
            ModoRolagem::Sistemas => "Sistema a sistema",
            ModoRolagem::Fita => "Fita contínua",
        }
    }

    /// Nome estável usado nas preferências salvas.
    pub fn chave(self) -> &'static str {
        match self {
            ModoRolagem::Sistemas => "sistemas",
            ModoRolagem::Fita => "fita",
        }
    }

    pub fn de_chave(s: &str) -> Option<Self> {
        Self::TODOS.into_iter().find(|m| m.chave() == s.trim())
    }
}

/// Opções de exibição.
#[derive(Clone, Copy, Debug)]
pub struct OpcoesVisuais {
    pub zoom: f32,
    /// Acompanhar a execução (rolagem automática).
    pub seguir: bool,
    pub modo: ModoRolagem,
    /// Força a paleta clara (papel branco, tinta escura) mesmo com o tema Noite.
    pub papel_branco: bool,
}

impl Default for OpcoesVisuais {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            seguir: true,
            modo: ModoRolagem::Sistemas,
            papel_branco: false,
        }
    }
}

/// O que o usuário fez na partitura neste quadro.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RespostaPartitura {
    /// Compasso clicado (mesmo quando o clique caiu numa nota).
    pub compasso_clicado: Option<usize>,
    /// Alvo do clique: a nota sob o ponteiro (se o layout tiver identidades) ou o compasso.
    pub acerto: Option<Acerto>,
    /// A vista da partitura tem o foco do teclado neste quadro (só com `Marcas::selecao`).
    pub com_foco: bool,
    /// Identidade do widget da vista (para pedir foco por programa e em testes).
    pub id: Option<Id>,
}

/// Seleção de nota e foco do teclado nesta vista (ticket 12). Sem ela a vista só mostra e
/// continua reportando o compasso clicado, como antes.
#[derive(Clone, Copy, Debug, Default)]
pub struct MarcaSelecao<'a> {
    /// Nota selecionada (por identidade).
    pub nota: Option<IdNota>,
    /// Nota a trazer para a vista neste quadro (depois de navegar pelo teclado).
    pub rolar_para: Option<IdNota>,
    /// `Tab` pertence à partitura (há mais de uma voz ou pauta): não troca o foco da janela.
    pub captura_tab: bool,
    /// Seleção em alto contraste.
    pub alto_contraste: bool,
    /// Descrição em texto da nota selecionada (rótulo de acessibilidade).
    pub descricao: &'a str,
}

/// Compasso em seleção (ponto de partida) e destaque de execução.
#[derive(Clone, Copy, Debug, Default)]
pub struct Marcas<'a> {
    pub destaque: Option<Destaque>,
    pub selecionado: Option<usize>,
    /// Notas acertadas (verde) e erradas (vermelho) da prática.
    pub resultados: Option<&'a MapaResultados>,
    /// Identidades das notas do documento: com elas o layout carrega a identidade em cada nota.
    /// O layout fica em cache até `EstadoVisual::invalidar`: troque o documento invalidando.
    pub ids: Option<&'a IdsPorNota>,
    /// Letra, cifra e dedilhado a desenhar junto das notas (ticket 19); só vale com `ids`. Fica
    /// em cache com o layout, como `ids`.
    pub exibicao: Option<&'a Exibicao>,
    /// Seleção de nota e foco do teclado (ticket 12); `None` = vista só de leitura.
    pub selecao: Option<MarcaSelecao<'a>>,
}

/// Desenha a partitura com rolagem própria. A largura do layout acompanha a do `ui`.
pub fn desenhar_partitura(
    ui: &mut Ui,
    p: &Partitura,
    estado: &mut EstadoVisual,
    opc: OpcoesVisuais,
    marcas: Marcas<'_>,
) -> RespostaPartitura {
    let zoom = opc.zoom.clamp(ZOOM_MIN, ZOOM_MAX);
    let esp = ESP_BASE_PX * zoom;
    let fita = opc.modo == ModoRolagem::Fita;
    let largura_px = (ui.available_width() - 12.0).max(120.0);
    let altura_vista = ui.available_height();
    let largura_sp = (largura_px / esp).max(LARGURA_MINIMA);
    // Na fita a largura da vista não muda o layout: a chave só distingue o modo.
    let chave: ChaveLayout = (
        if fita {
            0
        } else {
            (largura_sp * 8.0).round() as u32
        },
        (zoom * 1000.0).round() as u32,
        opc.modo,
    );
    let layout = match &estado.cache {
        Some((k, v, l)) if *k == chave && *v == estado.versao => l.clone(),
        _ => {
            let l = Arc::new(match (fita, marcas.ids, marcas.exibicao) {
                (true, Some(ids), Some(ex)) => calcular_layout_fita_com_exibicao(p, ids, ex),
                (true, Some(ids), None) => calcular_layout_fita_com_ids(p, ids),
                (true, None, _) => calcular_layout_fita(p),
                (false, Some(ids), Some(ex)) => {
                    calcular_layout_com_exibicao(p, ids, ex, largura_sp)
                }
                (false, Some(ids), None) => calcular_layout_com_ids(p, ids, largura_sp),
                (false, None, _) => calcular_layout(p, largura_sp),
            });
            estado.cache = Some((chave, estado.versao, l.clone()));
            estado.ultimo_sistema = None;
            l
        }
    };
    estado.ultimo_layout = Some(layout.clone());
    let pal = Paleta::para_tema_com(
        ui.visuals().dark_mode && !opc.papel_branco,
        marcas.selecao.is_some_and(|s| s.alto_contraste),
    );
    let cabeca_selecionada = marcas
        .selecao
        .and_then(|s| s.nota)
        .and_then(|id| layout.cabeca_da_nota(id));
    let cabeca_a_mostrar = marcas
        .selecao
        .and_then(|s| s.rolar_para)
        .and_then(|id| layout.cabeca_da_nota(id));
    let mut resposta = RespostaPartitura::default();
    let mut area = ScrollArea::both()
        .id_salt(if fita {
            "partitura-fita"
        } else {
            "partitura-rolagem"
        })
        .auto_shrink([false, false]);
    if fita
        && opc.seguir
        && let Some(d) = marcas.destaque
        && let Some(x) = layout.x_no_compasso(d.compasso, d.tempo)
    {
        // Cursor parado a 1/3 da largura visível: a fita é que anda, a cada quadro.
        area = area.horizontal_scroll_offset((x * esp - largura_px * FRACAO_CURSOR_FITA).max(0.0));
    }
    area.show(ui, |ui| {
        let altura_px = layout.altura * esp + 8.0;
        let tamanho = vec2(
            layout.largura * esp,
            if fita {
                altura_px.max(altura_vista)
            } else {
                altura_px
            },
        );
        let (resp, painter) = ui.allocate_painter(tamanho, Sense::click());
        resposta.id = Some(resp.id);
        if let Some(sel) = marcas.selecao {
            let rotulo = if sel.descricao.is_empty() {
                "Partitura. Setas esquerda e direita escolhem a nota; Tab troca de voz.".to_owned()
            } else {
                format!("Partitura. Nota selecionada: {}", sel.descricao)
            };
            resp.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, rotulo.clone()));
            resposta.com_foco = resp.has_focus();
            if resp.gained_focus() {
                // O filtro de teclas só vale a partir do quadro seguinte ao do foco: garante um.
                ui.ctx().request_repaint();
            }
            if resposta.com_foco {
                // Setas (horizontais para navegar e, desde o ticket 14, verticais para mudar a
                // altura) e Tab, havendo mais de uma voz, são da partitura; Esc devolve o foco
                // à janela, então o teclado nunca fica preso. Sem as verticais o egui moveria o
                // foco para outro controle ao primeiro ↑ ou ↓.
                ui.ctx().memory_mut(|m| {
                    m.set_focus_lock_filter(
                        resp.id,
                        EventFilter {
                            tab: sel.captura_tab,
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            escape: false,
                        },
                    );
                });
            }
        }
        // Na fita o sistema fica centrado na vertical, sobre o papel que ocupa a vista.
        let origem = resp.rect.min + vec2(0.0, ((tamanho.y - altura_px) / 2.0).max(0.0).floor());
        painter.rect_filled(resp.rect, 4.0, pal.papel);
        let visivel = ui.clip_rect();
        for (i, s) in layout.sistemas.iter().enumerate() {
            let faixa = Rect::from_min_max(
                pos2(origem.x, origem.y + (s.y_topo - 6.0) * esp),
                pos2(
                    origem.x + layout.largura * esp,
                    origem.y + (s.y_base + 6.0) * esp,
                ),
            );
            if !faixa.intersects(visivel) {
                continue;
            }
            // Seleção e compasso atual.
            for c in s.primeiro..s.fim {
                let cl = &layout.compassos[c];
                let r = Rect::from_min_max(
                    pos2(origem.x + cl.x0 * esp, origem.y + (s.y_topo - 1.0) * esp),
                    pos2(origem.x + cl.x1 * esp, origem.y + (s.y_base + 1.0) * esp),
                );
                if marcas.selecionado == Some(c) {
                    painter.rect_filled(
                        r,
                        2.0,
                        Color32::from_rgba_unmultiplied(
                            pal.esquerda.r(),
                            pal.esquerda.g(),
                            pal.esquerda.b(),
                            26,
                        ),
                    );
                }
                if marcas.destaque.map(|d| d.compasso) == Some(c) {
                    painter.rect_filled(
                        r,
                        2.0,
                        Color32::from_rgba_unmultiplied(
                            pal.direita.r(),
                            pal.direita.g(),
                            pal.direita.b(),
                            14,
                        ),
                    );
                }
            }
            if let Some(d) = marcas.destaque
                && (s.primeiro..s.fim).contains(&d.compasso)
                && let Some(x) = layout.x_no_compasso(d.compasso, d.tempo)
            {
                let r = Rect::from_min_max(
                    pos2(
                        origem.x + (x - 1.1) * esp,
                        origem.y + (s.y_topo - 1.5) * esp,
                    ),
                    pos2(
                        origem.x + (x + 1.1) * esp,
                        origem.y + (s.y_base + 1.5) * esp,
                    ),
                );
                painter.rect_filled(
                    r,
                    2.0,
                    Color32::from_rgba_unmultiplied(
                        pal.direita.r(),
                        pal.direita.g(),
                        pal.direita.b(),
                        38,
                    ),
                );
            }
            // Anel da nota selecionada, sob as notas: forma além da cor (mais grosso com foco).
            if let Some((sis, c)) = cabeca_selecionada
                && sis == i
            {
                let centro = pos2(origem.x + c.x * esp, origem.y + c.y * esp);
                let raio = RAIO_ANEL_SP * esp;
                let (r, g, b) = (pal.selecao.r(), pal.selecao.g(), pal.selecao.b());
                painter.circle_filled(centro, raio, Color32::from_rgba_unmultiplied(r, g, b, 56));
                let grossura = if resposta.com_foco { 3.0 } else { 1.5 };
                painter.circle_stroke(centro, raio, Stroke::new(grossura, pal.selecao));
            }
            pintar_itens(&painter, origem, esp, &s.itens, &pal, marcas, true);
        }
        if marcas.selecao.is_some() && resposta.com_foco {
            // Moldura de foco na região visível: o foco do teclado é sempre localizável.
            ui.painter().rect_stroke(
                visivel.shrink(1.5),
                3.0,
                Stroke::new(3.0, pal.selecao),
                StrokeKind::Inside,
            );
        }
        if let Some((_, c)) = cabeca_a_mostrar {
            let alvo = Rect::from_center_size(
                pos2(origem.x + c.x * esp, origem.y + c.y * esp),
                vec2(8.0 * esp, 10.0 * esp),
            );
            ui.scroll_to_rect(alvo, None);
        }
        if let Some(d) = marcas.destaque
            && opc.seguir
            && !fita
        {
            let atual = layout.compassos.get(d.compasso).map(|c| c.sistema);
            if atual.is_some() && atual != estado.ultimo_sistema {
                if let Some(s) = atual.and_then(|i| layout.sistemas.get(i)) {
                    // Sistema atual no topo (com folga para número de compasso e casas):
                    // o seguinte fica visível logo abaixo, para o leitor olhar adiante.
                    let r = Rect::from_min_max(
                        pos2(origem.x, origem.y + (s.y_topo - 4.0) * esp),
                        pos2(origem.x + 10.0, origem.y + (s.y_base + 3.0) * esp),
                    );
                    ui.scroll_to_rect(r, Some(Align::Min));
                }
                estado.ultimo_sistema = atual;
            }
        } else if marcas.destaque.is_none() {
            estado.ultimo_sistema = None;
        }
        if resp.clicked()
            && let Some(pos) = resp.interact_pointer_pos()
        {
            let (x, y) = ((pos.x - origem.x) / esp, (pos.y - origem.y) / esp);
            resposta.compasso_clicado = layout.compasso_em(x, y);
            resposta.acerto = layout.acertar(ponto(x, y), TOLERANCIA_NOTA_SP);
        }
    });
    resposta
}
