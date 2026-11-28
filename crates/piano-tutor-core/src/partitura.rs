//! Modelo puro de partitura lido de MusicXML `score-partwise` (F07, somente leitura: ADR-0007).
//!
//! Sem UI e sem áudio. Lê partes, pautas, compassos, armadura, fôrma, claves (inclusive trocas),
//! notas e pausas (altura, número MIDI, duração racional, voz, pauta, acorde, ligadura de
//! valor, pontos, acidentes, vigas, tuplos), andamento e repetições. Também gera a lista de
//! eventos de reprodução (início em semínimas, como fração exata) e o mapa de andamento.
//!
//! Segurança: tamanho máximo do texto, limite de nós XML, de partes, de compassos e de notas;
//! qualquer arquivo malformado devolve `Err`, nunca causa pânico. DTD é aceita (todo MusicXML
//! traz `DOCTYPE`), mas entidade externa nunca é resolvida e a expansão de entidades, o número
//! de nós e a profundidade são limitados: a política e a prova estão em [`crate::xml_seguro`] e
//! os limites estão reunidos em `docs/seguranca/LIMITES-XML-MXL.md`.
//!
//! Limitações conhecidas (registradas em `docs/aceite/F07/evidencia.md`): só `score-partwise`;
//! `.mxl` (zip) é lido pelo módulo [`crate::mxl`], que extrai o XML e chama este leitor; `<transpose>` e oitavas (`octave-shift`) não alteram o MIDI;
//! notas de adorno (`grace`) são mantidas no modelo mas não tocam; letra é ignorada.

use roxmltree::Node;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::{Add, Sub};

/// Tamanho máximo aceito do texto MusicXML.
pub const TAMANHO_MAXIMO_BYTES: usize = 8 * 1024 * 1024;
/// Limite de nós XML (inclui atributos e textos).
pub const MAX_NOS_XML: u32 = 2_000_000;
/// Limite de notas e pausas somadas.
pub const MAX_NOTAS: usize = 200_000;
/// Limite de compassos por parte.
pub const MAX_COMPASSOS: usize = 20_000;
/// Limite de partes.
pub const MAX_PARTES: usize = 64;
/// Limite de pautas por parte.
pub const MAX_PAUTAS: u8 = 8;

// --- Erros ---------------------------------------------------------------------------------

/// Motivo da recusa de um arquivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroPartitura {
    /// Texto maior que [`TAMANHO_MAXIMO_BYTES`].
    GrandeDemais(usize),
    /// XML malformado (mensagem do analisador).
    XmlInvalido(String),
    /// A raiz não é `score-partwise` (nome da raiz encontrada).
    RaizNaoSuportada(String),
    /// Nenhuma parte com compassos.
    SemPartes,
    /// Um limite de segurança foi excedido (qual).
    LimiteExcedido(&'static str),
    /// Valor inválido num elemento (descrição).
    ValorInvalido(String),
    /// Contêiner `.mxl` (zip) inválido, inseguro ou não suportado (motivo).
    ContainerInvalido(String),
}

impl From<crate::xml_seguro::FalhaXml> for ErroPartitura {
    fn from(f: crate::xml_seguro::FalhaXml) -> Self {
        use crate::xml_seguro::FalhaXml;
        match f {
            FalhaXml::Invalido(m) => Self::XmlInvalido(m),
            FalhaXml::GrandeDemais(n) => Self::GrandeDemais(n),
            FalhaXml::Limite(q) => Self::LimiteExcedido(q),
        }
    }
}

impl fmt::Display for ErroPartitura {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GrandeDemais(n) => write!(
                f,
                "arquivo grande demais ({n} bytes; máximo {TAMANHO_MAXIMO_BYTES})"
            ),
            Self::XmlInvalido(m) => write!(f, "XML inválido: {m}"),
            Self::RaizNaoSuportada(r) => write!(
                f,
                "só MusicXML 'score-partwise' é suportado (encontrado '{r}')"
            ),
            Self::SemPartes => write!(f, "a partitura não tem partes com compassos"),
            Self::LimiteExcedido(q) => write!(f, "limite de segurança excedido: {q}"),
            Self::ValorInvalido(d) => write!(f, "valor inválido: {d}"),
            Self::ContainerInvalido(m) => write!(f, "arquivo .mxl recusado: {m}"),
        }
    }
}

impl std::error::Error for ErroPartitura {}

// --- Fração --------------------------------------------------------------------------------

fn mdc(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Número racional exato (denominador sempre positivo, forma irredutível). Mede tempo em
/// semínimas desde o início do compasso ou da peça.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fracao {
    num: i64,
    den: i64,
}

impl Fracao {
    pub const ZERO: Fracao = Fracao { num: 0, den: 1 };

    /// Cria a fração; denominador zero vira zero (valor neutro, sem pânico).
    pub fn nova(num: i64, den: i64) -> Self {
        Self::de_i128(i128::from(num), i128::from(den))
    }

    pub const fn inteiro(n: i64) -> Self {
        Fracao { num: n, den: 1 }
    }

    fn de_i128(num: i128, den: i128) -> Self {
        if den == 0 {
            return Self::ZERO;
        }
        let g = mdc(num, den).max(1);
        let (mut n, mut d) = (num / g, den / g);
        if d < 0 {
            n = -n;
            d = -d;
        }
        Fracao {
            num: i64::try_from(n).unwrap_or(if n < 0 { i64::MIN } else { i64::MAX }),
            den: i64::try_from(d).unwrap_or(i64::MAX),
        }
    }

    pub const fn numerador(self) -> i64 {
        self.num
    }

    pub const fn denominador(self) -> i64 {
        self.den
    }

    pub fn como_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }

    pub const fn e_zero(self) -> bool {
        self.num == 0
    }

    /// Multiplica por um racional `a/b`.
    pub fn vezes(self, a: i64, b: i64) -> Self {
        Self::de_i128(
            i128::from(self.num) * i128::from(a),
            i128::from(self.den) * i128::from(b),
        )
    }
}

impl Add for Fracao {
    type Output = Fracao;
    fn add(self, o: Fracao) -> Fracao {
        Fracao::de_i128(
            i128::from(self.num) * i128::from(o.den) + i128::from(o.num) * i128::from(self.den),
            i128::from(self.den) * i128::from(o.den),
        )
    }
}

impl Sub for Fracao {
    type Output = Fracao;
    fn sub(self, o: Fracao) -> Fracao {
        Fracao::de_i128(
            i128::from(self.num) * i128::from(o.den) - i128::from(o.num) * i128::from(self.den),
            i128::from(self.den) * i128::from(o.den),
        )
    }
}

impl PartialOrd for Fracao {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl Ord for Fracao {
    fn cmp(&self, o: &Self) -> Ordering {
        (i128::from(self.num) * i128::from(o.den)).cmp(&(i128::from(o.num) * i128::from(self.den)))
    }
}

impl fmt::Display for Fracao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{}/{}", self.num, self.den)
        }
    }
}

// --- Elementos do modelo -------------------------------------------------------------------

/// Altura escrita: passo diatônico (0 = Dó … 6 = Si), alteração em semitons e oitava
/// científica (Dó central = Dó4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Altura {
    pub passo: u8,
    pub alteracao: i8,
    pub oitava: i8,
}

impl Altura {
    /// Posição diatônica absoluta: Dó4 = 28.
    pub fn diatonica(self) -> i32 {
        i32::from(self.oitava) * 7 + i32::from(self.passo)
    }

    /// Número MIDI (Dó4 = 60), se couber em 0..=127.
    pub fn numero_midi(self) -> Option<u8> {
        const SEMITONS: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
        let n = (i32::from(self.oitava) + 1) * 12
            + SEMITONS[usize::from(self.passo.min(6))]
            + i32::from(self.alteracao);
        u8::try_from(n).ok().filter(|v| *v <= 127)
    }

    /// Letra do passo (`C`..`B`).
    pub const fn letra(self) -> char {
        ['C', 'D', 'E', 'F', 'G', 'A', 'B'][if self.passo < 7 {
            self.passo as usize
        } else {
            0
        }]
    }
}

/// Figura (valor escrito) da nota ou pausa.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Figura {
    Breve,
    Semibreve,
    Minima,
    Seminima,
    Colcheia,
    Semicolcheia,
    Fusa,
    Semifusa,
    Quasifusa,
}

impl Figura {
    /// Duração escrita, em semínimas, sem pontos.
    pub fn duracao_nominal(self) -> Fracao {
        match self {
            Self::Breve => Fracao::inteiro(8),
            Self::Semibreve => Fracao::inteiro(4),
            Self::Minima => Fracao::inteiro(2),
            Self::Seminima => Fracao::inteiro(1),
            Self::Colcheia => Fracao::nova(1, 2),
            Self::Semicolcheia => Fracao::nova(1, 4),
            Self::Fusa => Fracao::nova(1, 8),
            Self::Semifusa => Fracao::nova(1, 16),
            Self::Quasifusa => Fracao::nova(1, 32),
        }
    }

    /// Número de bandeirolas (ou vigas) da figura.
    pub const fn bandeirolas(self) -> u8 {
        match self {
            Self::Breve | Self::Semibreve | Self::Minima | Self::Seminima => 0,
            Self::Colcheia => 1,
            Self::Semicolcheia => 2,
            Self::Fusa => 3,
            Self::Semifusa => 4,
            Self::Quasifusa => 5,
        }
    }

    /// Tem haste (mínima e mais curtas).
    pub const fn tem_haste(self) -> bool {
        !matches!(self, Self::Breve | Self::Semibreve)
    }

    /// Texto do `<type>` do MusicXML que escreve a figura.
    pub(crate) const fn texto_xml(self) -> &'static str {
        match self {
            Self::Breve => "breve",
            Self::Semibreve => "whole",
            Self::Minima => "half",
            Self::Seminima => "quarter",
            Self::Colcheia => "eighth",
            Self::Semicolcheia => "16th",
            Self::Fusa => "32nd",
            Self::Semifusa => "64th",
            Self::Quasifusa => "128th",
        }
    }

    /// Duração escrita, em semínimas, com `pontos` pontos de aumento (cada ponto vale metade do
    /// anterior: 1 ponto = 3/2, 2 pontos = 7/4, 3 pontos = 15/8).
    pub(crate) fn com_pontos(self, pontos: u8) -> Fracao {
        let nominal = self.duracao_nominal();
        match pontos {
            0 => nominal,
            1 => nominal.vezes(3, 2),
            2 => nominal.vezes(7, 4),
            _ => nominal.vezes(15, 8),
        }
    }

    pub(crate) fn de_texto(t: &str) -> Option<Self> {
        Some(match t {
            "breve" => Self::Breve,
            "whole" => Self::Semibreve,
            "half" => Self::Minima,
            "quarter" => Self::Seminima,
            "eighth" => Self::Colcheia,
            "16th" => Self::Semicolcheia,
            "32nd" => Self::Fusa,
            "64th" => Self::Semifusa,
            "128th" => Self::Quasifusa,
            _ => return None,
        })
    }

    pub(crate) const TODAS: [Figura; 9] = [
        Self::Breve,
        Self::Semibreve,
        Self::Minima,
        Self::Seminima,
        Self::Colcheia,
        Self::Semicolcheia,
        Self::Fusa,
        Self::Semifusa,
        Self::Quasifusa,
    ];

    /// Figura e pontos que escrevem a duração `d` (semínimas), se houver.
    pub(crate) fn por_duracao(d: Fracao) -> Option<(Self, u8)> {
        for f in Self::TODAS {
            for pontos in 0..=2u8 {
                let nominal = f.duracao_nominal();
                let total = match pontos {
                    0 => nominal,
                    1 => nominal.vezes(3, 2),
                    _ => nominal.vezes(7, 4),
                };
                if total == d {
                    return Some((f, pontos));
                }
            }
        }
        None
    }
}

