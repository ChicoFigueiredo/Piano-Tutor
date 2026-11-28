use piano_tutor_core::documento::{Altura, ComandoDeEdicao, DocumentoAberto, PosCompasso};
use piano_tutor_core::partitura::Fracao;

fn peca(medida: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<score-partwise version=\"4.0\">\n  \
         <part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list>\n  \
         <part id=\"P1\">\n    <measure number=\"1\"><attributes><divisions>1</divisions>\
         <key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time>\
         </attributes>\n{medida}    </measure>\n  </part>\n</score-partwise>\n"
    )
}

#[test]
fn sonda_escape_de_pauta_e_voz() {
    for (rot, v, s) in [("staff a&amp;b", "1", "a&amp;b"), ("voice a&lt;b", "a&lt;b", "1")] {
        let m = format!(
            "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>{v}</voice><type>quarter</type><staff>{s}</staff></note>\n\
             <note><rest/><duration>3</duration><voice>{v}</voice><type>half</type><dot/><staff>{s}</staff></note>\n"
        );
        let x = peca(&m);
        let r = std::panic::catch_unwind(|| {
            let mut d = DocumentoAberto::abrir_texto("t", &x).map_err(|e| e.to_string())?;
            let n = d.documento().notas().next().cloned().unwrap();
            let c = PosCompasso::nova("P1", 0);
            let r = d.aplicar(ComandoDeEdicao::InserirNota {
                compasso: c,
                voz: n.voz.clone(),
                pauta: n.pauta.clone(),
                posicao: Fracao::nova(1, 1),
                altura: Altura { passo: 'E', alter: 0, oitava: 4 },
                acidente: None,
                figura: piano_tutor_core::documento::figura_da_tecla(5).unwrap(),
                pontos: 0,
            });
            let t = d.exportar().map_err(|e| e.to_string())?;
            let reabre = DocumentoAberto::abrir_texto("t", &t).map(|_| ()).map_err(|e| e.to_string());
            let conf = d.conferir_exportacao(&t).map_err(|e| e.to_string());
            Ok::<_, String>((format!("{:?}", r.map(|r| r.alterou).map_err(|e| e.to_string())), reabre, conf, n.voz, n.pauta))
        });
        eprintln!("{rot} => {r:?}");
    }
}
