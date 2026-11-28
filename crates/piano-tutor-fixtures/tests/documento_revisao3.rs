//! Terceira revisão independente da E01 (`docs/evolucao-musical/e01/REVISAO-CODIGO.md`), achados
//! T1, T2 e T3 (a API será exposta por MCP).
//!
//! - T1: os pedaços de `<accidental>` "do arquivo" saem dos **elementos que o parser leu**; texto
//!   de comentário, CDATA e instrução de processamento não conta.
//! - T2: referência de caractere e caractere literal seguem a produção `Char` do XML 1.0.
//! - Propriedade de segurança: todo comando **aceito** produz texto que reabre por `abrir_texto`
//!   e passa `conferir_exportacao`.
//! - T3: o custo de validar uma grafia não cresce com o número de `<accidental` hostis no texto.

#![allow(clippy::unwrap_used)] // teste: o `unwrap` é a asserção

use piano_tutor_core::documento::{
    Altura, ComandoDeEdicao, DocumentoAberto, PosCompasso, figura_da_tecla,
};
use piano_tutor_core::partitura::Fracao;
use std::time::{Duration, Instant};

fn peca_com(prefixo: &str, raiz_attrs: &str, notas: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{prefixo}<score-partwise version=\"4.0\"{raiz_attrs}>\
         <part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list>\
         <part id=\"P1\"><measure number=\"1\"><attributes><divisions>4</divisions>\
         <time><beats>4</beats><beat-type>4</beat-type></time></attributes>{notas}</measure>\
         </part></score-partwise>\n"
    )
}

fn nota(step: char, alter: Option<i8>, acc: Option<&str>) -> String {
    let a = alter
        .map(|a| format!("<alter>{a}</alter>"))
        .unwrap_or_default();
    let ac = acc.unwrap_or("");
    format!(
        "<note><pitch><step>{step}</step>{a}<octave>4</octave></pitch><duration>4</duration>\
         <voice>1</voice><type>quarter</type>{ac}</note>"
    )
}

fn quatro(prefixo: &str, raiz_attrs: &str) -> String {
    peca_com(
        prefixo,
        raiz_attrs,
        &format!(
            "{}{}{}{}",
            nota('C', None, None),
            nota('F', None, None),
            nota('E', None, None),
            nota('G', None, None)
        ),
    )
}

/// `DefinirAltura` Fá♯4 na segunda nota; devolve `Ok(alterou)` ou a mensagem de recusa.
fn definir(d: &mut DocumentoAberto, grafia: &str) -> Result<bool, String> {
    let id = d.ids().id(0, 0, 1).unwrap();
    d.aplicar(ComandoDeEdicao::DefinirAltura {
        nota: id,
        altura: Altura {
            passo: 'F',
            alter: 1,
            oitava: 4,
        },
        acidente: Some(grafia.to_owned()),
    })
    .map(|r| r.alterou)
    .map_err(|e| e.to_string())
}

fn reabre_e_confere(d: &DocumentoAberto) -> Result<(), String> {
    let t = d.exportar().map_err(|e| e.to_string())?;
    DocumentoAberto::abrir_texto("x", &t).map_err(|e| format!("não reabre: {e}"))?;
    d.conferir_exportacao(&t)
        .map_err(|e| format!("não confere: {e}"))
}

/// `true` se a grafia foi recusada (nada muda); se aceita, o texto tem de reabrir e conferir.
fn recusado(prefixo: &str, raiz_attrs: &str, grafia: &str) -> bool {
    let mut d = DocumentoAberto::abrir_texto("x", &quatro(prefixo, raiz_attrs)).unwrap();
    match definir(&mut d, grafia) {
        Err(_) => {
            assert!(!d.editado(), "recusa não pode sujar: {grafia:?}");
            true
        }
        Ok(_) => {
            reabre_e_confere(&d).unwrap_or_else(|e| panic!("aceito mas {e}: {grafia:?}"));
            false
        }
    }
}

// ---------------------------------------------------------------- T1

#[test]
fn t1_pedaco_so_em_comentario_nao_e_do_arquivo() {
    let casos = [
        ("<!-- <accidental>sharp]]>y</accidental> -->\n", "sharp]]>y"),
        (
            "<!-- <accidental foo:bar=\"1\">sharp</accidental> -->\n",
            "sharp foo:bar=\"1\"",
        ),
        (
            "<!-- <accidental onclick=\"x\">sharp</accidental> -->\n",
            "sharp onclick=\"x\"",
        ),
        ("<!-- <accidental>zzz</accidental> -->\n", "zzz"),
        (
            "<!-- <accidental>sharp&bogus;</accidental> -->\n",
            "sharp&bogus;",
        ),
    ];
    for (prefixo, g) in casos {
        assert!(recusado(prefixo, "", g), "comentário não vale: {g:?}");
    }
}