/// Acidente escrito ao lado da nota.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acidente {
    Sustenido,
    Bemol,
    Bequadro,
    DobradoSustenido,
    DobradoBemol,
}

impl Acidente {
    pub fn de_texto(t: &str) -> Option<Self> {
        Some(match t {
            "sharp" => Self::Sustenido,
            "flat" => Self::Bemol,
            "natural" => Self::Bequadro,
            "double-sharp" | "sharp-sharp" => Self::DobradoSustenido,
            "flat-flat" => Self::DobradoBemol,
            _ => return None,
        })
    }
}

/// Estado de uma viga num nível (1 = viga principal).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EstadoViga {
    Inicio,
    Continua,
    Fim,
    GanchoAtras,
    GanchoFrente,
}

/// Direção explícita da haste, quando o arquivo a traz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirecaoHaste {
    Cima,
    Baixo,
}

/// Início ou fim de uma ligadura de expressão (slur) em uma nota.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarcaLigadura {
    pub numero: u8,
    pub inicio: bool,
}

/// Nota ou pausa dentro de um compasso.
#[derive(Clone, Debug, PartialEq)]
pub struct Nota {
    /// Início dentro do compasso, em semínimas.
    pub posicao: Fracao,
    /// Duração real, em semínimas (zero para notas de adorno).
    pub duracao: Fracao,
    /// `None` = pausa.
    pub altura: Option<Altura>,
    /// Número MIDI, `None` para pausa ou altura fora de 0..=127.
    pub midi: Option<u8>,
    pub voz: u8,
    /// Pauta dentro da parte (1 = primeira).
    pub pauta: u8,
    /// Soa junto com a nota anterior (mesmo início).
    pub acorde: bool,
    /// Nota de adorno (appoggiatura/acciaccatura).
    pub adorno: bool,
    /// Pausa que ocupa o compasso inteiro.
    pub pausa_de_compasso: bool,
    pub figura: Figura,
    pub pontos: u8,
    pub acidente: Option<Acidente>,
    pub ligada_ao_proximo: bool,
    pub ligada_ao_anterior: bool,
    /// Vigas por nível (índice 0 = nível 1). Vazio = sem viga.
    pub vigas: Vec<EstadoViga>,
    pub haste: Option<DirecaoHaste>,
    /// (reais, normais) de `time-modification`, ex.: (3, 2) para tercina.
    pub tuplo: Option<(u32, u32)>,
    pub inicio_tuplo: bool,
    pub fim_tuplo: bool,
    /// Ligaduras de expressão que começam ou terminam nesta nota.
    pub ligaduras: Vec<MarcaLigadura>,
}

impl Nota {
    pub const fn e_pausa(&self) -> bool {
        self.altura.is_none()
    }
}

/// Sinal de clave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SinalClave {
    Sol,
    Fa,
    Do,
    Percussao,
    Tablatura,
}

/// Clave: sinal, linha (1 = inferior) e troca de oitava (ex.: -1 = clave de sol 8vb).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clave {
    pub sinal: SinalClave,
    pub linha: u8,
    pub oitava: i8,
}

impl Clave {
    pub const SOL: Clave = Clave {
        sinal: SinalClave::Sol,
        linha: 2,
        oitava: 0,
    };
    pub const FA: Clave = Clave {
        sinal: SinalClave::Fa,
        linha: 4,
        oitava: 0,
    };

    /// Posição diatônica (Dó4 = 28) da linha inferior da pauta.
    pub fn linha_inferior(self) -> i32 {
        let linha = i32::from(self.linha.clamp(1, 5));
        let base = match self.sinal {
            SinalClave::Sol | SinalClave::Percussao | SinalClave::Tablatura => 32,
            SinalClave::Fa => 24,
            SinalClave::Do => 28,
        };
        base - 2 * (linha - 1) + 7 * i32::from(self.oitava)
    }
}

/// Troca de clave dentro do compasso (posição 0 = no início).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MudancaClave {
    pub pauta: u8,
    pub posicao: Fracao,
    pub clave: Clave,
}

/// Armadura de clave: quintas (-7..=7; negativo = bemóis) e modo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Armadura {
    pub quintas: i8,
    pub menor: bool,
}

/// Fôrma de compasso.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Forma {
    pub batidas: u32,
    pub unidade: u32,
    /// `common` (C) ou `cut` (C cortado) no arquivo.
    pub simbolo: Option<SimboloForma>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimboloForma {
    Comum,
    Cortado,
}

impl Forma {
    /// Duração nominal do compasso, em semínimas.
    pub fn duracao(self) -> Fracao {
        Fracao::nova(i64::from(self.batidas) * 4, i64::from(self.unidade.max(1)))
    }

    /// Duração de um tempo (pulso) em semínimas: composto (6/8, 9/8, 12/8) agrupa de 3 em 3.
    pub fn duracao_do_pulso(self) -> Fracao {
        let unidade = Fracao::nova(4, i64::from(self.unidade.max(1)));
        if self.unidade >= 8 && self.batidas >= 6 && self.batidas.is_multiple_of(3) {
            unidade.vezes(3, 1)
        } else {
            unidade
        }
    }
}

/// Barra no fim do compasso.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Barra {
    Simples,
    Dupla,
    Final,
    Nenhuma,
}

/// Compasso de uma parte.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Compasso {
    pub notas: Vec<Nota>,
    pub claves: Vec<MudancaClave>,
}

/// Parte (instrumento ou voz), com uma ou mais pautas.
#[derive(Clone, Debug, PartialEq)]
pub struct Parte {
    pub id: String,
    pub nome: String,
    pub pautas: u8,
    pub compassos: Vec<Compasso>,
}

/// Informações do compasso comuns a todas as partes (lidas da primeira parte; repetições
/// são a união de todas).
#[derive(Clone, Debug, PartialEq)]
pub struct InfoCompasso {
    /// Rótulo impresso (`number` do MusicXML).
    pub numero: String,
    /// Início, em semínimas desde o começo da peça (ordem escrita, sem repetições).
    pub inicio: Fracao,
    pub duracao: Fracao,
    pub armadura: Armadura,
    pub forma: Forma,
    pub armadura_mudou: bool,
    pub forma_mudou: bool,
    pub repeticao_inicio: bool,
    pub repeticao_fim: bool,
    /// Números das casas (1ª, 2ª vez) a que o compasso pertence; vazio = nenhuma.
    pub casas: Vec<u8>,
    pub barra_final: Barra,
    pub incompleto: bool,
}

/// Mudança de andamento (semínimas por minuto) no compasso `compasso`, na posição `posicao`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Andamento {
    pub compasso: usize,
    pub posicao: Fracao,
    pub bpm: f64,
}

/// Evento de reprodução: uma nota soando, com ligaduras de valor já fundidas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventoReproducao {
    /// Início na execução (repetições expandidas), em semínimas.
    pub inicio: Fracao,
    pub duracao: Fracao,
    pub midi: u8,
    pub parte: usize,
    pub pauta: u8,
    /// Índice do compasso (ordem escrita) onde o evento começa.
    pub compasso: usize,
}

/// Partitura inteira.
#[derive(Clone, Debug, PartialEq)]
pub struct Partitura {
    pub titulo: Option<String>,
    pub compositor: Option<String>,
    pub direitos: Option<String>,
    pub partes: Vec<Parte>,
    pub compassos: Vec<InfoCompasso>,
    pub andamentos: Vec<Andamento>,
}

// --- Leitura -------------------------------------------------------------------------------

fn filho<'a, 'b>(n: Node<'a, 'b>, nome: &str) -> Option<Node<'a, 'b>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == nome)
}

fn texto<'a>(n: Node<'a, '_>, nome: &str) -> Option<&'a str> {
    filho(n, nome).and_then(|c| c.text()).map(str::trim)
}

fn inteiro_de(t: &str) -> Option<i64> {
    let t = t.trim();
    t.parse::<i64>().ok().or_else(|| {
        t.parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && v.abs() < 1e12)
            .map(|v| v.round() as i64)
    })
}

fn texto_do_filho(n: Node<'_, '_>, nome: &str) -> Option<String> {
    texto(n, nome).filter(|t| !t.is_empty()).map(str::to_owned)
}

struct EstadoParte {
    divisoes: i64,
    armadura: Armadura,
    forma: Forma,
    claves: Vec<Clave>,
    pautas: u8,
    casa_atual: Option<Vec<u8>>,
}

impl EstadoParte {
    fn novo() -> Self {
        Self {
            divisoes: 1,
            armadura: Armadura {
                quintas: 0,
                menor: false,
            },
            forma: Forma {
                batidas: 4,
                unidade: 4,
                simbolo: None,
            },
            claves: vec![Clave::SOL],
            pautas: 1,
            casa_atual: None,
        }
    }
}

/// Resultado da leitura de um compasso de uma parte.
struct CompassoLido {
    compasso: Compasso,
    extensao: Fracao,
    armadura: Armadura,
    forma: Forma,
    armadura_mudou: bool,
    forma_mudou: bool,
    repeticao_inicio: bool,
    repeticao_fim: bool,
    casas: Vec<u8>,
    barra_final: Barra,
    numero: String,
    implicito: bool,
    andamentos: Vec<(Fracao, f64)>,
}

fn clave_de(n: Node<'_, '_>) -> Option<Clave> {
    let sinal = match texto(n, "sign")? {
        "G" => SinalClave::Sol,
        "F" => SinalClave::Fa,
        "C" => SinalClave::Do,
        "percussion" => SinalClave::Percussao,
        "TAB" => SinalClave::Tablatura,
        _ => return None,
    };
    let padrao = match sinal {
        SinalClave::Sol => 2,
        SinalClave::Fa => 4,
        SinalClave::Do => 3,
        _ => 3,
    };
    let linha = texto(n, "line")
        .and_then(inteiro_de)
        .map_or(padrao, |l| l.clamp(1, 5) as u8);
    let oitava = texto(n, "clef-octave-change")
        .and_then(inteiro_de)
        .map_or(0, |o| o.clamp(-2, 2) as i8);
    Some(Clave {
        sinal,
        linha,
        oitava,
    })
}

