//! Coleta de informações do ambiente (efeitos colaterais isolados aqui).
//!
//! Nada é coletado além de: SO (NAME/VERSION), kernel, versão do rustc,
//! nomes de portas MIDI de entrada e nomes de hosts/dispositivos de saída de áudio.

use crate::relatorio::{Enumeracao, InfoSistema};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Tempo máximo de cada enumeração.
pub const TEMPO_LIMITE: Duration = Duration::from_secs(3);

/// Extrai `NAME` e `VERSION` do conteúdo de `/etc/os-release` (lógica pura).
#[must_use]
pub fn parsear_os_release(conteudo: &str) -> (String, String) {
    let valor = |chave: &str| {
        conteudo
            .lines()
            .find_map(|linha| linha.strip_prefix(chave)?.strip_prefix('='))
            .map(|v| v.trim().trim_matches('"').to_string())
            .unwrap_or_else(|| "desconhecido".to_string())
    };
    (valor("NAME"), valor("VERSION"))
}

/// Coleta SO e kernel; falhas de leitura viram "desconhecido".
#[must_use]
pub fn coletar_sistema() -> InfoSistema {
    let (so_nome, so_versao) = match std::fs::read_to_string("/etc/os-release") {
        Ok(texto) => parsear_os_release(&texto),
        Err(e) => {
            tracing::warn!("não foi possível ler /etc/os-release: {e}");
            ("desconhecido".to_string(), "desconhecido".to_string())
        }
    };
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|t| t.trim().to_string())
        .unwrap_or_else(|e| {
            tracing::warn!("não foi possível ler a versão do kernel: {e}");
            "desconhecido".to_string()
        });
    InfoSistema {
        so_nome,
        so_versao,
        kernel,
    }
}

/// Versão do rustc usada na compilação (definida por `build.rs`).
#[must_use]
pub fn versao_rustc() -> String {
    option_env!("PT_RUSTC_VERSAO")
        .unwrap_or("desconhecida")
        .to_string()
}

/// Executa `f` em outra thread com limite de tempo. Pânico ou estouro de
/// tempo viram erro/`tempo_esgotado`, nunca travam nem derrubam o processo.
pub fn com_limite<F>(nome: &'static str, limite: Duration, f: F) -> Enumeracao
where
    F: FnOnce() -> Enumeracao + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    let criou = thread::Builder::new()
        .name(format!("enumerar-{nome}"))
        .spawn(move || {
            // Se o receptor já desistiu, o envio falha e o resultado é descartado.
            let _ = tx.send(f());
        });
    if let Err(e) = criou {
        return Enumeracao {
            erros: vec![format!(
                "não foi possível iniciar a thread de enumeração: {e}"
            )],
            ..Enumeracao::default()
        };
    }
    match rx.recv_timeout(limite) {
        Ok(resultado) => resultado,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            tracing::warn!("enumeração {nome} excedeu {limite:?}");
            Enumeracao {
                tempo_esgotado: true,
                ..Enumeracao::default()
            }
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Enumeracao {
            erros: vec![format!("a enumeração {nome} terminou com pânico interno")],
            ..Enumeracao::default()
        },
    }
}

/// Enumera portas de entrada MIDI com `midir`.
#[must_use]
pub fn enumerar_midi() -> Enumeracao {
    let mut saida = Enumeracao::default();
    match midir::MidiInput::new("piano-tutor-diag") {
        Ok(entrada) => {
            for porta in entrada.ports() {
                match entrada.port_name(&porta) {
                    Ok(nome) => saida.itens.push(nome),
                    Err(e) => saida
                        .erros
                        .push(format!("nome de porta MIDI ilegível: {e}")),
                }
            }
        }
        Err(e) => {
            tracing::warn!("backend MIDI indisponível: {e}");
            saida.erros.push(format!("backend MIDI indisponível: {e}"));
        }
    }
    saida
}

/// Enumera hosts e dispositivos de saída de áudio com `cpal`.
#[must_use]
pub fn enumerar_audio() -> Enumeracao {
    use cpal::traits::{DeviceTrait, HostTrait};

    let mut saida = Enumeracao::default();
    for id in cpal::available_hosts() {
        let nome_host = id.name();
        match cpal::host_from_id(id) {
            Ok(host) => match host.output_devices() {
                Ok(dispositivos) => {
                    let mut contagem = 0usize;
                    for dispositivo in dispositivos {
                        contagem += 1;
                        match dispositivo.description() {
                            Ok(d) => saida.itens.push(format!("{nome_host}: {}", d.name())),
                            Err(e) => saida
                                .erros
                                .push(format!("{nome_host}: nome de dispositivo ilegível: {e}")),
                        }
                    }
                    if contagem == 0 {
                        tracing::info!("host {nome_host} sem dispositivos de saída");
                    }
                }
                Err(e) => {
                    tracing::warn!("host {nome_host}: falha ao listar saídas: {e}");
                    saida
                        .erros
                        .push(format!("{nome_host}: falha ao listar saídas: {e}"));
                }
            },
            Err(e) => {
                tracing::warn!("host {nome_host} indisponível: {e}");
                saida
                    .erros
                    .push(format!("{nome_host}: host indisponível: {e}"));
            }
        }
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn parseia_os_release() {
        let texto = "NAME=\"Kubuntu\"\nVERSION=\"26.04 LTS\"\nHOME_URL=\"x\"\n";
        assert_eq!(
            parsear_os_release(texto),
            ("Kubuntu".into(), "26.04 LTS".into())
        );
    }

    #[test]
    fn os_release_ausente_vira_desconhecido() {
        assert_eq!(
            parsear_os_release(""),
            ("desconhecido".into(), "desconhecido".into())
        );
    }

    #[test]
    fn estouro_de_tempo_nao_trava() {
        let r = com_limite("teste", Duration::from_millis(50), || {
            thread::sleep(Duration::from_millis(500));
            Enumeracao::default()
        });
        assert!(r.tempo_esgotado);
    }

    #[test]
    fn panico_vira_erro() {
        let r = com_limite("teste", Duration::from_secs(1), || panic!("falha simulada"));
        assert_eq!(r.erros.len(), 1);
        assert!(!r.tempo_esgotado);
    }
}
