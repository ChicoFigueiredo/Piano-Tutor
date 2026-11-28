//! SONDA TEMPORARIA (terceira revisao E01): fuzz de grafias hostis por todos os comandos publicos.
use piano_tutor_core::documento::{
    Altura, ComandoDeEdicao, DocumentoAberto, PosCompasso, figura_da_tecla,
};
use piano_tutor_core::partitura::Fracao;

fn peca_com(prefixo: &str, notas: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{prefixo}<score-partwise version=\"4.0\">\
         <part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list>\
         <part id=\"P1\"><measure number=\"1\"><attributes><divisions>4</divisions>\
         <time><beats>4</beats><beat-type>4</beat-type></time></attributes>{notas}</measure>\
         </part></score-partwise>\n"
    )
}

fn nota(step: char, alter: Option<i8>, acc: Option<&str>) -> String {
    let a = alter.map(|a| format!("<alter>{a}</alter>")).unwrap_or_default();
    let ac = acc.unwrap_or("");
    format!(
        "<note><pitch><step>{step}</step>{a}<octave>4</octave></pitch><duration>4</duration>\
         <voice>1</voice><type>quarter</type>{ac}</note>"
    )
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
        v[(self.next() as usize) % v.len()]
    }
}

const TOKENS: &[&str] = &[
    "sharp", "flat", "natural", "double-sharp", " ", "  ", "\n", "\t", "\"", "'", ">", "<", "/", "=", "&amp;", "&lt;", "&#", "&#0;",
    "&#x41;", "]]>", "-->", "--", "<!--", "\u{1F}", "cautionary", "parentheses", "smufl", "id", "color", "font-family", "=\"yes\"",
    "='no'", "xmlns", "xmlns:x", "x:y", "<injetado/>", "</accidental>", "<accidental>", "\u{a0}", "\u{2028}", "\u{85}", "\u{fffe}", "\0",
];

fn gerar(r: &mut Lcg) -> String {
    let n = 1 + (r.next() % 7) as usize;
    let mut s = String::new();
    // metade comeca por um nome valido
    if r.next() % 2 == 0 {
        s.push_str(r.pick(&["sharp", "flat", "natural", "double-sharp"]));
        if r.next() % 2 == 0 {
            s.push(' ');
        }
    }
    for _ in 0..n {
        s.push_str(r.pick(TOKENS));
    }
    s
}

fn regiao_accidental(t: &str) -> Option<&str> {
    let i = t.find("<accidental")?;
    let f = t[i..].find("</accidental>")? + i + "</accidental>".len();
    Some(&t[i..f])
}

#[test]
fn zz3_fuzz_grafia_hostil() {
    let prefixos = [
        String::new(),
        "<!-- <accidental x=\"1\">sharp</accidental> <accidental>flat]]></accidental> <accidental a:b=\"1\">natural</accidental> -->\n".to_owned(),
    ];
    let base_notas = format!(
        "{}{}{}{}",
        nota('C', None, None),
        nota('F', None, None),
        nota('E', None, None),
        nota('G', None, None)
    );
    for (pi, prefixo) in prefixos.iter().enumerate() {
        let texto = peca_com(prefixo, &base_notas);
        let mut r = Lcg(0x1234_5678 + pi as u64);
        let (mut aceitos, mut quebrados, mut elementos_novos) = (0u32, 0u32, 0u32);
        let mut amostras: Vec<String> = Vec::new();
        for it in 0..60_000u32 {
            let g = gerar(&mut r);
            let mut d = DocumentoAberto::abrir_texto("x", &texto).unwrap();
            let id1 = d.ids().id(0, 0, 1).unwrap();
            let id0 = d.ids().id(0, 0, 0).unwrap();
            let (alter, passo) = match g.split_whitespace().next() {
                Some("sharp") => (1, 'F'),
                Some("flat") => (-1, 'F'),
                Some("double-sharp") => (2, 'F'),
                _ => (0, 'F'),
            };
            let cmd = match it % 4 {
                0 => ComandoDeEdicao::DefinirAltura { nota: id1, altura: Altura { passo, alter, oitava: 4 }, acidente: Some(g.clone()) },
                1 => ComandoDeEdicao::AdicionarAltura { nota: id0, altura: Altura { passo: 'G', alter, oitava: 4 }, acidente: Some(g.clone()) },
                2 => ComandoDeEdicao::RestaurarAltura { nota: id0, altura: Altura { passo: 'G', alter, oitava: 4 }, acidente: Some(g.clone()) },
                _ => ComandoDeEdicao::InserirNota {
                    compasso: PosCompasso::nova("P1", 0),
                    voz: "1".into(),
                    pauta: "1".into(),
                    posicao: Fracao::nova(0, 1),
                    altura: Altura { passo: 'G', alter, oitava: 4 },
                    acidente: Some(g.clone()),
                    figura: figura_da_tecla(5).unwrap(),
                    pontos: 0,
                },
            };
            let eh_inserir = it % 4 == 3;
            if d.aplicar(cmd).is_err() {
                continue;
            }
            aceitos += 1;
            let t = d.exportar().unwrap();
            let reabre = DocumentoAberto::abrir_texto("x", &t);
            // numero de elementos <accidental> abertos: 1 (so o novo) ou o das notas
            let n_acc = t.matches("<accidental").count() - prefixo.matches("<accidental").count();
            let reg = regiao_accidental(&t).unwrap_or("");
            let novos = reg.matches('<').count() != 2 || n_acc != 1 || t.contains("<injetado");
            if reabre.is_err() || novos {
                if reabre.is_err() {
                    quebrados += 1;
                }
                if novos {
                    elementos_novos += 1;
                }
                if amostras.len() < 12 {
                    amostras.push(format!("tipo={} g={g:?} reabre={:?} regiao={reg:?} n_acc={n_acc}", it % 4, reabre.as_ref().map(|_| ()).map_err(|e| e.to_string())));
                }
            }
            let _ = eh_inserir;
        }
        println!("SONDA3 fuzz prefixo#{pi}: aceitos={aceitos} nao_reabre={quebrados} estrutura_estranha={elementos_novos}");
        for a in amostras {
            println!("SONDA3 fuzz   amostra: {a}");
        }
    }
}

