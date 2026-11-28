//! Elementos que a Oficina só **mostra** e **preserva** (ticket 19, E01, decisão D13): letra
//! (várias estrofes), cifra e dedilhado, mais a lista dos elementos que o editor ainda não
//! desenha nem edita (critério A15).
//!
//! Tudo aqui é **leitura**: nada deste módulo altera o documento. A preservação no arquivo é
//! garantida pelo modelo híbrido (ADR-0017): o texto original é copiado byte a byte e só os
//! compassos sujos são regenerados, por peças, mantendo os filhos que o editor não conhece. Este
//! módulo lê o texto **atual** (o exportável: o original, ou com os compassos editados) e liga
//! cada letra, cada dedilhado e cada cifra à **identidade da nota** a que pertencem
//! ([`IdNota`]), que é o que o render precisa para desenhá-los junto das notas certas.
//!
//! # Regras de associação
//!
//! - **Letra e dedilhado** vivem dentro da própria `<note>`: pertencem a ela.
//! - **Cifra** (`<harmony>`) vem solta no compasso e vale para a **próxima** nota do compasso; se
//!   não houver nota depois dela, vale para a última nota anterior; compasso sem nota deixa a
//!   cifra sem âncora (o render a põe no início do compasso). O deslocamento `<offset>` da cifra
//!   é ignorado nesta fatia (limite declarado).
//! - **Estrofe** é o atributo `number` do `<lyric>` (padrão `"1"`); a ordem das estrofes na
//!   exibição é numérica quando possível.
//!
//! # Lista de não suportados
//!
//! Para cada nome de elemento que o editor não desenha, o nome e a **quantidade** no arquivo
//! inteiro. `reconhecido` distingue o que o projeto conhece por nome (dinâmica, ornamento,
//! fermata, pedal...) do que é **desconhecido** (extensão `x-...`, elemento fora da tabela).
//! Um elemento desconhecido nunca impede a abertura (T8): é listado e preservado.

use super::{Documento, IdNota};
use roxmltree::Node;
use std::collections::BTreeMap;

/// Como a sílaba se liga à seguinte (`<syllabic>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TipoDeSilaba {
    /// Palavra de uma sílaba só (ou `<syllabic>` ausente).
    Unica,
    /// Primeira sílaba de uma palavra maior: segue hífen.
    Inicio,
    /// Sílaba do meio: segue hífen.
    Meio,
    /// Última sílaba da palavra.
    Fim,
}

/// Uma sílaba de letra (uma estrofe de uma nota).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Silaba {
    /// Estrofe (`number` do `<lyric>`; `"1"` se faltar).
    pub estrofe: String,
    /// Texto da sílaba como escrito (várias partes de `<text>` e `<elision>` já juntas).
    pub texto: String,
    pub tipo: TipoDeSilaba,
    /// A sílaba continua nas notas seguintes (`<extend/>`): melisma.
    pub prolongada: bool,
}

impl Silaba {
    /// O texto como se desenha: com hífen no fim quando a palavra continua.
    pub fn texto_exibido(&self) -> String {
        match self.tipo {
            TipoDeSilaba::Inicio | TipoDeSilaba::Meio => format!("{}-", self.texto),
            _ => self.texto.clone(),
        }
    }
}

/// Um dedilhado (`<fingering>` dentro de `<technical>`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dedo {
    /// O texto como escrito (`"1"` a `"5"` no piano; também `"p"`, `"i"`... em outros contextos).
    pub texto: String,
    /// `substitution="yes"`: troca de dedo sobre a mesma nota (1-2).
    pub substituicao: bool,
}

/// Letra e dedilhado de uma nota.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ElementosDaNota {
    pub silabas: Vec<Silaba>,
    pub dedos: Vec<Dedo>,
}

impl ElementosDaNota {
    fn vazio(&self) -> bool {
        self.silabas.is_empty() && self.dedos.is_empty()
    }

    /// O dedilhado como texto único: `"3"` ou, havendo troca de dedo, `"1-2"`.
    pub fn dedilhado_em_texto(&self) -> String {
        self.dedos
            .iter()
            .map(|d| d.texto.as_str())
            .collect::<Vec<_>>()
            .join("-")
    }
}

/// Uma cifra (`<harmony>`) já com o nome pronto para desenhar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cifra {
    /// Índice da parte entre as partes que têm compasso (o mesmo de `Partitura::partes`).
    pub parte: usize,
    /// Índice do compasso na parte.
    pub compasso: usize,
    /// Nome para desenhar (`C`, `G7/B`, `Bbm`, `F#m7b5`...).
    pub texto: String,
    /// Nota a que a cifra vale; `None` se o compasso não tem nota.
    pub nota: Option<IdNota>,
}

