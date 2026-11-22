//! Fonte de portas simulada, para testes e para o modo simulação da UI.
//!
//! Clones de [`FonteFalsa`] compartilham o mesmo estado: um clone vai para o
//! gerenciador e outro fica com o teste para ligar/desligar portas e injetar bytes,
//! inclusive a partir de outra thread (como faria o backend real).

use crate::fonte::{CallbackBytes, ConexaoAtiva, ErroFonte, FontePortas};
use crate::relogio::Relogio;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

#[derive(Clone, Default)]
pub struct FonteFalsa {
    interno: Arc<Mutex<Interno>>,
}

#[derive(Default)]
struct Interno {
    portas: Vec<String>,
    ativa: Option<Ativa>,
    proximo_id: u64,
    conexoes_realizadas: u64,
    falhas_de_conexao_pendentes: u32,
    listagem_falha: bool,
}

struct Ativa {
    id: u64,
    porta: String,
    relogio: Arc<dyn Relogio>,
    callback: CallbackBytes,
    /// Falso depois que a porta "foi desplugada": nada mais é entregue.
    viva: bool,
}

impl FonteFalsa {
    #[must_use]
    pub fn nova<I, S>(portas: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let f = Self::default();
        f.travar().portas = portas.into_iter().map(Into::into).collect();
        f
    }

    fn travar(&self) -> MutexGuard<'_, Interno> {
        self.interno.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Simula plugar um dispositivo.
    pub fn adicionar_porta(&self, nome: impl Into<String>) {
        let nome = nome.into();
        let mut i = self.travar();
        if !i.portas.contains(&nome) {
            i.portas.push(nome);
        }
    }

    /// Simula desplugar: some da listagem e a conexão aberta para de receber.
    pub fn remover_porta(&self, nome: &str) {
        let mut i = self.travar();
        i.portas.retain(|p| p != nome);
        if let Some(a) = i.ativa.as_mut().filter(|a| a.porta == nome) {
            a.viva = false;
        }
    }

    /// As próximas `n` tentativas de conexão falham.
    pub fn falhar_proximas_conexoes(&self, n: u32) {
        self.travar().falhas_de_conexao_pendentes = n;
    }

    /// Faz `listar` falhar (backend indisponível) enquanto `falha` for verdadeiro.
    pub fn definir_falha_de_listagem(&self, falha: bool) {
        self.travar().listagem_falha = falha;
    }

    /// Injeta uma mensagem carimbada com o relógio da conexão. Retorna se foi entregue.
    pub fn injetar(&self, bytes: &[u8]) -> bool {
        let mut i = self.travar();
        match i.ativa.as_mut().filter(|a| a.viva) {
            Some(a) => {
                let ts = a.relogio.agora_ns();
                (a.callback)(bytes, ts);
                true
            }
            None => false,
        }
    }

    /// Injeta com timestamp explícito.
    pub fn injetar_com_ts(&self, bytes: &[u8], ts_ns: u64) -> bool {
        let mut i = self.travar();
        match i.ativa.as_mut().filter(|a| a.viva) {
            Some(a) => {
                (a.callback)(bytes, ts_ns);
                true
            }
            None => false,
        }
    }

    /// Porta da conexão aberta (viva ou não), se houver.
    #[must_use]
    pub fn conexao_aberta(&self) -> Option<String> {
        self.travar().ativa.as_ref().map(|a| a.porta.clone())
    }

    #[must_use]
    pub fn conexoes_realizadas(&self) -> u64 {
        self.travar().conexoes_realizadas
    }
}

impl FontePortas for FonteFalsa {
    fn listar(&mut self) -> Result<Vec<String>, ErroFonte> {
        let i = self.travar();
        if i.listagem_falha {
            return Err(ErroFonte::BackendIndisponivel("falha simulada".into()));
        }
        Ok(i.portas.clone())
    }

    fn conectar(
        &mut self,
        porta: &str,
        relogio: Arc<dyn Relogio>,
        ao_receber: CallbackBytes,
    ) -> Result<Box<dyn ConexaoAtiva>, ErroFonte> {
        let mut i = self.travar();
        if !i.portas.iter().any(|p| p == porta) {
            return Err(ErroFonte::PortaNaoEncontrada(porta.to_string()));
        }
        if i.falhas_de_conexao_pendentes > 0 {
            i.falhas_de_conexao_pendentes -= 1;
            return Err(ErroFonte::FalhaConexao("falha simulada".into()));
        }
        i.proximo_id += 1;
        i.conexoes_realizadas += 1;
        let id = i.proximo_id;
        i.ativa = Some(Ativa {
            id,
            porta: porta.to_string(),
            relogio,
            callback: ao_receber,
            viva: true,
        });
        Ok(Box::new(ConexaoFalsa {
            id,
            porta: porta.to_string(),
            interno: Arc::clone(&self.interno),
        }))
    }
}

struct ConexaoFalsa {
    id: u64,
    porta: String,
    interno: Arc<Mutex<Interno>>,
}

impl ConexaoAtiva for ConexaoFalsa {
    fn porta(&self) -> &str {
        &self.porta
    }
}

impl Drop for ConexaoFalsa {
    fn drop(&mut self) {
        let mut i = self.interno.lock().unwrap_or_else(PoisonError::into_inner);
        if i.ativa.as_ref().is_some_and(|a| a.id == self.id) {
            i.ativa = None;
        }
    }
}
