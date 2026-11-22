//! O registro de ações e as abas do Ribbon, como dados puros (sem janela).

use crate::estado::{EstadoApp, TEMAS, ids};
use bancada_casca::egui::Key;
use bancada_casca::{Aba, Acao, Atalho, Grupo, ids as ids_casca};

/// Ações que ainda não existem: nascem desabilitadas, com o motivo e a fase em que chegam.
/// `(id, rótulo, motivo)`. O rótulo termina em "(em breve)" para ninguém achar que funcionam.
pub const EM_BREVE: [(&str, &str, &str); 3] = [
    (
        ids::MONITOR_MIDI,
        "Monitor MIDI (em breve)",
        "Chega na fase F01, com o REVAS e o MIDI real.",
    ),
    (
        ids::TECLADO_61,
        "Teclado de 61 teclas (em breve)",
        "Chega na fase F02, com o espelho do teclado.",
    ),
    (
        ids::TIMBRES,
        "Timbres (em breve)",
        "Chega na fase F03, com o som.",
    ),
];

/// A lista única de ações do app, com o estado atual (tema marcado).
pub fn acoes(estado: &EstadoApp) -> Vec<Acao> {
    let mut lista = Vec::new();

    for (id, rotulo, motivo) in EM_BREVE {
        lista.push(Acao::nova(id, rotulo).desabilitada(motivo));
    }
    lista.push(
        Acao::nova(ids::SOBRE, "Sobre o Piano Tutor")
            .dica("Mostra a página \"Sobre\" no centro da janela"),
    );

    lista.push(
        Acao::nova(ids::TEMA_ALTERNAR, "Alternar tema")
            .dica("Passa ao próximo tema")
            .atalho(Atalho::ctrl(Key::T)),
    );
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
                &[ids::MONITOR_MIDI, ids::TECLADO_61],
            ))
            .grupo(Grupo::novo("som", "Som", &[ids::TIMBRES]))
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
            ids::SOBRE,
            ids::TEMA_ALTERNAR,
            "piano.tema.noite",
            "piano.tema.claro",
            "piano.tema.ebano",
        ] {
            assert!(l.get(id).is_some(), "falta a ação {id}");
        }
        assert_eq!(l.len(), 8);
        let ids: HashSet<&str> = l.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids.len(), l.len());
        assert!(ids.iter().all(|i| i.starts_with("piano.")));
    }

    #[test]
    fn acoes_em_breve_sao_desabilitadas_nomeadas_e_dizem_o_motivo() {
        let l = lista();
        for (id, rotulo, _) in EM_BREVE {
            let a = l.get(id).expect("registrada");
            assert!(!a.esta_habilitada(), "{id} não pode fingir que funciona");
            assert!(a.rotulo.ends_with("(em breve)"), "{rotulo}");
            let motivo = a.motivo().expect("motivo");
            assert!(motivo.contains("fase F0"), "{motivo}");
        }
    }

    #[test]
    fn acoes_reais_estao_habilitadas() {
        let l = lista();
        assert!(l.esta_habilitada(ids::SOBRE));
        assert!(l.esta_habilitada(ids::TEMA_ALTERNAR));
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
