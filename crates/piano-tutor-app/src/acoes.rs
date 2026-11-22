//! O registro de ações e as abas do Ribbon, como dados puros (sem janela).

use crate::estado::{EstadoApp, TEMAS, ids};
use bancada_casca::egui::Key;
use bancada_casca::{Aba, Acao, Atalho, Grupo, ids as ids_casca};

/// Atalho do Pânico: Ctrl+. (Esc já é da casca: fecha paleta, bastidores e KeyTips). Com Ctrl,
/// dispara mesmo com um campo de texto em foco.
pub const ATALHO_PANICO: Atalho = Atalho::ctrl(Key::Period);

/// A lista única de ações do app, com o estado atual (tema marcado).
pub fn acoes(estado: &EstadoApp) -> Vec<Acao> {
    let mut lista = vec![
        Acao::nova(ids::MONITOR_MIDI, "Monitor MIDI")
            .dica("Escolhe a porta MIDI, conecta e mostra eventos, capacidades e o log da sessão"),
        Acao::nova(ids::TECLADO_61, "Teclado de 61 teclas")
            .dica("Abre o espelho do teclado: notas do teclado físico ou, sem ele, da simulação"),
        Acao::nova(ids::TIMBRES, "Timbres")
            .dica("SoundFont, piano ou segundo timbre, volume e diagnóstico da saída de áudio"),
        Acao::nova(ids::CALIBRACAO, "Calibrar teclado")
            .dica("Mede as 61 teclas, a força do toque e o pedal do teclado físico"),
        Acao::nova(ids::RITMOS, "Ritmos")
            .dica("Metrônomo, bateria com padrões prontos, pads e exercício de pulso"),
        Acao::nova(ids::ACORDES, "Acordes e escalas")
            .dica("O que estou tocando, dicionário de acordes, escalas e desafio"),
        Acao::nova(ids::EXERCICIOS, "Exercícios")
            .dica("Dez exercícios progressivos com correção na hora e progresso salvo"),
        Acao::nova(ids::PANICO, "Pânico")
            .dica("Corta todo o som agora (all-notes-off imediato) e limpa o Espelho")
            .atalho(ATALHO_PANICO),
        Acao::nova(ids::SOBRE, "Sobre o Piano Tutor")
            .dica("Mostra a página \"Sobre\" no centro da janela"),
        Acao::nova(ids::TEMA_ALTERNAR, "Alternar tema")
            .dica("Passa ao próximo tema")
            .atalho(Atalho::ctrl(Key::T)),
    ];
    let atual = estado.tema_atual().id;
    for tema in TEMAS {
        lista.push(
            Acao::nova(ids::tema(tema.id), format!("Tema {}", tema.nome))
                .alternancia(tema.id == atual),
        );
    }
    lista
}

/// As abas do Ribbon: Início e Exibir. As ações `casca.*` são da casca.
pub fn abas() -> Vec<Aba> {
    let ids_dos_temas: Vec<String> = TEMAS.iter().map(|t| ids::tema(t.id)).collect();
    let mut grupo_tema: Vec<&str> = vec![ids::TEMA_ALTERNAR];
    grupo_tema.extend(ids_dos_temas.iter().map(String::as_str));

    vec![
        Aba::nova("inicio", "Início", "I")
            .grupo(Grupo::novo(
                "teclado",
                "Teclado",
                &[ids::MONITOR_MIDI, ids::CALIBRACAO, ids::TECLADO_61],
            ))
            .grupo(Grupo::novo(
                "praticar",
                "Praticar",
                &[ids::EXERCICIOS, ids::ACORDES, ids::RITMOS],
            ))
            .grupo(Grupo::novo("som", "Som", &[ids::TIMBRES, ids::PANICO]))
            .grupo(Grupo::novo("ajuda", "Ajuda", &[ids::SOBRE])),
        Aba::nova("exibir", "Exibir", "E")
            .grupo(Grupo::novo("tema", "Tema", &grupo_tema))
            .grupo(Grupo::novo(
                "ribbon",
                "Ribbon",
                &[
                    ids_casca::RIBBON_RECOLHER,
                    ids_casca::RIBBON_CLASSICO,
                    ids_casca::RIBBON_SIMPLES,
                ],
            ))
            .grupo(Grupo::novo(
                "janela",
                "Janela",
                &[
                    ids_casca::TELA_CHEIA,
                    ids_casca::MODO_FOCO,
                    ids_casca::DENSIDADE_COMPACTA,
                ],
            )),
    ]
}

#[cfg(test)]
mod testes {
    use super::*;
    use bancada_casca::ListaDeAcoes;
    use std::collections::HashSet;

