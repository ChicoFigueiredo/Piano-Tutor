//! Grafia das alturas (ticket 14, E01): regras puras, sem documento e sem texto, de **como
//! escrever** a nota que resulta de subir ou descer. Tudo aqui é função de entrada para saída,
//! com tabela de testes.
//!
//! # Regras (em linguagem de domínio)
//!
//! - **Semitom**: a altura sonora muda em ±1 semitom; a grafia é escolhida entre as candidatas
//!   (natural, sustenido da letra de baixo, bemol da letra de cima):
//!   1. a que a **armadura** já altera do mesmo jeito (em Sol maior, o Fá sustenido; em Mi bemol
//!      maior, o Mi bemol e o Lá bemol);
//!   2. senão, a tecla branca (sem acidente próprio);
//!   3. senão, a armadura decide: com sustenidos, sustenido; com bemóis, bemol; sem armadura,
//!      subindo escreve-se sustenido e descendo, bemol.
//! - **Grau da escala**: a letra seguinte ou anterior, com a alteração que a armadura dá a ela.
//! - **Oitava**: a mesma letra e alteração, uma oitava acima ou abaixo.
//! - **Acidente impresso**: só quando a alteração da nota difere da que está em vigor naquele
//!   ponto do compasso (a da armadura ou a do último acidente da mesma letra e oitava na mesma
//!   pauta). Acidentes de compassos anteriores e ligaduras entre compassos não são considerados
//!   (limite declarado).
//! - A armadura é maior ou menor indistintamente: o que vale são as quintas. A alteração do
//!   sétimo grau da menor harmônica é sempre um acidente explícito, não da armadura.

use super::Altura;
use crate::partitura::{self, Fracao};
use std::collections::HashSet;

const LETRAS: [char; 7] = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];
const SEMITONS: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
const ORDEM_DOS_SUSTENIDOS: [char; 7] = ['F', 'C', 'G', 'D', 'A', 'E', 'B'];
const ORDEM_DOS_BEMOIS: [char; 7] = ['B', 'E', 'A', 'D', 'G', 'C', 'F'];

/// Posição da letra de `C` (0) a `B` (6).
pub fn indice_da_letra(letra: char) -> Option<usize> {
    LETRAS.iter().position(|l| *l == letra)
}

/// Número MIDI da altura (Dó4 = 60), sem checar limites. `None` se a letra não existe.
pub fn midi(a: Altura) -> Option<i32> {
    let i = indice_da_letra(a.passo)?;
    Some((i32::from(a.oitava) + 1) * 12 + SEMITONS[i] + i32::from(a.alter))
}

/// Alteração que a armadura (`quintas` > 0 sustenidos, < 0 bemóis) dá à letra.
pub fn alter_da_armadura(quintas: i8, letra: char) -> i8 {
    let n = usize::from(quintas.unsigned_abs()).min(7);
    let ordem = if quintas >= 0 {
        &ORDEM_DOS_SUSTENIDOS
    } else {
        &ORDEM_DOS_BEMOIS
    };
    if ordem[..n].contains(&letra) {
        quintas.signum()
    } else {
        0
    }
}

/// Texto do `<accidental>` do MusicXML que corresponde a uma alteração inteira.
pub fn acidente_de(alter: i8) -> &'static str {
    match alter {
        1 => "sharp",
        -1 => "flat",
        2 => "double-sharp",
        -2 => "flat-flat",
        _ => "natural",
    }
}

/// Separador da forma **bruta** da grafia: `conteúdo␟abertura`. É a forma que `grafia_do_acidente`
/// devolve quando o `<accidental>` do arquivo tem espaço atípico (vários espaços, quebra de linha,
/// espaço antes do `>`, espaço em volta do nome): guarda o conteúdo e o texto da marca de
/// abertura (o que vem depois de `<accidental`) **byte a byte**, para o `inverso` restaurar o
/// elemento sem normalizar nada. U+001F não é espaço nem pode aparecer num nome ou atributo
/// válido.
pub const SEPARADOR_BRUTO: char = '\u{1F}';