/// Um elemento que o editor ainda não desenha nem edita.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementoNaoSuportado {
    /// Nome do elemento no MusicXML (`dynamics`, `trill-mark`, `x-extensao-futura`...).
    pub nome: String,
    /// Grupo em português para a lista (`dinâmica`, `ornamento`...); `"desconhecido"` quando o
    /// projeto não conhece o nome.
    pub categoria: &'static str,
    /// Quantas vezes aparece no arquivo.
    pub quantidade: usize,
    /// O projeto conhece o nome (preserva, não desenha); `false` = desconhecido.
    pub reconhecido: bool,
    /// Parte (id) e número do compasso da primeira ocorrência, para o titular achar.
    pub primeiro: Option<(String, String)>,
}

/// Tudo o que a Oficina mostra além das notas, lido do texto original.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Exibicao {
    por_nota: BTreeMap<IdNota, ElementosDaNota>,
    cifras: Vec<Cifra>,
    nao_suportados: Vec<ElementoNaoSuportado>,
}

impl Exibicao {
    /// Letra e dedilhado da nota, se ela tiver algum.
    pub fn da_nota(&self, id: IdNota) -> Option<&ElementosDaNota> {
        self.por_nota.get(&id)
    }

    /// Todas as cifras, na ordem do documento.
    pub fn cifras(&self) -> &[Cifra] {
        &self.cifras
    }

    /// Notas com letra ou dedilhado, em ordem de identidade.
    pub fn notas(&self) -> impl Iterator<Item = (IdNota, &ElementosDaNota)> {
        self.por_nota.iter().map(|(id, e)| (*id, e))
    }

    /// Elementos que o editor não desenha, do mais frequente para o menos.
    pub fn nao_suportados(&self) -> &[ElementoNaoSuportado] {
        &self.nao_suportados
    }

    pub fn total_de_silabas(&self) -> usize {
        self.por_nota.values().map(|e| e.silabas.len()).sum()
    }

    pub fn total_de_dedos(&self) -> usize {
        self.por_nota.values().map(|e| e.dedos.len()).sum()
    }

    /// Estrofes presentes (de qualquer nota), na ordem de exibição.
    pub fn estrofes(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .por_nota
            .values()
            .flat_map(|e| e.silabas.iter().map(|s| s.estrofe.clone()))
            .collect();
        ordenar_estrofes(&mut v);
        v.dedup();
        v
    }

    /// Não há nada para mostrar nem para listar.
    pub fn vazia(&self) -> bool {
        self.por_nota.is_empty() && self.cifras.is_empty() && self.nao_suportados.is_empty()
    }

    /// Texto curto para a lista visível na Oficina: uma linha por elemento (`nome (categoria): N`).
    pub fn linhas_de_nao_suportados(&self) -> Vec<String> {
        self.nao_suportados
            .iter()
            .map(|e| {
                if e.reconhecido {
                    format!("{} ({}): {}", e.nome, e.categoria, e.quantidade)
                } else {
                    format!("{} (desconhecido): {}", e.nome, e.quantidade)
                }
            })
            .collect()
    }
}

/// Ordem de exibição das estrofes: numérica quando todas são números; senão, alfabética.
pub fn ordenar_estrofes(v: &mut [String]) {
    v.sort_by(
        |a, b| match (a.trim().parse::<u32>(), b.trim().parse::<u32>()) {
            (Ok(x), Ok(y)) => x.cmp(&y).then_with(|| a.cmp(b)),
            (Ok(_), Err(_)) => std::cmp::Ordering::Less,
            (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
            (Err(_), Err(_)) => a.cmp(b),
        },
    );
}

// --- Classificação dos elementos --------------------------------------------------------------

/// Onde o elemento aparece (o mesmo nome pode significar coisas diferentes em cada lugar).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lugar {
    Compasso,
    Atributos,
    Direcao,
    Barra,
    Nota,
    Notacoes,
    Ornamentos,
    Articulacoes,
    Tecnica,
}

