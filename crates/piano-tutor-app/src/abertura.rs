//! Tela de abertura (splash): aparece por cima da janela ao abrir o app, mostra o nome, a versão
//! e o que está carregando (SoundFont, MIDI) e some sozinha com um esmaecimento. Clique ou
//! qualquer tecla fecha na hora. Desenhada só com o pintor do egui, sem imagem externa.

use crate::icones;
use bancada_casca::egui::{
    self, Align2, Color32, CornerRadius, FontFamily, FontId, Id, Mesh, Pos2, Rect, Stroke,
    StrokeKind, pos2, vec2,
};

/// Tempo mínimo na tela, em segundos.
pub const DURACAO_MINIMA: f64 = 2.6;
/// Tempo máximo, mesmo se algo ainda carrega.
pub const DURACAO_MAXIMA: f64 = 4.5;
/// Esmaecimento final.
pub const ESMAECER: f64 = 0.45;

/// Estado da abertura.
#[derive(Debug, Clone, PartialEq)]
pub struct Abertura {
    inicio: Option<f64>,
    fim: Option<f64>,
    ligada: bool,
}

impl Abertura {
    /// Abertura que aparece no primeiro quadro.
    pub fn ligada() -> Self {
        Self {
            inicio: None,
            fim: None,
            ligada: true,
        }
    }

    /// Sem abertura (testes e janelas sem tela).
    pub fn desligada() -> Self {
        Self {
            inicio: None,
            fim: None,
            ligada: false,
        }
    }

    pub fn visivel(&self) -> bool {
        self.ligada
    }

    /// Opacidade no instante `agora` (s). `carregando` segura a abertura até o máximo.
    /// Devolve `None` quando ela acabou (e desliga).
    pub fn opacidade(&mut self, agora: f64, carregando: bool) -> Option<f32> {
        if !self.ligada {
            return None;
        }
        let inicio = *self.inicio.get_or_insert(agora);
        let passou = agora - inicio;
        if self.fim.is_none()
            && passou >= DURACAO_MINIMA
            && (!carregando || passou >= DURACAO_MAXIMA)
        {
            self.fim = Some(agora);
        }
        match self.fim {
            Some(f) if agora - f >= ESMAECER => {
                self.ligada = false;
                None
            }
            Some(f) => Some((1.0 - (agora - f) / ESMAECER) as f32),
            None => Some(1.0),
        }
    }

    /// Fecha já (começa o esmaecimento).
    pub fn dispensar(&mut self, agora: f64) {
        if self.ligada && self.fim.is_none() {
            self.fim = Some(agora);
        }
    }

    /// Progresso da barra (0–1) em função do tempo, para dar ritmo à espera.
    pub fn progresso(&self, agora: f64) -> f32 {
        let inicio = self.inicio.unwrap_or(agora);
        (((agora - inicio) / DURACAO_MINIMA).clamp(0.0, 1.0)) as f32
    }
}

fn com_alfa(c: Color32, a: f32) -> Color32 {
    c.gamma_multiply(a.clamp(0.0, 1.0))
}

/// Desenha a abertura por cima de tudo. `status` é a linha do que está carregando.
pub fn desenhar(ctx: &egui::Context, abertura: &mut Abertura, status: &str, carregando: bool) {
    let agora = ctx.input(|i| i.time);
    let clicou = ctx.input(|i| {
        i.pointer.any_click()
            || i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Key { .. }))
    });
    if clicou {
        abertura.dispensar(agora);
    }
    let Some(alfa) = abertura.opacidade(agora, carregando) else {
        return;
    };
    ctx.request_repaint();
    let tela = ctx.content_rect();
    // Pintor direto numa camada de cima: uma Area nova fica invisível no primeiro quadro.
    let p = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        Id::new("piano.abertura"),
    ));
    pintar(&p, tela, alfa, abertura.progresso(agora), status, agora);
}

fn degrade(p: &egui::Painter, r: Rect, cima: Color32, baixo: Color32) {
    let mut m = Mesh::default();
    m.colored_vertex(r.left_top(), cima);
    m.colored_vertex(r.right_top(), cima);
    m.colored_vertex(r.left_bottom(), baixo);
    m.colored_vertex(r.right_bottom(), baixo);
    m.add_triangle(0, 1, 2);
    m.add_triangle(1, 2, 3);
    p.add(egui::Shape::mesh(m));
}

