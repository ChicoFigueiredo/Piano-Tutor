//! Capacidades **observadas** do dispositivo (nunca presumidas).
//!
//! "Não observado" significa que nada chegou até agora; a UI deve exibir
//! "não suportado pelo dispositivo" (ou "ainda não observado") sem inventar
//! valores. Não há padrão de velocity, pedal ou programa.

use piano_tutor_core::midi::{CanalMidi, EventoMidi};

/// Controlador do pedal de sustentação (MIDI CC64).
pub const CC_SUSTAIN: u8 = 64;

/// Resultado de uma observação.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Observacao {
    NaoObservado,
    Observado,
}

impl Observacao {
    const fn de(b: bool) -> Self {
        if b {
            Self::Observado
        } else {
            Self::NaoObservado
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Capacidades {
    velocidade_min: Option<u8>,
    velocidade_max: Option<u8>,
    ultimo_cc64: Option<u8>,
    ultimo_programa: Option<u8>,
    liberacoes: u64,
    /// Bit `n` ligado = canal interno `n` observado.
    canais: u16,
}

impl Capacidades {
    #[must_use]
    pub fn novas() -> Self {
        Self::default()
    }

    /// Atualiza as observações com um evento recebido.
    pub fn registrar(&mut self, evento: &EventoMidi) {
        let canal = match *evento {
            EventoMidi::NotaLigada {
                canal, velocidade, ..
            } => {
                self.velocidade_min = Some(
                    self.velocidade_min
                        .map_or(velocidade, |v| v.min(velocidade)),
                );
                self.velocidade_max = Some(
                    self.velocidade_max
                        .map_or(velocidade, |v| v.max(velocidade)),
                );
                Some(canal)
            }
            EventoMidi::NotaDesligada { canal, .. } => {
                self.liberacoes = self.liberacoes.saturating_add(1);
                Some(canal)
            }
            EventoMidi::ControleMudou {
                canal,
                controle,
                valor,
                ..
            } => {
                if controle == CC_SUSTAIN {
                    self.ultimo_cc64 = Some(valor);
                }
                Some(canal)
            }
            EventoMidi::ProgramaMudou {
                canal, programa, ..
            } => {
                self.ultimo_programa = Some(programa);
                Some(canal)
            }
            EventoMidi::PitchBend { canal, .. } => Some(canal),
            EventoMidi::Desconhecido { .. } => None,
        };
        if let Some(c) = canal {
            self.canais |= 1 << c.interno();
        }
    }

    /// Observado somente quando chegaram NoteOn com pelo menos duas
    /// velocidades distintas (velocity fixa = não observado).
    #[must_use]
    pub fn velocidade_variavel(&self) -> Observacao {
        Observacao::de(matches!(
            (self.velocidade_min, self.velocidade_max),
            (Some(a), Some(b)) if a != b
        ))
    }

    /// Menor e maior velocidade de NoteOn recebidas.
    #[must_use]
    pub fn faixa_velocidade(&self) -> Option<(u8, u8)> {
        self.velocidade_min.zip(self.velocidade_max)
    }

    #[must_use]
    pub fn pedal_sustain(&self) -> Observacao {
        Observacao::de(self.ultimo_cc64.is_some())
    }

    /// Último valor de CC64 recebido.
    #[must_use]
    pub fn ultimo_valor_sustain(&self) -> Option<u8> {
        self.ultimo_cc64
    }

    #[must_use]
    pub fn mudanca_de_programa(&self) -> Observacao {
        Observacao::de(self.ultimo_programa.is_some())
    }

    #[must_use]
    pub fn ultimo_programa(&self) -> Option<u8> {
        self.ultimo_programa
    }

    /// Liberação de nota (NoteOff ou NoteOn com velocity 0) recebida.
    #[must_use]
    pub fn liberacao_de_nota(&self) -> Observacao {
        Observacao::de(self.liberacoes > 0)
    }

    /// Canais em que chegou alguma mensagem de canal, em ordem crescente.
    pub fn canais_observados(&self) -> impl Iterator<Item = CanalMidi> + '_ {
        (0u8..16)
            .filter(|c| self.canais & (1 << c) != 0)
            .filter_map(CanalMidi::novo)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use piano_tutor_core::midi::interpretar;

    fn ev(bytes: &[u8]) -> EventoMidi {
        interpretar(bytes, 0).expect("mensagem válida")
    }

    #[test]
    fn nada_recebido_e_tudo_nao_observado() {
        let c = Capacidades::novas();
        assert_eq!(c.velocidade_variavel(), Observacao::NaoObservado);
        assert_eq!(c.pedal_sustain(), Observacao::NaoObservado);
        assert_eq!(c.mudanca_de_programa(), Observacao::NaoObservado);
        assert_eq!(c.liberacao_de_nota(), Observacao::NaoObservado);
        assert_eq!(c.faixa_velocidade(), None);
        assert_eq!(c.canais_observados().count(), 0);
    }

    #[test]
    fn velocity_fixa_nao_conta_como_variavel() {
        let mut c = Capacidades::novas();
        for n in 60..70 {
            c.registrar(&ev(&[0x90, n, 100]));
        }
        assert_eq!(c.velocidade_variavel(), Observacao::NaoObservado);
        assert_eq!(c.faixa_velocidade(), Some((100, 100)));
    }

    #[test]
    fn velocity_variavel_observada() {
        let mut c = Capacidades::novas();
        c.registrar(&ev(&[0x90, 60, 30]));
        c.registrar(&ev(&[0x90, 62, 110]));
        assert_eq!(c.velocidade_variavel(), Observacao::Observado);
        assert_eq!(c.faixa_velocidade(), Some((30, 110)));
    }

    #[test]
    fn velocity_zero_e_liberacao_nao_velocidade() {
        let mut c = Capacidades::novas();
        c.registrar(&ev(&[0x90, 60, 100]));
        c.registrar(&ev(&[0x90, 60, 0]));
        assert_eq!(c.velocidade_variavel(), Observacao::NaoObservado);
        assert_eq!(c.liberacao_de_nota(), Observacao::Observado);
    }

    #[test]
    fn cc64_e_programa_observados() {
        let mut c = Capacidades::novas();
        c.registrar(&ev(&[0xB0, 7, 100]));
        assert_eq!(c.pedal_sustain(), Observacao::NaoObservado);
        c.registrar(&ev(&[0xB0, 64, 127]));
        c.registrar(&ev(&[0xC3, 5]));
        assert_eq!(c.pedal_sustain(), Observacao::Observado);
        assert_eq!(c.ultimo_valor_sustain(), Some(127));
        assert_eq!(c.mudanca_de_programa(), Observacao::Observado);
        assert_eq!(c.ultimo_programa(), Some(5));
        let canais: Vec<u8> = c.canais_observados().map(CanalMidi::exibicao).collect();
        assert_eq!(canais, vec![1, 4]);
    }
}
