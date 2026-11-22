//! Liga o estado e as ações do app à `bancada-casca` (trait `AppCasca`).

use crate::acoes;
use crate::espelho::EstadoEspelho;
use crate::estado::{BOAS_VINDAS, Efeito, EstadoApp, ROTA_TECLADO, TEMAS, VERSAO};
use crate::tela_espelho;
use crate::{TEMAS_JSON, TOKENS_JSON};
use bancada_casca::casca::Preferencias;
use bancada_casca::{
    Aba, Acao, AppCasca, ConfigCasca, Elo, GrupoNavegacao, ItemNavegacao, Local, ModoRibbon,
    Pedido, Rota, SegmentoStatus, Tom, abrir, egui, link_item, pedir,
};

const ROTA_INICIO: &str = "inicio";
const ROTA_SOBRE: &str = "sobre";

/// Título da janela.
pub const TITULO: &str = "Piano Tutor";

/// O app "Olá Piano Tutor".
#[derive(Debug, Default)]
pub struct AppPianoTutor {
    pub estado: EstadoApp,
    /// Estado da tela Espelho (F02). O gerenciador MIDI, quando existir, chama
    /// `espelho.aplicar_evento(..)`.
    pub espelho: EstadoEspelho,
}

impl AppPianoTutor {
    pub fn novo() -> Self {
        Self::default()
    }
}

/// Configuração da casca: título, tokens, temas e tema inicial.
pub fn config() -> ConfigCasca {
    let mut config = ConfigCasca::nova(TITULO, TOKENS_JSON, TEMAS_JSON);
    config.tema_inicial = TEMAS[0].id.to_owned();
    config
}

impl AppCasca for AppPianoTutor {
    fn acoes(&self) -> Vec<Acao> {
        acoes::acoes(&self.estado)
    }

    fn abas(&self) -> Vec<Aba> {
        acoes::abas()
    }

    fn navegacao(&self) -> Vec<GrupoNavegacao> {
        vec![GrupoNavegacao::novo(
            "Piano Tutor",
            vec![
                ItemNavegacao::novo(ROTA_INICIO, "Início", "inicio"),
                ItemNavegacao::novo(ROTA_SOBRE, "Sobre", "sobre"),
                ItemNavegacao::novo("monitor-midi", "Monitor MIDI", "midi")
                    .indisponivel("F01", "Chega na fase F01, com o REVAS e o MIDI real."),
                ItemNavegacao::novo(ROTA_TECLADO, "Teclado de 61 teclas", "teclado"),
            ],
        )]
    }

    fn rota_inicial(&self) -> Rota {
        Rota::nova(ROTA_INICIO)
    }

    fn trilha(&self, rota: &Rota) -> Vec<Elo> {
        match rota.como_texto() {
            ROTA_SOBRE => vec![Elo::link("Início", ROTA_INICIO), Elo::texto("Sobre")],
            ROTA_TECLADO => vec![
                Elo::link("Início", ROTA_INICIO),
                Elo::texto("Teclado de 61 teclas"),
            ],
            _ => vec![Elo::texto("Início")],
        }
    }

    fn desenhar_pagina(&mut self, ui: &mut egui::Ui, rota: &Rota, _local: Local) {
        if rota.como_texto() != ROTA_TECLADO {
            // Fora da tela Espelho ninguém recebe o "solta": não deixa nota presa.
            self.espelho.liberar_simulacao();
        }
        match rota.como_texto() {
            ROTA_TECLADO => tela_espelho::desenhar_tela(ui, &mut self.espelho),
            ROTA_SOBRE => {
                ui.heading("Sobre o Piano Tutor");
                ui.add_space(8.0);
                ui.label(format!("Versão {VERSAO} (fase F00, em preparação)."));
                ui.label("Professor assistido de piano e teclado, local-first.");
                ui.label("Janela construída sobre a bancada-casca (MIT OR Apache-2.0).");
                ui.add_space(8.0);
                // O link já pede a navegação à casca.
                link_item(ui, "Voltar ao início", ROTA_INICIO);
            }
            _ => {
                ui.heading("Olá, Piano Tutor");
                ui.add_space(8.0);
                for paragrafo in BOAS_VINDAS {
                    ui.label(paragrafo);
                    ui.add_space(4.0);
                }
            }
        }
    }

