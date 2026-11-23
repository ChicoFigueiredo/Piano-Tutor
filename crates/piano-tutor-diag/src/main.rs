//! Binário `piano-tutor-diag`: grava o diagnóstico em Markdown.
//!
//! Uso: `piano-tutor-diag [--saida <arquivo>] [--rotulo <texto>]`
//!
//! Medição de latência (F03-4, F02-4): `piano-tutor-diag latencia-listar|latencia-loopback|`
//! `latencia-midi|latencia-quadros --ajuda`.

use piano_tutor_diag::coleta::{
    TEMPO_LIMITE, coletar_sistema, com_limite, enumerar_audio, enumerar_midi, versao_rustc,
};
use piano_tutor_diag::medicao;
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

/// Subcomandos de latência: devolve `None` se o primeiro argumento não for um deles.
fn rodar_latencia() -> Option<ExitCode> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (sub, op) = match medicao::ler_comando(&args) {
        Ok(Some(c)) => c,
        Ok(None) => return None,
        Err(msg) => {
            eprintln!("{msg}");
            return Some(ExitCode::from(2));
        }
    };
    let texto = match medicao::executar(sub, &op) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("erro: {e}");
            return Some(ExitCode::FAILURE);
        }
    };
    Some(match &op.saida_md {
        Some(caminho) => match std::fs::write(caminho, texto) {
            Ok(()) => {
                eprintln!("relatório gravado em {caminho}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("falha ao gravar {caminho}: {e}");
                ExitCode::FAILURE
            }
        },
        None => {
            print!("{texto}");
            ExitCode::SUCCESS
        }
    })
}

fn main() -> ExitCode {
    if let Some(codigo) = rodar_latencia() {
        return codigo;
    }
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