fn andamento_de_direcao(d: Node<'_, '_>) -> Option<f64> {
    let valido = |v: f64| (1.0..=1000.0).contains(&v).then_some(v);
    if let Some(s) = filho(d, "sound")
        && let Some(v) = s
            .attribute("tempo")
            .and_then(|t| t.trim().parse::<f64>().ok())
        && let Some(v) = valido(v)
    {
        return Some(v);
    }
    let m = filho(filho(d, "direction-type")?, "metronome")?;
    let unidade = texto(m, "beat-unit")?;
    let pontuada = filho(m, "beat-unit-dot").is_some();
    let por_minuto: f64 = texto(m, "per-minute")?.parse().ok()?;
    let fig = Figura::de_texto(unidade)?;
    let mut q = fig.duracao_nominal().como_f64();
    if pontuada {
        q *= 1.5;
    }
    valido(por_minuto * q)
}

fn parse_nota(
    n: Node<'_, '_>,
    est: &EstadoParte,
    pos: Fracao,
    ultima: Fracao,
) -> Result<Nota, ErroPartitura> {
    let adorno = filho(n, "grace").is_some();
    let acorde = filho(n, "chord").is_some();
    let pausa = filho(n, "rest");
    let pausa_de_compasso = pausa.is_some_and(|r| r.attribute("measure") == Some("yes"));
    let altura = match filho(n, "pitch") {
        Some(p) => {
            let passo = match texto(p, "step") {
                Some("C") => 0,
                Some("D") => 1,
                Some("E") => 2,
                Some("F") => 3,
                Some("G") => 4,
                Some("A") => 5,
                Some("B") => 6,
                other => {
                    return Err(ErroPartitura::ValorInvalido(format!(
                        "passo de nota '{}'",
                        other.unwrap_or("")
                    )));
                }
            };
            let oitava = texto(p, "octave").and_then(inteiro_de).ok_or_else(|| {
                ErroPartitura::ValorInvalido("oitava de nota ausente ou ilegível".into())
            })?;
            let alt = texto(p, "alter")
                .and_then(|a| a.parse::<f64>().ok())
                .filter(|a| a.is_finite())
                .map_or(0, |a| a.round().clamp(-3.0, 3.0) as i8);
            Some(Altura {
                passo,
                alteracao: alt,
                oitava: oitava.clamp(-2, 12) as i8,
            })
        }
        None => None,
    };
    if altura.is_none() && pausa.is_none() && filho(n, "unpitched").is_none() {
        return Err(ErroPartitura::ValorInvalido(
            "nota sem altura nem pausa".into(),
        ));
    }
    let duracao = match texto(n, "duration").and_then(inteiro_de) {
        _ if adorno => Fracao::ZERO,
        Some(d) if d >= 0 => Fracao::nova(d, est.divisoes),
        Some(_) => return Err(ErroPartitura::ValorInvalido("duração negativa".into())),
        None => Fracao::ZERO,
    };
    let (mut figura, mut pontos) = (None, 0u8);
    if let Some(f) = texto(n, "type").and_then(Figura::de_texto) {
        figura = Some(f);
        pontos = n
            .children()
            .filter(|c| c.is_element() && c.tag_name().name() == "dot")
            .count()
            .min(3) as u8;
    }
    let tuplo = filho(n, "time-modification").and_then(|t| {
        let a = texto(t, "actual-notes").and_then(inteiro_de)?;
        let nn = texto(t, "normal-notes").and_then(inteiro_de)?;
        (a > 0 && nn > 0 && a < 1000 && nn < 1000).then_some((a as u32, nn as u32))
    });
    let (figura, pontos) = match figura {
        _ if pausa_de_compasso => (Figura::Semibreve, 0),
        Some(f) => (f, pontos),
        None => {
            // Sem <type>: deduz da duração (desfazendo tuplo, se houver).
            let base = match tuplo {
                Some((a, nn)) => duracao.vezes(i64::from(a), i64::from(nn)),
                None => duracao,
            };
            Figura::por_duracao(base)
                .or_else(|| (pausa_de_compasso).then_some((Figura::Semibreve, 0)))
                .unwrap_or((Figura::Seminima, 0))
        }
    };
    let mut ligada_prox = false;
    let mut ligada_ant = false;
    for t in n
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "tie")
    {
        match t.attribute("type") {
            Some("start") => ligada_prox = true,
            Some("stop") => ligada_ant = true,
            _ => {}
        }
    }
    let mut vigas: Vec<(usize, EstadoViga)> = Vec::new();
    for b in n
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "beam")
    {
        let nivel = b
            .attribute("number")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(1)
            .clamp(1, 8);
        let estado = match b.text().map(str::trim) {
            Some("begin") => EstadoViga::Inicio,
            Some("continue") => EstadoViga::Continua,
            Some("end") => EstadoViga::Fim,
            Some("backward hook") => EstadoViga::GanchoAtras,
            Some("forward hook") => EstadoViga::GanchoFrente,
            _ => continue,
        };
        vigas.push((nivel, estado));
    }
    vigas.sort_by_key(|(n, _)| *n);
    let mut vetor = Vec::new();
    for (i, (nivel, estado)) in vigas.iter().enumerate() {
        if *nivel == i + 1 {
            vetor.push(*estado);
        } else {
            break;
        }
    }
    let haste = match texto(n, "stem") {
        Some("up") => Some(DirecaoHaste::Cima),
        Some("down") => Some(DirecaoHaste::Baixo),
        _ => None,
    };
    let (mut inicio_tuplo, mut fim_tuplo) = (false, false);
    let mut ligaduras = Vec::new();
    if let Some(nt) = filho(n, "notations") {
        for t in nt.children().filter(Node::is_element) {
            match (t.tag_name().name(), t.attribute("type")) {
                ("tuplet", Some("start")) => inicio_tuplo = true,
                ("tuplet", Some("stop")) => fim_tuplo = true,
                ("slur", Some(tipo @ ("start" | "stop"))) => {
                    ligaduras.push(MarcaLigadura {
                        numero: t
                            .attribute("number")
                            .and_then(|v| v.parse::<u8>().ok())
                            .unwrap_or(1),
                        inicio: tipo == "start",
                    });
                }
                _ => {}
            }
        }
    }
    let voz = texto(n, "voice")
        .and_then(inteiro_de)
        .map_or(1, |v| v.clamp(1, 255) as u8);
    let pauta = texto(n, "staff")
        .and_then(inteiro_de)
        .map_or(1, |v| v.clamp(1, i64::from(MAX_PAUTAS)) as u8);
    Ok(Nota {
        posicao: if acorde { ultima } else { pos },
        duracao,
        midi: altura.and_then(Altura::numero_midi),
        altura,
        voz,
        pauta,
        acorde,
        adorno,
        pausa_de_compasso,
        figura,
        pontos,
        acidente: texto(n, "accidental").and_then(Acidente::de_texto),
        ligada_ao_proximo: ligada_prox,
        ligada_ao_anterior: ligada_ant,
        vigas: vetor,
        haste,
        tuplo,
        inicio_tuplo,
        fim_tuplo,
        ligaduras,
    })
}

fn parse_compasso(
    m: Node<'_, '_>,
    est: &mut EstadoParte,
    notas_total: &mut usize,
) -> Result<CompassoLido, ErroPartitura> {
    let mut compasso = Compasso::default();
    let mut pos = Fracao::ZERO;
    let mut ultima = Fracao::ZERO;
    let mut extensao = Fracao::ZERO;
    let mut armadura_mudou = false;
    let mut forma_mudou = false;
    let mut rep_ini = false;
    let mut rep_fim = false;
    let mut barra = Barra::Simples;
    let mut andamentos = Vec::new();
    let mut casas_do_compasso: Vec<u8> = Vec::new();
    let mut fecha_casa = false;
    for e in m.children().filter(Node::is_element) {
        match e.tag_name().name() {
            "attributes" => {
                if let Some(d) = texto(e, "divisions").and_then(inteiro_de) {
                    if !(1..=1_000_000).contains(&d) {
                        return Err(ErroPartitura::ValorInvalido(format!("divisions = {d}")));
                    }
                    est.divisoes = d;
                }
                if let Some(s) = texto(e, "staves").and_then(inteiro_de) {
                    est.pautas = s.clamp(1, i64::from(MAX_PAUTAS)) as u8;
                }
                if let Some(k) = filho(e, "key")
                    && let Some(q) = texto(k, "fifths").and_then(inteiro_de)
                {
                    let novo = Armadura {
                        quintas: q.clamp(-7, 7) as i8,
                        menor: texto(k, "mode") == Some("minor"),
                    };
                    if novo != est.armadura {
                        armadura_mudou = true;
                    }
                    est.armadura = novo;
                }
                if let Some(t) = filho(e, "time") {
                    let soma = texto(t, "beats").map(|b| {
                        b.split('+')
                            .filter_map(|p| p.trim().parse::<u32>().ok())
                            .sum::<u32>()
                    });
                    let un = texto(t, "beat-type").and_then(inteiro_de);
                    if let (Some(b), Some(u)) = (soma, un)
                        && (1..=64).contains(&b)
                        && (1..=64).contains(&u)
                    {
                        let novo = Forma {
                            batidas: b,
                            unidade: u as u32,
                            simbolo: match t.attribute("symbol") {
                                Some("common") => Some(SimboloForma::Comum),
                                Some("cut") => Some(SimboloForma::Cortado),
                                _ => None,
                            },
                        };
                        if novo != est.forma {
                            forma_mudou = true;
                        }
                        est.forma = novo;
                    }
                }
                for c in e
                    .children()
                    .filter(|c| c.is_element() && c.tag_name().name() == "clef")
                {
                    let numero = c
                        .attribute("number")
                        .and_then(|v| v.parse::<u8>().ok())
                        .unwrap_or(1)
                        .clamp(1, MAX_PAUTAS);
                    if let Some(clave) = clave_de(c) {
                        while est.claves.len() < usize::from(numero) {
                            est.claves.push(Clave::SOL);
                        }
                        est.claves[usize::from(numero) - 1] = clave;
                        compasso.claves.push(MudancaClave {
                            pauta: numero,
                            posicao: pos,
                            clave,
                        });
                    }
                }
            }
            "note" => {
                *notas_total += 1;
                if *notas_total > MAX_NOTAS {
                    return Err(ErroPartitura::LimiteExcedido("número de notas"));
                }
                let nota = parse_nota(e, est, pos, ultima)?;
                if !nota.acorde && !nota.adorno {
                    ultima = pos;
                    pos = pos + nota.duracao;
                    extensao = extensao.max(pos);
                }
                compasso.notas.push(nota);
            }
            "backup" => {
                let d = texto(e, "duration")
                    .and_then(inteiro_de)
                    .filter(|d| *d >= 0)
                    .ok_or_else(|| ErroPartitura::ValorInvalido("backup sem duração".into()))?;
                pos = pos - Fracao::nova(d, est.divisoes);
                if pos < Fracao::ZERO {
                    pos = Fracao::ZERO;
                }
            }
            "forward" => {
                let d = texto(e, "duration")
                    .and_then(inteiro_de)
                    .filter(|d| *d >= 0)
                    .ok_or_else(|| ErroPartitura::ValorInvalido("forward sem duração".into()))?;
                pos = pos + Fracao::nova(d, est.divisoes);
                extensao = extensao.max(pos);
            }
            "direction" => {
                if let Some(bpm) = andamento_de_direcao(e) {
                    andamentos.push((pos, bpm));
                }
            }
            "sound" => {
                if let Some(v) = e
                    .attribute("tempo")
                    .and_then(|t| t.trim().parse::<f64>().ok())
                    && (1.0..=1000.0).contains(&v)
                {
                    andamentos.push((pos, v));
                }
            }
            "barline" => {
                let esquerda = e.attribute("location") == Some("left");
                if let Some(r) = filho(e, "repeat") {
                    match r.attribute("direction") {
                        Some("forward") => rep_ini = true,
                        Some("backward") => rep_fim = true,
                        _ => {}
                    }
                }
                if let Some(en) = filho(e, "ending") {
                    let numeros: Vec<u8> = en
                        .attribute("number")
                        .unwrap_or("")
                        .split([',', ' '])
                        .filter_map(|p| p.trim().trim_end_matches('.').parse::<u8>().ok())
                        .collect();
                    match en.attribute("type") {
                        Some("start") => {
                            est.casa_atual = Some(numeros);
                        }
                        Some("stop") | Some("discontinue") => fecha_casa = true,
                        _ => {}
                    }
                }
                if !esquerda {
                    barra = match texto(e, "bar-style") {
                        Some("light-heavy") => Barra::Final,
                        Some("light-light") | Some("heavy-heavy") => Barra::Dupla,
                        Some("none") => Barra::Nenhuma,
                        _ => barra,
                    };
                }
            }
            _ => {}
        }
    }
    if let Some(c) = &est.casa_atual {
        casas_do_compasso.clone_from(c);
    }
    if fecha_casa {
        est.casa_atual = None;
    }
    let nominal = est.forma.duracao();
    let extensao = if extensao.e_zero() { nominal } else { extensao };
    Ok(CompassoLido {
        compasso,
        extensao,
        armadura: est.armadura,
        forma: est.forma,
        armadura_mudou,
        forma_mudou,
        repeticao_inicio: rep_ini,
        repeticao_fim: rep_fim,
        casas: casas_do_compasso,
        barra_final: barra,
        numero: m.attribute("number").unwrap_or("").to_owned(),
        implicito: m.attribute("implicit") == Some("yes") || m.attribute("number") == Some("0"),
        andamentos,
    })
}

