//! Corpus de fixtures MusicXML com licença e origem registradas.
//!
//! O índice legível está em `fixtures/INDICE.md` (manifesto de licenças, com SHA-256). Um teste
//! confere que cada fixture do corpus consta no índice com o SHA-256 correto e que todo arquivo
//! de `fixtures/` está no corpus. Material comprado nunca entra (T2).

use crate::gerador::gerar_cem_compassos;

/// Origem e licença de uma fixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origem {
    /// Escrita à mão pelo projeto para este corpus (CC0 1.0).
    Propria,
    /// Gerada por script versionado (`gerador`), CC0 1.0.
    Gerada,
    /// OpenScore Lieder Corpus (CC0 1.0), obra em domínio público; já presente em `assets/`.
    OpenScore,
    /// Gravada pelo projeto a partir de obra de domínio público (CC0 1.0), em `assets/`.
    ProjetoAssets,
}

impl Origem {
    pub const fn descricao(self) -> &'static str {
        match self {
            Self::Propria => "própria (escrita à mão), CC0 1.0",
            Self::Gerada => "gerada por script versionado, CC0 1.0",
            Self::OpenScore => "OpenScore Lieder Corpus, CC0 1.0 (obra em domínio público)",
            Self::ProjetoAssets => "gravada pelo projeto, CC0 1.0 (obra em domínio público)",
        }
    }
}

/// Uma fixture do corpus.
#[derive(Clone, Debug)]
pub struct Fixture {
    /// Identificador estável (usado pelos testes dos tickets seguintes).
    pub id: &'static str,
    /// Caminho relativo à raiz do repositório.
    pub arquivo: &'static str,
    pub origem: Origem,
    /// Casos críticos cobertos (rótulos livres: "anacruse", "letra", ...).
    pub casos: &'static [&'static str],
    pub conteudo: String,
}

macro_rules! propria {
    ($id:literal, $arq:literal, [$($caso:literal),*]) => {
        Fixture {
            id: $id,
            arquivo: concat!("crates/piano-tutor-fixtures/fixtures/", $arq),
            origem: Origem::Propria,
            casos: &[$($caso),*],
            conteudo: include_str!(concat!("../fixtures/", $arq)).to_owned(),
        }
    };
}

macro_rules! de_assets {
    ($id:literal, $origem:expr, $arq:literal, [$($caso:literal),*]) => {
        Fixture {
            id: $id,
            arquivo: concat!("assets/partituras/", $arq),
            origem: $origem,
            casos: &[$($caso),*],
            conteudo: include_str!(concat!("../../../assets/partituras/", $arq)).to_owned(),
        }
    };
}

/// O corpus completo.
pub fn corpus() -> Vec<Fixture> {
    vec![
        propria!("anacruse", "anacruse.musicxml", ["anacruse"]),
        propria!(
            "duas-vozes",
            "duas-vozes.musicxml",
            ["duas vozes numa pauta"]
        ),
        propria!("tercina", "tercina.musicxml", ["tercina"]),
        propria!(
            "repeticao",
            "repeticao.musicxml",
            ["repetição", "casas 1 e 2"]
        ),
        propria!("letra", "letra.musicxml", ["letra", "dois versos"]),
        propria!("cifra", "cifra.musicxml", ["cifra", "baixo alternativo"]),
        propria!("dedilhado", "dedilhado.musicxml", ["dedilhado"]),
        propria!("dinamica", "dinamica.musicxml", ["dinâmica", "crescendo"]),
        propria!("ornamento", "ornamento.musicxml", ["ornamento", "fermata"]),
        propria!(
            "desconhecido",
            "desconhecido.musicxml",
            ["elemento sem suporte de renderização"]
        ),
        Fixture {
            id: "cem-compassos",
            arquivo: "crates/piano-tutor-fixtures/src/gerador.rs (gerar_cem_compassos)",
            origem: Origem::Gerada,
            casos: &["peça de 100 compassos"],
            conteudo: gerar_cem_compassos(),
        },
        de_assets!(
            "brahms-wiegenlied",
            Origem::OpenScore,
            "brahms-wiegenlied-op49-4.musicxml",
            ["letra", "voz e piano", "OpenScore"]
        ),
        de_assets!(
            "schubert-heidenroeslein",
            Origem::OpenScore,
            "schubert-heidenroeslein-d257.musicxml",
            ["letra", "voz e piano", "OpenScore"]
        ),
        de_assets!(
            "faure-barcarolle",
            Origem::OpenScore,
            "faure-barcarolle-op7-3.musicxml",
            ["letra", "voz e piano", "6/8", "OpenScore"]
        ),
        de_assets!(
            "beethoven-ode",
            Origem::ProjetoAssets,
            "beethoven-ode-a-alegria-tema.musicxml",
            ["duas pautas", "andamento"]
        ),
        de_assets!(
            "brilha-brilha",
            Origem::ProjetoAssets,
            "tradicional-brilha-brilha-estrelinha.musicxml",
            ["duas pautas"]
        ),
    ]
}

/// SHA-256 em hexadecimal minúsculo.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Busca uma fixture pelo id (pânico com mensagem clara se não existir: erro de teste).
pub fn fixture(id: &str) -> Fixture {
    corpus()
        .into_iter()
        .find(|f| f.id == id)
        .unwrap_or_else(|| panic!("fixture '{id}' não existe no corpus"))
}
