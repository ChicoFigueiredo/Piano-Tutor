//! Recusas explícitas e preservação de comentários no spike (ticket 08).

mod comum;

use piano_tutor_fixtures::corpus::fixture;
use piano_tutor_fixtures::documento::{Altura, Documento, ErroDocumento};

#[test]
fn score_timewise_e_recusado_com_erro_explicito() {
    let xml = "<?xml version=\"1.0\"?><score-timewise version=\"4.0\"><part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list><measure number=\"1\"><part id=\"P1\"/></measure></score-timewise>";
    assert!(matches!(
        Documento::abrir(xml),
        Err(ErroDocumento::NaoSuportado(_))
    ));
}

#[test]
fn xml_malformado_e_erro_e_nao_panico() {
    for ruim in [
        "",
        "<a>",
        "<score-partwise><part></score-partwise>",
        "texto solto",
    ] {
        assert!(
            matches!(Documento::abrir(ruim), Err(ErroDocumento::Xml(_))),
            "{ruim:?}"
        );
    }
}

#[test]
fn comentarios_e_instrucoes_dentro_do_compasso_editado_sobrevivem() {
    let f = fixture("duas-vozes");
    let (_, texto) = comum::variantes(&f.conteudo)
        .into_iter()
        .find(|(r, _)| *r == "comentarios-e-pi-dentro-do-compasso")
        .expect("variante");
    let mut doc = Documento::abrir(&texto).expect("abre");
    let id = doc.notas().next().expect("nota").id;
    doc.definir_altura(
        id,
        Altura {
            passo: 'A',
            alter: 0,
            oitava: 4,
        },
    )
    .expect("edita");
    let saida = doc.exportar().expect("exporta");
    assert_ne!(saida, texto);
    assert_eq!(saida.matches("<!--").count(), texto.matches("<!--").count());
    assert_eq!(
        saida.matches("<?pi dado?>").count(),
        texto.matches("<?pi dado?>").count()
    );
    assert!(saida.contains("<!-- n </note> &amp; -->"));
    assert!(saida.contains("<!-- fim --></measure>"));
}
