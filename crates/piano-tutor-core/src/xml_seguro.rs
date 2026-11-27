//! Porta única para analisar XML não confiável (MusicXML e `container.xml`): é aqui que ficam a
//! política de DTD, os limites de recursos e a tradução das falhas para mensagens em português.
//!
//! # Política de DTD (ticket 11, A13)
//!
//! O MusicXML oficial traz `<!DOCTYPE score-partwise PUBLIC "..." "http://...partwise.dtd">` e o
//! corpus da E01 inclui arquivos assim (critério A1, 16/16), então **desligar o DTD recusaria
//! partituras legítimas**. Em vez disso o DTD fica ligado (`allow_dtd: true`) com estas garantias,
//! cada uma coberta por teste:
//!
//! 1. **Nenhuma entidade externa é resolvida.** `roxmltree` só resolve entidade externa quando
//!    recebe um `entity_resolver`; aqui ele é sempre `None` (explícito em [`opcoes`]), então
//!    `SYSTEM`/`PUBLIC`, `file://`, `http://` e entidades de parâmetro externas são descartadas e
//!    qualquer referência a elas vira erro de "entidade não declarada". A dependência
//!    `roxmltree` não tem acesso a rede nem a disco (não depende de nenhum crate de I/O).
//! 2. **Expansão limitada.** O detector de laço do `roxmltree` limita profundidade (10) e
//!    referências aninhadas (255), o que barra o "billion laughs" clássico, mas **não** barra a
//!    explosão quadrática (uma entidade grande referenciada milhares de vezes no nível zero:
//!    100 KB x 4000 = 400 MB). Por isso [`verificar_entidades`] soma, antes de analisar, o
//!    tamanho expandido de todas as referências a entidades declaradas e recusa acima de
//!    [`MAX_EXPANSAO_ENTIDADES_BYTES`].
//! 3. **Estrutura limitada.** Tamanho do texto, número de nós e [`MAX_PROFUNDIDADE_XML`].
//!
//! O `container.xml` do `.mxl` é analisado com DTD desligado (qualquer `DOCTYPE` é recusado).

use crate::partitura::{MAX_NOS_XML, TAMANHO_MAXIMO_BYTES};
use roxmltree::{Document, Error, ParsingOptions};
use std::collections::HashMap;

/// Profundidade máxima de elementos aninhados. MusicXML real raramente passa de 15 níveis; o
/// `roxmltree` não recursa, mas o consumidor percorre árvores e a profundidade não deve ser
/// um recurso que o arquivo controla.
pub const MAX_PROFUNDIDADE_XML: usize = 128;

/// Teto do texto que as entidades declaradas no DTD podem produzir no documento, somando todas
/// as referências (1 MiB). Documento legítimo usa zero ou poucos bytes.
pub const MAX_EXPANSAO_ENTIDADES_BYTES: u64 = 1024 * 1024;

/// Tamanho máximo de um nome (de entidade, de entrada de zip) reproduzido numa mensagem.
pub const MAX_NOME_EM_MENSAGEM: usize = 48;

/// Por que o XML não foi aceito.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FalhaXml {
    /// XML malformado (mensagem em português, com linha e coluna quando há).
    Invalido(String),
    /// Texto maior que [`TAMANHO_MAXIMO_BYTES`].
    GrandeDemais(usize),
    /// Um limite de segurança foi excedido (qual).
    Limite(&'static str),
}

/// Opções do `roxmltree` para MusicXML: DTD interno permitido, entidades externas nunca.
fn opcoes<'a>(nodes_limit: u32) -> ParsingOptions<'a> {
    ParsingOptions {
        allow_dtd: true,
        nodes_limit,
        // Explícito: sem resolvedor, `SYSTEM` e `PUBLIC` nunca abrem arquivo nem rede.
        entity_resolver: None,
    }
}

/// Reduz um nome vindo do arquivo a algo seguro de mostrar: sem controle, no máximo
/// [`MAX_NOME_EM_MENSAGEM`] caracteres.
pub fn nome_para_mensagem(nome: &str) -> String {
    let mut s: String = nome
        .chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .take(MAX_NOME_EM_MENSAGEM)
        .collect();
    if nome.chars().count() > MAX_NOME_EM_MENSAGEM {
        s.push_str("...");
    }
    s
}

