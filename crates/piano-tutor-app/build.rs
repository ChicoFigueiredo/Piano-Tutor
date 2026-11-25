//! Embute as Lições de fábrica (`assets/licoes/**/*.toml`) no binário (ADR-0009).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn coletar(dir: &Path, saida: &mut Vec<PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entradas.flatten() {
        let p = e.path();
        if p.is_dir() {
            coletar(&p, saida);
        } else if p.extension().is_some_and(|x| x == "toml") {
            saida.push(p);
        }
    }
}

fn main() {
    let manifesto = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let raiz = manifesto.join("../../assets/licoes");
    println!("cargo:rerun-if-changed={}", raiz.display());
    let mut arquivos = Vec::new();
    coletar(&raiz, &mut arquivos);
    arquivos.sort();
    let mut s = String::from(
        "/// Lições de fábrica embutidas: (caminho relativo, texto TOML).\n\
         pub static LICOES_EMBUTIDAS: &[(&str, &str)] = &[\n",
    );
    for a in &arquivos {
        println!("cargo:rerun-if-changed={}", a.display());
        let rel = a
            .strip_prefix(&raiz)
            .map_or_else(|_| a.display().to_string(), |r| r.display().to_string());
        let abs = a.canonicalize().unwrap_or_else(|_| a.clone());
        let _ = writeln!(
            s,
            "    ({rel:?}, include_str!({:?})),",
            abs.display().to_string()
        );
    }
    s.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("licoes_embutidas.rs");
    std::fs::write(out, s).expect("gravar licoes_embutidas.rs");
}
