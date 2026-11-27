//! A fonte musical **Bravura** (SMuFL, OFL-1.1) no render de produção (ADR-0018, ticket 19).
//!
//! Aqui entra só o que o render de produção usa **hoje**: a fonte registrada como família do egui
//! e os glifos de dedilhado (`fingering0` a `fingering5`). O resto do protótipo da ADR-0018
//! (claves, cabeças, pausas, acidentes...) continua no spike de testes, e as formas desenhadas à
//! mão do layout atual seguem como estão: este ticket não troca o render das notas.
//!
//! O arquivo é o oficial, inteiro e sem alteração (a OFL reserva o nome "Bravura"): ver
//! `assets/fontes/bravura/LEIA-ME.md`. As medidas dos glifos abaixo vêm de `Bravura.json`
//! (`glyphAdvanceWidths` e `glyphBBoxes`); um teste as confere contra o JSON.

use bancada_casca::egui::{FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

/// A Bravura 1.482 oficial (OTF).
pub const BRAVURA_OTF: &[u8] = include_bytes!("../../../../assets/fontes/bravura/Bravura.otf");

/// Nome da família no egui.
pub const FAMILIA: &str = "Bravura";

/// Corpo da fonte em sp: o SMuFL define o em como quatro espaços de pauta.
pub const CORPO_EM_SP: f32 = 4.0;

/// Escala do dedilhado (1,0 = o tamanho do SMuFL, 1 sp de altura de algarismo).
pub const ESCALA_DEDILHADO: f32 = 1.15;

/// Código do `fingering0` (U+ED10); `fingeringN` é U+ED10 + N.
const FINGERING0: u32 = 0xED10;

/// `glyphAdvanceWidths` de `fingering0` a `fingering5` (Bravura.json), em sp.
const AVANCO_DEDO: [f32; 6] = [1.02, 0.628, 0.968, 0.9, 0.944, 0.9];

/// O glifo SMuFL do dedo `d` (0 a 5); `None` fora disso (o piano usa 1 a 5).
pub fn glifo_do_dedo(d: u32) -> Option<char> {
    (d <= 5).then(|| char::from_u32(FINGERING0 + d))?
}

/// O glifo de um texto de um só algarismo de dedo (`"3"`), se existir.
pub fn glifo_do_texto_de_dedo(texto: &str) -> Option<char> {
    let mut cs = texto.chars();
    let (c, resto) = (cs.next()?, cs.next());
    if resto.is_some() {
        return None;
    }
    glifo_do_dedo(c.to_digit(10)?)
}

/// Largura de avanço de um glifo de dedilhado, em sp na escala 1,0 (1,0 para outros glifos).
pub fn avanco_do_glifo(c: char) -> f32 {
    let n = c as u32;
    if (FINGERING0..=FINGERING0 + 5).contains(&n) {
        AVANCO_DEDO[(n - FINGERING0) as usize]
    } else {
        1.0
    }
}

/// Altura de algarismo dos glifos de dedilhado (a caixa de `fingeringN` tem de 1,00 a 1,032 sp).
pub const ALTURA_DO_ALGARISMO: f32 = 1.04;

/// As definições de fonte do egui com a Bravura como família `Bravura`, além das padrão.
pub fn fontes_com_bravura() -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    defs.font_data.insert(
        FAMILIA.to_owned(),
        Arc::new(FontData::from_static(BRAVURA_OTF)),
    );
    defs.families
        .insert(FontFamily::Name(FAMILIA.into()), vec![FAMILIA.to_owned()]);
    defs
}

/// A família `Bravura` existe nas fontes do contexto (a casca a registra na partida; contextos de
/// teste sem ela caem no texto comum).
pub fn bravura_registrada(ctx: &bancada_casca::egui::Context) -> bool {
    ctx.fonts(|f| {
        f.definitions()
            .families
            .contains_key(&FontFamily::Name(FAMILIA.into()))
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn dedos_zero_a_cinco_tem_glifo_e_o_resto_nao() {
        assert_eq!(glifo_do_dedo(1), Some('\u{ED11}'));
        assert_eq!(glifo_do_dedo(5), Some('\u{ED15}'));
        assert_eq!(glifo_do_dedo(6), None);
        assert_eq!(glifo_do_texto_de_dedo("3"), Some('\u{ED13}'));
        assert_eq!(glifo_do_texto_de_dedo("p"), None);
        assert_eq!(glifo_do_texto_de_dedo("12"), None);
        assert_eq!(glifo_do_texto_de_dedo(""), None);
    }

    #[test]
    fn avancos_conferem_com_a_metadata_oficial() {
        let j: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../assets/fontes/bravura/Bravura.json"
        ))
        .expect("Bravura.json");
        for d in 0..=5u32 {
            let nome = format!("fingering{d}");
            let esperado = j["glyphAdvanceWidths"][nome.as_str()]
                .as_f64()
                .expect("avanço") as f32;
            let c = glifo_do_dedo(d).expect("glifo");
            assert!(
                (avanco_do_glifo(c) - esperado).abs() < 1e-4,
                "{nome}: {} contra {esperado}",
                avanco_do_glifo(c)
            );
            let topo = j["glyphBBoxes"][nome.as_str()]["bBoxNE"][1]
                .as_f64()
                .expect("topo") as f32;
            assert!(topo <= ALTURA_DO_ALGARISMO + 1e-4, "{nome}: topo {topo}");
        }
    }

    #[test]
    fn a_fonte_e_a_oficial_e_a_familia_e_registrada() {
        assert_eq!(&BRAVURA_OTF[..4], b"OTTO");
        let defs = fontes_com_bravura();
        assert!(
            defs.families
                .contains_key(&FontFamily::Name(FAMILIA.into()))
        );
        assert!(defs.font_data.contains_key(FAMILIA));
    }
}
