//! Binário `piano-tutor-diag`: grava o diagnóstico em Markdown.
//!
//! Uso: `piano-tutor-diag [--saida <arquivo>] [--rotulo <texto>]`

use piano_tutor_diag::coleta::{
    TEMPO_LIMITE, coletar_sistema, com_limite, enumerar_audio, enumerar_midi, versao_rustc,
};
use piano_tutor_diag::relatorio::{Diagnostico, montar_relatorio};
use std::process::ExitCode;

struct Argumentos {
    saida: Option<String>,
    rotulo: Option<String>,
}

fn ler_argumentos() -> Result<Argumentos, String> {
    let mut args = std::env::args().skip(1);
    let mut r = Argumentos {
        saida: None,
        rotulo: None,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--saida" => r.saida = Some(args.next().ok_or("--saida exige um caminho")?),
            "--rotulo" => r.rotulo = Some(args.next().ok_or("--rotulo exige um texto")?),
            "--ajuda" | "-h" => {
                return Err("uso: piano-tutor-diag [--saida <arquivo>] [--rotulo <texto>]".into());
            }
            outro => return Err(format!("argumento desconhecido: {outro}")),
        }
    }
    Ok(r)
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    let args = match ler_argumentos() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };

    tracing::info!("enumerando MIDI e áudio (limite de {TEMPO_LIMITE:?} cada)");
    let midi = com_limite("midi", TEMPO_LIMITE, enumerar_midi);
    let audio = com_limite("audio", TEMPO_LIMITE, enumerar_audio);
    let diagnostico = Diagnostico {
        sistema: coletar_sistema(),
        rustc: versao_rustc(),
        midi,
        audio,
        rotulo_ambiente: args.rotulo,
    };
    let md = montar_relatorio(&diagnostico);

    match args.saida {
        Some(caminho) => match std::fs::write(&caminho, md) {
            Ok(()) => {
                tracing::info!("relatório gravado em {caminho}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                tracing::error!("falha ao gravar {caminho}: {e}");
                ExitCode::FAILURE
            }
        },
        None => {
            print!("{md}");
            ExitCode::SUCCESS
        }
    }
}