#[test]
fn t1_pedaco_em_cdata_ou_instrucao_nao_e_do_arquivo() {
    let cdata = "<![CDATA[<accidental onclick=\"x\">sharp</accidental>]]>";
    let texto = quatro("", "").replace(
        "<part-name>P</part-name>",
        &format!("<part-name>{cdata}</part-name>"),
    );
    let mut d = DocumentoAberto::abrir_texto("x", &texto).unwrap();
    assert!(definir(&mut d, "sharp onclick=\"x\"").is_err(), "CDATA");
    let pi = "<?alvo <accidental onclick=\"x\">sharp</accidental> ?>\n";
    assert!(recusado(pi, "", "sharp onclick=\"x\""), "instrução");
}

#[test]
fn t1_controle_elemento_real_fora_do_esquema_segue_aceito() {
    // O mesmo atributo, num `<accidental>` que o parser leu: preservado (ADR-0017).
    let texto = peca_com(
        "",
        "",
        &format!(
            "{}{}",
            nota('C', None, None),
            nota(
                'F',
                Some(1),
                Some("<accidental onclick=\"x\">sharp</accidental>")
            )
        ),
    );
    let mut d = DocumentoAberto::abrir_texto("x", &texto).unwrap();
    let id = d.ids().id(0, 0, 0).unwrap();
    let r = d.aplicar(ComandoDeEdicao::DefinirAltura {
        nota: id,
        altura: Altura {
            passo: 'C',
            alter: 1,
            oitava: 4,
        },
        acidente: Some("sharp onclick=\"x\"".into()),
    });
    assert!(r.unwrap().alterou);
    reabre_e_confere(&d).unwrap();
}

#[test]
fn t1_prefixo_de_namespace_so_vale_se_declarado_no_escopo() {
    // Declarado na raiz: o pedaço do arquivo vale, o inverso é aceito e reabre.
    let nota_ns = nota(
        'F',
        Some(1),
        Some("<accidental foo:bar=\"1\">sharp</accidental>"),
    );
    let declarado = peca_com("", " xmlns:foo=\"urn:x\"", &nota_ns);
    let mut d = DocumentoAberto::abrir_texto("x", &declarado).unwrap();
    let id = d.ids().id(0, 0, 0).unwrap();
    let r = d
        .aplicar(ComandoDeEdicao::DefinirAltura {
            nota: id,
            altura: Altura {
                passo: 'F',
                alter: 0,
                oitava: 4,
            },
            acidente: Some("natural".into()),
        })
        .unwrap();
    assert!(r.alterou);
    reabre_e_confere(&d).unwrap();
    d.desfazer().unwrap();
    reabre_e_confere(&d).unwrap();
    // Pedido com prefixo que ninguém declarou: recusado.
    assert!(recusado("", "", "sharp foo:bar=\"1\""));
}

// ---------------------------------------------------------------- T2

#[test]
fn t2_referencia_de_caractere_invalida_e_recusada() {
    for v in [
        "&#0;",
        "&#1;",
        "&#x1;",
        "&#x8;",
        "&#xB;",
        "&#xC;",
        "&#x1F;",
        "&#xD800;",
        "&#xDFFF;",
        "&#xFFFE;",
        "&#xFFFF;",
        "&#x110000;",
        "&#1114112;",
        "&#99999999999;",
        "&#x;",
        "&#;",
        "&#xZZ;",
        "\u{FFFE}",
        "\u{FFFF}",
        "\u{1}",
    ] {
        let g = format!("sharp cautionary=\"{v}\"");
        assert!(recusado("", "", &g), "{v:?}");
    }
}

#[test]
fn t2_referencia_de_caractere_valida_segue_aceita() {
    for v in [
        "&#9;",
        "&#10;",
        "&#13;",
        "&#32;",
        "&#x41;",
        "&#xD7FF;",
        "&#xE000;",
        "&#xFFFD;",
        "&#x10000;",
        "&#x10FFFF;",
        "&amp;",
        "&lt;",
        "&#128;",
        "é",
        "\u{10000}",
    ] {
        let g = format!("sharp cautionary=\"{v}\"");
        assert!(!recusado("", "", &g), "{v:?} é Char válido");
    }
}

// ---------------------------------------------------------------- propriedade

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
        v[(self.next() as usize) % v.len()]
    }
}

const TOKENS: &[&str] = &[
    "sharp",
    "flat",
    "natural",
    "double-sharp",
    " ",
    "\n",
    "\t",
    "\"",
    "'",
    ">",
    "<",
    "/",
    "=",
    "&amp;",
    "&lt;",
    "&#",
    "&#0;",
    "&#1;",
    "&#x8;",
    "&#xD800;",
    "&#xFFFE;",
    "&#x110000;",
    "&#99999999999;",
    "&#x41;",
    "]]>",
    "-->",
    "<!--",
    "\u{1F}",
    "cautionary",
    "parentheses",
    "smufl",
    "id",
    "=\"yes\"",
    "='no'",
    "xmlns",
    "xmlns:x",
    "x:y",
    "foo:bar",
    "onclick",
    "<injetado/>",
    "</accidental>",
    "\u{FFFE}",
    "\u{FFFF}",
    "\0",
];