/// Separa a **grafia completa** de um `<accidental>` em nome (o conteúdo do elemento, ex.:
/// `natural`) e atributos (o texto da marca de abertura depois do nome do elemento, ex.:
/// `cautionary="yes" parentheses="no"`; vazio se não há). A grafia é `nome` ou
/// `nome atributos` (ou a forma bruta, ver [`SEPARADOR_BRUTO`]; nela o nome e os atributos saem
/// aparados): é o que o `inverso` guarda para restaurar o acidente sem perder nada.
pub fn separar_acidente(grafia: &str) -> (&str, &str) {
    if let Some((conteudo, abertura)) = grafia.split_once(SEPARADOR_BRUTO) {
        return (conteudo.trim(), abertura.trim());
    }
    let g = grafia.trim();
    match g.split_once(char::is_whitespace) {
        Some((nome, atributos)) => (nome, atributos.trim()),
        None => (g, ""),
    }
}

/// Conteúdo e marca de abertura (o que vem depois de `<accidental`, com o espaço inicial) de uma
/// grafia, exatamente como vão para o XML.
pub fn partes_do_acidente(grafia: &str) -> (String, String) {
    if let Some((conteudo, abertura)) = grafia.split_once(SEPARADOR_BRUTO) {
        return (conteudo.to_owned(), abertura.to_owned());
    }
    let (nome, atributos) = separar_acidente(grafia);
    let abertura = if atributos.is_empty() {
        String::new()
    } else {
        format!(" {atributos}")
    };
    (nome.to_owned(), abertura)
}

/// Nomes de acidente do MusicXML 4.0 (`accidental-value`): lista fechada.
const ACIDENTES: &[&str] = &[
    "sharp",
    "natural",
    "flat",
    "double-sharp",
    "sharp-sharp",
    "flat-flat",
    "natural-sharp",
    "natural-flat",
    "quarter-flat",
    "quarter-sharp",
    "three-quarters-flat",
    "three-quarters-sharp",
    "sharp-down",
    "sharp-up",
    "natural-down",
    "natural-up",
    "flat-down",
    "flat-up",
    "double-sharp-down",
    "double-sharp-up",
    "flat-flat-down",
    "flat-flat-up",
    "arrow-down",
    "arrow-up",
    "triple-sharp",
    "triple-flat",
    "slash-quarter-sharp",
    "slash-sharp",
    "slash-flat",
    "double-slash-flat",
    "sharp-1",
    "sharp-2",
    "sharp-3",
    "sharp-5",
    "flat-1",
    "flat-2",
    "flat-3",
    "flat-4",
    "sori",
    "koron",
    "other",
];

/// Atributos que o esquema MusicXML 4.0 permite em `<accidental>`: lista fechada.
const ATRIBUTOS_DO_ACIDENTE: &[&str] = &[
    "cautionary",
    "editorial",
    "parentheses",
    "bracket",
    "size",
    "smufl",
    "id",
    "color",
    "default-x",
    "default-y",
    "relative-x",
    "relative-y",
    "font-family",
    "font-size",
    "font-style",
    "font-weight",
];

fn espaco_xml(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n')
}

/// Produção `Char` do XML 1.0: o que pode aparecer num documento (T2 da terceira revisão).
/// Recusa controles (menos tab, LF e CR), `U+FFFE` e `U+FFFF`; os substitutos (`U+D800` a
/// `U+DFFF`) e o que passa de `U+10FFFF` nem são `char` em Rust.
pub(crate) fn caractere_xml(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
    )
}

/// A referência numérica (`&#...;` ou `&#x...;`, sem o `&` e o `;`) nomeia um `Char` do XML?
fn referencia_numerica_valida(nome: &str) -> bool {
    let valor = match nome.strip_prefix("#x") {
        Some(h) => {
            if h.is_empty() || !h.bytes().all(|c| c.is_ascii_hexdigit()) {
                return false;
            }
            u32::from_str_radix(h, 16)
        }
        None => match nome.strip_prefix('#') {
            Some(d) if !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit()) => d.parse::<u32>(),
            _ => return false,
        },
    };
    valor
        .ok()
        .and_then(char::from_u32)
        .is_some_and(caractere_xml)
}