    fn status(&self) -> Vec<SegmentoStatus> {
        let [versao, midi, ribbon, tema] = self.estado.textos_de_status();
        vec![
            SegmentoStatus::novo("versao", versao),
            SegmentoStatus::novo("midi", midi)
                .tom(Tom::Aviso)
                .dica("Nenhum dispositivo MIDI aberto. Esta fase não usa MIDI."),
            SegmentoStatus::novo("ribbon", ribbon)
                .a_direita()
                .dica("Ctrl+F1 recolhe ou expande o Ribbon"),
            SegmentoStatus::novo("tema", tema)
                .a_direita()
                .dica("Ctrl+T alterna o tema"),
        ]
    }

    fn ao_executar(&mut self, ctx: &egui::Context, id: &str) {
        match self.estado.executar(id) {
            Some(Efeito::TrocarTema(tema)) => pedir(ctx, Pedido::Tema(tema.to_owned())),
            Some(Efeito::AbrirRota(rota)) => abrir(ctx, rota),
            None => {}
        }
    }

    fn ao_mudar_preferencias(&mut self, preferencias: &Preferencias) {
        self.estado.sincronizar(
            &preferencias.tema,
            preferencias.modo_ribbon == ModoRibbon::Recolhido,
        );
    }
}

#[cfg(test)]
mod testes {
    //! Testes "sem janela": a casca desenha num `egui::Context` com entrada sintética.
    use super::*;
    use bancada_casca::Casca;
    use bancada_casca::egui::{Context, Event, Key, Modifiers, RawInput, Rect, pos2, vec2};
    use bancada_casca::tema::CatalogoDeTemas;

    fn casca() -> Casca<AppPianoTutor> {
        Casca::nova(AppPianoTutor::novo(), config()).expect("tokens e temas do app")
    }

