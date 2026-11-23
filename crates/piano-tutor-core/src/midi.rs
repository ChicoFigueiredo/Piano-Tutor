//! Evento MIDI normalizado e parser de mensagens (spec §8.1, ticket #30, F01).
//!
//! Código puro: sem porta MIDI, sem relógio, sem threads. O timestamp
//! monotônico em nanossegundos é fornecido por quem chama.

use crate::NotaMidi;
use std::fmt;

/// Canal MIDI. Internamente zero-based (0–15); a UI exibe 1–16.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanalMidi(u8);

impl CanalMidi {
    /// Cria a partir do valor interno (0–15).
    pub const fn novo(interno: u8) -> Option<Self> {
        if interno <= 15 {
            Some(Self(interno))
        } else {
            None
        }
    }

    /// Cria a partir do número exibido na UI (1–16).
    pub const fn de_exibicao(exibido: u8) -> Option<Self> {
        if exibido >= 1 && exibido <= 16 {
            Some(Self(exibido - 1))
        } else {
            None
        }
    }

    /// Valor interno, 0–15.
    pub const fn interno(self) -> u8 {
        self.0
    }

    /// Valor para exibição, 1–16.
    pub const fn exibicao(self) -> u8 {
        self.0 + 1
    }
}

impl fmt::Display for CanalMidi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.exibicao())
    }
}

/// Evento MIDI normalizado. `velocidade` de `NotaLigada` está em 1–127
/// (NoteOn com velocity 0 vira `NotaDesligada`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventoMidi {
    NotaLigada {
        canal: CanalMidi,
        nota: NotaMidi,
        velocidade: u8,
        timestamp_monotonico_ns: u64,
    },
    NotaDesligada {
        canal: CanalMidi,
        nota: NotaMidi,
        velocidade: u8,
        timestamp_monotonico_ns: u64,
    },
    /// Control Change; CC64 (sustain) chega por aqui, sem tratamento especial.
    ControleMudou {
        canal: CanalMidi,
        controle: u8,
        valor: u8,
        timestamp_monotonico_ns: u64,
    },
    ProgramaMudou {
        canal: CanalMidi,
        programa: u8,
        timestamp_monotonico_ns: u64,
    },
    /// Pitch bend (0xEn). `valor` centrado em zero: -8192..=8191, 0 = sem
    /// curvatura (14 bits MIDI menos 8192).
    PitchBend {
        canal: CanalMidi,
        valor: i16,
        timestamp_monotonico_ns: u64,
    },
    /// Mensagem válida, mas fora do escopo atual (aftertouch, sistema, tempo
    /// real). Ignorada pelo restante do pipeline.
    Desconhecido { timestamp_monotonico_ns: u64 },
}

impl EventoMidi {
    pub const fn timestamp_monotonico_ns(&self) -> u64 {
        match *self {
            Self::NotaLigada {
                timestamp_monotonico_ns,
                ..
            }
            | Self::NotaDesligada {
                timestamp_monotonico_ns,
                ..
            }
            | Self::ControleMudou {
                timestamp_monotonico_ns,
                ..
            }
            | Self::ProgramaMudou {
                timestamp_monotonico_ns,
                ..
            }
            | Self::PitchBend {
                timestamp_monotonico_ns,
                ..
            }
            | Self::Desconhecido {
                timestamp_monotonico_ns,
            } => timestamp_monotonico_ns,
        }
    }
}

/// Erros de interpretação; o parser nunca entra em pânico.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErroMidi {
    /// Nenhum byte recebido.
    Vazia,
    /// Byte de dados sem status anterior (running status inexistente).
    SemStatus,
    /// Faltam bytes de dados para a mensagem.
    Incompleta { esperado: usize, recebido: usize },
    /// Byte com bit 7 ligado onde se esperava dado (0–127).
    DadoInvalido { posicao: usize, byte: u8 },
    /// Bytes sobrando após uma mensagem completa.
    BytesExcedentes { esperado: usize, recebido: usize },
}

impl fmt::Display for ErroMidi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Vazia => write!(f, "mensagem MIDI vazia"),
            Self::SemStatus => write!(f, "byte de dados sem status anterior"),
            Self::Incompleta { esperado, recebido } => {
                write!(
                    f,
                    "mensagem incompleta: esperados {esperado}, recebidos {recebido}"
                )
            }
            Self::DadoInvalido { posicao, byte } => {
                write!(
                    f,
                    "byte de dados inválido 0x{byte:02X} na posição {posicao}"
                )
            }
            Self::BytesExcedentes { esperado, recebido } => {
                write!(
                    f,
                    "bytes excedentes: esperados {esperado}, recebidos {recebido}"
                )
            }
        }
    }
}