/// Referências de entidade bem formadas: as cinco predefinidas e as numéricas que nomeiam um
/// caractere válido do XML 1.0 (`&#0;`, `&#xFFFE;`, substitutos e `> U+10FFFF` são recusados).
fn entidades_validas(valor: &str) -> bool {
    let mut resto = valor;
    while let Some(i) = resto.find('&') {
        let depois = &resto[i + 1..];
        let Some(fim) = depois.find(';') else {
            return false;
        };
        let nome = &depois[..fim];
        let ok = matches!(nome, "amp" | "lt" | "gt" | "quot" | "apos")
            || referencia_numerica_valida(nome);
        if !ok {
            return false;
        }
        resto = &depois[fim + 1..];
    }
    true
}

/// A marca de abertura (o que vem depois de `<accidental`) só tem atributos da lista fechada, cada
/// um uma vez, com valor entre aspas, sem `<`, sem controle e com `&` só em referência de
/// entidade. `>` e as outras aspas são válidos dentro do valor (XML).
fn abertura_valida(abertura: &str) -> bool {
    abertura_bem_formada(abertura, true)
}

/// Sintaxe de atributos XML da marca de abertura. Com `restrito`, só os nomes da lista fechada
/// do esquema; sem ele, qualquer nome de atributo XML (para bytes que já estão no arquivo).
fn abertura_bem_formada(abertura: &str, restrito: bool) -> bool {
    let b = abertura.as_bytes();
    let mut i = 0;
    let mut vistos: Vec<&str> = Vec::new();
    loop {
        let ini = i;
        while i < b.len() && espaco_xml(b[i]) {
            i += 1;
        }
        if i == b.len() {
            return true;
        }
        if i == ini {
            return false; // atributo colado no anterior (ou na marca): falta espaço
        }
        let n0 = i;
        while i < b.len()
            && (b[i].is_ascii_alphanumeric()
                || b[i] == b'-'
                || (!restrito && matches!(b[i], b'_' | b':' | b'.')))
        {
            i += 1;
        }
        let nome = &abertura[n0..i];
        if nome.is_empty()
            || (restrito && !ATRIBUTOS_DO_ACIDENTE.contains(&nome))
            || vistos.contains(&nome)
        {
            return false;
        }
        vistos.push(nome);
        while i < b.len() && espaco_xml(b[i]) {
            i += 1;
        }
        if b.get(i) != Some(&b'=') {
            return false;
        }
        i += 1;
        while i < b.len() && espaco_xml(b[i]) {
            i += 1;
        }
        let Some(&aspa) = b.get(i).filter(|c| matches!(c, b'"' | b'\'')) else {
            return false;
        };
        i += 1;
        let v0 = i;
        while i < b.len() && b[i] != aspa {
            i += 1;
        }
        if i == b.len() {
            return false;
        }
        let valor = &abertura[v0..i];
        if valor.contains('<') || !valor.chars().all(caractere_xml) || !entidades_validas(valor) {
            return false;
        }
        i += 1;
    }
}

/// Nomes dos atributos de uma marca de abertura já conferida por [`abertura_bem_formada`].
fn nomes_dos_atributos(abertura: &str) -> Vec<&str> {
    let b = abertura.as_bytes();
    let mut nomes = Vec::new();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && espaco_xml(b[i]) {
            i += 1;
        }
        let n0 = i;
        while i < b.len() && b[i] != b'=' && !espaco_xml(b[i]) {
            i += 1;
        }
        if n0 == i {
            break;
        }
        nomes.push(&abertura[n0..i]);
        while i < b.len() && b[i] != b'"' && b[i] != b'\'' {
            i += 1;
        }
        let Some(&aspa) = b.get(i) else { break };
        i += 1;
        while i < b.len() && b[i] != aspa {
            i += 1;
        }
        i += 1;
    }
    nomes
}

/// Todo atributo com prefixo de namespace usa um prefixo em escopo: `xml`, um dos `escopo`
/// (declarados fora do elemento, nos ancestrais) ou um `xmlns:prefixo` da própria marca.
fn prefixos_em_escopo(abertura: &str, escopo: &[&str]) -> bool {
    let nomes = nomes_dos_atributos(abertura);
    nomes.iter().all(|nome| {
        let Some((prefixo, _)) = nome.split_once(':') else {
            return true;
        };
        prefixo == "xml"
            || prefixo == "xmlns"
            || escopo.contains(&prefixo)
            || nomes
                .iter()
                .any(|n| n.strip_prefix("xmlns:") == Some(prefixo))
    })
}

