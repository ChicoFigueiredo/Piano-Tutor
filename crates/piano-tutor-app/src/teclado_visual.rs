//! Teclado de 61 teclas desenhado com acabamento, como componente reutilizável.
//!
//! Só pinta e acerta o ponto do mouse: o estado (teclas ligadas, guias, nomes) vem do
//! [`EstadoEspelho`]. A geometria e o hit-test são os de `espelho` ([`geometria`], [`tecla_em`]),
//! então o que se vê é o que o mouse acerta.
//!
//! Identidade (docs/referencias/COMPONENTES-VISUAIS.md): brancas em marfim fosco, pretas em
//! grafite com filete claro no alto, sombra só sob o conjunto. Tecla ligada **afunda 2 px** e
//! recebe a cor da mão em ~40% (direita âmbar, esquerda azul; sem ponto de divisão, âmbar).
//! Tecla-guia ganha contorno de 2 px na cor da mão, pulso lento e ponto.
//!
//! Acessibilidade: a cor nunca é o único sinal. Tecla ligada ganha contorno grosso e marca de
//! forma (círculo na branca, losango na preta; cheia na mão direita, vazada na esquerda); a
//! guia tem ponto; a errada tem um "X" desenhado e o nome da nota.

use crate::espelho::{
    ALTURA_BRANCA, EstadoEspelho, GeometriaTecla, MARGEM, ZOOM_MAX, ZOOM_MIN, geometria,
    largura_total, tecla_em,
};
use crate::tela_espelho::{rotulo_tela, texto_tela};
use bancada_casca::egui::epaint::{Mesh, Shadow};
use bancada_casca::egui::{
    Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2,
    Visuals, pos2, vec2,
};
use piano_tutor_core::NotaMidi;
use piano_tutor_core::teclado::{CorTecla, Idioma, nome_sem_oitava};

/// Largura de uma tecla branca com zoom 1,0, em pixels.
pub const LARGURA_BRANCA_BASE_PX: f32 = 24.0;
/// Espaço abaixo das teclas para a marca do Dó central, em pixels.
pub const FAIXA_DO_CENTRAL_PX: f32 = 30.0;
/// Largura mínima de tecla para escrever rótulos nela.
pub const LARGURA_MIN_ROTULO_PX: f32 = 15.0;
/// Velocidade assumida quando nenhuma foi informada.
pub const VELOCIDADE_PADRAO: u8 = 100;
/// Quanto a tecla ligada afunda, em pixels.
pub const AFUNDA_PX: f32 = 2.0;
/// Período do pulso da tecla-guia, em segundos.
pub const PERIODO_PULSO_S: f64 = 1.2;
/// Tamanho mínimo de texto da identidade visual.
const TEXTO_PX: f32 = 11.0;

const AMBAR: Color32 = Color32::from_rgb(255, 214, 102);
const PRETO: Color32 = Color32::BLACK;
const BRANCO: Color32 = Color32::WHITE;

/// Quais rótulos de nota aparecem nas teclas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rotulos {
    /// Modo compacto: só as marcas de forma (tecla ligada, guia e errada).
    Nenhum,
    /// Só nas teclas-guia (padrão da identidade visual).
    #[default]
    SoGuias,
    /// Nas guias e nos Dós, com a oitava (referência para se achar no teclado).
    SoDo,
    /// Nome de todas as teclas brancas, mais as guias.
    Todas,
}

/// Aparência e dados extras do teclado.
#[derive(Clone, Debug, PartialEq)]
pub struct EstiloTeclado {
    pub rotulos: Rotulos,
    /// Mostra a letra do teclado do PC que toca a nota (simulação).
    pub letras_pc: bool,
    /// Triângulo e texto "Dó central" abaixo das teclas.
    pub marca_do_central: bool,
    /// Quem posiciona pede a largura que faz as 61 teclas preencherem o espaço (ver
    /// [`largura_branca_ajustada`]); o componente em si não muda a largura recebida.
    pub ajustar_a_largura: bool,
    /// Cor do texto fora das teclas (do tema).
    pub tinta: Color32,
    /// Cor do texto sobre o marfim (secundário).
    pub tinta_suave: Color32,
    /// Cor da caixa que envolve o teclado.
    pub moldura: Color32,
    /// Cor da mão direita (âmbar).
    pub mao_direita: Color32,
    /// Cor da mão esquerda (azul).
    pub mao_esquerda: Color32,
    /// Cor da nota errada.
    pub erro: Color32,
    /// Primeira nota MIDI da mão direita: abaixo dela vale a esquerda. `None` (ponto de divisão
    /// desconhecido) pinta tudo como mão direita.
    pub divisao: Option<u8>,
    /// Pulso da guia: `Some(fase)` com fase de 0 a 1 (ver [`fase_do_pulso`]); `None` desliga o
    /// pulso e deixa o contorno fixo.
    pub pulso: Option<f32>,
    /// Notas tocadas erradas agora: tom de erro, "X" e nome da nota.
    pub erradas: Vec<NotaMidi>,
    /// Velocidade (0 a 127) por nota MIDI: a intensidade do tom acompanha. Nota sem entrada usa
    /// [`VELOCIDADE_PADRAO`].
    pub velocidades: Vec<(u8, u8)>,
}

