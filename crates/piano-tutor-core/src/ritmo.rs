//! Ritmo (F08 e F09): fórmulas de compasso, metrônomo, padrões de bateria de passos,
//! agendamento sem deriva, toque tap-tempo e avaliação de toque contra o pulso.
//!
//! Tudo é código puro sobre instantes em nanossegundos do relógio comum. A grade interna tem
//! **12 tiques por pulso** (pulso = semínima nas fórmulas /4 e semínima pontuada nas /8), o
//! que cabe semicolcheias (3 tiques em /4, 2 em /8) e tercinas (4 tiques em /4) sem arredondar.
//! O instante do tique `k` é calculado direto de `k` em aritmética inteira de 128 bits:
//! não há soma acumulada, logo não há deriva em sessões longas.

use std::fmt::Write as _;

/// Tiques por pulso da grade interna.
pub const TIQUES_POR_PULSO: u64 = 12;
/// Faixa de andamento aceita.
pub const BPM_MIN: u16 = 40;
pub const BPM_MAX: u16 = 220;

/// Fórmula de compasso suportada.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Formula {
    DoisQuatro,
    TresQuatro,
    QuatroQuatro,
    /// 6/8 contado em **2 pulsos compostos** (3+3 colcheias), não em seis semínimas.
    SeisOito,
    /// 12/8 contado em 4 pulsos compostos.
    DozeOito,
}

impl Formula {
    pub const TODAS: [Formula; 5] = [
        Formula::DoisQuatro,
        Formula::TresQuatro,
        Formula::QuatroQuatro,
        Formula::SeisOito,
        Formula::DozeOito,
    ];

    pub const fn texto(self) -> &'static str {
        match self {
            Formula::DoisQuatro => "2/4",
            Formula::TresQuatro => "3/4",
            Formula::QuatroQuatro => "4/4",
            Formula::SeisOito => "6/8",
            Formula::DozeOito => "12/8",
        }
    }

    pub fn de_texto(t: &str) -> Option<Self> {
        Self::TODAS.into_iter().find(|f| f.texto() == t.trim())
    }

    /// Pulsos contados por compasso.
    pub const fn pulsos(self) -> u64 {
        match self {
            Formula::DoisQuatro | Formula::SeisOito => 2,
            Formula::TresQuatro => 3,
            Formula::QuatroQuatro | Formula::DozeOito => 4,
        }
    }

    /// Compasso composto (pulso dividido em três colcheias).
    pub const fn composta(self) -> bool {
        matches!(self, Formula::SeisOito | Formula::DozeOito)
    }

    /// Passos do sequenciador por pulso: semicolcheias (4 em /4, 6 em /8).
    pub const fn passos_por_pulso(self) -> u64 {
        if self.composta() { 6 } else { 4 }
    }

    /// Tiques por passo do sequenciador.
    pub const fn tiques_por_passo(self) -> u64 {
        TIQUES_POR_PULSO / self.passos_por_pulso()
    }

    pub const fn passos_por_compasso(self) -> u64 {
        self.pulsos() * self.passos_por_pulso()
    }

    /// Explicação curta do agrupamento.
    pub const fn explicacao(self) -> &'static str {
        match self {
            Formula::DoisQuatro => "2 pulsos de semínima; acento no 1.",
            Formula::TresQuatro => "3 pulsos de semínima (valsa); acento no 1 de 3.",
            Formula::QuatroQuatro => "4 pulsos de semínima; acento no 1 de 4.",
            Formula::SeisOito => {
                "2 pulsos compostos (semínima pontuada), cada um com 3 colcheias: 1-2-3 4-5-6."
            }
            Formula::DozeOito => "4 pulsos compostos, cada um com 3 colcheias (shuffle, blues).",
        }
    }
}

/// Subdivisão audível do metrônomo entre os pulsos.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Subdivisao {
    Nenhuma,
    /// Binária em /4 (2 por pulso); nas compostas, as 3 colcheias do pulso.
    Colcheias,
    /// Ternária em /4 (3 por pulso); nas compostas, igual às colcheias.
    Tercinas,
    Semicolcheias,
}

impl Subdivisao {
    pub const TODAS: [Subdivisao; 4] = [
        Subdivisao::Nenhuma,
        Subdivisao::Colcheias,
        Subdivisao::Tercinas,
        Subdivisao::Semicolcheias,
    ];

    pub const fn texto(self) -> &'static str {
        match self {
            Subdivisao::Nenhuma => "só o pulso",
            Subdivisao::Colcheias => "colcheias",
            Subdivisao::Tercinas => "tercinas",
            Subdivisao::Semicolcheias => "semicolcheias",
        }
    }

    /// Tiques entre cliques da subdivisão (`None` = sem subdivisão).
    pub const fn intervalo_tiques(self, f: Formula) -> Option<u64> {
        match (self, f.composta()) {
            (Subdivisao::Nenhuma, _) => None,
            (Subdivisao::Colcheias, false) => Some(6),
            (Subdivisao::Tercinas, false) => Some(4),
            (Subdivisao::Colcheias | Subdivisao::Tercinas, true) => Some(4),
            (Subdivisao::Semicolcheias, false) => Some(3),
            (Subdivisao::Semicolcheias, true) => Some(2),
        }
    }
}

/// Peça de bateria (nota GM no canal 10).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Instrumento {
    pub nome: &'static str,
    pub nota_gm: u8,
}

/// Peças disponíveis no sequenciador, na ordem das linhas. As 8 primeiras são os pads.
pub const INSTRUMENTOS: [Instrumento; 12] = [
    Instrumento {
        nome: "Bumbo",
        nota_gm: 36,
    },
    Instrumento {
        nome: "Caixa",
        nota_gm: 38,
    },
    Instrumento {
        nome: "Chimbal fechado",
        nota_gm: 42,
    },
    Instrumento {
        nome: "Chimbal aberto",
        nota_gm: 46,
    },
    Instrumento {
        nome: "Palmas",
        nota_gm: 39,
    },
    Instrumento {
        nome: "Aro",
        nota_gm: 37,
    },
    Instrumento {
        nome: "Prato",
        nota_gm: 49,
    },
    Instrumento {
        nome: "Tom",
        nota_gm: 45,
    },
    Instrumento {
        nome: "Pandeiro",
        nota_gm: 54,
    },
    Instrumento {
        nome: "Ganzá",
        nota_gm: 70,
    },
    Instrumento {
        nome: "Triângulo",
        nota_gm: 81,
    },
    Instrumento {
        nome: "Cowbell",
        nota_gm: 56,
    },
];

