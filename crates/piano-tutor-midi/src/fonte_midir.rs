//! Implementação real de [`FontePortas`] com `midir` (ALSA no Linux).
//!
//! Não testada aqui contra hardware: a validação com o REVAS físico e o
//! hot-plug USB é do gate F01 (NÃO VERIFICADO até o ensaio no cerebro).

use crate::fonte::{CallbackBytes, ConexaoAtiva, ErroFonte, FontePortas};
use crate::relogio::Relogio;
use midir::{MidiInput, MidiInputConnection};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

const CLIENTE_LISTAGEM: &str = "piano-tutor-listagem";
const CLIENTE_ENTRADA: &str = "piano-tutor-entrada";
const NOME_PORTA_LOCAL: &str = "piano-tutor-monitor";

/// Fonte de portas via `midir`. Mantém um cliente só para listar (a consulta
/// ao ALSA reflete o hot-plug a cada chamada) e cria outro por conexão, pois
/// `MidiInput::connect` consome o cliente.
#[derive(Default)]
pub struct FonteMidir {
    listagem: Option<MidiInput>,
}

impl FonteMidir {
    #[must_use]
    pub fn nova() -> Self {
        Self::default()
    }

    fn cliente_listagem(&mut self) -> Result<&MidiInput, ErroFonte> {
        if self.listagem.is_none() {
            let entrada = MidiInput::new(CLIENTE_LISTAGEM)
                .map_err(|e| ErroFonte::BackendIndisponivel(e.to_string()))?;
            self.listagem = Some(entrada);
        }
        self.listagem
            .as_ref()
            .ok_or_else(|| ErroFonte::BackendIndisponivel("cliente não inicializado".into()))
    }
}

impl FontePortas for FonteMidir {
    fn listar(&mut self) -> Result<Vec<String>, ErroFonte> {
        let entrada = self.cliente_listagem()?;
        // Nome ilegível (porta sumindo durante a consulta) é simplesmente omitido.
        Ok(entrada
            .ports()
            .iter()
            .filter_map(|p| entrada.port_name(p).ok())
            .collect())
    }

    fn conectar(
        &mut self,
        porta: &str,
        relogio: Arc<dyn Relogio>,
        mut ao_receber: CallbackBytes,
    ) -> Result<Box<dyn ConexaoAtiva>, ErroFonte> {
        let entrada = MidiInput::new(CLIENTE_ENTRADA)
            .map_err(|e| ErroFonte::BackendIndisponivel(e.to_string()))?;
        let alvo = entrada
            .ports()
            .into_iter()
            .find(|p| entrada.port_name(p).is_ok_and(|n| n == porta))
            .ok_or_else(|| ErroFonte::PortaNaoEncontrada(porta.to_string()))?;
        // O timestamp do midir (µs, base do backend) é ignorado: o pipeline
        // usa a base `Instant` única do `Relogio`, carimbada na chegada.
        let conexao = entrada
            .connect(
                &alvo,
                NOME_PORTA_LOCAL,
                move |_ts_backend_us, bytes, _| {
                    let ts = relogio.agora_ns();
                    // Um pânico aqui derrubaria o processo no `join` do midir
                    // ao fechar a conexão; por isso é contido.
                    let _ = catch_unwind(AssertUnwindSafe(|| ao_receber(bytes, ts)));
                },
                (),
            )
            .map_err(|e| ErroFonte::FalhaConexao(e.to_string()))?;
        Ok(Box::new(ConexaoMidir {
            _conexao: conexao,
            porta: porta.to_string(),
        }))
    }
}

struct ConexaoMidir {
    _conexao: MidiInputConnection<()>,
    porta: String,
}

impl ConexaoAtiva for ConexaoMidir {
    fn porta(&self) -> &str {
        &self.porta
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::relogio::RelogioMonotonico;

    /// Sem sequenciador ALSA (CI/nuvem) deve virar erro; com ele, uma lista.
    /// Nunca pânico. Não valida hardware.
    #[test]
    fn listar_e_conectar_inexistente_nao_entram_em_panico() {
        let mut f = FonteMidir::nova();
        let _ = f.listar();
        let r = f.conectar(
            "porta-que-nao-existe 999:0",
            Arc::new(RelogioMonotonico::novo()),
            Box::new(|_, _| {}),
        );
        assert!(r.is_err());
    }
}
