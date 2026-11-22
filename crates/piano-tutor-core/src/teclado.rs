//! Modelo lógico do teclado de 61 teclas, estado de teclas pressionadas, nomes de nota
//! e fonte de eventos de simulação.
//!
//! Código puro: sem UI, sem hardware, sem MIDI real. A faixa do teclado vem de **calibração**
//! (nota MIDI mais grave medida no instrumento). A faixa padrão existe apenas para testes e
//! simulação e é nomeada como tal; nenhum mapeamento de hardware real é presumido.

use crate::NotaMidi;
use std::collections::VecDeque;

/// Quantidade de teclas do instrumento.
pub const TECLAS: usize = 61;

/// Nota MIDI do Dó central na convenção científica (C4 = 60).
pub const DO_CENTRAL_MIDI: u8 = 60;

/// Erros do modelo de teclado.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErroTeclado {
    /// A calibração não comporta 61 teclas dentro de 0..=127.
    CalibracaoInvalida { grave: u8 },
    /// A nota não pertence à faixa calibrada.
    ForaDaFaixa { nota: u8 },
}

/// Cor da tecla no desenho.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorTecla {
    Branca,
    Preta,
}

/// Descrição lógica de uma tecla.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tecla {
    /// Índice 0..=60 da esquerda para a direita.
    pub indice: usize,
    pub nota: NotaMidi,
    pub cor: CorTecla,
}

/// Teclado de 61 teclas com faixa definida por calibração.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Teclado61 {
    grave: u8,
}

impl Teclado61 {
    /// Nota mais grave da faixa **somente para testes e simulação** (Dó2 = 36, faixa 36..=96).
    /// Não representa medição de nenhum instrumento real.
    pub const GRAVE_PADRAO_PARA_TESTES_E_SIMULACAO: u8 = 36;

    /// Cria o teclado a partir da nota MIDI mais grave **medida** na calibração.
    pub fn calibrado(grave: u8) -> Result<Self, ErroTeclado> {
        if usize::from(grave) + TECLAS - 1 > 127 {
            return Err(ErroTeclado::CalibracaoInvalida { grave });
        }
        Ok(Self { grave })
    }

    /// Faixa padrão, apenas para testes e simulação.
    pub const fn padrao_para_testes_e_simulacao() -> Self {
        Self {
            grave: Self::GRAVE_PADRAO_PARA_TESTES_E_SIMULACAO,
        }
    }

    pub const fn nota_mais_grave(self) -> u8 {
        self.grave
    }

    pub const fn nota_mais_aguda(self) -> u8 {
        self.grave + (TECLAS as u8 - 1)
    }

    /// Índice 0..=60 da nota, ou `None` se fora da faixa.
    pub fn indice_de(self, nota: NotaMidi) -> Option<usize> {
        let n = nota.numero();
        if n < self.grave || n > self.nota_mais_aguda() {
            None
        } else {
            Some(usize::from(n - self.grave))
        }
    }

    pub fn nota_de(self, indice: usize) -> Option<NotaMidi> {
        if indice >= TECLAS {
            return None;
        }
        u8::try_from(indice)
            .ok()
            .and_then(|i| NotaMidi::nova(self.grave + i))
    }

    pub fn tecla(self, indice: usize) -> Option<Tecla> {
        let nota = self.nota_de(indice)?;
        Some(Tecla {
            indice,
            nota,
            cor: cor_da_nota(nota),
        })
    }

    /// Todas as teclas em ordem, da mais grave à mais aguda.
    pub fn teclas(self) -> impl Iterator<Item = Tecla> {
        (0..TECLAS).filter_map(move |i| self.tecla(i))
    }

    /// Índice do Dó central (MIDI 60) quando está dentro da faixa calibrada.
    pub fn indice_do_central(self) -> Option<usize> {
        NotaMidi::nova(DO_CENTRAL_MIDI).and_then(|n| self.indice_de(n))
    }

    /// Quantidade de teclas brancas na faixa (base para a geometria do desenho).
    pub fn quantidade_brancas(self) -> usize {
        self.teclas().filter(|t| t.cor == CorTecla::Branca).count()
    }
}

/// Cor da tecla pela classe de altura (independe da calibração).
pub const fn cor_da_nota(nota: NotaMidi) -> CorTecla {
    match nota.numero() % 12 {
        1 | 3 | 6 | 8 | 10 => CorTecla::Preta,
        _ => CorTecla::Branca,
    }
}

/// Estado das teclas pressionadas. Suporta todas as 61 simultâneas.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EstadoTeclas {
    teclado: Teclado61,
    ativas: [bool; TECLAS],
}

