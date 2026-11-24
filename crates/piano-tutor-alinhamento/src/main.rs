//! `piano-tutor-alinhar`: alinha uma partitura (MusicXML/MXL) ao áudio WAV de uma Aula.

use piano_tutor_alinhamento::dtw::ConfigDtw;
use piano_tutor_alinhamento::wav::ler_wav;
use piano_tutor_alinhamento::{ConfigAlinhamento, alinhar};
use piano_tutor_core::partitura::Partitura;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const AJUDA: &str = "\
piano-tutor-alinhar: alinha uma partitura ao áudio de uma Aula (DTW de croma).

Uso:
  piano-tutor-alinhar --partitura x.musicxml --audio aula.wav --exercicio <id> [opções] -o alinhamento.json

Opções:
  --partitura <arq>       MusicXML ou .mxl (obrigatório)
  --audio <arq.wav>       WAV mono (ffmpeg -i aula.mp4 -ac 1 -ar 22050 -vn aula.wav) (obrigatório)
  --exercicio <id>        id do Exercício no manifesto (obrigatório)
  -o, --saida <arq.json>  grava o alinhamento (sem isso, só imprime o resumo)
  --bpm <N>               andamento (semínimas/min) que sobrepõe o do arquivo
  --inicio-s <A> --fim-s <B>  restringe a busca ao trecho [A, B] da Aula
  --offset-aula-s <S>     instante da Aula em que o WAV começa (padrão 0)
  --audio-caminho <c>     caminho do áudio recortado no pacote (padrão audio/<id>.opus)
  --margem-s <S>          margem do recorte (padrão 0,5)
  --global                DTW global (o áudio inteiro é o exercício) em vez de subsequência
  --banda-s <S>           banda de Sakoe-Chiba em segundos (só com --global)
  --relatorio             imprime índice, MIDI, nome, t_audio_s e confiança por nota
  -h, --ajuda             esta mensagem
";

#[derive(Default)]
struct Args {
    partitura: Option<PathBuf>,
    audio: Option<PathBuf>,
    exercicio: Option<String>,
    saida: Option<PathBuf>,
    bpm: Option<f64>,
    inicio_s: Option<f64>,
    fim_s: Option<f64>,
    offset_aula_s: f64,
    audio_caminho: Option<String>,
    margem_s: Option<f64>,
    global: bool,
    banda_s: Option<f64>,
    relatorio: bool,
    ajuda: bool,
}

fn numero(opcao: &str, valor: Option<String>) -> Result<f64, String> {
    let v = valor.ok_or_else(|| format!("{opcao} precisa de um valor"))?;
    v.replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("valor inválido para {opcao}: {v}"))
}

fn texto(opcao: &str, valor: Option<String>) -> Result<String, String> {
    valor.ok_or_else(|| format!("{opcao} precisa de um valor"))
}

fn ler_args(mut it: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut a = Args::default();
    while let Some(op) = it.next() {
        match op.as_str() {
            "--partitura" => a.partitura = Some(texto(&op, it.next())?.into()),
            "--audio" => a.audio = Some(texto(&op, it.next())?.into()),
            "--exercicio" => a.exercicio = Some(texto(&op, it.next())?),
            "-o" | "--saida" => a.saida = Some(texto(&op, it.next())?.into()),
            "--bpm" => a.bpm = Some(numero(&op, it.next())?),
            "--inicio-s" => a.inicio_s = Some(numero(&op, it.next())?),
            "--fim-s" => a.fim_s = Some(numero(&op, it.next())?),
            "--offset-aula-s" => a.offset_aula_s = numero(&op, it.next())?,
            "--audio-caminho" => a.audio_caminho = Some(texto(&op, it.next())?),
            "--margem-s" => a.margem_s = Some(numero(&op, it.next())?),
            "--global" => a.global = true,
            "--banda-s" => a.banda_s = Some(numero(&op, it.next())?),
            "--relatorio" => a.relatorio = true,
            "-h" | "--ajuda" | "--help" => a.ajuda = true,
            outro => return Err(format!("opção desconhecida: {outro}")),
        }
    }
    Ok(a)
}

fn carregar_partitura(caminho: &Path) -> Result<Partitura, String> {
    let e_mxl = caminho
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mxl"));
    let r = if e_mxl {
        let bytes = std::fs::read(caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
        Partitura::de_mxl(&bytes)
    } else {
        let texto =
            std::fs::read_to_string(caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
        Partitura::de_musicxml(&texto)
    };
    r.map_err(|e| format!("partitura inválida: {e}"))
}

fn executar() -> Result<(), String> {
    let a = ler_args(std::env::args().skip(1))?;
    if a.ajuda {
        print!("{AJUDA}");
        return Ok(());
    }
    let partitura_arq = a.partitura.ok_or("falta --partitura")?;
    let audio_arq = a.audio.ok_or("falta --audio")?;
    let exercicio = a.exercicio.ok_or("falta --exercicio")?;

    let mut cfg = ConfigAlinhamento {
        bpm: a.bpm,
        ..ConfigAlinhamento::default()
    };
    if let Some(m) = a.margem_s {
        cfg.margem_s = m.max(0.0);
    }
    match (a.inicio_s, a.fim_s) {
        (None, None) => {}
        (ini, fim) => cfg.busca = Some((ini.unwrap_or(0.0), fim.unwrap_or(f64::MAX))),
    }
    if a.global {
        let quadro_s = cfg.croma.hop_s;
        cfg.dtw = ConfigDtw {
            subsequencia: false,
            banda: a.banda_s.map(|s| (s / quadro_s).ceil() as usize),
            ..ConfigDtw::default()
        };
    } else if a.banda_s.is_some() {
        return Err("--banda-s só vale com --global".into());
    }

    let partitura = carregar_partitura(&partitura_arq)?;
    let audio = ler_wav(&audio_arq).map_err(|e| format!("{}: {e}", audio_arq.display()))?;
    let r = alinhar(&partitura, &audio, &cfg).map_err(|e| e.to_string())?;

    let caminho_audio = a
        .audio_caminho
        .unwrap_or_else(|| format!("audio/{exercicio}.opus"));
    if let Some(saida) = &a.saida {
        let json = r.para_json(&exercicio, &caminho_audio, a.offset_aula_s);
        let corpo = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
        std::fs::write(saida, corpo + "\n").map_err(|e| format!("{}: {e}", saida.display()))?;
    }
    println!(
        "trecho encontrado: inicio_s={:.3} fim_s={:.3} (duração {:.3} s)",
        r.recorte.inicio_s,
        r.recorte.fim_s,
        r.recorte.fim_s - r.recorte.inicio_s
    );
    println!(
        "na Aula: inicio_s={:.3} fim_s={:.3}",
        a.offset_aula_s + r.recorte.inicio_s,
        a.offset_aula_s + r.recorte.fim_s
    );
    println!(
        "notas: {}  confiança média: {:.2}  custo médio do croma: {:.3}",
        r.notas.len(),
        r.confianca_media,
        r.custo_medio
    );
    if let Some(saida) = &a.saida {
        println!("alinhamento (hipótese) gravado em {}", saida.display());
    }
    if a.relatorio {
        print!("{}", r.relatorio());
    }
    Ok(())
}

fn main() -> ExitCode {
    match executar() {
        Ok(()) => ExitCode::SUCCESS,
        Err(m) => {
            eprintln!("erro: {m}");
            ExitCode::FAILURE
        }
    }
}
