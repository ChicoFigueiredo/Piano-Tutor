//! Acordes e escalas (F04 aplicado; base do exercício de F05): dicionário com teclas-guia,
//! escalas, campo harmônico e o desafio "toque o acorde" (também de ouvido).

use piano_tutor_core::NotaMidi;
use piano_tutor_core::teoria::{
    Acorde, ClasseAltura, Qualidade, TipoEscala, campo_harmonico_maior,
};

/// Aba da tela.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Modo {
    Dicionario,
    Escalas,
    Desafio,
}

/// Que acordes o desafio sorteia.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Conjunto {
    /// Tríades maiores e menores nas 12 tonalidades.
    MaioresEMenores,
    /// Campo harmônico maior da tonalidade escolhida.
    CampoHarmonico,
    /// Acordes com sétima (7, 7M, m7).
    ComSetima,
    /// Todas as qualidades.
    Todos,
}

impl Conjunto {
    pub const TODOS: [Conjunto; 4] = [
        Conjunto::MaioresEMenores,
        Conjunto::CampoHarmonico,
        Conjunto::ComSetima,
        Conjunto::Todos,
    ];

    pub const fn texto(self) -> &'static str {
        match self {
            Conjunto::MaioresEMenores => "maiores e menores",
            Conjunto::CampoHarmonico => "campo harmônico da tonalidade",
            Conjunto::ComSetima => "acordes com sétima",
            Conjunto::Todos => "todos os tipos",
        }
    }
}

/// Resultado do último acorde do desafio.
#[derive(Clone, Debug, PartialEq)]
pub struct Resultado {
    pub alvo: Acorde,
    pub segundos: f64,
}

/// Estado da tela de acordes.
#[derive(Clone, Debug)]
pub struct EstadoAcordes {
    pub modo: Modo,
    pub fundamental: ClasseAltura,
    pub qualidade: Qualidade,
    pub inversao: usize,
    /// Nota-base da oitava onde o acorde/escala é mostrado.
    pub base: u8,
    pub escala: TipoEscala,
    pub conjunto: Conjunto,
    pub de_ouvido: bool,
    pub mostrar_dica: bool,
    pub alvo: Option<Acorde>,
    pub alvo_desde_ns: u64,
    pub acertos: u32,
    pub pulados: u32,
    pub ultimo: Option<Resultado>,
    /// Depois de um acerto, espera soltar todas as teclas para sortear o próximo.
    pub aguardando_soltar: bool,
    semente: u64,
}

impl Default for EstadoAcordes {
    fn default() -> Self {
        Self {
            modo: Modo::Dicionario,
            fundamental: ClasseAltura::nova(0),
            qualidade: Qualidade::Maior,
            inversao: 0,
            base: 60,
            escala: TipoEscala::Maior,
            conjunto: Conjunto::MaioresEMenores,
            de_ouvido: false,
            mostrar_dica: false,
            alvo: None,
            alvo_desde_ns: 0,
            acertos: 0,
            pulados: 0,
            ultimo: None,
            aguardando_soltar: false,
            semente: 0x9E37_79B9_7F4A_7C15,
        }
    }
}

fn mascara(notas: impl IntoIterator<Item = NotaMidi>) -> u16 {
    notas
        .into_iter()
        .fold(0u16, |m, n| m | 1 << (n.numero() % 12))
}

impl EstadoAcordes {
    /// Acorde selecionado no dicionário.
    pub fn acorde(&self) -> Acorde {
        Acorde::novo(self.fundamental, self.qualidade)
    }

    fn nota_base(&self) -> NotaMidi {
        NotaMidi::nova(self.base.min(115)).unwrap_or(NotaMidi::nova(60).expect("60"))
    }

    /// Notas do acorde selecionado (com a inversão).
    pub fn notas_do_acorde(&self) -> Vec<NotaMidi> {
        self.acorde().notas(self.nota_base(), self.inversao)
    }

    /// Notas da escala selecionada (uma oitava a partir da tônica).
    pub fn notas_da_escala(&self) -> Vec<NotaMidi> {
        let b = self.nota_base().numero() + ClasseAltura::nova(self.base).ate(self.fundamental);
        NotaMidi::nova(b)
            .map(|n| self.escala.notas(n))
            .unwrap_or_default()
    }

    /// Teclas-guia a mostrar no teclado agora.
    pub fn guias(&self) -> Vec<NotaMidi> {
        match self.modo {
            Modo::Dicionario => self.notas_do_acorde(),
            Modo::Escalas => self.notas_da_escala(),
            Modo::Desafio => match self.alvo {
                Some(a) if self.mostrar_dica => a.notas(self.nota_base(), 0),
                _ => Vec::new(),
            },
        }
    }

