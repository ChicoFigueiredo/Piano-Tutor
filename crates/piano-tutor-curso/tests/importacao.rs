//! Testes do importador (`importar`): plano e oficina sintéticos → pasta-origem → pacote.
//! Nada aqui vem de curso real: as fixtures são criadas em pastas temporárias.
#![allow(clippy::unwrap_used)] // testes: falhar com panic é o comportamento desejado

mod comum;

use std::fs;
use std::path::{Path, PathBuf};

use comum::{ALINHAMENTO, MUSICXML, ROTEIRO, sha256_hex};
use piano_tutor_curso::cli::{ErroCli, executar};
use piano_tutor_curso::{
    ChavePrivada, ErroPacote, EstadoConteudo, TrechoAula, empacotar, importar, ler_pacote,
};

/// Plano sintético: 1 Seção, 3 Lições (2 com Exercícios, 1 de vídeo).
fn plano(sha: &str) -> String {
    format!(
        r#"formato_versao = 1
id = "teste.importado"
titulo = "Curso Importado de Teste"
versao = "0.1.0"
idioma = "pt-BR"
convencao_oitava = "do3-central"

[autor]
nome = "Autor Sintético"

[licenciado]
nome = "Aluno de Teste"
licenca = "Somente para testes automatizados."

[[secoes]]
id = "s01"
numero = 1
titulo = "Seção de teste"

[[secoes.licoes]]
id = "s01-a01"
numero = 1
titulo = "Três notas"
tipo = "exercicio"

[secoes.licoes.aula]
nome_arquivo = "01 - Três notas.mp4"
duracao_s = 60.0
sha256 = "{sha}"

[[secoes.licoes.exercicios]]
id = "tres-notas"
titulo = "Três notas"
modo = "praticar"
mao = "direita"

[[secoes.licoes.exercicios]]
id = "tres-notas-volta"
titulo = "Três notas, de volta"
modo = "chamada_e_resposta"
mao = "ambas"

[[secoes.licoes]]
id = "s01-a02"
numero = 2
titulo = "Só vídeo"
tipo = "assistir"

[[secoes.licoes]]
id = "s01-a03"
numero = 3
titulo = "Sem estado"
tipo = "exercicio"

[[secoes.licoes.exercicios]]
id = "sem-estado"
titulo = "Sem estado"
modo = "praticar"
mao = "direita"
"#
    )
}

/// Grava o plano e a oficina em `raiz`. Com `completa`, a oficina traz áudio, Alinhamento e Roteiro.
/// `sem-estado` não tem `estado.toml`: deve entrar como Hipótese.
fn preparar(raiz: &Path, completa: bool) -> (PathBuf, PathBuf) {
    let plano_path = raiz.join("plano.toml");
    fs::write(&plano_path, plano(&sha256_hex(b"aula sintetica"))).unwrap();
    let oficina = raiz.join("oficina");
    let escrever = |rel: &str, dados: &[u8]| {
        let destino = oficina.join(rel);
        fs::create_dir_all(destino.parent().unwrap()).unwrap();
        fs::write(destino, dados).unwrap();
    };
    for id in ["tres-notas", "tres-notas-volta", "sem-estado"] {
        escrever(&format!("partituras/{id}.musicxml"), MUSICXML.as_bytes());
    }
    escrever(
        "partituras/tres-notas.estado.toml",
        b"id = \"tres-notas\"\naula = 1\nestado = \"aprovada\"\nconfianca = 0.95\n\
          trecho_aula = { inicio_s = 5.0, fim_s = 15.5 }\nbpm_sugerido = 80\n",
    );
    escrever(
        "partituras/tres-notas-volta.estado.toml",
        b"id = \"tres-notas-volta\"\nestado = \"em_revisao\"\n",
    );
    if completa {
        escrever("audio/tres-notas.opus", b"OggS-sintetico");
        escrever("alinhamentos/tres-notas.json", ALINHAMENTO.as_bytes());
        escrever("roteiro.toml", ROTEIRO.as_bytes());
    }
    (plano_path, oficina)
}

fn rodar(args: &[&str]) -> Result<String, ErroCli> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut saida = Vec::new();
    executar(&args, &mut saida)?;
    Ok(String::from_utf8(saida).unwrap())
}

// ---------- ida e volta ----------