impl EstadoTeclas {
    pub const fn novo(teclado: Teclado61) -> Self {
        Self {
            teclado,
            ativas: [false; TECLAS],
        }
    }

    pub const fn teclado(&self) -> Teclado61 {
        self.teclado
    }

    /// Liga a nota. Idempotente. Nota fora da faixa é reportada e o estado não muda.
    pub fn ligar(&mut self, nota: NotaMidi) -> Result<(), ErroTeclado> {
        let i = self.indice(nota)?;
        self.ativas[i] = true;
        Ok(())
    }

    /// Desliga somente a nota indicada.
    pub fn desligar(&mut self, nota: NotaMidi) -> Result<(), ErroTeclado> {
        let i = self.indice(nota)?;
        self.ativas[i] = false;
        Ok(())
    }

    /// Apaga tudo (ex.: desconexão do dispositivo).
    pub fn limpar_tudo(&mut self) {
        self.ativas = [false; TECLAS];
    }

    pub fn esta_ligada(&self, nota: NotaMidi) -> bool {
        self.teclado.indice_de(nota).is_some_and(|i| self.ativas[i])
    }

    pub fn quantidade(&self) -> usize {
        self.ativas.iter().filter(|a| **a).count()
    }

    /// Índices ligados em ordem crescente.
    pub fn indices_ligados(&self) -> impl Iterator<Item = usize> + '_ {
        self.ativas
            .iter()
            .enumerate()
            .filter_map(|(i, a)| a.then_some(i))
    }

    /// Notas ligadas em ordem crescente.
    pub fn notas_ligadas(&self) -> impl Iterator<Item = NotaMidi> + '_ {
        let teclado = self.teclado;
        self.indices_ligados()
            .filter_map(move |i| teclado.nota_de(i))
    }

    /// Aplica um evento; fora da faixa é reportado como erro.
    pub fn aplicar(&mut self, evento: EventoTecla) -> Result<(), ErroTeclado> {
        match evento.acao {
            AcaoTecla::Ligar => self.ligar(evento.nota),
            AcaoTecla::Desligar => self.desligar(evento.nota),
        }
    }

    fn indice(&self, nota: NotaMidi) -> Result<usize, ErroTeclado> {
        self.teclado
            .indice_de(nota)
            .ok_or(ErroTeclado::ForaDaFaixa {
                nota: nota.numero(),
            })
    }
}

/// Convenção de numeração de oitava; afeta só o rótulo, nunca o número MIDI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConvencaoOitava {
    /// MIDI 60 = C4 (padrão científico).
    Do4,
    /// MIDI 60 = C3 (usada por alguns fabricantes/DAWs).
    Do3,
}

impl ConvencaoOitava {
    pub const fn oitava(self, nota: NotaMidi) -> i8 {
        let cientifica = nota.oitava();
        match self {
            Self::Do4 => cientifica,
            Self::Do3 => cientifica - 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Acidente {
    Sustenido,
    Bemol,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Idioma {
    PtBr,
    Internacional,
}

/// Configuração de rotulagem de notas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfigNomes {
    pub idioma: Idioma,
    pub acidente: Acidente,
    pub convencao: ConvencaoOitava,
}

impl Default for ConfigNomes {
    fn default() -> Self {
        Self {
            idioma: Idioma::PtBr,
            acidente: Acidente::Sustenido,
            convencao: ConvencaoOitava::Do4,
        }
    }
}

const PT_SUST: [&str; 12] = [
    "Dó", "Dó♯", "Ré", "Ré♯", "Mi", "Fá", "Fá♯", "Sol", "Sol♯", "Lá", "Lá♯", "Si",
];
const PT_BEMOL: [&str; 12] = [
    "Dó", "Ré♭", "Ré", "Mi♭", "Mi", "Fá", "Sol♭", "Sol", "Lá♭", "Lá", "Si♭", "Si",
];
const EN_SUST: [&str; 12] = [
    "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
];
const EN_BEMOL: [&str; 12] = [
    "C", "D♭", "D", "E♭", "E", "F", "G♭", "G", "A♭", "A", "B♭", "B",
];

/// Nome sem oitava, conforme idioma e acidente.
pub fn nome_sem_oitava(nota: NotaMidi, cfg: ConfigNomes) -> &'static str {
    let tabela = match (cfg.idioma, cfg.acidente) {
        (Idioma::PtBr, Acidente::Sustenido) => &PT_SUST,
        (Idioma::PtBr, Acidente::Bemol) => &PT_BEMOL,
        (Idioma::Internacional, Acidente::Sustenido) => &EN_SUST,
        (Idioma::Internacional, Acidente::Bemol) => &EN_BEMOL,
    };
    tabela[usize::from(nota.numero() % 12)]
}

/// Rótulo completo, por exemplo "Dó4" ou "C3". O número MIDI da nota não muda.
pub fn rotulo(nota: NotaMidi, cfg: ConfigNomes) -> String {
    format!(
        "{}{}",
        nome_sem_oitava(nota, cfg),
        cfg.convencao.oitava(nota)
    )
}

/// Origem declarada de uma fonte de eventos.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrigemEventos {
    Simulacao,
    DispositivoFisico,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcaoTecla {
    Ligar,
    Desligar,
}

/// Evento de tecla com origem explícita.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventoTecla {
    pub origem: OrigemEventos,
    pub acao: AcaoTecla,
    pub nota: NotaMidi,
}

/// Fonte de eventos de teclas, qualquer que seja a origem.
pub trait FonteEventos {
    fn origem(&self) -> OrigemEventos;
    fn proximo_evento(&mut self) -> Option<EventoTecla>;
}

/// Fonte de teste: sempre se identifica como simulação, nunca como dispositivo físico.
#[derive(Clone, Debug, Default)]
pub struct ModoSimulacao {
    fila: VecDeque<EventoTecla>,
}

impl ModoSimulacao {
    pub const AVISO: &'static str = "SIMULAÇÃO: eventos de teste, não vêm de dispositivo físico";

