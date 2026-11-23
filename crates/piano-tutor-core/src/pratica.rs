//! Lógica pura do modo "Praticar" da partitura (F07): confere as notas que o aluno toca contra
//! os eventos esperados, em dois modos.
//!
//! - [`ModoPratica::Espera`]: o cursor fica parado num passo (um instante com uma ou mais notas)
//!   até o aluno tocar todas as notas dele, em qualquer ordem. Nota errada marca as pendentes do
//!   passo de vermelho e conta como extra; o passo só avança quando as certas são tocadas. O
//!   tempo não importa.
//! - [`ModoPratica::Tempo`]: o relógio anda no andamento escolhido; cada nota esperada tem uma
//!   janela de tolerância (ms) em torno do seu instante, e o pareamento é o do módulo
//!   [`crate::comparador`] (ótimo por altura). Esperada sem par depois do fim da janela vira
//!   falta (vermelha); tocada sem par é extra.
//!
//! Sem relógio, sem UI, sem hardware: os instantes entram como segundos desde o início da
//! prática (podem ser negativos, para a contagem de entrada). Nada aqui foi ligado a um teclado
//! MIDI real: **NÃO VERIFICADO** fora dos testes.

use crate::comparador::{
    ConfigComparacao, EventoEsperado, NotaTocada, TOLERANCIA_MAXIMA_MS, comparar,
};

/// Como a prática avança.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModoPratica {
    Espera,
    Tempo,
}

impl ModoPratica {
    pub const TODOS: [Self; 2] = [Self::Espera, Self::Tempo];

    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Espera => "Esperar a nota certa",
            Self::Tempo => "Seguir o andamento",
        }
    }
}

/// Configuração da prática.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConfigPratica {
    pub modo: ModoPratica,
    /// Janela de acerto no modo Tempo, em ms (limitada a `0..=TOLERANCIA_MAXIMA_MS`).
    pub tolerancia_ms: f64,
}

impl Default for ConfigPratica {
    fn default() -> Self {
        Self {
            modo: ModoPratica::Espera,
            tolerancia_ms: 150.0,
        }
    }
}

/// Situação de uma nota esperada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EstadoEvento {
    /// Ainda não tocada e ainda dentro do prazo.
    Pendente,
    Acertou,
    /// Tocada errada no passo (Espera) ou não tocada na janela (Tempo).
    Errou,
}

/// O que aconteceu com uma nota tocada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Veredito {
    Acerto,
    /// Altura errada, fora da janela ou repetida.
    Erro,
    /// A prática já terminou (ou não há nada esperado).
    Ignorada,
}

/// Resumo ao fim (ou durante) a prática.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resumo {
    pub esperadas: usize,
    pub acertos: usize,
    /// Esperadas que não foram tocadas (só contadas com a prática encerrada, ou, no modo Tempo,
    /// com a janela já vencida).
    pub faltas: usize,
    /// Notas tocadas sem correspondência (altura errada, fora da janela ou repetidas).
    pub extras: usize,
    /// Desvio médio com sinal dos acertos, em ms (negativo = adiantado). Só no modo Tempo.
    pub desvio_medio_ms: Option<f64>,
    /// Desvio absoluto médio dos acertos, em ms. Só no modo Tempo.
    pub desvio_absoluto_ms: Option<f64>,
}

impl Resumo {
    /// Percentual de esperadas acertadas (100 se não havia nada esperado).
    pub fn precisao_pct(&self) -> f64 {
        if self.esperadas == 0 {
            100.0
        } else {
            100.0 * self.acertos as f64 / self.esperadas as f64
        }
    }
}

/// Sessão de prática sobre uma lista de notas esperadas.
#[derive(Clone, Debug)]
pub struct Pratica {
    config: ConfigPratica,
    esperados: Vec<EventoEsperado>,
    /// Índices (em `esperados`) agrupados por instante, em ordem de tempo.
    passos: Vec<Vec<usize>>,
    estados: Vec<EstadoEvento>,
    encerrada: bool,
    // Espera
    passo_atual: usize,
    extras_espera: usize,
    // Tempo
    tocadas: Vec<NotaTocada>,
    agora_s: f64,
    proximo_prazo_s: f64,
    extras: Vec<usize>,
    desvios_ms: Vec<f64>,
}

