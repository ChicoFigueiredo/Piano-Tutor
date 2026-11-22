//! Montagem do relatório Markdown a partir de estruturas (lógica pura).

use crate::sanitizar::sanitizar;

/// Frase de limitação obrigatória do relatório. Vale para qualquer máquina
/// (nuvem ou cerebro): listar portas não prova que o REVAS ou o áudio funcionam.
pub const LIMITACAO: &str = "Este relatório só lista o que o sistema expõe; não comprova o REVAS nem o áudio real (exige ensaio no cerebro)";

/// Informações do sistema operacional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoSistema {
    pub so_nome: String,
    pub so_versao: String,
    pub kernel: String,
}

/// Resultado de enumerar uma classe de dispositivos (MIDI ou áudio).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Enumeracao {
    pub itens: Vec<String>,
    /// Falhas de backend (ALSA etc.); viram linhas de limitação, nunca pânico.
    pub erros: Vec<String>,
    pub tempo_esgotado: bool,
}

/// Dados completos do diagnóstico.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostico {
    pub sistema: InfoSistema,
    pub rustc: String,
    pub midi: Enumeracao,
    pub audio: Enumeracao,
    /// Rótulo livre do ambiente (ex.: "nuvem", "cerebro").
    pub rotulo_ambiente: Option<String>,
}

fn secao_dispositivos(md: &mut String, titulo: &str, enumeracao: &Enumeracao) {
    md.push_str(&format!("## {titulo}\n\n"));
    if enumeracao.itens.is_empty() {
        // Zero itens só significa "nenhum dispositivo" se a enumeração terminou
        // sem falha; caso contrário, seria um falso negativo (AGENTS.md).
        if enumeracao.tempo_esgotado {
            md.push_str("- NÃO VERIFICADO: tempo limite excedido (ver Limitações)\n");
        } else if !enumeracao.erros.is_empty() {
            md.push_str("- NÃO VERIFICADO: backend indisponível (ver Limitações)\n");
        } else {
            md.push_str("- nenhum dispositivo encontrado\n");
        }
    } else {
        for item in &enumeracao.itens {
            md.push_str(&format!("- {}\n", sanitizar(item)));
        }
    }
    md.push('\n');
}

/// Gera o Markdown do diagnóstico.
#[must_use]
pub fn montar_relatorio(d: &Diagnostico) -> String {
    let mut md = String::from("# Diagnóstico de ambiente\n\n");
    if let Some(rotulo) = &d.rotulo_ambiente {
        md.push_str(&format!("> Ambiente: {}\n\n", sanitizar(rotulo)));
    }
    md.push_str("## Sistema\n\n");
    md.push_str(&format!(
        "- SO: {} {}\n- Kernel: {}\n\n",
        sanitizar(&d.sistema.so_nome),
        sanitizar(&d.sistema.so_versao),
        sanitizar(&d.sistema.kernel)
    ));
    md.push_str(&format!("## Rust\n\n- {}\n\n", d.rustc));
    // A versão do rustc é constante de build (não é dado do usuário); o hash
    // do commit seria um falso positivo do sanitizador.
    secao_dispositivos(&mut md, "MIDI", &d.midi);
    secao_dispositivos(&mut md, "Áudio", &d.audio);

    md.push_str("## Limitações\n\n");
    md.push_str(&format!("- {LIMITACAO}.\n"));
    for (classe, e) in [("MIDI", &d.midi), ("Áudio", &d.audio)] {
        for erro in &e.erros {
            md.push_str(&format!(
                "- {classe}: falha de backend: {}\n",
                sanitizar(erro)
            ));
        }
        if e.tempo_esgotado {
            md.push_str(&format!("- {classe}: enumeração excedeu o tempo limite\n"));
        }
    }
    md.push_str("- Hostname, usuário, IPs, números de série e VID:PID não são coletados.\n");
    md
}

#[cfg(test)]
mod testes {
    use super::*;

    fn base() -> Diagnostico {
        Diagnostico {
            sistema: InfoSistema {
                so_nome: "Ubuntu".into(),
                so_versao: "24.04".into(),
                kernel: "6.18.0".into(),
            },
            rustc: "rustc 1.98.0".into(),
            midi: Enumeracao::default(),
            audio: Enumeracao::default(),
            rotulo_ambiente: None,
        }
    }

    #[test]
    fn zero_dispositivos_traz_texto_esperado() {
        let md = montar_relatorio(&base());
        assert_eq!(md.matches("nenhum dispositivo encontrado").count(), 2);
    }

    #[test]
    fn tem_todas_as_secoes_e_limitacao() {
        let md = montar_relatorio(&base());
        for s in [
            "## Sistema",
            "## Rust",
            "## MIDI",
            "## Áudio",
            "## Limitações",
        ] {
            assert!(md.contains(s), "faltou {s}");
        }
        assert!(md.contains(LIMITACAO));
    }

    #[test]
    fn erro_de_backend_vira_linha_de_limitacao() {
        let mut d = base();
        d.audio.erros.push("ALSA: sem placas".into());
        let md = montar_relatorio(&d);
        let limitacoes = md.split("## Limitações").nth(1).unwrap_or_default();
        assert!(limitacoes.contains("Áudio: falha de backend: ALSA: sem placas"));
    }

    #[test]
    fn tempo_esgotado_vira_limitacao() {
        let mut d = base();
        d.midi.tempo_esgotado = true;
        assert!(montar_relatorio(&d).contains("MIDI: enumeração excedeu o tempo limite"));
    }

    #[test]
    fn nomes_de_porta_sao_sanitizados() {
        let mut d = base();
        d.midi.itens.push("KB-330 SN0123456789".into());
        let md = montar_relatorio(&d);
        assert!(!md.contains("SN0123456789"));
        assert!(md.contains("KB-330 [removido]"));
    }

    #[test]
    fn versao_do_rustc_preserva_hash_do_commit() {
        let mut d = base();
        d.rustc = "rustc 1.98.0 (88d9e12ae 2026-08-18)".into();
        assert!(montar_relatorio(&d).contains("(88d9e12ae 2026-08-18)"));
    }

    #[test]
    fn backend_indisponivel_nao_vira_zero_dispositivos() {
        let mut d = base();
        d.midi
            .erros
            .push("MIDI support could not be initialized".into());
        let md = montar_relatorio(&d);
        let midi = md.split("## MIDI").nth(1).unwrap_or_default();
        let midi = midi.split("## Áudio").next().unwrap_or_default();
        assert!(!midi.contains("nenhum dispositivo encontrado"));
        assert!(midi.contains("NÃO VERIFICADO: backend indisponível"));
    }

    #[test]
    fn tempo_esgotado_nao_vira_zero_dispositivos() {
        let mut d = base();
        d.audio.tempo_esgotado = true;
        let md = montar_relatorio(&d);
        let audio = md.split("## Áudio").nth(1).unwrap_or_default();
        let audio = audio.split("## Limitações").next().unwrap_or_default();
        assert!(!audio.contains("nenhum dispositivo encontrado"));
        assert!(audio.contains("NÃO VERIFICADO: tempo limite excedido"));
    }

    #[test]
    fn limitacao_nao_supoe_nuvem() {
        assert!(!LIMITACAO.contains("nuvem"));
    }

    #[test]
    fn rotulo_de_ambiente_aparece() {
        let mut d = base();
        d.rotulo_ambiente = Some("nuvem".into());
        assert!(montar_relatorio(&d).contains("> Ambiente: nuvem"));
    }
}
