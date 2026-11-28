//! SONDAS TEMPORARIAS da terceira revisao independente da E01 (N5-N8, validacao de grafia).
use piano_tutor_core::documento::{
    Altura, ComandoDeEdicao, DocumentoAberto, PosCompasso, SentidoDaAltura, UnidadeDeAltura,
    figura_da_tecla,
};
use piano_tutor_core::partitura::Fracao;
use std::time::Instant;

fn peca_com(prefixo: &str, notas: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{prefixo}<score-partwise version=\"4.0\">\
         <part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list>\
         <part id=\"P1\"><measure number=\"1\"><attributes><divisions>4</divisions>\
         <time><beats>4</beats><beat-type>4</beat-type></time></attributes>{notas}</measure>\
         </part></score-partwise>\n"
    )
}

fn nota(step: char, alter: Option<i8>, acc: Option<&str>) -> String {
    let a = alter.map(|a| format!("<alter>{a}</alter>")).unwrap_or_default();
    let ac = acc.unwrap_or("");
    format!(
        "<note><pitch><step>{step}</step>{a}<octave>4</octave></pitch><duration>4</duration>\
         <voice>1</voice><type>quarter</type>{ac}</note>"
    )
}

fn quatro(prefixo: &str) -> String {
    peca_com(
        prefixo,
        &format!(
            "{}{}{}{}",
            nota('C', None, None),
            nota('F', None, None),
            nota('E', None, None),
            nota('G', None, None)
        ),
    )
}

fn definir(
    d: &mut DocumentoAberto,
    ord: usize,
    alter: i8,
    acid: Option<&str>,
) -> Result<bool, String> {
    let id = d.ids().id(0, 0, ord).unwrap();
    d.aplicar(ComandoDeEdicao::DefinirAltura {
        nota: id,
        altura: Altura { passo: 'F', alter, oitava: 4 },
        acidente: acid.map(str::to_owned),
    })
    .map(|r| r.alterou)
    .map_err(|e| e.to_string())
}

fn relatorio(rotulo: &str, d: &DocumentoAberto, r: Result<bool, String>) {
    match r {
        Err(e) => println!("SONDA3 [{rotulo}] recusado: {e}"),
        Ok(alterou) => {
            let t = d.exportar().unwrap();
            let reabre = DocumentoAberto::abrir_texto("x", &t).map(|_| ()).map_err(|e| e.to_string());
            let conf = d.conferir_exportacao(&t).map_err(|e| e.to_string());
            let i = t.find("<accidental").unwrap_or(0);
            let fim = t[i..].find("</accidental>").map_or(t.len(), |f| i + f + 13);
            println!(
                "SONDA3 [{rotulo}] ACEITO alterou={alterou}; trecho={:?}; reabre={reabre:?}; conferir={conf:?}",
                &t[i..fim]
            );
        }
    }
}

#[test]
fn zz3_a_pedaco_de_comentario() {
    let casos: Vec<(&str, String, &str)> = vec![
        ("attr de comentario", "<!-- <accidental onclick=\"x\">sharp</accidental> -->\n".into(), "sharp onclick=\"x\""),
        ("attr ns indeclarado", "<!-- <accidental foo:bar=\"1\">sharp</accidental> -->\n".into(), "sharp foo:bar=\"1\""),
        ("conteudo cdata ]]>", "<!-- <accidental>sharp]]>y</accidental> -->\n".into(), "sharp]]>y"),
        ("conteudo lixo comentario", "<!-- <accidental>zzz</accidental> -->\n".into(), "zzz"),
        ("conteudo com entidade invalida", "<!-- <accidental>sharp&bogus;</accidental> -->\n".into(), "sharp&bogus;"),
        ("sem comentario (controle)", String::new(), "sharp onclick=\"x\""),
    ];
    for (nome, prefixo, g) in casos {
        let mut d = DocumentoAberto::abrir_texto("x", &quatro(&prefixo)).unwrap();
        let r = definir(&mut d, 1, 1, Some(g));
        relatorio(nome, &d, r);
    }
}

