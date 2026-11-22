//! Núcleo mínimo do Piano Tutor: independente de hardware e UI.
//! Este módulo inicial NÃO implementa MIDI em tempo real nem áudio.

/// Nota MIDI no intervalo de 0 a 127; convenção Dó4 = MIDI 60.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NotaMidi(u8);

impl NotaMidi {
    pub const fn nova(numero: u8) -> Option<Self> {
        if numero <= 127 { Some(Self(numero)) } else { None }
    }
    pub const fn numero(self) -> u8 { self.0 }
    pub fn frequencia_hz(self, la4_hz: f64) -> Option<f64> {
        if !la4_hz.is_finite() || la4_hz <= 0.0 { return None; }
        Some(la4_hz * 2.0_f64.powf((f64::from(self.0) - 69.0) / 12.0))
    }
    pub const fn nome_pt(self) -> &'static str {
        const N: [&str; 12] = ["Dó","Dó♯","Ré","Ré♯","Mi","Fá","Fá♯","Sol","Sol♯","Lá","Lá♯","Si"];
        N[(self.0 % 12) as usize]
    }
    pub const fn oitava(self) -> i8 { (self.0 / 12) as i8 - 1 }
}

#[cfg(test)]
mod testes {
    use super::NotaMidi;
    #[test]
    fn do_central_midi_60() {
        let n = NotaMidi::nova(60).expect("Dó4");
        assert_eq!(n.nome_pt(), "Dó");
        assert_eq!(n.oitava(), 4);
        assert!((n.frequencia_hz(440.0).expect("afinacao") - 261.625565).abs() < 0.0001);
    }
    #[test]
    fn la4_igual_440_hz() {
        assert!((NotaMidi::nova(69).expect("Lá4").frequencia_hz(440.0).expect("afinacao") - 440.0).abs() < 1e-10);
    }
    #[test]
    fn afinacao_invalida_nao_e_aceita() {
        let n = NotaMidi::nova(69).expect("nota");
        assert_eq!(n.frequencia_hz(0.0), None);
        assert_eq!(n.frequencia_hz(f64::NAN), None);
    }
    #[test]
    fn limite_de_nota() {
        assert!(NotaMidi::nova(127).is_some());
        assert!(NotaMidi::nova(128).is_none());
    }
}
