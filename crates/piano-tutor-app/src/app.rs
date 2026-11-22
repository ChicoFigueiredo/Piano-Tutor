//! Liga o estado e as ações do app à `bancada-casca` (trait `AppCasca`) e faz o roteamento da
//! M1: **teclado físico → MIDI → Espelho + som**.
//!
//! A cada quadro ([`AppPianoTutor::processar_quadro`]) o app chama
//! [`EstadoMonitor::atualizar`] (que chama `GerenciadorMidi::atualizar`), entrega cada evento ao
//! Espelho (origem `DispositivoFisico`) e ao [`ServicoSom`], trata `LimparNotas` (Espelho limpo e
//! pânico do motor) e agenda o próximo quadro com `request_repaint_after(proxima_varredura_em)`.
//! A thread do backend MIDI também pede um quadro a cada mensagem, para o som não esperar.
//!
//! **Relógio único.** [`Dispositivos`] cria um só `Arc<dyn Relogio>` e o entrega ao gerenciador
//! MIDI (que carimba os eventos) e à saída de áudio (que ancora frames ↔ ns no callback).
//!
//! **Produtor único da fila de áudio.** Só esta thread (UI/controle) envia comandos ao motor,
//! pelo [`ServicoSom`].

use crate::acoes;
use crate::espelho::EstadoEspelho;
use crate::estado::{
    BOAS_VINDAS, Efeito, EstadoApp, ROTA_MONITOR, ROTA_TECLADO, ROTA_TIMBRES, TEMAS, VERSAO,
};
use crate::monitor::{EstadoMonitor, pasta_de_sessoes_do_ambiente};
use crate::som::{EstadoSom, SF2_PADRAO, ServicoSom, abridor_cpal, carregador_arquivo};
use crate::tela_timbres::{AcaoTimbres, EstadoTimbres};
use crate::{TEMAS_JSON, TOKENS_JSON};
use crate::{tela_espelho, tela_monitor, tela_timbres};
use bancada_casca::casca::Preferencias;
use bancada_casca::{
    Aba, Acao, AppCasca, ConfigCasca, Elo, GrupoNavegacao, ItemNavegacao, Local, ModoRibbon,
    Pedido, Rota, SegmentoStatus, Tom, abrir, egui, link_item, pedir,
};
use piano_tutor_core::midi::EventoMidi;
use piano_tutor_core::teclado::{AcaoTecla, EventoTecla, OrigemEventos};
use piano_tutor_midi::{
    EstadoConexao, FonteFalsa, FonteMidir, FontePortas, Relogio, RelogioMonotonico, SaidaMidi,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const ROTA_INICIO: &str = "inicio";
const ROTA_SOBRE: &str = "sobre";

/// Título da janela.
pub const TITULO: &str = "Piano Tutor";

/// O que o app usa do mundo de fora: fonte de portas MIDI, relógio comum, som e pasta de dados.
pub struct Dispositivos {
    pub fonte_midi: Box<dyn FontePortas>,
    /// Relógio único: o mesmo `Arc` vai para o gerenciador MIDI e para o serviço de som.
    pub relogio: Arc<dyn Relogio>,
    pub som: ServicoSom,
    pub pasta_sessoes: Option<PathBuf>,
}

impl Dispositivos {
    /// Sem hardware: nenhuma porta MIDI (fonte falsa vazia) e som inativo. Para testes.
    pub fn sem_hardware() -> Self {
        let relogio: Arc<dyn Relogio> = Arc::new(RelogioMonotonico::novo());
        Self {
            fonte_midi: Box::new(FonteFalsa::nova(Vec::<String>::new())),
            som: ServicoSom::inativo(Arc::clone(&relogio)),
            relogio,
            pasta_sessoes: None,
        }
    }

    /// Os do sistema: `midir`/ALSA, saída `cpal` padrão, SF2 padrão (carregado em segundo
    /// plano) e a pasta XDG de dados do usuário.
    pub fn do_sistema() -> Self {
        let relogio: Arc<dyn Relogio> = Arc::new(RelogioMonotonico::novo());
        let mut som = ServicoSom::novo(Arc::clone(&relogio), abridor_cpal(), carregador_arquivo());
        som.carregar_sf2(SF2_PADRAO);
        Self {
            fonte_midi: Box::new(FonteMidir::nova()),
            som,
            relogio,
            pasta_sessoes: pasta_de_sessoes_do_ambiente(),
        }
    }
}

/// O app Piano Tutor.
pub struct AppPianoTutor {
    pub estado: EstadoApp,
    /// Estado da tela Espelho (F02). Recebe os eventos do gerenciador MIDI e da simulação.
    pub espelho: EstadoEspelho,
    /// Monitor MIDI (F01): o gerenciador, a tabela de eventos e o log.
    pub monitor: EstadoMonitor,
    /// Som (F03): SoundFont, saída, timbre, volume e pânico.
    pub som: ServicoSom,
    pub timbres: EstadoTimbres,
    relogio: Arc<dyn Relogio>,
    ultimo_quadro: Option<u64>,
    aviso_instalado: bool,
}

impl std::fmt::Debug for AppPianoTutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppPianoTutor")
            .field("estado", &self.estado)
            .field("monitor", &self.monitor)
            .field("som", &self.som)
            .finish_non_exhaustive()
    }
}