#[test]
fn zz3_b_referencias_de_caractere() {
    for v in [
        "&#0;", "&#1;", "&#x1;", "&#xD800;", "&#x110000;", "&#99999999999;", "&#xFFFE;", "&#xFFFF;",
        "&#x8;", "\u{FFFE}", "\u{FFFF}", "\u{7F}", "\u{85}",
    ] {
        let g = format!("sharp cautionary=\"{v}\"");
        let mut d = DocumentoAberto::abrir_texto("x", &quatro("")).unwrap();
        let r = definir(&mut d, 1, 1, Some(&g));
        relatorio(&format!("charref {v:?}"), &d, r);
    }
}

#[test]
fn zz3_c_definir_altura_inconsistente() {
    let mut d = DocumentoAberto::abrir_texto("x", &quatro("")).unwrap();
    let r = definir(&mut d, 1, 0, Some("sharp"));
    relatorio("F natural com acidente sharp", &d, r);
    let mut d = DocumentoAberto::abrir_texto("x", &quatro("")).unwrap();
    let r = definir(&mut d, 1, 1, Some("quarter-sharp"));
    relatorio("F# com acidente quarter-sharp", &d, r);
    let mut d = DocumentoAberto::abrir_texto("x", &quatro("")).unwrap();
    let r = definir(&mut d, 1, 1, Some("other"));
    relatorio("F# com acidente other", &d, r);
}

#[test]
fn zz3_d_restaurar_altura_publico() {
    let mut d = DocumentoAberto::abrir_texto("x", &quatro("")).unwrap();
    let id = d.ids().id(0, 0, 0).unwrap();
    let antes = d.exportar().unwrap();
    for _ in 0..3 {
        let r = d.aplicar(ComandoDeEdicao::RestaurarAltura {
            nota: id,
            altura: Altura { passo: 'C', alter: 0, oitava: 4 },
            acidente: None,
        });
        println!(
            "SONDA3 RestaurarAltura unissono por API publica: {:?}",
            r.as_ref().map(|r| r.alterou).map_err(|e| e.to_string())
        );
    }
    let t = d.exportar().unwrap();
    println!(
        "SONDA3 mudou texto={} reabre={:?}",
        t != antes,
        DocumentoAberto::abrir_texto("x", &t).map(|_| ()).map_err(|e| e.to_string())
    );
    let r = d.aplicar(ComandoDeEdicao::AdicionarAltura {
        nota: id,
        altura: Altura { passo: 'C', alter: 0, oitava: 4 },
        acidente: None,
    });
    println!("SONDA3 AdicionarAltura repetida (controle): {:?}", r.map(|r| r.alterou).map_err(|e| e.to_string()));
    while d.desfazer().is_some() {}
    println!("SONDA3 apos desfazer tudo, igual ao original={}", d.exportar().unwrap() == antes);
}

#[test]
fn zz3_e_pontos_e_rotulos() {
    for pontos in [0u8, 3, 4, 8, 63, 64, 200, 255] {
        let mut d = DocumentoAberto::abrir_texto("x", &quatro("")).unwrap();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            d.aplicar(ComandoDeEdicao::InserirPausa {
                compasso: PosCompasso::nova("P1", 0),
                voz: "2".into(),
                pauta: "1".into(),
                posicao: Fracao::nova(0, 1),
                figura: figura_da_tecla(5).unwrap(),
                pontos,
            })
            .map(|r| r.alterou)
            .map_err(|e| e.to_string())
        }));
        println!("SONDA3 InserirPausa pontos={pontos}: {r:?}");
    }
    let longa33 = "9".repeat(33);
    let longa32 = "9".repeat(32);
    for (voz, pauta) in [
        ("1", "1"),
        ("a.b-c_d", "x"),
        ("1", "-"),
        ("\u{660}", "1"),
        ("1\n", "1"),
        (longa33.as_str(), "1"),
        (longa32.as_str(), "1"),
        ("1 ", "1"),
        ("1", "1\u{0}"),
    ] {
        let mut d = DocumentoAberto::abrir_texto("x", &quatro("")).unwrap();
        let r = d.aplicar(ComandoDeEdicao::InserirPausa {
            compasso: PosCompasso::nova("P1", 0),
            voz: voz.into(),
            pauta: pauta.into(),
            posicao: Fracao::nova(0, 1),
            figura: figura_da_tecla(5).unwrap(),
            pontos: 0,
        });
        println!(
            "SONDA3 InserirPausa voz={voz:?} pauta={pauta:?}: {:?}",
            r.map(|r| r.alterou).map_err(|e| e.to_string())
        );
    }
}

