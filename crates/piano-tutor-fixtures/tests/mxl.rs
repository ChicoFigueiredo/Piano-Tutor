//! `.mxl` válidos e maliciosos gerados por script: o leitor atual abre os válidos e recusa os
//! maliciosos com erro (sem pânico e sem gravar nada fora do escopo).

use piano_tutor_core::partitura::Partitura;
use piano_tutor_fixtures::gerador::{COMPASSOS_LONGA, gravar_mxl_de_teste, mxl_de_teste};

#[test]
fn mxl_valido_abre_e_maliciosos_sao_recusados() {
    let todos = mxl_de_teste();
    assert!(todos.iter().any(|m| m.valido));
    assert!(todos.iter().filter(|m| !m.valido).count() >= 4);
    for m in todos {
        let r = Partitura::de_mxl(&m.bytes);
        if m.valido {
            let p = r.unwrap_or_else(|e| panic!("{} deveria abrir: {e}", m.nome));
            assert_eq!(p.compassos.len(), COMPASSOS_LONGA, "{}", m.nome);
        } else {
            assert!(
                r.is_err(),
                "{} ({}) deveria ser recusado",
                m.nome,
                m.descricao
            );
        }
    }
}

#[test]
fn zip_bomb_de_teste_e_pequena_no_disco() {
    let bomba = mxl_de_teste()
        .into_iter()
        .find(|m| m.nome.contains("zip-bomb"))
        .expect("existe");
    assert!(
        bomba.bytes.len() < 256 * 1024,
        "{} bytes",
        bomba.bytes.len()
    );
}

#[test]
fn mxl_de_teste_sao_gravados_em_pasta_e_nada_escapa_dela() {
    let pasta = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("mxl-de-teste");
    std::fs::create_dir_all(&pasta).expect("cria pasta");
    let caminhos = gravar_mxl_de_teste(&pasta).expect("grava");
    assert_eq!(caminhos.len(), mxl_de_teste().len());
    for c in &caminhos {
        let bytes = std::fs::read(c).expect("lê de volta");
        let _ = Partitura::de_mxl(&bytes);
    }
    assert!(
        !pasta
            .parent()
            .expect("pai")
            .join("fora-do-escopo.txt")
            .exists()
    );
}