impl Default for AppPianoTutor {
    fn default() -> Self {
        Self::novo()
    }
}

impl AppPianoTutor {
    /// App sem hardware (testes): nenhuma porta MIDI e som inativo.
    pub fn novo() -> Self {
        Self::com_dispositivos(Dispositivos::sem_hardware())
    }

    /// App com MIDI, áudio e SF2 do sistema (o executável).
    pub fn do_sistema() -> Self {
        Self::com_dispositivos(Dispositivos::do_sistema())
    }

    pub fn com_dispositivos(d: Dispositivos) -> Self {
        Self {
            estado: EstadoApp::novo(),
            espelho: EstadoEspelho::novo(),
            monitor: EstadoMonitor::novo(d.fonte_midi, Arc::clone(&d.relogio), d.pasta_sessoes),
            som: d.som,
            timbres: EstadoTimbres::default(),
            relogio: d.relogio,
            ultimo_quadro: None,
            aviso_instalado: false,
        }
    }

    /// O relógio comum (MIDI e áudio).
    pub fn relogio(&self) -> &Arc<dyn Relogio> {
        &self.relogio
    }

    /// Trabalho de todo quadro, uma vez por quadro (a casca pode desenhar várias páginas).
    pub fn processar_quadro(&mut self, ctx: &egui::Context) {
        let nr = ctx.cumulative_frame_nr();
        if self.ultimo_quadro == Some(nr) {
            return;
        }
        self.ultimo_quadro = Some(nr);
        if !self.aviso_instalado {
            let c = ctx.clone();
            let aviso: Arc<dyn Fn() + Send + Sync> = Arc::new(move || c.request_repaint());
            self.monitor
                .gerenciador_mut()
                .definir_aviso_de_chegada(Arc::clone(&aviso));
            self.som.definir_aviso(aviso);
            self.aviso_instalado = true;
        }
        self.tick();
        let mut proximo = self.monitor.proxima_varredura_em();
        if self.som.carregando().is_some() {
            proximo = proximo.min(Duration::from_millis(250));
        }
        ctx.request_repaint_after(proximo.max(Duration::from_millis(1)));
    }

    /// Um passo do pipeline, sem janela: MIDI → Espelho + som, simulação → som, som.
    pub fn tick(&mut self) {
        let saidas = self.monitor.atualizar();
        self.rotear(saidas);
        self.encaminhar_simulacao();
        self.som.atualizar();
    }