fn exotica(attrs: &str, conteudo: &str) -> String {
    format!("<accidental{attrs}>{conteudo}</accidental>")
}

fn trecho(t: &str) -> String {
    let i = t.find("<accidental").unwrap_or(0);
    let fim = t[i..].find("</note>").map_or(t.len(), |f| i + f);
    t[i..fim].replace('\n', "\\n")
}

#[test]
fn zz3_f_preservacao_exotica() {
    let casos: Vec<(&str, String)> = vec![
        ("controle", exotica("", "sharp")),
        ("print-object", exotica(" print-object=\"no\"", "sharp")),
        ("xml:lang", exotica(" xml:lang=\"pt\"", "sharp")),
        ("unicode no nome", exotica(" a\u{e7}\u{e3}o=\"1\"", "sharp")),
        ("underscore nome", exotica(" x_y=\"1\"", "sharp")),
        ("valor vazio", exotica(" smufl=\"\"", "sharp")),
        ("vazio", exotica("", "")),
        ("so espaco", exotica("", " ")),
        ("comentario dentro", exotica("", "sharp<!--c-->")),
        ("cdata", exotica("", "<![CDATA[sharp]]>")),
        ("entidade no conteudo", exotica("", "&#115;harp")),
        ("tab antes do >", exotica(" cautionary=\"yes\"\t", "sharp")),
        ("crlf", exotica("\r\n cautionary=\"yes\"", "sharp")),
        ("valor com &lt;", exotica(" font-family=\"a&lt;b\"", "sharp")),
        ("auto-fechado", "<accidental/>".to_owned()),
        ("conteudo desconhecido", exotica("", "x-futuro")),
        ("nome em maiuscula", exotica("", "Sharp")),
        ("default-x unico", exotica(" default-x=\"-12.5\" relative-y=\"3\"", "sharp")),
        ("aspas simples com >", exotica(" font-family='a>b'", "sharp")),
        ("valor com barra", exotica(" font-family=\"a/\"", "sharp")),
    ];
    for (nome, acc) in casos {
        let texto = peca_com(
            "",
            &format!(
                "{}{}{}{}",
                nota('C', None, None),
                nota('F', Some(1), Some(&acc)),
                nota('E', None, None),
                nota('G', None, None)
            ),
        );
        let mut d = match DocumentoAberto::abrir_texto("x", &texto) {
            Ok(d) => d,
            Err(e) => {
                println!("SONDA3 pres [{nome}] nem abre: {e}");
                continue;
            }
        };
        let id = d.ids().id(0, 0, 1).unwrap();
        let (mut exato, mut recusado, mut difere, mut undo_difere) = (0, 0, 0, 0);
        let mut det = String::new();
        for (u, s) in [
            (UnidadeDeAltura::Semitom, SentidoDaAltura::Acima),
            (UnidadeDeAltura::Semitom, SentidoDaAltura::Abaixo),
            (UnidadeDeAltura::GrauDaEscala, SentidoDaAltura::Acima),
            (UnidadeDeAltura::GrauDaEscala, SentidoDaAltura::Abaixo),
            (UnidadeDeAltura::Oitava, SentidoDaAltura::Acima),
        ] {
            let antes = d.exportar().unwrap();
            let r = match d.aplicar(ComandoDeEdicao::MudarAltura { nota: id, unidade: u, sentido: s }) {
                Ok(r) => r,
                Err(e) => {
                    recusado += 1;
                    det.push_str(&format!(" {u:?}/{s:?} recusado: {e};"));
                    continue;
                }
            };
            match d.aplicar(r.inverso.clone()) {
                Ok(_) => {
                    if d.exportar().unwrap() == antes {
                        exato += 1;
                    } else {
                        difere += 1;
                        det.push_str(&format!(
                            " inverso difere ({u:?}/{s:?}): {:?};",
                            trecho(&d.exportar().unwrap())
                        ));
                    }
                }
                Err(e) => {
                    recusado += 1;
                    det.push_str(&format!(" inverso recusado: {e};"));
                }
            }
            let n = d.entradas_de_desfazer();
            for _ in 0..n {
                d.desfazer();
            }
            if d.exportar().unwrap() != antes {
                undo_difere += 1;
            }
        }
        println!("SONDA3 pres [{nome}] exato={exato} recusado={recusado} difere={difere} undo_difere={undo_difere}{det}");
    }
}

