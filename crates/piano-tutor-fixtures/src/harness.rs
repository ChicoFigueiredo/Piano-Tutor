//! Harness de golden para MusicXML, reutilizável pelos tickets 08, 10 e 14.
//!
//! Dois níveis de comparação:
//! - **byte a byte** ([`comparar_bytes`]): o texto de cada `<measure>` (e tudo fora dos compassos)
//!   precisa ser idêntico; devolve a lista de compassos que diferem, por posição;
//! - **semântico** ([`comparar_semantica`]): notas, durações, vozes, pautas, ligaduras, tercinas,
//!   letra, cifra, dedilhado, dinâmicas, atributos e barras, ignorando formatação (espaços,
//!   atributos de posição como `default-x`, haste, vigas, ordem de elementos no mesmo instante).
//!
//! [`verificar_golden`] junta os dois para o uso típico de um ticket de edição: fora dos compassos
//! editados o resultado deve ser idêntico ao original byte a byte; nos editados, semanticamente
//! igual ao esperado.

use piano_tutor_core::partitura::Fracao;
use roxmltree::{Document, Node, ParsingOptions};
use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;

/// Erro de análise do XML comparado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroHarness(pub String);

impl fmt::Display for ErroHarness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "harness: {}", self.0)
    }
}

impl std::error::Error for ErroHarness {}

/// Posição de um compasso: parte (id) e índice (0 = primeiro compasso da parte).
///
/// A igualdade considera só `parte` e `indice`; `numero` (o rótulo impresso) serve para mensagens.
#[derive(Debug, Clone, Eq)]
pub struct PosCompasso {
    pub parte: String,
    pub indice: usize,
    pub numero: String,
}

impl PosCompasso {
    /// Posição para listas de compassos editáveis (sem rótulo).
    pub fn nova(parte: &str, indice: usize) -> Self {
        Self {
            parte: parte.to_owned(),
            indice,
            numero: String::new(),
        }
    }
}

impl PartialEq for PosCompasso {
    fn eq(&self, o: &Self) -> bool {
        self.parte == o.parte && self.indice == o.indice
    }
}

impl fmt::Display for PosCompasso {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.parte, self.indice)?;
        if !self.numero.is_empty() {
            write!(f, " (compasso {})", self.numero)?;
        }
        Ok(())
    }
}

fn analisar(xml: &str) -> Result<Document<'_>, ErroHarness> {
    Document::parse_with_options(
        xml,
        ParsingOptions {
            allow_dtd: true,
            nodes_limit: 4_000_000,
            ..ParsingOptions::default()
        },
    )
    .map_err(|e| ErroHarness(format!("XML inválido: {e}")))
}

fn nome<'a>(n: Node<'a, '_>) -> &'a str {
    n.tag_name().name()
}

fn filho<'a, 'b>(n: Node<'a, 'b>, nome_filho: &str) -> Option<Node<'a, 'b>> {
    n.children()
        .find(|c| c.is_element() && nome(*c) == nome_filho)
}

fn texto_de(n: Node<'_, '_>, nome_filho: &str) -> Option<String> {
    filho(n, nome_filho).map(|c| c.text().unwrap_or("").trim().to_owned())
}

/// (posição, intervalo de bytes) de cada compasso, na ordem do arquivo.
fn compassos_do_doc(doc: &Document<'_>) -> Vec<(PosCompasso, Range<usize>)> {
    let mut v = Vec::new();
    for parte in doc
        .root_element()
        .children()
        .filter(|c| c.is_element() && nome(*c) == "part")
    {
        let id = parte.attribute("id").unwrap_or("").to_owned();
        for (i, m) in parte
            .children()
            .filter(|c| c.is_element() && nome(*c) == "measure")
            .enumerate()
        {
            v.push((
                PosCompasso {
                    parte: id.clone(),
                    indice: i,
                    numero: m.attribute("number").unwrap_or("").to_owned(),
                },
                m.range(),
            ));
        }
    }
    v
}

// --- Byte a byte -----------------------------------------------------------------------------

/// Resultado da comparação byte a byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatorioBytes {
    /// Os dois textos são exatamente iguais.
    pub identicos: bool,
    /// Compassos cujo texto difere (ou que existem só de um lado), em ordem de arquivo.
    pub compassos_diferentes: Vec<PosCompasso>,
    /// O texto fora dos compassos (cabeçalho, lista de partes, rodapé) difere.
    pub fora_dos_compassos: bool,
}

/// Igualdade byte a byte simples.
pub fn iguais_byte_a_byte(a: &str, b: &str) -> bool {
    a.as_bytes() == b.as_bytes()
}