impl std::error::Error for ErroMidi {}

/// Parser com estado de running status. Uma chamada = uma mensagem
/// (como entregue por callbacks de porta MIDI).
#[derive(Clone, Copy, Debug, Default)]
pub struct Parser {
    running_status: Option<u8>,
}

impl Parser {
    pub const fn novo() -> Self {
        Self {
            running_status: None,
        }
    }

    /// Esquece o running status (ex.: após reconexão do dispositivo).
    pub fn reiniciar(&mut self) {
        self.running_status = None;
    }

    /// Interpreta uma mensagem. Quando chega um novo status de canal, o
    /// running status anterior é descartado; só é gravado após a mensagem
    /// ser validada (uma mensagem rejeitada não deixa status "meio válido").
    pub fn interpretar(&mut self, bytes: &[u8], ts_ns: u64) -> Result<EventoMidi, ErroMidi> {
        let primeiro = *bytes.first().ok_or(ErroMidi::Vazia)?;
        let tem_status = primeiro & 0x80 != 0;
        let (status, dados) = if tem_status {
            (primeiro, &bytes[1..])
        } else {
            (self.running_status.ok_or(ErroMidi::SemStatus)?, bytes)
        };
        // Posição do primeiro byte de dados dentro de `bytes`.
        let base = bytes.len() - dados.len();

        if status >= 0xF0 {
            // Tempo real (0xF8..) não afeta running status; sistema comum limpa.
            if status < 0xF8 {
                self.running_status = None;
            }
            return Ok(EventoMidi::Desconhecido {
                timestamp_monotonico_ns: ts_ns,
            });
        }

        let tipo = status & 0xF0;
        let esperado = if matches!(tipo, 0xC0 | 0xD0) { 1 } else { 2 };
        if tem_status {
            self.running_status = None;
        }
        if dados.len() < esperado {
            return Err(ErroMidi::Incompleta {
                esperado,
                recebido: dados.len(),
            });
        }
        if dados.len() > esperado {
            return Err(ErroMidi::BytesExcedentes {
                esperado,
                recebido: dados.len(),
            });
        }
        for (i, &b) in dados.iter().enumerate() {
            if b & 0x80 != 0 {
                return Err(ErroMidi::DadoInvalido {
                    posicao: base + i,
                    byte: b,
                });
            }
        }
        self.running_status = Some(status);

        // Canal sempre 0–15 por construção (status & 0x0F).
        let canal = CanalMidi(status & 0x0F);
        let timestamp_monotonico_ns = ts_ns;
        let d1 = dados[0];
        let d2 = if esperado == 2 { dados[1] } else { 0 };
        // Dados já validados em 0–127.
        let nota = NotaMidi::nova(d1).ok_or(ErroMidi::DadoInvalido {
            posicao: base,
            byte: d1,
        })?;
        Ok(match tipo {
            0x80 => EventoMidi::NotaDesligada {
                canal,
                nota,
                velocidade: d2,
                timestamp_monotonico_ns,
            },
            0x90 if d2 == 0 => EventoMidi::NotaDesligada {
                canal,
                nota,
                velocidade: 0,
                timestamp_monotonico_ns,
            },
            0x90 => EventoMidi::NotaLigada {
                canal,
                nota,
                velocidade: d2,
                timestamp_monotonico_ns,
            },
            0xB0 => EventoMidi::ControleMudou {
                canal,
                controle: d1,
                valor: d2,
                timestamp_monotonico_ns,
            },
            0xC0 => EventoMidi::ProgramaMudou {
                canal,
                programa: d1,
                timestamp_monotonico_ns,
            },
            // d1 = LSB, d2 = MSB; 14 bits com centro em 0x2000.
            0xE0 => EventoMidi::PitchBend {
                canal,
                valor: ((i16::from(d2) << 7) | i16::from(d1)) - 8192,
                timestamp_monotonico_ns,
            },
            // Aftertouch (0xA0, 0xD0): fora do escopo.
            _ => EventoMidi::Desconhecido {
                timestamp_monotonico_ns,
            },
        })
    }
}