    pub fn novo() -> Self {
        Self::default()
    }

    pub fn pressionar(&mut self, nota: NotaMidi) {
        self.empilhar(AcaoTecla::Ligar, nota);
    }

    pub fn soltar(&mut self, nota: NotaMidi) {
        self.empilhar(AcaoTecla::Desligar, nota);
    }

    pub const fn aviso(&self) -> &'static str {
        Self::AVISO
    }

    fn empilhar(&mut self, acao: AcaoTecla, nota: NotaMidi) {
        self.fila.push_back(EventoTecla {
            origem: OrigemEventos::Simulacao,
            acao,
            nota,
        });
    }
}

impl FonteEventos for ModoSimulacao {
    fn origem(&self) -> OrigemEventos {
        OrigemEventos::Simulacao
    }

    fn proximo_evento(&mut self) -> Option<EventoTecla> {
        self.fila.pop_front()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn n(x: u8) -> NotaMidi {
        NotaMidi::nova(x).expect("nota válida")
    }

    fn estado() -> EstadoTeclas {
        EstadoTeclas::novo(Teclado61::padrao_para_testes_e_simulacao())
    }

    struct Lcg(u64);
    impl Lcg {
        fn proximo(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }
    }

    #[test]
    fn dez_notas_simultaneas_e_soltar_uma() {
        let mut e = estado();
        for x in 48..58 {
            e.ligar(n(x)).expect("na faixa");
        }
        assert_eq!(e.quantidade(), 10);
        e.desligar(n(52)).expect("na faixa");
        assert_eq!(e.quantidade(), 9);
        assert!(!e.esta_ligada(n(52)));
        for x in (48..58).filter(|x| *x != 52) {
            assert!(e.esta_ligada(n(x)), "nota {x} deveria seguir ligada");
        }
    }

    #[test]
    fn todas_as_61_simultaneas() {
        let mut e = estado();
        for i in 0..TECLAS {
            let nota = e.teclado().nota_de(i).expect("índice válido");
            e.ligar(nota).expect("na faixa");
        }
        assert_eq!(e.quantidade(), 61);
    }

    #[test]
    fn sequencias_longas_deterministicas_sem_nota_presa() {
        let t = Teclado61::padrao_para_testes_e_simulacao();
        let mut e = EstadoTeclas::novo(t);
        let mut modelo = [false; 128];
        let mut rng = Lcg(42);
        for _ in 0..200_000 {
            let x = 30 + (rng.proximo() % 76) as u8; // inclui fora da faixa
            let ligar = rng.proximo().is_multiple_of(2);
            let nota = n(x);
            let r = if ligar {
                e.ligar(nota)
            } else {
                e.desligar(nota)
            };
            let dentro = (36..=96).contains(&x);
            assert_eq!(r.is_ok(), dentro);
            if dentro {
                modelo[usize::from(x)] = ligar;
            }
        }
        for x in 0..128u8 {
            assert_eq!(e.esta_ligada(n(x)), modelo[usize::from(x)]);
        }
        for x in 36..=96u8 {
            e.desligar(n(x)).expect("na faixa");
        }
        assert_eq!(e.quantidade(), 0);
    }

    #[test]
    fn limpar_tudo_na_desconexao() {
        let mut e = estado();
        for x in 40..50 {
            e.ligar(n(x)).expect("na faixa");
        }
        e.limpar_tudo();
        assert_eq!(e.quantidade(), 0);
        assert_eq!(e.notas_ligadas().count(), 0);
    }

    #[test]
    fn fora_da_faixa_e_reportada_sem_panico() {
        let mut e = estado();
        assert_eq!(e.ligar(n(10)), Err(ErroTeclado::ForaDaFaixa { nota: 10 }));
        assert_eq!(
            e.desligar(n(127)),
            Err(ErroTeclado::ForaDaFaixa { nota: 127 })
        );
        assert_eq!(e.quantidade(), 0);
    }

    #[test]
    fn calibracao_define_faixa_e_rejeita_invalida() {
        let t = Teclado61::calibrado(41).expect("válida");
        assert_eq!((t.nota_mais_grave(), t.nota_mais_aguda()), (41, 101));
        assert_eq!(t.indice_de(n(41)), Some(0));
        assert_eq!(t.indice_de(n(101)), Some(60));
        assert_eq!(t.indice_de(n(102)), None);
        assert_eq!(t.indice_de(n(40)), None);
        assert!(Teclado61::calibrado(67).is_ok());
        assert_eq!(
            Teclado61::calibrado(68),
            Err(ErroTeclado::CalibracaoInvalida { grave: 68 })
        );
    }

    #[test]
    fn do_central_depende_da_faixa() {
        let t = Teclado61::padrao_para_testes_e_simulacao();
        assert_eq!(t.indice_do_central(), Some(24));
        let alto = Teclado61::calibrado(65).expect("válida");
        assert_eq!(alto.indice_do_central(), None);
    }

    #[test]
    fn geometria_branca_preta() {
        let t = Teclado61::padrao_para_testes_e_simulacao();
        // Dó2..Dó7: 5 oitavas completas (35 brancas, 25 pretas) + Dó final branca.
        assert_eq!(t.quantidade_brancas(), 36);
        assert_eq!(t.teclas().count(), 61);
        let pretas = t.teclas().filter(|k| k.cor == CorTecla::Preta).count();
        assert_eq!(pretas, 25);
        assert_eq!(cor_da_nota(n(61)), CorTecla::Preta);
        assert_eq!(cor_da_nota(n(64)), CorTecla::Branca);
        assert_eq!(cor_da_nota(n(65)), CorTecla::Branca);
        assert_eq!(t.tecla(61), None);
    }

    #[test]
    fn nomes_pt_e_internacionais() {
        let pt = ConfigNomes::default();
        assert_eq!(rotulo(n(60), pt), "Dó4");
        assert_eq!(rotulo(n(61), pt), "Dó♯4");
        let bemol = ConfigNomes {
            acidente: Acidente::Bemol,
            ..pt
        };
        assert_eq!(rotulo(n(61), bemol), "Ré♭4");
        let en = ConfigNomes {
            idioma: Idioma::Internacional,
            ..pt
        };
        assert_eq!(rotulo(n(69), en), "A4");
        let en_bemol = ConfigNomes {
            acidente: Acidente::Bemol,
            ..en
        };
        assert_eq!(rotulo(n(70), en_bemol), "B♭4");
    }

    #[test]
    fn troca_de_convencao_nao_muda_numero_midi() {
        let nota = n(60);
        let c4 = ConfigNomes::default();
        let c3 = ConfigNomes {
            convencao: ConvencaoOitava::Do3,
            ..c4
        };
        assert_eq!(rotulo(nota, c4), "Dó4");
        assert_eq!(rotulo(nota, c3), "Dó3");
        assert_eq!(nota.numero(), 60);
    }

    #[test]
    fn frequencias() {
        let la = n(69).frequencia_hz(440.0).expect("válida");
        assert!((la - 440.0).abs() < 1e-9);
        let c = n(60).frequencia_hz(440.0).expect("válida");
        assert!((c - 261.625_565_3).abs() < 1e-4);
    }

    #[test]
    fn simulacao_se_identifica_e_alimenta_estado() {
        let mut sim = ModoSimulacao::novo();
        assert_eq!(sim.origem(), OrigemEventos::Simulacao);
        assert_ne!(sim.origem(), OrigemEventos::DispositivoFisico);
        assert!(sim.aviso().contains("SIMULAÇÃO"));
        sim.pressionar(n(60));
        sim.pressionar(n(64));
        sim.soltar(n(60));
        let mut e = estado();
        while let Some(ev) = sim.proximo_evento() {
            assert_eq!(ev.origem, OrigemEventos::Simulacao);
            e.aplicar(ev).expect("na faixa");
        }
        assert!(!e.esta_ligada(n(60)));
        assert!(e.esta_ligada(n(64)));
        assert_eq!(e.quantidade(), 1);
    }
}
