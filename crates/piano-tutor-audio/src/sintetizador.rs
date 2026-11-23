//! Contrato do sintetizador e um sintetizador falso determinístico para
//! testes. A implementação real (`rustysynth`) está em [`crate::rusty`].

use piano_tutor_core::NotaMidi;
use piano_tutor_core::midi::CanalMidi;

/// Sintetizador acionado **somente** pelo callback de áudio.
///
/// Implementações reais não podem alocar, bloquear nem fazer E/S nestes
/// métodos. `renderizar` sobrescreve `esq` e `dir` (mesmo comprimento) e
/// avança o tempo interno do sintetizador nesse número de frames.
pub trait Sintetizador: Send {
    fn nota_ligada(&mut self, canal: CanalMidi, nota: NotaMidi, velocidade: u8);
    fn nota_desligada(&mut self, canal: CanalMidi, nota: NotaMidi);
    fn controle(&mut self, canal: CanalMidi, controle: u8, valor: u8);
    fn programa(&mut self, canal: CanalMidi, programa: u8);
    /// Pitch bend centrado em zero (-8192..=8191; 0 = sem curvatura). Faixa
    /// padrão do GM (±2 semitons).
    fn pitch_bend(&mut self, canal: CanalMidi, valor: i16);
    /// All-notes-off com release normal.
    fn todas_notas_desligadas(&mut self);
    /// Pânico: corte imediato de todas as vozes e reset de controladores.
    fn silenciar_tudo(&mut self);
    fn renderizar(&mut self, esq: &mut [f32], dir: &mut [f32]);
}

/// O que o [`SintetizadorFalso`] recebeu, com o frame interno do momento.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistroFalso {
    Ligada {
        frame: u64,
        canal: u8,
        nota: u8,
        velocidade: u8,
    },
    Desligada {
        frame: u64,
        canal: u8,
        nota: u8,
    },
    Controle {
        frame: u64,
        canal: u8,
        controle: u8,
        valor: u8,
    },
    Programa {
        frame: u64,
        canal: u8,
        programa: u8,
    },
    PitchBend {
        frame: u64,
        canal: u8,
        valor: i16,
    },
    TodasDesligadas {
        frame: u64,
    },
    Silenciar {
        frame: u64,
    },
}

/// Sintetizador falso e determinístico, **somente para testes e render
/// offline** (o registro de eventos aloca).
///
/// Saída: um impulso de amplitude `velocidade / 127` no primeiro frame após
/// cada nota ligada, e silêncio exato no resto. Isso permite medir em que
/// frame cada evento soou.
#[derive(Debug)]
pub struct SintetizadorFalso {
    frame: u64,
    impulso_pendente: f32,
    ativas: [[bool; 128]; 16],
    registro: Vec<RegistroFalso>,
}

impl Default for SintetizadorFalso {
    fn default() -> Self {
        Self::novo()
    }
}

impl SintetizadorFalso {
    pub fn novo() -> Self {
        Self {
            frame: 0,
            impulso_pendente: 0.0,
            ativas: [[false; 128]; 16],
            registro: Vec::new(),
        }
    }

    pub fn registro(&self) -> &[RegistroFalso] {
        &self.registro
    }

    pub fn notas_ativas(&self) -> usize {
        self.ativas.iter().flatten().filter(|a| **a).count()
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }
}

impl Sintetizador for SintetizadorFalso {
    fn nota_ligada(&mut self, canal: CanalMidi, nota: NotaMidi, velocidade: u8) {
        self.ativas[usize::from(canal.interno())][usize::from(nota.numero())] = true;
        self.impulso_pendente += f32::from(velocidade.min(127)) / 127.0;
        self.registro.push(RegistroFalso::Ligada {
            frame: self.frame,
            canal: canal.interno(),
            nota: nota.numero(),
            velocidade,
        });
    }

    fn nota_desligada(&mut self, canal: CanalMidi, nota: NotaMidi) {
        self.ativas[usize::from(canal.interno())][usize::from(nota.numero())] = false;
        self.registro.push(RegistroFalso::Desligada {
            frame: self.frame,
            canal: canal.interno(),
            nota: nota.numero(),
        });
    }

    fn controle(&mut self, canal: CanalMidi, controle: u8, valor: u8) {
        self.registro.push(RegistroFalso::Controle {
            frame: self.frame,
            canal: canal.interno(),
            controle,
            valor,
        });
    }

    fn programa(&mut self, canal: CanalMidi, programa: u8) {
        self.registro.push(RegistroFalso::Programa {
            frame: self.frame,
            canal: canal.interno(),
            programa,
        });
    }

    fn pitch_bend(&mut self, canal: CanalMidi, valor: i16) {
        self.registro.push(RegistroFalso::PitchBend {
            frame: self.frame,
            canal: canal.interno(),
            valor,
        });
    }

    fn todas_notas_desligadas(&mut self) {
        self.ativas = [[false; 128]; 16];
        self.registro
            .push(RegistroFalso::TodasDesligadas { frame: self.frame });
    }

    fn silenciar_tudo(&mut self) {
        self.ativas = [[false; 128]; 16];
        self.impulso_pendente = 0.0;
        self.registro
            .push(RegistroFalso::Silenciar { frame: self.frame });
    }

    fn renderizar(&mut self, esq: &mut [f32], dir: &mut [f32]) {
        let n = esq.len().min(dir.len());
        esq.fill(0.0);
        dir.fill(0.0);
        if n > 0 && self.impulso_pendente != 0.0 {
            esq[0] = self.impulso_pendente;
            dir[0] = self.impulso_pendente;
            self.impulso_pendente = 0.0;
        }
        self.frame += n as u64;
    }
}
