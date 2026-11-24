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
    let mut mag = vec![0.0_f32; n_fft / 2 + 1];
    for t in 0..numero_de_quadros(amostras.len(), hop) {
        let inicio = (t * hop) as isize - (n_fft / 2) as isize;
        for i in 0..n_fft {
            let k = inicio + i as isize;
            let v = if k >= 0 && (k as usize) < amostras.len() {
                f64::from(amostras[k as usize])
            } else {
                0.0
            };
            re[i] = v * janela[i];
            im[i] = 0.0;
        }
        fft.transformar(&mut re, &mut im);
        for (k, m) in mag.iter_mut().enumerate() {
            *m = (re[k].hypot(im[k]) * escala) as f32;
        }
        f(t, &mag);
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