/// Grupo de corte (*choke*) do chimbal: o chimbal fechado (42) e o pedal (44) calam o aberto.
pub const CHIMBAL_FECHADO: u8 = 42;
pub const CHIMBAL_ABERTO: u8 = 46;

/// Nota que deve ser calada quando `nota` começa (grupo de corte), se houver.
pub const fn corta_nota(nota: u8) -> Option<u8> {
    if nota == CHIMBAL_FECHADO {
        Some(CHIMBAL_ABERTO)
    } else {
        None
    }
}

/// Notas GM do metrônomo (wood block agudo e grave).
pub const CLIQUE_FORTE: u8 = 76;
pub const CLIQUE_FRACO: u8 = 77;

/// Velocidades dos passos na notação de texto: `X` acento, `x` normal, `o` fantasma.
pub const VEL_ACENTO: u8 = 120;
pub const VEL_NORMAL: u8 = 95;
pub const VEL_FANTASMA: u8 = 55;

/// Padrão do sequenciador: uma linha de velocidades por instrumento (0 = silêncio).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Padrao {
    pub nome: String,
    pub formula: Formula,
    /// 1 a 4 compassos.
    pub compassos: u8,
    pub bpm: u16,
    /// `linhas[i]` é de `INSTRUMENTOS[i]`; cada uma tem `passos()` valores.
    pub linhas: Vec<Vec<u8>>,
}

/// Erro ao ler um padrão salvo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErroPadrao(pub String);

impl std::fmt::Display for ErroPadrao {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Padrao {
    /// Padrão vazio.
    pub fn vazio(nome: &str, formula: Formula, compassos: u8, bpm: u16) -> Self {
        let compassos = compassos.clamp(1, 4);
        let n = (formula.passos_por_compasso() * u64::from(compassos)) as usize;
        Self {
            nome: nome.to_owned(),
            formula,
            compassos,
            bpm: bpm.clamp(BPM_MIN, BPM_MAX),
            linhas: vec![vec![0; n]; INSTRUMENTOS.len()],
        }
    }

    pub fn passos(&self) -> usize {
        (self.formula.passos_por_compasso() * u64::from(self.compassos)) as usize
    }

    /// Velocidade de `instrumento` no passo (0 = nada).
    pub fn velocidade(&self, instrumento: usize, passo: usize) -> u8 {
        self.linhas
            .get(instrumento)
            .and_then(|l| l.get(passo))
            .copied()
            .unwrap_or(0)
    }

    /// Alterna um passo: vazio → normal → acento → vazio.
    pub fn alternar(&mut self, instrumento: usize, passo: usize) {
        if let Some(v) = self
            .linhas
            .get_mut(instrumento)
            .and_then(|l| l.get_mut(passo))
        {
            *v = match *v {
                0 => VEL_NORMAL,
                v if v < VEL_ACENTO => VEL_ACENTO,
                _ => 0,
            };
        }
    }

    /// Muda fórmula/compassos preservando o que couber.
    pub fn redimensionar(&mut self, formula: Formula, compassos: u8) {
        self.formula = formula;
        self.compassos = compassos.clamp(1, 4);
        let n = self.passos();
        for l in &mut self.linhas {
            l.resize(n, 0);
        }
    }

    pub fn limpar(&mut self) {
        for l in &mut self.linhas {
            l.fill(0);
        }
    }

    /// Texto salvo em disco (formato próprio, legível e versionável).
    pub fn para_texto(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "# Piano Tutor: padrão de ritmo v1");
        let _ = writeln!(s, "nome={}", self.nome.replace('\n', " "));
        let _ = writeln!(s, "formula={}", self.formula.texto());
        let _ = writeln!(s, "compassos={}", self.compassos);
        let _ = writeln!(s, "bpm={}", self.bpm);
        for (i, l) in self.linhas.iter().enumerate() {
            if l.iter().all(|v| *v == 0) {
                continue;
            }
            let passos: String = l.iter().map(|v| caractere(*v)).collect();
            let _ = writeln!(s, "{}={passos}", INSTRUMENTOS[i].nota_gm);
        }
        s
    }

    /// Lê o texto de [`Padrao::para_texto`]. Linhas desconhecidas são erro (sem adivinhar).
    pub fn de_texto(texto: &str) -> Result<Self, ErroPadrao> {
        let mut nome = None;
        let mut formula = None;
        let mut compassos = None;
        let mut bpm = None;
        let mut linhas: Vec<(u8, &str)> = Vec::new();
        for (n, linha) in texto.lines().enumerate() {
            let linha = linha.trim();
            if linha.is_empty() || linha.starts_with('#') {
                continue;
            }
            let Some((k, v)) = linha.split_once('=') else {
                return Err(ErroPadrao(format!("linha {}: falta '='", n + 1)));
            };
            match k.trim() {
                "nome" => nome = Some(v.trim().to_owned()),
                "formula" => {
                    formula = Some(Formula::de_texto(v).ok_or_else(|| {
                        ErroPadrao(format!("linha {}: fórmula desconhecida '{v}'", n + 1))
                    })?);
                }
                "compassos" => {
                    compassos = Some(
                        v.trim()
                            .parse::<u8>()
                            .ok()
                            .filter(|c| (1..=4).contains(c))
                            .ok_or_else(|| {
                                ErroPadrao(format!("linha {}: compassos vai de 1 a 4", n + 1))
                            })?,
                    );
                }
                "bpm" => {
                    bpm = Some(
                        v.trim()
                            .parse::<u16>()
                            .ok()
                            .filter(|b| (BPM_MIN..=BPM_MAX).contains(b))
                            .ok_or_else(|| {
                                ErroPadrao(format!(
                                    "linha {}: bpm vai de {BPM_MIN} a {BPM_MAX}",
                                    n + 1
                                ))
                            })?,
                    );
                }
                chave => {
                    let nota = chave.parse::<u8>().map_err(|_| {
                        ErroPadrao(format!("linha {}: chave desconhecida '{chave}'", n + 1))
                    })?;
                    linhas.push((nota, v.trim()));
                }
            }
        }
        let formula = formula.ok_or_else(|| ErroPadrao("falta a fórmula".to_owned()))?;
        let mut p = Padrao::vazio(
            &nome.unwrap_or_else(|| "Sem nome".to_owned()),
            formula,
            compassos.unwrap_or(1),
            bpm.unwrap_or(100),
        );
        let n = p.passos();
        for (nota, passos) in linhas {
            let i = INSTRUMENTOS
                .iter()
                .position(|x| x.nota_gm == nota)
                .ok_or_else(|| ErroPadrao(format!("instrumento GM {nota} não suportado")))?;
            let v: Vec<u8> = passos
                .chars()
                .filter(|c| !c.is_whitespace() && *c != '|')
                .map(velocidade_de)
                .collect::<Option<_>>()
                .ok_or_else(|| ErroPadrao(format!("passo inválido no instrumento {nota}")))?;
            if v.len() != n {
                return Err(ErroPadrao(format!(
                    "instrumento {nota}: {} passos, esperado {n}",
                    v.len()
                )));
            }
            p.linhas[i] = v;
        }
        Ok(p)
    }
}