#[test]
fn zz3_h_mudar_altura_em_nota_exotica_com_comentario_hostil() {
    // O caminho da tela: MudarAltura (seta) numa nota cujo <accidental> tem atributo fora do esquema
    // (aceito so porque o arquivo o tem), num arquivo com um comentario de `<accidental "` repetido.
    for n in [0usize, 4_000, 16_000] {
        let comentario = format!("<!-- {} -->\n", "<accidental \"".repeat(n));
        let texto = peca_com(
            &comentario,
            &format!(
                "{}{}{}{}",
                nota('C', None, None),
                nota('F', Some(1), Some("<accidental print-object=\"no\">sharp</accidental>")),
                nota('E', None, None),
                nota('G', None, None)
            ),
        );
        let mut d = DocumentoAberto::abrir_texto("x", &texto).unwrap();
        let id = d.ids().id(0, 0, 1).unwrap();
        let t = Instant::now();
        let r = d.aplicar(ComandoDeEdicao::MudarAltura {
            nota: id,
            unidade: UnidadeDeAltura::Semitom,
            sentido: SentidoDaAltura::Acima,
        });
        println!(
            "SONDA3 perf MudarAltura (nota com print-object) n={n} bytes={} ok={} em {:?}",
            texto.len(),
            r.is_ok(),
            t.elapsed()
        );
        let inv = r.unwrap().inverso;
        let t = Instant::now();
        let ri = d.aplicar(inv);
        println!("SONDA3 perf inverso (grafia do arquivo) n={n} ok={} em {:?}", ri.is_ok(), t.elapsed());
    }
}

#[test]
fn zz3_g_desempenho_pedacos() {
    for n in [2_000usize, 8_000, 16_000] {
        let comentario = format!("<!-- {} -->\n", "<accidental \"".repeat(n));
        let texto = quatro(&comentario);
        let t = Instant::now();
        let mut d = DocumentoAberto::abrir_texto("x", &texto).unwrap();
        let abre = t.elapsed();
        let t = Instant::now();
        let r = definir(&mut d, 1, 1, Some("sharp zzz=\"1\""));
        println!(
            "SONDA3 perf quadratico n={n} bytes={} abrir={abre:?} comando invalido aceito={:?} em {:?}",
            texto.len(),
            r.is_ok(),
            t.elapsed()
        );
    }
    for n in [5_000usize, 20_000, 40_000] {
        let texto = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<score-partwise version=\"4.0\"><part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list><part id=\"P1\">{}</part></score-partwise>",
            (0..n)
                .map(|i| format!(
                    "<measure number=\"{}\">{}{}</measure>",
                    i + 1,
                    if i == 0 {
                        "<attributes><divisions>4</divisions><time><beats>1</beats><beat-type>4</beat-type></time></attributes>"
                    } else {
                        ""
                    },
                    nota('F', Some(1), Some(&format!("<accidental default-x=\"-{i}.5\">sharp</accidental>")))
                ))
                .collect::<String>()
        );
        let mut d = match DocumentoAberto::abrir_texto("x", &texto) {
            Ok(d) => d,
            Err(e) => {
                println!("SONDA3 perf n={n} nem abre: {e}");
                continue;
            }
        };
        let id = d.ids().id(0, n / 2, 0).unwrap();
        let t = Instant::now();
        let r = d.aplicar(ComandoDeEdicao::DefinirAltura {
            nota: id,
            altura: Altura { passo: 'F', alter: 1, oitava: 4 },
            acidente: Some("sharp zzz=\"1\"".into()),
        });
        let e1 = t.elapsed();
        println!(
            "SONDA3 perf muitos distintos n={n} bytes={} grafia invalida recusada={:?} em {e1:?}",
            texto.len(),
            r.is_err()
        );
    }
}
