//! Lógica pura do modo "Praticar" da partitura (F07) e do Player de Aula: confere as notas que o
//! aluno toca contra os eventos esperados, em três modos.
//!
//! - [`ModoPratica::NotaANota`] (antes "Espera"): o cursor fica parado num passo (um instante com uma ou mais notas)
//!   até o aluno tocar todas as notas dele, em qualquer ordem. Nota errada marca as pendentes do
//!   passo de vermelho e conta como extra; o passo só avança quando as certas são tocadas. O
//!   tempo não importa.
//! - [`ModoPratica::Tempo`]: o relógio anda no andamento escolhido; cada nota esperada tem uma
//!   janela de tolerância (ms) em torno do seu instante, e o pareamento é o do módulo
//!   [`crate::comparador`] (ótimo por altura). Esperada sem par depois do fim da janela vira
//!   falta (vermelha); tocada sem par é extra.
//! - [`ModoPratica::EsperaTocar`]: o relógio anda no andamento e cada passo tem a mesma janela
//!   de tolerância; se a janela de um passo vence sem todas as notas dele, a prática **para**
//!   ([`Pratica::parada`]) e quem chama congela o relógio. Nota errada é extra, mas não para.
//!   Ao completar o passo parado, a prática pede a retomada ([`Pratica::tomar_retomada`]) num
//!   instante anterior conforme [`Retomada`]; os passos voltados são conferidos de novo. Cada
//!   parada conta como falta (ver [`Resumo::paradas`]).
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
    /// Para em cada nota até a certa, sem andamento (nome antigo: "Espera").
    NotaANota,
    /// Anda no andamento e só para quando a janela de uma nota vence sem ela.
    EsperaTocar,
    /// Segue o andamento sem parar.
    Tempo,
}

impl ModoPratica {
    pub const TODOS: [Self; 3] = [Self::NotaANota, Self::EsperaTocar, Self::Tempo];

    pub fn rotulo(self) -> &'static str {
        match self {
            Self::NotaANota => "Nota a nota",
            Self::EsperaTocar => "Espera tocar",
            Self::Tempo => "Seguir o andamento",
        }
    }

    /// Chave estável para gravar (a antiga "Espera" é lida como Nota a nota).
    pub fn chave(self) -> &'static str {
        match self {
            Self::NotaANota => "nota_a_nota",
            Self::EsperaTocar => "espera_tocar",
            Self::Tempo => "tempo",
        }
    }

    /// Lê a chave (ou o nome do `Debug`); aceita o nome antigo "Espera"/"espera".
    pub fn de_chave(t: &str) -> Option<Self> {
        match t.trim() {
            "nota_a_nota" | "NotaANota" | "Espera" | "espera" => Some(Self::NotaANota),
            "espera_tocar" | "EsperaTocar" => Some(Self::EsperaTocar),
            "tempo" | "Tempo" => Some(Self::Tempo),
            _ => None,
        }
    }

    /// O relógio anda no andamento (Tempo e Espera tocar).
    pub fn tem_andamento(self) -> bool {
        self != Self::NotaANota
    }
}

/// No Espera tocar: quanto voltar ao retomar depois de uma parada.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Retomada {
    /// Continua da nota em que parou.
    #[default]
    Zero,
    Uma,
    Duas,
    Quatro,
    /// Volta ao início da lição.
    Inicio,
}

impl Retomada {
    pub const TODAS: [Self; 5] = [
        Self::Zero,
        Self::Uma,
        Self::Duas,
        Self::Quatro,
        Self::Inicio,
    ];

    /// Quantas notas (passos) voltar; `None` = do início.
    pub fn notas(self) -> Option<usize> {
        match self {
            Self::Zero => Some(0),
            Self::Uma => Some(1),
            Self::Duas => Some(2),
            Self::Quatro => Some(4),
            Self::Inicio => None,
        }
    }

    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Zero => "Da nota em que parou",
            Self::Uma => "Voltar 1 nota",
            Self::Duas => "Voltar 2 notas",
            Self::Quatro => "Voltar 4 notas",
            Self::Inicio => "Do início da lição",
        }
    }

    pub fn chave(self) -> &'static str {
        match self {
            Self::Zero => "0",
            Self::Uma => "1",
            Self::Duas => "2",
            Self::Quatro => "4",
            Self::Inicio => "inicio",
        }
    }

    pub fn de_chave(t: &str) -> Option<Self> {
        Self::TODAS.into_iter().find(|r| r.chave() == t.trim())
    }
}