/// Índice do pulso que contém `pos` (pulso > 0).
fn indice_do_pulso(pos: Fracao, pulso: Fracao) -> i64 {
    let q = Fracao::de_i128(
        i128::from(pos.numerador()) * i128::from(pulso.denominador()),
        i128::from(pos.denominador()) * i128::from(pulso.numerador()),
    );
    q.numerador().div_euclid(q.denominador().max(1))
}

/// Vigas automáticas por pulso para arquivos que não trazem `<beam>`.
fn vigar_automaticamente(p: &mut Partitura) {
    let tem_alguma = p
        .partes
        .iter()
        .flat_map(|pt| &pt.compassos)
        .flat_map(|c| &c.notas)
        .any(|n| !n.vigas.is_empty());
    if tem_alguma {
        return;
    }
    for parte in &mut p.partes {
        for (i, comp) in parte.compassos.iter_mut().enumerate() {
            let Some(pulso) = p.compassos.get(i).map(|c| c.forma.duracao_do_pulso()) else {
                continue;
            };
            if pulso.numerador() <= 0 {
                continue;
            }
            let mut chaves: Vec<(u8, u8)> = comp.notas.iter().map(|n| (n.pauta, n.voz)).collect();
            chaves.sort_unstable();
            chaves.dedup();
            for (pauta, voz) in chaves {
                vigar_voz(comp, pulso, pauta, voz);
            }
        }
    }
}

/// Vigas automáticas por pulso da voz `voz` na pauta `pauta` de um compasso (as vigas antigas
/// dessa voz devem ter sido limpas antes). Notas de acorde herdam as vigas da principal.
pub(crate) fn vigar_voz(comp: &mut Compasso, pulso: Fracao, pauta: u8, voz: u8) {
    let mut idx: Vec<usize> = comp
        .notas
        .iter()
        .enumerate()
        .filter(|(_, n)| n.pauta == pauta && n.voz == voz && !n.adorno && !n.acorde)
        .map(|(i, _)| i)
        .collect();
    idx.sort_by_key(|i| comp.notas[*i].posicao);
    let mut grupo: Vec<usize> = Vec::new();
    let mut pulso_atual: Option<i64> = None;
    for i in idx {
        let n = &comp.notas[i];
        let elegivel = !n.e_pausa() && n.figura.bandeirolas() >= 1;
        let b = indice_do_pulso(n.posicao, pulso);
        if (!elegivel || pulso_atual.is_some_and(|p0| p0 != b)) && !grupo.is_empty() {
            if grupo.len() >= 2 {
                aplicar_vigas(&grupo, &mut comp.notas);
            }
            grupo.clear();
        }
        if elegivel {
            grupo.push(i);
            pulso_atual = Some(b);
        } else {
            pulso_atual = None;
        }
    }
    if grupo.len() >= 2 {
        aplicar_vigas(&grupo, &mut comp.notas);
    }
    // Notas de acorde herdam as vigas da nota principal.
    for k in 0..comp.notas.len() {
        if comp.notas[k].acorde {
            let (pa, vo, po) = (
                comp.notas[k].pauta,
                comp.notas[k].voz,
                comp.notas[k].posicao,
            );
            if pa != pauta || vo != voz {
                continue;
            }
            let herdadas = comp
                .notas
                .iter()
                .find(|o| !o.acorde && o.pauta == pa && o.voz == vo && o.posicao == po)
                .map(|o| o.vigas.clone());
            if let Some(v) = herdadas {
                comp.notas[k].vigas = v;
            }
        }
    }
}

fn aplicar_vigas(grupo: &[usize], notas: &mut [Nota]) {
    let max_nivel = grupo
        .iter()
        .map(|i| notas[*i].figura.bandeirolas())
        .max()
        .unwrap_or(1);
    for &i in grupo {
        notas[i].vigas.clear();
    }
    for nivel in 1..=max_nivel {
        let figuras: Vec<u8> = grupo
            .iter()
            .map(|i| notas[*i].figura.bandeirolas())
            .collect();
        let tem = |k: usize| figuras[k] >= nivel;
        for k in 0..grupo.len() {
            if !tem(k) {
                continue;
            }
            let ant = k > 0 && tem(k - 1);
            let prox = k + 1 < grupo.len() && tem(k + 1);
            let estado = match (ant, prox) {
                (false, true) => EstadoViga::Inicio,
                (true, true) => EstadoViga::Continua,
                (true, false) => EstadoViga::Fim,
                (false, false) => {
                    if k == 0 {
                        EstadoViga::GanchoFrente
                    } else {
                        EstadoViga::GanchoAtras
                    }
                }
            };
            notas[grupo[k]].vigas.push(estado);
        }
    }
}

/// Relê um único compasso (`<measure>...</measure>`) com o estado musical que valia ao seu
/// redor: o serviço de documento usa para refazer o modelo de tocar do compasso editado. O
/// resultado é o do leitor de sempre (mesmo código), sem as vigas automáticas (que dependem da
/// partitura inteira; ver [`vigar_voz`]).
pub(crate) fn reler_compasso(
    xml: &str,
    divisoes: i64,
    armadura: Armadura,
    forma: Forma,
) -> Result<Compasso, ErroPartitura> {
    let doc = crate::xml_seguro::analisar(xml)?;
    let raiz = doc.root_element();
    if raiz.tag_name().name() != "measure" {
        return Err(ErroPartitura::RaizNaoSuportada(
            raiz.tag_name().name().to_owned(),
        ));
    }
    let mut est = EstadoParte::novo();
    est.divisoes = divisoes;
    est.armadura = armadura;
    est.forma = forma;
    let mut total = 0usize;
    Ok(parse_compasso(raiz, &mut est, &mut total)?.compasso)
}

