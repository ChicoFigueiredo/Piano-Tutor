//! CLI `piano-tutor-curso`: gera chaves, empacota, verifica, instala e lista Pacotes de Curso.

use std::io::Write;
use std::process::ExitCode;

use piano_tutor_curso::cli::{ErroCli, executar};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut saida = std::io::stdout().lock();
    let resultado = executar(&args, &mut saida);
    let _ = saida.flush();
    match resultado {
        Ok(()) => ExitCode::SUCCESS,
        Err(ErroCli::Uso(m)) => {
            eprintln!("erro: {m}\n\n{}", piano_tutor_curso::cli::AJUDA);
            ExitCode::from(2)
        }
        Err(ErroCli::Falha(m)) => {
            eprintln!("erro: {m}");
            ExitCode::from(1)
        }
    }
}