/// Batidas de contagem antes de retomar depois de uma parada no Espera tocar.
pub const BATIDAS_RETOMADA: f64 = 2.0;

/// Configuração da prática.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConfigPratica {
    pub modo: ModoPratica,
    /// Janela de acerto no modo Tempo, em ms (limitada a `0..=TOLERANCIA_MAXIMA_MS`).
    pub tolerancia_ms: f64,
    /// No Espera tocar: quanto voltar ao retomar.
    pub retomada: Retomada,
}

impl Default for ConfigPratica {
    fn default() -> Self {
        Self {
            modo: ModoPratica::NotaANota,
            tolerancia_ms: 150.0,
            retomada: Retomada::Zero,
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
    /// Vezes que o Espera tocar parou (já somadas em `faltas`); zero nos outros modos.
    pub paradas: usize,
}

impl Resumo {
    /// Percentual de acertos sobre as esperadas mais as paradas (cada parada é uma falta a
    /// mais); 100 se não havia nada esperado.
    pub fn precisao_pct(&self) -> f64 {
        let base = self.esperadas + self.paradas;
        if base == 0 {
            100.0
        } else {
            100.0 * self.acertos as f64 / base as f64
        }
    }
}

/// No Espera tocar: onde está o fluxo.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Fluxo {
    Andando,
    /// Parou no passo `passo`, com o relógio congelado em `t_s`.
    Parada {
        passo: usize,
        t_s: f64,
    },
    /// O passo parado foi completado: retomar em `t_s` (instante da partitura).
    Retomar {
        t_s: f64,
    },
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
    // Espera tocar
    fluxo: Fluxo,
    paradas: usize,
    extras_et: usize,
    /// Desvio (ms) do acerto de cada nota esperada, se houver.
    desvio_et: Vec<Option<f64>>,
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
            fluxo: Fluxo::Andando,
            paradas: 0,
            extras_et: 0,
            desvio_et: vec![None; n],
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
            ModoPratica::NotaANota => self.passo_atual >= self.passos.len(),
            ModoPratica::EsperaTocar => {
                self.fluxo == Fluxo::Andando
                    && self.estados.iter().all(|e| *e == EstadoEvento::Acertou)
            }
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
            ModoPratica::NotaANota | ModoPratica::EsperaTocar => {
                for e in &mut self.estados {
                    if *e != EstadoEvento::Acertou {
                        *e = EstadoEvento::Errou;
                    }
                }
                self.fluxo = Fluxo::Andando;
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
            ModoPratica::NotaANota => self.tocada_espera(midi),
            ModoPratica::EsperaTocar => self.tocada_espera_tocar(midi, t_s),
            ModoPratica::Tempo => self.tocada_tempo(midi, t_s),
        }
    }

    /// Faz o tempo andar até `agora_s` (modos Tempo e Espera tocar). Devolve `true` se algum
    /// estado mudou (no Espera tocar, inclusive quando a prática para).
    pub fn avancar_tempo(&mut self, agora_s: f64) -> bool {
        if self.encerrada || !agora_s.is_finite() {
            return false;
        }
        match self.config.modo {
            ModoPratica::NotaANota => return false,
            ModoPratica::EsperaTocar => return self.avancar_espera_tocar(agora_s),
            ModoPratica::Tempo => {}
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
            (ModoPratica::NotaANota | ModoPratica::EsperaTocar, false) => 0,
        } + self.paradas;
        let extras = match self.config.modo {
            ModoPratica::NotaANota => self.extras_espera,
            ModoPratica::EsperaTocar => self.extras_et,
            ModoPratica::Tempo => self.extras.len(),
        };
        let desvios: Vec<f64> = match self.config.modo {
            ModoPratica::EsperaTocar => self.desvio_et.iter().flatten().copied().collect(),
            _ => self.desvios_ms.clone(),
        };
        let (desvio_medio_ms, desvio_absoluto_ms) = if desvios.is_empty() {
            (None, None)
        } else {
            let n = desvios.len() as f64;
            (
                Some(desvios.iter().sum::<f64>() / n),
                Some(desvios.iter().map(|d| d.abs()).sum::<f64>() / n),
            )
        };
        Resumo {
            esperadas: self.estados.len(),
            acertos,
            faltas,
            extras,
            desvio_medio_ms,
            desvio_absoluto_ms,
            paradas: self.paradas,
        }
    }

