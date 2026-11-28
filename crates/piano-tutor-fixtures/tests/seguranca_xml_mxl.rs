//! Ticket 11 (E01, A13): segurança de XML e MXL na abertura pelo serviço de documento.
//!
//! Cada fixture maliciosa (XML malformado, entidade externa/XXE, entidade expandida em cadeia,
//! zip bomb, caminho fora do escopo, nome absoluto, `..`, entradas demais) tem de virar um
//! [`ErroServico`] tipado, com mensagem em português, sem ler nada fora do limite e sem pânico.
//! Os limites existentes valem na rota do serviço e cada um é ultrapassado aqui.

use piano_tutor_core::documento::{DocumentoAberto, ErroServico, Formato};
use piano_tutor_core::mxl::{
    MXL_MAX_ENTRADAS, MXL_TAMANHO_MAXIMO_BYTES, empacotar_mxl, extrair_musicxml,
};
use piano_tutor_core::partitura::{
    ErroPartitura, MAX_COMPASSOS, MAX_NOTAS, MAX_PARTES, TAMANHO_MAXIMO_BYTES,
};
use piano_tutor_fixtures::gerador::{mxl_de_teste, xml_de_teste};
use std::io::Write;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

/// Palavras em inglês que as mensagens do analisador XML trazem e que não podem chegar à tela.
const INGLES: [&str; 10] = [
    "unexpected",
    "expected",
    "invalid",
    "unknown",
    "entity",
    "reference",
    "limit reached",
    "detected",
    "stream",
    "could not",
];

fn confere_mensagem(nome: &str, e: &ErroServico) {
    let m = e.to_string();
    assert!(!m.is_empty(), "{nome}: mensagem vazia");
    assert!(
        m.chars().count() <= 400,
        "{nome}: mensagem longa demais: {m}"
    );
    let baixa = m.to_lowercase();
    for p in INGLES {
        assert!(!baixa.contains(p), "{nome}: mensagem em inglês ({p}): {m}");
    }
    for vazamento in ["/etc/passwd", "root:", "/home/", "/tmp/"] {
        assert!(!m.contains(vazamento), "{nome}: vaza {vazamento}: {m}");
    }
}

fn limite_ou_tamanho(e: &ErroServico) -> bool {
    matches!(
        e,
        ErroServico::Partitura(ErroPartitura::LimiteExcedido(_) | ErroPartitura::GrandeDemais(_))
    )
}

#[test]
fn a13_xml_hostil_recusado_pelo_servico_com_erro_tipado() {
    let casos = xml_de_teste();
    assert!(casos.iter().filter(|c| !c.valido).count() >= 10);
    for c in &casos {
        let r = DocumentoAberto::abrir_texto(c.nome, &c.texto);
        if c.valido {
            let d = r.unwrap_or_else(|e| panic!("{} deveria abrir: {e}", c.nome));
            assert_eq!(d.exportar().expect("exporta"), c.texto, "{}", c.nome);
        } else {
            let e = r.expect_err(&format!(
                "{} ({}) deveria ser recusado",
                c.nome, c.descricao
            ));
            confere_mensagem(c.nome, &e);
            // Os que exigem recursos desproporcionais são limite de segurança, não "XML inválido".
            if [
                "billion-laughs",
                "explosao-quadratica-no-texto",
                "explosao-quadratica-no-atributo",
                "aninhamento-profundo",
                "nos-demais",
                "entidade-ciclica",
            ]
            .contains(&c.nome)
            {
                assert!(limite_ou_tamanho(&e), "{}: {e:?}", c.nome);
            }
        }
    }
}

#[test]
fn a13_o_mesmo_xml_hostil_dentro_de_um_mxl_e_recusado() {
    for c in xml_de_teste() {
        // O tamanho do texto já passou do limite do zip em alguns casos: o empacotador recusa.
        let Ok(mxl) = empacotar_mxl("score.xml", &c.texto) else {
            continue;
        };
        let r = DocumentoAberto::abrir_bytes(c.nome, &mxl, Formato::Mxl);
        assert_eq!(r.is_ok(), c.valido, "{} dentro do .mxl", c.nome);
        if let Err(e) = r {
            confere_mensagem(c.nome, &e);
        }
    }
}

#[test]
fn a13_mxl_malicioso_recusado_pelo_servico_com_mensagem_em_portugues() {
    for m in mxl_de_teste() {
        let r = DocumentoAberto::abrir_bytes(m.nome, &m.bytes, Formato::Mxl);
        assert_eq!(r.is_ok(), m.valido, "{} ({})", m.nome, m.descricao);
        if let Err(e) = r {
            confere_mensagem(m.nome, &e);
            assert!(
                matches!(e, ErroServico::Partitura(_)),
                "{}: erro de contêiner é de partitura: {e:?}",
                m.nome
            );
        }
    }
}

