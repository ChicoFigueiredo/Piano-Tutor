//! Erros do Pacote de Curso, todos com mensagem em português.

use std::fmt;

/// Tudo que pode dar errado ao ler, verificar, empacotar ou instalar um Pacote de Curso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroPacote {
    /// Falha de leitura ou gravação no disco.
    Io(String),
    /// O arquivo não é um zip válido ou tem entradas que o formato não admite.
    ZipInvalido(String),
    /// Algum limite de tamanho ou de quantidade foi excedido.
    LimiteExcedido(String),
    /// Não há `manifesto.toml` no pacote.
    ManifestoAusente,
    /// Não há `assinatura.ed25519` no pacote.
    AssinaturaAusente,
    /// A assinatura não tem o formato esperado (64 bytes).
    AssinaturaMalformada(String),
    /// O manifesto (ou outro documento) não pôde ser lido.
    ManifestoInvalido(String),
    /// O pacote foi feito para uma versão mais nova do formato.
    FormatoMaisNovo { encontrado: u32, suportado: u32 },
    /// A assinatura não confere com os bytes do manifesto.
    AssinaturaInvalida,
    /// Caminho com `..`, absoluto, com barra invertida, dois-pontos etc.
    CaminhoInseguro(String),
    /// Arquivo presente no zip e ausente de `[[arquivos]]`.
    ArquivoNaoListado { caminho: String },
    /// Arquivo listado em `[[arquivos]]` e ausente do zip.
    ArquivoAusente { caminho: String },
    /// O manifesto usa um arquivo que não está em `[[arquivos]]`.
    ArquivoReferenciadoNaoListado {
        caminho: String,
        referenciado_por: String,
    },
    /// O SHA-256 do arquivo não confere com o manifesto.
    HashDivergente { caminho: String },
    /// O tamanho do arquivo não confere com o manifesto.
    TamanhoDivergente { caminho: String },
    /// Identificador fora do padrão `[a-z0-9.-]`.
    IdInvalido { tipo: &'static str, id: String },
    /// Identificador repetido.
    IdDuplicado { tipo: &'static str, id: String },
    /// Qualquer outra inconsistência estrutural.
    Estrutura(String),
    /// Chave ed25519 ilegível ou malformada.
    ChaveInvalida(String),
    /// O curso já instalado foi assinado por outra chave.
    AutorDiferente { curso: String },
    /// Plano ou oficina não permitem gerar a pasta-origem (ver `importacao`).
    Importacao(String),
}

impl fmt::Display for ErroPacote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(m) => write!(f, "erro de arquivo: {m}"),
            Self::ZipInvalido(m) => write!(f, "o pacote não é um arquivo zip válido: {m}"),
            Self::LimiteExcedido(m) => write!(f, "limite excedido: {m}"),
            Self::ManifestoAusente => write!(f, "o pacote não tem manifesto.toml"),
            Self::AssinaturaAusente => write!(f, "o pacote não tem assinatura.ed25519"),
            Self::AssinaturaMalformada(m) => write!(f, "assinatura malformada: {m}"),
            Self::ManifestoInvalido(m) => write!(f, "documento inválido: {m}"),
            Self::FormatoMaisNovo {
                encontrado,
                suportado,
            } => write!(
                f,
                "pacote feito para versão mais nova do app (formato {encontrado}; este app entende até o formato {suportado})"
            ),
            Self::AssinaturaInvalida => write!(
                f,
                "assinatura inválida: o manifesto foi alterado ou não foi assinado pela chave que declara"
            ),
            Self::CaminhoInseguro(c) => write!(f, "caminho inseguro no pacote: {c:?}"),
            Self::ArquivoNaoListado { caminho } => write!(
                f,
                "arquivo {caminho:?} está no pacote mas não consta em [[arquivos]] do manifesto"
            ),
            Self::ArquivoAusente { caminho } => {
                write!(
                    f,
                    "arquivo {caminho:?} consta no manifesto mas não está no pacote"
                )
            }
            Self::ArquivoReferenciadoNaoListado {
                caminho,
                referenciado_por,
            } => write!(
                f,
                "{referenciado_por} usa o arquivo {caminho:?}, que não consta em [[arquivos]]"
            ),
            Self::HashDivergente { caminho } => {
                write!(f, "o SHA-256 de {caminho:?} não confere com o manifesto")
            }
            Self::TamanhoDivergente { caminho } => {
                write!(f, "o tamanho de {caminho:?} não confere com o manifesto")
            }
            Self::IdInvalido { tipo, id } => write!(
                f,
                "identificador de {tipo} inválido: {id:?} (use a-z, 0-9, ponto e hífen, começando por letra ou número)"
            ),
            Self::IdDuplicado { tipo, id } => write!(f, "identificador de {tipo} repetido: {id:?}"),
            Self::Estrutura(m) => write!(f, "estrutura inválida: {m}"),
            Self::ChaveInvalida(m) => write!(f, "chave inválida: {m}"),
            Self::AutorDiferente { curso } => write!(
                f,
                "o curso {curso:?} já está instalado e foi assinado por outra chave; remova o curso instalado antes de trocar de autor"
            ),
            Self::Importacao(m) => write!(f, "importação: {m}"),
        }
    }
}

impl std::error::Error for ErroPacote {}

impl From<std::io::Error> for ErroPacote {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}
