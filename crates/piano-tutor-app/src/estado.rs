//! Estado e regras do app "Olá Piano Tutor", sem janela e sem GUI: tudo aqui é testável
//! por unidade. A casca (`bancada-casca`) só entra em `app.rs`.

/// Versão do aplicativo, mostrada na barra de status.
pub const VERSAO: &str = env!("CARGO_PKG_VERSION");

/// Um tema oferecido ao usuário. O `id` existe em `dados/temas.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemaApp {
    pub id: &'static str,
    pub nome: &'static str,
}

/// Temas, na ordem em que "Alternar tema" os percorre. O primeiro é o inicial.
pub const TEMAS: [TemaApp; 3] = [
    TemaApp {
        id: "noite",
        nome: "Noite",
    },
    TemaApp {
        id: "claro",
        nome: "Claro",
    },
    TemaApp {
        id: "ebano",
        nome: "Ébano",
    },
];

/// Rota da tela Espelho do teclado (F02).
pub const ROTA_TECLADO: &str = "teclado";

/// Rota do painel Monitor MIDI (F01).
pub const ROTA_MONITOR: &str = "monitor-midi";

/// Rota da tela Timbres (F03).
pub const ROTA_TIMBRES: &str = "timbres";

/// Rota da calibração do teclado (F01).
pub const ROTA_CALIBRACAO: &str = "calibracao";

/// Rota de Ritmos (F08, F09).
pub const ROTA_RITMOS: &str = "ritmos";

/// Rota de Gravação (F06).
pub const ROTA_GRAVACAO: &str = "gravacao";

/// Rota de Exercícios (F05).
pub const ROTA_EXERCICIOS: &str = "exercicios";

/// Rota de Acordes e escalas (F04).
pub const ROTA_ACORDES: &str = "acordes";

/// O que o app pede à casca depois de executar uma ação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Efeito {
    TrocarTema(&'static str),
    AbrirRota(&'static str),
    /// All-notes-off imediato no motor de áudio (e Espelho limpo).
    Panico,
}

/// Ids das ações do app (prefixo `piano.`). As da casca (`casca.*`) ficam com a casca.
pub mod ids {
    pub const TEMA_ALTERNAR: &str = "piano.tema.alternar";
    pub const PREFIXO_TEMA: &str = "piano.tema.";
    pub const SOBRE: &str = "piano.sobre";
    pub const MONITOR_MIDI: &str = "piano.monitor-midi";
    pub const TECLADO_61: &str = "piano.teclado-61";
    pub const TIMBRES: &str = "piano.timbres";
    pub const PANICO: &str = "piano.panico";
    pub const CALIBRACAO: &str = "piano.calibracao";
    pub const RITMOS: &str = "piano.ritmos";
    pub const ACORDES: &str = "piano.acordes";
    pub const EXERCICIOS: &str = "piano.exercicios";
    pub const GRAVACAO: &str = "piano.gravacao";

    /// Id da ação que escolhe um tema específico.
    pub fn tema(id_do_tema: &str) -> String {
        format!("{PREFIXO_TEMA}{id_do_tema}")
    }
}

/// Estado do app (independente da janela).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstadoApp {
    tema: usize,
    ribbon_recolhido: bool,
}

impl Default for EstadoApp {
    fn default() -> Self {
        Self::novo()
    }
}

impl EstadoApp {
    pub fn novo() -> Self {
        Self {
            tema: 0,
            ribbon_recolhido: false,
        }
    }

    pub fn tema_atual(&self) -> TemaApp {
        TEMAS[self.tema]
    }

    pub fn ribbon_recolhido(&self) -> bool {
        self.ribbon_recolhido
    }

    /// Escolhe um tema pelo id. Devolve `false` (sem mudar nada) se o id não existe.
    pub fn definir_tema(&mut self, id: &str) -> bool {
        match TEMAS.iter().position(|t| t.id == id) {
            Some(i) => {
                self.tema = i;
                true
            }
            None => false,
        }
    }