/// Interpreta uma mensagem isolada, sem running status.
pub fn interpretar(bytes: &[u8], ts_ns: u64) -> Result<EventoMidi, ErroMidi> {
    Parser::novo().interpretar(bytes, ts_ns)
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::collections::HashMap;

    fn nota(n: u8) -> NotaMidi {
        NotaMidi::nova(n).expect("nota válida")
    }

    fn canal(c: u8) -> CanalMidi {
        CanalMidi::novo(c).expect("canal válido")
    }

    #[test]
    fn canal_interno_e_exibicao() {
        assert_eq!(canal(0).exibicao(), 1);
        assert_eq!(canal(15).exibicao(), 16);
        assert_eq!(CanalMidi::de_exibicao(1), CanalMidi::novo(0));
        assert_eq!(CanalMidi::de_exibicao(16), CanalMidi::novo(15));
        assert_eq!(CanalMidi::de_exibicao(0), None);
        assert_eq!(CanalMidi::de_exibicao(17), None);
        assert_eq!(CanalMidi::novo(16), None);
        assert_eq!(canal(9).to_string(), "10");
    }

    #[test]
    fn note_on_normal() {
        assert_eq!(
            interpretar(&[0x93, 60, 96], 7),
            Ok(EventoMidi::NotaLigada {
                canal: canal(3),
                nota: nota(60),
                velocidade: 96,
                timestamp_monotonico_ns: 7
            })
        );
    }

    #[test]
    fn note_on_velocity_zero_e_desligamento() {
        assert_eq!(
            interpretar(&[0x90, 60, 0], 1),
            Ok(EventoMidi::NotaDesligada {
                canal: canal(0),
                nota: nota(60),
                velocidade: 0,
                timestamp_monotonico_ns: 1
            })
        );
    }

    #[test]
    fn note_off_explicito() {
        assert_eq!(
            interpretar(&[0x8F, 61, 40], 2),
            Ok(EventoMidi::NotaDesligada {
                canal: canal(15),
                nota: nota(61),
                velocidade: 40,
                timestamp_monotonico_ns: 2
            })
        );
    }

    #[test]
    fn cc64_sustain_e_program_change() {
        assert_eq!(
            interpretar(&[0xB0, 64, 127], 3),
            Ok(EventoMidi::ControleMudou {
                canal: canal(0),
                controle: 64,
                valor: 127,
                timestamp_monotonico_ns: 3
            })
        );
        assert_eq!(
            interpretar(&[0xC2, 5], 4),
            Ok(EventoMidi::ProgramaMudou {
                canal: canal(2),
                programa: 5,
                timestamp_monotonico_ns: 4
            })
        );
    }

    #[test]
    fn mensagens_fora_do_escopo_sao_desconhecidas() {
        for msg in [
            &[0xA0u8, 60, 10][..],
            &[0xD0, 10],
            &[0xF8],
            &[0xFE],
            &[0xF0, 0x7E, 0xF7],
        ] {
            assert_eq!(
                interpretar(msg, 9),
                Ok(EventoMidi::Desconhecido {
                    timestamp_monotonico_ns: 9
                }),
                "{msg:02X?}"
            );
        }
    }

    #[test]
    fn pitch_bend_e_decodificado_em_14_bits_centrado_em_zero() {
        let pb = |bytes: &[u8]| interpretar(bytes, 5);
        let ev = |c: u8, valor: i16| {
            Ok(EventoMidi::PitchBend {
                canal: canal(c),
                valor,
                timestamp_monotonico_ns: 5,
            })
        };
        // Centro: lsb 0, msb 0x40.
        assert_eq!(pb(&[0xE0, 0x00, 0x40]), ev(0, 0));
        // Extremos.
        assert_eq!(pb(&[0xE0, 0x00, 0x00]), ev(0, -8192));
        assert_eq!(pb(&[0xE3, 0x7F, 0x7F]), ev(3, 8191));
        // lsb entra nos 7 bits baixos, msb nos altos.
        assert_eq!(pb(&[0xE1, 0x01, 0x40]), ev(1, 1));
        assert_eq!(pb(&[0xE1, 0x00, 0x41]), ev(1, 128));
        assert_eq!(pb(&[0xEF, 0x2A, 0x10]), ev(15, (0x10 << 7 | 0x2A) - 8192));
        assert_eq!(
            pb(&[0xE0, 0x00, 0x40])
                .expect("válido")
                .timestamp_monotonico_ns(),
            5
        );
    }

    #[test]
    fn pitch_bend_incompleto_ou_invalido_e_erro_e_running_status_funciona() {
        assert_eq!(
            interpretar(&[0xE0, 0x00], 0),
            Err(ErroMidi::Incompleta {
                esperado: 2,
                recebido: 1
            })
        );
        assert_eq!(
            interpretar(&[0xE0, 0x00, 0x80], 0),
            Err(ErroMidi::DadoInvalido {
                posicao: 2,
                byte: 0x80
            })
        );
        let mut p = Parser::novo();
        assert!(p.interpretar(&[0xE2, 0x00, 0x40], 1).is_ok());
        assert_eq!(
            p.interpretar(&[0x7F, 0x7F], 2),
            Ok(EventoMidi::PitchBend {
                canal: canal(2),
                valor: 8191,
                timestamp_monotonico_ns: 2
            })
        );
    }

    #[test]
    fn erros_tipados_sem_panico() {
        assert_eq!(interpretar(&[], 0), Err(ErroMidi::Vazia));
        assert_eq!(interpretar(&[60, 64], 0), Err(ErroMidi::SemStatus));
        assert_eq!(
            interpretar(&[0x90], 0),
            Err(ErroMidi::Incompleta {
                esperado: 2,
                recebido: 0
            })
        );
        assert_eq!(
            interpretar(&[0x90, 60], 0),
            Err(ErroMidi::Incompleta {
                esperado: 2,
                recebido: 1
            })
        );
        assert_eq!(
            interpretar(&[0xC0], 0),
            Err(ErroMidi::Incompleta {
                esperado: 1,
                recebido: 0
            })
        );
        assert_eq!(
            interpretar(&[0x90, 60, 0x80], 0),
            Err(ErroMidi::DadoInvalido {
                posicao: 2,
                byte: 0x80
            })
        );
        assert_eq!(
            interpretar(&[0x90, 60, 64, 1], 0),
            Err(ErroMidi::BytesExcedentes {
                esperado: 2,
                recebido: 3
            })
        );
    }

    #[test]
    fn nenhuma_entrada_curta_causa_panico() {
        let mut p = Parser::novo();
        for a in 0..=255u8 {
            let _ = p.interpretar(&[a], 0);
            for b in 0..=255u8 {
                let _ = p.interpretar(&[a, b], 0);
                let _ = p.interpretar(&[a, b, b ^ 0x55], 0);
            }
        }
    }

    #[test]
    fn running_status() {
        let mut p = Parser::novo();
        assert!(matches!(
            p.interpretar(&[0x91, 60, 100], 1),
            Ok(EventoMidi::NotaLigada { .. })
        ));
        assert_eq!(
            p.interpretar(&[62, 90], 2),
            Ok(EventoMidi::NotaLigada {
                canal: canal(1),
                nota: nota(62),
                velocidade: 90,
                timestamp_monotonico_ns: 2
            })
        );
        assert!(matches!(
            p.interpretar(&[62, 0], 3),
            Ok(EventoMidi::NotaDesligada { velocidade: 0, .. })
        ));
        // Tempo real não limpa o running status.
        assert!(p.interpretar(&[0xF8], 4).is_ok());
        assert!(matches!(
            p.interpretar(&[64, 50], 5),
            Ok(EventoMidi::NotaLigada { .. })
        ));
        // Mensagem de sistema comum limpa.
        assert!(p.interpretar(&[0xF1, 0], 6).is_ok());
        assert_eq!(p.interpretar(&[64, 50], 7), Err(ErroMidi::SemStatus));
    }

    #[test]
    fn running_status_de_program_change_usa_um_dado() {
        let mut p = Parser::novo();
        assert!(p.interpretar(&[0xC0, 1], 0).is_ok());
        assert_eq!(
            p.interpretar(&[2], 1),
            Ok(EventoMidi::ProgramaMudou {
                canal: canal(0),
                programa: 2,
                timestamp_monotonico_ns: 1
            })
        );
    }

    #[test]
    fn novo_status_invalido_invalida_running_status() {
        let mut p = Parser::novo();
        assert!(p.interpretar(&[0x90, 60, 100], 0).is_ok());
        assert!(p.interpretar(&[0x91, 60], 1).is_err());
        assert_eq!(p.interpretar(&[60, 100], 2), Err(ErroMidi::SemStatus));
        p.reiniciar();
        assert_eq!(p.interpretar(&[60, 100], 3), Err(ErroMidi::SemStatus));
    }

    #[test]
    fn timestamp_e_preservado_e_monotonico() {
        let mut p = Parser::novo();
        let mut anterior = 0;
        for i in 0..100u64 {
            let ts = 1_000 + i * 31;
            let vel = 1 + u8::try_from(i % 127).expect("cabe em u8");
            let ev = p.interpretar(&[0x90, 60, vel], ts);
            let ts_ev = ev.expect("válido").timestamp_monotonico_ns();
            assert_eq!(ts_ev, ts);
            assert!(ts_ev >= anterior);
            anterior = ts_ev;
        }
    }

    /// Gerador LCG determinístico (sem dependências).
    struct Lcg(u64);
    impl Lcg {
        fn proximo(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn byte7(&mut self, min: u8) -> u8 {
            let faixa = u64::from(128 - min);
            min + u8::try_from(self.proximo() % faixa).expect("< 128")
        }
    }

    #[test]
    fn dez_mil_pares_note_on_off_sem_perdas() {
        const PARES: usize = 10_000;
        let mut rng = Lcg(42);
        let mut p = Parser::novo();
        let mut ts = 0u64;
        // Chave (canal, nota) -> timestamp do NoteOn pendente.
        let mut pendentes: HashMap<(u8, u8), u64> = HashMap::new();
        let mut pareados = 0usize;
        let mut ligadas = 0usize;
        let mut desligadas = 0usize;
        let mut ultimo_ts = 0u64;

        for i in 0..PARES {
            let canal_i = u8::try_from(i % 16).expect("< 16");
            let nota_i = u8::try_from((i / 16) % 128).expect("< 128");
            let vel = rng.byte7(1);
            let com_vel_zero = rng.proximo().is_multiple_of(2);
            let status_on = 0x90 | canal_i;
            let off: Vec<u8> = if com_vel_zero {
                vec![status_on, nota_i, 0]
            } else {
                vec![0x80 | canal_i, nota_i, rng.byte7(0)]
            };

            ts += 1 + rng.proximo() % 1000;
            match p
                .interpretar(&[status_on, nota_i, vel], ts)
                .expect("NoteOn válido")
            {
                EventoMidi::NotaLigada {
                    canal,
                    nota,
                    velocidade,
                    timestamp_monotonico_ns,
                } => {
                    assert_eq!(velocidade, vel);
                    assert!(timestamp_monotonico_ns >= ultimo_ts);
                    ultimo_ts = timestamp_monotonico_ns;
                    ligadas += 1;
                    let antigo =
                        pendentes.insert((canal.interno(), nota.numero()), timestamp_monotonico_ns);
                    assert!(antigo.is_none(), "NoteOn duplicado pendente no índice {i}");
                }
                outro => panic!("esperava NotaLigada, veio {outro:?}"),
            }

            ts += 1 + rng.proximo() % 1000;
            match p.interpretar(&off, ts).expect("NoteOff válido") {
                EventoMidi::NotaDesligada {
                    canal,
                    nota,
                    timestamp_monotonico_ns,
                    ..
                } => {
                    assert!(timestamp_monotonico_ns >= ultimo_ts);
                    ultimo_ts = timestamp_monotonico_ns;
                    desligadas += 1;
                    let ts_on = pendentes
                        .remove(&(canal.interno(), nota.numero()))
                        .expect("NoteOff sem NoteOn correspondente");
                    assert!(ts_on <= timestamp_monotonico_ns);
                    pareados += 1;
                }
                outro => panic!("esperava NotaDesligada, veio {outro:?}"),
            }
        }

        assert_eq!(ligadas, PARES);
        assert_eq!(desligadas, PARES);
        assert_eq!(pareados, PARES, "pares perdidos");
        assert!(pendentes.is_empty(), "notas ligadas sem par");
    }

    #[test]
    fn dez_mil_pares_em_fluxo_running_status() {
        // Omite o byte de status quando igual ao anterior.
        const PARES: usize = 10_000;
        let mut rng = Lcg(7);
        let mut p = Parser::novo();
        let mut ultimo_status: Option<u8> = None;
        let mut pareados = 0usize;
        let mut ts = 0u64;
        for i in 0..PARES {
            let nota_i = u8::try_from(i % 128).expect("< 128");
            let vel = rng.byte7(1);
            let mut enviar = |p: &mut Parser, d2: u8| {
                ts += 1;
                let msg: Vec<u8> = if ultimo_status == Some(0x90) {
                    vec![nota_i, d2]
                } else {
                    vec![0x90, nota_i, d2]
                };
                ultimo_status = Some(0x90);
                p.interpretar(&msg, ts).expect("mensagem válida")
            };
            let on = enviar(&mut p, vel);
            let off = enviar(&mut p, 0);
            match (on, off) {
                (
                    EventoMidi::NotaLigada {
                        nota: a, canal: ca, ..
                    },
                    EventoMidi::NotaDesligada {
                        nota: b, canal: cb, ..
                    },
                ) if a == b && ca == cb => pareados += 1,
                outro => panic!("par incorreto: {outro:?}"),
            }
        }
        assert_eq!(pareados, PARES);
    }
}