impl Pratica {
    /// Cria a sessão. `esperados` pode estar em qualquer ordem; os índices dos estados seguem a
    /// ordem recebida. Instantes não finitos são tratados como zero.
    pub fn nova(esperados: &[EventoEsperado], config: ConfigPratica) -> Self {
        let mut esperados = esperados.to_vec();
        for e in &mut esperados {
            if !e.inicio_s.is_finite() {
                e.inicio_s = 0.0;
            }
        }
        let tolerancia_ms = if config.tolerancia_ms.is_finite() {
            config.tolerancia_ms.clamp(0.0, TOLERANCIA_MAXIMA_MS)
        } else {
            ConfigPratica::default().tolerancia_ms
        };
        let config = ConfigPratica {
            tolerancia_ms,
            ..config
        };
        let mut ordem: Vec<usize> = (0..esperados.len()).collect();
        ordem.sort_by(|&a, &b| {
            esperados[a]
                .inicio_s
                .total_cmp(&esperados[b].inicio_s)
                .then(esperados[a].midi.cmp(&esperados[b].midi))
                .then(a.cmp(&b))
        });
        let mut passos: Vec<Vec<usize>> = Vec::new();
        let mut t_passo = f64::NAN;
        for i in ordem {
            let t = esperados[i].inicio_s;
            match passos.last_mut() {
                Some(p) if (t - t_passo).abs() < 1e-6 => p.push(i),
                _ => {
                    passos.push(vec![i]);
                    t_passo = t;
                }
            }
        }
        let n = esperados.len();
        let mut s = Self {
            config,
            esperados,
            passos,
            estados: vec![EstadoEvento::Pendente; n],
            encerrada: false,
            passo_atual: 0,
            extras_espera: 0,
            tocadas: Vec::new(),
            agora_s: f64::NEG_INFINITY,
            proximo_prazo_s: f64::INFINITY,
            extras: Vec::new(),
            desvios_ms: Vec::new(),
        };
        s.recalcular_prazo();
        s
    }

    pub fn config(&self) -> ConfigPratica {
        self.config
    }

    pub fn esperados(&self) -> &[EventoEsperado] {
        &self.esperados
    }

    /// Estado de cada nota esperada, na ordem de entrada.
    pub fn estados(&self) -> &[EstadoEvento] {
        &self.estados
    }

    /// Índice (em `esperados`) da primeira nota ainda não acertada do passo atual, para posicionar o cursor no modo
    /// Espera; `None` se terminou ou não há notas.
    pub fn evento_do_passo_atual(&self) -> Option<usize> {
        if self.encerrada {
            return None;
        }
        self.passos.get(self.passo_atual).and_then(|p| {
            p.iter()
                .copied()
                .find(|&i| self.estados[i] != EstadoEvento::Acertou)
        })
    }

    /// Terminou: tudo foi tocado (Espera), todos os prazos venceram (Tempo) ou foi encerrada.
    pub fn concluida(&self) -> bool {
        if self.encerrada {
            return true;
        }
        match self.config.modo {
            ModoPratica::Espera => self.passo_atual >= self.passos.len(),
            ModoPratica::Tempo => self.estados.iter().all(|e| *e != EstadoEvento::Pendente),
        }
    }

    /// Encerra a prática: o que não foi tocado vira falta.
    pub fn encerrar(&mut self) {
        if self.encerrada {
            return;
        }
        self.encerrada = true;
        match self.config.modo {
            ModoPratica::Espera => {
                for e in &mut self.estados {
                    if *e != EstadoEvento::Acertou {
                        *e = EstadoEvento::Errou;
                    }
                }
            }
            ModoPratica::Tempo => self.recalcular(),
        }
    }

    /// Registra um note-on em `t_s` segundos desde o início da prática.
    pub fn nota_tocada(&mut self, midi: u8, t_s: f64) -> Veredito {
        if self.encerrada || self.esperados.is_empty() || !t_s.is_finite() {
            return Veredito::Ignorada;
        }
        match self.config.modo {
            ModoPratica::Espera => self.tocada_espera(midi),
            ModoPratica::Tempo => self.tocada_tempo(midi, t_s),
        }
    }

    /// Faz o tempo andar até `agora_s` (modo Tempo). Devolve `true` se algum estado mudou.
    pub fn avancar_tempo(&mut self, agora_s: f64) -> bool {
        if self.config.modo != ModoPratica::Tempo || self.encerrada || !agora_s.is_finite() {
            return false;
        }
        if agora_s > self.agora_s {
            self.agora_s = agora_s;
        }
        if self.agora_s > self.proximo_prazo_s {
            let antes = self.estados.clone();
            self.recalcular();
            return antes != self.estados;
        }
        false
    }