    fn quadro(c: &mut Casca<AppPianoTutor>, ctx: &Context, eventos: Vec<Event>, mods: Modifiers) {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
            events: std::iter::once(Event::ModifiersChanged(mods))
                .chain(eventos)
                .collect(),
            ..Default::default()
        };
        ctx.run_ui(entrada, |ui| c.desenhar(ui))
            .drop_without_applying_deltas();
    }

    fn apertar(c: &mut Casca<AppPianoTutor>, ctx: &Context, key: Key, mods: Modifiers) {
        let evento = Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: mods,
        };
        quadro(c, ctx, vec![evento], mods);
        quadro(c, ctx, Vec::new(), Modifiers::NONE);
    }

    #[test]
    fn catalogo_de_temas_do_app_bate_com_a_lista_do_estado() {
        let catalogo = CatalogoDeTemas::ler(TOKENS_JSON, TEMAS_JSON).expect("tokens e temas");
        let do_json: Vec<&str> = catalogo.temas().iter().map(|t| t.id.as_str()).collect();
        let do_estado: Vec<&str> = TEMAS.iter().map(|t| t.id).collect();
        assert_eq!(do_json, do_estado);
        assert!(do_json.len() >= 2);
    }

    #[test]
    fn a_janela_desenha_sem_erro_de_acoes_ou_abas() {
        let mut c = casca();
        let ctx = Context::default();
        for _ in 0..3 {
            quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
        }
        assert!(c.estado.erros.is_empty(), "{:?}", c.estado.erros);
    }

    #[test]
    fn ctrl_f1_recolhe_e_expande_o_ribbon_e_a_barra_de_status_acompanha() {
        let mut c = casca();
        let ctx = Context::default();
        quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
        assert_ne!(c.estado.ribbon.modo, ModoRibbon::Recolhido);
        assert!(!c.app.estado.ribbon_recolhido());

        apertar(&mut c, &ctx, Key::F1, Modifiers::COMMAND);
        assert_eq!(c.estado.ribbon.modo, ModoRibbon::Recolhido);
        assert!(c.app.estado.ribbon_recolhido());
        assert_eq!(c.app.estado.textos_de_status()[2], "Ribbon: recolhido");

        apertar(&mut c, &ctx, Key::F1, Modifiers::COMMAND);
        assert_ne!(c.estado.ribbon.modo, ModoRibbon::Recolhido);
        assert!(!c.app.estado.ribbon_recolhido());
        assert!(c.estado.erros.is_empty(), "{:?}", c.estado.erros);
    }

    #[test]
    fn ctrl_t_alterna_o_tema_na_casca_e_no_estado() {
        let mut c = casca();
        let ctx = Context::default();
        quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
        assert_eq!(c.tema().tema.id, "noite");

        apertar(&mut c, &ctx, Key::T, Modifiers::COMMAND);
        assert_eq!(c.tema().tema.id, "claro");
        assert_eq!(c.app.estado.tema_atual().id, "claro");
        assert_eq!(c.app.estado.textos_de_status()[3], "Tema: Claro");

        apertar(&mut c, &ctx, Key::T, Modifiers::COMMAND);
        apertar(&mut c, &ctx, Key::T, Modifiers::COMMAND);
        assert_eq!(c.tema().tema.id, "noite", "volta ao primeiro");
        assert!(c.estado.erros.is_empty(), "{:?}", c.estado.erros);
    }

    #[test]
    fn desenha_em_todos_os_temas_e_modos_do_ribbon_sem_panico() {
        let mut c = casca();
        let ctx = Context::default();
        for tema in TEMAS {
            for modo in [
                ModoRibbon::Classico,
                ModoRibbon::Simples,
                ModoRibbon::Recolhido,
            ] {
                c.definir_aparencia(Some(tema.id), None, None);
                c.estado.ribbon.definir_modo(modo);
                quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
                quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
            }
        }
        assert!(c.estado.erros.is_empty(), "{:?}", c.estado.erros);
    }

    #[test]
    fn item_de_navegacao_do_teclado_esta_disponivel_e_sem_em_breve() {
        let app = AppPianoTutor::novo();
        let itens: Vec<ItemNavegacao> = app.navegacao().into_iter().flat_map(|g| g.itens).collect();
        let teclado = itens
            .iter()
            .find(|i| i.rota.como_texto() == ROTA_TECLADO)
            .expect("item do teclado");
        assert!(teclado.indisponivel.is_none());
        assert!(!teclado.rotulo.contains("em breve"));
        let midi = itens
            .iter()
            .find(|i| i.rota.como_texto() == "monitor-midi")
            .expect("item do monitor MIDI");
        assert!(
            midi.indisponivel.is_some(),
            "só o teclado deixa de ser em breve"
        );
    }

    #[test]
    fn a_acao_do_ribbon_abre_a_tela_espelho_e_ela_mostra_o_aviso_de_simulacao() {
        let mut c = casca();
        let ctx = Context::default();
        quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
        assert_eq!(c.estado.historico.atual().como_texto(), ROTA_INICIO);

        bancada_casca::executar(&ctx, crate::estado::ids::TECLADO_61);
        for _ in 0..3 {
            quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
        }
        assert_eq!(c.estado.historico.atual().como_texto(), ROTA_TECLADO);
        let resumo = c
            .app
            .espelho
            .ultimo_desenho
            .as_ref()
            .expect("desenhou o espelho");
        assert_eq!(resumo.desenhadas, 61);
        assert!(resumo.aviso_simulacao_visivel);
        assert!(c.estado.erros.is_empty(), "{:?}", c.estado.erros);
    }

    #[test]
    fn sair_da_tela_espelho_solta_as_notas_da_simulacao() {
        let mut c = casca();
        let ctx = Context::default();
        c.abrir(&ctx, ROTA_TECLADO.into(), Local::Centro);
        quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
        apertar(&mut c, &ctx, Key::A, Modifiers::NONE);
        assert_eq!(c.app.espelho.teclas().quantidade(), 1);
        c.abrir(&ctx, ROTA_INICIO.into(), Local::Centro);
        quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
        assert_eq!(c.app.espelho.teclas().quantidade(), 0);
    }

    #[test]
    fn segmentos_de_status_tem_versao_e_midi_nao_conectado() {
        let app = AppPianoTutor::novo();
        let textos: Vec<String> = app.status().into_iter().map(|s| s.texto).collect();
        assert!(textos.iter().any(|t| t.contains(VERSAO)), "{textos:?}");
        assert!(textos.iter().any(|t| t == "MIDI: não conectado"));
    }
}