impl Partitura {
    /// Lê um texto MusicXML `score-partwise`.
    pub fn de_musicxml(texto_xml: &str) -> Result<Self, ErroPartitura> {
        if texto_xml.len() > TAMANHO_MAXIMO_BYTES {
            return Err(ErroPartitura::GrandeDemais(texto_xml.len()));
        }
        // DTD interno permitido, entidade externa nunca resolvida, expansão/nós/profundidade
        // limitados: a política inteira vive em `xml_seguro` (ticket 11).
        let doc = crate::xml_seguro::analisar(texto_xml)?;
        let raiz = doc.root_element();
        if raiz.tag_name().name() != "score-partwise" {
            return Err(ErroPartitura::RaizNaoSuportada(
                raiz.tag_name().name().to_owned(),
            ));
        }
        let titulo = filho(raiz, "movement-title")
            .and_then(|_| texto_do_filho(raiz, "movement-title"))
            .or_else(|| filho(raiz, "work").and_then(|w| texto_do_filho(w, "work-title")));
        let ident = filho(raiz, "identification");
        let compositor = ident.and_then(|i| {
            i.children()
                .filter(|c| c.is_element() && c.tag_name().name() == "creator")
                .find(|c| c.attribute("type") == Some("composer"))
                .and_then(|c| c.text())
                .map(|t| t.trim().to_owned())
        });
        let direitos = ident.and_then(|i| texto_do_filho(i, "rights"));

        let mut nomes: HashMap<String, String> = HashMap::new();
        if let Some(lista) = filho(raiz, "part-list") {
            for sp in lista
                .children()
                .filter(|c| c.is_element() && c.tag_name().name() == "score-part")
            {
                if let Some(id) = sp.attribute("id") {
                    nomes.insert(
                        id.to_owned(),
                        texto_do_filho(sp, "part-name").unwrap_or_default(),
                    );
                }
            }
        }

        let mut partes: Vec<Parte> = Vec::new();
        let mut lidos: Vec<Vec<CompassoLido>> = Vec::new();
        let mut notas_total = 0usize;
        for pn in raiz
            .children()
            .filter(|c| c.is_element() && c.tag_name().name() == "part")
        {
            if partes.len() >= MAX_PARTES {
                return Err(ErroPartitura::LimiteExcedido("número de partes"));
            }
            let mut est = EstadoParte::novo();
            let mut compassos = Vec::new();
            for m in pn
                .children()
                .filter(|c| c.is_element() && c.tag_name().name() == "measure")
            {
                if compassos.len() >= MAX_COMPASSOS {
                    return Err(ErroPartitura::LimiteExcedido("número de compassos"));
                }
                compassos.push(parse_compasso(m, &mut est, &mut notas_total)?);
            }
            if compassos.is_empty() {
                continue;
            }
            let id = pn.attribute("id").unwrap_or("").to_owned();
            partes.push(Parte {
                nome: nomes.get(&id).cloned().unwrap_or_default(),
                id,
                pautas: est.pautas,
                compassos: Vec::new(),
            });
            lidos.push(compassos);
        }
        if partes.is_empty() {
            return Err(ErroPartitura::SemPartes);
        }
        let n_comp = lidos.iter().map(Vec::len).max().unwrap_or(0);

        // Informação global dos compassos.
        let mut compassos: Vec<InfoCompasso> = Vec::with_capacity(n_comp);
        let mut andamentos: Vec<Andamento> = Vec::new();
        let mut inicio = Fracao::ZERO;
        let mut ultimo = (
            Armadura {
                quintas: 0,
                menor: false,
            },
            Forma {
                batidas: 4,
                unidade: 4,
                simbolo: None,
            },
        );
        for i in 0..n_comp {
            let base = lidos[0]
                .get(i)
                .or_else(|| lidos.iter().find_map(|l| l.get(i)));
            let (armadura, forma, a_mudou, f_mudou, numero, implicito) = match base {
                Some(b) => (
                    b.armadura,
                    b.forma,
                    b.armadura_mudou || i == 0,
                    b.forma_mudou || i == 0,
                    b.numero.clone(),
                    b.implicito,
                ),
                None => (ultimo.0, ultimo.1, false, false, String::new(), false),
            };
            ultimo = (armadura, forma);
            let extensao = lidos
                .iter()
                .filter_map(|l| l.get(i))
                .map(|c| c.extensao)
                .max()
                .unwrap_or(Fracao::ZERO);
            let nominal = forma.duracao();
            let (duracao, incompleto) = if implicito && !extensao.e_zero() && extensao < nominal {
                (extensao, true)
            } else {
                (extensao.max(nominal), false)
            };
            let uniao = |f: fn(&CompassoLido) -> bool| lidos.iter().filter_map(|l| l.get(i)).any(f);
            let casas = lidos
                .iter()
                .filter_map(|l| l.get(i))
                .map(|c| c.casas.clone())
                .find(|c| !c.is_empty())
                .unwrap_or_default();
            let barra_final = lidos
                .iter()
                .filter_map(|l| l.get(i))
                .map(|c| c.barra_final)
                .find(|b| *b != Barra::Simples)
                .unwrap_or(Barra::Simples);
            compassos.push(InfoCompasso {
                numero: if numero.is_empty() {
                    (i + 1).to_string()
                } else {
                    numero
                },
                inicio,
                duracao,
                armadura,
                forma,
                armadura_mudou: a_mudou,
                forma_mudou: f_mudou,
                repeticao_inicio: uniao(|c| c.repeticao_inicio),
                repeticao_fim: uniao(|c| c.repeticao_fim),
                casas,
                barra_final,
                incompleto,
            });
            for l in &lidos {
                if let Some(c) = l.get(i) {
                    for (pos, bpm) in &c.andamentos {
                        if !andamentos
                            .iter()
                            .any(|a| a.compasso == i && a.posicao == *pos)
                        {
                            andamentos.push(Andamento {
                                compasso: i,
                                posicao: *pos,
                                bpm: *bpm,
                            });
                        }
                    }
                }
            }
            inicio = inicio + duracao;
        }
        andamentos.sort_by(|a, b| a.compasso.cmp(&b.compasso).then(a.posicao.cmp(&b.posicao)));

        for (parte, l) in partes.iter_mut().zip(lidos) {
            let mut v: Vec<Compasso> = l.into_iter().map(|c| c.compasso).collect();
            v.resize_with(n_comp, Compasso::default);
            parte.compassos = v;
        }
        let mut p = Partitura {
            titulo,
            compositor,
            direitos,
            partes,
            compassos,
            andamentos,
        };
        vigar_automaticamente(&mut p);
        Ok(p)
    }

    /// Total de notas e pausas.
    pub fn total_de_notas(&self) -> usize {
        self.partes
            .iter()
            .flat_map(|p| &p.compassos)
            .map(|c| c.notas.len())
            .sum()
    }

    /// Total de pautas somando todas as partes.
    pub fn total_de_pautas(&self) -> usize {
        self.partes.iter().map(|p| usize::from(p.pautas)).sum()
    }

    /// Início de cada compasso na ordem escrita (sem repetições), em semínimas.
    pub fn inicios_dos_compassos(&self) -> Vec<Fracao> {
        self.compassos.iter().map(|c| c.inicio).collect()
    }

    /// Duração da peça escrita (sem repetições), em semínimas.
    pub fn duracao_escrita(&self) -> Fracao {
        self.compassos
            .last()
            .map_or(Fracao::ZERO, |c| c.inicio + c.duracao)
    }

    /// Ordem em que os compassos são tocados, expandindo repetições simples e casas 1/2.
    pub fn ordem_de_execucao(&self) -> Vec<usize> {
        let n = self.compassos.len();
        let mut ordem = Vec::with_capacity(n + 8);
        let mut feitos: HashSet<usize> = HashSet::new();
        let (mut i, mut ini, mut passagem) = (0usize, 0usize, 1u8);
        let mut guarda = 0usize;
        while i < n && guarda < n * 4 + 16 {
            guarda += 1;
            let c = &self.compassos[i];
            if !c.casas.is_empty() && !c.casas.contains(&passagem) {
                i += 1;
                continue;
            }
            if c.repeticao_inicio && !(ini == i && passagem > 1) {
                ini = i;
                passagem = 1;
            }
            ordem.push(i);
            if c.repeticao_fim && feitos.insert(i) {
                passagem = passagem.saturating_add(1);
                i = ini;
                continue;
            }
            i += 1;
        }
        ordem
    }

    /// Início (execução, com repetições) de cada compasso tocado: `(início, compasso)`.
    pub fn compassos_na_execucao(&self) -> Vec<(Fracao, usize)> {
        let mut acum = Fracao::ZERO;
        self.ordem_de_execucao()
            .into_iter()
            .map(|i| {
                let r = (acum, i);
                acum = acum + self.compassos[i].duracao;
                r
            })
            .collect()
    }

    /// Duração da execução (com repetições), em semínimas.
    pub fn duracao_da_execucao(&self) -> Fracao {
        self.compassos_na_execucao()
            .last()
            .map_or(Fracao::ZERO, |(ini, i)| *ini + self.compassos[*i].duracao)
    }

    /// Eventos de reprodução ordenados por início, com ligaduras fundidas e repetições
    /// expandidas. Pausas e notas de adorno não geram evento.
    pub fn eventos_de_reproducao(&self) -> Vec<EventoReproducao> {
        let mut eventos: Vec<EventoReproducao> = Vec::new();
        let mut abertas: HashMap<(usize, u8, u8), usize> = HashMap::new();
        for (inicio_comp, ic) in self.compassos_na_execucao() {
            for (ip, parte) in self.partes.iter().enumerate() {
                let Some(comp) = parte.compassos.get(ic) else {
                    continue;
                };
                for nota in &comp.notas {
                    let Some(midi) = nota.midi else { continue };
                    if nota.adorno {
                        continue;
                    }
                    let inicio = inicio_comp + nota.posicao;
                    let chave = (ip, midi, nota.pauta);
                    if nota.ligada_ao_anterior
                        && let Some(&k) = abertas.get(&chave)
                        && eventos[k].inicio + eventos[k].duracao == inicio
                    {
                        eventos[k].duracao = eventos[k].duracao + nota.duracao;
                        if !nota.ligada_ao_proximo {
                            abertas.remove(&chave);
                        }
                        continue;
                    }
                    eventos.push(EventoReproducao {
                        inicio,
                        duracao: nota.duracao,
                        midi,
                        parte: ip,
                        pauta: nota.pauta,
                        compasso: ic,
                    });
                    if nota.ligada_ao_proximo {
                        abertas.insert(chave, eventos.len() - 1);
                    } else {
                        abertas.remove(&chave);
                    }
                }
            }
        }
        eventos.sort_by(|a, b| {
            a.inicio
                .cmp(&b.inicio)
                .then(a.midi.cmp(&b.midi))
                .then(a.parte.cmp(&b.parte))
        });
        eventos
    }

    /// Eventos de reprodução **sem uníssonos**: onde duas ou mais notas de mesma altura MIDI
    /// começam no mesmo instante (típico das duas mãos tocando a mesma tecla, uma em cada
    /// pauta), fica um só evento, com a maior duração. Do grupo fundido sobrevivem `parte`,
    /// `pauta` e `compasso` do evento de menor (parte, pauta), isto é, a pauta de cima.
    /// Notas diferentes no mesmo instante (acordes) são mantidas. A ordem é a de
    /// [`Self::eventos_de_reproducao`]; é esta lista que dá o `indice` dos Alinhamentos.
    pub fn eventos_sem_unissono(&self) -> Vec<EventoReproducao> {
        let mut fundidos: Vec<EventoReproducao> = Vec::new();
        for e in self.eventos_de_reproducao() {
            // A lista vem ordenada por (início, altura): o candidato a uníssono é o último.
            match fundidos.last_mut() {
                Some(u) if u.inicio == e.inicio && u.midi == e.midi => {
                    let duracao = u.duracao.max(e.duracao);
                    if (e.parte, e.pauta) < (u.parte, u.pauta) {
                        *u = e;
                    }
                    u.duracao = duracao;
                }
                _ => fundidos.push(e),
            }
        }
        fundidos
    }

    /// Andamento inicial declarado, em semínimas por minuto.
    pub fn andamento_inicial(&self) -> Option<f64> {
        self.andamentos.first().map(|a| a.bpm)
    }

    /// Mapa de andamento sobre a execução (repetições expandidas).
    pub fn mapa_de_andamento(&self) -> MapaAndamento {
        let mut trechos: Vec<(f64, f64)> = Vec::new();
        let inicial = self.andamento_inicial();
        for (inicio_comp, ic) in self.compassos_na_execucao() {
            for a in self.andamentos.iter().filter(|a| a.compasso == ic) {
                let q = (inicio_comp + a.posicao).como_f64();
                match trechos.last() {
                    Some((q0, _)) if *q0 >= q => {}
                    _ => trechos.push((q, a.bpm)),
                }
            }
        }
        if trechos.first().is_none_or(|(q, _)| *q > 0.0)
            && let Some(b) = inicial
        {
            trechos.insert(0, (0.0, b));
        }
        MapaAndamento { trechos }
    }
}

