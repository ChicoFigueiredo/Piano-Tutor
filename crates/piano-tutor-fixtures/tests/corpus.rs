//! O corpus tem licença e origem registradas, cobre os casos críticos e o índice está em dia.

use piano_tutor_fixtures::corpus::{Origem, corpus, sha256_hex};
use piano_tutor_fixtures::gerador::gerar_cem_compassos;
use piano_tutor_fixtures::harness::extrair_semantica;
use std::collections::BTreeSet;
use std::path::Path;

const INDICE: &str = include_str!("../fixtures/INDICE.md");

#[test]
fn cada_fixture_consta_no_indice_com_sha256_e_licenca() {
    for f in corpus() {
        let linha = INDICE
            .lines()
            .find(|l| l.starts_with(&format!("| `{}` |", f.id)))
            .unwrap_or_else(|| panic!("fixture '{}' ausente de fixtures/INDICE.md", f.id));
        let sha = sha256_hex(f.conteudo.as_bytes());
        assert!(
            linha.contains(&sha),
            "SHA-256 de '{}' no índice não confere; o correto é {sha}",
            f.id
        );
        assert!(
            linha.contains("CC0"),
            "'{}' sem licença CC0 registrada no índice",
            f.id
        );
        assert!(
            linha.contains(f.arquivo.split(" (").next().unwrap_or_default()),
            "origem de '{}' não registrada",
            f.id
        );
    }
}

#[test]
fn todo_arquivo_de_fixtures_esta_no_corpus() {
    let pasta = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let no_corpus: BTreeSet<String> = corpus()
        .iter()
        .filter(|f| f.origem == Origem::Propria)
        .map(|f| f.arquivo.rsplit('/').next().unwrap_or_default().to_owned())
        .collect();
    let em_disco: BTreeSet<String> = std::fs::read_dir(pasta)
        .expect("lê fixtures/")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".musicxml"))
        .collect();
    assert_eq!(
        no_corpus, em_disco,
        "arquivo fora do corpus (ou o contrário)"
    );
}

#[test]
fn ids_sao_unicos_e_toda_fixture_declara_cc0() {
    let c = corpus();
    let ids: BTreeSet<_> = c.iter().map(|f| f.id).collect();
    assert_eq!(ids.len(), c.len());
    for f in &c {
        assert!(
            f.conteudo.contains("CC0"),
            "'{}' não declara CC0 em <rights>",
            f.id
        );
    }
}

#[test]
fn corpus_cobre_os_casos_criticos() {
    let casos: BTreeSet<&str> = corpus()
        .iter()
        .flat_map(|f| f.casos.iter().copied())
        .collect();
    for esperado in [
        "anacruse",
        "duas vozes numa pauta",
        "tercina",
        "repetição",
        "letra",
        "cifra",
        "dedilhado",
        "dinâmica",
        "ornamento",
        "elemento sem suporte de renderização",
        "peça de 100 compassos",
    ] {
        assert!(casos.contains(esperado), "caso '{esperado}' sem fixture");
    }
}

#[test]
fn fixtures_proprias_contem_o_que_dizem_conter() {
    let por_id = |id: &str| {
        corpus()
            .into_iter()
            .find(|f| f.id == id)
            .expect(id)
            .conteudo
    };
    assert!(por_id("anacruse").contains("implicit=\"yes\""));
    assert!(por_id("duas-vozes").contains("<backup>"));
    assert!(por_id("tercina").contains("<actual-notes>3</actual-notes>"));
    assert!(por_id("repeticao").contains("<repeat direction=\"backward\"/>"));
    assert!(por_id("letra").contains("<lyric"));
    assert!(por_id("cifra").contains("<harmony>"));
    assert!(por_id("dedilhado").contains("<fingering"));
    assert!(por_id("dinamica").contains("<dynamics>"));
    assert!(por_id("ornamento").contains("<ornaments>"));
    assert!(por_id("desconhecido").contains("<x-elemento-inexistente>"));
}

#[test]
fn peca_longa_tem_100_compassos_e_e_deterministica() {
    let a = gerar_cem_compassos();
    assert_eq!(a, gerar_cem_compassos());
    let s = extrair_semantica(&a).expect("analisa");
    assert_eq!(s.compassos.len(), 100);
}

#[test]
fn semantica_de_cada_fixture_e_analisavel() {
    for f in corpus() {
        let s = extrair_semantica(&f.conteudo)
            .unwrap_or_else(|e| panic!("'{}' não analisável: {e}", f.id));
        assert!(!s.compassos.is_empty(), "'{}' sem compassos", f.id);
    }
}
