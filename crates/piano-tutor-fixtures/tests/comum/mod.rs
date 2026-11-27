#![allow(dead_code)] // cada teste usa só parte do módulo comum
//! Variações adversas de um MusicXML (BOM, fim de linha, aspas, comentários, entidades, marcas com
//! espaço), usadas para procurar onde o modelo híbrido quebra a identidade byte a byte.

/// (rótulo, texto) de cada variação de `xml` que continua sendo o mesmo MusicXML.
pub fn variantes(xml: &str) -> Vec<(&'static str, String)> {
    let mut v: Vec<(&'static str, String)> = Vec::new();
    v.push(("bom-utf8", format!("\u{feff}{xml}")));
    v.push(("crlf", xml.replace('\n', "\r\n")));
    v.push(("cr-solto", xml.replace('\n', "\r")));
    v.push(("sem-quebra-final", xml.trim_end().to_owned()));
    v.push(("espacos-no-fim", format!("{xml}   \n\n  ")));
    v.push((
        "aspas-simples",
        xml.replace("version=\"4.0\"", "version='4.0'")
            .replace("number=\"1\"", "number='1'"),
    ));
    v.push((
        "marcas-com-espaco",
        xml.replace("<note>", "<note >")
            .replace("<note ", "<note \n\t ")
            .replace("</note>", "</note\n>")
            .replace("<measure ", "<measure  \n   ")
            .replace("</measure>", "</measure >"),
    ));
    v.push((
        "atributo-com-maior-que",
        xml.replace("<measure ", "<measure x-q='a>b\"c' "),
    ));
    v.push((
        "comentarios-e-pi-dentro-do-compasso",
        xml.replace("<note>", "<!-- n </note> &amp; --><?pi dado?><note>")
            .replace("</measure>", "<!-- fim --></measure>"),
    ));
    v.push((
        "cdata-e-entidades-no-texto",
        xml.replace(
            "<part-list>",
            "<!-- x --><part-list>",
        )
        .replacen(
            "</identification>",
            "<miscellaneous><miscellaneous-field name=\"t\"><![CDATA[a<b & c]]> &amp; &#233; &lt;</miscellaneous-field></miscellaneous></identification>",
            1,
        ),
    ));
    let subconjunto = " [\n<!ENTITY autor \"Fulano &amp; Cia\">\n<!ELEMENT x ANY>\n]>";
    let com_dtd = if xml.contains("partwise.dtd\">") {
        xml.replacen(
            "partwise.dtd\">",
            &format!("partwise.dtd\"{subconjunto}"),
            1,
        )
    } else {
        xml.replacen(
            "<score-partwise",
            &format!("<!DOCTYPE score-partwise{subconjunto}\n<score-partwise"),
            1,
        )
    };
    v.push((
        "doctype-com-subconjunto-interno",
        com_dtd.replacen("<work-title>", "<work-title>&autor; - ", 1),
    ));
    v.push((
        "tabs-e-indentacao-mista",
        xml.replace("\n    ", "\n\t\t").replace("\n  ", "\n\t"),
    ));
    v
}

/// Variações que o modelo recusa de forma explícita, em vez de aceitar errado.
pub fn hostis(xml: &str) -> Vec<(&'static str, String)> {
    let subconjunto = " [<!ENTITY nota \"<note><rest/><duration>1</duration></note>\">]>";
    let com_dtd = if xml.contains("partwise.dtd\">") {
        xml.replacen(
            "partwise.dtd\">",
            &format!("partwise.dtd\"{subconjunto}"),
            1,
        )
    } else {
        xml.replacen(
            "<score-partwise",
            &format!("<!DOCTYPE score-partwise{subconjunto}\n<score-partwise"),
            1,
        )
    };
    vec![(
        "entidade-que-expande-elemento",
        com_dtd.replacen("</measure>", "&nota;</measure>", 1),
    )]
}