fn caractere(v: u8) -> char {
    match v {
        0 => '.',
        v if v >= VEL_ACENTO => 'X',
        v if v <= VEL_FANTASMA => 'o',
        _ => 'x',
    }
}

fn velocidade_de(c: char) -> Option<u8> {
    match c {
        '.' | '-' => Some(0),
        'X' => Some(VEL_ACENTO),
        'x' => Some(VEL_NORMAL),
        'o' => Some(VEL_FANTASMA),
        _ => None,
    }
}

/// Padrões de fábrica (escritos aqui, sem material de terceiros).
pub fn padroes_de_fabrica() -> Vec<Padrao> {
    const FONTES: [&str; 11] = [
        "nome=Rock básico\nformula=4/4\nbpm=100\n36=x.......x.x.....\n38=....x.......x...\n42=x.x.x.x.x.x.x.x.\n49=x...............",
        "nome=Pop balada\nformula=4/4\nbpm=76\n36=x......x.x......\n38=....x.......x...\n42=x.x.x.x.x.x.x.x.",
        "nome=Pop dançante\nformula=4/4\nbpm=118\n36=x...x...x...x...\n39=....x.......x...\n42=x...x...x...x...\n46=..x...x...x...x.",
        "nome=Funk\nformula=4/4\nbpm=96\n36=x.x....x..x.....\n38=....X..o.o..X..o\n42=xoxoxoxoxoxoxoxo",
        "nome=Reggae (one drop)\nformula=4/4\nbpm=76\n36=........x.......\n37=........x.......\n42=..x...x...x...x.",
        "nome=Bossa nova\nformula=4/4\ncompassos=2\nbpm=128\n36=x..xx..xx..xx..xx..xx..xx..xx..x\n37=x..x..x...x..x....x..x...x..x...\n70=xoxoxoxoxoxoxoxoxoxoxoxoxoxoxoxo",
        "nome=Samba\nformula=2/4\ncompassos=2\nbpm=100\n36=....x.......X...\n54=xoox.xoxxoox.xox\n37=x..x..x...x..x..",
        "nome=Baião\nformula=2/4\ncompassos=2\nbpm=95\n36=x..x....x..x....\n81=xoxxxoxxxoxxxoxx\n37=....x.......x...",
        "nome=Valsa\nformula=3/4\nbpm=90\n36=x...........\n42=....x...x...",
        "nome=Balada 6/8\nformula=6/8\nbpm=58\n36=x...........\n38=......x.....\n42=x.x.x.x.x.x.",
        "nome=Blues shuffle\nformula=12/8\nbpm=72\n36=x...........x.....x.....\n38=......x...........x.....\n42=x...x.x...x.x...x.x...x.",
    ];
    FONTES
        .iter()
        .filter_map(|t| Padrao::de_texto(t).ok())
        .collect()
}

/// Acento de um clique do metrônomo.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Clique {
    /// Primeiro pulso do compasso.
    Forte,
    /// Outros pulsos.
    Pulso,
    /// Subdivisão entre pulsos.
    Subdivisao,
}

/// Evento produzido pelo agendador.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventoRitmo {
    Clique(Clique),
    /// `INSTRUMENTOS[instrumento]` com a velocidade do passo.
    Batida {
        instrumento: usize,
        velocidade: u8,
    },
}

/// Configuração do que toca.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfigRitmo {
    pub metronomo: bool,
    pub subdivisao: Subdivisao,
    pub bateria: bool,
    /// Compassos de contagem (só metrônomo) antes da bateria.
    pub contagem: u8,
}

impl Default for ConfigRitmo {
    fn default() -> Self {
        Self {
            metronomo: true,
            subdivisao: Subdivisao::Nenhuma,
            bateria: false,
            contagem: 0,
        }
    }
}

/// Agenda tiques a partir de uma âncora. `t(k) = inicio + (k - base) * 5e9 / bpm`.
#[derive(Clone, Debug)]
pub struct Agendador {
    inicio_ns: u64,
    base: u64,
    bpm: u16,
    proximo: u64,
    formula: Formula,
}

/// Nanossegundos por minuto dividido pelos tiques por pulso.
const NS_POR_TIQUE_X_BPM: u128 = 60_000_000_000 / TIQUES_POR_PULSO as u128;

impl Agendador {
    /// Começa no instante `inicio_ns` (tique 0 = primeiro pulso do primeiro compasso).
    pub fn novo(inicio_ns: u64, bpm: u16, formula: Formula) -> Self {
        Self {
            inicio_ns,
            base: 0,
            bpm: bpm.clamp(BPM_MIN, BPM_MAX),
            proximo: 0,
            formula,
        }
    }

    pub fn bpm(&self) -> u16 {
        self.bpm
    }

    pub fn formula(&self) -> Formula {
        self.formula
    }

    /// Instante do tique `k`.
    pub fn instante(&self, k: u64) -> u64 {
        let d = u128::from(k.saturating_sub(self.base)) * NS_POR_TIQUE_X_BPM / u128::from(self.bpm);
        self.inicio_ns
            .saturating_add(u64::try_from(d).unwrap_or(u64::MAX))
    }

    /// Duração de um pulso em ns no andamento atual.
    pub fn pulso_ns(&self) -> u64 {
        (60_000_000_000u64) / u64::from(self.bpm)
    }