#[test]
fn a13_bytes_que_nao_sao_xml_nem_zip_nao_derrubam() {
    let entradas: Vec<(&str, Vec<u8>)> = vec![
        ("vazio", vec![]),
        ("nul", vec![0; 4096]),
        ("utf16-le", [&[0xFF, 0xFE][..], b"<\0s\0"].concat()),
        ("utf8-invalido", vec![0xC3, 0x28, 0xA0, 0xA1, 0xFF]),
        ("zip-truncado", b"PK\x03\x04\x14\0\0\0".to_vec()),
        (
            "so-assinatura-fim-de-zip",
            b"PK\x05\x06\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0".to_vec(),
        ),
    ];
    for (nome, bytes) in entradas {
        for formato in [Formato::MusicXml, Formato::Mxl] {
            let e = DocumentoAberto::abrir_bytes(nome, &bytes, formato)
                .expect_err(&format!("{nome} {formato:?} deveria ser recusado"));
            confere_mensagem(nome, &e);
        }
    }
}

/// Prova real de XXE: um arquivo de verdade, com um marcador único, referenciado por entidade
/// externa. Se o marcador aparecer em qualquer lugar (documento aberto ou mensagem), houve leitura.
#[test]
fn xxe_nao_le_arquivo_algum_do_disco() {
    let pasta = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("xxe-ticket-11");
    std::fs::create_dir_all(&pasta).expect("pasta");
    let marcador = format!("SEGREDO-DO-DISCO-{}", std::process::id());
    let alvo = pasta.join("segredo.txt");
    std::fs::write(&alvo, &marcador).expect("grava");
    let uri = format!("file://{}", alvo.display());
    let variantes = [
        // Entidade geral externa usada no título.
        (format!("[<!ENTITY x SYSTEM \"{uri}\">]"), "&x;".to_owned()),
        // Idem com PUBLIC.
        (
            format!("[<!ENTITY x PUBLIC \"-//X//EN\" \"{uri}\">]"),
            "&x;".to_owned(),
        ),
        // Entidade de parâmetro externa com declaração injetada por ela.
        (
            format!("[<!ENTITY % p SYSTEM \"{uri}\"> %p;]"),
            "t".to_owned(),
        ),
        // Entidade interna que só *parece* um caminho (texto literal, nunca aberto).
        (format!("[<!ENTITY x \"{uri}\">]"), "t".to_owned()),
    ];
    let base = "<?xml version=\"1.0\"?>\n<!DOCTYPE score-partwise DOCTYPE_AQUI>\n<score-partwise version=\"4.0\"><work><work-title>TITULO_AQUI</work-title></work><part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list><part id=\"P1\"><measure number=\"1\"><attributes><divisions>1</divisions></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note></measure></part></score-partwise>";
    for (i, (doctype, titulo)) in variantes.iter().enumerate() {
        let xml = base
            .replace("DOCTYPE_AQUI", doctype)
            .replace("TITULO_AQUI", titulo);
        match DocumentoAberto::abrir_texto("x", &xml) {
            Ok(d) => {
                let tudo = format!(
                    "{:?}{}{}",
                    d.partitura().titulo,
                    d.texto_original(),
                    d.exportar().unwrap_or_default()
                );
                // Só o texto literal da própria entidade interna pode aparecer (variante 3).
                assert!(
                    !tudo.contains(&marcador),
                    "variante {i}: o conteúdo do arquivo foi lido"
                );
            }
            Err(e) => {
                let m = format!("{e} {e:?}");
                assert!(!m.contains(&marcador), "variante {i}: {m}");
            }
        }
    }
    let _ = std::fs::remove_dir_all(&pasta);
}

// --- Limites existentes, ultrapassados pela rota do serviço ------------------------------------

fn partitura_com(partes: usize, compassos: usize, notas_por_compasso: usize) -> String {
    let mut s = String::from("<score-partwise version=\"4.0\"><part-list>");
    for p in 0..partes {
        s.push_str(&format!(
            "<score-part id=\"P{p}\"><part-name>P</part-name></score-part>"
        ));
    }
    s.push_str("</part-list>");
    for p in 0..partes {
        s.push_str(&format!("<part id=\"P{p}\">"));
        for m in 0..compassos {
            s.push_str(&format!("<measure number=\"{}\">", m + 1));
            if m == 0 {
                s.push_str("<attributes><divisions>1</divisions></attributes>");
            }
            for _ in 0..notas_por_compasso {
                s.push_str("<note><rest/></note>");
            }
            s.push_str("</measure>");
        }
        s.push_str("</part>");
    }
    s.push_str("</score-partwise>");
    s
}

