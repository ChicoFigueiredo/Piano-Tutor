//! Testes da CLI: fluxo completo chave-gerar → empacotar → verificar → instalar → listar.
#![allow(clippy::unwrap_used)] // testes: falhar com panic é o comportamento desejado

mod comum;

use std::process::Command;

use piano_tutor_curso::cli::{ErroCli, executar};

fn rodar(args: &[&str]) -> Result<String, ErroCli> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut saida = Vec::new();
    executar(&args, &mut saida)?;
    Ok(String::from_utf8(saida).unwrap())
}

#[test]
fn fluxo_completo_da_cli() {
    let tmp = tempfile::tempdir().unwrap();
    let raiz = tmp.path();
    let origem = raiz.join("origem");
    comum::escrever_pasta_origem(&origem);
    let chaves = raiz.join("chaves");
    let pacote = raiz.join("curso.ptcurso");
    let dados = raiz.join("dados");
    let s = |p: &std::path::Path| p.to_str().unwrap().to_string();

    let saida = rodar(&["chave-gerar", &s(&chaves)]).unwrap();
    assert!(saida.contains("curso.chave"), "{saida}");
    assert!(chaves.join("curso.pub").is_file());

    let saida = rodar(&[
        "empacotar",
        &s(&origem),
        "--chave",
        &s(&chaves.join("curso.chave")),
        "-o",
        &s(&pacote),
    ])
    .unwrap();
    assert!(saida.contains("pacote gravado"), "{saida}");

    // Sem a chave nas confiáveis.
    let confiaveis = raiz.join("confiaveis");
    std::fs::create_dir_all(&confiaveis).unwrap();
    let saida = rodar(&["verificar", &s(&pacote), "--confiaveis", &s(&confiaveis)]).unwrap();
    assert!(saida.contains("Autor:       Autor Sintético"), "{saida}");
    assert!(saida.contains("Licenciado:  Aluno de Teste"), "{saida}");
    assert!(saida.contains("desconhecida"), "{saida}");
    assert!(
        saida.contains("Seções: 1  Lições: 2  Exercícios: 2"),
        "{saida}"
    );
    assert!(saida.contains("aprovada    1"), "{saida}");
    assert!(saida.contains("hipótese    1"), "{saida}");

    // Com a chave confiável.
    std::fs::copy(chaves.join("curso.pub"), confiaveis.join("autor.pub")).unwrap();
    let saida = rodar(&["verificar", &s(&pacote), "--confiaveis", &s(&confiaveis)]).unwrap();
    assert!(saida.contains("Confiança:   confiável"), "{saida}");

    let saida = rodar(&["listar", "--dados", &s(&dados)]).unwrap();
    assert!(saida.contains("nenhum curso instalado"), "{saida}");

    let saida = rodar(&["instalar", &s(&pacote), "--dados", &s(&dados)]).unwrap();
    assert!(saida.contains("instalado:"), "{saida}");
    assert!(
        saida.contains("aviso:"),
        "chave ainda não confiável: {saida}"
    );

    let saida = rodar(&["listar", "--dados", &s(&dados)]).unwrap();
    assert!(saida.contains("teste.sintetico"), "{saida}");
    assert!(saida.contains("2 exercícios (1 aprovados)"), "{saida}");
}

#[test]
fn erros_de_uso() {
    assert!(matches!(rodar(&[]), Err(ErroCli::Uso(_))));
    assert!(matches!(rodar(&["voar"]), Err(ErroCli::Uso(_))));
    assert!(matches!(rodar(&["chave-gerar"]), Err(ErroCli::Uso(_))));
    assert!(matches!(
        rodar(&["empacotar", "pasta"]),
        Err(ErroCli::Uso(_))
    ));
    assert!(matches!(
        rodar(&["verificar", "a.ptcurso", "--xis", "1"]),
        Err(ErroCli::Uso(_))
    ));
    assert!(matches!(
        rodar(&["listar", "--dados"]),
        Err(ErroCli::Uso(_))
    ));
    assert!(rodar(&["--help"]).unwrap().contains("chave-gerar"));
}

#[test]
fn verificar_pacote_adulterado_falha() {
    let tmp = tempfile::tempdir().unwrap();
    let chave = piano_tutor_curso::ChavePrivada::gerar();
    let mut e = comum::entradas(&comum::pacote_sintetico(&chave));
    comum::trocar(&mut e, "audio/tres-notas.opus", b"OggS-adulterad".to_vec());
    let arquivo = tmp.path().join("ruim.ptcurso");
    std::fs::write(&arquivo, comum::montar(&e)).unwrap();
    let erro = rodar(&["verificar", arquivo.to_str().unwrap()]).unwrap_err();
    assert!(matches!(erro, ErroCli::Falha(_)), "{erro:?}");
}

#[test]
fn binario_usa_codigos_de_saida() {
    let exe = env!("CARGO_BIN_EXE_piano-tutor-curso");
    let uso = Command::new(exe).arg("voar").output().unwrap();
    assert_eq!(uso.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&uso.stderr).contains("comando desconhecido"));
    let falha = Command::new(exe)
        .args(["verificar", "/nao/existe.ptcurso"])
        .output()
        .unwrap();
    assert_eq!(falha.status.code(), Some(1));
    let ok = Command::new(exe).arg("--help").output().unwrap();
    assert_eq!(ok.status.code(), Some(0));
}