/// Andamento por trecho: `(início em semínimas, semínimas por minuto)`.
#[derive(Clone, Debug, PartialEq)]
pub struct MapaAndamento {
    trechos: Vec<(f64, f64)>,
}

impl MapaAndamento {
    /// Andamento no começo da peça, se declarado.
    pub fn inicial(&self) -> Option<f64> {
        self.trechos.first().map(|(_, b)| *b)
    }

    /// Segundos até `quarters` semínimas. `bpm_padrao` vale onde o arquivo não declara
    /// andamento; `escala` multiplica o andamento (1,0 = como escrito).
    pub fn segundos(&self, quarters: f64, bpm_padrao: f64, escala: f64) -> f64 {
        let escala = if escala.is_finite() && escala > 0.0 {
            escala
        } else {
            1.0
        };
        let mut atual = bpm_padrao.max(1.0);
        let mut q0 = 0.0;
        let mut t = 0.0;
        for (q, bpm) in &self.trechos {
            if *q >= quarters {
                break;
            }
            if *q > q0 {
                t += (q - q0) * 60.0 / (atual * escala);
                q0 = *q;
            }
            atual = *bpm;
        }
        t + (quarters - q0).max(0.0) * 60.0 / (atual * escala)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 4.0 Partwise//EN" "http://www.musicxml.org/dtds/partwise.dtd">
<score-partwise version="4.0">
  <work><work-title>Peça de teste</work-title></work>
  <identification><creator type="composer">Anônimo</creator><rights>CC0</rights></identification>
  <part-list>
    <score-part id="P1"><part-name>Piano</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>2</divisions>
        <key><fifths>1</fifths><mode>major</mode></key>
        <time><beats>3</beats><beat-type>4</beat-type></time>
        <staves>2</staves>
        <clef number="1"><sign>G</sign><line>2</line></clef>
        <clef number="2"><sign>F</sign><line>4</line></clef>
      </attributes>
      <direction placement="above"><direction-type><metronome><beat-unit>quarter</beat-unit><per-minute>90</per-minute></metronome></direction-type><sound tempo="90"/></direction>
      <note><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type><staff>1</staff></note>
      <note><chord/><pitch><step>C</step><alter>1</alter><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>quarter</type><accidental>sharp</accidental><staff>1</staff></note>
      <note><pitch><step>A</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>eighth</type><staff>1</staff></note>
      <note><pitch><step>G</step><octave>4</octave></pitch><duration>3</duration><voice>1</voice><type>quarter</type><dot/><tie type="start"/><staff>1</staff></note>
      <backup><duration>6</duration></backup>
      <note><pitch><step>C</step><octave>3</octave></pitch><duration>6</duration><voice>2</voice><type>half</type><dot/><staff>2</staff></note>
    </measure>
    <measure number="2">
      <note><pitch><step>G</step><octave>4</octave></pitch><duration>6</duration><voice>1</voice><type>half</type><dot/><tie type="stop"/><staff>1</staff></note>
      <backup><duration>6</duration></backup>
      <note><rest measure="yes"/><duration>6</duration><voice>2</voice><staff>2</staff></note>
      <barline location="right"><bar-style>light-heavy</bar-style></barline>
    </measure>
  </part>
</score-partwise>"#;

    fn arq(n: &str) -> &'static str {
        match n {
            "wiegenlied" => {
                include_str!("../../../assets/partituras/brahms-wiegenlied-op49-4.musicxml")
            }
            "heidenroeslein" => {
                include_str!("../../../assets/partituras/schubert-heidenroeslein-d257.musicxml")
            }
            _ => include_str!("../../../assets/partituras/faure-barcarolle-op7-3.musicxml"),
        }
    }

    #[test]
    fn fracao_normaliza_compara_e_soma() {
        let a = Fracao::nova(2, -4);
        assert_eq!((a.numerador(), a.denominador()), (-1, 2));
        assert_eq!(Fracao::nova(1, 3) + Fracao::nova(1, 6), Fracao::nova(1, 2));
        assert!(Fracao::nova(1, 3) < Fracao::nova(1, 2));
        assert_eq!(Fracao::nova(5, 0), Fracao::ZERO);
        assert_eq!(Fracao::nova(3, 2).vezes(2, 3), Fracao::inteiro(1));
    }

    #[test]
    fn altura_para_midi() {
        let m = |passo, alt, oit| Altura {
            passo,
            alteracao: alt,
            oitava: oit,
        };
        assert_eq!(m(0, 0, 4).numero_midi(), Some(60));
        assert_eq!(m(5, 0, 4).numero_midi(), Some(69));
        assert_eq!(m(0, 1, 5).numero_midi(), Some(73));
        assert_eq!(m(6, -1, 3).numero_midi(), Some(58));
        assert_eq!(m(0, 0, -1).numero_midi(), Some(0));
        assert_eq!(m(4, 0, 9).numero_midi(), Some(127));
        assert_eq!(m(5, 0, 9).numero_midi(), None);
    }

    #[test]
    fn clave_linha_inferior() {
        assert_eq!(Clave::SOL.linha_inferior(), 30); // Mi4
        assert_eq!(Clave::FA.linha_inferior(), 18); // Sol2
        let do3 = Clave {
            sinal: SinalClave::Do,
            linha: 3,
            oitava: 0,
        };
        assert_eq!(do3.linha_inferior(), 24); // Fá3
        let sol8vb = Clave {
            sinal: SinalClave::Sol,
            linha: 2,
            oitava: -1,
        };
        assert_eq!(sol8vb.linha_inferior(), 23);
    }

    #[test]
    fn le_fixture_com_duas_pautas_armadura_e_forma() {
        let p = Partitura::de_musicxml(FIXTURE).expect("fixture válida");
        assert_eq!(p.titulo.as_deref(), Some("Peça de teste"));
        assert_eq!(p.compositor.as_deref(), Some("Anônimo"));
        assert_eq!(p.partes.len(), 1);
        assert_eq!(p.partes[0].pautas, 2);
        assert_eq!(p.compassos.len(), 2);
        let c0 = &p.compassos[0];
        assert_eq!(c0.armadura.quintas, 1);
        assert!(!c0.armadura.menor);
        assert_eq!((c0.forma.batidas, c0.forma.unidade), (3, 4));
        assert_eq!(c0.duracao, Fracao::inteiro(3));
        assert_eq!(p.compassos[1].inicio, Fracao::inteiro(3));
        assert_eq!(p.compassos[1].barra_final, Barra::Final);
        let claves = &p.partes[0].compassos[0].claves;
        assert_eq!(claves.len(), 2);
        assert_eq!(claves[0].clave, Clave::SOL);
        assert_eq!(claves[1].clave, Clave::FA);
        assert_eq!(p.andamento_inicial(), Some(90.0));
    }

    #[test]
    fn notas_acorde_pontos_ligadura_e_pausa() {
        let p = Partitura::de_musicxml(FIXTURE).expect("fixture válida");
        let c = &p.partes[0].compassos[0];
        let n = &c.notas;
        assert_eq!(n.len(), 5);
        assert_eq!(n[0].midi, Some(64));
        assert_eq!(n[0].duracao, Fracao::inteiro(1));
        assert!(!n[0].acorde);
        assert_eq!(n[1].midi, Some(73));
        assert!(n[1].acorde);
        assert_eq!(n[1].posicao, n[0].posicao);
        assert_eq!(n[1].acidente, Some(Acidente::Sustenido));
        assert_eq!(n[2].posicao, Fracao::inteiro(1));
        assert_eq!(n[2].figura, Figura::Colcheia);
        assert_eq!(n[3].posicao, Fracao::nova(3, 2));
        assert_eq!(n[3].duracao, Fracao::nova(3, 2));
        assert_eq!(n[3].pontos, 1);
        assert!(n[3].ligada_ao_proximo);
        assert_eq!((n[4].pauta, n[4].voz, n[4].midi), (2, 2, Some(48)));
        assert_eq!(n[4].posicao, Fracao::ZERO);
        let c2 = &p.partes[0].compassos[1];
        assert!(c2.notas[0].ligada_ao_anterior);
        assert!(c2.notas[1].e_pausa());
        assert!(c2.notas[1].pausa_de_compasso);
        assert_eq!(c2.notas[1].figura, Figura::Semibreve);
    }

    #[test]
    fn eventos_de_reproducao_fundem_ligadura_e_ordenam() {
        let p = Partitura::de_musicxml(FIXTURE).expect("fixture válida");
        let ev = p.eventos_de_reproducao();
        // E4, C#5 (acorde), A4, G4 ligada (1,5 + 3 = 4,5), C3.
        assert_eq!(ev.len(), 5);
        let tuplas: Vec<(Fracao, u8)> = ev.iter().map(|e| (e.inicio, e.midi)).collect();
        assert_eq!(
            tuplas,
            vec![
                (Fracao::ZERO, 48),
                (Fracao::ZERO, 64),
                (Fracao::ZERO, 73),
                (Fracao::inteiro(1), 69),
                (Fracao::nova(3, 2), 67),
            ]
        );
        let sol = ev.iter().find(|e| e.midi == 67).expect("sol");
        assert_eq!(sol.duracao, Fracao::nova(9, 2));
        assert_eq!(sol.compasso, 0);
        let do3 = ev.iter().find(|e| e.midi == 48).expect("dó3");
        assert_eq!(do3.duracao, Fracao::nova(3, 1));
        for w in ev.windows(2) {
            assert!(w[0].inicio <= w[1].inicio);
        }
        assert_eq!(
            p.inicios_dos_compassos(),
            vec![Fracao::ZERO, Fracao::inteiro(3)]
        );
        assert_eq!(p.duracao_da_execucao(), Fracao::inteiro(6));
    }

    #[test]
    fn vigas_automaticas_quando_o_arquivo_nao_tem() {
        let xml = r#"<score-partwise><part-list><score-part id="P1"><part-name>x</part-name></score-part></part-list>
<part id="P1"><measure number="1"><attributes><divisions>2</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes>
<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>eighth</type></note>
<note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><type>eighth</type></note>
<note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><type>eighth</type></note>
<note><pitch><step>F</step><octave>4</octave></pitch><duration>1</duration><type>eighth</type></note>
</measure></part></score-partwise>"#;
        let p = Partitura::de_musicxml(xml).expect("válida");
        let n = &p.partes[0].compassos[0].notas;
        assert_eq!(n[0].vigas, vec![EstadoViga::Inicio]);
        assert_eq!(n[1].vigas, vec![EstadoViga::Fim]);
        assert_eq!(n[2].vigas, vec![EstadoViga::Inicio]);
        assert_eq!(n[3].vigas, vec![EstadoViga::Fim]);
    }