/// Pedaços de `<accidental>` que **já existem no arquivo** (conteúdos e marcas de abertura, byte
/// a byte). São confiáveis: o princípio de preservação (ADR-0017) manda que o que o modelo não
/// conhece sobreviva, inclusive atributos fora do esquema, e o `inverso` precisa devolvê-los.
/// Quem os colhe lê **elementos que o parser reconheceu** (comentário, CDATA e instrução de
/// processamento não contam) e aqui só confere a sintaxe; quem os usa só aceita pedaço
/// **idêntico** a um deles. Colhidos uma vez por abertura (conjuntos: consulta sem varredura).
#[derive(Clone, Debug, Default)]
pub struct PedacosDoArquivo {
    conteudos: HashSet<String>,
    aberturas: HashSet<String>,
}

impl PedacosDoArquivo {
    /// Registra um `<accidental>` do arquivo (conteúdo e o texto depois de `<accidental`), se a
    /// sintaxe é a de um elemento XML bem formado, o conteúdo só tem `Char` do XML sem `]]>` e
    /// todo prefixo de namespace dos atributos está em `escopo` (ou declarado na própria marca).
    pub fn registrar(&mut self, conteudo: &str, abertura: &str, escopo: &[&str]) {
        let conteudo_ok = !conteudo.contains(['<', SEPARADOR_BRUTO])
            && !conteudo.contains("]]>")
            && conteudo.chars().all(caractere_xml)
            && !conteudo
                .chars()
                .any(|c| c.is_control() && !c.is_whitespace())
            && entidades_validas(conteudo);
        if !conteudo_ok
            || !abertura_bem_formada(abertura, false)
            || !prefixos_em_escopo(abertura, escopo)
        {
            return;
        }
        if !self.conteudos.contains(conteudo) {
            self.conteudos.insert(conteudo.to_owned());
        }
        if !self.aberturas.contains(abertura) {
            self.aberturas.insert(abertura.to_owned());
        }
    }
}

/// **Validação estrita** (lista fechada): o nome é um dos `accidental-value` do MusicXML (com
/// espaço só nas pontas, na forma bruta), os atributos são da lista permitida, sem repetição, com
/// valor entre aspas e escapado. Nenhum texto que passe aqui consegue fechar o elemento, abrir
/// outro ou pôr atributo fora do esquema. Com `arquivo` vazio é só isso; além dela, aceita também
/// conteúdo e marca de abertura **idênticos** a um pedaço que já existe no arquivo. Um chamador não forja bytes do arquivo:
/// o que não está lá (ou só está parecido) segue sob a lista fechada.
pub fn grafia_de_acidente_aceita(grafia: &str, arquivo: &PedacosDoArquivo) -> bool {
    let (conteudo, abertura) = partes_do_acidente(grafia);
    let nome = conteudo.trim_matches([' ', '\t', '\r', '\n']);
    (ACIDENTES.contains(&nome) || arquivo.conteudos.contains(&conteudo))
        && (abertura_valida(&abertura) || arquivo.aberturas.contains(&abertura))
}

/// O nome do acidente (`natural`, `sharp`...) de uma grafia completa.
pub fn nome_do_acidente(grafia: &str) -> &str {
    separar_acidente(grafia).0
}

/// Alteração que um texto de `<accidental>` expressa; `None` para os que a grafia não entende
/// (quarto de tom, setas, etc.).
pub fn alter_do_acidente(texto: &str) -> Option<i8> {
    Some(match nome_do_acidente(texto) {
        "sharp" => 1,
        "flat" => -1,
        "natural" => 0,
        "double-sharp" | "sharp-sharp" => 2,
        "flat-flat" => -2,
        _ => return None,
    })
}

