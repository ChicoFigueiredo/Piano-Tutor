//! Comandos da thread de controle para o callback de áudio e o lado produtor
//! da fila sem lock.
//!
//! **Fila:** `rtrb` 0.4.1 (MIT OR Apache-2.0, sem dependências), anel SPSC
//! *wait-free* que não aloca depois de criado. Um anel próprio exigiria
//! `unsafe` (proibido pelo lint do workspace). Há um único produtor
//! ([`ControleAudio`], `Send` mas não `Sync`): se MIDI e UI precisarem enviar
//! de threads diferentes, elas encaminham para a thread de controle (ou
//! envolvem o `ControleAudio` em `Mutex` **do lado de controle**; o callback
//! nunca toma lock).
//!
//! **Pânico:** não depende de vaga na fila. [`ControleAudio::panico`]
//! incrementa uma geração atômica; o callback silencia tudo, descarta os
//! agendamentos e ignora comandos enviados antes do pânico.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use piano_tutor_core::NotaMidi;
use piano_tutor_core::midi::{CanalMidi, EventoMidi};

use crate::erro::ErroAudio;
use crate::motor::EstadoMotor;

/// Menor valor de pitch bend (curvatura máxima para baixo).
pub const PITCH_BEND_MIN: i16 = -8192;
/// Maior valor de pitch bend (curvatura máxima para cima).
pub const PITCH_BEND_MAX: i16 = 8191;

/// Converte o pitch bend centrado em zero (`-8192..=8191`, limitado) nos dois bytes
/// de dados MIDI `(lsb, msb)` do status `0xEn` (14 bits, centro 0x2000 = 8192).
pub const fn pitch_bend_para_bytes(valor: i16) -> (u8, u8) {
    let v = if valor < PITCH_BEND_MIN {
        PITCH_BEND_MIN
    } else if valor > PITCH_BEND_MAX {
        PITCH_BEND_MAX
    } else {
        valor
    };
    let bruto = (v as i32 + 8192) as u16;
    ((bruto & 0x7F) as u8, ((bruto >> 7) & 0x7F) as u8)
}

/// Quando o comando deve soar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Quando {
    /// No início do próximo bloco de áudio (menor latência, jitter de até um
    /// bloco).
    Imediato,
    /// Instante monotônico de captura ([`crate::RelogioMonotonico`]); o motor
    /// converte para frame e soma a latência alvo (jitter baixo, latência
    /// constante).
    Instante(u64),
    /// Frame absoluto do relógio mestre (metrônomo, playback, pads).
    Frame(u64),
}

/// Ação aplicada ao sintetizador ou ao mixer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Acao {
    NotaLigada {
        canal: CanalMidi,
        nota: NotaMidi,
        velocidade: u8,
    },
    NotaDesligada {
        canal: CanalMidi,
        nota: NotaMidi,
    },
    Controle {
        canal: CanalMidi,
        controle: u8,
        valor: u8,
    },
    Programa {
        canal: CanalMidi,
        programa: u8,
    },
    /// Pitch bend (roda de afinação). `valor` está centrado em zero: 0 = sem
    /// curvatura, de [`PITCH_BEND_MIN`] (-8192) a [`PITCH_BEND_MAX`] (+8191).
    /// Na fiação MIDI vira 14 bits (`valor + 8192`), ver [`pitch_bend_para_bytes`].
    /// A faixa de curvatura é a padrão do GM/SF2 (±2 semitons, RPN 0 não é alterado).
    PitchBend {
        canal: CanalMidi,
        valor: i16,
    },
    /// Libera todas as notas com o *release* normal (all-notes-off).
    TodasNotasDesligadas,
    /// Ganho mestre de saída, limitado a 0,0–1,0, com rampa curta.
    Volume(f32),
}

/// Comando enviado pela fila. `geracao` é preenchida pelo [`ControleAudio`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComandoAudio {
    pub quando: Quando,
    pub acao: Acao,
    pub(crate) geracao: u32,
}

impl ComandoAudio {
    pub fn novo(quando: Quando, acao: Acao) -> Self {
        Self {
            quando,
            acao,
            geracao: 0,
        }
    }

