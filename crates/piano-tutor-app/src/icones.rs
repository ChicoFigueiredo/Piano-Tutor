//! Ícones próprios do Piano Tutor: SVGs em `assets/icones/svg` (grade 24×24, traço 1,5)
//! convertidos numa fonte monocromática por `scripts/gerar-icones.py`. A fonte entra na
//! `bancada-casca` como família de ícones, então Ribbon, navegação e status desenham os glifos na
//! cor do tema. O mapa nome → código vem do `mapa.txt` gerado junto com a fonte.

use bancada_casca::FamiliaDeIcones;

/// A fonte gerada (glifos na área de uso privado, U+E000 em diante).
pub const FONTE: &[u8] = include_bytes!("../../../assets/icones/piano-tutor-icones.ttf");
/// `nome<TAB>código hexadecimal`, uma linha por ícone.
pub const MAPA: &str = include_str!("../../../assets/icones/mapa.txt");

/// Pares (nome, glifo) do mapa. Linhas malformadas são ignoradas.
pub fn pares() -> impl Iterator<Item = (&'static str, char)> {
    MAPA.lines().filter_map(|l| {
        let (nome, cod) = l.split_once('\t')?;
        let c = char::from_u32(u32::from_str_radix(cod.trim(), 16).ok()?)?;
        Some((nome.trim(), c))
    })
}

/// O glifo de um ícone, como texto (vazio se o nome não existe).
pub fn glifo(nome: &str) -> String {
    pares()
        .find(|(n, _)| *n == nome)
        .map(|(_, c)| c.to_string())
        .unwrap_or_default()
}

/// A família de ícones para a `ConfigCasca`, com os nomes que a casca usa ("voltar",
/// "avancar") já incluídos.
pub fn familia() -> FamiliaDeIcones {
    let glifos: Vec<(String, String)> = pares()
        .map(|(n, c)| (n.to_owned(), c.to_string()))
        .collect();
    glifos
        .iter()
        .fold(FamiliaDeIcones::com_fonte(FONTE.to_vec()), |f, (n, g)| {
            f.glifo(n.as_str(), g.as_str())
        })
}

/// Ícone da janela (128×128 RGBA), desenhado em código com a mesma geometria de
/// `assets/icones/piano-tutor.svg` (grade 256): quadrado azul arredondado com teclado branco.
pub fn icone_janela() -> bancada_casca::egui::IconData {
    const L: usize = 128;
    const SS: usize = 4; // superamostragem por eixo
    let mut rgba = vec![0u8; L * L * 4];
    for y in 0..L {
        for x in 0..L {
            let (mut fundo, mut branco) = (0u32, 0u32);
            for sy in 0..SS {
                for sx in 0..SS {
                    // Coordenadas na grade 256 do SVG.
                    let gx = (x as f32 + (sx as f32 + 0.5) / SS as f32) * 2.0;
                    let gy = (y as f32 + (sy as f32 + 0.5) / SS as f32) * 2.0;
                    if dentro_arredondado(gx, gy, 16.0, 16.0, 224.0, 224.0, 52.0) {
                        fundo += 1;
                        if branco_do_logo(gx, gy) {
                            branco += 1;
                        }
                    }
                }
            }
            let total = (SS * SS) as f32;
            let a = fundo as f32 / total;
            let w = if fundo == 0 {
                0.0
            } else {
                branco as f32 / fundo as f32
            };
            let t = y as f32 / L as f32;
            let azul = [
                90.0 + (32.0 - 90.0) * t,
                170.0 + (121.0 - 170.0) * t,
                247.0 + (214.0 - 247.0) * t,
            ];
            let i = (y * L + x) * 4;
            for c in 0..3 {
                rgba[i + c] = (azul[c] + (255.0 - azul[c]) * w).round() as u8;
            }
            rgba[i + 3] = (a * 255.0).round() as u8;
        }
    }
    bancada_casca::egui::IconData {
        rgba,
        width: L as u32,
        height: L as u32,
    }
}

fn dentro_arredondado(x: f32, y: f32, x0: f32, y0: f32, l: f32, a: f32, r: f32) -> bool {
    let cx = x.clamp(x0 + r, x0 + l - r);
    let cy = y.clamp(y0 + r, y0 + a - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

fn branco_do_logo(x: f32, y: f32) -> bool {
    // Contorno do teclado (traço 10 em volta de 52,78 152×108 r12).
    let fora = dentro_arredondado(x, y, 47.0, 73.0, 162.0, 118.0, 17.0);
    let dentro = dentro_arredondado(x, y, 57.0, 83.0, 142.0, 98.0, 7.0);
    let contorno = fora && !dentro;
    let pretas = [80.0, 120.0, 160.0]
        .iter()
        .any(|&px| x >= px && x <= px + 16.0 && (78.0..=140.0).contains(&y));
    let divisorias = [88.0, 128.0, 168.0]
        .iter()
        .any(|&dx| (x - dx).abs() <= 4.0 && (136.0..=190.0).contains(&y));
    contorno || pretas || divisorias
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn mapa_tem_os_icones_usados_pelo_app_e_pela_casca() {
        for nome in [
            "inicio",
            "midi",
            "teclado",
            "calibrar",
            "exercicios",
            "acordes",
            "ritmos",
            "gravar",
            "partitura",
            "timbres",
            "panico",
            "sobre",
            "tema",
            "voltar",
            "avancar",
            "buscar",
            "piano",
            "expandir",
        ] {
            assert!(!glifo(nome).is_empty(), "falta o ícone {nome}");
        }
        let codigos: std::collections::HashSet<char> = pares().map(|(_, c)| c).collect();
        assert_eq!(codigos.len(), pares().count(), "códigos únicos");
        assert!(pares().all(|(_, c)| ('\u{E000}'..='\u{F8FF}').contains(&c)));
    }

    #[test]
    fn icone_da_janela_tem_canto_transparente_e_centro_opaco() {
        let i = icone_janela();
        assert_eq!((i.width, i.height, i.rgba.len()), (128, 128, 128 * 128 * 4));
        assert_eq!(i.rgba[3], 0, "canto transparente");
        let centro = (64 * 128 + 64) * 4;
        assert_eq!(i.rgba[centro + 3], 255);
    }

    #[test]
    fn fonte_e_um_truetype() {
        assert_eq!(&FONTE[..4], &[0, 1, 0, 0]);
    }
}