/// Sobe ou desce um semitom. O resultado pode sair dos limites: quem chama confere.
pub fn por_semitom(atual: Altura, acima: bool, quintas: i8) -> Option<Altura> {
    let alvo = midi(atual)? + if acima { 1 } else { -1 };
    let pc = alvo.rem_euclid(12);
    // Candidatas: letra e alteração (-1, 0, +1) que dão a classe de altura `pc`.
    let candidatas: Vec<(char, i8)> = LETRAS
        .iter()
        .zip(SEMITONS)
        .flat_map(|(l, s)| (-1_i8..=1).map(move |a| (*l, s, a)))
        .filter(|(_, s, a)| (s + i32::from(*a)).rem_euclid(12) == pc)
        .map(|(l, _, a)| (l, a))
        .collect();
    let escolhida = candidatas
        .iter()
        .find(|(l, a)| *a == alter_da_armadura(quintas, *l))
        .or_else(|| candidatas.iter().find(|(_, a)| *a == 0))
        .or_else(|| {
            let quer = match quintas.signum() {
                0 => i8::from(acima) * 2 - 1,
                s => s,
            };
            candidatas.iter().find(|(_, a)| *a == quer)
        })
        .or(candidatas.first())?;
    let (letra, alter) = *escolhida;
    let i = indice_da_letra(letra)?;
    let oitava = (alvo - SEMITONS[i] - i32::from(alter)).div_euclid(12) - 1;
    Some(Altura {
        passo: letra,
        alter,
        oitava: i8::try_from(oitava).ok()?,
    })
}

/// Sobe ou desce um grau da escala da armadura.
pub fn por_grau(atual: Altura, acima: bool, quintas: i8) -> Option<Altura> {
    let i = indice_da_letra(atual.passo)?;
    let (novo, oitava) = match (i, acima) {
        (6, true) => (0, atual.oitava.checked_add(1)?),
        (0, false) => (6, atual.oitava.checked_sub(1)?),
        (_, true) => (i + 1, atual.oitava),
        (_, false) => (i - 1, atual.oitava),
    };
    let letra = LETRAS[novo];
    Some(Altura {
        passo: letra,
        alter: alter_da_armadura(quintas, letra),
        oitava,
    })
}

/// Sobe ou desce uma oitava, mantendo letra e alteração.
pub fn por_oitava(atual: Altura, acima: bool) -> Option<Altura> {
    Some(Altura {
        oitava: if acima {
            atual.oitava.checked_add(1)?
        } else {
            atual.oitava.checked_sub(1)?
        },
        ..atual
    })
}

/// Uma nota anterior no compasso, só com o que importa para o acidente em vigor.
#[derive(Clone, Copy, Debug)]
pub struct Anterior {
    pub letra: char,
    pub oitava: i8,
    pub alter: i8,
    /// Início dentro do compasso, em semínimas.
    pub posicao: Fracao,
}

/// Alteração em vigor para `letra` e `oitava`: a do último acidente da mesma letra e oitava (por
/// tempo; no empate vale a que vem depois na lista) ou, se não houver, a da armadura.
pub fn alteracao_em_vigor(quintas: i8, letra: char, oitava: i8, anteriores: &[Anterior]) -> i8 {
    let mut melhor: Option<&Anterior> = None;
    for a in anteriores
        .iter()
        .filter(|a| a.letra == letra && a.oitava == oitava)
    {
        if melhor.is_none_or(|m| !antes(a.posicao, m.posicao)) {
            melhor = Some(a);
        }
    }
    melhor.map_or_else(|| alter_da_armadura(quintas, letra), |a| a.alter)
}

/// `a` começa estritamente antes de `b`?
pub fn antes(a: Fracao, b: Fracao) -> bool {
    i128::from(a.numerador()) * i128::from(b.denominador())
        < i128::from(b.numerador()) * i128::from(a.denominador())
}

/// Letra, alteração e oitava do modelo de tocar para o do documento.
pub fn de_partitura(a: partitura::Altura) -> Altura {
    Altura {
        passo: LETRAS[usize::from(a.passo.min(6))],
        alter: a.alteracao,
        oitava: a.oitava,
    }
}

/// Altura do documento para a do modelo de tocar.
pub fn para_partitura(a: Altura) -> Option<partitura::Altura> {
    Some(partitura::Altura {
        passo: u8::try_from(indice_da_letra(a.passo)?).ok()?,
        alteracao: a.alter,
        oitava: a.oitava,
    })
}

const NOMES: [&str; 7] = ["Dó", "Ré", "Mi", "Fá", "Sol", "Lá", "Si"];

