//! Regressões fixas do leitor de XML/MXL (ticket 11). Toda entrada que o fuzz
//! (`fuzz_xml_mxl.rs`) ou uma revisão achar problemática entra aqui e passa a rodar em todo
//! `cargo test`. Entradas binárias (por exemplo `.mxl` mutados) podem ser copiadas de
//! `target/tmp/fuzz-falhas/` para `tests/regressoes/` (extensão `.musicxml` ou `.mxl`): o teste
//! reabre todos os arquivos dessa pasta, se ela existir.

use piano_tutor_core::documento::{DocumentoAberto, Formato};

/// (rótulo, bytes, formato, deve abrir?)
fn embutidas() -> Vec<(&'static str, Vec<u8>, Formato, bool)> {
    let g = "a".repeat(20_000);
    vec![
        // Achado do ticket 11: o detector de laço do roxmltree não conta referências no nível
        // zero, então 20 KB x 200 referências (4 MB) expandiriam sem aviso; com 8 MiB de arquivo
        // seriam dezenas de GB.
        (
            "explosao-quadratica-pequena",
            format!(
                "<!DOCTYPE score-partwise [<!ENTITY g \"{g}\">]><score-partwise>{}</score-partwise>",
                "<work-title>&g;</work-title>".repeat(200)
            )
            .into_bytes(),
            Formato::MusicXml,
            false,
        ),
        // DOCTYPE falso dentro de comentário antes do verdadeiro não pode enganar o orçamento.
        (
            "doctype-falso-em-comentario",
            format!(
                "<!-- <!DOCTYPE s [<!ENTITY g \"\">]> --><!DOCTYPE score-partwise [<!ENTITY g \"{g}\">]><score-partwise a=\"{}\"/>",
                "&g;".repeat(200)
            )
            .into_bytes(),
            Formato::MusicXml,
            false,
        ),
        // Aninhamento que não estoura a pilha.
        (
            "aninhamento-de-meio-milhao",
            format!("{}{}", "<a>".repeat(500_000), "</a>".repeat(500_000)).into_bytes(),
            Formato::MusicXml,
            false,
        ),
        // Bytes soltos.
        ("so-bom", vec![0xEF, 0xBB, 0xBF], Formato::MusicXml, false),
        ("so-pk", b"PK".to_vec(), Formato::Mxl, false),
        ("doctype-sem-fim", b"<!DOCTYPE a [<!ENTITY x \"".to_vec(), Formato::MusicXml, false),
    ]
}

#[test]
fn regressoes_embutidas_seguem_recusadas_sem_panico() {
    for (nome, bytes, formato, deve_abrir) in embutidas() {
        let r = DocumentoAberto::abrir_bytes(nome, &bytes, formato);
        assert_eq!(r.is_ok(), deve_abrir, "{nome}");
        if let Ok(d) = r {
            assert_eq!(
                d.exportar().expect("exporta").as_bytes(),
                &bytes[..],
                "{nome}"
            );
        }
    }
}

#[test]
fn regressoes_da_pasta_seguem_sem_panico_e_com_a1() {
    let pasta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/regressoes");
    let Ok(lista) = std::fs::read_dir(&pasta) else {
        return;
    };
    for e in lista.flatten() {
        let caminho = e.path();
        let formato = match caminho.extension().and_then(|x| x.to_str()) {
            Some("mxl") => Formato::Mxl,
            Some("musicxml" | "xml") => Formato::MusicXml,
            _ => continue,
        };
        let bytes = std::fs::read(&caminho).expect("lê");
        if let Ok(d) = DocumentoAberto::abrir_bytes("regressao", &bytes, formato)
            && formato == Formato::MusicXml
        {
            assert_eq!(d.exportar().expect("exporta").as_bytes(), &bytes[..]);
        }
    }
}
