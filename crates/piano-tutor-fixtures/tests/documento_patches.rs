//! As faixas de bytes só valem para o texto original (ticket 08): patches aplicados uma única vez,
//! de trás para frente; a segunda aplicação sobre o texto novo é recusada.

use piano_tutor_fixtures::corpus::fixture;
use piano_tutor_fixtures::documento::{Altura, Documento, ErroDocumento, Patch, Patches};

#[test]
fn patches_que_mudam_o_tamanho_aplicados_de_tras_para_frente_dao_o_texto_esperado() {
    let base = "0123456789";
    // Crescem, encolhem e somem: se a ordem fosse de frente para trás, as faixas seguintes
    // apontariam para o lugar errado.
    let patches = Patches::novo(
        base,
        vec![
            Patch {
                faixa: 2..4,
                novo: "AAAAA".into(),
            },
            Patch {
                faixa: 5..6,
                novo: String::new(),
            },
            Patch {
                faixa: 8..9,
                novo: "ZZ".into(),
            },
        ],
    );
    assert_eq!(
        patches.clone().aplicar(base).expect("aplica"),
        "01AAAAA467ZZ9"
    );
}

#[test]
fn aplicar_de_frente_para_tras_daria_resultado_errado_prova_que_a_ordem_importa() {
    let base = "0123456789";
    let itens = [(2..4, "AAAAA"), (8..9, "ZZ")];
    // Ingênuo, de frente para trás, reusando as faixas originais depois que o texto cresceu.
    let mut errado = base.to_owned();
    for (faixa, novo) in &itens {
        errado.replace_range(faixa.clone(), novo);
    }
    let certo = Patches::novo(
        base,
        itens
            .iter()
            .map(|(f, n)| Patch {
                faixa: f.clone(),
                novo: (*n).to_owned(),
            })
            .collect(),
    )
    .aplicar(base)
    .expect("aplica");
    assert_eq!(certo, "01AAAAA4567ZZ9");
    assert_ne!(errado, certo);
}

#[test]
fn segunda_aplicacao_sobre_o_texto_novo_e_recusada() {
    let f = fixture("duas-vozes");
    let mut doc = Documento::abrir(&f.conteudo).expect("abre");
    let id = doc.notas().next().expect("nota").id;
    doc.definir_altura(
        id,
        Altura {
            passo: 'G',
            alter: 1,
            oitava: 6,
        },
    )
    .expect("edita");
    let patches = doc.patches();
    assert_eq!(patches.itens().len(), 1);
    let novo = patches.clone().aplicar(&f.conteudo).expect("1ª aplicação");
    assert_ne!(novo, f.conteudo);
    // Mesmo com uma cópia dos patches, o texto novo tem outra impressão digital: recusado.
    assert_eq!(
        patches.clone().aplicar(&novo),
        Err(ErroDocumento::PatchesDeOutroTexto)
    );
    // Sobre o texto-base continua sendo determinístico.
    assert_eq!(patches.aplicar(&f.conteudo).expect("de novo no base"), novo);
}

#[test]
fn patches_de_um_documento_nao_se_aplicam_a_outro_texto() {
    let a = fixture("duas-vozes");
    let b = fixture("tercina");
    let mut doc = Documento::abrir(&a.conteudo).expect("abre");
    let id = doc.notas().next().expect("nota").id;
    doc.definir_altura(
        id,
        Altura {
            passo: 'A',
            alter: 0,
            oitava: 5,
        },
    )
    .expect("edita");
    assert_eq!(
        doc.patches().aplicar(&b.conteudo),
        Err(ErroDocumento::PatchesDeOutroTexto)
    );
}

#[test]
fn patches_sobrepostos_ou_fora_do_texto_sao_recusados() {
    let base = "abcdef";
    let sobrepostos = Patches::novo(
        base,
        vec![
            Patch {
                faixa: 1..4,
                novo: "x".into(),
            },
            Patch {
                faixa: 3..5,
                novo: "y".into(),
            },
        ],
    );
    assert!(matches!(
        sobrepostos.aplicar(base),
        Err(ErroDocumento::PatchesInvalidos(_))
    ));
    let fora = Patches::novo(
        base,
        vec![Patch {
            faixa: 4..9,
            novo: "x".into(),
        }],
    );
    assert!(matches!(
        fora.aplicar(base),
        Err(ErroDocumento::PatchesInvalidos(_))
    ));
    let meio_caractere = "aé";
    let corta = Patches::novo(
        meio_caractere,
        vec![Patch {
            faixa: 2..3,
            novo: "x".into(),
        }],
    );
    assert!(matches!(
        corta.aplicar(meio_caractere),
        Err(ErroDocumento::PatchesInvalidos(_))
    ));
}
