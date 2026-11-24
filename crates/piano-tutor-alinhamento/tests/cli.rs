//! Teste da CLI `piano-tutor-alinhar` com fixtures sintéticas gravadas em `CARGO_TARGET_TMPDIR`.

mod comum;

use comum::{escrever_wav, gravar, melodia, musicxml};
use std::path::PathBuf;
use std::process::Command;

fn pasta(nome: &str) -> PathBuf {
    let p = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(nome);
    std::fs::create_dir_all(&p).expect("criar pasta");
    p
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_piano-tutor-alinhar"))
}

#[test]
fn cli_gera_alinhamento_json_e_relatorio() {
    let dir = pasta("cli-ok");
    let notas = melodia();
    let g = gravar(&notas, 100.0, 1.0, 6.0);
    escrever_wav(&dir.join("aula.wav"), &g.audio);
    std::fs::write(dir.join("ex.musicxml"), musicxml(&notas, 100)).expect("gravar xml");
    let saida = dir.join("alinhamento.json");
    let r = bin()
        .args([
            "--partitura",
            dir.join("ex.musicxml").to_str().expect("utf8"),
            "--audio",
            dir.join("aula.wav").to_str().expect("utf8"),
            "--exercicio",
            "melodia-de-teste",
            "--offset-aula-s",
            "100",
            "--relatorio",
            "-o",
            saida.to_str().expect("utf8"),
        ])
        .output()
        .expect("executar");
    let out = String::from_utf8_lossy(&r.stdout);
    assert!(
        r.status.success(),
        "{out}\n{}",
        String::from_utf8_lossy(&r.stderr)
    );
    assert!(out.contains("trecho encontrado: inicio_s="), "{out}");
    assert!(out.contains("confiança média"), "{out}");
    assert!(out.contains("Dó4") && out.contains("Sol4"), "{out}");

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&saida).expect("ler json")).expect("json");
    assert_eq!(json["formato_versao"], 1);
    assert_eq!(json["exercicio"], "melodia-de-teste");
    assert_eq!(json["audio"], "audio/melodia-de-teste.opus");
    assert_eq!(json["estado"], "hipotese");
    assert_eq!(json["metodo"], "dtw-croma");
    let offset = json["offset_aula_s"].as_f64().expect("número");
    // Recorte começa em ~5,5 s do WAV; a Aula começa em 100 s.
    assert!((offset - 105.5).abs() < 0.1, "offset {offset}");
    let n = json["notas"].as_array().expect("lista");
    assert_eq!(n.len(), 7);
    for (k, nota) in n.iter().enumerate() {
        assert_eq!(nota["indice"], k);
        let t_aula = offset + nota["t_audio_s"].as_f64().expect("número");
        let real = 100.0 + g.verdade[k].expect("tocada");
        assert!((t_aula - real).abs() < 0.06, "nota {k}: {t_aula} vs {real}");
        let c = nota["confianca"].as_f64().expect("número");
        assert!((0.0..=1.0).contains(&c));
    }
    assert_eq!(n[3]["midi"], 67);
    assert_eq!(n[0]["compasso"], 1);
    assert_eq!(n[6]["compasso"], 2);
}

#[test]
fn cli_aceita_mxl_bpm_e_busca() {
    let dir = pasta("cli-mxl");
    let notas = melodia();
    let g = gravar(&notas, 80.0, 1.0, 6.0);
    escrever_wav(&dir.join("aula.wav"), &g.audio);
    let mxl = piano_tutor_core::mxl::empacotar_mxl("ex.musicxml", &musicxml(&notas, 100))
        .expect("empacotar");
    std::fs::write(dir.join("ex.mxl"), mxl).expect("gravar mxl");
    let r = bin()
        .args([
            "--partitura",
            dir.join("ex.mxl").to_str().expect("utf8"),
            "--audio",
            dir.join("aula.wav").to_str().expect("utf8"),
            "--exercicio",
            "x",
            "--bpm",
            "80",
            "--inicio-s",
            "4",
            "--fim-s",
            "14",
        ])
        .output()
        .expect("executar");
    let out = String::from_utf8_lossy(&r.stdout);
    assert!(
        r.status.success(),
        "{out}\n{}",
        String::from_utf8_lossy(&r.stderr)
    );
    assert!(out.contains("trecho encontrado"), "{out}");
}

#[test]
fn cli_reporta_erros_de_uso() {
    let r = bin().args(["--audio", "x.wav"]).output().expect("executar");
    assert!(!r.status.success());
    assert!(String::from_utf8_lossy(&r.stderr).contains("falta --partitura"));
    let r = bin().arg("--nao-existe").output().expect("executar");
    assert!(!r.status.success());
    assert!(String::from_utf8_lossy(&r.stderr).contains("opção desconhecida"));
    let r = bin().arg("--ajuda").output().expect("executar");
    assert!(r.status.success());
    assert!(String::from_utf8_lossy(&r.stdout).contains("--exercicio"));
}