enum Veredito {
    /// O editor entende e desenha (ou é estrutura que não precisa de desenho).
    Tratado,
    /// Entra na lista de não suportados, com esta categoria.
    NaoDesenhado(&'static str),
    /// O elemento tem filhos que também são classificados (`notations`, `direction`...).
    Descer(Lugar),
    /// Não é problema do render: não entra na lista.
    Ignorar,
}

fn veredito(lugar: Lugar, nome: &str) -> Option<Veredito> {
    use Veredito::{Descer, Ignorar, NaoDesenhado, Tratado};
    Some(match (lugar, nome) {
        (Lugar::Compasso, "backup" | "forward" | "harmony" | "sound") => Tratado,
        (Lugar::Compasso, "note") => Descer(Lugar::Nota),
        (Lugar::Compasso, "attributes") => Descer(Lugar::Atributos),
        (Lugar::Compasso, "direction") => Descer(Lugar::Direcao),
        (Lugar::Compasso, "barline") => Descer(Lugar::Barra),
        (Lugar::Compasso, "print") => NaoDesenhado("layout de página e sistema"),
        (Lugar::Compasso, "figured-bass") => NaoDesenhado("baixo cifrado"),
        (Lugar::Compasso, "grouping") => NaoDesenhado("agrupamento"),
        (Lugar::Compasso, "link" | "bookmark") => NaoDesenhado("link e marcador"),

        (Lugar::Atributos, "divisions" | "key" | "time" | "clef" | "staves") => Tratado,
        (Lugar::Atributos, "footnote" | "level") => Ignorar,
        (Lugar::Atributos, "transpose") => NaoDesenhado("transposição"),
        (Lugar::Atributos, "staff-details") => NaoDesenhado("detalhes da pauta"),
        (Lugar::Atributos, "measure-style") => NaoDesenhado("estilo de compasso"),
        (Lugar::Atributos, "instruments" | "part-symbol" | "directive") => {
            NaoDesenhado("atributos da parte")
        }

        (Lugar::Direcao, "direction-type") => Descer(Lugar::Direcao),
        (Lugar::Direcao, "offset" | "staff" | "voice" | "sound" | "footnote" | "level") => Ignorar,
        (Lugar::Direcao, "dynamics") => NaoDesenhado("dinâmica"),
        (Lugar::Direcao, "wedge") => NaoDesenhado("crescendo e diminuendo"),
        (Lugar::Direcao, "words") => NaoDesenhado("texto de direção"),
        (Lugar::Direcao, "pedal") => NaoDesenhado("pedal"),
        (Lugar::Direcao, "octave-shift") => NaoDesenhado("8va e 8vb"),
        (Lugar::Direcao, "segno" | "coda") => NaoDesenhado("segno e coda"),
        (Lugar::Direcao, "rehearsal") => NaoDesenhado("marca de ensaio"),
        (Lugar::Direcao, "metronome") => NaoDesenhado("marca de andamento"),
        (Lugar::Direcao, "bracket" | "dashes" | "symbol" | "other-direction") => {
            NaoDesenhado("outra direção")
        }

        (Lugar::Barra, "bar-style" | "repeat" | "ending") => Tratado,
        (Lugar::Barra, "footnote" | "level") => Ignorar,
        (Lugar::Barra, "fermata") => NaoDesenhado("fermata"),
        (Lugar::Barra, "segno" | "coda") => NaoDesenhado("segno e coda"),
        (Lugar::Barra, "wavy-line") => NaoDesenhado("ornamento"),

        (
            Lugar::Nota,
            "pitch" | "rest" | "duration" | "voice" | "type" | "dot" | "accidental" | "stem"
            | "staff" | "beam" | "tie" | "chord" | "grace" | "time-modification" | "lyric"
            | "instrument" | "play" | "listen",
        ) => Tratado,
        (Lugar::Nota, "footnote" | "level") => Ignorar,
        (Lugar::Nota, "notations") => Descer(Lugar::Notacoes),
        (Lugar::Nota, "notehead" | "notehead-text") => NaoDesenhado("cabeça de nota especial"),
        (Lugar::Nota, "cue") => NaoDesenhado("nota de apoio (cue)"),
        (Lugar::Nota, "unpitched") => NaoDesenhado("nota sem altura definida"),

        (Lugar::Notacoes, "tied" | "slur" | "tuplet") => Tratado,
        (Lugar::Notacoes, "footnote" | "level") => Ignorar,
        (Lugar::Notacoes, "technical") => Descer(Lugar::Tecnica),
        (Lugar::Notacoes, "ornaments") => Descer(Lugar::Ornamentos),
        (Lugar::Notacoes, "articulations") => Descer(Lugar::Articulacoes),
        (Lugar::Notacoes, "dynamics") => NaoDesenhado("dinâmica"),
        (Lugar::Notacoes, "fermata") => NaoDesenhado("fermata"),
        (Lugar::Notacoes, "arpeggiate" | "non-arpeggiate") => NaoDesenhado("arpejo"),
        (Lugar::Notacoes, "glissando" | "slide") => NaoDesenhado("glissando"),
        (Lugar::Notacoes, "accidental-mark") => NaoDesenhado("acidente de ornamento"),
        (Lugar::Notacoes, "other-notation") => NaoDesenhado("outra notação"),

        (
            Lugar::Ornamentos,
            "trill-mark"
            | "turn"
            | "delayed-turn"
            | "inverted-turn"
            | "delayed-inverted-turn"
            | "vertical-turn"
            | "inverted-vertical-turn"
            | "shake"
            | "wavy-line"
            | "mordent"
            | "inverted-mordent"
            | "schleifer"
            | "tremolo"
            | "haydn"
            | "other-ornament"
            | "accidental-mark",
        ) => NaoDesenhado("ornamento"),

        (
            Lugar::Articulacoes,
            "accent" | "strong-accent" | "staccato" | "tenuto" | "detached-legato"
            | "staccatissimo" | "spiccato" | "scoop" | "plop" | "doit" | "falloff" | "breath-mark"
            | "caesura" | "stress" | "unstress" | "soft-accent" | "other-articulation",
        ) => NaoDesenhado("articulação"),

        (Lugar::Tecnica, "fingering") => Tratado,
        (
            Lugar::Tecnica,
            "up-bow" | "down-bow" | "harmonic" | "open-string" | "thumb-position" | "string"
            | "fret" | "pluck" | "double-tongue" | "triple-tongue" | "stopped" | "snap-pizzicato"
            | "hammer-on" | "pull-off" | "bend" | "tap" | "heel" | "toe" | "fingernails" | "hole"
            | "arrow" | "handbell" | "brass-bend" | "flip" | "smear" | "open" | "half-muted"
            | "harmon-mute" | "golpe" | "other-technical",
        ) => NaoDesenhado("técnica"),

        _ => return None,
    })
}

/// Acumula a contagem por nome de elemento (o próprio item da lista, que vai crescendo).
#[derive(Default)]
struct Contagem {
    por_nome: BTreeMap<String, ElementoNaoSuportado>,
}

impl Contagem {
    fn somar(
        &mut self,
        nome: &str,
        categoria: &'static str,
        reconhecido: bool,
        onde: (&str, &str),
    ) {
        let e = self
            .por_nome
            .entry(nome.to_owned())
            .or_insert_with(|| ElementoNaoSuportado {
                nome: nome.to_owned(),
                categoria,
                quantidade: 0,
                reconhecido,
                primeiro: Some((onde.0.to_owned(), onde.1.to_owned())),
            });
        e.quantidade += 1;
    }