fn pintar(p: &egui::Painter, tela: Rect, a: f32, progresso: f32, status: &str, t: f64) {
    let azul = Color32::from_rgb(71, 158, 245);
    let marfim = Color32::from_rgb(244, 246, 250);
    let suave = Color32::from_rgb(150, 162, 180);
    degrade(
        p,
        tela,
        com_alfa(Color32::from_rgb(16, 22, 34), a),
        com_alfa(Color32::from_rgb(6, 8, 13), a),
    );

    // Faixa de teclado estilizada no rodapé: 3 oitavas, teclas que acendem em onda.
    let altura_teclado = (tela.height() * 0.22).clamp(90.0, 170.0);
    let base = Rect::from_min_max(
        pos2(tela.left(), tela.bottom() - altura_teclado),
        tela.right_bottom(),
    );
    let brancas = 36;
    let lb = base.width() / brancas as f32;
    for i in 0..brancas {
        let x = base.left() + i as f32 * lb;
        let onda = ((t * 2.2 - f64::from(i) * 0.18).sin() * 0.5 + 0.5) as f32;
        let r = Rect::from_min_size(pos2(x + 1.0, base.top()), vec2(lb - 2.0, base.height()));
        let cor = Color32::from_rgb(28, 34, 46)
            .lerp_to_gamma(Color32::from_rgb(40, 70, 110), onda * 0.35);
        p.rect_filled(r, CornerRadius::same(3), com_alfa(cor, a));
    }
    for i in 0..brancas {
        if matches!(i % 7, 2 | 6) {
            continue;
        }
        let x = base.left() + (i as f32 + 0.68) * lb;
        let r = Rect::from_min_size(pos2(x, base.top()), vec2(lb * 0.64, base.height() * 0.6));
        p.rect_filled(
            r,
            CornerRadius::same(2),
            com_alfa(Color32::from_rgb(8, 10, 15), a),
        );
    }
    // Brilho do acento acima do teclado.
    degrade(
        p,
        Rect::from_min_max(pos2(base.left(), base.top() - 60.0), base.right_top()),
        Color32::TRANSPARENT,
        com_alfa(azul, a * 0.18),
    );

    // Marca: quadrado arredondado com o ícone do piano.
    let centro = pos2(
        tela.center().x,
        tela.top() + (tela.height() - altura_teclado) * 0.42,
    );
    let lado = 104.0;
    let marca = Rect::from_center_size(centro - vec2(0.0, 40.0), vec2(lado, lado));
    p.rect_filled(marca, CornerRadius::same(24), com_alfa(azul, a));
    p.rect_stroke(
        marca.expand(6.0),
        CornerRadius::same(30),
        Stroke::new(1.0, com_alfa(azul, a * 0.35)),
        StrokeKind::Outside,
    );
    p.text(
        marca.center() + vec2(0.0, 2.0),
        Align2::CENTER_CENTER,
        icones::glifo("piano"),
        FontId::new(64.0, FontFamily::Proportional),
        com_alfa(Color32::WHITE, a),
    );

    let y = marca.bottom() + 34.0;
    p.text(
        pos2(centro.x, y),
        Align2::CENTER_TOP,
        "Piano Tutor",
        FontId::proportional(44.0),
        com_alfa(marfim, a),
    );
    p.text(
        pos2(centro.x, y + 58.0),
        Align2::CENTER_TOP,
        "Professor assistido de piano e teclado",
        FontId::proportional(17.0),
        com_alfa(suave, a),
    );

    // Barra de progresso fina e o status.
    let barra = Rect::from_center_size(pos2(centro.x, y + 112.0), vec2(260.0, 3.0));
    p.rect_filled(
        barra,
        CornerRadius::same(2),
        com_alfa(Color32::from_rgb(40, 48, 62), a),
    );
    let cheia = Rect::from_min_size(barra.min, vec2(barra.width() * progresso, barra.height()));
    p.rect_filled(cheia, CornerRadius::same(2), com_alfa(azul, a));
    p.text(
        pos2(centro.x, barra.bottom() + 12.0),
        Align2::CENTER_TOP,
        status,
        FontId::proportional(13.0),
        com_alfa(suave, a),
    );
    p.text(
        Pos2::new(tela.right() - 16.0, tela.top() + 14.0),
        Align2::RIGHT_TOP,
        format!("versão {}", crate::estado::VERSAO),
        FontId::proportional(12.0),
        com_alfa(suave, a * 0.8),
    );
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn some_apos_o_minimo_e_esmaece() {
        let mut a = Abertura::ligada();
        assert_eq!(a.opacidade(10.0, false), Some(1.0));
        assert_eq!(a.opacidade(11.0, false), Some(1.0));
        assert_eq!(a.opacidade(10.01 + DURACAO_MINIMA, false), Some(1.0));
        let meio = a.opacidade(10.01 + DURACAO_MINIMA + ESMAECER / 2.0, false);
        assert!(meio.is_some_and(|x| x < 1.0 && x > 0.0));
        assert_eq!(
            a.opacidade(10.0 + DURACAO_MINIMA + ESMAECER * 2.0, false),
            None
        );
        assert!(!a.visivel());
    }

    #[test]
    fn segura_enquanto_carrega_ate_o_maximo() {
        let mut a = Abertura::ligada();
        let _ = a.opacidade(0.0, true);
        assert_eq!(a.opacidade(DURACAO_MINIMA + 1.0, true), Some(1.0));
        let _ = a.opacidade(DURACAO_MAXIMA, true);
        assert_eq!(a.opacidade(DURACAO_MAXIMA + ESMAECER + 0.01, true), None);
    }

    #[test]
    fn clique_dispensa_e_desligada_nao_aparece() {
        let mut a = Abertura::ligada();
        let _ = a.opacidade(0.0, false);
        a.dispensar(0.5);
        assert_eq!(a.opacidade(0.5 + ESMAECER + 0.01, false), None);
        assert_eq!(Abertura::desligada().opacidade(0.0, false), None);
    }
}