impl Default for EstiloTeclado {
    fn default() -> Self {
        Self::noite()
    }
}

impl EstiloTeclado {
    /// Tema Noite.
    pub fn noite() -> Self {
        Self {
            rotulos: Rotulos::SoGuias,
            letras_pc: true,
            marca_do_central: true,
            ajustar_a_largura: false,
            tinta: Color32::from_rgb(0xE8, 0xE4, 0xDA),
            tinta_suave: Color32::from_rgb(0x6B, 0x67, 0x5F),
            moldura: Color32::from_rgb(30, 28, 30),
            mao_direita: Color32::from_rgb(0xE2, 0xA2, 0x4B),
            mao_esquerda: Color32::from_rgb(0x4F, 0xA3, 0xC7),
            erro: Color32::from_rgb(0xE8, 0x70, 0x6A),
            divisao: None,
            pulso: Some(1.0),
            erradas: Vec::new(),
            velocidades: Vec::new(),
        }
    }

    /// Tema Claro.
    pub fn claro() -> Self {
        Self {
            tinta: Color32::from_rgb(0x1D, 0x1B, 0x17),
            moldura: Color32::from_rgb(58, 50, 48),
            mao_direita: Color32::from_rgb(0xB5, 0x65, 0x1D),
            mao_esquerda: Color32::from_rgb(0x1F, 0x6F, 0x8B),
            erro: Color32::from_rgb(0xB3, 0x26, 0x1E),
            ..Self::noite()
        }
    }

    /// Tema Ébano.
    pub fn ebano() -> Self {
        Self {
            tinta: Color32::from_rgb(0xE9, 0xDC, 0xC4),
            moldura: Color32::from_rgb(20, 18, 16),
            mao_esquerda: Color32::from_rgb(0x6F, 0xB6, 0xD4),
            ..Self::noite()
        }
    }

    /// Estilo que acompanha o tema do app: claro, escuro ou o escuríssimo "Ébano".
    pub fn para_visuals(v: &Visuals) -> Self {
        let fundo = v.panel_fill;
        let soma = u16::from(fundo.r()) + u16::from(fundo.g()) + u16::from(fundo.b());
        let mut s = if !v.dark_mode {
            Self::claro()
        } else if soma < 45 {
            Self::ebano()
        } else {
            Self::noite()
        };
        s.tinta = v.text_color();
        s
    }

    /// Modo compacto, sem rótulos nem letras do PC.
    pub fn compacto(mut self) -> Self {
        self.rotulos = Rotulos::Nenhum;
        self.letras_pc = false;
        self
    }

    /// Informa a velocidade de uma nota ligada (a intensidade do tom acompanha).
    pub fn com_velocidade(mut self, nota: NotaMidi, velocidade: u8) -> Self {
        self.definir_velocidade(nota, velocidade);
        self
    }

    pub fn definir_velocidade(&mut self, nota: NotaMidi, velocidade: u8) {
        let n = nota.numero();
        let v = velocidade.min(127);
        match self.velocidades.iter_mut().find(|(m, _)| *m == n) {
            Some(par) => par.1 = v,
            None => self.velocidades.push((n, v)),
        }
    }

    fn velocidade_de(&self, nota: NotaMidi) -> u8 {
        self.velocidades
            .iter()
            .find(|(m, _)| *m == nota.numero())
            .map_or(VELOCIDADE_PADRAO, |(_, v)| *v)
    }

    /// `true` se a nota é da mão esquerda (só quando o ponto de divisão é conhecido).
    pub fn eh_mao_esquerda(&self, nota: NotaMidi) -> bool {
        self.divisao.is_some_and(|d| nota.numero() < d)
    }

    /// Cor da mão que toca a nota.
    pub fn cor_da_mao(&self, nota: NotaMidi) -> Color32 {
        if self.eh_mao_esquerda(nota) {
            self.mao_esquerda
        } else {
            self.mao_direita
        }
    }
}

/// Fase do pulso (0 a 1, suave) para o instante `tempo_s`, com período [`PERIODO_PULSO_S`].
pub fn fase_do_pulso(tempo_s: f64) -> f32 {
    let x = (tempo_s / PERIODO_PULSO_S).fract();
    (0.5 - 0.5 * (x * std::f64::consts::TAU).cos()) as f32
}

// ---------------------------------------------------------------------------------------------
// Medidas
// ---------------------------------------------------------------------------------------------

/// Altura da caixa (com o feltro) acima das teclas, em pixels.
pub fn altura_cabecalho(largura_branca: f32) -> f32 {
    (0.55 * largura_branca).clamp(9.0, 18.0)
}

/// Tamanho total a reservar na tela: caixa + teclas + faixa do Dó central.
pub fn tamanho_do_teclado(e: &EstadoEspelho, largura_branca: f32) -> Vec2 {
    vec2(
        largura_total(e.teclado()) * largura_branca,
        altura_cabecalho(largura_branca) + ALTURA_BRANCA * largura_branca + FAIXA_DO_CENTRAL_PX,
    )
}

/// Retângulo das teclas dentro do espaço reservado por [`tamanho_do_teclado`].
pub fn area_das_teclas(reservado: Rect, largura_branca: f32) -> Rect {
    Rect::from_min_size(
        reservado.min + vec2(0.0, altura_cabecalho(largura_branca)),
        vec2(reservado.width(), ALTURA_BRANCA * largura_branca),
    )
}

