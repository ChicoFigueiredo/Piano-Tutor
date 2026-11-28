//! Ticket 10 (E01): o serviço de documento do núcleo abre e exporta idêntico, sem GUI, sem áudio
//! e sem rede. Critério A1 pela porta única (`DocumentoAberto`), no corpus todo.

use piano_tutor_core::documento::{DocumentoAberto, ErroDocumento, ErroServico, Formato};
use piano_tutor_core::mxl::empacotar_mxl;
use piano_tutor_fixtures::corpus::corpus;

#[test]
fn a1_corpus_inteiro_abre_e_exporta_identico_pelo_servico() {
    for f in corpus() {
        let d = DocumentoAberto::abrir_texto(f.id, &f.conteudo)
            .unwrap_or_else(|e| panic!("{}: não abriu: {e}", f.id));
        let saida = d
            .exportar()
            .unwrap_or_else(|e| panic!("{}: não exportou: {e}", f.id));
        assert_eq!(saida.as_bytes(), f.conteudo.as_bytes(), "{}", f.id);
        assert!(!d.editado(), "{}: nada foi editado", f.id);
        assert_eq!(d.texto_original(), f.conteudo, "{}", f.id);
        d.conferir_exportacao(&saida)
            .unwrap_or_else(|e| panic!("{}: conferência: {e}", f.id));
    }
}

#[test]
fn a1_o_mesmo_vale_para_o_xml_dentro_do_mxl() {
    for f in corpus() {
        let mxl = empacotar_mxl("partitura.musicxml", &f.conteudo).expect("zip");
        let d = DocumentoAberto::abrir_bytes(f.id, &mxl, Formato::Mxl)
            .unwrap_or_else(|e| panic!("{}: mxl não abriu: {e}", f.id));
        assert_eq!(d.formato(), Formato::Mxl);
        assert_eq!(
            d.exportar().expect("exporta").as_bytes(),
            f.conteudo.as_bytes(),
            "{}: o contrato vale sobre o XML interno",
            f.id
        );
    }
}

#[test]
fn a1_bytes_utf8_de_musicxml_abrem_e_voltam_identicos() {
    for f in corpus() {
        let d = DocumentoAberto::abrir_bytes(f.id, f.conteudo.as_bytes(), Formato::MusicXml)
            .unwrap_or_else(|e| panic!("{}: {e}", f.id));
        assert_eq!(d.formato(), Formato::MusicXml);
        assert_eq!(d.exportar().expect("exporta"), f.conteudo, "{}", f.id);
    }
}

#[test]
fn toda_nota_do_modelo_de_tocar_tem_identidade_unica_e_igual_a_do_documento() {
    for f in corpus() {
        let d = DocumentoAberto::abrir_texto(f.id, &f.conteudo).expect("abre");
        let mut vistos = std::collections::HashSet::new();
        let mut total = 0;
        for (pi, parte) in d.partitura().partes.iter().enumerate() {
            for (ci, comp) in parte.compassos.iter().enumerate() {
                for oi in 0..comp.notas.len() {
                    let id = d
                        .ids()
                        .id(pi, ci, oi)
                        .unwrap_or_else(|| panic!("{}: sem id em {pi}/{ci}/{oi}", f.id));
                    assert!(vistos.insert(id), "{}: id repetido", f.id);
                    assert!(d.documento().nota(id).is_some(), "{}", f.id);
                    total += 1;
                }
            }
        }
        assert_eq!(total, d.partitura().total_de_notas(), "{}", f.id);
        assert_eq!(total, d.documento().total_de_notas(), "{}", f.id);
        assert_eq!(d.ids().total(), total, "{}", f.id);
    }
}

#[test]
fn a_altura_da_nota_do_documento_confere_com_a_do_modelo_de_tocar() {
    // Prova que o mapeamento (parte, compasso, ordinal) liga a nota certa nos dois modelos.
    for f in corpus() {
        let d = DocumentoAberto::abrir_texto(f.id, &f.conteudo).expect("abre");
        for (pi, parte) in d.partitura().partes.iter().enumerate() {
            for (ci, comp) in parte.compassos.iter().enumerate() {
                for (oi, n) in comp.notas.iter().enumerate() {
                    let id = d.ids().id(pi, ci, oi).expect("id");
                    let nd = d.documento().nota(id).expect("nota do documento");
                    assert_eq!(
                        n.altura.is_none(),
                        nd.altura.is_none(),
                        "{} {pi}/{ci}/{oi}",
                        f.id
                    );
                    if let (Some(a), Some(ad)) = (n.altura, nd.altura) {
                        let passo = ['C', 'D', 'E', 'F', 'G', 'A', 'B'][a.passo as usize];
                        assert_eq!(passo, ad.passo, "{} {pi}/{ci}/{oi}", f.id);
                        assert_eq!(a.oitava, ad.oitava, "{} {pi}/{ci}/{oi}", f.id);
                    }
                    assert_eq!(n.acorde, nd.acorde, "{} {pi}/{ci}/{oi}", f.id);
                    assert_eq!(n.adorno, nd.graca, "{} {pi}/{ci}/{oi}", f.id);
                }
            }
        }
    }
}

#[test]
fn as_recusas_do_documento_viram_erro_tipado_do_servico() {
    let utf16: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain("<a/>".encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    let e = DocumentoAberto::abrir_bytes("x", &utf16, Formato::MusicXml).expect_err("recusa");
    assert!(
        matches!(e, ErroServico::Documento(ErroDocumento::NaoUtf8(_))),
        "{e:?}"
    );
    let e = DocumentoAberto::abrir_texto("x", "<score-timewise/>").expect_err("recusa");
    assert!(matches!(e, ErroServico::Partitura(_)), "{e:?}");
    let e = DocumentoAberto::abrir_bytes("x", b"nao e zip", Formato::Mxl).expect_err("recusa");
    assert!(matches!(e, ErroServico::Partitura(_)), "{e:?}");
}

#[test]
fn conferir_recusa_texto_gravado_diferente() {
    let a = corpus().into_iter().next().expect("corpus");
    let b = corpus().into_iter().nth(1).expect("corpus");
    let d = DocumentoAberto::abrir_texto("a", &a.conteudo).expect("abre");
    let e = d.conferir_exportacao(&b.conteudo).expect_err("difere");
    assert!(matches!(e, ErroServico::ReleituraDifere(_)), "{e:?}");
    let e = d.conferir_exportacao("não é xml").expect_err("inválido");
    assert!(!e.to_string().is_empty());
}