fn erro_de_limite(xml: &str, esperado: &str) {
    match DocumentoAberto::abrir_texto("limite", xml) {
        Err(ErroServico::Partitura(ErroPartitura::LimiteExcedido(q))) => {
            assert_eq!(q, esperado);
        }
        outro => panic!("esperava limite '{esperado}', veio {:?}", outro.map(|_| ())),
    }
}

#[test]
fn limite_de_partes_vale_na_rota_do_servico() {
    DocumentoAberto::abrir_texto("ok", &partitura_com(MAX_PARTES, 1, 1)).expect("no limite abre");
    erro_de_limite(&partitura_com(MAX_PARTES + 1, 1, 1), "número de partes");
}

#[test]
fn limite_de_compassos_vale_na_rota_do_servico() {
    erro_de_limite(
        &partitura_com(1, MAX_COMPASSOS + 1, 0),
        "número de compassos",
    );
}

#[test]
fn limite_de_notas_vale_na_rota_do_servico() {
    // Cabe nos 8 MiB e nos 2 milhões de nós, mas passa de 200 mil notas.
    let xml = partitura_com(1, 1, MAX_NOTAS + 1);
    assert!(xml.len() <= TAMANHO_MAXIMO_BYTES, "{} bytes", xml.len());
    erro_de_limite(&xml, "número de notas");
}

#[test]
fn limite_de_tamanho_do_texto_vale_na_rota_do_servico() {
    let grande = format!("<a>{}</a>", " ".repeat(TAMANHO_MAXIMO_BYTES));
    assert!(matches!(
        DocumentoAberto::abrir_texto("g", &grande),
        Err(ErroServico::Partitura(ErroPartitura::GrandeDemais(_)))
    ));
    assert!(matches!(
        DocumentoAberto::abrir_bytes("g", grande.as_bytes(), Formato::MusicXml),
        Err(ErroServico::Partitura(ErroPartitura::GrandeDemais(_)))
    ));
}

#[test]
fn limite_de_nos_xml_vale_na_rota_do_servico() {
    let caso = xml_de_teste()
        .into_iter()
        .find(|c| c.nome == "nos-demais")
        .expect("existe");
    erro_de_limite(&caso.texto, "número de nós XML");
}

#[test]
fn limite_de_tamanho_do_mxl_vale_na_rota_do_servico() {
    let enorme = vec![0_u8; MXL_TAMANHO_MAXIMO_BYTES + 1];
    assert!(matches!(
        DocumentoAberto::abrir_bytes("g", &enorme, Formato::Mxl),
        Err(ErroServico::Partitura(ErroPartitura::GrandeDemais(_)))
    ));
}

#[test]
fn limite_de_entradas_do_mxl_vale_na_rota_do_servico() {
    let m = mxl_de_teste()
        .into_iter()
        .find(|m| m.nome.contains("entradas-demais"))
        .expect("existe");
    assert!(matches!(
        DocumentoAberto::abrir_bytes(m.nome, &m.bytes, Formato::Mxl),
        Err(ErroServico::Partitura(ErroPartitura::LimiteExcedido(_)))
    ));
    const { assert!(MXL_MAX_ENTRADAS < 202) };
}

#[test]
fn limite_de_descompactacao_do_mxl_vale_na_rota_do_servico() {
    let m = mxl_de_teste()
        .into_iter()
        .find(|m| m.nome.contains("zip-bomb"))
        .expect("existe");
    assert!(matches!(
        DocumentoAberto::abrir_bytes(m.nome, &m.bytes, Formato::Mxl),
        Err(ErroServico::Partitura(ErroPartitura::LimiteExcedido(_)))
    ));
}

#[test]
fn nome_de_entrada_gigante_nao_vaza_na_mensagem() {
    let nome = format!("../{}", "a".repeat(5000));
    let mut z = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let o = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    z.start_file(nome.as_str(), o).expect("início");
    z.write_all(b"x").expect("escrita");
    let bytes = z.finish().expect("fim").into_inner();
    let e = extrair_musicxml(&bytes).expect_err("recusa");
    assert!(e.to_string().chars().count() < 300, "{e}");
}