    fn varrer(&mut self, no: Node<'_, '_>, lugar: Lugar, onde: (&str, &str)) {
        for el in no.children().filter(Node::is_element) {
            let nome = el.tag_name().name();
            match veredito(lugar, nome) {
                Some(Veredito::Tratado | Veredito::Ignorar) => {}
                Some(Veredito::Descer(l)) => self.varrer(el, l, onde),
                Some(Veredito::NaoDesenhado(cat)) => self.somar(nome, cat, true, onde),
                None => self.somar(nome, "desconhecido", false, onde),
            }
        }
    }

    fn finalizar(self) -> Vec<ElementoNaoSuportado> {
        let mut v: Vec<ElementoNaoSuportado> = self.por_nome.into_values().collect();
        v.sort_by(|a, b| {
            b.quantidade
                .cmp(&a.quantidade)
                .then_with(|| a.nome.cmp(&b.nome))
        });
        v
    }
}

// --- Leitura de letra, dedilhado e cifra -----------------------------------------------------

fn filho<'a, 'b>(n: Node<'a, 'b>, nome: &str) -> Option<Node<'a, 'b>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == nome)
}

fn texto_de(n: Node<'_, '_>, nome: &str) -> Option<String> {
    filho(n, nome).map(|c| c.text().unwrap_or("").trim().to_owned())
}

