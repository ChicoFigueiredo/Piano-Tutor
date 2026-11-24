//! FFT radix-2 iterativa, em Rust puro (sem dependências), para as janelas do STFT.
//!
//! Escolha deliberada de não depender de `rustfft`: o alinhamento só precisa de tamanhos
//! potência de dois (1024 a 8192) e a implementação abaixo é conferida contra a DFT direta.

use std::f64::consts::PI;

/// Plano de FFT para um tamanho fixo (potência de dois, no mínimo 2).
#[derive(Clone, Debug)]
pub struct Fft {
    n: usize,
    cos: Vec<f64>,
    sin: Vec<f64>,
    inversao: Vec<usize>,
}

impl Fft {
    /// `None` se `n` não for potência de dois maior ou igual a 2.
    pub fn novo(n: usize) -> Option<Self> {
        if n < 2 || !n.is_power_of_two() {
            return None;
        }
        let bits = n.trailing_zeros();
        let inversao = (0..n)
            .map(|i| i.reverse_bits() >> (usize::BITS - bits))
            .collect();
        let cos = (0..n / 2)
            .map(|k| (-2.0 * PI * k as f64 / n as f64).cos())
            .collect();
        let sin = (0..n / 2)
            .map(|k| (-2.0 * PI * k as f64 / n as f64).sin())
            .collect();
        Some(Self {
            n,
            cos,
            sin,
            inversao,
        })
    }

    pub fn tamanho(&self) -> usize {
        self.n
    }

    /// Transformada direta, no lugar: `X[k] = Σ x[n]·e^(-2πikn/N)`.
    /// Os dois vetores precisam ter exatamente o tamanho do plano.
    pub fn transformar(&self, re: &mut [f64], im: &mut [f64]) {
        assert_eq!(re.len(), self.n, "parte real com tamanho errado");
        assert_eq!(im.len(), self.n, "parte imaginária com tamanho errado");
        for i in 0..self.n {
            let j = self.inversao[i];
            if j > i {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        let mut meio = 1;
        while meio < self.n {
            let passo = self.n / (meio * 2);
            for inicio in (0..self.n).step_by(meio * 2) {
                for k in 0..meio {
                    let (c, s) = (self.cos[k * passo], self.sin[k * passo]);
                    let (a, b) = (inicio + k, inicio + k + meio);
                    let tr = re[b] * c - im[b] * s;
                    let ti = re[b] * s + im[b] * c;
                    re[b] = re[a] - tr;
                    im[b] = im[a] - ti;
                    re[a] += tr;
                    im[a] += ti;
                }
            }
            meio *= 2;
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn rejeita_tamanhos_invalidos() {
        assert!(Fft::novo(0).is_none());
        assert!(Fft::novo(1).is_none());
        assert!(Fft::novo(12).is_none());
        assert!(Fft::novo(8).is_some());
    }

    #[test]
    fn confere_com_a_dft_direta() {
        let n = 16;
        let x: Vec<f64> = (0..n)
            .map(|i| (i as f64 * 0.7).sin() + 0.3 * (i as f64 * 2.1).cos() + (i % 3) as f64)
            .collect();
        let mut re = x.clone();
        let mut im = vec![0.0; n];
        Fft::novo(n)
            .expect("potência de dois")
            .transformar(&mut re, &mut im);
        for k in 0..n {
            let (mut er, mut ei) = (0.0, 0.0);
            for (i, v) in x.iter().enumerate() {
                let ang = -2.0 * PI * (k * i) as f64 / n as f64;
                er += v * ang.cos();
                ei += v * ang.sin();
            }
            assert!((re[k] - er).abs() < 1e-9, "re[{k}]");
            assert!((im[k] - ei).abs() < 1e-9, "im[{k}]");
        }
    }

    #[test]
    fn senoide_pura_cai_no_bin_certo() {
        let n = 64;
        let mut re: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * 5.0 * i as f64 / n as f64).cos())
            .collect();
        let mut im = vec![0.0; n];
        Fft::novo(n)
            .expect("potência de dois")
            .transformar(&mut re, &mut im);
        let mag = |k: usize| re[k].hypot(im[k]);
        assert!((mag(5) - 32.0).abs() < 1e-9);
        assert!(mag(4) < 1e-9 && mag(6) < 1e-9);
    }
}
