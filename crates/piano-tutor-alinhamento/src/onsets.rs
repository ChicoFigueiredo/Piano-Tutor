//! Detecção de onsets por fluxo espectral com escolha de picos.
//!
//! Fluxo no quadro `t`: soma, na banda útil, dos aumentos positivos da magnitude comprimida em
//! log entre o quadro `t - 1` e o `t`. Picos locais acima de um limiar adaptativo viram onsets,
//! refinados por interpolação parabólica.

use crate::espectro::{para_cada_quadro, proxima_potencia_de_dois};
use crate::wav::Audio;

/// Um onset detectado.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Onset {
    /// Instante, em segundos do áudio.
    pub t_s: f64,
    /// Valor do fluxo no pico (unidade arbitrária, só para comparar).
    pub forca: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConfigOnsets {
    /// Janela do STFT em segundos (curta, para localizar bem no tempo).
    pub janela_s: f64,
    pub hop_s: f64,
    pub fmin_hz: f64,
    pub fmax_hz: f64,
    pub compressao: f32,
    /// Meia-largura (s) em que o pico precisa ser o máximo.
    pub meia_largura_pico_s: f64,
    /// Meia-largura (s) da média local do limiar.
    pub meia_largura_media_s: f64,
    /// Meia-largura (s) do máximo local usado no limiar.
    pub meia_largura_maximo_s: f64,
    /// Fração (do máximo local menos a média local) somada à média local para formar o limiar.
    pub delta_rel: f32,
    /// Fluxo mínimo absoluto para aceitar um pico.
    pub fluxo_minimo: f32,
    /// Correção sistemática somada a cada instante (s).
    pub correcao_s: f64,
}

impl Default for ConfigOnsets {
    fn default() -> Self {
        Self {
            janela_s: 0.046,
            hop_s: 0.0116,
            fmin_hz: 80.0,
            fmax_hz: 6000.0,
            compressao: 10.0,
            meia_largura_pico_s: 0.05,
            meia_largura_media_s: 0.3,
            meia_largura_maximo_s: 1.0,
            delta_rel: 0.15,
            fluxo_minimo: 0.05,
            correcao_s: 0.0,
        }
    }
}

/// Curva de fluxo espectral por quadro e o passo em segundos.
pub fn fluxo_espectral(audio: &Audio, cfg: &ConfigOnsets) -> (Vec<f32>, f64) {
    let taxa = f64::from(audio.taxa_hz);
    let hop = ((cfg.hop_s * taxa).round() as usize).max(1);
    let n_fft = proxima_potencia_de_dois((cfg.janela_s * taxa).round() as usize);
    let bin_hz = taxa / n_fft as f64;
    let k0 = ((cfg.fmin_hz / bin_hz).ceil() as usize).max(1);
    let k1 = ((cfg.fmax_hz / bin_hz).floor() as usize).min(n_fft / 2);
    let mut anterior = vec![0.0_f32; k1 + 1];
    let mut fluxo = Vec::new();
    para_cada_quadro(&audio.amostras, n_fft, hop, |t, mag| {
        let mut soma = 0.0_f32;
        for k in k0..=k1 {
            let atual = (1.0 + cfg.compressao * mag[k]).ln();
            if t > 0 {
                soma += (atual - anterior[k]).max(0.0);
            }
            anterior[k] = atual;
        }
        fluxo.push(soma);
    });
    (fluxo, hop as f64 / taxa)
}

