//! Preferências de som salvas entre aberturas (timbre, volume, divisão, camada,
//! transposição, efeitos) em `~/.local/share/piano-tutor/preferencias.txt`.

use crate::dedos::NomenclaturaDedos;
use crate::partitura_visual::{ModoRolagem, ZOOM_MAX, ZOOM_MIN};
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

/// Nome do arquivo das preferências da tela Partitura (na pasta de dados, ao lado das de som;
/// arquivo separado para que a gravação das preferências de som não o sobrescreva).
pub const ARQUIVO_PREFERENCIAS_PARTITURA: &str = "preferencias-partitura.txt";

/// Preferências de exibição da tela Partitura.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreferenciasPartitura {
    pub modo: ModoRolagem,
    pub papel_branco: bool,
    pub seguir: bool,
    pub zoom: f32,
}

impl Default for PreferenciasPartitura {
    fn default() -> Self {
        Self {
            modo: ModoRolagem::Sistemas,
            papel_branco: false,
            seguir: true,
            zoom: 1.0,
        }
    }
}

impl PreferenciasPartitura {
    pub fn para_texto(&self) -> String {
        let mut s = String::from("# Piano Tutor: preferências da partitura v1\n");
        let _ = writeln!(s, "rolagem={}", self.modo.chave());
        let _ = writeln!(s, "papel_branco={}", u8::from(self.papel_branco));
        let _ = writeln!(s, "seguir={}", u8::from(self.seguir));
        let _ = writeln!(s, "zoom={:.3}", self.zoom);
        s
    }

    /// Lê o texto; chaves desconhecidas ou valores inválidos ficam no padrão.
    pub fn de_texto(t: &str) -> Self {
        let mut p = Self::default();
        let booleano = |v: &str, atual: bool| match v {
            "1" | "true" => true,
            "0" | "false" => false,
            _ => atual,
        };
        for l in t.lines() {
            let Some((k, v)) = l.trim().split_once('=') else {
                continue;
            };
            let v = v.trim();
            match k.trim() {
                "rolagem" => p.modo = ModoRolagem::de_chave(v).unwrap_or(p.modo),
                "papel_branco" => p.papel_branco = booleano(v, p.papel_branco),
                "seguir" => p.seguir = booleano(v, p.seguir),
                "zoom" => {
                    p.zoom = v
                        .parse::<f32>()
                        .ok()
                        .filter(|x| x.is_finite())
                        .map_or(p.zoom, |x| x.clamp(ZOOM_MIN, ZOOM_MAX));
                }
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

/// Nome do arquivo das preferências de exibição gerais (por enquanto, a nomenclatura dos
/// dedos). Separado do de som pelo mesmo motivo do da Partitura.
pub const ARQUIVO_PREFERENCIAS_EXIBICAO: &str = "preferencias-exibicao.txt";

/// Preferências de exibição que valem em todas as telas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreferenciasExibicao {
    pub dedos: NomenclaturaDedos,
    /// Mapa de dedos para decorar em todas as teclas: tônica maior (0 = Dó) ou desligado.
    pub mapa_dedos: Option<u8>,
}

impl PreferenciasExibicao {
    pub fn para_texto(&self) -> String {
        let mut s = String::from("# Piano Tutor: preferências de exibição v1\n");
        let _ = writeln!(s, "dedos={}", self.dedos.chave());
        match self.mapa_dedos {
            Some(t) => {
                let _ = writeln!(s, "mapa_dedos={t}");
            }
            None => {
                let _ = writeln!(s, "mapa_dedos=nao");
            }
        }
        s
    }

    /// Lê o texto; chaves desconhecidas ou valores inválidos ficam no padrão.
    pub fn de_texto(t: &str) -> Self {
        let mut p = Self::default();
        for l in t.lines() {
            match l.trim().split_once('=').map(|(k, v)| (k.trim(), v.trim())) {
                Some(("dedos", v)) => p.dedos = NomenclaturaDedos::de_chave(v).unwrap_or(p.dedos),
                Some(("mapa_dedos", v)) => {
                    p.mapa_dedos = v.parse::<u8>().ok().filter(|t| *t < 12);
                }
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
    fn exibicao_ida_e_volta_e_lixo_vira_padrao() {
        for dedos in crate::dedos::NOMENCLATURAS {
            for mapa_dedos in [None, Some(0), Some(11)] {
                let p = PreferenciasExibicao { dedos, mapa_dedos };
                assert_eq!(PreferenciasExibicao::de_texto(&p.para_texto()), p);
            }
        }
        assert_eq!(
            PreferenciasExibicao::de_texto("dedos=klingon\nx=1"),
            PreferenciasExibicao::default()
        );
    }

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

    #[test]
    fn partitura_ida_e_volta_padrao_e_lixo() {
        let p = PreferenciasPartitura {
            modo: ModoRolagem::Fita,
            papel_branco: true,
            seguir: false,
            zoom: 1.5,
        };
        assert_eq!(PreferenciasPartitura::de_texto(&p.para_texto()), p);
        // Arquivo antigo ou vazio: padrões; chaves desconhecidas e valores inválidos ignorados.
        assert_eq!(
            PreferenciasPartitura::de_texto(""),
            PreferenciasPartitura::default()
        );
        let q = PreferenciasPartitura::de_texto(
            "futuro=1\nrolagem=xyz\npapel_branco=talvez\nseguir=0\nzoom=NaN",
        );
        assert_eq!(q.modo, ModoRolagem::Sistemas);
        assert!(!q.papel_branco);
        assert!(!q.seguir);
        assert_eq!(q.zoom, 1.0);
        assert_eq!(PreferenciasPartitura::de_texto("zoom=99").zoom, ZOOM_MAX);
    }
}
