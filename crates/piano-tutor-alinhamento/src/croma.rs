//! Croma (12 classes de altura) por quadros, a partir do áudio e a partir da partitura.
//!
//! - **Do áudio**: STFT (Hann), magnitude comprimida em log somada por classe de altura entre
//!   `fmin_hz` e `fmax_hz`, normalizada (norma L2). Quadros quase silenciosos viram vetor zero.
//! - **Da partitura**: croma "sintético" na mesma taxa de quadros: cada nota ativa a classe da
//!   sua altura e, com peso menor, as classes dos primeiros harmônicos.
//!
//! O custo entre dois quadros ([`custo_cosseno`]) trata o vetor zero (silêncio) à parte.

use crate::espectro::{numero_de_quadros, para_cada_quadro, proxima_potencia_de_dois};
use crate::wav::Audio;
use piano_tutor_core::comparador::EventoEsperado;

pub const CLASSES: usize = 12;

/// Um quadro de croma: norma L2 igual a 1, ou todo zero (silêncio).
pub type Quadro = [f32; CLASSES];

/// Sequência de quadros de croma.
#[derive(Clone, Debug, PartialEq)]
pub struct Croma {
    pub quadros: Vec<Quadro>,
    /// Duração de um quadro (passo), em segundos.
    pub quadro_s: f64,
    /// Instante, em segundos, do quadro 0 na base de tempo de origem (a partitura começa na
    /// primeira nota; o áudio começa em 0).
    pub origem_s: f64,
}

impl Croma {
    pub fn tempo_do_quadro(&self, i: usize) -> f64 {
        self.origem_s + i as f64 * self.quadro_s
    }
}

/// Parâmetros do croma do áudio.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigCroma {
    /// Duração da janela do STFT, em segundos (arredondada para potência de dois de amostras).
    pub janela_s: f64,
    /// Passo entre quadros, em segundos.
    pub hop_s: f64,
    /// Janela curta (s) usada só para decidir o que é silêncio. A janela do croma é longa
    /// (resolução em frequência) e borra o começo das notas; a janela curta liga o croma só
    /// quando o som realmente começou.
    pub janela_energia_s: f64,
    pub fmin_hz: f64,
    pub fmax_hz: f64,
    /// `ln(1 + compressao * magnitude)`; menor = mais linear.
    pub compressao: f32,
    /// Quadro com energia abaixo desta fração do quadro mais forte é silêncio.
    pub limiar_silencio_rel: f32,
    /// Piso absoluto de energia (soma das magnitudes na banda) para não ser silêncio.
    pub limiar_silencio_abs: f32,
}

impl Default for ConfigCroma {
    fn default() -> Self {
        Self {
            janela_s: 0.186,
            hop_s: 0.0232,
            janela_energia_s: 0.046,
            fmin_hz: 150.0,
            fmax_hz: 3500.0,
            compressao: 10.0,
            limiar_silencio_rel: 0.01,
            limiar_silencio_abs: 1e-3,
        }
    }
}

/// Pesos dos harmônicos 1..=N no croma sintético da partitura.
pub const HARMONICOS_PADRAO: [f32; 6] = [1.0, 0.8, 0.6, 0.5, 0.4, 0.3];

/// Classe de altura (0 = Dó) da nota mais próxima de uma frequência.
fn classe_da_frequencia(hz: f64) -> usize {
    let midi = 69.0 + 12.0 * (hz / 440.0).log2();
    (midi.round() as i64).rem_euclid(12) as usize
}

fn normalizar(q: &mut Quadro) {
    let norma = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norma > 1e-12 {
        q.iter_mut().for_each(|v| *v /= norma);
    } else {
        *q = [0.0; CLASSES];
    }
}