    /// Troca o andamento sem saltar nem duplicar: o próximo tique ainda não emitido mantém
    /// seu instante e os seguintes passam a usar o novo andamento.
    pub fn definir_bpm(&mut self, bpm: u16) {
        let bpm = bpm.clamp(BPM_MIN, BPM_MAX);
        if bpm == self.bpm {
            return;
        }
        let t = self.instante(self.proximo);
        self.inicio_ns = t;
        self.base = self.proximo;
        self.bpm = bpm;
    }

    /// Tique onde está o instante `t` (fracionário truncado); `None` antes do início.
    pub fn tique_em(&self, t_ns: u64) -> Option<u64> {
        if t_ns < self.inicio_ns {
            return None;
        }
        let d = u128::from(t_ns - self.inicio_ns) * u128::from(self.bpm) / NS_POR_TIQUE_X_BPM;
        Some(self.base + u64::try_from(d).unwrap_or(u64::MAX))
    }

    /// Emite os tiques com instante `< horizonte_ns` ainda não emitidos, com os eventos de
    /// metrônomo e bateria de cada um.
    pub fn emitir_ate(
        &mut self,
        horizonte_ns: u64,
        config: ConfigRitmo,
        padrao: Option<&Padrao>,
        saida: &mut Vec<(u64, EventoRitmo)>,
    ) {
        while self.instante(self.proximo) < horizonte_ns {
            let k = self.proximo;
            let t = self.instante(k);
            self.eventos_do_tique(k, config, padrao, |e| saida.push((t, e)));
            self.proximo += 1;
        }
    }

    fn eventos_do_tique(
        &self,
        k: u64,
        config: ConfigRitmo,
        padrao: Option<&Padrao>,
        mut emitir: impl FnMut(EventoRitmo),
    ) {
        let f = self.formula;
        let tiques_compasso = TIQUES_POR_PULSO * f.pulsos();
        let contagem_tiques = u64::from(config.contagem) * tiques_compasso;
        let contando = k < contagem_tiques;
        if config.metronomo || contando {
            if k.is_multiple_of(TIQUES_POR_PULSO) {
                let pulso = (k / TIQUES_POR_PULSO) % f.pulsos();
                emitir(EventoRitmo::Clique(if pulso == 0 {
                    Clique::Forte
                } else {
                    Clique::Pulso
                }));
            } else if let Some(i) = config.subdivisao.intervalo_tiques(f)
                && k.is_multiple_of(i)
            {
                emitir(EventoRitmo::Clique(Clique::Subdivisao));
            }
        }
        if !config.bateria || contando {
            return;
        }
        let Some(p) = padrao else {
            return;
        };
        let tpp = p.formula.tiques_por_passo();
        let kk = k - contagem_tiques;
        if !kk.is_multiple_of(tpp) || p.passos() == 0 {
            return;
        }
        let passo = ((kk / tpp) % p.passos() as u64) as usize;
        for (i, linha) in p.linhas.iter().enumerate() {
            let v = linha.get(passo).copied().unwrap_or(0);
            if v > 0 {
                emitir(EventoRitmo::Batida {
                    instrumento: i,
                    velocidade: v,
                });
            }
        }
    }

    /// Pulso (0-based desde o início) e posição dentro do compasso para mostrar na tela.
    pub fn posicao(&self, t_ns: u64) -> Option<Posicao> {
        let k = self.tique_em(t_ns)?;
        let pulso_abs = k / TIQUES_POR_PULSO;
        let pulsos = self.formula.pulsos();
        Some(Posicao {
            compasso: pulso_abs / pulsos,
            pulso: pulso_abs % pulsos,
            tique: k,
        })
    }

    /// Desvio de `t_ns` para o pulso mais próximo (negativo = adiantado).
    pub fn desvio_do_pulso(&self, t_ns: u64) -> Option<i64> {
        let k = self.tique_em(t_ns)?;
        let anterior = k / TIQUES_POR_PULSO * TIQUES_POR_PULSO;
        let seguinte = anterior + TIQUES_POR_PULSO;
        let ta = self.instante(anterior) as i128;
        let ts = self.instante(seguinte) as i128;
        let t = i128::from(t_ns);
        let d = if t - ta <= ts - t { t - ta } else { t - ts };
        i64::try_from(d).ok()
    }
}

/// Posição do relógio de ritmo.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Posicao {
    pub compasso: u64,
    pub pulso: u64,
    pub tique: u64,
}

impl Posicao {
    /// Passo do sequenciador (para destacar a coluna).
    pub fn passo(&self, p: &Padrao, contagem: u8) -> Option<usize> {
        let tiques_compasso = TIQUES_POR_PULSO * p.formula.pulsos();
        let kk = self
            .tique
            .checked_sub(u64::from(contagem) * tiques_compasso)?;
        Some(((kk / p.formula.tiques_por_passo()) % p.passos().max(1) as u64) as usize)
    }
}

/// Classificação de um toque contra o pulso.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Julgamento {
    Adiantado,
    NoTempo,
    Atrasado,
}

/// Cor semântica do feedback (a interface a traduz para a paleta; aqui só a lógica).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorFeedback {
    Verde,
    Ambar,
    Vermelho,
}

impl Julgamento {
    /// Texto mostrado ao aluno.
    pub const fn texto(self) -> &'static str {
        match self {
            Julgamento::NoTempo => "no tempo",
            Julgamento::Adiantado => "adiantado",
            Julgamento::Atrasado => "atrasado",
        }
    }

    /// Cor do feedback visual: verde certo, âmbar adiantado, vermelho atrasado.
    pub const fn cor(self) -> CorFeedback {
        match self {
            Julgamento::NoTempo => CorFeedback::Verde,
            Julgamento::Adiantado => CorFeedback::Ambar,
            Julgamento::Atrasado => CorFeedback::Vermelho,
        }
    }
}

/// Faixa da tolerância configurável do exercício de pulso, em ms (±).
pub const TOLERANCIA_MIN_MS: u32 = 10;
pub const TOLERANCIA_MAX_MS: u32 = 150;

/// Classifica um desvio com tolerância simétrica.
pub fn julgar(desvio_ns: i64, tolerancia_ns: u64) -> Julgamento {
    if desvio_ns.unsigned_abs() <= tolerancia_ns {
        Julgamento::NoTempo
    } else if desvio_ns < 0 {
        Julgamento::Adiantado
    } else {
        Julgamento::Atrasado
    }
}

