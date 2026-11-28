//! Sondas temporárias da revisão independente (não entram no commit).
use piano_tutor_core::documento::{
    ComandoDeEdicao, DocumentoAberto, IdNota, SentidoDaAltura, UnidadeDeAltura,
};
use piano_tutor_core::partitura::Figura;

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

fn tentar(rotulo: &str, x: &str) {
    let x = x.to_owned();
    let r = std::panic::catch_unwind(move || {
        let mut d = match DocumentoAberto::abrir_texto("t", &x) {
            Ok(d) => d,
            Err(e) => return format!("recusa: {e}"),
        };
        let mut ids: Vec<IdNota> = Vec::new();
        for (p, parte) in d.partitura().partes.iter().enumerate() {
            for (c, comp) in parte.compassos.iter().enumerate() {
                for o in 0..comp.notas.len() {
                    if let Some(id) = d.ids().id(p, c, o) {
                        ids.push(id);
                    }
                }
            }
        }
        let mut n = 0;
        for id in ids {
            for cmd in [
                ComandoDeEdicao::MudarDuracao {
                    nota: id,
                    figura: Figura::Minima,
                },
                ComandoDeEdicao::MudarDuracao {
                    nota: id,
                    figura: Figura::Semifusa,
                },
                ComandoDeEdicao::AlternarPonto { nota: id },
                ComandoDeEdicao::RemoverNota { nota: id },
                ComandoDeEdicao::MudarAltura {
                    nota: id,
                    unidade: UnidadeDeAltura::Oitava,
                    sentido: SentidoDaAltura::Acima,
                },
            ] {
                if d.aplicar(cmd).is_ok() {
                    n += 1;
                }
            }
        }
        while d.desfazer().is_some() {}
        let e = d
            .exportar()
            .map(|t| t == d.texto_original())
            .unwrap_or(false);
        format!("ok edicoes={n} idem_apos_desfazer_tudo={e}")
    });
    match r {
        Ok(s) => eprintln!("{rotulo}: {s}"),
        Err(_) => eprintln!("{rotulo}: PANICO"),
    }
}

#[test]
fn sonda_numeros_grandes() {
    let b = base();
    for (r, de, para) in [
        (
            "dur=2^62",
            "<duration>1</duration>",
            "<duration>4611686018427387904</duration>",
        ),
        (
            "dur=2^40",
            "<duration>1</duration>",
            "<duration>1099511627776</duration>",
        ),
        (
            "div=2^40",
            "<divisions>1</divisions>",
            "<divisions>1099511627776</divisions>",
        ),
        (
            "div=1e9+7",
            "<divisions>1</divisions>",
            "<divisions>1000000007</divisions>",
        ),
        ("beats=1e9", "<beats>4</beats>", "<beats>1000000000</beats>"),
        ("beats=0", "<beats>4</beats>", "<beats>0</beats>"),
        (
            "beattype=0",
            "<beat-type>4</beat-type>",
            "<beat-type>0</beat-type>",
        ),
        ("fifths=100", "<fifths>0</fifths>", "<fifths>100</fifths>"),
        ("octave=9", "<octave>4</octave>", "<octave>9</octave>"),
        ("octave=-1", "<octave>4</octave>", "<octave>-1</octave>"),
        (
            "alter=2.5",
            "<step>C</step>",
            "<step>C</step><alter>2.5</alter>",
        ),
    ] {
        let x = b.replace(de, para);
        tentar(r, &x);
    }
    let x = b.replace(
        "<duration>1</duration>",
        "<duration>2305843009213693952</duration>",
    );
    tentar("todas dur=2^61", &x);
}