/// Traduz o erro do `roxmltree` (em inglês) para a falha tipada, com mensagem em português que
/// não reproduz conteúdo do arquivo além de um nome truncado e da posição.
pub fn traduzir(e: &Error) -> FalhaXml {
    let pos = e.pos();
    let onde = format!("linha {}, coluna {}", pos.row, pos.col);
    match e {
        Error::NodesLimitReached => FalhaXml::Limite("número de nós XML"),
        Error::AttributesLimitReached => FalhaXml::Limite("número de atributos XML"),
        Error::NamespacesLimitReached => FalhaXml::Limite("número de namespaces XML"),
        Error::EntityReferenceLoop(_) => FalhaXml::Limite("expansão de entidades"),
        Error::DtdDetected => {
            FalhaXml::Invalido("declaração DOCTYPE não é permitida neste arquivo".into())
        }
        Error::UnknownEntityReference(nome, _) => FalhaXml::Invalido(format!(
            "entidade '{}' não declarada ou externa (entidades externas nunca são lidas), {onde}",
            nome_para_mensagem(nome)
        )),
        Error::UnexpectedEndOfStream | Error::UnclosedRootNode | Error::NoRootNode => {
            FalhaXml::Invalido("documento vazio ou cortado antes do fim".into())
        }
        Error::EntityResolver(..) | Error::InvalidExternalID(_) => {
            FalhaXml::Invalido(format!("identificador externo do DOCTYPE inválido, {onde}"))
        }
        _ => FalhaXml::Invalido(format!("marcação malformada, {onde}")),
    }
}

/// Mensagem em português de um erro do `roxmltree` (para quem analisa por conta própria).
pub fn mensagem(e: &Error) -> String {
    match traduzir(e) {
        FalhaXml::Invalido(m) => m,
        FalhaXml::GrandeDemais(n) => format!("arquivo grande demais ({n} bytes)"),
        FalhaXml::Limite(q) => format!("limite de segurança excedido: {q}"),
    }
}

/// Analisa um texto MusicXML com todas as defesas: tamanho, expansão de entidades, nós e
/// profundidade. Nunca lê arquivo nem rede.
pub fn analisar(texto: &str) -> Result<Document<'_>, FalhaXml> {
    if texto.len() > TAMANHO_MAXIMO_BYTES {
        return Err(FalhaXml::GrandeDemais(texto.len()));
    }
    verificar_entidades(texto)?;
    let doc = Document::parse_with_options(texto, opcoes(MAX_NOS_XML)).map_err(|e| traduzir(&e))?;
    verificar_profundidade(&doc)?;
    Ok(doc)
}

/// Recusa documento com elementos aninhados além de [`MAX_PROFUNDIDADE_XML`].
fn verificar_profundidade(doc: &Document<'_>) -> Result<(), FalhaXml> {
    // `roxmltree` cria os pais antes dos filhos: a profundidade de cada nó sai do pai em O(n).
    let mut prof: Vec<u32> = Vec::new();
    for n in doc.descendants() {
        let id = n.id().get_usize();
        let d = n.parent().map_or(0, |p| {
            prof.get(p.id().get_usize()).copied().unwrap_or(0) + 1
        });
        if prof.len() <= id {
            prof.resize(id + 1, 0);
        }
        prof[id] = d;
        if d as usize > MAX_PROFUNDIDADE_XML {
            return Err(FalhaXml::Limite("profundidade do XML"));
        }
    }
    Ok(())
}

// --- Orçamento de expansão de entidades ----------------------------------------------------------
//
// Espelha a gramática do prólogo do `roxmltree` (declaração XML, comentários, instruções e um
// `<!DOCTYPE>` com subconjunto interno). Na dúvida (declaração que este leitor não entende) recusa:
// falhar fechado é seguro, falhar aberto não.

struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cursor<'a> {
    fn resto(&self) -> &'a [u8] {
        &self.b[self.i.min(self.b.len())..]
    }
    fn comeca(&self, p: &[u8]) -> bool {
        self.resto().starts_with(p)
    }
    fn espacos(&mut self) {
        while self
            .b
            .get(self.i)
            .is_some_and(|c| matches!(c, b' ' | b'\t' | b'\r' | b'\n'))
        {
            self.i += 1;
        }
    }
    /// Avança até depois de `fim`; `None` se não existir.
    fn pular_ate(&mut self, fim: &[u8]) -> Option<()> {
        let r = self.resto();
        let p = r.windows(fim.len()).position(|w| w == fim)?;
        self.i += p + fim.len();
        Some(())
    }
    /// Lê um literal entre aspas (simples ou duplas) e devolve o conteúdo.
    fn literal(&mut self) -> Option<&'a [u8]> {
        let q = *self.b.get(self.i)?;
        if q != b'"' && q != b'\'' {
            return None;
        }
        self.i += 1;
        let r = self.resto();
        let p = r.iter().position(|c| *c == q)?;
        self.i += p + 1;
        Some(&r[..p])
    }
    fn nome(&mut self) -> Option<&'a [u8]> {
        let r = self.resto();
        let n = r
            .iter()
            .position(|c| matches!(c, b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'[' | b'"' | b'\''))
            .unwrap_or(r.len());
        if n == 0 {
            return None;
        }
        self.i += n;
        Some(&r[..n])
    }
    fn id_externo(&mut self) -> Option<()> {
        let publico = if self.comeca(b"SYSTEM") {
            false
        } else if self.comeca(b"PUBLIC") {
            true
        } else {
            return None;
        };
        self.i += 6;
        self.espacos();
        self.literal()?;
        if publico {
            self.espacos();
            self.literal()?;
        }
        Some(())
    }
}

/// Referência `&nome;` que começa em `b[k]`: o nome e o tamanho total (com `&` e `;`). O nome
/// só tem caracteres de nome (até 80): um `&` solto, por exemplo dentro de `CDATA`, não "engole"
/// o `;` de uma referência real adiante.
fn referencia(b: &[u8], k: usize) -> Option<(&[u8], usize)> {
    if b.get(k) != Some(&b'&') {
        return None;
    }
    let resto = &b[k + 1..];
    let n = resto
        .iter()
        .take(80)
        .take_while(|c| {
            !matches!(
                c,
                b';' | b'&' | b'<' | b'>' | b'"' | b'\'' | b' ' | b'\t' | b'\r' | b'\n'
            )
        })
        .count();
    if n > 0 && resto.get(n) == Some(&b';') {
        Some((&resto[..n], n + 2))
    } else {
        None
    }
}

const NAO_ENTENDI: &str = "declaração DOCTYPE não reconhecida";

fn nao_entendi() -> FalhaXml {
    FalhaXml::Invalido(NAO_ENTENDI.into())
}

type Entidades<'a> = HashMap<&'a [u8], Vec<&'a [u8]>>;

/// Tamanho que a entidade `nome` produz ao ser expandida (máximo entre declarações repetidas).
fn tamanho_da_entidade<'a>(
    nome: &'a [u8],
    ents: &Entidades<'a>,
    memo: &mut HashMap<&'a [u8], u64>,
    prof: u8,
) -> Result<u64, FalhaXml> {
    if let Some(t) = memo.get(nome) {
        return Ok(*t);
    }
    if prof > 10 {
        return Err(FalhaXml::Limite("expansão de entidades"));
    }
    let mut maior = 0_u64;
    for valor in ents.get(nome).map_or(&[][..], Vec::as_slice) {
        let mut total = 0_u64;
        let mut k = 0;
        while k < valor.len() {
            if let Some((ref_nome, len)) = referencia(valor, k) {
                total = total.saturating_add(if ref_nome.first() == Some(&b'#') {
                    4
                } else if ents.contains_key(ref_nome) {
                    tamanho_da_entidade(ref_nome, ents, memo, prof + 1)?
                } else {
                    1
                });
                k += len;
                continue;
            }
            total = total.saturating_add(1);
            k += 1;
        }
        maior = maior.max(total);
    }
    memo.insert(nome, maior);
    Ok(maior)
}