    /// Espera tocar: o instante (da partitura) em que a prática está parada, se estiver. Quem
    /// chama congela o relógio (vídeo, acompanhamento, metrônomo) enquanto isto for `Some`.
    pub fn parada(&self) -> Option<f64> {
        match self.fluxo {
            Fluxo::Parada { t_s, .. } => Some(t_s),
            _ => None,
        }
    }

    /// Espera tocar: índices (em `esperados`) do passo parado que ainda faltam tocar.
    pub fn notas_da_parada(&self) -> Vec<usize> {
        match self.fluxo {
            Fluxo::Parada { passo, .. } => self.passos[passo]
                .iter()
                .copied()
                .filter(|&i| self.estados[i] != EstadoEvento::Acertou)
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Espera tocar: há retomada pedida (o aluno completou o passo parado)?
    pub fn retomada_pendente(&self) -> Option<f64> {
        match self.fluxo {
            Fluxo::Retomar { t_s } => Some(t_s),
            _ => None,
        }
    }

    /// Espera tocar: consome a retomada pedida e devolve o instante da partitura de onde tocar
    /// de novo. Quem chama conta [`BATIDAS_RETOMADA`] tempos e põe o relógio nesse instante
    /// (ou um pouco antes); o tempo da prática pode então andar de novo a partir dali.
    pub fn tomar_retomada(&mut self) -> Option<f64> {
        let t = self.retomada_pendente()?;
        self.fluxo = Fluxo::Andando;
        self.agora_s = f64::NEG_INFINITY;
        Some(t)
    }

    // --- Espera tocar ----------------------------------------------------------------------

    fn passo_completo(&self, p: usize) -> bool {
        self.passos[p]
            .iter()
            .all(|&i| self.estados[i] == EstadoEvento::Acertou)
    }

    fn avancar_espera_tocar(&mut self, agora_s: f64) -> bool {
        if self.fluxo != Fluxo::Andando {
            return false;
        }
        if agora_s > self.agora_s {
            self.agora_s = agora_s;
        }
        let tol_s = self.config.tolerancia_ms / 1000.0;
        // O primeiro passo incompleto cujo prazo já venceu.
        let vencido = (0..self.passos.len()).find(|&p| {
            !self.passo_completo(p)
                && self.esperados[self.passos[p][0]].inicio_s + tol_s < self.agora_s
        });
        let Some(p) = vencido else {
            return false;
        };
        let t_s = self.esperados[self.passos[p][0]].inicio_s + tol_s;
        self.agora_s = t_s;
        self.paradas += 1;
        self.fluxo = Fluxo::Parada { passo: p, t_s };
        true
    }

    fn tocada_espera_tocar(&mut self, midi: u8, t_s: f64) -> Veredito {
        match self.fluxo {
            Fluxo::Parada { passo, .. } => {
                let achado = self.passos[passo].iter().copied().find(|&i| {
                    self.esperados[i].midi == midi && self.estados[i] != EstadoEvento::Acertou
                });
                let Some(i) = achado else {
                    self.extras_et += 1;
                    return Veredito::Erro;
                };
                self.estados[i] = EstadoEvento::Acertou;
                self.desvio_et[i] = None;
                if self.passo_completo(passo) {
                    self.preparar_retomada(passo);
                }
                Veredito::Acerto
            }
            Fluxo::Andando | Fluxo::Retomar { .. } => {
                if t_s > self.agora_s && matches!(self.fluxo, Fluxo::Andando) {
                    self.agora_s = t_s;
                }
                let tol_s = self.config.tolerancia_ms / 1000.0;
                // A nota pendente mais próxima no tempo, dentro da janela, com essa altura.
                let achado = (0..self.esperados.len())
                    .filter(|&i| {
                        self.esperados[i].midi == midi
                            && self.estados[i] != EstadoEvento::Acertou
                            && (self.esperados[i].inicio_s - t_s).abs() <= tol_s + 1e-9
                    })
                    .min_by(|&a, &b| {
                        (self.esperados[a].inicio_s - t_s)
                            .abs()
                            .total_cmp(&(self.esperados[b].inicio_s - t_s).abs())
                    });
                match achado {
                    Some(i) => {
                        self.estados[i] = EstadoEvento::Acertou;
                        self.desvio_et[i] = Some((t_s - self.esperados[i].inicio_s) * 1000.0);
                        Veredito::Acerto
                    }
                    None => {
                        self.extras_et += 1;
                        Veredito::Erro
                    }
                }
            }
        }
    }

    /// O passo parado `passo` foi completado: decide de onde retomar e reabre os voltados.
    fn preparar_retomada(&mut self, passo: usize) {
        let desde = match self.config.retomada.notas() {
            Some(n) => passo.saturating_sub(n),
            None => 0,
        };
        // Com 0 o passo parado fica cumprido; voltando, ele e os anteriores são conferidos de
        // novo.
        if self.config.retomada != Retomada::Zero {
            for p in desde..=passo {
                for &i in &self.passos[p] {
                    self.estados[i] = EstadoEvento::Pendente;
                    self.desvio_et[i] = None;
                }
            }
        }
        if self.estados.iter().all(|e| *e == EstadoEvento::Acertou) {
            // Era o último passo: nada a retomar.
            self.fluxo = Fluxo::Andando;
            return;
        }
        let t_s = self.esperados[self.passos[desde][0]].inicio_s;
        self.fluxo = Fluxo::Retomar { t_s };
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
            retomada: Retomada::Zero,
        }
    }

    fn et(ms: f64, retomada: Retomada) -> ConfigPratica {
        ConfigPratica {
            modo: ModoPratica::EsperaTocar,
            tolerancia_ms: ms,
            retomada,
        }
    }

    /// Dó Ré Mi Fá Sol, uma por segundo.
    fn escala() -> Vec<EventoEsperado> {
        (0..5)
            .map(|k| esp([60, 62, 64, 65, 67][k], k as f64))
            .collect()
    }

    #[test]
    fn espera_tocar_para_ao_fim_da_janela() {
        let e = escala();
        let mut p = Pratica::nova(&e, et(150.0, Retomada::Zero));
        assert_eq!(p.nota_tocada(60, 0.05), Veredito::Acerto);
        assert!(!p.avancar_tempo(1.1), "ainda dentro da janela do Ré");
        assert_eq!(p.parada(), None);
        assert!(p.avancar_tempo(1.2), "a janela do Ré venceu");
        let t = p.parada().expect("parou");
        assert!((t - 1.15).abs() < 1e-9, "congela no fim da janela: {t}");
        assert_eq!(p.notas_da_parada(), vec![1]);
        // Parada, o tempo não anda e não para de novo.
        assert!(!p.avancar_tempo(5.0));
        assert_eq!(p.resumo().paradas, 1);
        assert_eq!(p.resumo().faltas, 1);
        assert!(!p.concluida());
    }

    #[test]
    fn espera_tocar_nota_errada_nao_para() {
        let e = escala();
        let mut p = Pratica::nova(&e, et(150.0, Retomada::Zero));
        assert_eq!(p.nota_tocada(61, 0.0), Veredito::Erro);
        assert_eq!(p.parada(), None, "errar não para");
        assert_eq!(p.nota_tocada(60, 0.1), Veredito::Acerto, "ainda dá tempo");
        assert!(!p.avancar_tempo(0.9));
        assert_eq!(p.parada(), None);
        let r = p.resumo();
        assert_eq!((r.acertos, r.extras, r.paradas), (1, 1, 0));
        // Parado, nota errada é extra e não libera.
        assert!(p.avancar_tempo(1.2));
        assert_eq!(p.nota_tocada(64, 9.0), Veredito::Erro);
        assert!(p.parada().is_some());
        assert_eq!(p.resumo().extras, 2);
    }

    #[test]
    fn espera_tocar_acorde_incompleto_para() {
        let e = [esp(60, 0.0), esp(64, 0.0), esp(67, 0.0), esp(72, 1.0)];
        let mut p = Pratica::nova(&e, et(100.0, Retomada::Zero));
        assert_eq!(p.nota_tocada(60, 0.0), Veredito::Acerto);
        assert_eq!(p.nota_tocada(67, 0.02), Veredito::Acerto);
        assert!(p.avancar_tempo(0.2), "faltou o Mi do acorde");
        assert_eq!(p.notas_da_parada(), vec![1]);
        assert_eq!(p.nota_tocada(60, 0.3), Veredito::Erro, "já tocada");
        assert_eq!(p.retomada_pendente(), None);
        assert_eq!(p.nota_tocada(64, 0.3), Veredito::Acerto);
        assert_eq!(p.parada(), None);
        assert_eq!(p.tomar_retomada(), Some(0.0), "0 notas: do próprio acorde");
        assert_eq!(p.tomar_retomada(), None, "consumida");
        // O acorde fica cumprido; segue para o Dó agudo.
        assert_eq!(p.nota_tocada(72, 1.0), Veredito::Acerto);
        assert!(p.concluida());
        let r = p.resumo();
        assert_eq!((r.acertos, r.paradas, r.faltas, r.extras), (4, 1, 1, 1));
        assert!((r.precisao_pct() - 80.0).abs() < 1e-9, "4 de 4 + 1 parada");
    }

    /// Para no Sol (passo 4) e completa; devolve o instante de retomada e os estados.
    fn parar_no_sol(retomada: Retomada) -> (Pratica, f64) {
        let e = escala();
        let mut p = Pratica::nova(&e, et(150.0, retomada));
        for (k, m) in [60, 62, 64, 65].into_iter().enumerate() {
            assert_eq!(p.nota_tocada(m, k as f64), Veredito::Acerto);
        }
        assert!(p.avancar_tempo(4.5));
        assert_eq!(p.notas_da_parada(), vec![4]);
        assert_eq!(p.nota_tocada(67, 6.0), Veredito::Acerto);
        let t = p.tomar_retomada().unwrap_or(f64::NAN);
        (p, t)
    }

    #[test]
    fn espera_tocar_retomada_volta_0_1_2_4_ou_do_inicio() {
        // O Sol é o último: com 0 não há o que retomar e a prática termina.
        let (p, t) = parar_no_sol(Retomada::Zero);
        assert!(t.is_nan());
        assert!(p.concluida());
        for (r, t_esperado, reabertas) in [
            (Retomada::Uma, 3.0, 3..5),
            (Retomada::Duas, 2.0, 2..5),
            (Retomada::Quatro, 0.0, 0..5),
            (Retomada::Inicio, 0.0, 0..5),
        ] {
            let (mut p, t) = parar_no_sol(r);
            assert_eq!(t, t_esperado, "{r:?}");
            for i in 0..5 {
                let pendente = p.estados()[i] == EstadoEvento::Pendente;
                assert_eq!(pendente, reabertas.contains(&i), "{r:?} nota {i}");
            }
            assert!(!p.concluida());
            // As voltadas são conferidas de novo: tocá-las certas conclui (relógio recomeça
            // antes, na contagem).
            assert!(!p.avancar_tempo(t - 2.0));
            for i in reabertas.clone() {
                assert_eq!(p.nota_tocada(e_midi(i), i as f64 + 0.01), Veredito::Acerto);
            }
            assert!(p.concluida(), "{r:?}");
            assert_eq!(p.resumo().paradas, 1);
        }
        // Do meio, 4 notas que passam do início ficam no início.
        let e = escala();
        let mut p = Pratica::nova(&e, et(150.0, Retomada::Quatro));
        assert_eq!(p.nota_tocada(60, 0.0), Veredito::Acerto);
        assert!(p.avancar_tempo(1.5));
        assert_eq!(p.nota_tocada(62, 2.0), Veredito::Acerto);
        assert_eq!(p.tomar_retomada(), Some(0.0));
        // Voltadas e não tocadas param de novo.
        assert!(p.avancar_tempo(0.5));
        assert_eq!(p.notas_da_parada(), vec![0]);
        assert_eq!(p.resumo().paradas, 2);
    }

    fn e_midi(i: usize) -> u8 {
        [60, 62, 64, 65, 67][i]
    }

    #[test]
    fn espera_tocar_paradas_contam_na_precisao_e_encerrar_fecha() {
        let e = escala();
        let mut p = Pratica::nova(&e, et(150.0, Retomada::Zero));
        assert!(p.avancar_tempo(0.5), "não tocou o Dó");
        assert_eq!(p.nota_tocada(60, 3.0), Veredito::Acerto);
        assert_eq!(p.tomar_retomada(), Some(0.0));
        assert_eq!(p.nota_tocada(62, 1.0), Veredito::Acerto);
        assert!(p.avancar_tempo(2.5), "não tocou o Mi");
        assert_eq!(p.nota_tocada(64, 9.0), Veredito::Acerto);
        assert!(p.tomar_retomada().is_some());
        let r = p.resumo();
        assert_eq!((r.acertos, r.paradas, r.faltas), (3, 2, 2));
        assert!((r.precisao_pct() - 300.0 / 7.0).abs() < 1e-9);
        p.encerrar();
        let r = p.resumo();
        assert_eq!(
            (r.acertos, r.paradas, r.faltas),
            (3, 2, 4),
            "2 não tocadas + 2 paradas"
        );
        assert_eq!(p.parada(), None);
        assert!(p.concluida());
    }

    #[test]
    fn espera_tocar_mede_desvio_dos_acertos() {
        let e = escala();
        let mut p = Pratica::nova(&e, et(150.0, Retomada::Zero));
        assert_eq!(p.nota_tocada(60, 0.1), Veredito::Acerto);
        assert_eq!(p.nota_tocada(62, 0.9), Veredito::Acerto);
        let r = p.resumo();
        assert!(r.desvio_medio_ms.is_some_and(|d| d.abs() < 1e-6));
        assert!(
            r.desvio_absoluto_ms
                .is_some_and(|d| (d - 100.0).abs() < 1e-6)
        );
    }

    #[test]
    fn chaves_de_modo_e_retomada_ida_e_volta_e_nome_antigo() {
        for m in ModoPratica::TODOS {
            assert_eq!(ModoPratica::de_chave(m.chave()), Some(m));
            assert_eq!(ModoPratica::de_chave(&format!("{m:?}")), Some(m));
        }
        assert_eq!(
            ModoPratica::de_chave("Espera"),
            Some(ModoPratica::NotaANota)
        );
        assert_eq!(ModoPratica::de_chave("lixo"), None);
        for r in Retomada::TODAS {
            assert_eq!(Retomada::de_chave(r.chave()), Some(r));
        }
        assert_eq!(Retomada::de_chave("3"), None);
    }

    #[test]
    fn espera_avanca_so_com_a_nota_certa_e_conta_extras() {
        let e = [esp(60, 0.0), esp(62, 0.5), esp(64, 1.0)];
        let mut p = Pratica::nova(&e, cfg(ModoPratica::NotaANota, 0.0));
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
        let mut p = Pratica::nova(&e, cfg(ModoPratica::NotaANota, 0.0));
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
        let mut p = Pratica::nova(&e, cfg(ModoPratica::NotaANota, 0.0));
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
        let mut p = Pratica::nova(&e, cfg(ModoPratica::NotaANota, 0.0));
        assert_eq!(p.evento_do_passo_atual(), Some(1));
        assert_eq!(p.nota_nota(60), Veredito::Acerto);
        assert_eq!(p.estados(), [EstadoEvento::Pendente, EstadoEvento::Acertou]);
        assert_eq!(p.evento_do_passo_atual(), Some(0));
    }
}