fn ler_silabas(nota: Node<'_, '_>) -> Vec<Silaba> {
    let mut v = Vec::new();
    for l in nota
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "lyric")
    {
        let mut texto = String::new();
        for p in l.children().filter(Node::is_element) {
            match p.tag_name().name() {
                "text" => texto.push_str(p.text().unwrap_or("").trim()),
                "elision" => {
                    let e = p.text().unwrap_or("").trim();
                    texto.push_str(if e.is_empty() { "\u{203F}" } else { e });
                }
                _ => {}
            }
        }
        if texto.is_empty() {
            continue;
        }
        let tipo = match texto_de(l, "syllabic").as_deref() {
            Some("begin") => TipoDeSilaba::Inicio,
            Some("middle") => TipoDeSilaba::Meio,
            Some("end") => TipoDeSilaba::Fim,
            _ => TipoDeSilaba::Unica,
        };
        let estrofe = l
            .attribute("number")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("1")
            .to_owned();
        v.push(Silaba {
            estrofe,
            texto,
            tipo,
            prolongada: filho(l, "extend").is_some(),
        });
    }
    v
}

fn ler_dedos(nota: Node<'_, '_>) -> Vec<Dedo> {
    let mut v = Vec::new();
    for not in nota
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "notations")
    {
        for tec in not
            .children()
            .filter(|c| c.is_element() && c.tag_name().name() == "technical")
        {
            for f in tec
                .children()
                .filter(|c| c.is_element() && c.tag_name().name() == "fingering")
            {
                let t = f.text().unwrap_or("").trim();
                if !t.is_empty() {
                    v.push(Dedo {
                        texto: t.to_owned(),
                        substituicao: f.attribute("substitution") == Some("yes"),
                    });
                }
            }
        }
    }
    v
}

fn alteracao(a: i32) -> &'static str {
    match a {
        1 => "#",
        -1 => "b",
        2 => "##",
        -2 => "bb",
        _ => "",
    }
}

fn nota_da_cifra(raiz: Node<'_, '_>, passo: &str, alter: &str) -> String {
    let p = texto_de(raiz, passo).unwrap_or_default();
    let a = texto_de(raiz, alter)
        .and_then(|t| t.parse::<f64>().ok())
        .map_or(0, |v| v.round() as i32);
    format!("{p}{}", alteracao(a))
}

/// Sufixo de um `<kind>` quando o arquivo não traz o atributo `text`.
fn sufixo_do_tipo(kind: &str) -> &'static str {
    match kind {
        "major" => "",
        "minor" => "m",
        "dominant" => "7",
        "major-seventh" => "maj7",
        "minor-seventh" => "m7",
        "diminished" => "dim",
        "diminished-seventh" => "dim7",
        "augmented" => "aug",
        "augmented-seventh" => "7#5",
        "half-diminished" => "m7b5",
        "major-minor" => "m(maj7)",
        "major-sixth" => "6",
        "minor-sixth" => "m6",
        "dominant-ninth" => "9",
        "major-ninth" => "maj9",
        "minor-ninth" => "m9",
        "dominant-11th" => "11",
        "dominant-13th" => "13",
        "suspended-second" => "sus2",
        "suspended-fourth" => "sus4",
        "power" => "5",
        "none" => "",
        _ => "",
    }
}

/// O nome da cifra para desenhar: raiz, tipo (o `text` do arquivo manda), graus e baixo.
fn nome_da_cifra(h: Node<'_, '_>) -> String {
    let mut s = String::new();
    let kind = filho(h, "kind");
    if kind.is_some_and(|k| k.text().map(str::trim) == Some("none")) && filho(h, "root").is_none() {
        return "N.C.".to_owned();
    }
    if let Some(r) = filho(h, "root") {
        s.push_str(&nota_da_cifra(r, "root-step", "root-alter"));
    } else if let Some(r) = filho(h, "function") {
        s.push_str(r.text().unwrap_or("").trim());
    }
    if let Some(k) = kind {
        match k.attribute("text") {
            Some(t) => s.push_str(t.trim()),
            None => s.push_str(sufixo_do_tipo(k.text().unwrap_or("").trim())),
        }
    }
    for g in h
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "degree")
    {
        let valor = texto_de(g, "degree-value").unwrap_or_default();
        let alter = texto_de(g, "degree-alter")
            .and_then(|t| t.parse::<f64>().ok())
            .map_or(0, |v| v.round() as i32);
        let tipo = texto_de(g, "degree-type").unwrap_or_default();
        let prefixo = match tipo.as_str() {
            "subtract" => "no",
            "add" => "add",
            _ => "",
        };
        s.push_str(&format!("({prefixo}{}{valor})", alteracao(alter)));
    }
    if let Some(b) = filho(h, "bass") {
        s.push('/');
        s.push_str(&nota_da_cifra(b, "bass-step", "bass-alter"));
    }
    s
}