/// Nome em português, sem símbolos fora da fonte: "Fá sustenido 4".
pub fn nome_da_altura(a: Altura) -> String {
    let nome = indice_da_letra(a.passo).map_or("?", |i| NOMES[i]);
    let alteracao = match a.alter {
        0 => String::new(),
        1 => " sustenido".to_owned(),
        -1 => " bemol".to_owned(),
        2 => " dobrado sustenido".to_owned(),
        -2 => " dobrado bemol".to_owned(),
        n => format!(" ({n:+} semitons)"),
    };
    format!("{nome}{alteracao} {}", a.oitava)
}

/// Nome de um número MIDI (grafia com sustenido): 60 vira "Dó 4".
pub fn nome_do_midi(m: i32) -> String {
    const POR_CLASSE: [(char, i8); 12] = [
        ('C', 0),
        ('C', 1),
        ('D', 0),
        ('D', 1),
        ('E', 0),
        ('F', 0),
        ('F', 1),
        ('G', 0),
        ('G', 1),
        ('A', 0),
        ('A', 1),
        ('B', 0),
    ];
    let (passo, alter) = POR_CLASSE[usize::try_from(m.rem_euclid(12)).unwrap_or(0)];
    let oitava = i8::try_from(m.div_euclid(12) - 1).unwrap_or(i8::MAX);
    nome_da_altura(Altura {
        passo,
        alter,
        oitava,
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn a(passo: char, alter: i8, oitava: i8) -> Altura {
        Altura {
            passo,
            alter,
            oitava,
        }
    }

    #[test]
    fn armadura_altera_as_letras_na_ordem_das_quintas() {
        let sust = |q| {
            LETRAS
                .iter()
                .filter(|l| alter_da_armadura(q, **l) == 1)
                .collect::<String>()
        };
        assert_eq!(sust(0), "");
        assert_eq!(sust(1), "F");
        assert_eq!(sust(2), "CF");
        assert_eq!(sust(7), "CDEFGAB");
        let bem = |q| {
            LETRAS
                .iter()
                .filter(|l| alter_da_armadura(q, **l) == -1)
                .collect::<String>()
        };
        assert_eq!(bem(-1), "B");
        assert_eq!(bem(-3), "EAB");
        assert_eq!(bem(-7), "CDEFGAB");
        // Quintas absurdas não estouram.
        assert_eq!(alter_da_armadura(i8::MAX, 'C'), 1);
        assert_eq!(alter_da_armadura(i8::MIN, 'C'), -1);
    }

    /// Tabela de enarmonia por semitom: (armadura, atual, sobe?, esperado).
    #[test]
    fn semitom_escolhe_a_grafia_pela_armadura_e_pela_direcao() {
        let t: &[(i8, Altura, bool, Altura)] = &[
            // Dó maior: subindo sustenido, descendo bemol; teclas brancas sem acidente.
            (0, a('C', 0, 4), true, a('C', 1, 4)),
            (0, a('C', 1, 4), true, a('D', 0, 4)),
            (0, a('D', 0, 4), true, a('D', 1, 4)),
            (0, a('D', 0, 4), false, a('D', -1, 4)),
            (0, a('D', -1, 4), false, a('C', 0, 4)),
            (0, a('E', 0, 4), true, a('F', 0, 4)),
            (0, a('F', 0, 4), false, a('E', 0, 4)),
            (0, a('F', 0, 4), true, a('F', 1, 4)),
            (0, a('G', 0, 4), false, a('G', -1, 4)),
            (0, a('B', 0, 4), true, a('C', 0, 5)),
            (0, a('C', 0, 4), false, a('B', 0, 3)),
            // A grafia de partida não manda: Si bemol sobe para Si, Dó sustenido desce para Dó.
            (0, a('B', -1, 3), true, a('B', 0, 3)),
            (0, a('C', 1, 4), false, a('C', 0, 4)),
            // Sol maior (Fá sustenido): o Fá sustenido é diatônico.
            (1, a('E', 0, 4), true, a('F', 0, 4)),
            (1, a('F', 1, 4), true, a('G', 0, 4)),
            (1, a('G', 0, 4), false, a('F', 1, 4)),
            (1, a('F', 0, 4), true, a('F', 1, 4)),
            (1, a('G', 0, 4), true, a('G', 1, 4)),
            // Ré maior (Fá e Dó sustenidos).
            (2, a('B', 0, 3), true, a('C', 0, 4)),
            (2, a('D', 0, 4), false, a('C', 1, 4)),
            (2, a('D', 0, 4), true, a('D', 1, 4)),
            // Fá maior (Si bemol): descendo de Si vai a Si bemol; subindo de Lá, Si bemol.
            (-1, a('B', 0, 3), false, a('B', -1, 3)),
            (-1, a('A', 0, 3), true, a('B', -1, 3)),
            (-1, a('C', 0, 4), true, a('D', -1, 4)),
            // Mi bemol maior (Si, Mi e Lá bemóis).
            (-3, a('D', 0, 4), true, a('E', -1, 4)),
            (-3, a('G', 0, 4), true, a('A', -1, 4)),
            (-3, a('A', 0, 4), false, a('A', -1, 4)),
            (-3, a('F', 0, 4), true, a('G', -1, 4)),
            (-3, a('C', 0, 4), true, a('D', -1, 4)),
            // Casos extremos de armadura: Dó sustenido maior tem Mi e Si sustenidos.
            (7, a('D', 1, 4), true, a('E', 0, 4)),
            (7, a('E', 0, 4), true, a('E', 1, 4)),
            (7, a('B', 0, 3), true, a('B', 1, 3)),
            (7, a('C', 1, 4), true, a('D', 0, 4)),
            // Dó bemol maior tem Dó e Fá bemóis.
            (-7, a('B', -1, 3), true, a('C', -1, 4)),
            (-7, a('E', -1, 4), true, a('F', -1, 4)),
        ];
        for (q, atual, sobe, esperado) in t {
            let r = por_semitom(*atual, *sobe, *q).expect("resolve");
            assert_eq!(
                (r, midi(r)),
                (*esperado, midi(*esperado)),
                "quintas {q}, de {} {}",
                nome_da_altura(*atual),
                if *sobe { "sobe" } else { "desce" }
            );
        }
    }

    #[test]
    fn semitom_muda_exatamente_um_semitom_sonoro_em_toda_armadura_e_altura() {
        for q in -7..=7_i8 {
            for letra in LETRAS {
                for alter in -1..=1_i8 {
                    for oitava in 1..=7_i8 {
                        let atual = a(letra, alter, oitava);
                        for sobe in [true, false] {
                            let r = por_semitom(atual, sobe, q).expect("resolve");
                            let delta = midi(r).expect("midi") - midi(atual).expect("midi");
                            assert_eq!(delta, if sobe { 1 } else { -1 }, "{atual:?} {q} {r:?}");
                            assert!(r.alter.abs() <= 1, "{r:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn grau_da_escala_segue_a_armadura_e_atravessa_a_oitava() {
        let t: &[(i8, Altura, bool, Altura)] = &[
            (0, a('C', 0, 4), true, a('D', 0, 4)),
            (0, a('B', 0, 4), true, a('C', 0, 5)),
            (0, a('C', 0, 4), false, a('B', 0, 3)),
            (1, a('E', 0, 4), true, a('F', 1, 4)),
            (1, a('G', 0, 4), false, a('F', 1, 4)),
            (-1, a('A', 0, 4), true, a('B', -1, 4)),
            (-3, a('D', 0, 4), true, a('E', -1, 4)),
            // Uma nota cromática sai para o grau vizinho da escala.
            (0, a('C', 1, 4), true, a('D', 0, 4)),
            (0, a('E', -1, 4), false, a('D', 0, 4)),
            (2, a('B', 0, 3), true, a('C', 1, 4)),
        ];
        for (q, atual, sobe, esperado) in t {
            assert_eq!(
                por_grau(*atual, *sobe, *q),
                Some(*esperado),
                "quintas {q}, {atual:?}"
            );
        }
        assert_eq!(por_grau(a('B', 0, i8::MAX), true, 0), None);
    }

    #[test]
    fn oitava_mantem_letra_e_alteracao() {
        assert_eq!(por_oitava(a('F', 1, 4), true), Some(a('F', 1, 5)));
        assert_eq!(por_oitava(a('B', -1, 4), false), Some(a('B', -1, 3)));
        assert_eq!(por_oitava(a('C', 0, i8::MIN), false), None);
    }

    #[test]
    fn alteracao_em_vigor_usa_o_ultimo_acidente_da_mesma_letra_e_oitava() {
        let f = |n, d| Fracao::nova(n, d);
        let ant = [
            Anterior {
                letra: 'F',
                oitava: 4,
                alter: 1,
                posicao: f(0, 1),
            },
            Anterior {
                letra: 'F',
                oitava: 5,
                alter: -1,
                posicao: f(1, 1),
            },
            Anterior {
                letra: 'F',
                oitava: 4,
                alter: 0,
                posicao: f(2, 1),
            },
        ];
        // Em Dó maior, o último Fá4 do compasso (bequadro) vale; o de outra oitava não conta.
        assert_eq!(alteracao_em_vigor(0, 'F', 4, &ant), 0);
        assert_eq!(alteracao_em_vigor(0, 'F', 5, &ant), -1);
        assert_eq!(alteracao_em_vigor(0, 'F', 3, &ant), 0);
        // Sem anterior, vale a armadura.
        assert_eq!(alteracao_em_vigor(1, 'F', 3, &ant), 1);
        assert_eq!(alteracao_em_vigor(-1, 'B', 4, &[]), -1);
        // A ordem na lista não importa: vale o mais tardio no tempo.
        let mut invertida = ant;
        invertida.reverse();
        assert_eq!(alteracao_em_vigor(0, 'F', 4, &invertida), 0);
    }

    #[test]
    fn acidentes_de_ida_e_volta_e_nomes() {
        for alter in -2..=2_i8 {
            assert_eq!(alter_do_acidente(acidente_de(alter)), Some(alter));
        }
        assert_eq!(alter_do_acidente("quarter-sharp"), None);
        assert_eq!(nome_da_altura(a('F', 1, 4)), "Fá sustenido 4");
        assert_eq!(nome_do_midi(60), "Dó 4");
        assert_eq!(nome_do_midi(21), "Lá 0");
        assert_eq!(nome_do_midi(108), "Dó 8");
        let p = para_partitura(a('A', -1, 3)).expect("converte");
        assert_eq!((p.passo, p.alteracao, p.oitava), (5, -1, 3));
        assert_eq!(de_partitura(p), a('A', -1, 3));
        assert_eq!(
            p.numero_midi(),
            midi(a('A', -1, 3)).and_then(|m| u8::try_from(m).ok())
        );
    }

    #[test]
    fn referencia_de_caractere_segue_a_producao_char_do_xml() {
        for ruim in [
            "&#0;",
            "&#8;",
            "&#xB;",
            "&#xD800;",
            "&#xFFFE;",
            "&#xFFFF;",
            "&#x110000;",
            "&#99999999999;",
            "&#x;",
        ] {
            assert!(!entidades_validas(ruim), "{ruim}");
        }
        for boa in ["&#9;", "&#xA;", "&#13;", "&#x20;", "&#xFFFD;", "&#x10FFFF;"] {
            assert!(entidades_validas(boa), "{boa}");
        }
        assert!(!abertura_valida(" cautionary=\"\u{FFFE}\""));
        assert!(!abertura_valida(" cautionary=\"\u{1}\""));
        assert!(abertura_valida(" cautionary=\"\u{10000}\""));
    }

    #[test]
    fn pedacos_do_arquivo_exigem_prefixo_em_escopo_e_sem_fim_de_cdata() {
        let mut p = PedacosDoArquivo::default();
        p.registrar("sharp", " xmlns:foo=\"u\" foo:bar=\"1\"", &[]);
        p.registrar("sharp]]>y", "", &[]);
        p.registrar("sharp", " cautionary=\"&#0;\"", &[]);
        assert!(grafia_de_acidente_aceita(
            "sharp xmlns:foo=\"u\" foo:bar=\"1\"",
            &p
        ));
        assert!(!grafia_de_acidente_aceita("sharp]]>y", &p));
        assert!(!grafia_de_acidente_aceita("sharp cautionary=\"&#0;\"", &p));
        let mut sem_escopo = PedacosDoArquivo::default();
        sem_escopo.registrar("sharp", " foo:bar=\"1\"", &[]);
        assert!(!grafia_de_acidente_aceita(
            "sharp foo:bar=\"1\"",
            &sem_escopo
        ));
        let mut com_escopo = PedacosDoArquivo::default();
        com_escopo.registrar("sharp", " foo:bar=\"1\"", &["foo"]);
        assert!(grafia_de_acidente_aceita(
            "sharp foo:bar=\"1\"",
            &com_escopo
        ));
    }
}