/// Soma o texto que as referências a entidades declaradas produzem no documento e recusa se
/// passar de [`MAX_EXPANSAO_ENTIDADES_BYTES`] (ou se o DTD for profundo/cíclico demais).
pub fn verificar_entidades(texto: &str) -> Result<(), FalhaXml> {
    let mut c = Cursor {
        b: texto.as_bytes(),
        i: 0,
    };
    if c.comeca(&[0xEF, 0xBB, 0xBF]) {
        c.i += 3;
    }
    // Prólogo até o DOCTYPE; sem DOCTYPE não há entidade declarada (o resto é com o analisador).
    loop {
        c.espacos();
        if c.comeca(b"<?") {
            if c.pular_ate(b"?>").is_none() {
                return Ok(());
            }
        } else if c.comeca(b"<!--") {
            if c.pular_ate(b"-->").is_none() {
                return Ok(());
            }
        } else if c.comeca(b"<!DOCTYPE") {
            break;
        } else {
            return Ok(());
        }
    }
    c.i += 9;
    c.espacos();
    c.nome().ok_or_else(nao_entendi)?;
    c.espacos();
    if c.comeca(b"SYSTEM") || c.comeca(b"PUBLIC") {
        c.id_externo().ok_or_else(nao_entendi)?;
    }
    c.espacos();
    let mut ents: Entidades<'_> = HashMap::new();
    match c.b.get(c.i) {
        Some(b'>') => return Ok(()),
        Some(b'[') => c.i += 1,
        _ => return Err(nao_entendi()),
    }
    loop {
        c.espacos();
        if c.comeca(b"<!ENTITY") {
            c.i += 8;
            c.espacos();
            if c.b.get(c.i) == Some(&b'%') {
                c.i += 1;
                c.espacos();
            }
            let nome = c.nome().ok_or_else(nao_entendi)?;
            c.espacos();
            match c.b.get(c.i) {
                Some(b'"' | b'\'') => {
                    let valor = c.literal().ok_or_else(nao_entendi)?;
                    ents.entry(nome).or_default().push(valor);
                }
                // Externa: descartada pelo analisador (sem resolvedor), não expande nada.
                Some(b'S' | b'P') => {
                    c.id_externo().ok_or_else(nao_entendi)?;
                    c.espacos();
                    if c.comeca(b"NDATA") {
                        c.i += 5;
                        c.espacos();
                        c.nome().ok_or_else(nao_entendi)?;
                    }
                }
                _ => return Err(nao_entendi()),
            }
            c.espacos();
            if c.b.get(c.i) != Some(&b'>') {
                return Err(nao_entendi());
            }
            c.i += 1;
        } else if c.comeca(b"<!--") {
            c.pular_ate(b"-->").ok_or_else(nao_entendi)?;
        } else if c.comeca(b"<?") {
            c.pular_ate(b"?>").ok_or_else(nao_entendi)?;
        } else if c.comeca(b"<!ELEMENT") || c.comeca(b"<!ATTLIST") || c.comeca(b"<!NOTATION") {
            c.pular_ate(b">").ok_or_else(nao_entendi)?;
        } else if c.b.get(c.i) == Some(&b']') {
            c.i += 1;
            c.espacos();
            if c.b.get(c.i) != Some(&b'>') {
                return Err(nao_entendi());
            }
            c.i += 1;
            break;
        } else {
            return Err(nao_entendi());
        }
    }
    if ents.is_empty() {
        return Ok(());
    }
    // Corpo: cada `&nome;` de entidade declarada custa o tamanho que ela expande.
    let corpo = c.resto();
    let mut memo: HashMap<&[u8], u64> = HashMap::new();
    let mut gasto = 0_u64;
    let mut k = 0;
    while k < corpo.len() {
        if let Some((nome, len)) = referencia(corpo, k) {
            if ents.contains_key(nome) {
                gasto = gasto.saturating_add(tamanho_da_entidade(nome, &ents, &mut memo, 0)?);
                if gasto > MAX_EXPANSAO_ENTIDADES_BYTES {
                    return Err(FalhaXml::Limite("expansão de entidades"));
                }
            }
            k += len;
        } else {
            k += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    const BASE: &str = "<score-partwise><work><work-title>T</work-title></work></score-partwise>";

    fn com_doctype(dtd: &str, corpo: &str) -> String {
        format!("<?xml version=\"1.0\"?>\n<!DOCTYPE score-partwise {dtd}>\n{corpo}")
    }

    #[test]
    fn doctype_publico_do_musicxml_continua_aceito() {
        let x = com_doctype(
            "PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \"http://www.musicxml.org/dtds/partwise.dtd\"",
            BASE,
        );
        analisar(&x).expect("abre");
        analisar(&com_doctype("SYSTEM \"file:///etc/passwd\"", BASE)).expect("abre");
        analisar(&com_doctype("[<!ELEMENT x ANY>]", BASE)).expect("abre");
    }

    #[test]
    fn entidade_interna_pequena_continua_aceita() {
        let x = com_doctype(
            "[<!ENTITY autor \"Fulano &amp; Cia\">]",
            "<score-partwise><work><work-title>&autor;</work-title></work></score-partwise>",
        );
        let d = analisar(&x).expect("abre");
        assert!(d.descendants().any(|n| n.text() == Some("Fulano & Cia")));
    }

    #[test]
    fn entidade_externa_nunca_e_resolvida_nem_lida() {
        for decl in [
            "[<!ENTITY x SYSTEM \"file:///etc/passwd\">]",
            "[<!ENTITY x PUBLIC \"-//X//EN\" \"http://127.0.0.1:9/x\">]",
            "[<!ENTITY x SYSTEM \"https://exemplo.invalido/x.xml\">]",
        ] {
            let x = com_doctype(
                decl,
                "<score-partwise><work><work-title>&x;</work-title></work></score-partwise>",
            );
            match analisar(&x) {
                Err(FalhaXml::Invalido(m)) => {
                    assert!(m.contains("externa"), "{m}");
                    assert!(!m.contains("passwd") && !m.contains("http"), "{m}");
                }
                outro => panic!("esperava recusa: {:?}", outro.map(|_| ())),
            }
        }
    }

    #[test]
    fn entidade_externa_nao_usada_e_inofensiva() {
        let x = com_doctype("[<!ENTITY x SYSTEM \"file:///etc/passwd\">]", BASE);
        analisar(&x).expect("declarar sem usar não lê nada");
    }

    #[test]
    fn billion_laughs_e_recusado() {
        let mut dtd = String::from("[<!ENTITY l0 \"lol\">");
        for n in 1..=9 {
            dtd.push_str(&format!(
                "<!ENTITY l{n} \"{}\">",
                format!("&l{};", n - 1).repeat(10)
            ));
        }
        dtd.push(']');
        let x = com_doctype(
            &dtd,
            "<score-partwise><work><work-title>&l9;</work-title></work></score-partwise>",
        );
        assert_eq!(
            analisar(&x).err(),
            Some(FalhaXml::Limite("expansão de entidades"))
        );
    }

    #[test]
    fn explosao_quadratica_e_recusada_antes_de_expandir() {
        let grande = "a".repeat(100_000);
        let x = com_doctype(
            &format!("[<!ENTITY g \"{grande}\">]"),
            &format!(
                "<score-partwise><work><work-title>{}</work-title></work></score-partwise>",
                "&g;".repeat(4000)
            ),
        );
        assert_eq!(
            analisar(&x).err(),
            Some(FalhaXml::Limite("expansão de entidades"))
        );
        // No atributo também.
        let y = com_doctype(
            &format!("[<!ENTITY g \"{grande}\">]"),
            &format!("<score-partwise a=\"{}\"/>", "&g;".repeat(4000)),
        );
        assert_eq!(
            analisar(&y).err(),
            Some(FalhaXml::Limite("expansão de entidades"))
        );
    }

    #[test]
    fn orcamento_de_expansao_tem_fronteira_exata() {
        let valor = "a".repeat(1000);
        let dtd = format!("[<!ENTITY g \"{valor}\">]");
        let corpo = |n: usize| {
            format!(
                "<score-partwise><work><work-title>{}</work-title></work></score-partwise>",
                "&g;".repeat(n)
            )
        };
        let limite = (MAX_EXPANSAO_ENTIDADES_BYTES / 1000) as usize;
        analisar(&com_doctype(&dtd, &corpo(limite))).expect("no orçamento");
        assert!(analisar(&com_doctype(&dtd, &corpo(limite + 2))).is_err());
    }

    #[test]
    fn declaracao_que_o_leitor_nao_entende_falha_fechado() {
        let x = com_doctype("[<!ENTITY x>]", BASE);
        assert!(matches!(analisar(&x), Err(FalhaXml::Invalido(_))));
        let y = com_doctype("[%pe;]", BASE);
        assert!(matches!(analisar(&y), Err(FalhaXml::Invalido(_))));
    }

    #[test]
    fn e_comercial_solto_nao_esconde_referencias_do_orcamento() {
        // `&` em CDATA antes de cada referência: não pode "engolir" o `;` da referência.
        let grande = "a".repeat(100_000);
        let corpo = "<![CDATA[&]]>&g;".repeat(4000);
        let x = com_doctype(
            &format!("[<!ENTITY g \"{grande}\">]"),
            &format!(
                "<score-partwise><work><work-title>{corpo}</work-title></work></score-partwise>"
            ),
        );
        assert_eq!(
            analisar(&x).err(),
            Some(FalhaXml::Limite("expansão de entidades"))
        );
    }

    #[test]
    fn doctype_dentro_de_comentario_nao_engana_o_orcamento() {
        let grande = "a".repeat(100_000);
        let x = format!(
            "<!-- <!DOCTYPE s [<!ENTITY g \"\">]> -->\n{}",
            com_doctype(
                &format!("[<!ENTITY g \"{grande}\">]"),
                &format!(
                    "<score-partwise><work><work-title>{}</work-title></work></score-partwise>",
                    "&g;".repeat(4000)
                )
            )
        );
        assert!(analisar(&x).is_err());
    }

    #[test]
    fn profundidade_alem_do_limite_e_recusada_e_no_limite_passa() {
        let aninhado = |n: usize| format!("{}{}", "<a>".repeat(n), "</a>".repeat(n));
        analisar(&aninhado(MAX_PROFUNDIDADE_XML)).expect("no limite");
        assert_eq!(
            analisar(&aninhado(MAX_PROFUNDIDADE_XML + 1)).err(),
            Some(FalhaXml::Limite("profundidade do XML"))
        );
        // Muito fundo: sem pânico nem estouro de pilha.
        assert!(analisar(&aninhado(500_000)).is_err());
    }

    #[test]
    fn mensagens_sao_em_portugues_e_nao_reproduzem_o_arquivo() {
        for (xml, trecho) in [
            ("", "vazio"),
            ("<a>", "cortado"),
            ("<a></b>", "malformada"),
            ("<a x=1/>", "malformada"),
        ] {
            match analisar(xml) {
                Err(FalhaXml::Invalido(m)) => assert!(m.contains(trecho), "{xml:?}: {m}"),
                outro => panic!("{xml:?}: {:?}", outro.map(|_| ())),
            }
        }
        let comprido = "n".repeat(500);
        let x = format!("<a>&{comprido};</a>");
        match analisar(&x) {
            Err(FalhaXml::Invalido(m)) => assert!(m.len() < 250, "{m}"),
            outro => panic!("{:?}", outro.map(|_| ())),
        }
    }

    #[test]
    fn nome_para_mensagem_corta_e_limpa() {
        assert_eq!(nome_para_mensagem("a\u{0}b"), "a?b");
        let longo = nome_para_mensagem(&"x".repeat(1000));
        assert_eq!(longo.chars().count(), MAX_NOME_EM_MENSAGEM + 3);
    }
}