/// Calcula o croma do áudio. Devolve também `quadro_s` exato (`hop / taxa`).
pub fn croma_do_audio(audio: &Audio, cfg: &ConfigCroma) -> Croma {
    let taxa = f64::from(audio.taxa_hz);
    let hop = ((cfg.hop_s * taxa).round() as usize).max(1);
    let n_fft = proxima_potencia_de_dois((cfg.janela_s * taxa).round() as usize);
    let bin_hz = taxa / n_fft as f64;
    let k0 = ((cfg.fmin_hz / bin_hz).ceil() as usize).max(1);
    let k1 = ((cfg.fmax_hz / bin_hz).floor() as usize).min(n_fft / 2);
    let classes: Vec<usize> = (0..=k1)
        .map(|k| {
            if k >= k0 {
                classe_da_frequencia(k as f64 * bin_hz)
            } else {
                0
            }
        })
        .collect();

    let total = numero_de_quadros(audio.amostras.len(), hop);
    let mut quadros: Vec<Quadro> = Vec::with_capacity(total);
    para_cada_quadro(&audio.amostras, n_fft, hop, |_, mag| {
        let mut q = [0.0_f32; CLASSES];
        for k in k0..=k1 {
            q[classes[k]] += (1.0 + cfg.compressao * mag[k]).ln();
        }
        quadros.push(q);
    });

    // Energia por quadro numa janela curta (mesmos instantes de quadro).
    let n_curto = proxima_potencia_de_dois((cfg.janela_energia_s * taxa).round() as usize);
    let bin_curto = taxa / n_curto as f64;
    let c0 = ((cfg.fmin_hz / bin_curto).ceil() as usize).max(1);
    let c1 = ((cfg.fmax_hz / bin_curto).floor() as usize).min(n_curto / 2);
    let mut energias: Vec<f32> = Vec::with_capacity(total);
    para_cada_quadro(&audio.amostras, n_curto, hop, |_, mag| {
        energias.push(mag[c0..=c1].iter().sum());
    });
    let maxima = energias.iter().copied().fold(0.0_f32, f32::max);
    let limiar = (cfg.limiar_silencio_rel * maxima).max(cfg.limiar_silencio_abs);
    for (q, e) in quadros.iter_mut().zip(&energias) {
        if *e < limiar {
            *q = [0.0; CLASSES];
        } else {
            normalizar(q);
        }
    }
    Croma {
        quadros,
        quadro_s: hop as f64 / taxa,
        origem_s: 0.0,
    }
}

/// Croma sintético da partitura com quadros de `quadro_s` segundos. O quadro 0 corresponde ao
/// início da primeira nota. `None` sem notas.
pub fn croma_da_partitura(
    esperados: &[EventoEsperado],
    quadro_s: f64,
    harmonicos: &[f32],
) -> Option<Croma> {
    if esperados.is_empty() || !(quadro_s.is_finite() && quadro_s > 0.0) {
        return None;
    }
    let origem = esperados
        .iter()
        .map(|e| e.inicio_s)
        .fold(f64::INFINITY, f64::min);
    let fim = esperados
        .iter()
        .map(|e| e.inicio_s + e.duracao_s)
        .fold(f64::NEG_INFINITY, f64::max);
    let n = (((fim - origem) / quadro_s).ceil() as usize).max(1);
    let mut quadros = vec![[0.0_f32; CLASSES]; n];
    for e in esperados {
        let a = (((e.inicio_s - origem) / quadro_s).round() as usize).min(n - 1);
        let b = (((e.inicio_s + e.duracao_s - origem) / quadro_s).round() as usize).clamp(a + 1, n);
        for (h, peso) in harmonicos.iter().enumerate() {
            let semitons = (12.0 * ((h + 1) as f64).log2()).round() as i32;
            let classe = (i32::from(e.midi) + semitons).rem_euclid(12) as usize;
            for q in &mut quadros[a..b] {
                q[classe] += peso;
            }
        }
    }
    quadros.iter_mut().for_each(normalizar);
    Some(Croma {
        quadros,
        quadro_s,
        origem_s: origem,
    })
}