/// Tap tempo: média dos últimos intervalos; recomeça após 2 s sem toque.
#[derive(Clone, Debug, Default)]
pub struct TapTempo {
    toques: Vec<u64>,
}

impl TapTempo {
    const PAUSA_NS: u64 = 2_000_000_000;
    const MAX_TOQUES: usize = 6;

    /// Registra um toque e devolve o andamento estimado (a partir do segundo toque).
    pub fn tocar(&mut self, t_ns: u64) -> Option<u16> {
        if self
            .toques
            .last()
            .is_some_and(|u| t_ns.saturating_sub(*u) > Self::PAUSA_NS || t_ns < *u)
        {
            self.toques.clear();
        }
        self.toques.push(t_ns);
        if self.toques.len() > Self::MAX_TOQUES {
            self.toques.remove(0);
        }
        let n = self.toques.len();
        if n < 2 {
            return None;
        }
        let medio = (self.toques[n - 1] - self.toques[0]) / (n as u64 - 1);
        if medio == 0 {
            return None;
        }
        let bpm = (60_000_000_000u64 + medio / 2) / medio;
        Some(
            u16::try_from(bpm)
                .unwrap_or(BPM_MAX)
                .clamp(BPM_MIN, BPM_MAX),
        )
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn cliques(eventos: &[(u64, EventoRitmo)]) -> Vec<(u64, Clique)> {
        eventos
            .iter()
            .filter_map(|(t, e)| match e {
                EventoRitmo::Clique(c) => Some((*t, *c)),
                EventoRitmo::Batida { .. } => None,
            })
            .collect()
    }

    #[test]
    fn a_120_bpm_a_seminima_tem_500_ms_exatos() {
        let mut a = Agendador::novo(1_000, 120, Formula::QuatroQuatro);
        let mut v = Vec::new();
        a.emitir_ate(1_000 + 4_000_000_000, ConfigRitmo::default(), None, &mut v);
        let c = cliques(&v);
        assert_eq!(c.len(), 8);
        for (i, (t, _)) in c.iter().enumerate() {
            assert_eq!(*t, 1_000 + i as u64 * 500_000_000);
        }
    }

    #[test]
    fn acentos_por_formula() {
        for (f, esperado) in [
            (
                Formula::TresQuatro,
                vec![true, false, false, true, false, false],
            ),
            (
                Formula::QuatroQuatro,
                vec![true, false, false, false, true, false],
            ),
            // 6/8 em dois pulsos compostos: acento a cada 2 pulsos, não a cada 6.
            (
                Formula::SeisOito,
                vec![true, false, true, false, true, false],
            ),
        ] {
            let mut a = Agendador::novo(0, 60, f);
            let mut v = Vec::new();
            a.emitir_ate(6_000_000_000 - 1, ConfigRitmo::default(), None, &mut v);
            let fortes: Vec<bool> = cliques(&v)
                .iter()
                .map(|(_, c)| *c == Clique::Forte)
                .collect();
            assert_eq!(fortes, esperado, "{}", f.texto());
        }
    }

    #[test]
    fn subdivisao_em_6_8_da_tres_colcheias_por_pulso() {
        let mut a = Agendador::novo(0, 60, Formula::SeisOito);
        let mut v = Vec::new();
        let cfg = ConfigRitmo {
            subdivisao: Subdivisao::Colcheias,
            ..ConfigRitmo::default()
        };
        a.emitir_ate(1_000_000_000, cfg, None, &mut v);
        assert_eq!(cliques(&v).len(), 3);
    }

    #[test]
    fn vinte_minutos_sem_deriva() {
        let mut a = Agendador::novo(0, 97, Formula::QuatroQuatro);
        let mut v = Vec::new();
        let vinte_min = 20 * 60 * 1_000_000_000u64;
        // Em fatias como a UI faria (quadros de ~16 ms).
        let mut h = 0;
        while h < vinte_min {
            h += 16_666_667;
            a.emitir_ate(h, ConfigRitmo::default(), None, &mut v);
        }
        let c = cliques(&v);
        let n = c.len() as u64;
        let (ultimo, _) = c[c.len() - 1];
        // Instante teórico do último pulso: (n-1) * 60/97 s, em ns, arredondado para baixo.
        let teorico = ((n - 1) as u128 * 60_000_000_000 / 97) as u64;
        assert!(ultimo.abs_diff(teorico) <= 1, "{ultimo} vs {teorico}");
        // Intervalos: erro máximo de 1 ns em relação a 60/97 s.
        let ideal = 60_000_000_000f64 / 97.0;
        for w in c.windows(2) {
            let d = (w[1].0 - w[0].0) as f64;
            assert!((d - ideal).abs() <= 1.0);
        }
    }

    #[test]
    fn mudar_bpm_no_meio_nao_duplica_nem_salta() {
        let mut a = Agendador::novo(0, 120, Formula::QuatroQuatro);
        let mut v = Vec::new();
        a.emitir_ate(1_200_000_000, ConfigRitmo::default(), None, &mut v);
        a.definir_bpm(60);
        a.emitir_ate(5_000_000_000, ConfigRitmo::default(), None, &mut v);
        let t: Vec<u64> = cliques(&v).iter().map(|(t, _)| *t).collect();
        // 0, 0,5 e 1,0 s a 120; o pulso que estava em curso termina num tempo entre os dois
        // andamentos (sem clique duplo nem pulso pulado); depois, de 1 em 1 s.
        assert_eq!(&t[..3], &[0, 500_000_000, 1_000_000_000]);
        let transicao = t[3] - t[2];
        assert!(
            (500_000_000..1_000_000_000).contains(&transicao),
            "{transicao}"
        );
        for w in t[3..].windows(2) {
            assert_eq!(w[1] - w[0], 1_000_000_000);
        }
        assert_eq!(t.len(), 7);
        let fortes: Vec<bool> = cliques(&v)
            .iter()
            .map(|(_, c)| *c == Clique::Forte)
            .collect();
        assert_eq!(fortes, vec![true, false, false, false, true, false, false]);
    }

    #[test]
    fn padrao_toca_os_passos_e_a_contagem_vem_antes() {
        let p = padroes_de_fabrica()
            .into_iter()
            .find(|p| p.nome == "Rock básico")
            .expect("rock");
        let mut a = Agendador::novo(0, 120, Formula::QuatroQuatro);
        let cfg = ConfigRitmo {
            metronomo: false,
            bateria: true,
            contagem: 1,
            subdivisao: Subdivisao::Nenhuma,
        };
        let mut v = Vec::new();
        a.emitir_ate(4_000_000_000, cfg, Some(&p), &mut v);
        // Contagem: 4 cliques no primeiro compasso (2 s a 120 bpm), sem bateria.
        let c = cliques(&v);
        assert_eq!(c.len(), 4);
        assert!(c.iter().all(|(t, _)| *t < 2_000_000_000));
        let bumbos: Vec<u64> = v
            .iter()
            .filter(|(_, e)| matches!(e, EventoRitmo::Batida { instrumento: 0, .. }))
            .map(|(t, _)| *t)
            .collect();
        // Bumbo nos passos 0, 8 e 10 (semicolcheia = 125 ms).
        assert_eq!(bumbos, vec![2_000_000_000, 3_000_000_000, 3_250_000_000]);
    }

    #[test]
    fn padroes_de_fabrica_sao_validos_e_sobrevivem_a_ida_e_volta() {
        let ps = padroes_de_fabrica();
        assert_eq!(ps.len(), 11);
        for p in ps {
            let t = p.para_texto();
            let q = Padrao::de_texto(&t).unwrap_or_else(|e| panic!("{}: {e}", p.nome));
            assert_eq!(p, q);
            assert!(p.linhas.iter().any(|l| l.iter().any(|v| *v > 0)));
        }
    }

    #[test]
    fn texto_invalido_vira_erro_sem_panico() {
        for t in [
            "",
            "formula=5/4",
            "formula=4/4\n36=x..",
            "formula=4/4\n99=x...............",
            "formula=4/4\nbpm=999",
            "formula=4/4\nlixo",
            "formula=4/4\n36=x......?........",
        ] {
            assert!(Padrao::de_texto(t).is_err(), "{t}");
        }
    }

    #[test]
    fn alternar_e_redimensionar() {
        let mut p = Padrao::vazio("t", Formula::QuatroQuatro, 1, 100);
        p.alternar(0, 0);
        assert_eq!(p.velocidade(0, 0), VEL_NORMAL);
        p.alternar(0, 0);
        assert_eq!(p.velocidade(0, 0), VEL_ACENTO);
        p.alternar(0, 0);
        assert_eq!(p.velocidade(0, 0), 0);
        p.alternar(1, 15);
        p.redimensionar(Formula::TresQuatro, 2);
        assert_eq!(p.passos(), 24);
        assert_eq!(p.velocidade(1, 15), VEL_NORMAL);
    }

    #[test]
    fn julgar_toques_adiantados_certos_e_atrasados() {
        let a = Agendador::novo(0, 120, Formula::QuatroQuatro);
        let tol = 40_000_000;
        let casos = [
            (500_000_000 - 90_000_000, Julgamento::Adiantado),
            (500_000_000 + 10_000_000, Julgamento::NoTempo),
            (500_000_000 + 100_000_000, Julgamento::Atrasado),
            (1_000_000_000 - 30_000_000, Julgamento::NoTempo),
        ];
        for (t, esperado) in casos {
            let d = a.desvio_do_pulso(t).expect("após o início");
            assert_eq!(julgar(d, tol), esperado, "t={t} d={d}");
        }
        assert_eq!(
            a.desvio_do_pulso(500_000_000 - 90_000_000),
            Some(-90_000_000)
        );
    }

    #[test]
    fn chimbal_fechado_corta_o_aberto_e_so_ele() {
        assert_eq!(corta_nota(CHIMBAL_FECHADO), Some(CHIMBAL_ABERTO));
        for i in INSTRUMENTOS.iter().filter(|i| i.nota_gm != CHIMBAL_FECHADO) {
            assert_eq!(corta_nota(i.nota_gm), None, "{}", i.nome);
        }
        // As duas notas do grupo existem como peças de bateria.
        for n in [CHIMBAL_FECHADO, CHIMBAL_ABERTO] {
            assert!(INSTRUMENTOS.iter().any(|i| i.nota_gm == n));
        }
    }

    #[test]
    fn tap_tempo_estima_e_recomeca_apos_pausa() {
        let mut t = TapTempo::default();
        assert_eq!(t.tocar(0), None);
        assert_eq!(t.tocar(500_000_000), Some(120));
        assert_eq!(t.tocar(1_000_000_000), Some(120));
        // Pausa longa: recomeça.
        assert_eq!(t.tocar(10_000_000_000), None);
        assert_eq!(t.tocar(11_000_000_000), Some(60));
    }

    #[test]
    fn posicao_e_passo_para_a_tela() {
        let a = Agendador::novo(0, 120, Formula::QuatroQuatro);
        let p = Padrao::vazio("t", Formula::QuatroQuatro, 1, 120);
        let pos = a.posicao(2_600_000_000).expect("posição");
        assert_eq!((pos.compasso, pos.pulso), (1, 1));
        assert_eq!(pos.passo(&p, 0), Some(4));
        assert_eq!(pos.passo(&p, 2), None);
    }

    // --- F08-3: tolerância configurável e feedback ------------------------------------------

    #[test]
    fn feedback_de_cada_julgamento_tem_texto_e_cor_proprios() {
        let todos = [
            Julgamento::Adiantado,
            Julgamento::NoTempo,
            Julgamento::Atrasado,
        ];
        assert_eq!(
            todos.map(Julgamento::texto),
            ["adiantado", "no tempo", "atrasado"]
        );
        assert_eq!(
            todos.map(Julgamento::cor),
            [
                CorFeedback::Ambar,
                CorFeedback::Verde,
                CorFeedback::Vermelho
            ]
        );
    }

    #[test]
    fn a_tolerancia_configurada_muda_o_julgamento_do_mesmo_toque() {
        // Três exercícios: toques deliberadamente adiantados, certos e atrasados.
        let ms = 1_000_000i64;
        let adiantados = [-120 * ms, -80 * ms, -45 * ms];
        let certos = [-5 * ms, 0, 8 * ms];
        let atrasados = [45 * ms, 80 * ms, 120 * ms];
        let conta = |desvios: &[i64], tol_ms: u32| -> [usize; 3] {
            let mut c = [0; 3];
            for d in desvios {
                let tol = u64::from(tol_ms) * 1_000_000;
                c[match julgar(*d, tol) {
                    Julgamento::Adiantado => 0,
                    Julgamento::NoTempo => 1,
                    Julgamento::Atrasado => 2,
                }] += 1;
            }
            c
        };
        // Tolerância mínima (10 ms): os certos continuam certos e os outros, fora.
        assert_eq!(conta(&adiantados, TOLERANCIA_MIN_MS), [3, 0, 0]);
        assert_eq!(conta(&certos, TOLERANCIA_MIN_MS), [0, 3, 0]);
        assert_eq!(conta(&atrasados, TOLERANCIA_MIN_MS), [0, 0, 3]);
        // 50 ms: o desvio de 45 ms passa a valer como certo, nos dois sentidos.
        assert_eq!(conta(&adiantados, 50), [2, 1, 0]);
        assert_eq!(conta(&atrasados, 50), [0, 1, 2]);
        // 100 ms: só os de 120 ms ficam fora.
        assert_eq!(conta(&adiantados, 100), [1, 2, 0]);
        assert_eq!(conta(&atrasados, 100), [0, 2, 1]);
        // Máxima (150 ms): tudo é "no tempo".
        assert_eq!(conta(&adiantados, TOLERANCIA_MAX_MS), [0, 3, 0]);
        assert_eq!(conta(&atrasados, TOLERANCIA_MAX_MS), [0, 3, 0]);
        // Na borda exata da tolerância ainda é "no tempo".
        assert_eq!(julgar(50 * ms, 50_000_000), Julgamento::NoTempo);
        assert_eq!(julgar(50 * ms + 1, 50_000_000), Julgamento::Atrasado);
        assert_eq!(julgar(-50 * ms - 1, 50_000_000), Julgamento::Adiantado);
    }

    // --- F09: pads, grade e fábrica ---------------------------------------------------------

    /// Nomes GM (nível 1, canal 10) das notas usadas, conferidos peça a peça.
    const NOMES_GM: [(u8, &str); 12] = [
        (36, "Bass Drum 1"),
        (38, "Acoustic Snare"),
        (42, "Closed Hi-Hat"),
        (46, "Open Hi-Hat"),
        (39, "Hand Clap"),
        (37, "Side Stick"),
        (49, "Crash Cymbal 1"),
        (45, "Low Tom"),
        (54, "Tambourine"),
        (70, "Maracas"),
        (81, "Open Triangle"),
        (56, "Cowbell"),
    ];

    #[test]
    fn os_oito_pads_sao_oito_sons_gm_distintos_e_auditados() {
        let pads = &INSTRUMENTOS[..8];
        let mut notas: Vec<u8> = pads.iter().map(|i| i.nota_gm).collect();
        notas.sort_unstable();
        notas.dedup();
        assert_eq!(notas.len(), 8, "8 pads, 8 notas GM distintas");
        // Todas as 12 peças: notas únicas, dentro da faixa GM de percussão (35 a 81), com o
        // nome GM esperado, e sem colidir com os cliques do metrônomo (76 e 77).
        let mut todas: Vec<u8> = INSTRUMENTOS.iter().map(|i| i.nota_gm).collect();
        todas.sort_unstable();
        todas.dedup();
        assert_eq!(todas.len(), INSTRUMENTOS.len());
        for (i, (nota, nome_gm)) in INSTRUMENTOS.iter().zip(NOMES_GM) {
            assert_eq!(i.nota_gm, nota, "{}: deveria ser GM {nome_gm}", i.nome);
            assert!((35..=81).contains(&i.nota_gm), "{}", i.nome);
            assert!(
                ![CLIQUE_FORTE, CLIQUE_FRACO].contains(&i.nota_gm),
                "{}",
                i.nome
            );
        }
        // Os cliques também são notas GM de percussão: 76 Hi Wood Block e 77 Low Wood Block.
        assert_eq!((CLIQUE_FORTE, CLIQUE_FRACO), (76, 77));
        // O pad 1 é o bumbo e o pad 3 o chimbal fechado, o que dá sentido às teclas 1 a 8.
        assert_eq!(pads[0].nome, "Bumbo");
        assert_eq!(pads[2].nota_gm, CHIMBAL_FECHADO);
    }

    #[test]
    fn grade_do_sequenciador_por_formula() {
        // (fórmula, passos por compasso, passos por pulso, tiques por passo)
        for (f, compasso, pulso, tiques) in [
            (Formula::DoisQuatro, 8, 4, 3),
            (Formula::TresQuatro, 12, 4, 3),
            (Formula::QuatroQuatro, 16, 4, 3),
            (Formula::SeisOito, 12, 6, 2),
            (Formula::DozeOito, 24, 6, 2),
        ] {
            assert_eq!(f.passos_por_compasso(), compasso, "{}", f.texto());
            assert_eq!(f.passos_por_pulso(), pulso, "{}", f.texto());
            assert_eq!(f.tiques_por_passo(), tiques, "{}", f.texto());
            // O passo cabe exatamente no pulso: nenhum arredondamento na grade.
            assert_eq!(
                f.tiques_por_passo() * f.passos_por_pulso(),
                TIQUES_POR_PULSO
            );
        }
        // Padrão vazio com 1 a 4 compassos tem a grade da fórmula, em qualquer fórmula.
        for f in Formula::TODAS {
            for c in 1..=4u8 {
                let p = Padrao::vazio("t", f, c, 100);
                assert_eq!(p.passos() as u64, f.passos_por_compasso() * u64::from(c));
                assert!(p.linhas.iter().all(|l| l.len() == p.passos()));
            }
        }
    }

    /// Instante (ns) de cada passo com batida de `instrumento`, em `p`, tocando `compassos`
    /// compassos a partir de 0 e a `bpm`.
    fn instantes_do_instrumento(p: &Padrao, instrumento: usize, bpm: u16) -> Vec<u64> {
        let mut a = Agendador::novo(0, bpm, p.formula);
        let cfg = ConfigRitmo {
            metronomo: false,
            bateria: true,
            ..ConfigRitmo::default()
        };
        let mut v = Vec::new();
        let duracao = p.formula.pulsos() * u64::from(p.compassos) * 60_000_000_000 / u64::from(bpm);
        a.emitir_ate(duracao, cfg, Some(p), &mut v);
        v.iter()
            .filter_map(|(t, e)| match e {
                EventoRitmo::Batida { instrumento: i, .. } if *i == instrumento => Some(*t),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn grade_de_tres_por_quatro_tem_doze_passos_e_um_acento_de_tres() {
        let valsa = padroes_de_fabrica()
            .into_iter()
            .find(|p| p.nome == "Valsa")
            .expect("valsa");
        assert_eq!(valsa.formula, Formula::TresQuatro);
        assert_eq!(valsa.passos(), 12);
        // A 60 bpm o pulso é 1 s e o passo, 250 ms. Bumbo só no passo 0; chimbal nos pulsos 2 e 3
        // (passos 4 e 8), ou seja, o acento do 1 de 3.
        assert_eq!(instantes_do_instrumento(&valsa, 0, 60), vec![0]);
        assert_eq!(
            instantes_do_instrumento(&valsa, 2, 60),
            vec![1_000_000_000, 2_000_000_000]
        );
        // O metrônomo no mesmo compasso acentua 1 de 3.
        let mut a = Agendador::novo(0, 60, Formula::TresQuatro);
        let mut v = Vec::new();
        a.emitir_ate(3_000_000_000, ConfigRitmo::default(), None, &mut v);
        let fortes = cliques(&v)
            .iter()
            .filter(|(_, c)| *c == Clique::Forte)
            .count();
        assert_eq!(fortes, 1);
    }

    #[test]
    fn grade_de_seis_por_oito_tem_doze_passos_em_dois_pulsos_de_tres_colcheias() {
        let balada = padroes_de_fabrica()
            .into_iter()
            .find(|p| p.nome == "Balada 6/8")
            .expect("balada 6/8");
        assert_eq!(balada.formula, Formula::SeisOito);
        assert_eq!(balada.passos(), 12);
        // A 60 bpm (pulso composto = 1 s): bumbo no passo 0, caixa no passo 6 (2º pulso
        // composto), chimbal nos passos 0, 2, 4, 6, 8 e 10 (as seis colcheias, 166,667 ms).
        assert_eq!(instantes_do_instrumento(&balada, 0, 60), vec![0]);
        assert_eq!(
            instantes_do_instrumento(&balada, 1, 60),
            vec![1_000_000_000]
        );
        let chimbal = instantes_do_instrumento(&balada, 2, 60);
        assert_eq!(chimbal.len(), 6);
        for (n, t) in chimbal.iter().enumerate() {
            assert_eq!(*t, n as u64 * 1_000_000_000 / 3, "colcheia {n}");
        }
        // 6/8 não é seis semínimas: o compasso dura 2 pulsos e a explicação diz isso.
        assert_eq!(Formula::SeisOito.pulsos(), 2);
        let e = Formula::SeisOito.explicacao();
        assert!(
            e.contains("2 pulsos compostos") && e.contains("3 colcheias"),
            "{e}"
        );
        assert!(e.contains("1-2-3 4-5-6"), "{e}");
        // Cada fórmula tem explicação não vazia e própria.
        let mut textos: Vec<&str> = Formula::TODAS.iter().map(|f| f.explicacao()).collect();
        assert!(textos.iter().all(|t| !t.is_empty()));
        textos.sort_unstable();
        textos.dedup();
        assert_eq!(textos.len(), Formula::TODAS.len());
    }

    #[test]
    fn a_fabrica_tem_os_onze_padroes_documentados() {
        // A lista é a mesma de docs/aceite/F09/padroes-de-fabrica.md: mudar uma exige mudar a outra.
        let nomes: Vec<String> = padroes_de_fabrica().into_iter().map(|p| p.nome).collect();
        assert_eq!(
            nomes,
            [
                "Rock básico",
                "Pop balada",
                "Pop dançante",
                "Funk",
                "Reggae (one drop)",
                "Bossa nova",
                "Samba",
                "Baião",
                "Valsa",
                "Balada 6/8",
                "Blues shuffle",
            ]
        );
    }

    #[test]
    fn mil_compassos_no_agendador_nao_acumulam_erro_em_nenhuma_formula() {
        // O instante do primeiro tique de cada compasso é calculado do índice, não somado:
        // erro máximo de 1 ns (truncamento) em qualquer dos 1.000 compassos.
        for formula in Formula::TODAS {
            for bpm in [BPM_MIN, 60, 97, 120, 133, BPM_MAX] {
                let inicio = 7_000_000_000u64;
                let a = Agendador::novo(inicio, bpm, formula);
                let tiques_compasso = TIQUES_POR_PULSO * formula.pulsos();
                let mut anterior = inicio;
                for k in 0..=1_000u64 {
                    let t = a.instante(k * tiques_compasso);
                    // Exato, em racional: início + k * pulsos * 60e9 / bpm.
                    let exato = u128::from(k) * u128::from(formula.pulsos()) * 60_000_000_000u128
                        / u128::from(bpm);
                    let esperado = inicio + u64::try_from(exato).expect("cabe em u64");
                    assert!(
                        t.abs_diff(esperado) <= 1,
                        "{} a {bpm} bpm, compasso {k}: {t} vs {esperado}",
                        formula.texto()
                    );
                    // Compassos consecutivos: a distância nunca difere do ideal em mais de 1 ns.
                    if k > 0 {
                        let ideal = formula.pulsos() as f64 * 60_000_000_000f64 / f64::from(bpm);
                        assert!(((t - anterior) as f64 - ideal).abs() <= 1.0);
                    }
                    anterior = t;
                }
            }
        }
    }

    #[test]
    fn mil_compassos_de_4_4_a_97_bpm_batem_com_o_total_teorico() {
        // 1.000 compassos de 4/4 = 4.000 pulsos a 97 bpm = 240.000/97 s.
        let mut a = Agendador::novo(0, 97, Formula::QuatroQuatro);
        let mut v = Vec::new();
        let fim = a.instante(4_000 * TIQUES_POR_PULSO);
        let mut h = 0;
        while h < fim {
            h += 16_666_667;
            a.emitir_ate(h, ConfigRitmo::default(), None, &mut v);
        }
        let c = cliques(&v);
        // O pulso de índice 3.999 (último do compasso 1.000) ocorre em 3.999 * 60/97 s.
        let (instante, _) = c[3_999];
        let teorico = (3_999u128 * 60_000_000_000 / 97) as u64;
        assert!(instante.abs_diff(teorico) <= 1, "{instante} vs {teorico}");
        // Acentos: um forte a cada quatro cliques, nos 1.000 compassos.
        let fortes = c
            .iter()
            .take(4_000)
            .filter(|(_, k)| matches!(k, Clique::Forte))
            .count();
        assert_eq!(fortes, 1_000);
    }
}