#[test]
fn importa_empacota_e_le_com_os_estados_do_estado_toml() {
    let tmp = tempfile::tempdir().unwrap();
    let (plano, oficina) = preparar(tmp.path(), true);
    let origem = tmp.path().join("origem");

    let resumo = importar(&plano, &oficina, &origem, false).unwrap();
    assert_eq!(resumo.contagem.licoes, 3);
    assert_eq!(resumo.contagem.exercicios, 3);
    assert_eq!(resumo.contagem.por_estado(EstadoConteudo::Aprovada), 1);
    assert_eq!(resumo.contagem.por_estado(EstadoConteudo::EmRevisao), 1);
    assert_eq!(resumo.contagem.por_estado(EstadoConteudo::Hipotese), 1);
    assert_eq!(resumo.partituras, 3);
    assert_eq!(resumo.audios, 1);
    assert_eq!(resumo.alinhamentos, 1);
    assert_eq!(
        (resumo.roteiro, resumo.falas, resumo.falas_aprovadas),
        (true, 2, 1)
    );
    assert_eq!((resumo.omitidos_exercicios, resumo.omitidos_falas), (0, 0));
    assert!(
        resumo.avisos.iter().any(|a| a.contains("sem-estado")),
        "{:?}",
        resumo.avisos
    );

    // A pasta-origem tem o que o empacotador espera, sem assinatura nem [[arquivos]].
    assert!(origem.join("partituras/tres-notas.musicxml").is_file());
    assert!(origem.join("audio/tres-notas.opus").is_file());
    assert!(origem.join("alinhamentos/tres-notas.json").is_file());
    assert!(origem.join("roteiro.toml").is_file());
    let manifesto = fs::read_to_string(origem.join("manifesto.toml")).unwrap();
    assert!(!manifesto.contains("[assinatura]"), "{manifesto}");
    assert!(!manifesto.contains("[[arquivos]]"), "{manifesto}");

    let bytes = empacotar(&origem, &ChavePrivada::gerar()).unwrap();
    let pacote = ler_pacote(&bytes).unwrap();
    assert_eq!(pacote.contagem(), resumo.contagem);
    assert_eq!(pacote.roteiro.as_ref().unwrap().falas.len(), 2);

    let estado_de = |id: &str| {
        pacote
            .manifesto
            .exercicios()
            .find(|e| e.exercicio.id == id)
            .unwrap()
            .exercicio
            .clone()
    };
    let aprovado = estado_de("tres-notas");
    assert_eq!(aprovado.estado, EstadoConteudo::Aprovada);
    assert_eq!(aprovado.bpm, Some(80), "bpm_sugerido do estado.toml");
    assert_eq!(
        aprovado.trecho_aula,
        Some(TrechoAula {
            inicio_s: 5.0,
            fim_s: 15.5
        })
    );
    assert_eq!(aprovado.audio.as_deref(), Some("audio/tres-notas.opus"));
    assert!(pacote.alinhamento("tres-notas").is_some());
    assert_eq!(
        estado_de("tres-notas-volta").estado,
        EstadoConteudo::EmRevisao
    );
    assert_eq!(estado_de("tres-notas-volta").audio, None);
    assert_eq!(estado_de("sem-estado").estado, EstadoConteudo::Hipotese);
}

#[test]
fn somente_aprovadas_omite_exercicios_e_falas_que_nao_sao_aprovadas() {
    let tmp = tempfile::tempdir().unwrap();
    let (plano, oficina) = preparar(tmp.path(), true);
    // Fala aprovada presa a um Exercício em revisão: sai junto com o Exercício.
    let roteiro = format!(
        "{ROTEIRO}\n[[falas]]\nid = \"s01-a01-extra\"\nlicao = \"s01-a01\"\n\
         exercicio = \"tres-notas-volta\"\nmomento = \"antes\"\n\
         texto = \"Agora a volta.\"\nestado = \"aprovada\"\n"
    );
    fs::write(oficina.join("roteiro.toml"), roteiro).unwrap();
    // Exercício omitido não precisa de partitura.
    fs::remove_file(oficina.join("partituras/tres-notas-volta.musicxml")).unwrap();
    fs::remove_file(oficina.join("partituras/sem-estado.musicxml")).unwrap();
    let origem = tmp.path().join("origem");

    let resumo = importar(&plano, &oficina, &origem, true).unwrap();
    assert_eq!(resumo.contagem.exercicios, 1);
    assert_eq!(resumo.contagem.por_estado(EstadoConteudo::Aprovada), 1);
    assert_eq!(resumo.omitidos_exercicios, 2);
    assert_eq!(
        resumo.omitidos_falas, 2,
        "passo em hipótese e fala de Exercício omitido"
    );
    assert_eq!((resumo.falas, resumo.falas_aprovadas), (1, 1));
    assert!(!origem.join("partituras/tres-notas-volta.musicxml").exists());
    assert!(!origem.join("partituras/sem-estado.musicxml").exists());

    let pacote = ler_pacote(&empacotar(&origem, &ChavePrivada::gerar()).unwrap()).unwrap();
    assert_eq!(pacote.contagem().exercicios, 1);
    assert_eq!(pacote.roteiro.as_ref().unwrap().falas.len(), 1);
}

