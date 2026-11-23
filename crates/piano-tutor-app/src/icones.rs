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
    fn fonte_e_um_truetype() {
        assert_eq!(&FONTE[..4], &[0, 1, 0, 0]);
    }
}
