//! Leitura de todas as fixtures com o modelo atual e relatório do que ele descarta.

use piano_tutor_fixtures::corpus::corpus;
use piano_tutor_fixtures::leitura::{relatorio_de_leitura, renderizar_markdown};

const SNAPSHOT: &str = "RELATORIO-DESCARTES.md";

#[test]
fn modelo_atual_abre_todas_as_fixtures_sem_erro() {
    for f in corpus() {
        let r = relatorio_de_leitura(&f);
        assert_eq!(r.leitura, Ok(()), "modelo atual recusou '{}'", f.id);
    }
}

#[test]
fn relatorio_registra_o_que_o_modelo_descarta() {
    let rels: Vec<_> = corpus().iter().map(relatorio_de_leitura).collect();
    let item = |id: &str, cat: &str| {
        rels.iter()
            .find(|r| r.id == id)
            .and_then(|r| r.itens.iter().find(|i| i.categoria.starts_with(cat)))
            .cloned()
            .unwrap_or_else(|| panic!("{id}/{cat}"))
    };
    // O que o modelo guarda chega inteiro.
    let notas = item("duas-vozes", "notas e pausas");
    assert_eq!(notas.no_arquivo, 6);
    assert_eq!(notas.descartados(), 0);
    assert_eq!(item("tercina", "tercinas").descartados(), 0);
    assert_eq!(item("repeticao", "marcas de repetição").descartados(), 0);
    // O que não tem campo no modelo é descartado por inteiro.
    for (id, cat, quantos) in [
        ("letra", "letra", 6),
        ("cifra", "cifra", 3),
        ("dedilhado", "dedilhado", 6),
        ("dinamica", "dinâmica", 4),
        ("ornamento", "ornamentos", 4),
        ("desconhecido", "elementos desconhecidos", 6),
    ] {
        let i = item(id, cat);
        assert_eq!(i.no_modelo, None, "{id}");
        assert_eq!(i.no_arquivo, quantos, "{id}/{cat}");
        assert_eq!(i.descartados(), quantos);
    }
}

/// Grava o relatório em `target/tmp`; com `PT_ATUALIZAR_RELATORIO=1` atualiza a cópia versionada
/// (`crates/piano-tutor-fixtures/RELATORIO-DESCARTES.md`); com `PT_VERIFICAR_RELATORIO=1` falha
/// se a cópia versionada estiver desatualizada (não é exigido por padrão para não bloquear
/// tickets que mudam o modelo).
#[test]
fn relatorio_de_descartes_e_gravado() {
    let rels: Vec<_> = corpus().iter().map(relatorio_de_leitura).collect();
    let md = renderizar_markdown(&rels);
    let tmp = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(SNAPSHOT);
    std::fs::write(&tmp, &md).expect("grava relatório temporário");
    let versionado = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SNAPSHOT);
    if std::env::var_os("PT_ATUALIZAR_RELATORIO").is_some() {
        std::fs::write(&versionado, &md).expect("atualiza relatório versionado");
    } else if std::env::var_os("PT_VERIFICAR_RELATORIO").is_some() {
        let atual = std::fs::read_to_string(&versionado).unwrap_or_default();
        assert_eq!(
            atual, md,
            "relatório desatualizado; rode com PT_ATUALIZAR_RELATORIO=1"
        );
    }
    assert!(md.contains("## letra"));
}