    /// Converte um evento MIDI normalizado em comando agendado pelo seu
    /// timestamp monotônico. `None` para eventos fora do escopo.
    pub fn de_evento_midi(evento: &EventoMidi) -> Option<Self> {
        let quando = Quando::Instante(evento.timestamp_monotonico_ns());
        let acao = match *evento {
            EventoMidi::NotaLigada {
                canal,
                nota,
                velocidade,
                ..
            } => Acao::NotaLigada {
                canal,
                nota,
                velocidade,
            },
            EventoMidi::NotaDesligada { canal, nota, .. } => Acao::NotaDesligada { canal, nota },
            EventoMidi::ControleMudou {
                canal,
                controle,
                valor,
                ..
            } => Acao::Controle {
                canal,
                controle,
                valor,
            },
            EventoMidi::ProgramaMudou {
                canal, programa, ..
            } => Acao::Programa { canal, programa },
            EventoMidi::PitchBend { canal, valor, .. } => Acao::PitchBend { canal, valor },
            EventoMidi::Desconhecido { .. } => return None,
        };
        Some(Self::novo(quando, acao))
    }
}

/// Lado de controle (produtor) da fila. Nunca bloqueia.
pub struct ControleAudio {
    produtor: rtrb::Producer<ComandoAudio>,
    estado: Arc<EstadoMotor>,
}

impl ControleAudio {
    pub(crate) fn novo(produtor: rtrb::Producer<ComandoAudio>, estado: Arc<EstadoMotor>) -> Self {
        Self { produtor, estado }
    }

    /// Envia um comando. Com a fila cheia devolve [`ErroAudio::FilaCheia`]
    /// (recuperável: tentar de novo depois).
    pub fn enviar(&mut self, mut comando: ComandoAudio) -> Result<(), ErroAudio> {
        comando.geracao = self.estado.geracao_panico.load(Ordering::Acquire);
        self.produtor
            .push(comando)
            .map_err(|_| ErroAudio::FilaCheia)
    }

    /// Agenda um evento MIDI pelo seu timestamp. `Ok(false)` quando o evento
    /// é ignorado (tipo fora do escopo).
    pub fn enviar_evento_midi(&mut self, evento: &EventoMidi) -> Result<bool, ErroAudio> {
        match ComandoAudio::de_evento_midi(evento) {
            Some(c) => self.enviar(c).map(|()| true),
            None => Ok(false),
        }
    }

    /// Ganho mestre (0,0–1,0) no próximo bloco.
    pub fn volume(&mut self, volume: f32) -> Result<(), ErroAudio> {
        self.enviar(ComandoAudio::novo(Quando::Imediato, Acao::Volume(volume)))
    }

    /// Pitch bend (-8192..=8191, 0 = centro) no próximo bloco.
    pub fn pitch_bend(&mut self, canal: CanalMidi, valor: i16) -> Result<(), ErroAudio> {
        self.enviar(ComandoAudio::novo(
            Quando::Imediato,
            Acao::PitchBend { canal, valor },
        ))
    }

    /// All-notes-off (com release) no próximo bloco.
    pub fn todas_notas_desligadas(&mut self) -> Result<(), ErroAudio> {
        self.enviar(ComandoAudio::novo(
            Quando::Imediato,
            Acao::TodasNotasDesligadas,
        ))
    }

    /// Pânico: corte imediato de todas as vozes, reset de controladores
    /// (inclusive sustain) e descarte de agendamentos anteriores. Não usa a
    /// fila, portanto funciona mesmo com a fila cheia. Usar em desconexão do
    /// teclado, troca de timbre ou erro de dispositivo (spec §3.4).
    pub fn panico(&self) {
        self.estado.geracao_panico.fetch_add(1, Ordering::AcqRel);
    }

