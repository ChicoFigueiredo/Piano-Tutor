//! Saída: JSON `alinhamentos/<id>.json` (contrato em `docs/specs/FORMATO-PACOTE-DE-CURSO.md`) e
//! relatório em texto por nota.

use crate::Alinhamento;
use piano_tutor_core::NotaMidi;
use serde_json::{Value, json};
use std::fmt::Write;

/// Versão do formato do JSON de alinhamento.
pub const FORMATO_VERSAO: u32 = 1;

fn arredondar(v: f64, casas: i32) -> f64 {
    let f = 10.0_f64.powi(casas);
    (v * f).round() / f
}

/// Nome da nota em português com oitava científica (60 = Dó4).
pub fn nome_da_nota(midi: u8) -> String {
    match NotaMidi::nova(midi) {
        Some(n) => format!("{}{}", n.nome_pt(), n.oitava()),
        None => format!("?{midi}"),
    }
}

impl Alinhamento {
    /// JSON do Alinhamento com estado `hipotese`.
    ///
    /// `t_audio_s` sai relativo ao início do recorte (`audio` é o trecho recortado) e
    /// `offset_aula_s` = `offset_base_s` + início do recorte, para que o instante no vídeo seja
    /// `offset_aula_s + t_audio_s`. `offset_base_s` é o instante da Aula em que começa o WAV
    /// analisado (0 quando o WAV é a Aula inteira). `compasso` sai em base 1.
    pub fn para_json(&self, exercicio: &str, audio: &str, offset_base_s: f64) -> Value {
        let notas: Vec<Value> = self
            .notas
            .iter()
            .map(|n| {
                json!({
                    "indice": n.indice,
                    "compasso": n.compasso + 1,
                    "midi": n.midi,
                    "t_partitura_s": arredondar(n.t_partitura_s, 4),
                    "t_audio_s": arredondar((n.t_audio_s - self.recorte.inicio_s).max(0.0), 4),
                    "confianca": arredondar(f64::from(n.confianca), 3),
                })
            })
            .collect();
        json!({
            "formato_versao": FORMATO_VERSAO,
            "exercicio": exercicio,
            "audio": audio,
            "estado": "hipotese",
            "metodo": "dtw-croma",
            "offset_aula_s": arredondar(offset_base_s + self.recorte.inicio_s, 4),
            "notas": notas,
        })
    }

    /// Uma linha por nota: índice, MIDI, nome, instante no áudio e confiança.
    pub fn relatorio(&self) -> String {
        let mut s = String::from("indice  midi  nota     t_audio_s  confianca  onset\n");
        for n in &self.notas {
            let _ = writeln!(
                s,
                "{:>6}  {:>4}  {:<7}  {:>9.3}  {:>9.2}  {}",
                n.indice,
                n.midi,
                nome_da_nota(n.midi),
                n.t_audio_s,
                n.confianca,
                if n.com_onset { "sim" } else { "nao" },
            );
        }
        s
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::mapeamento::{NotaAlinhada, Recorte};

    fn exemplo() -> Alinhamento {
        Alinhamento {
            notas: vec![NotaAlinhada {
                indice: 0,
                compasso: 0,
                midi: 60,
                t_partitura_s: 0.0,
                duracao_partitura_s: 0.5,
                t_audio_s: 95.4213,
                confianca: 0.8123,
                com_onset: true,
            }],
            recorte: Recorte {
                inicio_s: 95.0,
                fim_s: 97.0,
            },
            confianca_media: 0.8123,
            custo_medio: 0.1,
        }
    }

    #[test]
    fn json_segue_o_contrato() {
        let j = exemplo().para_json("cinco-notas", "audio/cinco-notas.opus", 0.0);
        assert_eq!(j["formato_versao"], 1);
        assert_eq!(j["exercicio"], "cinco-notas");
        assert_eq!(j["audio"], "audio/cinco-notas.opus");
        assert_eq!(j["estado"], "hipotese");
        assert_eq!(j["metodo"], "dtw-croma");
        assert_eq!(j["offset_aula_s"], 95.0);
        let n = &j["notas"][0];
        assert_eq!(n["indice"], 0);
        assert_eq!(n["compasso"], 1);
        assert_eq!(n["midi"], 60);
        assert_eq!(n["t_partitura_s"], 0.0);
        assert_eq!(n["t_audio_s"], 0.4213);
        assert_eq!(n["confianca"], 0.812);
    }

    #[test]
    fn offset_base_soma_ao_inicio_do_recorte() {
        let j = exemplo().para_json("x", "audio/x.opus", 100.0);
        assert_eq!(j["offset_aula_s"], 195.0);
    }

    #[test]
    fn relatorio_tem_nome_em_portugues() {
        let r = exemplo().relatorio();
        assert!(r.contains("Dó4"), "{r}");
        assert!(r.contains("95.421"), "{r}");
        assert_eq!(nome_da_nota(61), "Dó♯4");
    }
}