    /// Aplica as saídas do gerenciador MIDI, na ordem.
    pub fn rotear(&mut self, saidas: Vec<SaidaMidi>) {
        for s in saidas {
            match s {
                SaidaMidi::Evento(ev) => {
                    if let Some(t) = evento_tecla(&ev) {
                        // Nota fora da faixa fica registrada no Espelho; as outras não mudam.
                        let _ = self.espelho.aplicar_evento(t);
                    }
                    self.som.evento_midi(&ev);
                }
                SaidaMidi::LimparNotas => {
                    self.espelho.limpar_tudo();
                    self.som.limpar_notas();
                }
                SaidaMidi::EstadoMudou(_) => {}
            }
        }
    }

    /// Leva ao som as notas que a simulação (mouse e teclado do PC) aplicou no Espelho.
    pub fn encaminhar_simulacao(&mut self) {
        for ev in self.espelho.drenar_simulados() {
            self.som.nota_simulada(ev);
        }
    }

    /// Pânico: all-notes-off imediato no motor e Espelho limpo (simulação inclusive).
    pub fn panico(&mut self) {
        self.espelho.liberar_simulacao();
        // As liberações da simulação perdem o sentido: o pânico corta tudo.
        let _ = self.espelho.drenar_simulados();
        self.espelho.limpar_tudo();
        self.som.panico();
    }

    pub fn executar_timbres(&mut self, pedidos: Vec<AcaoTimbres>) {
        for p in pedidos {
            match p {
                AcaoTimbres::CarregarSf2(caminho) if caminho.is_empty() => {
                    self.timbres.erro_timbre = Some("Informe o caminho de um arquivo .sf2.".into());
                }
                AcaoTimbres::CarregarSf2(caminho) => {
                    self.timbres.erro_timbre = None;
                    self.som.carregar_sf2(caminho);
                }
                AcaoTimbres::Timbre { banco, programa } => {
                    self.timbres.erro_timbre = self.som.selecionar_timbre(banco, programa).err();
                }
                AcaoTimbres::Volume(v) => self.som.definir_volume(v),
                AcaoTimbres::Panico => self.panico(),
                AcaoTimbres::ReabrirSaida => self.som.reabrir_saida(),
            }
        }
    }
}