/// Largura de tecla branca (px) que faz as 61 teclas preencherem `largura_disponivel`, dentro
/// dos limites de zoom.
pub fn largura_branca_ajustada(e: &EstadoEspelho, largura_disponivel: f32) -> f32 {
    let ideal = largura_disponivel / largura_total(e.teclado());
    if ideal.is_finite() {
        ideal.clamp(
            LARGURA_BRANCA_BASE_PX * ZOOM_MIN,
            LARGURA_BRANCA_BASE_PX * ZOOM_MAX,
        )
    } else {
        LARGURA_BRANCA_BASE_PX
    }
}

/// Retângulo (px) de uma tecla, dada a área das teclas e a largura da branca.
pub fn retangulo_da_tecla(area: Rect, largura_branca: f32, g: &GeometriaTecla) -> Rect {
    let origem = area.min + vec2(MARGEM * largura_branca, 0.0);
    Rect::from_min_size(
        origem + vec2(g.x * largura_branca, 0.0),
        vec2(g.largura * largura_branca, g.altura * largura_branca),
    )
}

/// Tecla sob o ponto `p` (px). Mesmo critério do desenho: pretas por cima das brancas.
pub fn tecla_no_ponto(
    geo: &[GeometriaTecla],
    area: Rect,
    largura_branca: f32,
    p: Pos2,
) -> Option<usize> {
    let origem = area.min + vec2(MARGEM * largura_branca, 0.0);
    tecla_em(
        geo,
        (p.x - origem.x) / largura_branca,
        (p.y - origem.y) / largura_branca,
    )
}

// ---------------------------------------------------------------------------------------------
// Primitivas
// ---------------------------------------------------------------------------------------------

