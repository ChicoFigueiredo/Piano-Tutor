//! Sondas temporárias da revisão independente (não entram no commit).
use piano_tutor_core::documento::{
    Altura, ComandoDeEdicao, DocumentoAberto, IdNota, SentidoDaAltura, TabelaIds, UnidadeDeAltura,
};
use piano_tutor_core::partitura::{Figura, Fracao};

fn peca(medida: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<score-partwise version=\"4.0\">\n  \
         <part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list>\n  \
         <part id=\"P1\">\n    <measure number=\"1\"><attributes><divisions>1</divisions>\
         <key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time>\
         </attributes>\n{medida}    </measure>\n  </part>\n</score-partwise>\n"
    )
}

fn nota(p: char, d: u32, t: &str) -> String {
    format!(
        "      <note><pitch><step>{p}</step><octave>4</octave></pitch><duration>{d}</duration>\
         <voice>1</voice><type>{t}</type></note>\n"
    )
}

fn base() -> String {
    peca(&format!(
        "{}{}{}{}",
        nota('C', 1, "quarter"),
        nota('D', 1, "quarter"),
        nota('E', 1, "quarter"),
        nota('F', 1, "quarter")
    ))
}

#[test]
fn sonda_ids_proximo_maximo() {
    let t = TabelaIds::de_texto("piano-tutor-ids v1\nproximo=18446744073709551615\n").expect("lê");
    let r = std::panic::catch_unwind(|| {
        DocumentoAberto::abrir_texto_com_ids("t", &base(), Some(&t)).map(|d| d.ids().total())
    });
    eprintln!("proximo=MAX => {:?}", r.as_ref().map(|x| x.as_ref().map_err(|e| e.to_string())));
}

#[test]
fn sonda_ids_duplicados() {
    // Duas chaves por posição com o mesmo IdNota.
    let lateral = "piano-tutor-ids v1\nproximo=10\n\
        pos\tP1\t0\t0\t1\npos\tP1\t0\t1\t1\npos\tP1\t0\t2\t1\npos\tP1\t0\t3\t1\n";
    let t = TabelaIds::de_texto(lateral).expect("lê");
    let r = std::panic::catch_unwind(|| {
        let mut d = DocumentoAberto::abrir_texto_com_ids("t", &base(), Some(&t)).expect("abre");
        eprintln!("notas no indice = {}", d.documento().total_de_notas());
        let id = IdNota(1);
        let r1 = d.aplicar(ComandoDeEdicao::MudarAltura {
            nota: id,
            unidade: UnidadeDeAltura::Semitom,
            sentido: SentidoDaAltura::Acima,
        });
        eprintln!("mudar altura id1: {:?}", r1.as_ref().map(|r| r.alterou).map_err(|e| e.to_string()));
        let r2 = d.aplicar(ComandoDeEdicao::RemoverNota { nota: id });
        eprintln!("remover id1: {:?}", r2.as_ref().map(|r| r.alterou).map_err(|e| e.to_string()));
        let _ = d.desfazer();
        let _ = d.desfazer();
        let r3 = d.exportar().map(|t| t.len());
        eprintln!("exportar: {:?}", r3.map_err(|e| e.to_string()));
    });
    eprintln!("duplicados => {:?}", r.is_ok());
}

#[test]
fn sonda_divisions_enorme() {
    let x = base().replace(
        "<divisions>1</divisions>",
        "<divisions>9223372036854775807</divisions>",
    );
    let r = std::panic::catch_unwind(|| DocumentoAberto::abrir_texto("t", &x).map(|_| ()));
    eprintln!("divisions MAX => ok? {:?}", r.map(|x| x.map_err(|e| e.to_string())));
    let x = base().replace("<duration>1</duration>", "<duration>9223372036854775807</duration>");
    let r = std::panic::catch_unwind(|| DocumentoAberto::abrir_texto("t", &x).map(|_| ()));
    eprintln!("duration MAX => {:?}", r.map(|x| x.map_err(|e| e.to_string())));
    let x = base().replace("<duration>1</duration>", "<duration>-5</duration>");
    let r = std::panic::catch_unwind(|| {
        let mut d = DocumentoAberto::abrir_texto("t", &x)?;
        let id = d.ids().id(0, 0, 0).unwrap();
        let _ = d.aplicar(ComandoDeEdicao::DefinirDuracao { nota: id, figura: Figura::Minima, pontos: 0 });
        d.exportar()
    });
    eprintln!("duration negativa => {:?}", r.map(|x| x.map(|s| s.len()).map_err(|e| e.to_string())));
}

#[test]
fn sonda_entidade_em_nota() {
    // Entidade interna no texto de <step> e <duration>.
    let x = base()
        .replacen("<score-partwise", "<!DOCTYPE score-partwise [<!ENTITY p \"C\"><!ENTITY d \"1\">]>\n<score-partwise", 1)
        .replacen("<step>C</step>", "<step>&p;</step>", 1)
        .replacen("<duration>1</duration>", "<duration>&d;</duration>", 1);
    let r = std::panic::catch_unwind(|| {
        let mut d = DocumentoAberto::abrir_texto("t", &x).map_err(|e| e.to_string())?;
        let id = d.ids().id(0, 0, 0).unwrap();
        let r = d.aplicar(ComandoDeEdicao::MudarAltura {
            nota: id,
            unidade: UnidadeDeAltura::Semitom,
            sentido: SentidoDaAltura::Acima,
        });
        eprintln!("entidade: edicao {:?}", r.map(|r| r.alterou).map_err(|e| e.to_string()));
        let t = d.exportar().map_err(|e| e.to_string())?;
        d.conferir_exportacao(&t).map_err(|e| e.to_string())?;
        Ok::<String, String>(t)
    });
    eprintln!("entidade => {:?}", r.map(|x| x.map(|t| t.contains("&p;"))));
}

#[test]
fn sonda_alongar_em_compasso_incompleto_e_insercao_dupla_voz() {
    // Voz 2 com backup; alongar nota da voz 1 não pode tomar pausa da voz 2.
    let m = format!(
        "{}{}<backup><duration>2</duration></backup>\
         <note><rest/><duration>2</duration><voice>2</voice><type>half</type></note>\n",
        nota('C', 1, "quarter"),
        "      <note><rest/><duration>1</duration><voice>1</voice><type>quarter</type></note>\n"
    );
    let x = peca(&m);
    let r = std::panic::catch_unwind(|| {
        let mut d = DocumentoAberto::abrir_texto("t", &x).map_err(|e| e.to_string())?;
        let id = d.ids().id(0, 0, 0).unwrap();
        let r = d.aplicar(ComandoDeEdicao::MudarDuracao { nota: id, figura: Figura::Minima });
        eprintln!("alongar: {:?}", r.map(|r| r.alterou).map_err(|e| e.to_string()));
        d.exportar().map_err(|e| e.to_string())
    });
    eprintln!("dupla voz => {:?}", r.map(|x| x.map(|t| t.len())));
    let _ = (Altura { passo: 'C', alter: 0, oitava: 4 }, Fracao::ZERO);
}
