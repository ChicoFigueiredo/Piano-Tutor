use super::IdNota;
use crate::partitura::Fracao;
use std::fmt;

/// Falhas explícitas do modelo de documento: nunca se arredonda nem se descarta em silêncio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroDocumento {
    /// XML malformado (mensagem do analisador).
    Xml(String),
    /// Bytes que não são UTF-8 (UTF-16, Latin-1 com acento): exigem normalização na importação.
    NaoUtf8(String),
    /// Estrutura fora do que o spike modela (por exemplo `score-timewise`).
    NaoSuportado(String),
    /// Um limite de segurança foi excedido (qual): tamanho, nós, profundidade, expansão.
    LimiteExcedido(&'static str),
    /// Faixa de um elemento fora do que o pai admite (típico de entidade que expande elementos).
    FaixaInconsistente(String),
    NotaInexistente(IdNota),
    NotaSemAltura(IdNota),
    AlturaInvalida(String),
    /// A duração não é múltipla inteira de `1/divisions`; `necessarias` é o MMC que a representa.
    DuracaoNaoRepresentavel {
        valor: Fracao,
        divisions: i64,
        necessarias: i64,
    },
    DuracaoInvalida(String),
    /// Conta que estouraria o inteiro (MMC de denominadores, por exemplo).
    Overflow(String),
    /// Patches aplicados a um texto que não é o texto-base.
    PatchesDeOutroTexto,
    PatchesInvalidos(String),
    /// Tabela lateral de ids gravada para outro texto.
    TabelaIdsDeOutroTexto,
    TabelaIdsInvalida(String),
}

impl fmt::Display for ErroDocumento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Xml(m) => write!(f, "XML inválido: {m}"),
            Self::LimiteExcedido(q) => write!(f, "limite de segurança excedido: {q}"),
            Self::NaoUtf8(m) => write!(f, "texto não é UTF-8: {m}"),
            Self::NaoSuportado(m) => write!(f, "não suportado: {m}"),
            Self::FaixaInconsistente(m) => write!(f, "faixa de bytes inconsistente: {m}"),
            Self::NotaInexistente(id) => write!(f, "nota {} não existe", id.0),
            Self::NotaSemAltura(id) => write!(f, "nota {} não tem altura editável", id.0),
            Self::AlturaInvalida(m) => write!(f, "altura inválida: {m}"),
            Self::DuracaoNaoRepresentavel {
                valor,
                divisions,
                necessarias,
            } => write!(
                f,
                "duração {valor} não cabe em divisions={divisions} (precisa de {necessarias})"
            ),
            Self::DuracaoInvalida(m) => write!(f, "duração inválida: {m}"),
            Self::Overflow(m) => write!(f, "estouro de inteiro: {m}"),
            Self::PatchesDeOutroTexto => write!(f, "patches não pertencem a este texto"),
            Self::PatchesInvalidos(m) => write!(f, "patches inválidos: {m}"),
            Self::TabelaIdsDeOutroTexto => write!(f, "tabela de ids é de outro texto"),
            Self::TabelaIdsInvalida(m) => write!(f, "tabela de ids inválida: {m}"),
        }
    }
}

impl ErroDocumento {
    /// Recusa de bytes que não são UTF-8, com mensagem em português (sem o texto do `Utf8Error`).
    pub fn nao_utf8(bytes: &[u8], e: &std::str::Utf8Error) -> Self {
        let dica = if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
            " (BOM de UTF-16)"
        } else {
            ""
        };
        Self::NaoUtf8(format!(
            "byte inválido na posição {}{dica}",
            e.valid_up_to()
        ))
    }
}

impl From<crate::xml_seguro::FalhaXml> for ErroDocumento {
    fn from(f: crate::xml_seguro::FalhaXml) -> Self {
        use crate::xml_seguro::FalhaXml;
        match f {
            FalhaXml::Invalido(m) => Self::Xml(m),
            FalhaXml::GrandeDemais(_) => Self::LimiteExcedido("tamanho do texto"),
            FalhaXml::Limite(q) => Self::LimiteExcedido(q),
        }
    }
}

impl std::error::Error for ErroDocumento {}
