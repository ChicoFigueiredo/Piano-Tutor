//! Adaptador do layout de partitura para o egui (ticket 06): único ponto em que as primitivas
//! geométricas próprias (`geometria`) viram `Shape`s do egui, mais o estado e o componente de
//! tela (rolagem, cache). O cálculo do layout e o hit test não passam por aqui.

use super::*;
use crate::geometria::Alinha;
use bancada_casca::egui::{
    Align, Align2, Color32, FontId, Painter, Pos2, Rect, ScrollArea, Sense, Shape, Stroke, Ui,
    pos2, vec2,
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
}

impl Paleta {
    pub fn para_tema(escuro: bool) -> Self {
        if escuro {
            Self {
                papel: Color32::from_rgb(0x1E, 0x21, 0x29),
                tinta: Color32::from_rgb(0xE8, 0xE4, 0xDA),
                suave: Color32::from_rgb(0x8C, 0x8A, 0x84),
                direita: Color32::from_rgb(0xE2, 0xA2, 0x4B),
                esquerda: Color32::from_rgb(0x4F, 0xA3, 0xC7),
                acerto: Color32::from_rgb(0x4C, 0xC3, 0x6B),
                erro: Color32::from_rgb(0xE8, 0x5A, 0x50),
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
        let cor = cor_do_item_com(it, pal, marcas.destaque, marcas.resultados);
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
        }
    }
}

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
    /// Compasso clicado.
    pub compasso_clicado: Option<usize>,
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
            let l = Arc::new(match (fita, marcas.ids) {
                (true, Some(ids)) => calcular_layout_fita_com_ids(p, ids),
                (true, None) => calcular_layout_fita(p),
                (false, Some(ids)) => calcular_layout_com_ids(p, ids, largura_sp),
                (false, None) => calcular_layout(p, largura_sp),
            });
            estado.cache = Some((chave, estado.versao, l.clone()));
            estado.ultimo_sistema = None;
            l
        }
    };
    estado.ultimo_layout = Some(layout.clone());
    let pal = Paleta::para_tema(ui.visuals().dark_mode && !opc.papel_branco);
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
            let _ = i;
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
            pintar_itens(&painter, origem, esp, &s.itens, &pal, marcas, true);
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
        }
    });
    resposta
}