    fn lista() -> ListaDeAcoes {
        ListaDeAcoes::nova(acoes(&EstadoApp::novo())).expect("ids únicos e atalhos sem conflito")
    }

    #[test]
    fn registro_tem_as_acoes_esperadas_com_ids_unicos() {
        let l = lista();
        for id in [
            ids::MONITOR_MIDI,
            ids::TECLADO_61,
            ids::TIMBRES,
            ids::PANICO,
            ids::CALIBRACAO,
            ids::RITMOS,
            ids::ACORDES,
            ids::EXERCICIOS,
            ids::SOBRE,
            ids::TEMA_ALTERNAR,
            "piano.tema.noite",
            "piano.tema.claro",
            "piano.tema.ebano",
        ] {
            assert!(l.get(id).is_some(), "falta a ação {id}");
        }
        assert_eq!(l.len(), 13);
        let ids: HashSet<&str> = l.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids.len(), l.len());
        assert!(ids.iter().all(|i| i.starts_with("piano.")));
    }

    #[test]
    fn nenhuma_acao_fica_em_breve() {
        let l = lista();
        for a in l.iter() {
            assert!(!a.rotulo.contains("em breve"), "{}", a.rotulo);
            assert!(a.esta_habilitada(), "{} desabilitada", a.id);
        }
    }

    #[test]
    fn panico_tem_atalho_ctrl_ponto_e_fica_no_grupo_som() {
        let l = lista();
        let a = l.get(ids::PANICO).expect("registrada");
        assert_eq!(a.atalho, Some(Atalho::ctrl(Key::Period)));
        let inicio = abas()
            .into_iter()
            .find(|a| a.id == "inicio")
            .expect("aba Início");
        let ids_inicio: Vec<&str> = inicio.acoes().collect();
        assert!(ids_inicio.contains(&ids::PANICO));
        assert!(ids_inicio.contains(&ids::TIMBRES));
        assert!(ids_inicio.contains(&ids::MONITOR_MIDI));
    }

    #[test]
    fn acoes_reais_estao_habilitadas() {
        let l = lista();
        assert!(l.esta_habilitada(ids::SOBRE));
        let teclado = l.get(ids::TECLADO_61).expect("registrada");
        assert!(teclado.esta_habilitada());
        assert!(!teclado.rotulo.contains("em breve"), "{}", teclado.rotulo);
        assert!(l.esta_habilitada(ids::TECLADO_61));
        assert!(l.esta_habilitada(ids::TEMA_ALTERNAR));
        assert!(l.esta_habilitada(ids::MONITOR_MIDI));
        assert!(l.esta_habilitada(ids::TIMBRES));
        assert!(l.esta_habilitada(ids::PANICO));
    }

    #[test]
    fn so_o_tema_atual_aparece_marcado_e_acompanha_a_troca() {
        let marcados = |e: &EstadoApp| -> Vec<String> {
            acoes(e)
                .into_iter()
                .filter(|a| a.id.starts_with(ids::PREFIXO_TEMA) && a.marcada)
                .map(|a| a.id)
                .collect()
        };
        let mut e = EstadoApp::novo();
        assert_eq!(marcados(&e), vec!["piano.tema.noite"]);
        e.alternar_tema();
        assert_eq!(marcados(&e), vec!["piano.tema.claro"]);
    }

    #[test]
    fn alternar_tema_tem_atalho_ctrl_t() {
        let l = lista();
        let a = l.get(ids::TEMA_ALTERNAR).expect("registrada");
        assert_eq!(a.atalho, Some(Atalho::ctrl(Key::T)));
    }

    #[test]
    fn abas_minimas_inicio_e_exibir() {
        let abas = abas();
        let rotulos: Vec<&str> = abas.iter().map(|a| a.rotulo.as_str()).collect();
        assert_eq!(rotulos, vec!["Início", "Exibir"]);
    }

    #[test]
    fn toda_acao_das_abas_existe_no_app_ou_e_da_casca() {
        let l = lista();
        for aba in abas() {
            for id in aba.acoes() {
                assert!(
                    l.get(id).is_some() || id.starts_with("casca."),
                    "a aba {} usa a ação desconhecida {id}",
                    aba.id
                );
            }
        }
    }

    #[test]
    fn exibir_oferece_recolher_o_ribbon_e_todos_os_temas() {
        let abas = abas();
        let exibir = abas.iter().find(|a| a.id == "exibir").expect("aba Exibir");
        let ids_exibir: Vec<&str> = exibir.acoes().collect();
        assert!(ids_exibir.contains(&ids_casca::RIBBON_RECOLHER));
        for t in TEMAS {
            assert!(ids_exibir.contains(&ids::tema(t.id).as_str()));
        }
    }
}