    /// Vagas livres na fila.
    pub fn vagas(&self) -> usize {
        self.produtor.slots()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn canal(n: u8) -> CanalMidi {
        CanalMidi::novo(n).expect("canal válido")
    }

    fn nota(n: u8) -> NotaMidi {
        NotaMidi::nova(n).expect("nota válida")
    }

    #[test]
    fn evento_midi_vira_comando_agendado_pelo_timestamp() {
        let ev = EventoMidi::NotaLigada {
            canal: canal(0),
            nota: nota(60),
            velocidade: 100,
            timestamp_monotonico_ns: 123,
        };
        let c = ComandoAudio::de_evento_midi(&ev).expect("nota é suportada");
        assert_eq!(c.quando, Quando::Instante(123));
        assert_eq!(
            c.acao,
            Acao::NotaLigada {
                canal: canal(0),
                nota: nota(60),
                velocidade: 100
            }
        );
    }

    #[test]
    fn todos_os_tipos_suportados_sao_convertidos() {
        let off = EventoMidi::NotaDesligada {
            canal: canal(9),
            nota: nota(36),
            velocidade: 0,
            timestamp_monotonico_ns: 1,
        };
        let cc = EventoMidi::ControleMudou {
            canal: canal(0),
            controle: 64,
            valor: 127,
            timestamp_monotonico_ns: 2,
        };
        let pc = EventoMidi::ProgramaMudou {
            canal: canal(0),
            programa: 21,
            timestamp_monotonico_ns: 3,
        };
        assert!(matches!(
            ComandoAudio::de_evento_midi(&off).map(|c| c.acao),
            Some(Acao::NotaDesligada { .. })
        ));
        assert!(matches!(
            ComandoAudio::de_evento_midi(&cc).map(|c| c.acao),
            Some(Acao::Controle {
                controle: 64,
                valor: 127,
                ..
            })
        ));
        assert!(matches!(
            ComandoAudio::de_evento_midi(&pc).map(|c| c.acao),
            Some(Acao::Programa { programa: 21, .. })
        ));
        let desconhecido = EventoMidi::Desconhecido {
            timestamp_monotonico_ns: 4,
        };
        assert!(ComandoAudio::de_evento_midi(&desconhecido).is_none());
    }

    #[test]
    fn pitch_bend_codifica_lsb_msb_em_14_bits_com_centro_2000h() {
        assert_eq!(pitch_bend_para_bytes(0), (0x00, 0x40));
        assert_eq!(pitch_bend_para_bytes(PITCH_BEND_MIN), (0x00, 0x00));
        assert_eq!(pitch_bend_para_bytes(PITCH_BEND_MAX), (0x7F, 0x7F));
        // 8192 + 1 = 0x2001: lsb 1, msb 0x40.
        assert_eq!(pitch_bend_para_bytes(1), (0x01, 0x40));
        // 8192 + 128 = 0x2080: lsb 0, msb 0x41.
        assert_eq!(pitch_bend_para_bytes(128), (0x00, 0x41));
        // Fora da faixa é limitado, nunca estoura o bit 7.
        assert_eq!(pitch_bend_para_bytes(i16::MIN), (0x00, 0x00));
        assert_eq!(pitch_bend_para_bytes(i16::MAX), (0x7F, 0x7F));
        for v in (PITCH_BEND_MIN..=PITCH_BEND_MAX).step_by(7) {
            let (l, m) = pitch_bend_para_bytes(v);
            assert!(l < 128 && m < 128);
            let volta = (i32::from(m) << 7 | i32::from(l)) - 8192;
            assert_eq!(volta, i32::from(v));
        }
    }

    #[test]
    fn evento_pitch_bend_vira_comando() {
        let ev = EventoMidi::PitchBend {
            canal: canal(2),
            valor: -300,
            timestamp_monotonico_ns: 55,
        };
        let c = ComandoAudio::de_evento_midi(&ev).expect("pitch bend é suportado");
        assert_eq!(c.quando, Quando::Instante(55));
        assert_eq!(
            c.acao,
            Acao::PitchBend {
                canal: canal(2),
                valor: -300
            }
        );
    }

    #[test]
    fn fila_cheia_e_erro_recuperavel() {
        let estado = Arc::new(EstadoMotor::novo());
        let (produtor, mut consumidor) = rtrb::RingBuffer::new(2);
        let mut controle = ControleAudio::novo(produtor, estado);
        let cmd = ComandoAudio::novo(Quando::Imediato, Acao::TodasNotasDesligadas);
        assert!(controle.enviar(cmd).is_ok());
        assert!(controle.enviar(cmd).is_ok());
        assert_eq!(controle.vagas(), 0);
        assert!(matches!(controle.enviar(cmd), Err(ErroAudio::FilaCheia)));
        assert!(consumidor.pop().is_ok());
        assert!(controle.enviar(cmd).is_ok());
    }

    #[test]
    fn panico_funciona_com_fila_cheia() {
        let estado = Arc::new(EstadoMotor::novo());
        let (produtor, _consumidor) = rtrb::RingBuffer::new(1);
        let mut controle = ControleAudio::novo(produtor, Arc::clone(&estado));
        let cmd = ComandoAudio::novo(Quando::Imediato, Acao::TodasNotasDesligadas);
        assert!(controle.enviar(cmd).is_ok());
        controle.panico();
        assert_eq!(estado.geracao_panico.load(Ordering::Acquire), 1);
    }
}