#[test]
fn exercicio_sem_partitura_da_erro_claro_e_nada_e_gravado() {
    let tmp = tempfile::tempdir().unwrap();
    let (plano, oficina) = preparar(tmp.path(), true);
    fs::remove_file(oficina.join("partituras/sem-estado.musicxml")).unwrap();
    let origem = tmp.path().join("origem");

    let err = importar(&plano, &oficina, &origem, false).unwrap_err();
    let msg = err.to_string();
    assert!(matches!(err, ErroPacote::Importacao(_)), "{msg}");
    assert!(msg.contains("sem-estado"), "{msg}");
    assert!(msg.contains("partituras/sem-estado.musicxml"), "{msg}");
    assert!(!origem.exists(), "erro antes de gravar qualquer coisa");
}

#[test]
fn arquivos_opcionais_ausentes_so_nao_entram() {
    let tmp = tempfile::tempdir().unwrap();
    let (plano, oficina) = preparar(tmp.path(), false);
    let origem = tmp.path().join("origem");

    let resumo = importar(&plano, &oficina, &origem, false).unwrap();
    assert_eq!((resumo.audios, resumo.alinhamentos), (0, 0));
    assert_eq!((resumo.roteiro, resumo.falas), (false, 0));
    assert!(!origem.join("audio").exists());
    assert!(!origem.join("alinhamentos").exists());
    assert!(!origem.join("roteiro.toml").exists());
    assert!(
        resumo.avisos.iter().any(|a| a.contains("roteiro.toml")),
        "{:?}",
        resumo.avisos
    );

    let pacote = ler_pacote(&empacotar(&origem, &ChavePrivada::gerar()).unwrap()).unwrap();
    assert!(pacote.roteiro.is_none());
    let ex = pacote
        .manifesto
        .exercicios()
        .find(|e| e.exercicio.id == "tres-notas")
        .unwrap()
        .exercicio;
    assert_eq!((ex.audio.as_ref(), ex.alinhamento.as_ref()), (None, None));
}

// ---------- recusas ----------

#[test]
fn id_de_exercicio_com_barra_e_recusado_antes_de_qualquer_caminho() {
    let tmp = tempfile::tempdir().unwrap();
    let (plano, oficina) = preparar(tmp.path(), true);
    let texto = fs::read_to_string(&plano)
        .unwrap()
        .replace("id = \"tres-notas-volta\"", "id = \"../fora\"");
    fs::write(&plano, texto).unwrap();
    let err = importar(&plano, &oficina, &tmp.path().join("origem"), false).unwrap_err();
    assert!(
        matches!(
            err,
            ErroPacote::IdInvalido {
                tipo: "exercício",
                ..
            }
        ),
        "{err}"
    );
}

#[test]
fn destino_nao_vazio_e_recusado() {
    let tmp = tempfile::tempdir().unwrap();
    let (plano, oficina) = preparar(tmp.path(), true);
    let origem = tmp.path().join("origem");
    fs::create_dir_all(&origem).unwrap();
    fs::write(origem.join("sobra.txt"), b"x").unwrap();
    let err = importar(&plano, &oficina, &origem, false).unwrap_err();
    assert!(matches!(err, ErroPacote::Importacao(_)), "{err}");
    assert!(err.to_string().contains("não está vazia"), "{err}");
}

// ---------- CLI ----------

#[test]
fn cli_importar_mostra_o_resumo_e_valida_os_argumentos() {
    let tmp = tempfile::tempdir().unwrap();
    let (plano, oficina) = preparar(tmp.path(), true);
    let s = |p: &Path| p.to_str().unwrap().to_string();
    let origem = tmp.path().join("origem");

    let saida = rodar(&[
        "importar",
        "--plano",
        &s(&plano),
        "--oficina",
        &s(&oficina),
        "-o",
        &s(&origem),
    ])
    .unwrap();
    assert!(saida.contains("Lições: 3"), "{saida}");
    assert!(saida.contains("Exercícios: 3"), "{saida}");
    assert!(saida.contains("aprovada 1"), "{saida}");
    assert!(origem.join("manifesto.toml").is_file());

    let sem_destino = rodar(&["importar", "--plano", &s(&plano), "--oficina", &s(&oficina)]);
    assert!(
        matches!(sem_destino, Err(ErroCli::Uso(_))),
        "{sem_destino:?}"
    );

    let filtrada = tmp.path().join("filtrada");
    let saida = rodar(&[
        "importar",
        "--plano",
        &s(&plano),
        "--oficina",
        &s(&oficina),
        "-o",
        &s(&filtrada),
        "--somente-aprovadas",
    ])
    .unwrap();
    assert!(saida.contains("Exercícios: 1"), "{saida}");
}
