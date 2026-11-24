//! Leitura de WAV PCM/float para amostras mono em `f32` (-1..1).
//!
//! O pipeline gera o WAV com `ffmpeg -i aula.mp4 -ac 1 -ar 22050 -vn x.wav`. Se vier mais de um
//! canal, a leitura faz a média dos canais; a taxa de amostragem é a do arquivo.

use hound::{SampleFormat, WavReader};
use std::fmt;
use std::io::Read;
use std::path::Path;

/// Áudio mono em memória.
#[derive(Clone, Debug, PartialEq)]
pub struct Audio {
    pub taxa_hz: u32,
    pub amostras: Vec<f32>,
}

#[derive(Debug)]
pub enum ErroWav {
    Leitura(String),
    FormatoNaoSuportado(String),
    Vazio,
}

impl fmt::Display for ErroWav {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Leitura(m) => write!(f, "falha ao ler o WAV: {m}"),
            Self::FormatoNaoSuportado(m) => write!(f, "formato de WAV não suportado: {m}"),
            Self::Vazio => write!(f, "o WAV não tem amostras"),
        }
    }
}

impl std::error::Error for ErroWav {}

impl Audio {
    pub fn duracao_s(&self) -> f64 {
        self.amostras.len() as f64 / f64::from(self.taxa_hz)
    }

    /// Trecho `[inicio_s, fim_s)` (limitado ao áudio). Devolve o trecho e o instante real
    /// (em segundos do áudio original) em que ele começa.
    pub fn recortar(&self, inicio_s: f64, fim_s: f64) -> (Audio, f64) {
        let taxa = f64::from(self.taxa_hz);
        let n = self.amostras.len();
        let a = ((inicio_s.max(0.0) * taxa).round() as usize).min(n);
        let b = ((fim_s.max(0.0) * taxa).round() as usize).clamp(a, n);
        (
            Audio {
                taxa_hz: self.taxa_hz,
                amostras: self.amostras[a..b].to_vec(),
            },
            a as f64 / taxa,
        )
    }
}

/// Lê um WAV de um arquivo.
pub fn ler_wav(caminho: &Path) -> Result<Audio, ErroWav> {
    let leitor = WavReader::open(caminho).map_err(|e| ErroWav::Leitura(e.to_string()))?;
    converter(leitor)
}

/// Lê um WAV de qualquer fonte (por exemplo, bytes em memória nos testes).
pub fn ler_wav_de<R: Read>(fonte: R) -> Result<Audio, ErroWav> {
    let leitor = WavReader::new(fonte).map_err(|e| ErroWav::Leitura(e.to_string()))?;
    converter(leitor)
}

fn converter<R: Read>(leitor: WavReader<R>) -> Result<Audio, ErroWav> {
    let spec = leitor.spec();
    let canais = usize::from(spec.channels);
    if canais == 0 {
        return Err(ErroWav::FormatoNaoSuportado("0 canais".into()));
    }
    let bruto: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Int, bits @ 8..=32) => {
            let escala = 2.0_f64.powi(i32::from(bits) - 1);
            leitor
                .into_samples::<i32>()
                .map(|s| s.map(|v| (f64::from(v) / escala) as f32))
                .collect::<Result<_, _>>()
                .map_err(|e| ErroWav::Leitura(e.to_string()))?
        }
        (SampleFormat::Float, 32) => leitor
            .into_samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| ErroWav::Leitura(e.to_string()))?,
        (formato, bits) => {
            return Err(ErroWav::FormatoNaoSuportado(format!(
                "{formato:?} de {bits} bits"
            )));
        }
    };
    let amostras: Vec<f32> = if canais == 1 {
        bruto
    } else {
        bruto
            .chunks_exact(canais)
            .map(|c| c.iter().sum::<f32>() / canais as f32)
            .collect()
    };
    if amostras.is_empty() {
        return Err(ErroWav::Vazio);
    }
    Ok(Audio {
        taxa_hz: spec.sample_rate,
        amostras,
    })
}

#[cfg(test)]
mod testes {
    use super::*;
    use hound::{WavSpec, WavWriter};
    use std::io::Cursor;

    fn escrever_i16(canais: u16, dados: &[i16]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        let spec = WavSpec {
            channels: canais,
            sample_rate: 22_050,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut w = WavWriter::new(&mut buf, spec).expect("cabeçalho");
        for d in dados {
            w.write_sample(*d).expect("amostra");
        }
        w.finalize().expect("fim");
        buf.into_inner()
    }

    #[test]
    fn le_pcm16_mono() {
        let bytes = escrever_i16(1, &[0, 16384, -16384, i16::MIN]);
        let a = ler_wav_de(Cursor::new(bytes)).expect("válido");
        assert_eq!(a.taxa_hz, 22_050);
        assert_eq!(a.amostras, vec![0.0, 0.5, -0.5, -1.0]);
    }

    #[test]
    fn estereo_vira_media() {
        let bytes = escrever_i16(2, &[16384, 0, -16384, -16384]);
        let a = ler_wav_de(Cursor::new(bytes)).expect("válido");
        assert_eq!(a.amostras, vec![0.25, -0.5]);
    }

    #[test]
    fn le_float32() {
        let mut buf = Cursor::new(Vec::new());
        let spec = WavSpec {
            channels: 1,
            sample_rate: 8000,
            bits_per_sample: 32,
            sample_format: SampleFormat::Float,
        };
        let mut w = WavWriter::new(&mut buf, spec).expect("cabeçalho");
        w.write_sample(0.25_f32).expect("amostra");
        w.finalize().expect("fim");
        let a = ler_wav_de(Cursor::new(buf.into_inner())).expect("válido");
        assert_eq!(a.amostras, vec![0.25]);
        assert_eq!(a.taxa_hz, 8000);
    }

    #[test]
    fn rejeita_lixo_e_vazio() {
        assert!(matches!(
            ler_wav_de(Cursor::new(b"nao e wav".to_vec())),
            Err(ErroWav::Leitura(_))
        ));
        let vazio = escrever_i16(1, &[]);
        assert!(matches!(
            ler_wav_de(Cursor::new(vazio)),
            Err(ErroWav::Vazio)
        ));
    }

    #[test]
    fn recorta_dentro_dos_limites() {
        let a = Audio {
            taxa_hz: 10,
            amostras: (0..100).map(|i| i as f32).collect(),
        };
        let (t, ini) = a.recortar(2.0, 3.0);
        assert_eq!(ini, 2.0);
        assert_eq!(t.amostras.len(), 10);
        assert_eq!(t.amostras[0], 20.0);
        let (t, ini) = a.recortar(-5.0, 1000.0);
        assert_eq!((t.amostras.len(), ini), (100, 0.0));
        let (t, _) = a.recortar(50.0, 60.0);
        assert!(t.amostras.is_empty());
    }
}