    #[test]
    fn repeticao_com_casas_expande_a_execucao() {
        let compasso = |n: u32, extra: &str| {
            format!(
                r#"<measure number="{n}">{}<note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note>{extra}</measure>"#,
                if n == 1 {
                    "<attributes><divisions>1</divisions></attributes>"
                } else {
                    ""
                }
            )
        };
        let xml = format!(
            r#"<score-partwise><part-list><score-part id="P1"><part-name>x</part-name></score-part></part-list><part id="P1">{}{}{}{}</part></score-partwise>"#,
            compasso(
                1,
                r#"<barline location="left"><repeat direction="forward"/></barline>"#
            ),
            compasso(
                2,
                r#"<barline location="left"><ending number="1" type="start"/></barline><barline location="right"><ending number="1" type="stop"/><repeat direction="backward"/></barline>"#
            ),
            compasso(
                3,
                r#"<barline location="left"><ending number="2" type="start"/></barline><barline location="right"><ending number="2" type="stop"/></barline>"#
            ),
            compasso(4, ""),
        );
        let p = Partitura::de_musicxml(&xml).expect("válida");
        assert_eq!(p.ordem_de_execucao(), vec![0, 1, 0, 2, 3]);
        let ex = p.compassos_na_execucao();
        assert_eq!(ex[3], (Fracao::inteiro(12), 2));
        assert_eq!(p.duracao_da_execucao(), Fracao::inteiro(20));
        assert_eq!(p.eventos_de_reproducao().len(), 5);
    }