fn sem_compassos(xml: &str, compassos: &[(PosCompasso, Range<usize>)]) -> String {
    let mut ranges: Vec<&Range<usize>> = compassos.iter().map(|(_, r)| r).collect();
    ranges.sort_by_key(|r| r.start);
    let mut fora = String::with_capacity(xml.len());
    let mut cursor = 0;
    for r in ranges {
        fora.push_str(&xml[cursor..r.start]);
        cursor = r.end;
    }
    fora.push_str(&xml[cursor..]);
    fora
}

/// Compara dois MusicXML byte a byte, localizando os compassos que diferem.
pub fn comparar_bytes(a: &str, b: &str) -> Result<RelatorioBytes, ErroHarness> {
    if iguais_byte_a_byte(a, b) {
        return Ok(RelatorioBytes {
            identicos: true,
            compassos_diferentes: Vec::new(),
            fora_dos_compassos: false,
        });
    }
    let (da, db) = (analisar(a)?, analisar(b)?);
    let (ma, mb) = (compassos_do_doc(&da), compassos_do_doc(&db));
    let mapa_b: BTreeMap<(String, usize), &Range<usize>> = mb
        .iter()
        .map(|(p, r)| ((p.parte.clone(), p.indice), r))
        .collect();
    let mut diferentes = Vec::new();
    for (p, r) in &ma {
        match mapa_b.get(&(p.parte.clone(), p.indice)) {
            Some(rb) if b[(*rb).clone()].as_bytes() == a[r.clone()].as_bytes() => {}
            _ => diferentes.push(p.clone()),
        }
    }
    let mapa_a: BTreeMap<(String, usize), ()> = ma
        .iter()
        .map(|(p, _)| ((p.parte.clone(), p.indice), ()))
        .collect();
    for (p, _) in &mb {
        if !mapa_a.contains_key(&(p.parte.clone(), p.indice)) {
            diferentes.push(p.clone());
        }
    }
    let fora_dos_compassos = sem_compassos(a, &ma) != sem_compassos(b, &mb);
    Ok(RelatorioBytes {
        identicos: false,
        compassos_diferentes: diferentes,
        fora_dos_compassos,
    })
}

// --- Semântica -------------------------------------------------------------------------------

/// Um item musicalmente significativo de um compasso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemSemantico {
    Nota {
        posicao: Fracao,
        duracao: Fracao,
        voz: String,
        pauta: String,
        /// `None` = pausa; senão "C4", "F#5", "B-3" (alter inteiro).
        altura: Option<String>,
        acorde: bool,
        adorno: bool,
        /// (reais, normais) de `time-modification`.
        tuplo: Option<(u32, u32)>,
        /// "tie:start", "tie:stop", "slur:start:1", "tuplet:stop"...
        ligaduras: Vec<String>,
        /// "verso:silabico:texto".
        letra: Vec<String>,
        dedilhado: Vec<String>,
    },
    Cifra {
        posicao: Fracao,
        texto: String,
    },
    Atributo(String),
    Barra(String),
    Direcao {
        posicao: Fracao,
        texto: String,
    },
}

impl ItemSemantico {
    fn chave(&self) -> (f64, u8, String) {
        match self {
            Self::Atributo(t) => (-1.0, 0, t.clone()),
            Self::Barra(t) => (1e12, 0, t.clone()),
            Self::Cifra { posicao, texto } => (posicao.como_f64(), 1, texto.clone()),
            Self::Direcao { posicao, texto } => (posicao.como_f64(), 2, texto.clone()),
            Self::Nota {
                posicao,
                voz,
                pauta,
                altura,
                ..
            } => (
                posicao.como_f64(),
                3,
                format!("{pauta}|{voz}|{}", altura.clone().unwrap_or_default()),
            ),
        }
    }
}

/// Semântica de um compasso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompassoSemantico {
    pub pos: PosCompasso,
    pub itens: Vec<ItemSemantico>,
}

/// Semântica de um MusicXML inteiro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Semantica {
    /// (id, nome) das partes, na ordem da lista de partes.
    pub partes: Vec<(String, String)>,
    pub compassos: Vec<CompassoSemantico>,
}

fn alter_texto(alter: Option<&str>) -> String {
    match alter.and_then(|a| a.parse::<f64>().ok()) {
        Some(v) if v > 0.0 => "#".repeat(v.round().max(1.0) as usize),
        Some(v) if v < 0.0 => "b".repeat((-v).round().max(1.0) as usize),
        _ => String::new(),
    }
}

