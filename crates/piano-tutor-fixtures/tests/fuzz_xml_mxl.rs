//! Teste de mutação (fuzz determinístico) do leitor de MusicXML e de MXL pelo serviço de
//! documento (ticket 11, A13). Sem dependência nova: um gerador pseudoaleatório xorshift com
//! semente fixa, para que uma falha seja reproduzível.
//!
//! Propriedades verificadas em cada entrada mutada:
//! 1. nunca há pânico (o teste de unidade falha e a entrada é gravada para virar regressão);
//! 2. cada abertura termina em tempo razoável (teto por entrada);
//! 3. se abre, a exportação sem edição devolve **exatamente** o texto de origem (critério A1).
//!
//! Orçamento: na CI roda por tempo limitado (padrão 15 s); localmente aumente com variáveis de
//! ambiente, por exemplo
//! `PIANO_TUTOR_FUZZ_SEGUNDOS=600 cargo test --release -p piano-tutor-fixtures --test fuzz_xml_mxl`.
//!
//! - `PIANO_TUTOR_FUZZ_SEGUNDOS`: tempo total (padrão 15);
//! - `PIANO_TUTOR_FUZZ_SEMENTE`: semente (padrão fixa);
//! - `PIANO_TUTOR_FUZZ_ITERACOES`: máximo de mutações por semente de entrada (padrão 400).
//!
//! Entrada que falhar vai para `target/tmp/fuzz-falhas/` (caminho impresso) e deve ser copiada
//! para `REGRESSOES` em `regressoes_xml_mxl.rs`, que a reabre em todo `cargo test`.

use piano_tutor_core::documento::{DocumentoAberto, Formato};
use piano_tutor_core::mxl::empacotar_mxl;
use piano_tutor_fixtures::corpus::fixture;
use piano_tutor_fixtures::gerador::{mxl_de_teste, xml_de_teste};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

/// Teto de tempo de uma única abertura (build de depuração, entradas pequenas).
const TETO_POR_ENTRADA: Duration = Duration::from_secs(10);

struct Xorshift(u64);

impl Xorshift {
    fn proximo(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn ate(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.proximo() % n as u64) as usize
        }
    }
}

const FICHAS: [&[u8]; 20] = [
    b"<!DOCTYPE x [<!ENTITY a \"&b;&b;\"><!ENTITY b \"&a;\">]>",
    b"<!ENTITY x SYSTEM \"file:///etc/passwd\">",
    b"&x;",
    b"&amp;",
    b"&#0;",
    b"&#x110000;",
    b"<![CDATA[",
    b"]]>",
    b"<!--",
    b"-->",
    b"<?xml version=\"1.0\" encoding=\"UTF-16\"?>",
    b"<measure>",
    b"</measure>",
    b"<note>",
    b"<divisions>0</divisions>",
    b"<duration>99999999999999999999</duration>",
    b"<score-partwise>",
    b"\0",
    b"PK\x03\x04",
    b"../",
];

fn mutar(r: &mut Xorshift, entrada: &[u8]) -> Vec<u8> {
    let mut v = entrada.to_vec();
    for _ in 0..=r.ate(3) {
        if v.is_empty() {
            v.push(b'<');
        }
        let n = v.len();
        match r.ate(8) {
            0 => {
                let i = r.ate(n);
                v[i] ^= 1 << r.ate(8);
            }
            1 => {
                let i = r.ate(n);
                v[i] = [0, 0xFF, b'<', b'>', b'&', b'"', b']'][r.ate(7)];
            }
            2 => {
                let i = r.ate(n);
                let f = (i + 1 + r.ate(64)).min(n);
                v.drain(i..f);
            }
            3 => {
                let i = r.ate(n);
                let f = (i + 1 + r.ate(64)).min(n);
                let trecho = v[i..f].to_vec();
                let onde = r.ate(n);
                for (k, b) in trecho.iter().enumerate() {
                    v.insert(onde + k, *b);
                }
            }
            4 => {
                let ficha = FICHAS[r.ate(FICHAS.len())];
                let onde = r.ate(n + 1);
                for (k, b) in ficha.iter().enumerate() {
                    v.insert(onde + k, *b);
                }
            }
            5 => v.truncate(r.ate(n)),
            6 => {
                let (a, b) = (r.ate(n), r.ate(n));
                v.swap(a, b);
            }
            _ => {
                // Repete um trecho muitas vezes (aninhamento e expansão).
                let i = r.ate(n);
                let f = (i + 1 + r.ate(16)).min(n);
                let trecho = v[i..f].to_vec();
                let vezes = 1 + r.ate(200);
                let mut novo = v[..f].to_vec();
                for _ in 0..vezes {
                    novo.extend_from_slice(&trecho);
                }
                novo.extend_from_slice(&v[f..]);
                v = novo;
            }
        }
        if v.len() > 400_000 {
            v.truncate(400_000);
        }
    }
    v
}

fn gravar_falha(bytes: &[u8], rotulo: &str) -> String {
    let pasta = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("fuzz-falhas");
    let _ = std::fs::create_dir_all(&pasta);
    let soma: u64 = bytes.iter().fold(1469598103934665603_u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(1099511628211)
    });
    let arq = pasta.join(format!("{rotulo}-{soma:016x}.bin"));
    let _ = std::fs::write(&arq, bytes);
    arq.display().to_string()
}

