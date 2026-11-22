//! Núcleo mínimo para provar a toolchain: **não** implementa conexão MIDI/áudio.
//! Este módulo inicial só realiza cálculos determinísticos de nota em temperamento igual.
//! O módulo [`midi`] adiciona o evento MIDI normalizado e o parser (código puro).
//! O módulo [`sanitizar`] remove seriais e VID:PID de textos destinados a relatórios e logs.
//! O módulo [`tempo`] define a base de tempo monotônico única, compartilhada por MIDI e áudio.

pub mod midi;

pub mod sanitizar;

pub mod teclado;

pub mod tempo;

pub mod partitura;

pub mod ritmo;

pub mod teoria;

/// Identificador de altura MIDI (0 a 127). O nome de oitava segue C4 = 60.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NotaMidi(u8);

impl NotaMidi {
    /// Números MIDI válidos ocupam 7 bits.
    pub const fn nova(numero: u8) -> Option<Self> {
        if numero <= 127 {
            Some(Self(numero))
        } else {
            None
        }
    }

    pub const fn numero(self) -> u8 {
        self.0
    }

    /// Frequência fundamental no temperamento igual, com afinação de Lá4 ajustável.
    pub fn frequencia_hz(self, la4_hz: f64) -> Option<f64> {
        if !la4_hz.is_finite() || la4_hz <= 0.0 {
            return None;
        }
        Some(la4_hz * 2.0_f64.powf((f64::from(self.0) - 69.0) / 12.0))
    }

    /// Nome em português (sem a oitava).
    pub const fn nome_pt(self) -> &'static str {
        const NOMES: [&str; 12] = [
            "Dó", "Dó♯", "Ré", "Ré♯", "Mi", "Fá", "Fá♯", "Sol", "Sol♯", "Lá", "Lá♯", "Si",
        ];
        NOMES[(self.0 % 12) as usize]
    }

    /// Oitava científica: 60 = C4. A nota 0 está na oitava -1.
    pub const fn oitava(self) -> i8 {
        (self.0 / 12) as i8 - 1
    }
}

#[cfg(test)]
mod testes {
    use super::NotaMidi;

    #[test]
    fn do_central_e_midi_60() {
        let nota = NotaMidi::nova(60).expect("MIDI 60 válido");
        assert_eq!(nota.nome_pt(), "Dó");
        assert_eq!(nota.oitava(), 4);
        let hz = nota.frequencia_hz(440.0).expect("afinação válida");
        assert!((hz - 261.625565).abs() < 0.0001);
    }

    #[test]
    fn la4_igual_440_hz() {
        let hz = NotaMidi::nova(69)
            .expect("MIDI válido")
            .frequencia_hz(440.0)
            .expect("afinação válida");
        assert!((hz - 440.0).abs() < 1e-10);
    }

    #[test]
    fn rejeita_afinacao_invalida() {
        let nota = NotaMidi::nova(69).expect("nota válida");
        assert_eq!(nota.frequencia_hz(0.0), None);
        assert_eq!(nota.frequencia_hz(f64::NAN), None);
    }

    #[test]
    fn cobre_limites_midi() {
        assert_eq!(
            NotaMidi::nova(127)
                .expect("limite superior válido")
                .numero(),
            127
        );
        assert_eq!(NotaMidi::nova(128), None);
    }
}