    /// Resumo da sessão neste momento.
    pub fn resumo(&self) -> Resumo {
        let acertos = self
            .estados
            .iter()
            .filter(|e| **e == EstadoEvento::Acertou)
            .count();
        let faltas = match (self.config.modo, self.encerrada) {
            (_, true) => self.estados.len() - acertos,
            (ModoPratica::Tempo, false) => self
                .estados
                .iter()
                .filter(|e| **e == EstadoEvento::Errou)
                .count(),
            (ModoPratica::Espera, false) => 0,
        };
        let extras = match self.config.modo {
            ModoPratica::Espera => self.extras_espera,
            ModoPratica::Tempo => self.extras.len(),
        };
        let (desvio_medio_ms, desvio_absoluto_ms) = if self.desvios_ms.is_empty() {
            (None, None)
        } else {
            let n = self.desvios_ms.len() as f64;
            (
                Some(self.desvios_ms.iter().sum::<f64>() / n),
                Some(self.desvios_ms.iter().map(|d| d.abs()).sum::<f64>() / n),
            )
        };
        Resumo {
            esperadas: self.estados.len(),
            acertos,
            faltas,
            extras,
            desvio_medio_ms,
            desvio_absoluto_ms,
        }
    }

    // --- Espera ----------------------------------------------------------------------------

    fn tocada_espera(&mut self, midi: u8) -> Veredito {
        let Some(passo) = self.passos.get(self.passo_atual) else {
            return Veredito::Ignorada;
        };
        let achado = passo
            .iter()
            .copied()
            .find(|&i| self.esperados[i].midi == midi && self.estados[i] != EstadoEvento::Acertou);
        if let Some(i) = achado {
            self.estados[i] = EstadoEvento::Acertou;
            if passo
                .iter()
                .all(|&j| self.estados[j] == EstadoEvento::Acertou)
            {
                self.passo_atual += 1;
            }
            Veredito::Acerto
        } else {
            self.extras_espera += 1;
            for &i in &self.passos[self.passo_atual] {
                if self.estados[i] == EstadoEvento::Pendente {
                    self.estados[i] = EstadoEvento::Errou;
                }
            }
            Veredito::Erro
        }
    }

    // --- Tempo -----------------------------------------------------------------------------

    fn tocada_tempo(&mut self, midi: u8, t_s: f64) -> Veredito {
        if t_s > self.agora_s {
            self.agora_s = t_s;
        }
        self.tocadas.push(NotaTocada {
            midi,
            inicio_s: t_s,
        });
        let nova = self.tocadas.len() - 1;
        self.recalcular();
        if self.extras.contains(&nova) {
            Veredito::Erro
        } else {
            Veredito::Acerto
        }
    }

    fn recalcular(&mut self) {
        let cfg = ConfigComparacao {
            tolerancia_ms: self.config.tolerancia_ms,
            compensacao_ms: 0.0,
        };
        let c = comparar(&self.esperados, &self.tocadas, cfg).unwrap_or_default();
        let tol_s = self.config.tolerancia_ms / 1000.0;
        let mut casado = vec![false; self.esperados.len()];
        for a in &c.acertos {
            casado[a.esperado] = true;
        }
        for (i, e) in self.esperados.iter().enumerate() {
            self.estados[i] = if casado[i] {
                EstadoEvento::Acertou
            } else if self.encerrada || e.inicio_s + tol_s < self.agora_s {
                EstadoEvento::Errou
            } else {
                EstadoEvento::Pendente
            };
        }
        self.desvios_ms = c.acertos.iter().map(|a| a.desvio_ms).collect();
        self.extras = c.extras;
        self.recalcular_prazo();
    }