fn mistura(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

fn escurecer(c: Color32, fator: f32) -> Color32 {
    mistura(c, PRETO, 1.0 - fator.clamp(0.0, 1.0))
}

/// Cor com transparência (o `Color32` do egui é pré-multiplicado).
fn alfa(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

fn alfa_f(c: Color32, a: f32) -> Color32 {
    alfa(c, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Retângulo com cor por canto: esquerda-cima, direita-cima, direita-baixo, esquerda-baixo.
fn malha(p: &Painter, r: Rect, cores: [Color32; 4]) {
    if r.width() <= 0.0 || r.height() <= 0.0 {
        return;
    }
    let mut m = Mesh::default();
    m.colored_vertex(r.left_top(), cores[0]);
    m.colored_vertex(r.right_top(), cores[1]);
    m.colored_vertex(r.right_bottom(), cores[2]);
    m.colored_vertex(r.left_bottom(), cores[3]);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(Shape::mesh(m));
}

fn degrade_vertical(p: &Painter, r: Rect, topo: Color32, base: Color32) {
    malha(p, r, [topo, topo, base, base]);
}

/// Opacidade do tom da mão na tecla ligada (~40% na velocidade padrão, 25% a 45% no total).
pub fn opacidade_do_tom(velocidade: u8) -> f32 {
    0.25 + 0.2 * f32::from(velocidade.min(127)) / 127.0
}

/// Estado de uma tecla na hora de pintar.
#[derive(Clone, Copy)]
struct Marca {
    ligada: bool,
    guia: bool,
    errada: bool,
    esquerda: bool,
    mao: Color32,
    tom: f32,
    /// Rótulo do dedo sugerido (já na nomenclatura escolhida), quando a Lição informa.
    dedo: Option<&'static str>,
}

// ---------------------------------------------------------------------------------------------
// Desenho
// ---------------------------------------------------------------------------------------------

/// Pinta o teclado inteiro. `area` é o retângulo das teclas (use [`area_das_teclas`]); a caixa
/// com o feltro é pintada logo acima dele e a marca do Dó central logo abaixo.
pub fn pintar_teclado(
    p: &Painter,
    area: Rect,
    largura_branca: f32,
    e: &EstadoEspelho,
    estilo: &EstiloTeclado,
) {
    let w = largura_branca;
    let geo = geometria(e.teclado());
    let ligadas: Vec<usize> = e.teclas().indices_ligados().collect();
    let marca_de = |g: &GeometriaTecla| Marca {
        ligada: ligadas.contains(&g.indice),
        guia: e.guias.contains(&g.nota),
        errada: estilo.erradas.contains(&g.nota),
        esquerda: estilo.eh_mao_esquerda(g.nota),
        mao: estilo.cor_da_mao(g.nota),
        tom: opacidade_do_tom(estilo.velocidade_de(g.nota)),
        dedo: e.rotulo_do_dedo(g.nota),
    };

    pintar_caixa(p, area, w, estilo);
    for passo in [CorTecla::Branca, CorTecla::Preta] {
        for g in geo.iter().filter(|g| g.cor == passo) {
            let r = retangulo_da_tecla(area, w, g);
            let m = marca_de(g);
            match passo {
                CorTecla::Branca => pintar_branca(p, e, estilo, g, r, w, m),
                CorTecla::Preta => pintar_preta(p, e, estilo, g, r, w, m),
            }
        }
    }
    if estilo.marca_do_central {
        pintar_do_central(p, area, w, e, &geo, estilo);
    }
}

/// Caixa escura nas pontas e no alto, com o feltro vermelho acima das teclas; a única sombra do
/// conjunto fica sob ele.
fn pintar_caixa(p: &Painter, area: Rect, w: f32, estilo: &EstiloTeclado) {
    let cab = altura_cabecalho(w);
    let topo = area.min.y - cab;
    let lateral = (MARGEM * w).max(2.0);
    let conjunto = Rect::from_min_max(pos2(area.min.x, topo), area.max);

    let sombra = Shadow {
        offset: [0, 3],
        blur: 10,
        spread: 0,
        color: alfa(PRETO, 70),
    };
    p.add(sombra.as_shape(conjunto, 4));
    // Fundo escuro: aparece nas frestas entre as teclas.
    p.rect_filled(area, 0, Color32::from_rgb(16, 14, 16));

    let caixa = Rect::from_min_max(pos2(area.min.x, topo), pos2(area.max.x, area.min.y));
    let cr_topo = CornerRadius {
        nw: 4,
        ne: 4,
        sw: 0,
        se: 0,
    };
    p.rect_filled(caixa, cr_topo, estilo.moldura);
    // Fio de luz no tampo (fosco, sem brilho).
    p.line_segment(
        [
            pos2(caixa.min.x + 4.0, caixa.min.y + 1.0),
            pos2(caixa.max.x - 4.0, caixa.min.y + 1.0),
        ],
        Stroke::new(1.0, alfa(BRANCO, 28)),
    );
    // Feltro.
    let alt_feltro = (cab * 0.34).clamp(2.5, 6.0);
    let feltro = Rect::from_min_max(
        pos2(area.min.x + lateral, area.min.y - alt_feltro),
        pos2(area.max.x - lateral, area.min.y),
    );
    degrade_vertical(
        p,
        feltro,
        Color32::from_rgb(138, 28, 38),
        Color32::from_rgb(98, 18, 28),
    );
    // Cheias laterais.
    let esq = Rect::from_min_max(
        pos2(area.min.x, topo),
        pos2(area.min.x + lateral, area.max.y),
    );
    let dir = Rect::from_min_max(
        pos2(area.max.x - lateral, topo),
        pos2(area.max.x, area.max.y),
    );
    p.rect_filled(
        esq,
        CornerRadius {
            nw: 4,
            sw: 3,
            ne: 0,
            se: 0,
        },
        estilo.moldura,
    );
    p.rect_filled(
        dir,
        CornerRadius {
            nw: 0,
            sw: 0,
            ne: 4,
            se: 3,
        },
        estilo.moldura,
    );
}

/// Contorno de 2 px na cor da mão, com pulso lento, para a tecla-guia.
fn contorno_guia(p: &Painter, r: Rect, canto: CornerRadius, m: Marca, estilo: &EstiloTeclado) {
    let a = match estilo.pulso {
        Some(f) => 0.55 + 0.45 * f.clamp(0.0, 1.0),
        None => 1.0,
    };
    p.rect_stroke(
        r,
        canto,
        Stroke::new(2.0, alfa_f(m.mao, a)),
        StrokeKind::Inside,
    );
}

fn pintar_branca(
    p: &Painter,
    e: &EstadoEspelho,
    estilo: &EstiloTeclado,
    g: &GeometriaTecla,
    r: Rect,
    w: f32,
    m: Marca,
) {
    let raio = (0.18 * w).clamp(2.0, 5.0) as u8;
    let canto = CornerRadius {
        nw: 0,
        ne: 0,
        sw: raio,
        se: raio,
    };
    // Fresta de 1 px entre as teclas; a ligada afunda 2 px.
    let mut c = r.shrink2(vec2(0.5, 0.0));
    if m.ligada {
        c.min.y += AFUNDA_PX;
    }
    let lip = if m.ligada {
        (0.2 * w).max(2.0)
    } else {
        (0.28 * w).max(3.0)
    };
    let corpo = Rect::from_min_max(c.min, pos2(c.max.x, c.max.y - lip));

    // Marfim fosco: degradê de ~2% entre o topo e a base.
    p.rect_filled(c, canto, Color32::from_rgb(238, 234, 222));
    degrade_vertical(
        p,
        corpo,
        Color32::from_rgb(246, 242, 231),
        Color32::from_rgb(241, 237, 225),
    );
    p.line_segment(
        [pos2(c.min.x, corpo.max.y), pos2(c.max.x, corpo.max.y)],
        Stroke::new(1.0, alfa(PRETO, 26)),
    );
    p.rect_stroke(
        c,
        canto,
        Stroke::new(1.0, Color32::from_rgb(150, 146, 136)),
        StrokeKind::Inside,
    );

    if m.ligada {
        p.rect_filled(c, canto, alfa_f(m.mao, m.tom));
        p.rect_stroke(
            c,
            canto,
            Stroke::new(2.0, escurecer(m.mao, 0.5)),
            StrokeKind::Inside,
        );
    }
    if m.errada {
        p.rect_filled(c, canto, alfa_f(estilo.erro, 0.5));
        p.rect_stroke(c, canto, Stroke::new(2.0, estilo.erro), StrokeKind::Inside);
    }
    if m.guia {
        let dentro = if m.ligada { 2.0 } else { 0.0 };
        contorno_guia(p, c.shrink(dentro), canto, m, estilo);
    }

    let centro_x = r.center().x;
    if m.ligada {
        // Marca de forma: círculo perto da base (cheio na direita, vazado na esquerda).
        let centro = Pos2::new(centro_x, r.bottom() - 0.75 * w);
        let (fundo, traco) = if m.esquerda {
            (
                Color32::from_rgb(246, 242, 231),
                Stroke::new(3.0, escurecer(m.mao, 0.55)),
            )
        } else {
            (m.mao, Stroke::new(2.0, escurecer(m.mao, 0.4)))
        };
        p.circle(centro, 0.34 * w, fundo, traco);
        if let Some(d) = m.dedo {
            pintar_dedo(p, centro, w, d, escurecer(m.mao, 0.2));
        }
    }
    if m.errada {
        pintar_x(
            p,
            Pos2::new(centro_x, r.bottom() - 1.1 * w),
            0.28 * w,
            estilo.erro,
        );
        pintar_nome(
            p,
            e,
            g,
            Pos2::new(centro_x, r.bottom() - 1.6 * w),
            estilo.erro,
            true,
        );
    }
    if m.guia {
        pintar_ponto_guia(p, centro_ponto_guia_branca(r, w), w, m.mao, m.dedo, false);
    }
    if w < LARGURA_MIN_ROTULO_PX {
        return;
    }
    // Rótulos: nome só na guia (padrão); Dós e demais só se a opção pedir.
    let mut y = r.bottom() - 0.28 * w - 4.0;
    let eh_do = g.nota.numero().is_multiple_of(12);
    let texto = match estilo.rotulos {
        Rotulos::Nenhum => None,
        _ if m.guia && !m.ligada => Some(texto_tela(nome_sem_oitava(g.nota, e.nomes))),
        Rotulos::SoDo if eh_do => Some(rotulo_tela(g.nota, e.nomes)),
        Rotulos::Todas if eh_do => Some(rotulo_tela(g.nota, e.nomes)),
        Rotulos::Todas => Some(texto_tela(nome_sem_oitava(g.nota, e.nomes))),
        _ => None,
    };
    if m.ligada || m.errada {
        // O nome da tecla ligada já está no painel; aqui só a marca de forma.
    } else if let Some(t) = texto {
        p.text(
            Pos2::new(centro_x, y),
            Align2::CENTER_BOTTOM,
            t,
            FontId::proportional(TEXTO_PX),
            estilo.tinta_suave,
        );
        y -= 13.0;
    }
    if estilo.letras_pc
        && estilo.rotulos != Rotulos::Nenhum
        && !m.ligada
        && !m.guia
        && let Some(l) = e.letra_pc_da_nota(g.nota)
    {
        p.text(
            Pos2::new(centro_x, y - 2.0),
            Align2::CENTER_BOTTOM,
            l,
            FontId::monospace(TEXTO_PX),
            Color32::from_rgb(128, 125, 118),
        );
    }
}

fn pintar_preta(
    p: &Painter,
    e: &EstadoEspelho,
    estilo: &EstiloTeclado,
    g: &GeometriaTecla,
    r: Rect,
    w: f32,
    m: Marca,
) {
    let raio = (0.14 * w).clamp(1.5, 4.0) as u8;
    let canto = CornerRadius {
        nw: 0,
        ne: 0,
        sw: raio,
        se: raio,
    };
    let mut c = r;
    if m.ligada {
        c.min.y += AFUNDA_PX;
    }
    // Grafite fosco com filete mais claro no alto e nas quinas (borda chanfrada).
    p.rect_filled(c, canto, Color32::from_rgb(24, 24, 29));
    let chanfro = (0.09 * w).clamp(1.5, 3.0);
    let face = Rect::from_min_max(
        pos2(c.min.x + chanfro, c.min.y),
        pos2(c.max.x - chanfro, c.max.y - chanfro),
    );
    degrade_vertical(
        p,
        face,
        Color32::from_rgb(48, 48, 55),
        Color32::from_rgb(36, 36, 42),
    );
    let filete = Color32::from_rgb(92, 92, 104);
    p.line_segment(
        [
            pos2(c.min.x + 1.0, c.min.y + 0.75),
            pos2(c.max.x - 1.0, c.min.y + 0.75),
        ],
        Stroke::new(1.5, filete),
    );
    p.line_segment(
        [
            pos2(c.min.x + 0.75, c.min.y + 1.0),
            pos2(c.min.x + 0.75, c.max.y - 2.0),
        ],
        Stroke::new(1.0, alfa(filete, 110)),
    );
    p.rect_stroke(c, canto, Stroke::new(1.0, PRETO), StrokeKind::Inside);

    if m.ligada {
        p.rect_filled(c, canto, alfa_f(m.mao, m.tom));
        p.rect_stroke(
            c,
            canto,
            Stroke::new(2.0, mistura(m.mao, BRANCO, 0.25)),
            StrokeKind::Inside,
        );
    }
    if m.errada {
        p.rect_filled(c, canto, alfa_f(estilo.erro, 0.55));
        p.rect_stroke(c, canto, Stroke::new(2.0, estilo.erro), StrokeKind::Inside);
    }
    if m.guia {
        let dentro = if m.ligada { 2.0 } else { 0.0 };
        contorno_guia(p, c.shrink(dentro), canto, m, estilo);
    }

    let centro_x = r.center().x;
    if m.ligada {
        // Marca de forma própria: losango (cheio na direita, vazado na esquerda).
        let centro = Pos2::new(centro_x, r.bottom() - 0.55 * w);
        let d = 0.3 * w;
        let pontos = vec![
            centro + vec2(0.0, -d),
            centro + vec2(d, 0.0),
            centro + vec2(0.0, d),
            centro + vec2(-d, 0.0),
        ];
        let (fundo, traco) = if m.esquerda {
            (Color32::from_rgb(36, 36, 42), Stroke::new(2.5, m.mao))
        } else {
            (m.mao, Stroke::new(1.0, BRANCO))
        };
        p.add(Shape::convex_polygon(pontos, fundo, traco));
        if let Some(d) = m.dedo {
            let tinta = if m.esquerda {
                BRANCO
            } else {
                escurecer(m.mao, 0.2)
            };
            pintar_dedo(p, centro, 0.85 * w, d, tinta);
        }
    }
    if m.errada {
        pintar_x(
            p,
            Pos2::new(centro_x, r.bottom() - 0.55 * w),
            0.22 * w,
            estilo.erro,
        );
        pintar_nome(
            p,
            e,
            g,
            Pos2::new(centro_x, r.bottom() - 1.0 * w),
            BRANCO,
            false,
        );
    }
    if m.guia {
        pintar_ponto_guia(
            p,
            Pos2::new(centro_x, r.bottom() - 1.3 * w),
            w,
            m.mao,
            m.dedo,
            true,
        );
    }
    if w < LARGURA_MIN_ROTULO_PX || estilo.rotulos == Rotulos::Nenhum {
        return;
    }
    if m.guia && !m.ligada {
        p.text(
            Pos2::new(centro_x, r.bottom() - 1.3 * w - 0.4 * w),
            Align2::CENTER_BOTTOM,
            texto_tela(nome_sem_oitava(g.nota, e.nomes)),
            FontId::proportional(TEXTO_PX),
            Color32::from_rgb(214, 210, 200),
        );
    } else if estilo.letras_pc
        && !m.ligada
        && !m.guia
        && let Some(l) = e.letra_pc_da_nota(g.nota)
    {
        p.text(
            Pos2::new(centro_x, r.bottom() - 4.0),
            Align2::CENTER_BOTTOM,
            l,
            FontId::monospace(TEXTO_PX),
            Color32::from_rgb(190, 190, 205),
        );
    }
}

/// Centro do ponto de dedilhado/guia numa tecla **branca** de retângulo `r`: na parte visível
/// da branca, abaixo das pretas (que descem até `ALTURA_PRETA`), no quarto inferior da tecla e
/// acima do rótulo da nota. O ponto da preta fica onde estava (dentro da própria preta).
pub fn centro_ponto_guia_branca(r: Rect, largura_branca: f32) -> Pos2 {
    Pos2::new(r.center().x, r.bottom() - 1.45 * largura_branca)
}

/// Ponto da tecla-guia: miolo na cor da mão com anel escuro (forma própria, não só cor). Com
/// dedilhado, o miolo dá lugar ao dedo (número ou letra) e a bolinha cresce um pouco.
fn pintar_ponto_guia(p: &Painter, c: Pos2, w: f32, mao: Color32, dedo: Option<&str>, preta: bool) {
    let anel = escurecer(mao, 0.35);
    match dedo {
        Some(d) => {
            // Na preta a bolinha precisa caber na largura da tecla (~0,6 da branca).
            let fator = if preta { 0.72 } else { 1.0 };
            p.circle(c, 0.38 * w * fator, mao, Stroke::new(2.0, anel));
            pintar_dedo(p, c, w * fator, d, escurecer(mao, 0.2));
        }
        None => {
            p.circle(c, 0.26 * w, mao, Stroke::new(2.0, anel));
            p.circle_filled(c, 0.09 * w, anel);
        }
    }
}

/// Número ou letra do dedo, centrado na bolinha; o tamanho acompanha a largura da tecla.
fn pintar_dedo(p: &Painter, c: Pos2, w: f32, dedo: &str, cor: Color32) {
    let px = (0.52 * w).clamp(9.0, 20.0);
    p.text(
        c,
        Align2::CENTER_CENTER,
        dedo,
        FontId::proportional(px),
        cor,
    );
}

/// "X" desenhado (a fonte embutida não tem o glifo): marca de nota errada.
fn pintar_x(p: &Painter, c: Pos2, meio: f32, cor: Color32) {
    let fundo = Stroke::new(5.0, alfa(BRANCO, 230));
    let traco = Stroke::new(2.5, cor);
    for st in [fundo, traco] {
        p.line_segment([c + vec2(-meio, -meio), c + vec2(meio, meio)], st);
        p.line_segment([c + vec2(-meio, meio), c + vec2(meio, -meio)], st);
    }
}

fn pintar_nome(
    p: &Painter,
    e: &EstadoEspelho,
    g: &GeometriaTecla,
    pos: Pos2,
    cor: Color32,
    sobre_claro: bool,
) {
    let cor = if sobre_claro {
        escurecer(cor, 0.65)
    } else {
        cor
    };
    p.text(
        pos,
        Align2::CENTER_BOTTOM,
        texto_tela(nome_sem_oitava(g.nota, e.nomes)),
        FontId::proportional(TEXTO_PX),
        cor,
    );
}

fn pintar_do_central(
    p: &Painter,
    area: Rect,
    w: f32,
    e: &EstadoEspelho,
    geo: &[GeometriaTecla],
    estilo: &EstiloTeclado,
) {
    let Some(i) = e.teclado().indice_do_central() else {
        return;
    };
    let Some(g) = geo.get(i) else {
        return;
    };
    let r = retangulo_da_tecla(area, w, g);
    let topo = Pos2::new(r.center().x, r.bottom() + 4.0);
    p.add(Shape::convex_polygon(
        vec![topo, topo + vec2(-7.0, 12.0), topo + vec2(7.0, 12.0)],
        AMBAR,
        Stroke::new(1.5, estilo.tinta),
    ));
    if estilo.rotulos == Rotulos::Nenhum {
        return;
    }
    let texto = match e.nomes.idioma {
        Idioma::PtBr => format!("Dó central ({})", rotulo_tela(g.nota, e.nomes)),
        Idioma::Internacional => format!("Middle C ({})", rotulo_tela(g.nota, e.nomes)),
    };
    p.text(
        Pos2::new(r.center().x, r.bottom() + 17.0),
        Align2::CENTER_TOP,
        texto,
        FontId::proportional(12.0),
        estilo.tinta,
    );
}

#[cfg(test)]
mod testes {
    //! Desenho "sem janela" do componente, com vários zooms e temas.
    use super::*;
    use bancada_casca::egui::{Context, RawInput};

    fn n(x: u8) -> NotaMidi {
        NotaMidi::nova(x).expect("nota válida")
    }

    /// Desenha o teclado num contexto e devolve quantas formas saíram.
    fn desenhar(e: &EstadoEspelho, w: f32, estilo: &EstiloTeclado) -> usize {
        let ctx = Context::default();
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(2400.0, 900.0))),
            ..Default::default()
        };
        let saida = ctx.run_ui(entrada, |ui| {
            let reservado = Rect::from_min_size(pos2(10.0, 10.0), tamanho_do_teclado(e, w));
            let area = area_das_teclas(reservado, w);
            pintar_teclado(ui.painter(), area, w, e, estilo);
        });
        let total = saida.shapes.len();
        saida.drop_without_applying_deltas();
        total
    }

    fn tres_temas() -> Vec<EstiloTeclado> {
        let mut ebano = Visuals::dark();
        ebano.panel_fill = Color32::from_rgb(11, 10, 9);
        vec![
            EstiloTeclado::para_visuals(&Visuals::dark()),
            EstiloTeclado::para_visuals(&Visuals::light()),
            EstiloTeclado::para_visuals(&ebano),
        ]
    }

    #[test]
    fn temas_escolhem_as_paletas_da_identidade() {
        let t = tres_temas();
        assert_eq!(t[0].mao_direita, Color32::from_rgb(0xE2, 0xA2, 0x4B));
        assert_eq!(t[1].mao_direita, Color32::from_rgb(0xB5, 0x65, 0x1D));
        assert_eq!(t[2].mao_esquerda, Color32::from_rgb(0x6F, 0xB6, 0xD4));
    }

    #[test]
    fn desenha_em_varios_zooms_e_temas_sem_panico() {
        let mut e = EstadoEspelho::novo();
        e.pressionar_simulado(n(60));
        e.pressionar_simulado(n(61));
        for estilo in tres_temas() {
            for z in [ZOOM_MIN, 0.8, 1.0, 1.7, ZOOM_MAX] {
                for rotulos in [
                    Rotulos::Nenhum,
                    Rotulos::SoGuias,
                    Rotulos::SoDo,
                    Rotulos::Todas,
                ] {
                    let est = EstiloTeclado {
                        rotulos,
                        ..estilo.clone()
                    };
                    assert!(desenhar(&e, LARGURA_BRANCA_BASE_PX * z, &est) > 100);
                }
            }
        }
    }

    #[test]
    fn guia_ligada_e_errada_juntas_nao_quebram() {
        let mut e = EstadoEspelho::novo();
        e.guias = vec![n(60), n(61), n(64)];
        e.pressionar_simulado(n(60));
        e.pressionar_simulado(n(61));
        e.pressionar_simulado(n(72));
        let est = EstiloTeclado {
            erradas: vec![n(62), n(61), n(66)],
            divisao: Some(60),
            pulso: Some(0.4),
            ..EstiloTeclado::default()
        };
        for z in [ZOOM_MIN, 1.0, ZOOM_MAX] {
            assert!(desenhar(&e, LARGURA_BRANCA_BASE_PX * z, &est) > 100);
        }
    }

    #[test]
    fn tecla_ligada_guia_e_errada_acrescentam_formas() {
        let w = LARGURA_BRANCA_BASE_PX;
        let estilo = EstiloTeclado::default();
        let mut e = EstadoEspelho::novo();
        let sem = desenhar(&e, w, &estilo);
        e.pressionar_simulado(n(62));
        let ligada = desenhar(&e, w, &estilo);
        assert!(ligada > sem, "{sem} -> {ligada}");
        e.guias = vec![n(65)];
        let guia = desenhar(&e, w, &estilo);
        assert!(guia > ligada);
        let errada = EstiloTeclado {
            erradas: vec![n(67)],
            ..estilo
        };
        assert!(desenhar(&e, w, &errada) > guia);
    }

    #[test]
    fn mao_esquerda_so_com_ponto_de_divisao() {
        let sem = EstiloTeclado::default();
        assert!(!sem.eh_mao_esquerda(n(36)));
        assert_eq!(sem.cor_da_mao(n(36)), sem.mao_direita);
        let com = EstiloTeclado {
            divisao: Some(60),
            ..sem
        };
        assert!(com.eh_mao_esquerda(n(59)) && !com.eh_mao_esquerda(n(60)));
        assert_eq!(com.cor_da_mao(n(59)), com.mao_esquerda);
        assert_eq!(com.cor_da_mao(n(60)), com.mao_direita);
    }

    #[test]
    fn cada_tecla_e_achada_no_proprio_centro() {
        let e = EstadoEspelho::novo();
        let geo = geometria(e.teclado());
        for z in [ZOOM_MIN, 1.0, ZOOM_MAX] {
            let w = LARGURA_BRANCA_BASE_PX * z;
            let reservado = Rect::from_min_size(pos2(7.0, 3.0), tamanho_do_teclado(&e, w));
            let area = area_das_teclas(reservado, w);
            for g in &geo {
                let r = retangulo_da_tecla(area, w, g);
                let ponto = match g.cor {
                    CorTecla::Branca => pos2(r.center().x, r.bottom() - 0.4 * w),
                    CorTecla::Preta => r.center(),
                };
                assert_eq!(tecla_no_ponto(&geo, area, w, ponto), Some(g.indice));
            }
        }
    }

    #[test]
    fn ponto_de_guia_da_branca_fica_abaixo_das_pretas_e_dentro_da_tecla() {
        use crate::espelho::ALTURA_PRETA;
        let e = EstadoEspelho::novo();
        let geo = geometria(e.teclado());
        for z in [ZOOM_MIN, 1.0, ZOOM_MAX] {
            let w = LARGURA_BRANCA_BASE_PX * z;
            let area = area_das_teclas(
                Rect::from_min_size(pos2(7.0, 3.0), tamanho_do_teclado(&e, w)),
                w,
            );
            for g in geo.iter().filter(|g| g.cor == CorTecla::Branca) {
                let r = retangulo_da_tecla(area, w, g);
                let c = centro_ponto_guia_branca(r, w);
                let raio = 0.26 * w + 1.0;
                let fim_da_preta = r.top() + ALTURA_PRETA * w;
                assert!(
                    c.y - raio > fim_da_preta,
                    "ponto invade a região das pretas"
                );
                assert!(c.y + raio < r.bottom(), "ponto sai da tecla");
                assert!(
                    (r.bottom() - c.y) / r.height() < 0.35,
                    "ponto fora do terço inferior"
                );
                assert_eq!(c.x, r.center().x);
            }
        }
    }

    #[test]
    fn ajuste_a_largura_preenche_e_respeita_os_limites_de_zoom() {
        let e = EstadoEspelho::novo();
        let total = largura_total(e.teclado());
        let w = largura_branca_ajustada(&e, 1000.0);
        assert!((w * total - 1000.0).abs() < 0.5, "{w}");
        assert_eq!(
            largura_branca_ajustada(&e, 10.0),
            LARGURA_BRANCA_BASE_PX * ZOOM_MIN
        );
        assert_eq!(
            largura_branca_ajustada(&e, 100_000.0),
            LARGURA_BRANCA_BASE_PX * ZOOM_MAX
        );
        assert_eq!(
            largura_branca_ajustada(&e, f32::NAN),
            LARGURA_BRANCA_BASE_PX
        );
    }

    #[test]
    fn velocidade_escala_o_tom_e_o_pulso_fica_entre_0_e_1() {
        assert!(opacidade_do_tom(10) < opacidade_do_tom(127));
        assert!((opacidade_do_tom(VELOCIDADE_PADRAO) - 0.4).abs() < 0.05);
        let est = EstiloTeclado::default().com_velocidade(n(60), 30);
        assert_eq!(est.velocidade_de(n(60)), 30);
        assert_eq!(est.velocidade_de(n(61)), VELOCIDADE_PADRAO);
        let est = est.com_velocidade(n(60), 200);
        assert_eq!(est.velocidade_de(n(60)), 127);
        for i in 0..50 {
            let f = fase_do_pulso(f64::from(i) * 0.07);
            assert!((0.0..=1.0).contains(&f), "{f}");
        }
        assert!(fase_do_pulso(0.0) < 0.01 && fase_do_pulso(0.6) > 0.99);
        let mut e = EstadoEspelho::novo();
        e.pressionar_simulado(n(60));
        assert!(desenhar(&e, 24.0, &est) > 100);
    }
}