    fn aleatorio(&mut self) -> u64 {
        // xorshift64*: suficiente para sortear exercícios.
        let mut x = self.semente;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.semente = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Mistura a semente (chamar com o relógio ao abrir a tela).
    pub fn semear(&mut self, s: u64) {
        self.semente ^= s | 1;
    }

    fn candidatos(&self) -> Vec<Acorde> {
        let todas = |qs: &[Qualidade]| -> Vec<Acorde> {
            (0..12u8)
                .flat_map(|r| {
                    qs.iter()
                        .map(move |q| Acorde::novo(ClasseAltura::nova(r), *q))
                })
                .collect()
        };
        match self.conjunto {
            Conjunto::MaioresEMenores => todas(&[Qualidade::Maior, Qualidade::Menor]),
            Conjunto::CampoHarmonico => campo_harmonico_maior(self.fundamental).to_vec(),
            Conjunto::ComSetima => todas(&[
                Qualidade::SetimaDominante,
                Qualidade::SetimaMaior,
                Qualidade::MenorSetima,
            ]),
            Conjunto::Todos => todas(&Qualidade::TODAS),
        }
    }

    /// Sorteia o próximo acorde (diferente do atual quando possível).
    pub fn sortear(&mut self, agora_ns: u64) -> Acorde {
        let c = self.candidatos();
        let mut a = c[(self.aleatorio() % c.len() as u64) as usize];
        if c.len() > 1 && Some(a) == self.alvo {
            a = c[(self.aleatorio() % c.len() as u64) as usize];
        }
        self.alvo = Some(a);
        self.alvo_desde_ns = agora_ns;
        self.aguardando_soltar = false;
        a
    }

    /// Notas para tocar o alvo de exemplo (ouvir).
    pub fn notas_do_alvo(&self) -> Vec<NotaMidi> {
        self.alvo
            .map(|a| a.notas(self.nota_base(), 0))
            .unwrap_or_default()
    }

    /// Confere as teclas seguradas contra o alvo. Aceita qualquer inversão e dobras de
    /// oitava; exige todas as classes do acorde e nenhuma a mais. Devolve `true` no acerto.
    pub fn conferir(&mut self, seguradas: &[NotaMidi], agora_ns: u64) -> bool {
        if self.modo != Modo::Desafio {
            return false;
        }
        if self.aguardando_soltar {
            if seguradas.is_empty() {
                self.sortear(agora_ns);
            }
            return false;
        }
        let Some(alvo) = self.alvo else {
            return false;
        };
        let m_alvo = mascara(alvo.notas(self.nota_base(), 0));
        if seguradas.len() >= 2 && mascara(seguradas.iter().copied()) == m_alvo {
            self.acertos += 1;
            self.ultimo = Some(Resultado {
                alvo,
                segundos: agora_ns.saturating_sub(self.alvo_desde_ns) as f64 / 1e9,
            });
            self.aguardando_soltar = true;
            return true;
        }
        false
    }

    /// Pula o alvo atual.
    pub fn pular(&mut self, agora_ns: u64) -> Acorde {
        self.pulados += 1;
        self.sortear(agora_ns)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn n(x: u8) -> NotaMidi {
        NotaMidi::nova(x).expect("nota")
    }

    #[test]
    fn dicionario_da_as_guias_do_acorde_com_inversao() {
        let mut e = EstadoAcordes {
            fundamental: ClasseAltura::nova(9),
            qualidade: Qualidade::Menor,
            base: 60,
            ..EstadoAcordes::default()
        };
        assert_eq!(
            e.guias().iter().map(|x| x.numero()).collect::<Vec<_>>(),
            vec![69, 72, 76]
        );
        e.inversao = 1;
        assert_eq!(
            e.guias().iter().map(|x| x.numero()).collect::<Vec<_>>(),
            vec![72, 76, 81]
        );
    }

    #[test]
    fn escala_comeca_na_tonica_escolhida() {
        let e = EstadoAcordes {
            modo: Modo::Escalas,
            fundamental: ClasseAltura::nova(7),
            base: 60,
            ..EstadoAcordes::default()
        };
        assert_eq!(
            e.guias().iter().map(|x| x.numero()).collect::<Vec<_>>(),
            vec![67, 69, 71, 72, 74, 76, 78, 79]
        );
    }

    #[test]
    fn desafio_aceita_qualquer_inversao_e_espera_soltar() {
        let mut e = EstadoAcordes {
            modo: Modo::Desafio,
            ..EstadoAcordes::default()
        };
        e.alvo = Some(Acorde::novo(ClasseAltura::nova(0), Qualidade::Maior));
        assert!(!e.conferir(&[n(60), n(64)], 0), "incompleto");
        assert!(!e.conferir(&[n(60), n(63), n(67)], 0), "menor não vale");
        assert!(!e.conferir(&[n(60), n(64), n(67), n(70)], 0), "nota a mais");
        assert!(
            e.conferir(&[n(52), n(55), n(60)], 2_000_000_000),
            "C/E vale"
        );
        assert_eq!(e.acertos, 1);
        assert_eq!(e.ultimo.as_ref().map(|r| r.segundos), Some(2.0));
        assert!(!e.conferir(&[n(52), n(55), n(60)], 0), "esperando soltar");
        assert!(!e.conferir(&[], 0));
        assert!(!e.aguardando_soltar);
        assert!(e.alvo.is_some());
    }

    #[test]
    fn sorteio_respeita_o_conjunto() {
        let mut e = EstadoAcordes {
            modo: Modo::Desafio,
            conjunto: Conjunto::CampoHarmonico,
            fundamental: ClasseAltura::nova(2),
            ..EstadoAcordes::default()
        };
        let campo = campo_harmonico_maior(ClasseAltura::nova(2));
        for i in 0..50 {
            let a = e.sortear(i);
            assert!(campo.contains(&a), "{}", a.cifra());
        }
        e.conjunto = Conjunto::ComSetima;
        for i in 0..50 {
            let a = e.sortear(i);
            assert!(a.qualidade.intervalos().len() == 4);
        }
    }
}