impl Documento {
    /// Lê, do texto original, a letra, o dedilhado e a cifra (cada um ligado à sua nota) e a
    /// lista de elementos que o editor não desenha. Só leitura; barato o bastante para ser feito
    /// uma vez por abertura. Texto que não analisa devolve uma exibição vazia (o documento já
    /// foi aberto antes, então isso não acontece na prática).
    pub fn exibicao(&self) -> Exibicao {
        // O texto **atual** (o exportável): depois de inserir ou remover notas, os ordinais das
        // notas do original já não são os do compasso editado. Sem edição, é o próprio original.
        let atual = self.exportar().ok();
        let texto = atual.as_deref().unwrap_or_else(|| self.original());
        let Ok(doc) = crate::xml_seguro::analisar(texto) else {
            return Exibicao::default();
        };
        let mut ex = Exibicao::default();
        let mut contagem = Contagem::default();
        // Índice da parte entre as que têm compasso (o mesmo da `Partitura`).
        let mut indice_parte = 0usize;
        let partes_xml = doc
            .root_element()
            .children()
            .filter(|c| c.is_element() && c.tag_name().name() == "part");
        for (pi, parte_xml) in partes_xml.enumerate() {
            let Some(parte) = self.partes().get(pi) else {
                break;
            };
            let com_compasso = !parte.compassos.is_empty();
            let medidas = parte_xml
                .children()
                .filter(|c| c.is_element() && c.tag_name().name() == "measure");
            for (ci, m) in medidas.enumerate() {
                let Some(comp) = parte.compassos.get(ci) else {
                    break;
                };
                let numero = m.attribute("number").unwrap_or("");
                contagem.varrer(m, Lugar::Compasso, (&parte.id, numero));
                self.ler_compasso(&mut ex, m, comp, indice_parte, ci);
            }
            if com_compasso {
                indice_parte += 1;
            }
        }
        ex.nao_suportados = contagem.finalizar();
        ex
    }

