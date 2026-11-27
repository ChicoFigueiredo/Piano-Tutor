//! Mede tempo e pico de memória (VmHWM, Linux) da abertura pelo serviço de documento no pior
//! caso de cada limite (ticket 11). Cada caso roda num processo próprio, para o pico ser dele.
//!
//! `cargo run --release -p piano-tutor-fixtures --example medir_seguranca`
//! (um caso isolado: `... --example medir_seguranca -- <nome>`).

use piano_tutor_core::documento::{DocumentoAberto, Formato};
use piano_tutor_core::mxl::empacotar_mxl;
use piano_tutor_fixtures::gerador::{mxl_de_teste, xml_de_teste};
use std::time::Instant;

fn pico_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
        })
        .unwrap_or(0)
}

/// Compassos de pausas até quase os limites de notas (199 000) e de tamanho.
fn partitura_no_limite() -> String {
    let mut s = String::from(
        "<score-partwise version=\"4.0\"><part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list><part id=\"P1\">",
    );
    for m in 0..1990 {
        s.push_str(&format!("<measure number=\"{}\">", m + 1));
        if m == 0 {
            s.push_str("<attributes><divisions>1</divisions></attributes>");
        }
        for _ in 0..100 {
            s.push_str("<note><rest/></note>");
        }
        s.push_str("</measure>");
    }
    s.push_str("</part></score-partwise>");
    s
}

fn casos() -> Vec<(String, Vec<u8>, Formato)> {
    let mut v: Vec<(String, Vec<u8>, Formato)> = Vec::new();
    let ok = partitura_no_limite();
    v.push((
        "maior-partitura-valida".into(),
        ok.clone().into_bytes(),
        Formato::MusicXml,
    ));
    v.push((
        "mesma-dentro-de-mxl".into(),
        empacotar_mxl("s.xml", &ok).expect("zip"),
        Formato::Mxl,
    ));
    // Quase 2 milhões de nós (o máximo aceito antes de recusar).
    let nos = format!(
        "<score-partwise>{}</score-partwise>",
        "<a/>".repeat(1_990_000)
    );
    v.push((
        "quase-2-milhoes-de-nos".into(),
        nos.into_bytes(),
        Formato::MusicXml,
    ));
    for c in xml_de_teste() {
        v.push((
            format!("xml/{}", c.nome),
            c.texto.into_bytes(),
            Formato::MusicXml,
        ));
    }
    for m in mxl_de_teste() {
        v.push((format!("mxl/{}", m.nome), m.bytes, Formato::Mxl));
    }
    v.push((
        "mxl/16-MiB-de-lixo".into(),
        vec![0x50; 16 * 1024 * 1024],
        Formato::Mxl,
    ));
    v
}

fn main() {
    let todos = casos();
    if let Some(nome) = std::env::args().nth(1) {
        let (_, bytes, formato) = todos
            .into_iter()
            .find(|(n, _, _)| *n == nome)
            .expect("caso desconhecido");
        let base = pico_kib();
        let t = Instant::now();
        let r = DocumentoAberto::abrir_bytes("medida", &bytes, formato);
        let dt = t.elapsed();
        let resumo = match &r {
            Ok(d) => format!("abriu ({} notas)", d.ids().total()),
            Err(e) => format!("recusou: {e}"),
        };
        println!(
            "{:>9.1} ms | pico {:>7} KiB (com a entrada: {:>7} KiB) | {resumo}",
            dt.as_secs_f64() * 1000.0,
            pico_kib(),
            base
        );
        return;
    }
    let exe = std::env::current_exe().expect("exe");
    for (nome, _, _) in &todos {
        let saida = std::process::Command::new(&exe)
            .arg(nome)
            .output()
            .expect("filho");
        print!("{nome:<44} {}", String::from_utf8_lossy(&saida.stdout));
        if !saida.status.success() {
            println!("   FALHOU: {}", String::from_utf8_lossy(&saida.stderr));
        }
    }
}
