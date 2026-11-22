//! Registra a versão do rustc usada na compilação (variável `PT_RUSTC_VERSAO`).

use std::process::Command;

fn main() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let versao = Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .and_then(|saida| String::from_utf8(saida.stdout).ok())
        .map(|texto| texto.trim().to_string())
        .filter(|texto| !texto.is_empty())
        .unwrap_or_else(|| "desconhecida".to_string());
    println!("cargo:rustc-env=PT_RUSTC_VERSAO={versao}");
    println!("cargo:rerun-if-changed=build.rs");
}