fn gerar(r: &mut Lcg) -> String {
    let mut s = String::new();
    if r.next().is_multiple_of(2) {
        s.push_str(r.pick(&["sharp", "flat", "natural", "double-sharp"]));
        s.push(' ');
    }
    for _ in 0..(1 + r.next() % 7) {
        s.push_str(r.pick(TOKENS));
    }
    s
}

#[test]
fn todo_comando_aceito_reabre_e_confere() {
    let hostil = "<!-- <accidental x=\"1\">sharp</accidental> <accidental>flat]]></accidental> \
                  <accidental a:b=\"1\">natural</accidental> <accidental cautionary=\"&#0;\">sharp</accidental> -->\n\
                  <?pi <accidental onclick=\"x\">sharp</accidental> ?>\n";
    for (pi, prefixo) in ["", hostil].into_iter().enumerate() {
        let texto = quatro(prefixo, "");
        let mut r = Lcg(0x1234_5678 + pi as u64);
        let mut aceitos = 0u32;
        for it in 0..4_000u32 {
            let g = gerar(&mut r);
            let alter = match g.split_whitespace().next() {
                Some("sharp") => 1,
                Some("flat") => -1,
                Some("double-sharp") => 2,
                _ => 0,
            };
            let mut d = DocumentoAberto::abrir_texto("x", &texto).unwrap();
            let id0 = d.ids().id(0, 0, 0).unwrap();
            let id1 = d.ids().id(0, 0, 1).unwrap();
            let altura = |passo| Altura {
                passo,
                alter,
                oitava: 4,
            };
            let cmd = match it % 4 {
                0 => ComandoDeEdicao::DefinirAltura {
                    nota: id1,
                    altura: altura('F'),
                    acidente: Some(g.clone()),
                },
                1 => ComandoDeEdicao::AdicionarAltura {
                    nota: id0,
                    altura: altura('G'),
                    acidente: Some(g.clone()),
                },
                2 => ComandoDeEdicao::RestaurarAltura {
                    nota: id0,
                    altura: altura('G'),
                    acidente: Some(g.clone()),
                },
                _ => ComandoDeEdicao::InserirNota {
                    compasso: PosCompasso::nova("P1", 0),
                    voz: "1".into(),
                    pauta: "1".into(),
                    posicao: Fracao::nova(0, 1),
                    altura: altura('G'),
                    acidente: Some(g.clone()),
                    figura: figura_da_tecla(5).unwrap(),
                    pontos: 0,
                },
            };
            if d.aplicar(cmd).is_err() {
                continue;
            }
            aceitos += 1;
            reabre_e_confere(&d).unwrap_or_else(|e| panic!("aceito mas {e}: {g:?}"));
        }
        assert!(aceitos > 10, "o gerador precisa aceitar alguns ({aceitos})");
    }
}

// ---------------------------------------------------------------- T3

#[test]
fn t3_comentario_hostil_nao_torna_o_comando_quadratico() {
    // Aspas desbalanceadas repetidas: antes, 5,9 s com 16 000 repetições.
    let n = 16_000;
    let prefixo = format!("<!-- {} -->\n", "<accidental \"".repeat(n));
    let mut d = DocumentoAberto::abrir_texto("x", &quatro(&prefixo, "")).unwrap();
    let t = Instant::now();
    let r = definir(&mut d, "sharp onclick=\"x\"");
    let gasto = t.elapsed();
    assert!(r.is_err());
    assert!(gasto < Duration::from_millis(500), "levou {gasto:?}");
}

#[test]
fn t3_muitos_acidentes_distintos_no_arquivo_nao_custam_por_comando() {
    let n = 20_000;
    let mut notas = String::new();
    for i in 0..n {
        notas.push_str(&format!(
            "<note><pitch><step>C</step><octave>4</octave></pitch><duration>0</duration>\
             <voice>1</voice><type>quarter</type>\
             <accidental default-x=\"{i}\">natural</accidental></note>"
        ));
    }
    let texto = peca_com("", "", &(nota('F', None, None) + &notas));
    let mut d = DocumentoAberto::abrir_texto("x", &texto).unwrap();
    let id = d.ids().id(0, 0, 0).unwrap();
    let t = Instant::now();
    for _ in 0..20 {
        let r = d.aplicar(ComandoDeEdicao::DefinirAltura {
            nota: id,
            altura: Altura {
                passo: 'F',
                alter: 1,
                oitava: 4,
            },
            acidente: Some("sharp foo=\"1\"".into()),
        });
        assert!(r.is_err());
    }
    let gasto = t.elapsed();
    // Antes: 1,1 s por comando (23 s nos 20). O resto é custo linear do comando em si.
    assert!(gasto < Duration::from_secs(5), "20 comandos: {gasto:?}");
}