/// Nota do teclado físico para o Espelho (origem `DispositivoFisico`). Controles e programa não
/// mexem no Espelho.
pub fn evento_tecla(ev: &EventoMidi) -> Option<EventoTecla> {
    let (acao, nota) = match *ev {
        EventoMidi::NotaLigada { nota, .. } => (AcaoTecla::Ligar, nota),
        EventoMidi::NotaDesligada { nota, .. } => (AcaoTecla::Desligar, nota),
        _ => return None,
    };
    Some(EventoTecla {
        origem: OrigemEventos::DispositivoFisico,
        acao,
        nota,
    })
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
                ItemNavegacao::novo(ROTA_MONITOR, "Monitor MIDI", "midi"),
                ItemNavegacao::novo(ROTA_TECLADO, "Teclado de 61 teclas", "teclado"),
                ItemNavegacao::novo(ROTA_TIMBRES, "Timbres", "som"),
                ItemNavegacao::novo(ROTA_SOBRE, "Sobre", "sobre"),
            ],
        )]
    }

    fn rota_inicial(&self) -> Rota {
        Rota::nova(ROTA_INICIO)
    }

    fn trilha(&self, rota: &Rota) -> Vec<Elo> {
        let inicio = || Elo::link("Início", ROTA_INICIO);
        match rota.como_texto() {
            ROTA_SOBRE => vec![inicio(), Elo::texto("Sobre")],
            ROTA_TECLADO => vec![inicio(), Elo::texto("Teclado de 61 teclas")],
            ROTA_MONITOR => vec![inicio(), Elo::texto("Monitor MIDI")],
            ROTA_TIMBRES => vec![inicio(), Elo::texto("Timbres")],
            _ => vec![Elo::texto("Início")],
        }
    }

    fn desenhar_pagina(&mut self, ui: &mut egui::Ui, rota: &Rota, _local: Local) {
        self.processar_quadro(ui.ctx());
        if rota.como_texto() != ROTA_TECLADO {
            // Fora da tela Espelho ninguém recebe o "solta": não deixa nota presa.
            self.espelho.liberar_simulacao();
        }
        match rota.como_texto() {
            ROTA_TECLADO => tela_espelho::desenhar_tela(ui, &mut self.espelho),
            ROTA_MONITOR => {
                tela_monitor::desenhar_tela(ui, &mut self.monitor);
            }
            ROTA_TIMBRES => {
                let pedidos = tela_timbres::desenhar_tela(ui, &self.som, &mut self.timbres);
                self.executar_timbres(pedidos);
            }
            ROTA_SOBRE => {
                ui.heading("Sobre o Piano Tutor");
                ui.add_space(8.0);
                ui.label(format!("Versão {VERSAO} (marco M1, em homologação)."));
                ui.label("Professor assistido de piano e teclado, local-first.");
                ui.label("Janela construída sobre a bancada-casca (MIT OR Apache-2.0).");
                ui.label(
                    "Som: rustysynth (MIT) e cpal (Apache-2.0). SoundFont padrão FluidR3_GM \
                     (MIT), instalado pelo pacote fluid-soundfont-gm.",
                );
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
        // Notas da simulação tocam já neste quadro (sem esperar o próximo).
        self.encaminhar_simulacao();
    }

    fn status(&self) -> Vec<SegmentoStatus> {
        let [versao, ribbon, tema] = self.estado.textos_de_status();
        let tom_midi = match self.monitor.estado() {
            EstadoConexao::Conectado { .. } => Tom::Sucesso,
            EstadoConexao::Erro { .. } => Tom::Perigo,
            EstadoConexao::Desconectado | EstadoConexao::Conectando { .. } => Tom::Aviso,
        };
        let dica_midi = self
            .monitor
            .detalhe_estado()
            .unwrap_or_else(|| "Clique para abrir o Monitor MIDI".to_owned());
        let (tom_audio, dica_audio) = match self.som.estado() {
            EstadoSom::Pronto => (
                Tom::Sucesso,
                self.som.descricao_saida().unwrap_or_default().to_owned(),
            ),
            EstadoSom::Carregando => (Tom::Info, "Lendo o SoundFont".to_owned()),
            EstadoSom::Indisponivel | EstadoSom::SemSoundFont => (
                Tom::Aviso,
                self.som
                    .erro_saida()
                    .or(self.som.erro_sf2())
                    .unwrap_or("Sem som")
                    .to_owned(),
            ),
        };
        vec![
            SegmentoStatus::novo("versao", versao),
            SegmentoStatus::novo("midi", self.monitor.texto_status())
                .tom(tom_midi)
                .dica(dica_midi)
                .clicavel(),
            SegmentoStatus::novo("audio", self.som.texto_status())
                .tom(tom_audio)
                .dica(dica_audio)
                .clicavel(),
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
            Some(Efeito::Panico) => {
                self.panico();
                ctx.request_repaint();
            }
            None => {}
        }
    }

    fn ao_clicar_status(&mut self, ctx: &egui::Context, id: &str) {
        match id {
            "midi" => abrir(ctx, ROTA_MONITOR),
            "audio" => abrir(ctx, ROTA_TIMBRES),
            _ => {}
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
    use crate::som::apoio_testes::{BLOCO, MotorDeTeste, servico_pronto, taxa};
    use bancada_casca::Casca;
    use bancada_casca::egui::{Context, Event, Key, Modifiers, RawInput, Rect, pos2, vec2};
    use bancada_casca::tema::CatalogoDeTemas;
    use piano_tutor_audio::relogio::frames_para_ns;
    use piano_tutor_audio::sintetizador::RegistroFalso;
    use piano_tutor_midi::RelogioManual;

    const KB: &str = "KB-330:KB-330 MIDI 1 20:0";

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

    /// App com teclado falso, relógio manual comum e motor de áudio de teste.
    struct Bancada {
        app: AppPianoTutor,
        fonte: FonteFalsa,
        relogio: RelogioManual,
        motor: MotorDeTeste,
    }

    fn bancada() -> Bancada {
        let relogio = RelogioManual::novo();
        let comum: Arc<dyn Relogio> = Arc::new(relogio.clone());
        let fonte = FonteFalsa::nova([KB]);
        let (som, motor) = servico_pronto(Arc::clone(&comum));
        let app = AppPianoTutor::com_dispositivos(Dispositivos {
            fonte_midi: Box::new(fonte.clone()),
            relogio: comum,
            som,
            pasta_sessoes: None,
        });
        Bancada {
            app,
            fonte,
            relogio,
            motor,
        }
    }

    fn ligadas(r: &[RegistroFalso]) -> Vec<(u64, u8)> {
        r.iter()
            .filter_map(|e| match *e {
                RegistroFalso::Ligada { frame, nota, .. } => Some((frame, nota)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn o_relogio_do_midi_e_o_do_audio_sao_o_mesmo_tipo_e_a_mesma_instancia() {
        // Mesmo tipo: as duas crates reexportam o do núcleo.
        fn mesmo_tipo(
            r: piano_tutor_midi::RelogioMonotonico,
        ) -> piano_tutor_audio::RelogioMonotonico {
            r
        }
        let r = mesmo_tipo(piano_tutor_core::tempo::RelogioMonotonico::novo());
        let _ = r.agora_ns();
        // Mesma instância: o app guarda o `Arc` que entregou ao gerenciador e ao som.
        let b = bancada();
        b.relogio.avancar(Duration::from_millis(5));
        assert_eq!(b.app.relogio().agora_ns(), 5_000_000);
    }

    #[test]
    fn evento_do_teclado_chega_ao_espelho_e_ao_sintetizador_no_frame_esperado() {
        let mut b = bancada();
        b.app.tick();
        b.app
            .monitor
            .executar(crate::monitor::AcaoMonitor::Conectar);
        b.app.tick();
        assert_eq!(b.app.monitor.texto_estado(), "Conectado");

        // O "callback" desperta em t0 = 1 s do relógio comum (lido pela saída de áudio).
        const T0: u64 = 1_000_000_000;
        b.relogio.avancar(Duration::from_nanos(T0));
        b.motor.renderizar_no_relogio(BLOCO); // âncora: frame 0 ↔ T0
        let antes = ligadas(&b.motor.registro()).len();

        // A tecla chega no instante em que o próximo bloco desperta; a fonte a carimba com o
        // mesmo relógio (o que o gerenciador recebeu), não com um timestamp do teste.
        b.relogio
            .avancar(Duration::from_nanos(frames_para_ns(BLOCO as u64, taxa())));
        assert!(b.fonte.injetar(&[0x90, 60, 100]));
        b.app.tick();
        // Espelho: nota ligada, origem física, aviso de simulação some.
        assert_eq!(b.app.espelho.teclas().quantidade(), 1);
        assert_eq!(b.app.espelho.aviso_simulacao(), None);
        assert!(matches!(
            b.app.monitor.eventos().last().map(|l| l.ts_ns),
            Some(ts) if ts == T0 + frames_para_ns(BLOCO as u64, taxa())
        ));

        b.motor.renderizar_no_relogio(BLOCO * 8);
        let novas = ligadas(&b.motor.registro())[antes..].to_vec();
        let esperado = BLOCO as u64 + b.motor.latencia_alvo_frames();
        assert_eq!(
            novas,
            vec![(esperado, 60)],
            "frame = instante + latência alvo"
        );
    }

    #[test]
    fn desconexao_limpa_espelho_e_motor_e_status_mostra_aguardando() {
        let mut b = bancada();
        b.app.tick();
        b.app
            .monitor
            .executar(crate::monitor::AcaoMonitor::Conectar);
        b.app.tick();
        for n in [60, 64, 67] {
            assert!(b.fonte.injetar(&[0x90, n, 100]));
        }
        b.app.tick();
        b.motor.renderizar(BLOCO * 8, 0);
        assert_eq!(b.app.espelho.teclas().quantidade(), 3);
        assert_eq!(b.motor.notas_ativas(), 3);

        b.fonte.remover_porta(KB);
        b.relogio.avancar(Duration::from_secs(1));
        b.app.tick();
        b.motor.renderizar(BLOCO, 0);
        assert_eq!(b.app.espelho.teclas().quantidade(), 0, "Espelho limpo");
        assert_eq!(b.motor.notas_ativas(), 0, "motor silenciado");
        assert!(
            b.motor
                .registro()
                .iter()
                .any(|e| matches!(e, RegistroFalso::Silenciar { .. }))
        );
        assert_eq!(
            b.app.monitor.texto_status(),
            "MIDI: Desconectado — aguardando a porta voltar"
        );
        let textos: Vec<String> = b.app.status().into_iter().map(|s| s.texto).collect();
        assert!(
            textos
                .iter()
                .any(|t| t == "MIDI: Desconectado — aguardando a porta voltar")
        );
    }

    #[test]
    fn simulacao_toca_no_motor_quando_ha_audio() {
        let mut b = bancada();
        let n = piano_tutor_core::NotaMidi::nova(62).expect("nota");
        b.app.espelho.pressionar_simulado(n);
        b.app.encaminhar_simulacao();
        b.motor.renderizar(BLOCO, 0);
        assert_eq!(b.motor.notas_ativas(), 1);
        assert!(ligadas(&b.motor.registro()).iter().any(|(_, x)| *x == 62));
        b.app.espelho.soltar_simulado(n);
        b.app.encaminhar_simulacao();
        b.motor.renderizar(BLOCO, 0);
        assert_eq!(b.motor.notas_ativas(), 0);
    }

    #[test]
    fn panico_corta_o_motor_e_limpa_o_espelho() {
        let mut b = bancada();
        b.app.tick();
        b.app
            .monitor
            .executar(crate::monitor::AcaoMonitor::Conectar);
        b.app.tick();
        assert!(b.fonte.injetar(&[0x90, 60, 100]));
        b.app.tick();
        b.app
            .espelho
            .pressionar_simulado(piano_tutor_core::NotaMidi::nova(72).expect("nota"));
        b.app.encaminhar_simulacao();
        b.motor.renderizar(BLOCO * 8, 0);
        assert_eq!(b.motor.notas_ativas(), 2);
        b.app.panico();
        b.motor.renderizar(BLOCO, 0);
        assert_eq!(b.motor.notas_ativas(), 0);
        assert_eq!(b.app.espelho.teclas().quantidade(), 0);
        assert_eq!(b.app.som.panicos(), 1);
    }

    #[test]
    fn troca_de_preset_pela_tela_e_sf2_invalido_pela_tela_nao_derrubam() {
        let mut b = bancada();
        b.app.executar_timbres(vec![AcaoTimbres::Timbre {
            banco: 0,
            programa: 19,
        }]);
        assert_eq!(b.app.som.timbre().programa, 19);
        assert_eq!(b.app.timbres.erro_timbre, None);
        b.app.executar_timbres(vec![AcaoTimbres::Timbre {
            banco: 0,
            programa: 21,
        }]);
        assert!(
            b.app.timbres.erro_timbre.is_some(),
            "acordeon não existe no SF2 sintético"
        );
        let lixo = crate::som::apoio_testes::arquivo_com("lixo-app.sf2", b"nao e sf2");
        b.app
            .executar_timbres(vec![AcaoTimbres::CarregarSf2(lixo.display().to_string())]);
        assert!(b.app.som.aguardar_carregamento(Duration::from_secs(10)));
        assert_eq!(
            b.app.som.estado(),
            EstadoSom::Pronto,
            "som anterior continua"
        );
        assert!(b.app.som.erro_sf2().is_some());
        b.app
            .executar_timbres(vec![AcaoTimbres::CarregarSf2(String::new())]);
        assert!(b.app.timbres.erro_timbre.is_some());
    }

    #[test]
    fn sem_audio_o_app_funciona_e_status_diz_indisponivel() {
        let mut c = casca();
        let ctx = Context::default();
        for rota in [ROTA_INICIO, ROTA_MONITOR, ROTA_TECLADO, ROTA_TIMBRES] {
            c.abrir(&ctx, rota.into(), Local::Centro);
            quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
            quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
        }
        let textos: Vec<String> = c.app.status().into_iter().map(|s| s.texto).collect();
        assert!(
            textos.iter().any(|t| t == "Áudio: indisponível"),
            "{textos:?}"
        );
        assert!(
            textos.iter().any(|t| t == "MIDI: Desconectado"),
            "{textos:?}"
        );
        assert!(c.estado.erros.is_empty(), "{:?}", c.estado.erros);
    }

    #[test]
    fn ctrl_ponto_dispara_o_panico_e_limpa_a_simulacao() {
        let mut c = casca();
        let ctx = Context::default();
        c.abrir(&ctx, ROTA_TECLADO.into(), Local::Centro);
        quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
        // Tecla A segurada (sem soltar).
        quadro(
            &mut c,
            &ctx,
            vec![Event::Key {
                key: Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            Modifiers::NONE,
        );
        assert_eq!(c.app.espelho.teclas().quantidade(), 1);
        apertar(&mut c, &ctx, Key::Period, Modifiers::COMMAND);
        assert_eq!(c.app.espelho.teclas().quantidade(), 0);
        assert!(c.estado.erros.is_empty(), "{:?}", c.estado.erros);
    }

    #[test]
    fn a_fonte_da_janela_tem_os_glifos_usados_nos_textos_das_telas() {
        let mut c = casca();
        let ctx = Context::default();
        quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
        quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
        let fonte = egui::FontId::proportional(14.0);
        let faltam: String = "·ÁÉÓÚàáâãçéêíóôõú—–…"
            .chars()
            .filter(|ch| !ctx.fonts_mut(|f| f.has_glyph(&fonte, *ch)))
            .collect();
        assert!(faltam.is_empty(), "glifos ausentes: {faltam:?}");
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
        assert_eq!(c.app.estado.textos_de_status()[1], "Ribbon: recolhido");

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
        assert_eq!(c.app.estado.textos_de_status()[2], "Tema: Claro");

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
    fn itens_de_navegacao_estao_todos_disponiveis() {
        let app = AppPianoTutor::novo();
        let itens: Vec<ItemNavegacao> = app.navegacao().into_iter().flat_map(|g| g.itens).collect();
        for rota in [
            ROTA_INICIO,
            ROTA_MONITOR,
            ROTA_TECLADO,
            ROTA_TIMBRES,
            ROTA_SOBRE,
        ] {
            let item = itens
                .iter()
                .find(|i| i.rota.como_texto() == rota)
                .unwrap_or_else(|| panic!("falta o item {rota}"));
            assert!(item.indisponivel.is_none(), "{rota}");
            assert!(!item.rotulo.contains("em breve"));
        }
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
    fn as_acoes_do_ribbon_abrem_monitor_e_timbres() {
        let mut c = casca();
        let ctx = Context::default();
        quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
        for (id, rota) in [
            (crate::estado::ids::MONITOR_MIDI, ROTA_MONITOR),
            (crate::estado::ids::TIMBRES, ROTA_TIMBRES),
        ] {
            bancada_casca::executar(&ctx, id);
            for _ in 0..3 {
                quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
            }
            assert_eq!(c.estado.historico.atual().como_texto(), rota);
        }
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
    fn segmentos_de_status_tem_versao_midi_e_audio() {
        let app = AppPianoTutor::novo();
        let textos: Vec<String> = app.status().into_iter().map(|s| s.texto).collect();
        assert!(textos.iter().any(|t| t.contains(VERSAO)), "{textos:?}");
        assert!(
            textos.iter().any(|t| t == "MIDI: Desconectado"),
            "{textos:?}"
        );
        assert!(
            textos.iter().any(|t| t.starts_with("Áudio: ")),
            "{textos:?}"
        );
    }
}
