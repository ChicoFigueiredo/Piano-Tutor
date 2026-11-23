//! O registro de ações e as abas do Ribbon, como dados puros (sem janela).

use crate::dedos::NOMENCLATURAS;
use crate::estado::{EstadoApp, TEMAS, ids};
use bancada_casca::egui::Key;
use bancada_casca::{Aba, Acao, Atalho, Grupo, ids as ids_casca};

/// Id do seletor de instrumento (com busca) desenhado pelo app no Ribbon.
pub const CONTROLE_INSTRUMENTO: &str = "piano.seletor-instrumento";

/// Id do seletor da nomenclatura dos dedos (números ou letras) desenhado pelo app no Ribbon.
pub const CONTROLE_DEDOS: &str = "piano.seletor-dedos";

/// Atalho do Pânico: Ctrl+. (Esc já é da casca: fecha paleta, bastidores e KeyTips). Com Ctrl,
/// dispara mesmo com um campo de texto em foco.
pub const ATALHO_PANICO: Atalho = Atalho::ctrl(Key::Period);

/// A lista única de ações do app, com o estado atual (tema marcado).
pub fn acoes(estado: &EstadoApp) -> Vec<Acao> {
    let mut lista = vec![
        Acao::nova(ids::MONITOR_MIDI, "Monitor MIDI")
            .icone("midi")
            .dica("Escolhe a porta MIDI, conecta e mostra eventos, capacidades e o log da sessão"),
        Acao::nova(ids::TECLADO_61, "Teclado")
            .icone("teclado")
            .dica("Abre o espelho do teclado: notas do teclado físico ou, sem ele, da simulação"),
        Acao::nova(ids::TIMBRES, "Sons e timbres")
            .icone("timbres")
            .dica("SoundFont, piano ou segundo timbre, volume e diagnóstico da saída de áudio"),
        Acao::nova(ids::CALIBRACAO, "Calibrar")
            .icone("calibrar")
            .dica("Mede as 61 teclas, a força do toque e o pedal do teclado físico"),
        Acao::nova(ids::RITMOS, "Ritmos")
            .icone("ritmos")
            .dica("Metrônomo, bateria com padrões prontos, pads e exercício de pulso"),
        Acao::nova(ids::ACORDES, "Acordes e escalas")
            .icone("acordes")
            .dica("O que estou tocando, dicionário de acordes, escalas e desafio"),
        Acao::nova(ids::EXERCICIOS, "Exercícios")
            .icone("exercicios")
            .dica("Dez exercícios progressivos com correção na hora e progresso salvo"),
        Acao::nova(ids::GRAVACAO, "Gravar e ouvir")
            .icone("gravar")
            .dica("Grava o que você toca, toca de volta em outro timbre e salva .mid"),
        Acao::nova(ids::PARTITURA, "Partitura")
            .icone("partitura")
            .dica("Abre um MusicXML em duas claves e toca com o cursor acompanhando"),
        Acao::nova(ids::PANICO, "Pânico")
            .icone("panico")
            .dica("Corta todo o som agora (all-notes-off imediato) e limpa o Espelho")
            .atalho(ATALHO_PANICO),
        Acao::nova(ids::SOBRE, "Sobre o Piano Tutor")
            .icone("sobre")
            .dica("Mostra a página \"Sobre\" no centro da janela"),
        Acao::nova(ids::TEMA_ALTERNAR, "Alternar tema")
            .icone("tema")
            .dica("Passa ao próximo tema")
            .atalho(Atalho::ctrl(Key::T)),
    ];
    let dedos = estado.nomenclatura_dedos();
    for n in NOMENCLATURAS {
        lista.push(
            Acao::nova(ids::dedos(n), n.nome())
                .icone("dedos")
                .dica(n.dica())
                .alternancia(n == dedos),
        );
    }
    let atual = estado.tema_atual().id;
    for tema in TEMAS {
        lista.push(
            Acao::nova(ids::tema(tema.id), format!("Tema {}", tema.nome))
                .icone("tema")
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
    let ids_dos_dedos: Vec<String> = NOMENCLATURAS.iter().map(|n| ids::dedos(*n)).collect();
    let grupo_dedos: Vec<&str> = ids_dos_dedos.iter().map(String::as_str).collect();

    vec![
        Aba::nova("inicio", "Início", "I")
            .grupo(
                Grupo::novo("instrumento", "Instrumento", &[]).com_controle(CONTROLE_INSTRUMENTO),
            )
            .grupo(Grupo::novo("som", "Som", &[ids::TIMBRES, ids::PANICO]))
            .grupo(Grupo::novo(
                "teclado",
                "Teclado",
                &[ids::TECLADO_61, ids::MONITOR_MIDI, ids::CALIBRACAO],
            ))
            .grupo(Grupo::novo(
                "praticar",
                "Praticar",
                &[ids::EXERCICIOS, ids::ACORDES, ids::RITMOS],
            ))
            .grupo(Grupo::novo(
                "musica",
                "Música",
                &[ids::PARTITURA, ids::GRAVACAO],
            ))
            .grupo(Grupo::novo("dedos", "Dedilhado", &[]).com_controle(CONTROLE_DEDOS))
            .grupo(Grupo::novo("ajuda", "Ajuda", &[ids::SOBRE])),
        Aba::nova("exibir", "Exibir", "E")
            .grupo(Grupo::novo("tema", "Tema", &grupo_tema))
            .grupo(Grupo::novo("dedos-exibir", "Dedilhado", &grupo_dedos))
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
            ids::GRAVACAO,
            ids::PARTITURA,
            ids::SOBRE,
            ids::TEMA_ALTERNAR,
            "piano.tema.noite",
            "piano.tema.claro",
            "piano.tema.ebano",
            "piano.tema.grafite",
            "piano.tema.executivo",
            "piano.dedos.numeros",
            "piano.dedos.letras",
            "piano.dedos.ingles-antigo",
        ] {
            assert!(l.get(id).is_some(), "falta a ação {id}");
        }
        assert_eq!(l.len(), 20);
        let ids: HashSet<&str> = l.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids.len(), l.len());
        assert!(ids.iter().all(|i| i.starts_with("piano.")));
    }

    #[test]
    fn seletor_de_dedos_marca_so_a_nomenclatura_escolhida_e_fica_no_exibir() {
        use crate::dedos::NomenclaturaDedos;
        let mut e = EstadoApp::novo();
        e.definir_nomenclatura_dedos(NomenclaturaDedos::LetrasPt);
        let l = ListaDeAcoes::nova(acoes(&e)).expect("ids únicos");
        let marcadas: Vec<&str> = l
            .iter()
            .filter(|a| a.id.starts_with(ids::PREFIXO_DEDOS) && a.marcada)
            .map(|a| a.id.as_str())
            .collect();
        assert_eq!(marcadas, vec!["piano.dedos.letras"]);
        let exibir = abas()
            .into_iter()
            .find(|a| a.id == "exibir")
            .expect("aba Exibir");
        assert!(exibir.acoes().any(|i| i == "piano.dedos.numeros"));
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
        assert_eq!(marcados(&e), vec!["piano.tema.grafite"]);
        e.alternar_tema();
        assert_eq!(marcados(&e), vec!["piano.tema.executivo"]);
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