/// Abre e confere as propriedades; devolve a descrição da violação, se houver.
fn conferir(bytes: &[u8], formato: Formato) -> Option<String> {
    let t = Instant::now();
    let r = catch_unwind(AssertUnwindSafe(|| {
        DocumentoAberto::abrir_bytes("fuzz", bytes, formato).map(|d| {
            let saida = d.exportar();
            (d.texto_original().to_owned(), saida)
        })
    }));
    if t.elapsed() > TETO_POR_ENTRADA {
        return Some(format!("demorou {:?}", t.elapsed()));
    }
    match r {
        Err(_) => Some("pânico".into()),
        Ok(Err(_)) => None, // recusa tipada: ok
        Ok(Ok((original, saida))) => match saida {
            Ok(s) if s == original => None,
            Ok(_) => Some("exportação sem edição difere do original".into()),
            Err(e) => Some(format!("abriu mas não exportou: {e}")),
        },
    }
}

fn sementes() -> Vec<(&'static str, Vec<u8>, Formato)> {
    let mut v: Vec<(&'static str, Vec<u8>, Formato)> = Vec::new();
    for id in ["duas-vozes", "anacruse"] {
        // Não depende de o id existir: o corpus tem estes dois (ver `corpus.rs`); se faltar, falha alto.
        let f = fixture(id);
        v.push((id, f.conteudo.clone().into_bytes(), Formato::MusicXml));
        let mxl = empacotar_mxl("score.xml", &f.conteudo).expect("zip");
        v.push(("mxl-do-corpus", mxl, Formato::Mxl));
    }
    for c in xml_de_teste() {
        if c.texto.len() < 200_000 {
            v.push((c.nome, c.texto.into_bytes(), Formato::MusicXml));
        }
    }
    for m in mxl_de_teste() {
        if m.bytes.len() < 200_000 {
            v.push((m.nome, m.bytes, Formato::Mxl));
        }
    }
    v
}

fn numero_do_ambiente(nome: &str, padrao: u64) -> u64 {
    std::env::var(nome)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(padrao)
}

#[test]
fn mutacoes_do_leitor_de_musicxml_e_mxl_nunca_panicam_nem_quebram_o_a1() {
    let explicito = std::env::var("PIANO_TUTOR_FUZZ_SEGUNDOS").is_ok();
    let segundos = numero_do_ambiente("PIANO_TUTOR_FUZZ_SEGUNDOS", 15);
    let por_semente = numero_do_ambiente("PIANO_TUTOR_FUZZ_ITERACOES", 400);
    let mut r = Xorshift(numero_do_ambiente(
        "PIANO_TUTOR_FUZZ_SEMENTE",
        0x9E37_79B9_7F4A_7C15,
    ));
    let inicio = Instant::now();
    let orcamento = Duration::from_secs(segundos);
    let sementes = sementes();
    let mut total = 0_u64;
    let mut abriu = 0_u64;
    let mut falhas: Vec<String> = Vec::new();
    // Rodadas até acabar o orçamento: cada rodada percorre todas as sementes.
    'tempo: loop {
        for (nome, base, formato) in &sementes {
            for _ in 0..por_semente {
                if inicio.elapsed() > orcamento {
                    break 'tempo;
                }
                let m = mutar(&mut r, base);
                total += 1;
                // Em `.mxl`, além de mutar o zip (quase sempre quebra o CRC), muta o XML de dentro
                // e reempacota, para alcançar o leitor de MusicXML por esta rota.
                let casos: Vec<(Vec<u8>, Formato)> = if *formato == Formato::Mxl {
                    let mut c = vec![(m.clone(), Formato::Mxl)];
                    if let Ok(xml) = piano_tutor_core::mxl::extrair_musicxml(base) {
                        let mx = mutar(&mut r, xml.as_bytes());
                        let mx = String::from_utf8_lossy(&mx).into_owned();
                        if let Ok(z) = empacotar_mxl("score.xml", &mx) {
                            c.push((z, Formato::Mxl));
                        }
                    }
                    c
                } else {
                    vec![(m, Formato::MusicXml)]
                };
                for (bytes, f) in casos {
                    if let Some(motivo) = conferir(&bytes, f) {
                        let onde = gravar_falha(&bytes, nome);
                        falhas.push(format!("{nome}: {motivo} (entrada em {onde})"));
                    } else if DocumentoAberto::abrir_bytes("f", &bytes, f).is_ok() {
                        abriu += 1;
                    }
                }
                if falhas.len() >= 5 {
                    break 'tempo;
                }
            }
        }
        // Sem orçamento explícito (CI), uma rodada completa basta; com ele, roda até o tempo acabar.
        if inicio.elapsed() > orcamento || !explicito {
            break;
        }
    }
    eprintln!(
        "fuzz: {total} mutações em {:?}, {abriu} ainda abriram, {} falhas",
        inicio.elapsed(),
        falhas.len()
    );
    assert!(falhas.is_empty(), "{}", falhas.join("\n"));
    assert!(
        total > 100,
        "o orçamento de tempo foi curto demais: {total}"
    );
}
