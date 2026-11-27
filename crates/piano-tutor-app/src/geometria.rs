//! Camada geométrica própria da partitura (ticket 06): pontos, vetores, retângulos, âncoras de
//! texto e a identidade de nota, **sem nenhum tipo do egui**. O layout (`partitura_visual`)
//! calcula tudo em espaços de pauta (sp) com estes tipos, o que permite testar posições e
//! hit test sem criar contexto gráfico. A conversão para o egui acontece num único adaptador
//! (`partitura_visual::adaptador_egui`).

use std::ops::{Add, AddAssign, Mul, Neg, Sub};

/// Identidade estável de uma nota: o mesmo tipo do documento do núcleo (ticket 10). Quem a
/// atribui é o serviço de documento; o layout a carrega nas primitivas da nota quando recebe as
/// identidades (`partitura_visual::calcular_layout_com_ids`). Sem elas, nenhuma primitiva a
/// carrega (`None`) e valem os comportamentos por compasso.
pub use piano_tutor_core::documento::IdNota;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ponto {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vetor {
    pub x: f32,
    pub y: f32,
}

pub const fn ponto(x: f32, y: f32) -> Ponto {
    Ponto { x, y }
}

pub const fn vetor(x: f32, y: f32) -> Vetor {
    Vetor { x, y }
}

impl Ponto {
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Vetor {
    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    pub fn normalized(self) -> Vetor {
        let l = self.length();
        if l <= 0.0 {
            self
        } else {
            vetor(self.x / l, self.y / l)
        }
    }
}

impl Add<Vetor> for Ponto {
    type Output = Ponto;
    fn add(self, v: Vetor) -> Ponto {
        ponto(self.x + v.x, self.y + v.y)
    }
}

impl AddAssign<Vetor> for Ponto {
    fn add_assign(&mut self, v: Vetor) {
        self.x += v.x;
        self.y += v.y;
    }
}

impl Sub<Vetor> for Ponto {
    type Output = Ponto;
    fn sub(self, v: Vetor) -> Ponto {
        ponto(self.x - v.x, self.y - v.y)
    }
}

impl Sub for Ponto {
    type Output = Vetor;
    fn sub(self, o: Ponto) -> Vetor {
        vetor(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for Vetor {
    type Output = Vetor;
    fn mul(self, k: f32) -> Vetor {
        vetor(self.x * k, self.y * k)
    }
}

impl Neg for Vetor {
    type Output = Vetor;
    fn neg(self) -> Vetor {
        vetor(-self.x, -self.y)
    }
}

/// Retângulo alinhado aos eixos (`min` e `max` em sp).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Retangulo {
    pub min: Ponto,
    pub max: Ponto,
}

impl Retangulo {
    /// Retângulo vazio: estendê-lo com um ponto o reduz a esse ponto.
    pub const NOTHING: Retangulo = Retangulo {
        min: ponto(f32::INFINITY, f32::INFINITY),
        max: ponto(f32::NEG_INFINITY, f32::NEG_INFINITY),
    };

    pub fn from_min_max(min: Ponto, max: Ponto) -> Self {
        Self { min, max }
    }

    pub fn from_two_pos(a: Ponto, b: Ponto) -> Self {
        Self {
            min: ponto(a.x.min(b.x), a.y.min(b.y)),
            max: ponto(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    pub fn from_center_size(centro: Ponto, tam: Vetor) -> Self {
        let m = tam * 0.5;
        Self {
            min: centro - m,
            max: centro + m,
        }
    }

    pub fn extend_with(&mut self, p: Ponto) {
        self.min = ponto(self.min.x.min(p.x), self.min.y.min(p.y));
        self.max = ponto(self.max.x.max(p.x), self.max.y.max(p.y));
    }

    pub fn contains(&self, p: Ponto) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    /// Cópia aumentada em `m` de cada lado.
    pub fn expand(&self, m: f32) -> Self {
        Self {
            min: ponto(self.min.x - m, self.min.y - m),
            max: ponto(self.max.x + m, self.max.y + m),
        }
    }

    pub fn intersects(&self, o: Retangulo) -> bool {
        self.min.x <= o.max.x
            && o.min.x <= self.max.x
            && self.min.y <= o.max.y
            && o.min.y <= self.max.y
    }
}

/// Alinhamento em um eixo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alinha {
    Min,
    Centro,
    Max,
}

/// Âncora de um texto em relação ao seu ponto (horizontal, vertical).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ancora(pub [Alinha; 2]);

impl Ancora {
    pub const CENTER_CENTER: Ancora = Ancora([Alinha::Centro, Alinha::Centro]);
    pub const LEFT_BOTTOM: Ancora = Ancora([Alinha::Min, Alinha::Max]);
    pub const LEFT_TOP: Ancora = Ancora([Alinha::Min, Alinha::Min]);
    pub const CENTER_TOP: Ancora = Ancora([Alinha::Centro, Alinha::Min]);
    pub const CENTER_BOTTOM: Ancora = Ancora([Alinha::Centro, Alinha::Max]);
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn retangulo_estende_e_cruza() {
        let mut r = Retangulo::NOTHING;
        r.extend_with(ponto(1.0, 2.0));
        r.extend_with(ponto(3.0, -1.0));
        assert_eq!(
            r,
            Retangulo::from_min_max(ponto(1.0, -1.0), ponto(3.0, 2.0))
        );
        assert!(r.contains(ponto(2.0, 0.0)));
        assert!(!r.contains(ponto(4.0, 0.0)));
        assert!(r.intersects(Retangulo::from_center_size(
            ponto(3.5, 0.0),
            vetor(2.0, 2.0)
        )));
        assert!(!r.intersects(Retangulo::from_center_size(
            ponto(6.0, 0.0),
            vetor(2.0, 2.0)
        )));
    }

    #[test]
    fn vetor_normaliza() {
        let n = (ponto(3.0, 4.0) - ponto(0.0, 0.0)).normalized();
        assert!((n.length() - 1.0).abs() < 1e-6);
        assert_eq!(vetor(0.0, 0.0).normalized(), vetor(0.0, 0.0));
    }
}