#[test]
fn zz3_exoticos_remover_e_inverso() {
    let accs = [
        "<accidental print-object=\"no\">sharp</accidental>",
        "<accidental\r\n cautionary=\"yes\"\t>sharp</accidental>",
        "<accidental   xml:lang=\"pt\">  sharp </accidental>",
        "<accidental default-x=\"-12.5\" font-family=\"a>b\">sharp</accidental>",
        "<accidental/>",
    ];
    for acc in accs {
        for chord in [false, true] {
            let notas = if chord {
                format!(
                    "<note><pitch><step>F</step><alter>1</alter><octave>4</octave></pitch><duration>16</duration><voice>1</voice><type>whole</type>{acc}</note><note><chord/><pitch><step>A</step><octave>4</octave></pitch><duration>16</duration><voice>1</voice><type>whole</type></note>"
                )
            } else {
                format!(
                    "<note><pitch><step>F</step><alter>1</alter><octave>4</octave></pitch><duration>16</duration><voice>1</voice><type>whole</type>{acc}</note>"
                )
            };
            let texto = peca_com("", &notas);
            let mut d = match DocumentoAberto::abrir_texto("x", &texto) {
                Ok(d) => d,
                Err(e) => {
                    println!("SONDA3 exo nem abre {acc:?}: {e}");
                    continue;
                }
            };
            let id = d.ids().id(0, 0, 0).unwrap();
            let antes = d.exportar().unwrap();
            let r = d.aplicar(ComandoDeEdicao::RemoverNota { nota: id });
            let r = match r {
                Ok(r) => r,
                Err(e) => {
                    println!("SONDA3 exo remover recusado chord={chord} {acc:?}: {e}");
                    continue;
                }
            };
            let inv = d.aplicar(r.inverso.clone());
            let ok_inv = inv.is_ok();
            let igual = d.exportar().unwrap() == antes;
            let reabre = DocumentoAberto::abrir_texto("x", &d.exportar().unwrap()).is_ok();
            let mut desf = 0;
            while d.desfazer().is_some() {
                desf += 1;
            }
            println!(
                "SONDA3 exo chord={chord} {:?}: inverso_aceito={ok_inv} {} texto_igual={igual} reabre={reabre} desfazer({desf}) volta_ao_original={}",
                acc.chars().take(50).collect::<String>(),
                inv.as_ref().err().map(|e| e.to_string()).unwrap_or_default(),
                d.exportar().unwrap() == antes
            );
        }
    }
}

#[test]
fn zz3_restaurar_em_pausa_e_pontos_grandes() {
    // compasso: semibreve de pausa + outro compasso com notas
    let texto = peca_com("", "<note><rest/><duration>16</duration><voice>1</voice><type>whole</type></note>");
    let mut d = DocumentoAberto::abrir_texto("x", &texto).unwrap();
    let rest = d.ids().id(0, 0, 0).unwrap();
    for (rot, cmd) in [
        ("RestaurarAltura em pausa", ComandoDeEdicao::RestaurarAltura { nota: rest, altura: Altura { passo: 'C', alter: 0, oitava: 4 }, acidente: None }),
        ("AdicionarAltura em pausa", ComandoDeEdicao::AdicionarAltura { nota: rest, altura: Altura { passo: 'C', alter: 0, oitava: 4 }, acidente: None }),
    ] {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| d.aplicar(cmd).map(|r| r.alterou).map_err(|e| e.to_string())));
        println!("SONDA3 {rot}: {r:?}");
    }
    for pontos in [0u8, 3, 4, 5, 63, 64, 255] {
        let mut d = DocumentoAberto::abrir_texto("x", &texto).unwrap();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            d.aplicar(ComandoDeEdicao::InserirNota {
                compasso: PosCompasso::nova("P1", 0),
                voz: "1".into(),
                pauta: "1".into(),
                posicao: Fracao::nova(0, 1),
                altura: Altura { passo: 'C', alter: 0, oitava: 4 },
                acidente: None,
                figura: figura_da_tecla(5).unwrap(),
                pontos,
            })
            .map(|r| r.alterou)
            .map_err(|e| e.to_string())
        }));
        println!("SONDA3 InserirNota pontos={pontos}: {r:?}");
        let r2 = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut d = DocumentoAberto::abrir_texto("x", &texto).unwrap();
            d.aplicar(ComandoDeEdicao::InserirPausa {
                compasso: PosCompasso::nova("P1", 0),
                voz: "1".into(),
                pauta: "1".into(),
                posicao: Fracao::nova(0, 1),
                figura: figura_da_tecla(5).unwrap(),
                pontos,
            })
            .map(|r| r.alterou)
            .map_err(|e| e.to_string())
        }));
        println!("SONDA3 InserirPausa pontos={pontos}: {r2:?}");
    }
}