    fn ler_compasso(
        &self,
        ex: &mut Exibicao,
        m: Node<'_, '_>,
        comp: &super::Compasso,
        indice_parte: usize,
        indice_compasso: usize,
    ) {
        let mut ordinal = 0usize;
        // Cifras sem nota ainda (esperam a próxima nota do compasso).
        let mut pendentes: Vec<usize> = Vec::new();
        for el in m.children().filter(Node::is_element) {
            match el.tag_name().name() {
                "harmony" => {
                    ex.cifras.push(Cifra {
                        parte: indice_parte,
                        compasso: indice_compasso,
                        texto: nome_da_cifra(el),
                        nota: None,
                    });
                    pendentes.push(ex.cifras.len() - 1);
                }
                "note" => {
                    let Some(nota) = comp.notas.get(ordinal) else {
                        ordinal += 1;
                        continue;
                    };
                    ordinal += 1;
                    for i in pendentes.drain(..) {
                        ex.cifras[i].nota = Some(nota.id);
                    }
                    let e = ElementosDaNota {
                        silabas: ler_silabas(el),
                        dedos: ler_dedos(el),
                    };
                    if !e.vazio() {
                        ex.por_nota.insert(nota.id, e);
                    }
                }
                _ => {}
            }
        }
        // Cifra depois da última nota: vale para a última nota anterior, se houver.
        if !pendentes.is_empty()
            && ordinal > 0
            && let Some(ultima) = comp.notas.get(ordinal - 1)
        {
            for i in pendentes {
                ex.cifras[i].nota = Some(ultima.id);
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const BASE: &str = "<?xml version=\"1.0\"?>\n<score-partwise version=\"4.0\">\n\
<part-list><score-part id=\"P1\"><part-name>A</part-name></score-part></part-list>\n\
<part id=\"P1\">";
    const FIM: &str = "</part>\n</score-partwise>\n";
    const ATRIB: &str = "<attributes><divisions>1</divisions><clef><sign>G</sign><line>2</line></clef></attributes>";

    fn nota(extra: &str) -> String {
        format!(
            "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>\
<voice>1</voice><type>quarter</type>{extra}</note>"
        )
    }

    fn doc(compassos: &str) -> Documento {
        Documento::abrir(&format!("{BASE}{compassos}{FIM}")).expect("abre")
    }

    #[test]
    fn letra_com_duas_estrofes_fica_na_nota_com_hifen_e_melisma() {
        let d = doc(&format!(
            "<measure number=\"1\">{ATRIB}{}</measure>",
            nota(
                "<lyric number=\"1\"><syllabic>begin</syllabic><text>Pi</text></lyric>\
<lyric number=\"2\"><syllabic>single</syllabic><text>lá</text><extend/></lyric>"
            )
        ));
        let ex = d.exibicao();
        let id = d.notas().next().expect("nota").id;
        let e = ex.da_nota(id).expect("elementos");
        assert_eq!(e.silabas.len(), 2);
        assert_eq!(e.silabas[0].estrofe, "1");
        assert_eq!(e.silabas[0].texto_exibido(), "Pi-");
        assert_eq!(e.silabas[1].estrofe, "2");
        assert!(e.silabas[1].prolongada);
        assert_eq!(e.silabas[1].texto_exibido(), "lá");
        assert_eq!(ex.estrofes(), vec!["1".to_owned(), "2".to_owned()]);
        assert_eq!(ex.total_de_silabas(), 2);
    }

    #[test]
    fn estrofes_ordenam_por_numero_e_depois_por_nome() {
        let mut v: Vec<String> = ["10", "2", "refrão", "1"].map(String::from).to_vec();
        ordenar_estrofes(&mut v);
        assert_eq!(v, ["1", "2", "10", "refrão"]);
    }

    #[test]
    fn elisao_junta_as_duas_partes_e_lyric_sem_texto_e_ignorado() {
        let d = doc(&format!(
            "<measure number=\"1\">{ATRIB}{}</measure>",
            nota(
                "<lyric><text>de</text><elision>_</elision><text>a</text></lyric>\
<lyric number=\"2\"><extend/></lyric>"
            )
        ));
        let ex = d.exibicao();
        let e = ex.da_nota(d.notas().next().expect("nota").id).expect("e");
        assert_eq!(e.silabas.len(), 1);
        assert_eq!(e.silabas[0].texto, "de_a");
    }

    #[test]
    fn dedilhado_com_substituicao_vira_texto_unico() {
        let d = doc(&format!(
            "<measure number=\"1\">{ATRIB}{}</measure>",
            nota(
                "<notations><technical><fingering>1</fingering>\
<fingering substitution=\"yes\">2</fingering></technical></notations>"
            )
        ));
        let ex = d.exibicao();
        let e = ex.da_nota(d.notas().next().expect("nota").id).expect("e");
        assert_eq!(e.dedos.len(), 2);
        assert!(e.dedos[1].substituicao);
        assert_eq!(e.dedilhado_em_texto(), "1-2");
        assert_eq!(ex.total_de_dedos(), 2);
    }

    #[test]
    fn cifra_vale_para_a_proxima_nota_ou_para_a_ultima_ou_fica_sem_ancora() {
        let h = |r: &str, k: &str| {
            format!("<harmony><root><root-step>{r}</root-step></root><kind>{k}</kind></harmony>")
        };
        let d = doc(&format!(
            "<measure number=\"1\">{ATRIB}{}{}{}{}</measure>\
<measure number=\"2\">{}</measure>",
            h("C", "major"),
            nota(""),
            nota(""),
            h("G", "dominant"),
            h("A", "minor"),
        ));
        let ids: Vec<IdNota> = d.notas().map(|n| n.id).collect();
        let ex = d.exibicao();
        let c = ex.cifras();
        assert_eq!(c.len(), 3);
        assert_eq!((c[0].texto.as_str(), c[0].nota), ("C", Some(ids[0])));
        // Depois da última nota do compasso: vale para ela.
        assert_eq!((c[1].texto.as_str(), c[1].nota), ("G7", Some(ids[1])));
        // Compasso sem nota: sem âncora, mas com posição (parte e compasso).
        assert_eq!((c[2].texto.as_str(), c[2].nota), ("Am", None));
        assert_eq!((c[2].parte, c[2].compasso), (0, 1));
    }

    #[test]
    fn nome_da_cifra_cobre_raiz_alterada_tipo_texto_grau_e_baixo() {
        let x = "<harmony><root><root-step>B</root-step><root-alter>-1</root-alter></root>\
<kind text=\"m7\">minor-seventh</kind><degree><degree-value>9</degree-value>\
<degree-alter>-1</degree-alter><degree-type>add</degree-type></degree>\
<bass><bass-step>F</bass-step><bass-alter>1</bass-alter></bass></harmony>";
        let r = roxmltree::Document::parse(x).expect("xml");
        assert_eq!(nome_da_cifra(r.root_element()), "Bbm7(addb9)/F#");
        // O `text=""` do arquivo vence o nome do tipo (acorde maior sem sufixo).
        let y =
            "<harmony><root><root-step>C</root-step></root><kind text=\"\">major</kind></harmony>";
        let r = roxmltree::Document::parse(y).expect("xml");
        assert_eq!(nome_da_cifra(r.root_element()), "C");
        let z = "<harmony><kind>none</kind></harmony>";
        let r = roxmltree::Document::parse(z).expect("xml");
        assert_eq!(nome_da_cifra(r.root_element()), "N.C.");
    }

    #[test]
    fn nao_suportados_listam_nome_categoria_e_quantidade_e_desconhecido_nao_aborta() {
        let d = doc(&format!(
            "<measure number=\"1\">{ATRIB}\
<direction><direction-type><dynamics><mf/></dynamics></direction-type></direction>\
{}{}\
<x-elemento-inexistente><filho>t</filho></x-elemento-inexistente></measure>",
            nota(
                "<notations><ornaments><trill-mark/></ornaments>\
<articulations><staccato/></articulations><x-extensao-futura valor=\"1\"/></notations>"
            ),
            nota("<notations><dynamics><p/></dynamics><fermata/></notations>"),
        ));
        let ex = d.exibicao();
        let achar = |n: &str| {
            ex.nao_suportados()
                .iter()
                .find(|e| e.nome == n)
                .unwrap_or_else(|| panic!("{n} não listado: {:?}", ex.nao_suportados()))
        };
        assert_eq!(achar("dynamics").quantidade, 2);
        assert_eq!(achar("dynamics").categoria, "dinâmica");
        assert!(achar("dynamics").reconhecido);
        assert_eq!(achar("trill-mark").categoria, "ornamento");
        assert_eq!(achar("staccato").categoria, "articulação");
        assert_eq!(achar("fermata").quantidade, 1);
        assert!(!achar("x-extensao-futura").reconhecido);
        assert!(!achar("x-elemento-inexistente").reconhecido);
        assert_eq!(
            achar("x-elemento-inexistente").primeiro,
            Some(("P1".to_owned(), "1".to_owned()))
        );
        // Dinâmica é a mais frequente: vem primeiro.
        assert_eq!(ex.nao_suportados()[0].nome, "dynamics");
        let linhas = ex.linhas_de_nao_suportados();
        assert!(
            linhas.contains(&"dynamics (dinâmica): 2".to_owned()),
            "{linhas:?}"
        );
        assert!(
            linhas.contains(&"x-extensao-futura (desconhecido): 1".to_owned()),
            "{linhas:?}"
        );
    }

    #[test]
    fn o_que_o_editor_desenha_nao_entra_na_lista() {
        let d = doc(&format!(
            "<measure number=\"1\">{ATRIB}{}{}</measure>",
            nota(
                "<lyric><text>a</text></lyric><notations><slur type=\"start\"/>\
<technical><fingering>3</fingering></technical></notations>"
            ),
            nota(
                "<beam number=\"1\">begin</beam><tie type=\"start\"/><accidental>sharp</accidental>"
            ),
        ));
        assert!(d.exibicao().nao_suportados().is_empty());
    }

    #[test]
    fn documento_sem_extras_tem_exibicao_vazia() {
        let d = doc(&format!(
            "<measure number=\"1\">{ATRIB}{}</measure>",
            nota("")
        ));
        assert!(d.exibicao().vazia());
    }

    #[test]
    fn cifra_de_parte_vazia_nao_desloca_o_indice_da_parte() {
        // A segunda parte não tem compasso: a terceira é a segunda da Partitura.
        let xml = "<?xml version=\"1.0\"?>\n<score-partwise version=\"4.0\">\n\
<part-list><score-part id=\"P1\"/><score-part id=\"P2\"/><score-part id=\"P3\"/></part-list>\n\
<part id=\"P1\"><measure number=\"1\"><attributes><divisions>1</divisions></attributes>\
<note><rest/><duration>4</duration><voice>1</voice></note></measure></part>\n\
<part id=\"P2\"></part>\n\
<part id=\"P3\"><measure number=\"1\"><attributes><divisions>1</divisions></attributes>\
<harmony><root><root-step>D</root-step></root><kind>major</kind></harmony>\
<note><rest/><duration>4</duration><voice>1</voice></note></measure></part>\n\
</score-partwise>\n";
        let d = Documento::abrir(xml).expect("abre");
        let ex = d.exibicao();
        assert_eq!(ex.cifras().len(), 1);
        assert_eq!(ex.cifras()[0].parte, 1);
    }
}