fn dur_em_seminimas(duration: i64, divisions: i64) -> Fracao {
    Fracao::nova(duration, divisions.max(1))
}

fn inteiro(n: Node<'_, '_>, nome_filho: &str) -> Option<i64> {
    texto_de(n, nome_filho).and_then(|t| t.parse::<f64>().ok().map(|v| v.round() as i64))
}

fn nota_semantica(
    n: Node<'_, '_>,
    divisions: i64,
    pos_atual: &mut Fracao,
    ultima_pos: &mut Fracao,
) -> ItemSemantico {
    let acorde = filho(n, "chord").is_some();
    let adorno = filho(n, "grace").is_some();
    let duracao = if adorno {
        Fracao::ZERO
    } else {
        inteiro(n, "duration").map_or(Fracao::ZERO, |d| dur_em_seminimas(d, divisions))
    };
    let posicao = if acorde { *ultima_pos } else { *pos_atual };
    if !acorde {
        *ultima_pos = *pos_atual;
        *pos_atual = *pos_atual + duracao;
    }
    let altura = filho(n, "pitch").map(|p| {
        format!(
            "{}{}{}",
            texto_de(p, "step").unwrap_or_default(),
            alter_texto(texto_de(p, "alter").as_deref()),
            texto_de(p, "octave").unwrap_or_default()
        )
    });
    let tuplo = filho(n, "time-modification").and_then(|t| {
        Some((
            u32::try_from(inteiro(t, "actual-notes")?).ok()?,
            u32::try_from(inteiro(t, "normal-notes")?).ok()?,
        ))
    });
    let mut ligaduras = Vec::new();
    let mut letra = Vec::new();
    let mut dedilhado = Vec::new();
    for c in n.children().filter(Node::is_element) {
        match nome(c) {
            "tie" => ligaduras.push(format!("tie:{}", c.attribute("type").unwrap_or(""))),
            "lyric" => letra.push(format!(
                "{}:{}:{}",
                c.attribute("number").unwrap_or("1"),
                texto_de(c, "syllabic").unwrap_or_default(),
                texto_de(c, "text").unwrap_or_default()
            )),
            "notations" => {
                for d in c.descendants().filter(Node::is_element) {
                    match nome(d) {
                        "slur" => ligaduras.push(format!(
                            "slur:{}:{}",
                            d.attribute("type").unwrap_or(""),
                            d.attribute("number").unwrap_or("1")
                        )),
                        "tuplet" => {
                            ligaduras.push(format!("tuplet:{}", d.attribute("type").unwrap_or("")));
                        }
                        "fingering" => {
                            dedilhado.push(d.text().unwrap_or("").trim().to_owned());
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    ligaduras.sort();
    letra.sort();
    ItemSemantico::Nota {
        posicao,
        duracao,
        voz: texto_de(n, "voice").unwrap_or_else(|| "1".into()),
        pauta: texto_de(n, "staff").unwrap_or_else(|| "1".into()),
        altura,
        acorde,
        adorno,
        tuplo,
        ligaduras,
        letra,
        dedilhado,
    }
}

fn atributos_semanticos(n: Node<'_, '_>, divisions: &mut i64, itens: &mut Vec<ItemSemantico>) {
    for c in n.children().filter(Node::is_element) {
        match nome(c) {
            // As divisões só mudam a unidade, não a música: já normalizadas em semínimas.
            "divisions" => {
                if let Some(d) = c.text().and_then(|t| t.trim().parse::<i64>().ok()) {
                    *divisions = d.max(1);
                }
            }
            "key" => itens.push(ItemSemantico::Atributo(format!(
                "key:{}:{}",
                texto_de(c, "fifths").unwrap_or_default(),
                texto_de(c, "mode").unwrap_or_default()
            ))),
            "time" => itens.push(ItemSemantico::Atributo(format!(
                "time:{}/{}",
                texto_de(c, "beats").unwrap_or_default(),
                texto_de(c, "beat-type").unwrap_or_default()
            ))),
            "staves" => itens.push(ItemSemantico::Atributo(format!(
                "staves:{}",
                c.text().unwrap_or("").trim()
            ))),
            "clef" => itens.push(ItemSemantico::Atributo(format!(
                "clef:{}:{}:{}",
                c.attribute("number").unwrap_or("1"),
                texto_de(c, "sign").unwrap_or_default(),
                texto_de(c, "line").unwrap_or_default()
            ))),
            _ => {}
        }
    }
}

fn cifra_texto(h: Node<'_, '_>) -> String {
    let raiz = filho(h, "root")
        .map(|r| {
            format!(
                "{}{}",
                texto_de(r, "root-step").unwrap_or_default(),
                alter_texto(texto_de(r, "root-alter").as_deref())
            )
        })
        .unwrap_or_default();
    let tipo = filho(h, "kind")
        .map(|k| k.text().unwrap_or("").trim().to_owned())
        .unwrap_or_default();
    let baixo = filho(h, "bass")
        .map(|b| {
            format!(
                "/{}{}",
                texto_de(b, "bass-step").unwrap_or_default(),
                alter_texto(texto_de(b, "bass-alter").as_deref())
            )
        })
        .unwrap_or_default();
    format!("{raiz}:{tipo}{baixo}")
}

fn direcao_texto(d: Node<'_, '_>) -> Option<String> {
    let mut partes = Vec::new();
    for x in d.descendants().filter(Node::is_element) {
        match nome(x) {
            "dynamics" => {
                for din in x.children().filter(Node::is_element) {
                    partes.push(format!("din:{}", nome(din)));
                }
            }
            "wedge" => partes.push(format!("wedge:{}", x.attribute("type").unwrap_or(""))),
            "sound" => {
                if let Some(t) = x.attribute("tempo") {
                    partes.push(format!("tempo:{t}"));
                }
            }
            "words" => partes.push(format!("texto:{}", x.text().unwrap_or("").trim())),
            _ => {}
        }
    }
    (!partes.is_empty()).then(|| partes.join(","))
}

fn barra_texto(b: Node<'_, '_>) -> String {
    let mut p = vec![format!(
        "loc:{}",
        b.attribute("location").unwrap_or("right")
    )];
    for c in b.children().filter(Node::is_element) {
        match nome(c) {
            "bar-style" => p.push(format!("estilo:{}", c.text().unwrap_or("").trim())),
            "repeat" => p.push(format!("repete:{}", c.attribute("direction").unwrap_or(""))),
            "ending" => p.push(format!(
                "casa:{}:{}",
                c.attribute("number").unwrap_or(""),
                c.attribute("type").unwrap_or("")
            )),
            _ => {}
        }
    }
    p.join(",")
}

/// Extrai a semântica de um MusicXML.
pub fn extrair_semantica(xml: &str) -> Result<Semantica, ErroHarness> {
    let doc = analisar(xml)?;
    let raiz = doc.root_element();
    let partes_lista: Vec<(String, String)> = filho(raiz, "part-list")
        .map(|pl| {
            pl.children()
                .filter(|c| c.is_element() && nome(*c) == "score-part")
                .map(|sp| {
                    (
                        sp.attribute("id").unwrap_or("").to_owned(),
                        texto_de(sp, "part-name").unwrap_or_default(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let mut compassos = Vec::new();
    for parte in raiz
        .children()
        .filter(|c| c.is_element() && nome(*c) == "part")
    {
        let id = parte.attribute("id").unwrap_or("").to_owned();
        let mut divisions = 1_i64;
        for (i, m) in parte
            .children()
            .filter(|c| c.is_element() && nome(*c) == "measure")
            .enumerate()
        {
            let mut itens = Vec::new();
            let mut pos = Fracao::ZERO;
            let mut ultima = Fracao::ZERO;
            for c in m.children().filter(Node::is_element) {
                match nome(c) {
                    "attributes" => atributos_semanticos(c, &mut divisions, &mut itens),
                    "note" => itens.push(nota_semantica(c, divisions, &mut pos, &mut ultima)),
                    "backup" => {
                        pos = pos
                            - inteiro(c, "duration")
                                .map_or(Fracao::ZERO, |d| dur_em_seminimas(d, divisions));
                    }
                    "forward" => {
                        pos = pos
                            + inteiro(c, "duration")
                                .map_or(Fracao::ZERO, |d| dur_em_seminimas(d, divisions));
                    }
                    "harmony" => itens.push(ItemSemantico::Cifra {
                        posicao: pos,
                        texto: cifra_texto(c),
                    }),
                    "direction" => {
                        if let Some(texto) = direcao_texto(c) {
                            itens.push(ItemSemantico::Direcao {
                                posicao: pos,
                                texto,
                            });
                        }
                    }
                    "barline" => itens.push(ItemSemantico::Barra(barra_texto(c))),
                    _ => {}
                }
            }
            itens.sort_by(|a, b| {
                let (ka, kb) = (a.chave(), b.chave());
                ka.0.total_cmp(&kb.0)
                    .then(ka.1.cmp(&kb.1))
                    .then_with(|| ka.2.cmp(&kb.2))
            });
            compassos.push(CompassoSemantico {
                pos: PosCompasso {
                    parte: id.clone(),
                    indice: i,
                    numero: m.attribute("number").unwrap_or("").to_owned(),
                },
                itens,
            });
        }
    }
    Ok(Semantica {
        partes: partes_lista,
        compassos,
    })
}

/// Resultado da comparação semântica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatorioSemantico {
    /// Compassos cuja semântica difere (ou que existem só de um lado).
    pub compassos_diferentes: Vec<PosCompasso>,
    /// A lista de partes (ids e nomes) difere.
    pub partes_diferentes: bool,
    /// Descrição humana de cada diferença (para mensagens de falha).
    pub detalhes: Vec<String>,
}

impl RelatorioSemantico {
    pub fn equivalentes(&self) -> bool {
        self.compassos_diferentes.is_empty() && !self.partes_diferentes
    }
}

/// Compara a semântica de dois MusicXML (diferença por compasso).
pub fn comparar_semantica(a: &str, b: &str) -> Result<RelatorioSemantico, ErroHarness> {
    let (sa, sb) = (extrair_semantica(a)?, extrair_semantica(b)?);
    let mut r = RelatorioSemantico {
        compassos_diferentes: Vec::new(),
        partes_diferentes: sa.partes != sb.partes,
        detalhes: Vec::new(),
    };
    if r.partes_diferentes {
        r.detalhes
            .push(format!("partes: {:?} != {:?}", sa.partes, sb.partes));
    }
    let mapa_b: BTreeMap<(String, usize), &CompassoSemantico> = sb
        .compassos
        .iter()
        .map(|c| ((c.pos.parte.clone(), c.pos.indice), c))
        .collect();
    for ca in &sa.compassos {
        match mapa_b.get(&(ca.pos.parte.clone(), ca.pos.indice)) {
            Some(cb) if cb.itens == ca.itens => {}
            Some(cb) => {
                r.detalhes
                    .push(format!("{}: {:?} != {:?}", ca.pos, ca.itens, cb.itens));
                r.compassos_diferentes.push(ca.pos.clone());
            }
            None => {
                r.detalhes.push(format!("{}: só no primeiro", ca.pos));
                r.compassos_diferentes.push(ca.pos.clone());
            }
        }
    }
    for cb in &sb.compassos {
        if !sa.compassos.iter().any(|ca| ca.pos == cb.pos) {
            r.detalhes.push(format!("{}: só no segundo", cb.pos));
            r.compassos_diferentes.push(cb.pos.clone());
        }
    }
    Ok(r)
}

// --- Golden ----------------------------------------------------------------------------------

/// Uma violação do golden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violacao {
    /// Compasso que não podia mudar mudou (byte a byte).
    CompassoAlterado(PosCompasso),
    /// O texto fora dos compassos (cabeçalho, partes) mudou.
    ForaDosCompassosAlterado,
    /// Os compassos editados não equivalem semanticamente ao esperado.
    SemanticaDivergente(PosCompasso),
    /// A lista de partes do obtido não equivale à do esperado.
    PartesDivergentes,
}

/// Verificação de golden para uma edição.
///
/// - Todo compasso fora de `editados` deve ser idêntico ao `original` byte a byte, assim como
///   tudo fora dos compassos;
/// - se `esperado` for dado, o `obtido` deve ser semanticamente igual a ele em todos os
///   compassos (formatação à parte).
pub fn verificar_golden(
    original: &str,
    obtido: &str,
    editados: &[PosCompasso],
    esperado: Option<&str>,
) -> Result<(), Vec<Violacao>> {
    let mut v = Vec::new();
    match comparar_bytes(original, obtido) {
        Ok(rel) => {
            if rel.fora_dos_compassos {
                v.push(Violacao::ForaDosCompassosAlterado);
            }
            for p in rel.compassos_diferentes {
                if !editados.contains(&p) {
                    v.push(Violacao::CompassoAlterado(p));
                }
            }
        }
        Err(_) => v.push(Violacao::ForaDosCompassosAlterado),
    }
    if let Some(esp) = esperado {
        match comparar_semantica(obtido, esp) {
            Ok(rel) => {
                if rel.partes_diferentes {
                    v.push(Violacao::PartesDivergentes);
                }
                for p in rel.compassos_diferentes {
                    v.push(Violacao::SemanticaDivergente(p));
                }
            }
            Err(_) => v.push(Violacao::PartesDivergentes),
        }
    }
    if v.is_empty() { Ok(()) } else { Err(v) }
}
