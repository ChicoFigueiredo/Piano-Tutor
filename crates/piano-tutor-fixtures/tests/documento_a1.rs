//! A1 do spike (ticket 08): abrir e exportar sem editar devolve bytes idênticos, no corpus todo
//! e em variações adversas de codificação, fim de linha, DTD, entidades e atributos.

mod comum;

use piano_tutor_fixtures::corpus::corpus;
use piano_tutor_fixtures::documento::{Documento, ErroDocumento};

#[test]
fn a1_corpus_inteiro_abre_e_exporta_identico() {
    for f in corpus() {
        let doc =
            Documento::abrir(&f.conteudo).unwrap_or_else(|e| panic!("{}: não abriu: {e}", f.id));
        let saida = doc.exportar().expect("exporta");
        assert_eq!(
            saida.as_bytes(),
            f.conteudo.as_bytes(),
            "{}: bytes diferentes",
            f.id
        );
        assert!(
            doc.compassos_sujos().is_empty(),
            "{}: nada foi editado",
            f.id
        );
    }
}

#[test]
fn a1_a_decomposicao_em_pecas_reconstroi_o_texto_sem_depender_do_atalho() {
    // `exportar` copia o original quando nada está sujo; este teste força regenerar TODOS os
    // compassos a partir das peças, para provar que a decomposição não perde nenhum byte.
    for f in corpus() {
        let doc = Documento::abrir(&f.conteudo).expect("abre");
        let saida = doc.exportar_regenerando_tudo().expect("regenera");
        assert_eq!(saida.as_bytes(), f.conteudo.as_bytes(), "{}", f.id);
    }
}

#[test]
fn texto_que_nao_e_utf8_e_recusado_com_erro_explicito() {
    let mut utf16 = vec![0xFF, 0xFE];
    utf16.extend("<a/>".encode_utf16().flat_map(u16::to_le_bytes));
    assert!(matches!(
        Documento::abrir_bytes(&utf16),
        Err(ErroDocumento::NaoUtf8(_))
    ));
    let latin1 = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><a>\xe9</a>";
    assert!(matches!(
        Documento::abrir_bytes(latin1),
        Err(ErroDocumento::NaoUtf8(_))
    ));
}

#[test]
fn a1_variantes_adversas_de_todo_o_corpus_voltam_identicas() {
    let mut falhas = Vec::new();
    let mut total = 0;
    for f in corpus() {
        for (rotulo, texto) in comum::variantes(&f.conteudo) {
            total += 1;
            match Documento::abrir(&texto).and_then(|d| {
                let a = d.exportar()?;
                let b = d.exportar_regenerando_tudo()?;
                Ok((a, b))
            }) {
                Ok((a, b)) if a == texto && b == texto => {}
                Ok(_) => falhas.push(format!("{}/{rotulo}: bytes diferentes", f.id)),
                Err(e) => falhas.push(format!("{}/{rotulo}: {e}", f.id)),
            }
        }
    }
    assert!(
        falhas.is_empty(),
        "{} de {total} falharam:\n{}",
        falhas.len(),
        falhas.join("\n")
    );
}

#[test]
fn entidade_que_expande_elementos_e_recusada_ou_identica_nunca_corrompe() {
    for f in corpus() {
        for (rotulo, texto) in comum::hostis(&f.conteudo) {
            match Documento::abrir(&texto) {
                Err(e) => assert!(
                    matches!(e, ErroDocumento::FaixaInconsistente(_)),
                    "{}/{rotulo}: recusa com causa errada: {e}",
                    f.id
                ),
                Ok(d) => {
                    let saida = d.exportar_regenerando_tudo().expect("regenera");
                    assert_eq!(saida, texto, "{}/{rotulo}: aceitou e corrompeu", f.id);
                }
            }
        }
    }
}