    #[test]
    fn mapa_de_andamento_converte_para_segundos() {
        let p = Partitura::de_musicxml(FIXTURE).expect("fixture válida");
        let m = p.mapa_de_andamento();
        assert_eq!(m.inicial(), Some(90.0));
        // 3 semínimas a 90 bpm = 2 s; com escala 2 cai para 1 s.
        assert!((m.segundos(3.0, 100.0, 1.0) - 2.0).abs() < 1e-9);
        assert!((m.segundos(3.0, 100.0, 2.0) - 1.0).abs() < 1e-9);
        let vazio = MapaAndamento { trechos: vec![] };
        assert!((vazio.segundos(2.0, 120.0, 1.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn recusa_arquivos_invalidos_sem_panico() {
        assert!(matches!(
            Partitura::de_musicxml("isto não é xml"),
            Err(ErroPartitura::XmlInvalido(_))
        ));
        assert!(matches!(
            Partitura::de_musicxml("<score-timewise/>"),
            Err(ErroPartitura::RaizNaoSuportada(_))
        ));
        assert!(matches!(
            Partitura::de_musicxml("<score-partwise><part-list/></score-partwise>"),
            Err(ErroPartitura::SemPartes)
        ));
        assert!(matches!(
            Partitura::de_musicxml(""),
            Err(ErroPartitura::XmlInvalido(_))
        ));
        let grande = "x".repeat(TAMANHO_MAXIMO_BYTES + 1);
        assert!(matches!(
            Partitura::de_musicxml(&grande),
            Err(ErroPartitura::GrandeDemais(_))
        ));
        // Passo inválido.
        let ruim = r#"<score-partwise><part id="P1"><measure number="1"><note><pitch><step>H</step><octave>4</octave></pitch><duration>1</duration></note></measure></part></score-partwise>"#;
        assert!(matches!(
            Partitura::de_musicxml(ruim),
            Err(ErroPartitura::ValorInvalido(_))
        ));
        // Divisions zero.
        let ruim = r#"<score-partwise><part id="P1"><measure number="1"><attributes><divisions>0</divisions></attributes></measure></part></score-partwise>"#;
        assert!(Partitura::de_musicxml(ruim).is_err());
        // Aninhamento profundo e bomba de entidades não causam pânico.
        let fundo = format!("{}{}", "<a>".repeat(5000), "</a>".repeat(5000));
        assert!(Partitura::de_musicxml(&fundo).is_err());
        let bomba = r#"<?xml version="1.0"?><!DOCTYPE s [<!ENTITY a "aaaaaaaaaa"><!ENTITY b "&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;"><!ENTITY c "&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;"><!ENTITY d "&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;"><!ENTITY e "&d;&d;&d;&d;&d;&d;&d;&d;&d;&d;"><!ENTITY f "&e;&e;&e;&e;&e;&e;&e;&e;&e;&e;"><!ENTITY g "&f;&f;&f;&f;&f;&f;&f;&f;&f;&f;"><!ENTITY h "&g;&g;&g;&g;&g;&g;&g;&g;&g;&g;">]><score-partwise>&h;&h;&h;&h;&h;&h;&h;&h;&h;&h;</score-partwise>"#;
        assert!(Partitura::de_musicxml(bomba).is_err());
    }

    #[test]
    fn entidade_externa_nao_le_o_disco() {
        let xml = r#"<?xml version="1.0"?><!DOCTYPE score-partwise [<!ENTITY x SYSTEM "file:///etc/passwd">]><score-partwise><work><work-title>&x;</work-title></work></score-partwise>"#;
        // Sem partes: erro; e nada do disco é lido (roxmltree não resolve entidades externas).
        let r = Partitura::de_musicxml(xml);
        assert!(r.is_err());
        if let Err(e) = r {
            assert!(!e.to_string().contains("root:"));
        }
    }

    #[test]
    fn arquivos_empacotados_tem_tres_pautas_e_fôrmas_esperadas() {
        for (nome, quintas, batidas, unidade, compassos) in [
            ("wiegenlied", -3, 3, 4, 19),
            ("heidenroeslein", 1, 2, 4, 16),
            ("barcarolle", -4, 6, 8, 42),
        ] {
            let p = Partitura::de_musicxml(arq(nome)).unwrap_or_else(|e| panic!("{nome}: {e}"));
            assert_eq!(p.partes.len(), 2, "{nome}");
            assert_eq!(p.partes[0].pautas, 1, "{nome}");
            assert_eq!(p.partes[1].pautas, 2, "{nome}");
            assert_eq!(p.total_de_pautas(), 3);
            assert_eq!(p.compassos.len(), compassos, "{nome}");
            assert_eq!(p.compassos[0].armadura.quintas, quintas, "{nome}");
            assert_eq!(
                (p.compassos[0].forma.batidas, p.compassos[0].forma.unidade),
                (batidas, unidade),
                "{nome}"
            );
            assert_eq!(p.direitos.as_deref().map(|d| d.contains("CC0")), Some(true));
            let claves = &p.partes[1].compassos[0].claves;
            assert_eq!(claves.len(), 2, "{nome}");
            assert_eq!(claves[0].clave.sinal, SinalClave::Sol);
            assert_eq!(claves[1].clave.sinal, SinalClave::Fa);
            assert!(!p.eventos_de_reproducao().is_empty());
            assert!(p.total_de_notas() > 100);
            // Cada compasso completo preenche a fôrma em cada pauta de piano (voz 1).
            let nominal = p.compassos[1].forma.duracao();
            assert!(p.compassos[1].duracao >= nominal);
        }
    }

    #[test]
    fn heidenroeslein_primeiro_compasso_e_tempo() {
        let p = Partitura::de_musicxml(arq("heidenroeslein")).expect("válida");
        assert_eq!(
            p.andamento_inicial().map(f64::round),
            Some(p.andamentos[0].bpm.round())
        );
        assert!(p.andamentos[0].bpm > 40.0 && p.andamentos[0].bpm < 200.0);
        // Repetições expandem a execução além da escrita.
        assert!(p.duracao_da_execucao() >= p.duracao_escrita());
        // Cada compasso (exceto o do início) soma 2 semínimas na voz 1 da voz cantada.
        let vocal = &p.partes[0];
        for (i, c) in vocal.compassos.iter().enumerate().skip(1).take(5) {
            let soma = c
                .notas
                .iter()
                .filter(|n| n.voz == 1 && !n.acorde && !n.adorno)
                .fold(Fracao::ZERO, |a, n| a + n.duracao);
            assert_eq!(soma, p.compassos[i].duracao, "compasso {}", i + 1);
        }
    }

    #[test]
    fn barcarolle_tem_tercinas_e_seis_oitavos_com_pulso_composto() {
        let p = Partitura::de_musicxml(arq("barcarolle")).expect("válida");
        let f = p.compassos[0].forma;
        assert_eq!(f.duracao(), Fracao::inteiro(3));
        assert_eq!(f.duracao_do_pulso(), Fracao::nova(3, 2));
        let tem_tuplo = p
            .partes
            .iter()
            .flat_map(|x| &x.compassos)
            .flat_map(|c| &c.notas)
            .any(|n| n.tuplo.is_some() && n.inicio_tuplo);
        assert!(tem_tuplo);
    }

    #[test]
    fn execucao_dos_arquivos_empacotados_e_coerente() {
        for nome in ["wiegenlied", "heidenroeslein", "barcarolle"] {
            let p = Partitura::de_musicxml(arq(nome)).expect("válida");
            let ev = p.eventos_de_reproducao();
            let fim = p.duracao_da_execucao();
            for e in &ev {
                assert!(e.duracao > Fracao::ZERO, "{nome}: duração nula");
                assert!(e.inicio + e.duracao <= fim + Fracao::inteiro(8), "{nome}");
                assert!(e.midi >= 21 && e.midi <= 108, "{nome}: midi {}", e.midi);
            }
            for w in ev.windows(2) {
                assert!(w[0].inicio <= w[1].inicio);
            }
        }
    }

    #[test]
    fn wiegenlied_anacruse_ligadura_repeticao_e_andamento() {
        let p = Partitura::de_musicxml(arq("wiegenlied")).expect("válida");
        // Compasso 0 é a anacruse (1/2 semínima), marcada como incompleta.
        assert!(p.compassos[0].incompleto);
        assert_eq!(p.compassos[0].duracao, Fracao::nova(1, 2));
        assert_eq!(p.compassos[1].inicio, Fracao::nova(1, 2));
        assert_eq!(p.andamento_inicial(), Some(75.0));
        // A peça inteira é repetida uma vez (19 compassos escritos, 38 tocados).
        assert_eq!(p.ordem_de_execucao().len(), 38);
        let ev = p.eventos_de_reproducao();
        // Sol4 e Si♭4 da anacruse ligam-se aos do compasso 1: soam 1 semínima, de 0.
        let primeiros: Vec<(i64, u8, Fracao)> = ev
            .iter()
            .filter(|e| e.inicio == Fracao::ZERO)
            .map(|e| (e.inicio.numerador(), e.midi, e.duracao))
            .collect();
        assert_eq!(
            primeiros,
            vec![(0, 67, Fracao::inteiro(1)), (0, 70, Fracao::inteiro(1))]
        );
        // Mão esquerda: Mi♭2 (39) no tempo 1/2 do compasso 1, pauta 2.
        assert!(ev.iter().any(|e| e.midi == 39
            && e.pauta == 2
            && e.inicio == Fracao::nova(1, 2)
            && e.compasso == 1));
    }

    #[test]
    fn ligaduras_de_expressao_sao_lidas() {
        let p = Partitura::de_musicxml(arq("wiegenlied")).expect("válida");
        let marcas: Vec<MarcaLigadura> = p
            .partes
            .iter()
            .flat_map(|x| &x.compassos)
            .flat_map(|c| &c.notas)
            .flat_map(|n| n.ligaduras.clone())
            .collect();
        let inicios = marcas.iter().filter(|m| m.inicio).count();
        let fins = marcas.len() - inicios;
        assert!(inicios > 5);
        assert_eq!(inicios, fins);
    }

    // --- F04-5: 1.000 compassos no mapa simbólico, sem erro acumulado ------------------------

    /// MusicXML com `n` compassos de 4/4 feitos de 12 colcheias de tercina (`divisions` = 3,
    /// cada nota dura 1/3 de semínima): a soma em ponto flutuante acumularia erro; em racionais
    /// é exata. Andamento declarado: `bpm`.
    fn xml_de_tercinas(n: usize, bpm: u32) -> String {
        let mut x = String::from(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
<part id="P1">"#,
        );
        for k in 0..n {
            x.push_str(&format!(r#"<measure number="{}">"#, k + 1));
            if k == 0 {
                x.push_str(&format!(
                    "<attributes><divisions>3</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>\
                     <direction><direction-type><metronome><beat-unit>quarter</beat-unit><per-minute>{bpm}</per-minute></metronome></direction-type><sound tempo=\"{bpm}\"/></direction>"
                ));
            }
            for _ in 0..12 {
                x.push_str(
                    "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>eighth</type><time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification></note>",
                );
            }
            x.push_str("</measure>");
        }
        x.push_str("</part></score-partwise>");
        x
    }

    #[test]
    fn mil_compassos_de_tercinas_nao_acumulam_erro_no_mapa_simbolico() {
        let p = Partitura::de_musicxml(&xml_de_tercinas(1_000, 97)).expect("1.000 compassos");
        assert_eq!(p.compassos.len(), 1_000);
        assert_eq!(p.total_de_notas(), 12_000);
        // Início de cada compasso: exatamente 4 * k semínimas, sem resíduo.
        for (k, c) in p.compassos.iter().enumerate() {
            assert_eq!(c.inicio, Fracao::inteiro(4 * k as i64), "compasso {k}");
            assert_eq!(c.duracao, Fracao::inteiro(4));
            assert!(!c.incompleto);
        }
        assert_eq!(p.duracao_escrita(), Fracao::inteiro(4_000));
        assert_eq!(p.duracao_da_execucao(), Fracao::inteiro(4_000));
        // Eventos: o j-ésimo começa em j/3 de semínima, exato (comparação racional).
        let ev = p.eventos_de_reproducao();
        assert_eq!(ev.len(), 12_000);
        for (j, e) in ev.iter().enumerate() {
            assert_eq!(e.inicio, Fracao::nova(j as i64, 3), "evento {j}");
            assert_eq!(e.duracao, Fracao::nova(1, 3));
            assert_eq!(e.compasso, j / 12);
        }
        let ultimo = ev.last().expect("último evento");
        assert_eq!(ultimo.inicio + ultimo.duracao, Fracao::inteiro(4_000));
        // Tempo: 4.000 semínimas a 97 bpm = 240.000/97 s; erro relativo < 1e-12 (um único
        // produto em ponto flutuante, sem soma acumulada).
        let mapa = p.mapa_de_andamento();
        let s = mapa.segundos(p.duracao_escrita().como_f64(), 120.0, 1.0);
        let exato = 4_000.0 * 60.0 / 97.0;
        assert!((s - exato).abs() / exato < 1e-12, "{s} vs {exato}");
    }

    #[test]
    fn mil_compassos_a_120_bpm_dao_exatamente_2000_segundos() {
        let p = Partitura::de_musicxml(&xml_de_tercinas(1_000, 120)).expect("1.000 compassos");
        let mapa = p.mapa_de_andamento();
        let s = mapa.segundos(p.duracao_escrita().como_f64(), 120.0, 1.0);
        let ns = (s * 1e9).round() as i64;
        assert_eq!(
            ns,
            2_000_000_000_000,
            "erro de {} ns em 1.000 compassos",
            ns - 2_000_000_000_000
        );
        // O início do compasso 1.000 (índice 999) também cai em tempo exato: 3.996 semínimas.
        let inicio = p.compassos[999].inicio;
        assert_eq!(inicio, Fracao::inteiro(3_996));
        let s999 = mapa.segundos(inicio.como_f64(), 120.0, 1.0);
        assert_eq!((s999 * 1e9).round() as i64, 1_998_000_000_000);
    }

    #[test]
    fn mil_compassos_de_formulas_mistas_3_4_4_4_e_6_8_somam_exatamente() {
        // Ciclo 3/4, 4/4, 6/8 (3 + 4 + 3 semínimas); compassos só com pausa de compasso.
        let formulas = [(3, 4), (4, 4), (6, 8)];
        let mut x = String::from(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
<part id="P1">"#,
        );
        for k in 0..1_000usize {
            let (b, u) = formulas[k % 3];
            x.push_str(&format!(r#"<measure number="{}">"#, k + 1));
            x.push_str(&format!(
                "<attributes><divisions>2</divisions><time><beats>{b}</beats><beat-type>{u}</beat-type></time></attributes>"
            ));
            x.push_str("</measure>");
        }
        x.push_str("</part></score-partwise>");
        let p = Partitura::de_musicxml(&x).expect("1.000 compassos mistos");
        assert_eq!(p.compassos.len(), 1_000);
        let mut esperado = Fracao::ZERO;
        for (k, c) in p.compassos.iter().enumerate() {
            let (b, u) = formulas[k % 3];
            assert_eq!((c.forma.batidas, c.forma.unidade), (b, u), "compasso {k}");
            assert_eq!(c.inicio, esperado, "compasso {k}");
            esperado = esperado + Fracao::nova(i64::from(b) * 4, i64::from(u));
        }
        // 333 ciclos de 10 semínimas + um compasso de 3/4 = 3.333 semínimas.
        assert_eq!(esperado, Fracao::inteiro(3_333));
        assert_eq!(p.duracao_escrita(), Fracao::inteiro(3_333));
        assert_eq!(p.duracao_da_execucao(), Fracao::inteiro(3_333));
        // Pulso composto de 6/8 vale 3/2 de semínima; 3/4 e 4/4, 1.
        assert_eq!(p.compassos[2].forma.duracao_do_pulso(), Fracao::nova(3, 2));
        assert_eq!(p.compassos[0].forma.duracao_do_pulso(), Fracao::inteiro(1));
    }

    const ODE: &str =
        include_str!("../../../assets/partituras/beethoven-ode-a-alegria-tema.musicxml");
    const ESTRELINHA: &str =
        include_str!("../../../assets/partituras/tradicional-brilha-brilha-estrelinha.musicxml");

    #[test]
    fn ode_a_alegria_e_4_4_em_do_maior_com_16_compassos_e_94_notas() {
        let p = Partitura::de_musicxml(ODE).expect("válida");
        assert_eq!(p.partes.len(), 1);
        assert_eq!(p.partes[0].pautas, 2);
        assert_eq!(p.compassos.len(), 16);
        let c0 = &p.compassos[0];
        assert_eq!((c0.forma.batidas, c0.forma.unidade), (4, 4));
        assert_eq!(c0.armadura.quintas, 0);
        assert_eq!(p.total_de_notas(), 94); // 62 da melodia + 32 do baixo
        assert_eq!(p.duracao_escrita(), Fracao::inteiro(64));
        assert_eq!(p.andamento_inicial(), Some(100.0));
        assert!(p.direitos.as_deref().is_some_and(|d| d.contains("CC0")));
        let claves = &p.partes[0].compassos[0].claves;
        assert_eq!(claves.len(), 2);
        assert_eq!(claves[0].clave.sinal, SinalClave::Sol);
        assert_eq!(claves[1].clave.sinal, SinalClave::Fa);
        // Primeiro compasso da melodia: Mi Mi Fá Sol; baixo: Dó3 e Sol2.
        let ev = p.eventos_de_reproducao();
        let mel: Vec<u8> = ev
            .iter()
            .filter(|e| e.pauta == 1 && e.compasso == 0)
            .map(|e| e.midi)
            .collect();
        assert_eq!(mel, [64, 64, 65, 67]);
        let baixo: Vec<u8> = ev
            .iter()
            .filter(|e| e.pauta == 2 && e.compasso == 0)
            .map(|e| e.midi)
            .collect();
        assert_eq!(baixo, [48, 43]);
        assert_eq!(ev.len(), 94);
        // Compasso 4: semínima pontuada + colcheia + mínima (D4 D4 em 3/2 + 1/2 + 2).
        let c4: Vec<_> = ev
            .iter()
            .filter(|e| e.pauta == 1 && e.compasso == 3)
            .collect();
        assert_eq!(c4.len(), 3);
        assert_eq!(c4[0].duracao, Fracao::nova(3, 2));
        assert_eq!(c4[1].duracao, Fracao::nova(1, 2));
        assert_eq!(c4[2].duracao, Fracao::inteiro(2));
        assert!(p.compassos.iter().all(|c| !c.incompleto));
    }

    #[test]
    fn estrelinha_e_4_4_em_sol_maior_com_12_compassos_e_66_notas() {
        let p = Partitura::de_musicxml(ESTRELINHA).expect("válida");
        assert_eq!(p.partes[0].pautas, 2);
        assert_eq!(p.compassos.len(), 12);
        assert_eq!(p.compassos[0].armadura.quintas, 1);
        assert_eq!(
            (p.compassos[0].forma.batidas, p.compassos[0].forma.unidade),
            (4, 4)
        );
        assert_eq!(p.total_de_notas(), 66); // 42 da melodia + 24 do baixo
        assert_eq!(p.duracao_escrita(), Fracao::inteiro(48));
        assert_eq!(p.andamento_inicial(), Some(96.0));
        let ev = p.eventos_de_reproducao();
        let mel: Vec<u8> = ev
            .iter()
            .filter(|e| e.pauta == 1 && e.compasso == 0)
            .map(|e| e.midi)
            .collect();
        assert_eq!(mel, [67, 67, 74, 74]); // Sol Sol Ré Ré
        // Último compasso termina na mínima Sol4.
        let ultimo = ev
            .iter()
            .rfind(|e| e.pauta == 1 && e.compasso == 11)
            .expect("nota");
        assert_eq!((ultimo.midi, ultimo.duracao), (67, Fracao::inteiro(2)));
        assert!(p.compassos.iter().all(|c| !c.incompleto));
    }
}