/// Custo entre um quadro da partitura (`p`) e um do áudio (`a`), de 0 (iguais) a 1.
///
/// - os dois silenciosos: 0;
/// - partitura silenciosa e áudio soando (cauda da nota anterior, ruído): 0,3;
/// - partitura soando e áudio silencioso: 1;
/// - senão, `1 - cos(p, a)`.
pub fn custo_cosseno(p: &Quadro, a: &Quadro) -> f32 {
    let p_zero = p.iter().all(|v| *v == 0.0);
    let a_zero = a.iter().all(|v| *v == 0.0);
    match (p_zero, a_zero) {
        (true, true) => 0.0,
        (true, false) => 0.3,
        (false, true) => 1.0,
        (false, false) => {
            let d: f32 = p.iter().zip(a).map(|(x, y)| x * y).sum();
            (1.0 - d).clamp(0.0, 1.0)
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::f64::consts::PI;

    fn nota(midi: u8, ini: f64, dur: f64) -> EventoEsperado {
        EventoEsperado {
            midi,
            inicio_s: ini,
            duracao_s: dur,
            parte: 0,
            pauta: 1,
            compasso: 0,
        }
    }

    fn tom(hz: f64, taxa: u32, seg: f64) -> Audio {
        Audio {
            taxa_hz: taxa,
            amostras: (0..(seg * f64::from(taxa)) as usize)
                .map(|i| (0.3 * (2.0 * PI * hz * i as f64 / f64::from(taxa)).sin()) as f32)
                .collect(),
        }
    }

    fn maior(q: &Quadro) -> usize {
        (0..CLASSES)
            .max_by(|a, b| q[*a].total_cmp(&q[*b]))
            .expect("12 classes")
    }

    #[test]
    fn la440_cai_na_classe_la() {
        let c = croma_do_audio(&tom(440.0, 22_050, 1.0), &ConfigCroma::default());
        let meio = &c.quadros[c.quadros.len() / 2];
        assert_eq!(maior(meio), 9);
        let norma: f32 = meio.iter().map(|v| v * v).sum();
        assert!((norma - 1.0).abs() < 1e-4);
        assert!((c.quadro_s - 512.0 / 22_050.0).abs() < 1e-12);
    }

    #[test]
    fn do_central_cai_na_classe_do() {
        let c = croma_do_audio(&tom(261.6256, 22_050, 1.0), &ConfigCroma::default());
        assert_eq!(maior(&c.quadros[20]), 0);
    }

    #[test]
    fn silencio_vira_zero() {
        let a = Audio {
            taxa_hz: 22_050,
            amostras: vec![0.0; 22_050],
        };
        let c = croma_do_audio(&a, &ConfigCroma::default());
        assert!(c.quadros.iter().all(|q| q.iter().all(|v| *v == 0.0)));
    }

    #[test]
    fn croma_da_partitura_marca_nota_e_harmonicos() {
        let c =
            croma_da_partitura(&[nota(60, 1.0, 0.5)], 0.025, &HARMONICOS_PADRAO).expect("há nota");
        assert_eq!(c.origem_s, 1.0);
        assert_eq!(c.quadros.len(), 20);
        let q = &c.quadros[0];
        assert_eq!(maior(q), 0);
        // Terceiro harmônico (quinta justa acima): Sol (7); quinto: Mi (4).
        assert!(q[7] > 0.0 && q[4] > 0.0);
        assert_eq!(q[1], 0.0);
    }

    #[test]
    fn pausa_entre_notas_fica_zerada_e_vazio_devolve_none() {
        let c = croma_da_partitura(
            &[nota(60, 0.0, 0.1), nota(62, 0.5, 0.1)],
            0.1,
            &HARMONICOS_PADRAO,
        )
        .expect("há notas");
        assert!(c.quadros[2].iter().all(|v| *v == 0.0));
        assert!(croma_da_partitura(&[], 0.1, &HARMONICOS_PADRAO).is_none());
    }

    #[test]
    fn custo_cosseno_casos_basicos() {
        let mut a = [0.0; CLASSES];
        a[0] = 1.0;
        let mut b = [0.0; CLASSES];
        b[1] = 1.0;
        let z = [0.0; CLASSES];
        assert_eq!(custo_cosseno(&a, &a), 0.0);
        assert_eq!(custo_cosseno(&a, &b), 1.0);
        assert_eq!(custo_cosseno(&z, &z), 0.0);
        assert_eq!(custo_cosseno(&z, &a), 0.3);
        assert_eq!(custo_cosseno(&a, &z), 1.0);
    }
}