/// Escolhe os picos de uma curva de fluxo. `passo_s` é o tempo entre amostras da curva.
pub fn escolher_picos(fluxo: &[f32], passo_s: f64, cfg: &ConfigOnsets) -> Vec<Onset> {
    let n = fluxo.len();
    let largura = |s: f64| ((s / passo_s).round() as usize).max(1);
    let (wp, wm, wx) = (
        largura(cfg.meia_largura_pico_s),
        largura(cfg.meia_largura_media_s),
        largura(cfg.meia_largura_maximo_s),
    );
    let mut saida: Vec<Onset> = Vec::new();
    for t in 1..n.saturating_sub(1) {
        let v = fluxo[t];
        if v < cfg.fluxo_minimo || v < fluxo[t - 1] || v <= fluxo[t + 1] {
            continue;
        }
        let a = t.saturating_sub(wp);
        let b = (t + wp + 1).min(n);
        if fluxo[a..b].iter().any(|x| *x > v) {
            continue;
        }
        let am = t.saturating_sub(wm);
        let bm = (t + wm + 1).min(n);
        let media = fluxo[am..bm].iter().sum::<f32>() / (bm - am) as f32;
        let ax = t.saturating_sub(wx);
        let bx = (t + wx + 1).min(n);
        let maximo = fluxo[ax..bx].iter().copied().fold(0.0_f32, f32::max);
        if v < media + cfg.delta_rel * (maximo - media) {
            continue;
        }
        // Interpolação parabólica. O fluxo do quadro t compara os quadros t-1 e t, então a
        // subida está meio passo antes do centro do quadro t.
        let (y0, y1, y2) = (fluxo[t - 1], v, fluxo[t + 1]);
        let denom = y0 - 2.0 * y1 + y2;
        let delta = if denom.abs() > 1e-12 {
            f64::from(0.5 * (y0 - y2) / denom).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let t_s = (t as f64 + delta - 0.5) * passo_s + cfg.correcao_s;
        saida.push(Onset {
            t_s: t_s.max(0.0),
            forca: v,
        });
    }
    saida
}

/// Onsets do áudio.
pub fn detectar_onsets(audio: &Audio, cfg: &ConfigOnsets) -> Vec<Onset> {
    let (fluxo, passo) = fluxo_espectral(audio, cfg);
    escolher_picos(&fluxo, passo, cfg)
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn escolhe_apenas_picos_acima_do_limiar() {
        let mut f = vec![0.0_f32; 100];
        f[20] = 1.0;
        f[19] = 0.4;
        f[21] = 0.4;
        f[60] = 0.8;
        f[40] = 0.02; // abaixo do mínimo absoluto
        f[80] = 0.06; // pico minúsculo perto de um grande não passa do limiar relativo
        f[81] = 0.0;
        let cfg = ConfigOnsets::default();
        let o = escolher_picos(&f, 0.0116, &cfg);
        assert_eq!(o.len(), 2, "{o:?}");
        assert!((o[0].t_s - 19.5 * 0.0116).abs() < 1e-9);
        assert!((o[1].t_s - 59.5 * 0.0116).abs() < 1e-3);
    }

    #[test]
    fn interpolacao_parabolica_desloca_o_pico() {
        let mut f = vec![0.0_f32; 50];
        f[20] = 1.0;
        f[21] = 0.8;
        f[19] = 0.0;
        let o = escolher_picos(&f, 0.01, &ConfigOnsets::default());
        assert_eq!(o.len(), 1);
        assert!(
            o[0].t_s > 19.5 * 0.01,
            "deve puxar para o lado do vizinho maior"
        );
    }

    fn nota_com_ataque(buf: &mut [f32], taxa: f64, hz: f64, ini_s: f64, dur_s: f64) {
        let a = (ini_s * taxa) as usize;
        let b = (((ini_s + dur_s) * taxa) as usize).min(buf.len());
        for (k, amostra) in buf[a..b].iter_mut().enumerate() {
            let t = k as f64 / taxa;
            let env =
                (-t / 0.4).exp() * (t / 0.004).min(1.0) * ((dur_s - t) / 0.03).clamp(0.0, 1.0);
            *amostra += (0.4 * env * (2.0 * PI * hz * t).sin()) as f32;
        }
    }

    #[test]
    fn detecta_notas_sinteticas_com_erro_pequeno() {
        let taxa = 22_050.0;
        let mut x = vec![0.0_f32; (4.0 * taxa) as usize];
        let inicios = [0.5, 1.2, 2.0, 2.9];
        for (k, t) in inicios.iter().enumerate() {
            nota_com_ataque(&mut x, taxa, 262.0 * 2.0_f64.powf(k as f64 / 6.0), *t, 0.6);
        }
        let audio = Audio {
            taxa_hz: 22_050,
            amostras: x,
        };
        let o = detectar_onsets(&audio, &ConfigOnsets::default());
        assert_eq!(o.len(), 4, "{o:?}");
        for (d, t) in o.iter().zip(inicios) {
            assert!((d.t_s - t).abs() < 0.03, "esperado {t}, achei {}", d.t_s);
        }
    }

    #[test]
    fn silencio_nao_tem_onsets() {
        let audio = Audio {
            taxa_hz: 22_050,
            amostras: vec![0.0; 22_050],
        };
        assert!(detectar_onsets(&audio, &ConfigOnsets::default()).is_empty());
    }
}