    /// Passa ao próximo tema da lista, voltando ao primeiro depois do último.
    pub fn alternar_tema(&mut self) -> TemaApp {
        self.tema = (self.tema + 1) % TEMAS.len();
        self.tema_atual()
    }

    /// Espelha o que a casca informou (tema e Ribbon podem mudar por outros caminhos, como
    /// Ctrl+F1 ou as opções de aparência). Tema desconhecido é ignorado.
    pub fn sincronizar(&mut self, tema_id: &str, ribbon_recolhido: bool) {
        self.definir_tema(tema_id);
        self.ribbon_recolhido = ribbon_recolhido;
    }

    /// Executa uma ação do app e diz o que pedir à casca. Id desconhecido não faz nada.
    pub fn executar(&mut self, id: &str) -> Option<Efeito> {
        if id == ids::TEMA_ALTERNAR {
            return Some(Efeito::TrocarTema(self.alternar_tema().id));
        }
        if id == ids::SOBRE {
            return Some(Efeito::AbrirRota("sobre"));
        }
        if id == ids::TECLADO_61 {
            return Some(Efeito::AbrirRota(ROTA_TECLADO));
        }
        if id == ids::MONITOR_MIDI {
            return Some(Efeito::AbrirRota(ROTA_MONITOR));
        }
        if id == ids::TIMBRES {
            return Some(Efeito::AbrirRota(ROTA_TIMBRES));
        }
        if id == ids::CALIBRACAO {
            return Some(Efeito::AbrirRota(ROTA_CALIBRACAO));
        }
        if id == ids::RITMOS {
            return Some(Efeito::AbrirRota(ROTA_RITMOS));
        }
        if id == ids::GRAVACAO {
            return Some(Efeito::AbrirRota(ROTA_GRAVACAO));
        }
        if id == ids::EXERCICIOS {
            return Some(Efeito::AbrirRota(ROTA_EXERCICIOS));
        }
        if id == ids::ACORDES {
            return Some(Efeito::AbrirRota(ROTA_ACORDES));
        }
        if id == ids::PANICO {
            return Some(Efeito::Panico);
        }
        let id_do_tema = id.strip_prefix(ids::PREFIXO_TEMA)?;
        if self.definir_tema(id_do_tema) {
            Some(Efeito::TrocarTema(self.tema_atual().id))
        } else {
            None
        }
    }

    /// Textos da barra de status que dependem só deste estado: versão, Ribbon e tema. Os de
    /// MIDI e áudio vêm do Monitor e do serviço de som.
    pub fn textos_de_status(&self) -> [String; 3] {
        [
            format!("Piano Tutor {VERSAO}"),
            if self.ribbon_recolhido {
                "Ribbon: recolhido".to_owned()
            } else {
                "Ribbon: expandido".to_owned()
            },
            format!("Tema: {}", self.tema_atual().nome),
        ]
    }
}

