//! Preferências de som salvas entre aberturas (timbre, volume, divisão, camada,
//! transposição, efeitos) em `~/.local/share/piano-tutor/preferencias.txt`.

use crate::som::{ConfigTeclado, Timbre};
use std::fmt::Write as _;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreferenciasSom {
    pub timbre: Timbre,
    pub volume: f32,
    pub teclado: ConfigTeclado,
}

fn timbre_texto(t: Timbre) -> String {
    format!("{}:{}", t.banco, t.programa)
}

fn timbre_de(s: &str) -> Option<Timbre> {
    let (b, p) = s.split_once(':')?;
    Some(Timbre {
        banco: b.trim().parse().ok()?,
        programa: p.trim().parse().ok().filter(|p: &u8| *p <= 127)?,
    })
}

impl PreferenciasSom {
    pub fn para_texto(&self) -> String {
        let c = &self.teclado;
        let mut s = String::from("# Piano Tutor: preferências de som v1\n");
        let _ = writeln!(s, "timbre={}", timbre_texto(self.timbre));
        let _ = writeln!(s, "volume={:.3}", self.volume);
        if let Some(d) = c.divisao {
            let _ = writeln!(s, "divisao={d}");
        }
        let _ = writeln!(s, "esquerda={}", timbre_texto(c.timbre_esquerda));
        if let Some(t) = c.camada {
            let _ = writeln!(s, "camada={}", timbre_texto(t));
        }
        let _ = writeln!(s, "transposicao={}", c.transposicao);
        let _ = writeln!(s, "reverb={}", c.reverb);
        let _ = writeln!(s, "coro={}", c.coro);
        s
    }

    /// Lê o texto; chaves desconhecidas ou valores inválidos ficam no padrão.
    pub fn de_texto(t: &str) -> Self {
        let mut p = PreferenciasSom {
            timbre: Timbre::PIANO,
            volume: crate::som::VOLUME_PADRAO,
            teclado: ConfigTeclado::default(),
        };
        for l in t.lines() {
            let Some((k, v)) = l.trim().split_once('=') else {
                continue;
            };
            let v = v.trim();
            match k.trim() {
                "timbre" => p.timbre = timbre_de(v).unwrap_or(p.timbre),
                "volume" => {
                    p.volume = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map_or(p.volume, |x| x.clamp(0.0, 1.0));
                }
                "divisao" => p.teclado.divisao = v.parse().ok().filter(|d: &u8| *d <= 127),
                "esquerda" => {
                    p.teclado.timbre_esquerda = timbre_de(v).unwrap_or(p.teclado.timbre_esquerda);
                }
                "camada" => p.teclado.camada = timbre_de(v),
                "transposicao" => {
                    p.teclado.transposicao = v
                        .parse::<i8>()
                        .ok()
                        .filter(|x| (-24..=24).contains(x))
                        .unwrap_or(0);
                }
                "reverb" => {
                    p.teclado.reverb = v.parse().map_or(p.teclado.reverb, |x: u8| x.min(127))
                }
                "coro" => p.teclado.coro = v.parse().map_or(p.teclado.coro, |x: u8| x.min(127)),
                _ => {}
            }
        }
        p
    }

    pub fn ler(caminho: &Path) -> Option<Self> {
        let t = std::fs::read_to_string(caminho).ok()?;
        (t.len() <= 16 * 1024).then(|| Self::de_texto(&t))
    }

    pub fn gravar(&self, caminho: &Path) -> std::io::Result<()> {
        if let Some(p) = caminho.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(caminho, self.para_texto())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ida_e_volta() {
        let p = PreferenciasSom {
            timbre: Timbre {
                banco: 0,
                programa: 4,
            },
            volume: 0.75,
            teclado: ConfigTeclado {
                divisao: Some(55),
                camada: Some(Timbre {
                    banco: 0,
                    programa: 48,
                }),
                transposicao: -2,
                reverb: 60,
                coro: 10,
                ..ConfigTeclado::default()
            },
        };
        assert_eq!(PreferenciasSom::de_texto(&p.para_texto()), p);
    }

    #[test]
    fn lixo_vira_padrao() {
        let p = PreferenciasSom::de_texto("timbre=x\nvolume=NaN\ntransposicao=99\nreverb=abc");
        assert_eq!(p.timbre, Timbre::PIANO);
        assert_eq!(p.teclado.transposicao, 0);
    }
}
