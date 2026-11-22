//! Abstração da origem das portas MIDI de entrada.
//!
//! [`FontePortas`] separa o gerenciador do backend real (`midir`/ALSA) para que
//! conexão, hot-plug e perda de mensagens sejam testáveis sem hardware.

use crate::relogio::Relogio;
use std::fmt;
use std::sync::Arc;

/// Callback chamado pela fonte a cada mensagem MIDI recebida, com os bytes e o
/// timestamp monotônico em ns (do [`Relogio`] passado em `conectar`). Roda na
/// thread do backend: deve ser curto e nunca entrar em pânico.
pub type CallbackBytes = Box<dyn FnMut(&[u8], u64) + Send + 'static>;

/// Conexão aberta. Fechar = soltar (`Drop`).
pub trait ConexaoAtiva: Send {
    /// Nome completo da porta conectada.
    fn porta(&self) -> &str;
}

/// Origem de portas MIDI de entrada.
pub trait FontePortas: Send {
    /// Lista os nomes das portas de entrada presentes agora.
    fn listar(&mut self) -> Result<Vec<String>, ErroFonte>;

    /// Conecta à porta de nome exato `porta`. Cada mensagem chega a `ao_receber`
    /// carimbada com `relogio.agora_ns()` no momento da chegada.
    fn conectar(
        &mut self,
        porta: &str,
        relogio: Arc<dyn Relogio>,
        ao_receber: CallbackBytes,
    ) -> Result<Box<dyn ConexaoAtiva>, ErroFonte>;
}

/// Falhas da fonte. Nunca derrubam o processo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ErroFonte {
    /// Backend MIDI (ex.: sequenciador ALSA) indisponível.
    BackendIndisponivel(String),
    /// A porta pedida não está presente.
    PortaNaoEncontrada(String),
    /// A porta existe, mas a conexão falhou.
    FalhaConexao(String),
}

impl fmt::Display for ErroFonte {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendIndisponivel(m) => write!(f, "backend MIDI indisponível: {m}"),
            Self::PortaNaoEncontrada(p) => write!(f, "porta MIDI não encontrada: {p}"),
            Self::FalhaConexao(m) => write!(f, "falha ao conectar à porta MIDI: {m}"),
        }
    }
}

impl std::error::Error for ErroFonte {}

/// Parte estável do nome de uma porta: remove o sufixo ` <cliente>:<porta>`
/// que o `midir` acrescenta no ALSA (ex.: `"KB-330:KB-330 MIDI 1 20:0"` →
/// `"KB-330:KB-330 MIDI 1"`). O número do cliente ALSA pode mudar quando o
/// dispositivo USB é reconectado; o restante do nome não.
#[must_use]
pub fn chave_estavel(nome: &str) -> &str {
    let Some((antes, sufixo)) = nome.rsplit_once(' ') else {
        return nome;
    };
    let numerico = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    match sufixo.split_once(':') {
        Some((cliente, porta)) if numerico(cliente) && numerico(porta) && !antes.is_empty() => {
            antes
        }
        _ => nome,
    }
}

/// Encontra `desejada` em `portas`: nome exato primeiro; senão, a única porta
/// com a mesma [`chave_estavel`]. Ambiguidade (duas candidatas) = não encontrada,
/// para nunca conectar ao dispositivo errado.
#[must_use]
pub fn localizar_porta<'a>(portas: &'a [String], desejada: &str) -> Option<&'a str> {
    if let Some(p) = portas.iter().find(|p| p.as_str() == desejada) {
        return Some(p.as_str());
    }
    let chave = chave_estavel(desejada);
    let mut candidatas = portas.iter().filter(|p| chave_estavel(p) == chave);
    match (candidatas.next(), candidatas.next()) {
        (Some(unica), None) => Some(unica.as_str()),
        _ => None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn chave_estavel_remove_sufixo_alsa() {
        assert_eq!(
            chave_estavel("KB-330:KB-330 MIDI 1 20:0"),
            "KB-330:KB-330 MIDI 1"
        );
    }

    #[test]
    fn chave_estavel_preserva_nome_sem_sufixo() {
        assert_eq!(chave_estavel("Teclado USB"), "Teclado USB");
        assert_eq!(chave_estavel("20:0"), "20:0");
        assert_eq!(chave_estavel("Porta a:b"), "Porta a:b");
        assert_eq!(chave_estavel(""), "");
    }

    #[test]
    fn localiza_por_nome_exato() {
        let portas = vec!["A 20:0".to_string(), "A 24:0".to_string()];
        assert_eq!(localizar_porta(&portas, "A 24:0"), Some("A 24:0"));
    }

    #[test]
    fn localiza_quando_o_cliente_alsa_muda() {
        let portas = vec![
            "Midi Through:Port-0 14:0".to_string(),
            "A:B 24:0".to_string(),
        ];
        assert_eq!(localizar_porta(&portas, "A:B 20:0"), Some("A:B 24:0"));
    }

    #[test]
    fn ambiguidade_nao_conecta() {
        let portas = vec!["A:B 24:0".to_string(), "A:B 28:0".to_string()];
        assert_eq!(localizar_porta(&portas, "A:B 20:0"), None);
    }

    #[test]
    fn ausente_nao_e_encontrada() {
        let portas = vec!["X 1:0".to_string()];
        assert_eq!(localizar_porta(&portas, "Y 1:0"), None);
    }
}