    fn recalcular_prazo(&mut self) {
        let tol_s = self.config.tolerancia_ms / 1000.0;
        self.proximo_prazo_s = self
            .esperados
            .iter()
            .zip(&self.estados)
            .filter(|(_, s)| **s == EstadoEvento::Pendente)
            .map(|(e, _)| e.inicio_s + tol_s)
            .fold(f64::INFINITY, f64::min);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn esp(midi: u8, t: f64) -> EventoEsperado {
        EventoEsperado {
            midi,
            inicio_s: t,
            duracao_s: 0.4,
            parte: 0,
            pauta: 1,
            compasso: 0,
        }
    }

    fn cfg(modo: ModoPratica, ms: f64) -> ConfigPratica {
        ConfigPratica {
            modo,
            tolerancia_ms: ms,
        }
    }

    #[test]
    fn espera_avanca_so_com_a_nota_certa_e_conta_extras() {
        let e = [esp(60, 0.0), esp(62, 0.5), esp(64, 1.0)];
        let mut p = Pratica::nova(&e, cfg(ModoPratica::Espera, 0.0));
        assert_eq!(p.evento_do_passo_atual(), Some(0));
        assert_eq!(p.nota_nota(61), Veredito::Erro);
        assert_eq!(p.estados()[0], EstadoEvento::Errou);
        assert_eq!(p.evento_do_passo_atual(), Some(0), "cursor não avança");
        assert_eq!(p.nota_nota(60), Veredito::Acerto);
        assert_eq!(p.estados()[0], EstadoEvento::Acertou, "vermelha vira verde");
        assert_eq!(p.evento_do_passo_atual(), Some(1));
        // Tocar a nota do passo seguinte antes da hora é erro.
        assert_eq!(p.nota_nota(64), Veredito::Erro);
        assert_eq!(p.nota_nota(62), Veredito::Acerto);
        assert!(!p.concluida());
        assert_eq!(p.nota_nota(64), Veredito::Acerto);
        assert!(p.concluida());
        assert_eq!(p.evento_do_passo_atual(), None);
        assert_eq!(p.nota_nota(60), Veredito::Ignorada);
        let r = p.resumo();
        assert_eq!((r.esperadas, r.acertos, r.faltas, r.extras), (3, 3, 0, 2));
        assert_eq!(r.precisao_pct(), 100.0);
        assert_eq!(r.desvio_medio_ms, None);
    }

    impl Pratica {
        /// Atalho de teste: o tempo não importa no modo Espera.
        fn nota_nota(&mut self, midi: u8) -> Veredito {
            self.nota_tocada(midi, 0.0)
        }
    }

    #[test]
    fn espera_acorde_aceita_qualquer_ordem_e_repeticao_e_extra() {
        let e = [esp(64, 0.0), esp(48, 0.0), esp(67, 0.0), esp(60, 1.0)];
        let mut p = Pratica::nova(&e, cfg(ModoPratica::Espera, 0.0));
        assert_eq!(p.nota_nota(67), Veredito::Acerto);
        assert_eq!(p.nota_nota(67), Veredito::Erro, "repetida");
        assert_eq!(p.nota_nota(48), Veredito::Acerto);
        assert!(p.evento_do_passo_atual().is_some_and(|i| e[i].midi == 64));
        assert_eq!(p.nota_nota(64), Veredito::Acerto);
        assert_eq!(p.evento_do_passo_atual(), Some(3));
        assert_eq!(p.nota_nota(60), Veredito::Acerto);
        assert!(p.concluida());
        assert_eq!(p.resumo().extras, 1);
    }

    #[test]
    fn espera_encerrar_conta_o_que_faltou() {
        let e = [esp(60, 0.0), esp(62, 0.5), esp(64, 1.0)];
        let mut p = Pratica::nova(&e, cfg(ModoPratica::Espera, 0.0));
        assert_eq!(p.nota_nota(60), Veredito::Acerto);
        assert_eq!(p.resumo().faltas, 0, "ainda não alcançadas");
        p.encerrar();
        assert!(p.concluida());
        let r = p.resumo();
        assert_eq!((r.acertos, r.faltas, r.extras), (1, 2, 0));
        assert!((r.precisao_pct() - 100.0 / 3.0).abs() < 1e-9);
        assert_eq!(p.estados()[2], EstadoEvento::Errou);
        assert_eq!(p.nota_nota(62), Veredito::Ignorada);
    }

    #[test]
    fn tempo_acerta_dentro_da_janela_e_mede_o_desvio() {
        let e = [esp(60, 0.0), esp(62, 0.5), esp(64, 1.0)];
        let mut p = Pratica::nova(&e, cfg(ModoPratica::Tempo, 100.0));
        assert_eq!(p.nota_tocada(60, 0.02), Veredito::Acerto);
        assert_eq!(p.nota_tocada(62, 0.45), Veredito::Acerto);
        assert_eq!(p.nota_tocada(64, 1.0), Veredito::Acerto);
        assert!(p.concluida());
        let r = p.resumo();
        assert_eq!((r.esperadas, r.acertos, r.faltas, r.extras), (3, 3, 0, 0));
        assert!((r.desvio_medio_ms.unwrap_or(99.0) - (20.0 - 50.0) / 3.0).abs() < 1e-6);
        assert!((r.desvio_absoluto_ms.unwrap_or(0.0) - 70.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn tempo_vence_o_prazo_vira_falta_e_nota_errada_vira_extra() {
        let e = [esp(60, 0.0), esp(62, 0.5)];
        let mut p = Pratica::nova(&e, cfg(ModoPratica::Tempo, 100.0));
        assert!(!p.avancar_tempo(0.05));
        assert_eq!(p.estados(), [EstadoEvento::Pendente; 2]);
        assert_eq!(p.nota_tocada(61, 0.05), Veredito::Erro);
        assert!(p.avancar_tempo(0.2), "a janela de 60 venceu");
        assert_eq!(p.estados()[0], EstadoEvento::Errou);
        assert_eq!(p.estados()[1], EstadoEvento::Pendente);
        assert!(!p.concluida());
        // Atrasada além da tolerância: extra, e a esperada já era falta.
        assert_eq!(p.nota_tocada(62, 0.75), Veredito::Erro);
        assert!(p.concluida());
        let r = p.resumo();
        assert_eq!((r.acertos, r.faltas, r.extras), (0, 2, 2));
        assert_eq!(r.desvio_medio_ms, None);
        assert_eq!(r.precisao_pct(), 0.0);
    }

    #[test]
    fn tempo_aceita_contagem_de_entrada_negativa_e_encerrar_fecha_pendentes() {
        let e = [esp(60, 0.0), esp(62, 2.0)];
        let mut p = Pratica::nova(&e, cfg(ModoPratica::Tempo, 150.0));
        assert_eq!(p.nota_tocada(60, -0.1), Veredito::Acerto);
        assert!(!p.concluida());
        p.encerrar();
        assert!(p.concluida());
        assert_eq!(p.estados()[1], EstadoEvento::Errou);
        assert_eq!(p.resumo().faltas, 1);
        assert!(!p.avancar_tempo(10.0));
    }

    #[test]
    fn tempo_so_recalcula_ao_cruzar_um_prazo() {
        let e = [esp(60, 1.0)];
        let mut p = Pratica::nova(&e, cfg(ModoPratica::Tempo, 100.0));
        assert!(!p.avancar_tempo(0.5));
        assert!(!p.avancar_tempo(1.09));
        assert!(p.avancar_tempo(1.11));
        assert!(!p.avancar_tempo(5.0), "nada mais a mudar");
        // O tempo nunca anda para trás.
        assert!(!p.avancar_tempo(0.0));
        assert_eq!(p.resumo().faltas, 1);
    }

    #[test]
    fn entradas_degeneradas_nao_quebram() {
        let mut p = Pratica::nova(&[], ConfigPratica::default());
        assert!(p.concluida());
        assert_eq!(p.nota_tocada(60, 0.0), Veredito::Ignorada);
        assert_eq!(p.resumo().precisao_pct(), 100.0);
        assert_eq!(p.evento_do_passo_atual(), None);
        let e = [esp(60, f64::NAN)];
        let mut p = Pratica::nova(&e, cfg(ModoPratica::Tempo, f64::NAN));
        assert_eq!(p.config().tolerancia_ms, 150.0);
        assert_eq!(p.nota_tocada(60, f64::NAN), Veredito::Ignorada);
        assert_eq!(p.nota_tocada(60, 0.0), Veredito::Acerto);
        let p = Pratica::nova(&e, cfg(ModoPratica::Tempo, 1e9));
        assert_eq!(p.config().tolerancia_ms, TOLERANCIA_MAXIMA_MS);
        let p = Pratica::nova(&e, cfg(ModoPratica::Tempo, -5.0));
        assert_eq!(p.config().tolerancia_ms, 0.0);
    }

    #[test]
    fn entrada_fora_de_ordem_mantem_indices_originais() {
        let e = [esp(64, 1.0), esp(60, 0.0)];
        let mut p = Pratica::nova(&e, cfg(ModoPratica::Espera, 0.0));
        assert_eq!(p.evento_do_passo_atual(), Some(1));
        assert_eq!(p.nota_nota(60), Veredito::Acerto);
        assert_eq!(p.estados(), [EstadoEvento::Pendente, EstadoEvento::Acertou]);
        assert_eq!(p.evento_do_passo_atual(), Some(0));
    }
}