/// Parágrafos da página inicial (central).
pub const BOAS_VINDAS: [&str; 5] = [
    "Marco M1: teclado físico, MIDI, Espelho e som, no Linux.",
    "Monitor MIDI: escolha a porta do teclado, conecte e acompanhe os eventos e o log da sessão. Sem dispositivo, o teclado de 61 teclas funciona em simulação (mouse e teclado do PC).",
    "Timbres: SoundFont, piano ou segundo timbre, volume e Pânico (Ctrl+. corta todo o som).",
    "Ctrl+F1 recolhe ou expande o Ribbon. Ctrl+T alterna o tema. Ctrl+K abre a paleta de ações.",
    "A aba Exibir tem os três temas (Noite, Claro e Ébano) e os modos do Ribbon.",
];

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn comeca_no_primeiro_tema_com_ribbon_expandido() {
        let e = EstadoApp::novo();
        assert_eq!(e.tema_atual().id, "noite");
        assert!(!e.ribbon_recolhido());
    }

    #[test]
    fn alternar_tema_percorre_todos_e_volta_ao_primeiro() {
        let mut e = EstadoApp::novo();
        let vistos: Vec<&str> = (0..TEMAS.len()).map(|_| e.alternar_tema().id).collect();
        assert_eq!(vistos, vec!["claro", "ebano", "noite"]);
        assert_eq!(e.tema_atual().id, "noite");
    }

    #[test]
    fn ha_pelo_menos_dois_temas_com_ids_e_nomes_unicos() {
        assert!(TEMAS.len() >= 2);
        for (i, a) in TEMAS.iter().enumerate() {
            for b in &TEMAS[i + 1..] {
                assert_ne!(a.id, b.id);
                assert_ne!(a.nome, b.nome);
            }
        }
    }

    #[test]
    fn definir_tema_desconhecido_nao_muda_nada() {
        let mut e = EstadoApp::novo();
        assert!(e.definir_tema("claro"));
        assert!(!e.definir_tema("inexistente"));
        assert_eq!(e.tema_atual().id, "claro");
    }

    #[test]
    fn executar_acao_de_tema_pede_a_troca_a_casca() {
        let mut e = EstadoApp::novo();
        assert_eq!(
            e.executar(&ids::tema("ebano")),
            Some(Efeito::TrocarTema("ebano"))
        );
        assert_eq!(e.tema_atual().id, "ebano");
        assert_eq!(
            e.executar(ids::TEMA_ALTERNAR),
            Some(Efeito::TrocarTema("noite"))
        );
        assert_eq!(e.executar("piano.tema.inexistente"), None);
        assert_eq!(e.tema_atual().id, "noite");
    }

    #[test]
    fn acoes_desconhecidas_nao_fazem_nada() {
        let mut e = EstadoApp::novo();
        let antes = e.clone();
        for id in ["piano.inexistente", "casca.ribbon.recolher", ""] {
            assert_eq!(e.executar(id), None, "{id}");
        }
        assert_eq!(e, antes);
    }

    #[test]
    fn teclado_61_abre_a_rota_do_espelho() {
        let mut e = EstadoApp::novo();
        assert_eq!(
            e.executar(ids::TECLADO_61),
            Some(Efeito::AbrirRota("teclado"))
        );
        assert_eq!(ROTA_TECLADO, "teclado");
    }

    #[test]
    fn monitor_timbres_e_panico_tem_efeito() {
        let mut e = EstadoApp::novo();
        assert_eq!(
            e.executar(ids::MONITOR_MIDI),
            Some(Efeito::AbrirRota(ROTA_MONITOR))
        );
        assert_eq!(
            e.executar(ids::TIMBRES),
            Some(Efeito::AbrirRota(ROTA_TIMBRES))
        );
        assert_eq!(e.executar(ids::PANICO), Some(Efeito::Panico));
    }

    #[test]
    fn sobre_abre_a_rota_sobre() {
        let mut e = EstadoApp::novo();
        assert_eq!(e.executar(ids::SOBRE), Some(Efeito::AbrirRota("sobre")));
    }

    #[test]
    fn sincronizar_espelha_tema_e_ribbon_da_casca() {
        let mut e = EstadoApp::novo();
        e.sincronizar("claro", true);
        assert_eq!(e.tema_atual().id, "claro");
        assert!(e.ribbon_recolhido());
        e.sincronizar("tema-que-nao-existe", false);
        assert_eq!(e.tema_atual().id, "claro", "tema desconhecido é ignorado");
        assert!(!e.ribbon_recolhido());
    }

    #[test]
    fn status_mostra_versao_ribbon_e_tema() {
        let t = EstadoApp::novo().textos_de_status();
        assert_eq!(t[0], format!("Piano Tutor {}", env!("CARGO_PKG_VERSION")));
        assert!(t[0].contains(VERSAO));
        assert_eq!(t[1], "Ribbon: expandido");
        assert_eq!(t[2], "Tema: Noite");
    }

    #[test]
    fn status_acompanha_ribbon_recolhido_e_tema() {
        let mut e = EstadoApp::novo();
        e.sincronizar("ebano", true);
        let t = e.textos_de_status();
        assert_eq!(t[1], "Ribbon: recolhido");
        assert_eq!(t[2], "Tema: Ébano");
    }
}
