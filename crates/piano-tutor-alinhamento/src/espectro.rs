//! STFT com janela de Hann, quadro a quadro (sem guardar o espectrograma inteiro).
//!
//! Os quadros são **centrados**: o quadro `t` está centrado na amostra `t * hop` (fora do áudio
//! vale zero), então o tempo do quadro é `t * hop / taxa`. A magnitude é normalizada para que uma
//! senoide de amplitude `A` caia em torno de `A` no bin mais próximo.

use crate::fft::Fft;
use std::f64::consts::PI;

/// Menor potência de dois com pelo menos `minimo` elementos (no mínimo 2).
pub fn proxima_potencia_de_dois(minimo: usize) -> usize {
    minimo.max(2).next_power_of_two()
}

/// Número de quadros que cobrem `n_amostras` com passo `hop` (quadros centrados em 0, hop, ...).
pub fn numero_de_quadros(n_amostras: usize, hop: usize) -> usize {
    if n_amostras == 0 || hop == 0 {
        0
    } else {
        n_amostras.div_ceil(hop)
    }
}

/// Chama `f(t, magnitudes)` para cada quadro `t`, com `n_fft / 2 + 1` magnitudes lineares.
/// Não faz nada se `n_fft` não for potência de dois ou `hop` for zero.
///
/// Dois quadros reais por FFT complexa (um na parte real, outro na imaginária); os espectros
/// se separam pela simetria hermitiana.
pub fn para_cada_quadro(
    amostras: &[f32],
    n_fft: usize,
    hop: usize,
    mut f: impl FnMut(usize, &[f32]),
) {
    let Some(fft) = Fft::novo(n_fft) else { return };
    if hop == 0 {
        return;
    }
    let janela: Vec<f64> = (0..n_fft)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / n_fft as f64).cos())
        .collect();
    let soma: f64 = janela.iter().sum();
    let escala = 2.0 / soma;
    let mut re = vec![0.0; n_fft];
    let mut im = vec![0.0; n_fft];
    let mut mag_a = vec![0.0_f32; n_fft / 2 + 1];
    let mut mag_b = vec![0.0_f32; n_fft / 2 + 1];
    let total = numero_de_quadros(amostras.len(), hop);
    let preencher = |destino: &mut [f64], t: usize| {
        let inicio = (t * hop) as isize - (n_fft / 2) as isize;
        for (i, d) in destino.iter_mut().enumerate() {
            let k = inicio + i as isize;
            let v = if k >= 0 && (k as usize) < amostras.len() {
                f64::from(amostras[k as usize])
            } else {
                0.0
            };
            *d = v * janela[i];
        }
    };
    let mut t = 0;
    while t < total {
        let tem_b = t + 1 < total;
        preencher(&mut re, t);
        if tem_b {
            preencher(&mut im, t + 1);
        } else {
            im.fill(0.0);
        }
        fft.transformar(&mut re, &mut im);
        for k in 0..=n_fft / 2 {
            let k2 = (n_fft - k) % n_fft;
            // A[k] = (Z[k] + conj(Z[N-k])) / 2 ; B[k] = (Z[k] - conj(Z[N-k])) / (2i)
            let (ar, ai) = (0.5 * (re[k] + re[k2]), 0.5 * (im[k] - im[k2]));
            let (br, bi) = (0.5 * (im[k] + im[k2]), -0.5 * (re[k] - re[k2]));
            mag_a[k] = (ar.hypot(ai) * escala) as f32;
            mag_b[k] = (br.hypot(bi) * escala) as f32;
        }
        f(t, &mag_a);
        if tem_b {
            f(t + 1, &mag_b);
        }
        t += 2;
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn senoide(hz: f64, taxa: f64, n: usize, amp: f64) -> Vec<f32> {
        (0..n)
            .map(|i| (amp * (2.0 * PI * hz * i as f64 / taxa).sin()) as f32)
            .collect()
    }

    #[test]
    fn senoide_tem_magnitude_proxima_da_amplitude_no_bin_certo() {
        let taxa = 8192.0;
        // 1024 Hz com n_fft 1024 e taxa 8192: bin 128 exato.
        let x = senoide(1024.0, taxa, 8192, 0.5);
        let mut visto = false;
        para_cada_quadro(&x, 1024, 512, |t, mag| {
            if t == 8 {
                visto = true;
                assert!((mag[128] - 0.5).abs() < 0.01, "mag = {}", mag[128]);
                assert!(mag[200] < 0.001);
            }
        });
        assert!(visto);
    }

    #[test]
    fn dois_quadros_por_fft_igualam_a_dft_direta() {
        // Soma de duas senoides em bins exatos; quadro 3 (ímpar) e 4 (par) devem concordar
        // com a conta direta de uma DFT sobre o quadro janelado.
        let n_fft = 64;
        let x: Vec<f32> = (0..400)
            .map(|i| {
                ((2.0 * PI * 5.0 * i as f64 / 64.0).sin() * 0.7
                    + (2.0 * PI * 11.0 * i as f64 / 64.0).cos() * 0.2) as f32
            })
            .collect();
        let hop = 16;
        let janela: Vec<f64> = (0..n_fft)
            .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / n_fft as f64).cos())
            .collect();
        let escala = 2.0 / janela.iter().sum::<f64>();
        let mut conferidos = 0;
        para_cada_quadro(&x, n_fft, hop, |t, mag| {
            let inicio = (t * hop) as isize - 32;
            for k in [0, 5, 11, 20, 32] {
                let (mut er, mut ei) = (0.0, 0.0);
                for (i, w) in janela.iter().enumerate() {
                    let p = inicio + i as isize;
                    let v = if p >= 0 && (p as usize) < x.len() {
                        f64::from(x[p as usize])
                    } else {
                        0.0
                    };
                    let ang = -2.0 * PI * (k * i) as f64 / n_fft as f64;
                    er += v * w * ang.cos();
                    ei += v * w * ang.sin();
                }
                let esperado = er.hypot(ei) * escala;
                assert!(
                    (f64::from(mag[k]) - esperado).abs() < 1e-5,
                    "quadro {t}, bin {k}: {} vs {esperado}",
                    mag[k]
                );
            }
            conferidos += 1;
        });
        assert_eq!(conferidos, 25);
    }

    #[test]
    fn conta_quadros_e_ignora_entradas_invalidas() {
        assert_eq!(numero_de_quadros(1000, 256), 4);
        assert_eq!(numero_de_quadros(0, 256), 0);
        let mut n = 0;
        para_cada_quadro(&[0.0; 1000], 1000, 256, |_, _| n += 1);
        para_cada_quadro(&[0.0; 1000], 1024, 0, |_, _| n += 1);
        assert_eq!(n, 0);
        para_cada_quadro(&[0.0; 1000], 1024, 256, |_, m| {
            assert_eq!(m.len(), 513);
            n += 1;
        });
        assert_eq!(n, 4);
    }
}
