//! Erros recuperáveis do motor de áudio. Nenhum caminho do motor entra em
//! pânico por falha de dispositivo, de SoundFont ou de fila cheia.

use std::fmt;

/// Erro recuperável do motor de áudio.
#[derive(Debug)]
pub enum ErroAudio {
    /// Nenhum dispositivo de saída padrão disponível no host.
    SemDispositivo,
    /// O dispositivo existe, mas não aceitou a configuração pedida.
    ConfiguracaoNaoSuportada(String),
    /// Formato de amostra do dispositivo que o motor não converte.
    FormatoNaoSuportado(String),
    /// Falha ao criar ou iniciar o fluxo de saída.
    Dispositivo(String),
    /// Arquivo SF2 ausente, ilegível ou inválido.
    SoundFontInvalido(String),
    /// Parâmetros do sintetizador fora da faixa aceita.
    Sintetizador(String),
    /// Parâmetro de configuração inválido (taxa zero, bloco zero etc.).
    ParametroInvalido(&'static str),
    /// Fila de comandos cheia: o comando não foi enviado.
    FilaCheia,
    /// Erro de E/S ao gravar arquivo (por exemplo WAV de teste).
    Io(std::io::Error),
}

impl fmt::Display for ErroAudio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemDispositivo => write!(f, "nenhum dispositivo de saída de áudio padrão"),
            Self::ConfiguracaoNaoSuportada(m) => {
                write!(f, "configuração de áudio não suportada: {m}")
            }
            Self::FormatoNaoSuportado(m) => write!(f, "formato de amostra não suportado: {m}"),
            Self::Dispositivo(m) => write!(f, "falha no dispositivo de áudio: {m}"),
            Self::SoundFontInvalido(m) => write!(f, "SoundFont inválido: {m}"),
            Self::Sintetizador(m) => write!(f, "sintetizador: {m}"),
            Self::ParametroInvalido(m) => write!(f, "parâmetro inválido: {m}"),
            Self::FilaCheia => write!(f, "fila de comandos de áudio cheia"),
            Self::Io(e) => write!(f, "E/S: {e}"),
        }
    }
}

impl std::error::Error for ErroAudio {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ErroAudio {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
