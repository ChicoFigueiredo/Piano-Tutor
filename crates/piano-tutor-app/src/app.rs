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

use crate::abertura::{self, Abertura};
use crate::acoes;
use crate::acordes::EstadoAcordes;
use crate::baixar_sf2::{BaixadorSf2, FLUIDR3_GM, abridor_http, caminho_baixado};
use crate::calibracao::{Calibracao, PerfilTeclado};
use crate::chamada_app::ChamadaAtiva;
use crate::cursos::{EstadoCursos, ObservadorFabrica};
use crate::dedos::{NOMENCLATURAS, NomenclaturaDedos};
use crate::espelho::EstadoEspelho;
use crate::estado::{
    Efeito, EstadoApp, ROTA_ACORDES, ROTA_CALIBRACAO, ROTA_CURSOS, ROTA_EXERCICIOS, ROTA_GRAVACAO,
    ROTA_MONITOR, ROTA_PARTITURA, ROTA_RITMOS, ROTA_TECLADO, ROTA_TIMBRES, TEMAS, VERSAO, ids,
};
use crate::exercicios::EstadoExercicios;
use crate::gravacao::EstadoGravacao;
use crate::monitor::{EstadoMonitor, pasta_de_sessoes_do_ambiente};
use crate::mpv::{ExecutorReal, Midia};
use crate::perfis::{ARQUIVO_BANCO, BancoPreferencias, Gravacao, Perfil, secoes};
use crate::preferencias::{PreferenciasExibicao, PreferenciasPartitura, PreferenciasSom};
use crate::professor::Professor;
use crate::professor_falante::{VozMuda, voz_do_sistema};
use crate::ritmo::EstadoRitmo;
use crate::seletor_instrumento::{self, EstadoSeletor};
use crate::som::{
    ConfigTeclado, EstadoSom, ServicoSom, abridor_cpal, carregador_arquivo, sf2_encontrado,
};
use crate::tela_calibracao::{AcaoCalibracao, ContextoCalibracao};
use crate::tela_gravacao;
use crate::tela_inicio::{self, Resumo};
use crate::tela_partitura::{self, EstadoPartitura};
use crate::tela_revisao::{self, EstadoRevisao, ROTA_REVISAO};
use crate::tela_timbres::{AcaoTimbres, EstadoTimbres};
use crate::{TEMAS_JSON, TOKENS_JSON};
use crate::{tela_acordes, tela_calibracao, tela_cursos, tela_exercicios, tela_ritmos};
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
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

mod cursos_app;

const ROTA_INICIO: &str = "inicio";
const ROTA_SOBRE: &str = "sobre";

/// Telas que o app reabre na próxima abertura (a Calibração e o Sobre ficam de fora).
const ROTAS_LEMBRADAS: [&str; 11] = [
    ROTA_INICIO,
    ROTA_TECLADO,
    ROTA_MONITOR,
    ROTA_TIMBRES,
    ROTA_RITMOS,
    ROTA_ACORDES,
    ROTA_EXERCICIOS,
    ROTA_PARTITURA,
    ROTA_GRAVACAO,
    ROTA_CURSOS,
    ROTA_REVISAO,
];

/// A rota (de `ROTAS_LEMBRADAS`) que `texto` nomeia, se nomear alguma.
fn rota_lembrada(texto: &str) -> Option<&'static str> {
    ROTAS_LEMBRADAS.into_iter().find(|r| *r == texto)
}

/// Título da janela.
pub const TITULO: &str = "Piano Tutor";

/// O que o app usa do mundo de fora: fonte de portas MIDI, relógio comum, som e pasta de dados.
pub struct Dispositivos {
    pub fonte_midi: Box<dyn FontePortas>,
    /// Relógio único: o mesmo `Arc` vai para o gerenciador MIDI e para o serviço de som.
    pub relogio: Arc<dyn Relogio>,
    pub som: ServicoSom,
    pub pasta_sessoes: Option<PathBuf>,
    /// Baixa o SF2 padrão quando ele não existe. `None` nos testes sem rede.
    pub baixador_sf2: Option<BaixadorSf2>,
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
            baixador_sf2: None,
        }
    }

    /// Os do sistema: `midir`/ALSA, saída `cpal` padrão, SF2 padrão (carregado em segundo
    /// plano) e a pasta XDG de dados do usuário.
    pub fn do_sistema() -> Self {
        let relogio: Arc<dyn Relogio> = Arc::new(RelogioMonotonico::novo());
        let mut som = ServicoSom::novo(Arc::clone(&relogio), abridor_cpal(), carregador_arquivo());
        let mut baixador = BaixadorSf2::novo(FLUIDR3_GM, abridor_http());
        let var = std::env::var(crate::som::VAR_SF2).ok();
        let pedido = var.as_deref().map(str::trim).filter(|v| !v.is_empty());
        match pedido {
            Some(v) => som.carregar_sf2(v),
            None => match sf2_encontrado(caminho_baixado().as_deref(), Path::is_file) {
                Some(p) => som.carregar_sf2(p),
                // Nenhum SF2 no sistema: baixa uma vez para a pasta de dados do usuário.
                None => baixador.iniciar(caminho_baixado()),
            },
        }
        Self {
            fonte_midi: Box::new(FonteMidir::nova()),
            som,
            relogio,
            pasta_sessoes: pasta_de_sessoes_do_ambiente(),
            baixador_sf2: Some(baixador),
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
    baixador_sf2: Option<BaixadorSf2>,
    pub timbres: EstadoTimbres,
    /// Metrônomo, bateria e exercício de pulso.
    pub ritmo: EstadoRitmo,
    /// Dicionário de acordes, escalas e desafio.
    pub acordes: EstadoAcordes,
    /// Exercícios progressivos (F05).
    pub exercicios: EstadoExercicios,
    /// Gravação e reprodução (F06).
    pub gravacao: EstadoGravacao,
    /// Partitura MusicXML (F07).
    pub partitura: EstadoPartitura,
    /// Revisão do conteúdo do tutor (M4): partituras e falas em Hipótese.
    pub revisao: EstadoRevisao,
    /// Cursos (M4): seletor, árvore, Exercício de Curso, áudio do professor e vídeo.
    pub cursos: EstadoCursos,
    /// O professor que fala as falas aprovadas do Roteiro (e as não revisadas, se a preferência
    /// "Usar conteúdo não revisado" estiver ligada).
    pub professor: Professor,
    /// Chamada e resposta em andamento (Curso ou exercício de fábrica).
    pub chamada: Option<ChamadaAtiva>,
    observador_fabrica: ObservadorFabrica,
    /// Assistente de calibração e o perfil em uso.
    pub calibracao: Calibracao,
    pub perfil: Option<PerfilTeclado>,
    mensagem_calibracao: Option<String>,
    pasta_dados: Option<PathBuf>,
    prefs_pendentes: Option<PreferenciasSom>,
    /// Perfis de preferências (SQLite, ADR-0010).
    banco: BancoPreferencias,
    /// Campo de nome do seletor de perfil e o último aviso dele.
    nome_perfil: String,
    aviso_perfil: Option<String>,
    /// A casca avisa o tema na abertura; só as trocas seguintes são gravadas no perfil.
    tema_sincronizado: bool,
    /// Último texto gravado de cada retrato de tela ([`crate::retratos`]).
    retratos_gravados: Vec<(&'static str, String)>,
    /// Configuração do teclado (divisão, camada, efeitos, timbre das demonstrações) como o app a
    /// aplicou ou gravou por último: quando uma tela a muda sozinha, o app grava `som`.
    config_som_vista: Option<ConfigTeclado>,
    rota_atual: String,
    relogio: Arc<dyn Relogio>,
    ultimo_quadro: Option<u64>,
    aviso_instalado: bool,
    /// Seletor de instrumento do Ribbon (busca e família).
    pub seletor: EstadoSeletor,
    /// Tela de abertura (só no executável).
    pub abertura: Abertura,
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
        let d = Dispositivos::do_sistema();
        // Versão da pasta de dados: backup e migrações antes de ler perfil e preferências (F14).
        if let Some(pasta) = d.pasta_sessoes.as_ref().and_then(|p| p.parent())
            && pasta.is_dir()
            && let Err(e) = crate::dados_usuario::preparar(pasta)
        {
            eprintln!("Piano Tutor: dados do usuário: {e}");
        }
        let mut app = Self::com_dispositivos(d);
        // mpv e voz do sistema (o Piper, se instalado; senão só o texto na tela).
        app.cursos.midia = Midia::do_sistema();
        if let Some(p) = app.pasta_dados.clone() {
            app.professor.trocar_voz(voz_do_sistema(&p));
        }
        app.abertura = Abertura::ligada();
        app
    }

    pub fn com_dispositivos(d: Dispositivos) -> Self {
        let pasta_dados = d
            .pasta_sessoes
            .as_ref()
            .and_then(|p| p.parent())
            .map(PathBuf::from);
        let perfil = pasta_dados
            .as_ref()
            .and_then(|p| PerfilTeclado::ler(&p.join(ARQUIVO_PERFIL)));
        let espelho = match perfil.as_ref().and_then(PerfilTeclado::teclado) {
            Some(t) => EstadoEspelho::calibrado(t),
            None => EstadoEspelho::novo(),
        };
        let banco = abrir_banco(pasta_dados.as_deref());
        let exibicao = PreferenciasExibicao::de_texto(&ler_secao(&banco, secoes::EXIBICAO));
        let mut estado = EstadoApp::novo();
        estado.definir_nomenclatura_dedos(exibicao.dedos);
        let mut espelho = espelho;
        espelho.nomenclatura = exibicao.dedos;
        espelho.mapa_dedos = exibicao.mapa_dedos;
        espelho.modo_mapa = exibicao.modo_mapa;
        let prefs_pendentes = Some(PreferenciasSom::de_texto(&ler_secao(&banco, secoes::SOM)));
        let mut partitura = EstadoPartitura::novo(pasta_dados.clone());
        partitura.definir_preferencias(PreferenciasPartitura::de_texto(&ler_secao(
            &banco,
            secoes::PARTITURA,
        )));
        let mut monitor =
            EstadoMonitor::novo(d.fonte_midi, Arc::clone(&d.relogio), d.pasta_sessoes);
        monitor.definir_porta_automatica(perfil.as_ref().map(|p| p.dispositivo.clone()));
        let professor = Professor::novo(Box::new(VozMuda::nova()), pasta_dados.as_deref());
        let mut cursos = EstadoCursos::novo(
            pasta_dados.clone(),
            Midia::com(Box::new(ExecutorReal), None),
        );
        cursos.usar_nao_revisado = professor.usar_nao_revisado();
        let mut app = Self {
            estado,
            espelho,
            monitor,
            som: d.som,
            baixador_sf2: d.baixador_sf2,
            timbres: EstadoTimbres::default(),
            ritmo: EstadoRitmo::novo(pasta_dados.clone()),
            acordes: EstadoAcordes::default(),
            exercicios: EstadoExercicios::novo(pasta_dados.clone()),
            gravacao: EstadoGravacao::novo(pasta_dados.clone()),
            partitura,
            revisao: EstadoRevisao::novo(pasta_dados.clone()),
            cursos,
            professor,
            chamada: None,
            observador_fabrica: ObservadorFabrica::default(),
            calibracao: Calibracao::default(),
            perfil,
            mensagem_calibracao: None,
            pasta_dados,
            prefs_pendentes,
            banco,
            nome_perfil: String::new(),
            aviso_perfil: None,
            tema_sincronizado: false,
            retratos_gravados: Vec::new(),
            config_som_vista: None,
            rota_atual: String::new(),
            relogio: d.relogio,
            ultimo_quadro: None,
            aviso_instalado: false,
            seletor: EstadoSeletor::default(),
            abertura: Abertura::desligada(),
        };
        app.carregar_retratos();
        app
    }

    /// Texto atual de um retrato de tela.
    fn retrato(&mut self, secao: &str) -> String {
        use crate::retratos::{self as r, secoes as s};
        match secao {
            s::TECLADO => r::retrato_teclado(&self.espelho),
            s::ACORDES => r::retrato_acordes(&self.acordes),
            s::RITMOS => r::retrato_ritmos(&mut self.ritmo),
            s::RITMOS_PADRAO => r::retrato_padrao(&self.ritmo),
            s::GRAVACAO => r::retrato_gravacao(&self.gravacao),
            s::PARTITURA => r::retrato_partitura(&self.partitura),
            s::PLAYER_AULA => r::retrato_player_aula(&mut self.cursos),
            s::CURSOS => r::retrato_cursos(&self.cursos),
            s::REVISAO => r::retrato_revisao(&self.revisao),
            s::TIMBRES => r::retrato_timbres(&self.seletor),
            // Telas que não se lembram (Calibração, Sobre) e o quadro antes da primeira tela
            // deixam valer a última rota gravada.
            s::APP => match rota_lembrada(&self.rota_atual) {
                Some(rota) => r::retrato_app(rota),
                None => self
                    .retratos_gravados
                    .iter()
                    .find(|(sec, _)| *sec == s::APP)
                    .map(|(_, t)| t.clone())
                    .unwrap_or_default(),
            },
            _ => String::new(),
        }
    }

    /// Aplica os retratos do perfil ativo (ao abrir e ao trocar de perfil), sem gravar nada.
    fn carregar_retratos(&mut self) {
        use crate::retratos::{self as r, secoes as s};
        self.retratos_gravados.clear();
        for secao in s::TODAS {
            let t = ler_secao(&self.banco, secao);
            match secao {
                s::TECLADO => r::aplicar_teclado(&mut self.espelho, &t),
                s::ACORDES => r::aplicar_acordes(&mut self.acordes, &t),
                s::RITMOS => r::aplicar_ritmos(&mut self.ritmo, &t),
                s::RITMOS_PADRAO => r::aplicar_padrao(&mut self.ritmo, &t),
                s::GRAVACAO => r::aplicar_gravacao(&mut self.gravacao, &t),
                s::PARTITURA => r::aplicar_partitura(&mut self.partitura, &t),
                s::PLAYER_AULA => r::aplicar_player_aula(&mut self.cursos, &t),
                s::CURSOS => r::aplicar_cursos(&mut self.cursos, &t),
                s::REVISAO => r::aplicar_revisao(&mut self.revisao, &t),
                s::TIMBRES => r::aplicar_timbres(&mut self.seletor, &t),
                // A rota só vale na abertura (`rota_inicial`); trocar de perfil não muda de tela.
                s::APP => {
                    self.retratos_gravados.push((secao, t));
                    continue;
                }
                _ => {}
            }
            let atual = self.retrato(secao);
            self.retratos_gravados.push((secao, atual));
        }
    }

    /// Grava na hora cada opção de tela que mudou (chamado a cada quadro, fora de arrasto).
    pub fn gravar_retratos(&mut self) {
        self.gravar_som_se_mudou();
        for i in 0..self.retratos_gravados.len() {
            let secao = self.retratos_gravados[i].0;
            let atual = self.retrato(secao);
            if atual != self.retratos_gravados[i].1 {
                // Só navegar não cria "Meu perfil": o de fábrica não guarda a última tela.
                let so_navegou_na_fabrica = secao == crate::retratos::secoes::APP
                    && self.banco.ativo().is_ok_and(|p| p.de_fabrica);
                if !so_navegou_na_fabrica {
                    self.gravar_secao(secao, &atual);
                }
                self.retratos_gravados[i].1 = atual;
            }
        }
    }

    /// Grava `som` quando uma tela mudou a configuração do teclado sem passar pelo pedido de
    /// timbres (o timbre das demonstrações, na tela Gravação). Espera as preferências do perfil
    /// serem aplicadas, para não gravar o padrão por cima delas.
    fn gravar_som_se_mudou(&mut self) {
        if self.prefs_pendentes.is_some() {
            return;
        }
        let atual = self.som.config_teclado();
        match self.config_som_vista {
            Some(vista) if vista != atual => self.salvar_preferencias(),
            Some(_) => {}
            None => self.config_som_vista = Some(atual),
        }
    }

    /// Aplica as preferências salvas assim que o som fica pronto (o SF2 carrega em segundo
    /// plano). Preset que não existir no SF2 é ignorado.
    fn aplicar_preferencias(&mut self) {
        if self.som.estado() != EstadoSom::Pronto {
            return;
        }
        let Some(p) = self.prefs_pendentes.take() else {
            return;
        };
        let _ = self
            .som
            .selecionar_timbre(p.timbre.banco, p.timbre.programa);
        self.som.definir_volume(p.volume);
        let _ = self.som.definir_config_teclado(p.teclado);
        self.config_som_vista = Some(self.som.config_teclado());
    }

    /// Troca a nomenclatura dos dedos no teclado e grava nas preferências de exibição.
    pub fn trocar_dedos(&mut self, n: NomenclaturaDedos) {
        self.estado.definir_nomenclatura_dedos(n);
        self.espelho.nomenclatura = n;
        self.gravar_exibicao();
    }

    /// Liga, desliga ou troca a tônica do mapa de dedos em todas as teclas e grava.
    pub fn trocar_mapa_dedos(&mut self, mapa: Option<u8>) {
        self.espelho.mapa_dedos = mapa.map(|t| t % 12);
        self.gravar_exibicao();
    }

    fn gravar_exibicao(&mut self) {
        let p = PreferenciasExibicao {
            dedos: self.espelho.nomenclatura,
            mapa_dedos: self.espelho.mapa_dedos,
            modo_mapa: self.espelho.modo_mapa,
        };
        self.gravar_secao(secoes::EXIBICAO, &p.para_texto());
    }

    /// Grava uma seção no perfil ativo; sobre o de fábrica, avisa que criou "Meu perfil".
    fn gravar_secao(&mut self, secao: &str, texto: &str) {
        match self.banco.gravar(secao, texto) {
            Ok(Gravacao::CriouPessoal) => {
                // O perfil novo ainda não sabe em que tela o aluno está: regrava a rota.
                if let Some((_, t)) = self
                    .retratos_gravados
                    .iter_mut()
                    .find(|(sec, _)| *sec == crate::retratos::secoes::APP)
                {
                    t.clear();
                }
                self.aviso_perfil = Some(format!(
                    "O perfil de fábrica não muda: suas preferências agora ficam em \"{}\".",
                    crate::perfis::NOME_PESSOAL
                ));
            }
            Ok(_) => {}
            Err(e) => self.aviso_perfil = Some(e),
        }
    }

    /// Grava as preferências da Partitura quando a tela as muda.
    fn gravar_preferencias_da_partitura(&mut self) {
        if let Some(p) = self.partitura.tomar_preferencias_mudadas() {
            self.gravar_secao(secoes::PARTITURA, &p.para_texto());
        }
    }

    /// Seletor de perfil: escolher, criar (cópia do atual), renomear e apagar. Devolve o tema a
    /// pedir à casca quando a troca de perfil muda o tema.
    fn desenhar_seletor_perfil(&mut self, ui: &mut egui::Ui, compacto: bool) -> Vec<Pedido> {
        let (perfis, ativo) = self.perfis();
        let ativo_id = ativo.as_ref().map(|p| p.id);
        let fabrica = ativo.as_ref().is_some_and(|p| p.de_fabrica);
        let nome_ativo = ativo.map(|p| p.nome).unwrap_or_default();
        let mut escolhido = None;
        let mut criar = false;
        let mut renomear = false;
        let mut apagar = false;
        ui.vertical(|ui| {
            egui::ComboBox::from_id_salt("piano.seletor-perfil")
                .width(if compacto { 110.0 } else { 140.0 })
                .selected_text(&nome_ativo)
                .show_ui(ui, |ui| {
                    for p in &perfis {
                        let r = ui.selectable_label(Some(p.id) == ativo_id, &p.nome);
                        let r = if p.de_fabrica {
                            r.on_hover_text(
                                "Os padrões do app. Não muda: mexer em algo com ele \
                                             ativo cria \"Meu perfil\".",
                            )
                        } else {
                            r
                        };
                        if r.clicked() && Some(p.id) != ativo_id {
                            escolhido = Some(p.id);
                        }
                    }
                    ui.separator();
                    ui.add(
                        egui::TextEdit::singleline(&mut self.nome_perfil)
                            .hint_text("nome do perfil")
                            .desired_width(150.0),
                    );
                    ui.horizontal(|ui| {
                        criar = ui
                            .button("Novo")
                            .on_hover_text(
                                "Cria um perfil com as preferências atuais e passa a usá-lo",
                            )
                            .clicked();
                        renomear = ui
                            .add_enabled(!fabrica, egui::Button::new("Renomear"))
                            .clicked();
                    });
                    apagar = ui
                        .add_enabled(!fabrica, egui::Button::new("Apagar este perfil"))
                        .on_hover_text("Apaga o perfil em uso e volta ao de fábrica")
                        .clicked();
                })
                .response
                .on_hover_text(
                    "Perfil de preferências (som, dedilhado, tema e as opções de cada tela, mais a última \
                     tela aberta), guardado no computador. \"De fábrica\" devolve tudo ao padrão.",
                );
            if !compacto && let Some(a) = &self.aviso_perfil {
                ui.label(egui::RichText::new(a).small().weak());
            }
        });
        let nome = self.nome_perfil.clone();
        if criar {
            self.criar_perfil(&nome);
        }
        if renomear {
            self.renomear_perfil(&nome);
        }
        if apagar {
            return self.apagar_perfil();
        }
        escolhido.map_or_else(Vec::new, |id| self.trocar_perfil(id))
    }

    /// Perfis para o seletor do Ribbon e o ativo.
    pub fn perfis(&self) -> (Vec<Perfil>, Option<Perfil>) {
        (
            self.banco.perfis().unwrap_or_default(),
            self.banco.ativo().ok(),
        )
    }

    /// Ativa um perfil e aplica tudo o que ele guarda (som, exibição, partitura); devolve o
    /// tema do perfil, se houver, para a casca trocar.
    pub fn trocar_perfil(&mut self, id: i64) -> Vec<Pedido> {
        if let Err(e) = self.banco.ativar(id) {
            self.aviso_perfil = Some(e);
            return Vec::new();
        }
        self.aviso_perfil = None;
        let exibicao = PreferenciasExibicao::de_texto(&ler_secao(&self.banco, secoes::EXIBICAO));
        self.estado.definir_nomenclatura_dedos(exibicao.dedos);
        self.espelho.nomenclatura = exibicao.dedos;
        self.espelho.mapa_dedos = exibicao.mapa_dedos;
        self.espelho.modo_mapa = exibicao.modo_mapa;
        self.prefs_pendentes = Some(PreferenciasSom::de_texto(&ler_secao(
            &self.banco,
            secoes::SOM,
        )));
        self.aplicar_preferencias();
        self.partitura
            .definir_preferencias(PreferenciasPartitura::de_texto(&ler_secao(
                &self.banco,
                secoes::PARTITURA,
            )));
        self.carregar_retratos();
        // Janela: tema, densidade e escala do perfil (padrões no de fábrica ou sem registro).
        let janela = preferencias_da_janela(&self.banco);
        let tema = janela
            .as_ref()
            .map(|p| p.tema.clone())
            .or_else(|| self.banco.ler(secoes::TEMA).ok().flatten())
            .unwrap_or_else(|| TEMAS[0].id.to_owned());
        vec![
            Pedido::Tema(tema),
            Pedido::Densidade(janela.as_ref().map(|p| p.densidade).unwrap_or_default()),
            Pedido::Escala(janela.as_ref().map_or(1.0, |p| p.escala)),
        ]
    }

    /// Cria um perfil com as preferências do ativo e passa a usá-lo.
    pub fn criar_perfil(&mut self, nome: &str) {
        let resultado = self
            .banco
            .ativo()
            .and_then(|a| self.banco.criar(nome, a.id))
            .and_then(|p| self.banco.ativar(p.id));
        match resultado {
            Ok(p) => {
                self.aviso_perfil = Some(format!("Perfil \"{}\" criado e em uso.", p.nome));
                self.nome_perfil.clear();
            }
            Err(e) => self.aviso_perfil = Some(e),
        }
    }

    pub fn renomear_perfil(&mut self, nome: &str) {
        let r = self
            .banco
            .ativo()
            .and_then(|a| self.banco.renomear(a.id, nome));
        match r {
            Ok(()) => {
                self.aviso_perfil = None;
                self.nome_perfil.clear();
            }
            Err(e) => self.aviso_perfil = Some(e),
        }
    }

    /// Apaga o perfil ativo (nunca o de fábrica) e volta ao de fábrica; devolve o tema dele.
    pub fn apagar_perfil(&mut self) -> Vec<Pedido> {
        let r = self.banco.ativo().and_then(|a| self.banco.apagar(a.id));
        match r {
            Ok(()) => match self.banco.ativo() {
                Ok(f) => self.trocar_perfil(f.id),
                Err(_) => Vec::new(),
            },
            Err(e) => {
                self.aviso_perfil = Some(e);
                Vec::new()
            }
        }
    }

    fn salvar_preferencias(&mut self) {
        let p = PreferenciasSom {
            timbre: self.som.timbre(),
            volume: self.som.volume(),
            teclado: self.som.config_teclado(),
        };
        self.config_som_vista = Some(p.teclado);
        self.gravar_secao(secoes::SOM, &p.para_texto());
    }

    /// Executa os pedidos da tela Calibração.
    pub fn executar_calibracao(&mut self, pedidos: Vec<AcaoCalibracao>) {
        for p in pedidos {
            match p {
                AcaoCalibracao::SalvarEUsar => self.salvar_calibracao(),
                AcaoCalibracao::Esquecer => {
                    if let Some(pasta) = &self.pasta_dados {
                        let _ = std::fs::remove_file(pasta.join(ARQUIVO_PERFIL));
                    }
                    self.perfil = None;
                    self.trocar_faixa(None);
                    self.mensagem_calibracao =
                        Some("Perfil esquecido: o Espelho voltou à faixa padrão.".to_owned());
                }
            }
        }
    }

    fn trocar_faixa(&mut self, perfil: Option<&PerfilTeclado>) {
        let (zoom, nomes, dedos, mapa, opcoes) = (
            self.espelho.zoom(),
            self.espelho.nomes,
            self.espelho.nomenclatura,
            (self.espelho.mapa_dedos, self.espelho.modo_mapa),
            self.espelho.opcoes_tela,
        );
        self.panico();
        self.espelho = match perfil.and_then(PerfilTeclado::teclado) {
            Some(t) => EstadoEspelho::calibrado(t),
            None => EstadoEspelho::novo(),
        };
        self.espelho.definir_zoom(zoom);
        self.espelho.nomes = nomes;
        self.espelho.nomenclatura = dedos;
        (self.espelho.mapa_dedos, self.espelho.modo_mapa) = mapa;
        self.espelho.opcoes_tela = opcoes;
    }

    fn salvar_calibracao(&mut self) {
        let Some(perfil) = self.calibracao.perfil() else {
            return;
        };
        self.monitor
            .definir_porta_automatica(Some(perfil.dispositivo.clone()));
        let mut msg = String::from("Perfil em uso no Espelho.");
        if let Some(pasta) = self.pasta_dados.clone() {
            let r = std::fs::create_dir_all(&pasta)
                .and_then(|()| std::fs::write(pasta.join(ARQUIVO_PERFIL), perfil.para_texto()));
            let rel = self
                .calibracao
                .relatorio(&data_de_hoje())
                .map(|t| std::fs::write(pasta.join(ARQUIVO_RELATORIO), t));
            msg = match (r, rel) {
                (Ok(()), Some(Ok(()))) => format!(
                    "Perfil salvo e em uso. Relatório: {}",
                    pasta.join(ARQUIVO_RELATORIO).display()
                ),
                (Err(e), _) | (_, Some(Err(e))) => {
                    format!("Perfil em uso, mas não foi possível gravar: {e}")
                }
                _ => msg,
            };
        }
        self.trocar_faixa(Some(&perfil));
        self.perfil = Some(perfil);
        self.mensagem_calibracao = Some(msg);
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
            if let Some(b) = self.baixador_sf2.as_mut() {
                b.definir_aviso(Arc::clone(&aviso));
            }
            self.som.definir_aviso(aviso);
            self.aviso_instalado = true;
        }
        self.tick();
        self.atualizar_roda_bend(ctx);
        self.gravar_preferencias_da_partitura();
        // Grava as opções de tela que mudaram; durante um arrasto (zoom, volume), só ao soltar.
        if !ctx.input(|i| i.pointer.any_down()) {
            self.gravar_retratos();
        }
        let mut proximo = self.monitor.proxima_varredura_em();
        if self.som.carregando().is_some()
            || self
                .baixador_sf2
                .as_ref()
                .is_some_and(BaixadorSf2::baixando)
        {
            proximo = proximo.min(Duration::from_millis(250));
        }
        if !self.espelho.roda_bend.centrada() || self.som.pitch_bend_roda() != 0 {
            // Mola da roda de bend: ~60 quadros por segundo até voltar ao centro.
            proximo = proximo.min(Duration::from_millis(16));
        }
        if self.ritmo.tocando()
            || self.gravacao.tocando()
            || self.gravacao.gravando()
            || self.partitura.tocando()
            || self.chamada.as_ref().is_some_and(|c| !c.concluida())
            || self.cursos.precisa_de_quadros()
        {
            // Agenda o ritmo e anima o pulso: ~60 quadros por segundo enquanto toca.
            proximo = proximo.min(Duration::from_millis(16));
        }
        ctx.request_repaint_after(proximo.max(Duration::from_millis(1)));
    }

    /// Roda de pitch bend virtual: sem foco ela volta ao centro na hora; senão avança a mola e
    /// entrega o valor ao som (todos os canais do teclado).
    fn atualizar_roda_bend(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.focused) {
            self.espelho.roda_bend.zerar();
        }
        let agora = ctx.input(|i| i.time);
        let valor = self.espelho.roda_bend.atualizar(agora);
        self.som.definir_pitch_bend(valor);
    }

    /// Um passo do pipeline, sem janela: MIDI → Espelho + som, simulação → som, som.
    pub fn tick(&mut self) {
        let saidas = self.monitor.atualizar();
        self.rotear(saidas);
        self.encaminhar_simulacao();
        self.atualizar_download();
        self.som.atualizar();
        self.aplicar_preferencias();
        self.espelho.divisao = self.som.config_teclado().divisao;
        self.ritmo.atualizar(&mut self.som);
        self.gravacao.atualizar(&mut self.som);
        self.partitura.atualizar(&mut self.som);
        self.conferir_desafio();
        self.atualizar_cursos();
    }

    /// Download do SF2 padrão: carrega quando termina e publica o progresso na tela Timbres.
    fn atualizar_download(&mut self) {
        let Some(b) = self.baixador_sf2.as_mut() else {
            return;
        };
        if let Some(p) = b.atualizar() {
            self.timbres.campo_caminho = p.display().to_string();
            self.som.carregar_sf2(p);
        }
        self.timbres.download_texto = b.texto();
        self.timbres.download_fracao = b.fracao();
    }

    /// "Usar o padrão": o SF2 do sistema ou o já baixado; se não houver nenhum, baixa.
    fn usar_sf2_padrao(&mut self) {
        let baixado = caminho_baixado();
        match sf2_encontrado(baixado.as_deref(), Path::is_file) {
            Some(p) => {
                self.timbres.campo_caminho = p.display().to_string();
                self.som.carregar_sf2(p);
            }
            None => match self.baixador_sf2.as_mut() {
                Some(b) => b.iniciar(baixado),
                None => self.som.carregar_sf2(crate::som::SF2_PADRAO),
            },
        }
    }

    /// Desafio de acordes: confere as teclas seguradas (físicas ou simuladas).
    fn conferir_desafio(&mut self) {
        if self.rota_atual != ROTA_EXERCICIOS && self.exercicios.treino.ativo() {
            self.exercicios.parar_treino(&mut self.ritmo, &mut self.som);
        }
        if self.rota_atual != ROTA_ACORDES {
            if self.acordes.treino.ativo() {
                self.acordes.treino.parar(&mut self.ritmo, &mut self.som);
            }
            return;
        }
        let seguradas: Vec<_> = self.espelho.teclas().notas_ligadas().collect();
        let agora = self.som.agora_ns();
        self.acordes.conferir(&seguradas, agora);
        self.acordes
            .treino
            .atualizar(&mut self.ritmo, &mut self.som, &seguradas);
    }

    /// Aplica as saídas do gerenciador MIDI, na ordem.
    pub fn rotear(&mut self, saidas: Vec<SaidaMidi>) {
        for s in saidas {
            match s {
                SaidaMidi::Evento(mut ev) => {
                    if self.rota_atual == ROTA_CALIBRACAO {
                        // A calibração do bend precisa do valor cru da roda.
                        self.calibracao.evento(&ev);
                    }
                    if let EventoMidi::PitchBend { valor, .. } = &mut ev {
                        if let Some(p) = &self.perfil {
                            *valor = p.bend_corrigido(*valor);
                        }
                        self.espelho.bend_fisico = *valor;
                    }
                    self.gravacao.evento(&ev);
                    if let EventoMidi::NotaLigada {
                        timestamp_monotonico_ns,
                        ..
                    } = ev
                    {
                        self.ritmo.toque(timestamp_monotonico_ns);
                    }
                    if let EventoMidi::NotaLigada {
                        nota,
                        timestamp_monotonico_ns,
                        ..
                    } = ev
                    {
                        self.nota_para_cursos(true, nota, timestamp_monotonico_ns);
                    }
                    if self.rota_atual == ROTA_EXERCICIOS && self.chamada.is_none() {
                        match ev {
                            EventoMidi::NotaLigada {
                                nota,
                                timestamp_monotonico_ns,
                                ..
                            } => self.exercicios.nota(true, nota, timestamp_monotonico_ns),
                            EventoMidi::NotaDesligada {
                                nota,
                                timestamp_monotonico_ns,
                                ..
                            } => self.exercicios.nota(false, nota, timestamp_monotonico_ns),
                            _ => {}
                        }
                    }
                    if self.rota_atual == ROTA_PARTITURA {
                        match ev {
                            EventoMidi::NotaLigada {
                                nota,
                                timestamp_monotonico_ns,
                                ..
                            } => self
                                .partitura
                                .nota_tocada(true, nota, timestamp_monotonico_ns),
                            EventoMidi::NotaDesligada {
                                nota,
                                timestamp_monotonico_ns,
                                ..
                            } => self
                                .partitura
                                .nota_tocada(false, nota, timestamp_monotonico_ns),
                            _ => {}
                        }
                    }
                    if let Some(t) = evento_tecla(&ev) {
                        // Nota fora da faixa fica registrada no Espelho; as outras não mudam.
                        let _ = self.espelho.aplicar_evento(t);
                    }
                    self.som.evento_midi(&ev);
                }
                SaidaMidi::LimparNotas => {
                    self.espelho.roda_bend.zerar();
                    self.espelho.bend_fisico = 0;
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
            let agora = self.som.agora_ns();
            if ev.acao == AcaoTecla::Ligar {
                self.ritmo.toque(agora);
            }
            self.gravacao
                .nota_simulada(ev.acao == AcaoTecla::Ligar, ev.nota, agora);
            if ev.acao == AcaoTecla::Ligar {
                self.nota_para_cursos(true, ev.nota, agora);
            }
            if self.rota_atual == ROTA_EXERCICIOS && self.chamada.is_none() {
                self.exercicios
                    .nota(ev.acao == AcaoTecla::Ligar, ev.nota, agora);
            }
            if self.rota_atual == ROTA_PARTITURA {
                self.partitura
                    .nota_tocada(ev.acao == AcaoTecla::Ligar, ev.nota, agora);
            }
            self.som.nota_simulada(ev);
        }
    }

    /// Pânico: all-notes-off imediato no motor e Espelho limpo (simulação inclusive).
    pub fn panico(&mut self) {
        self.ritmo.parar();
        // Também centra a roda de bend (liberar_simulacao).
        self.espelho.liberar_simulacao();
        // As liberações da simulação perdem o sentido: o pânico corta tudo.
        let _ = self.espelho.drenar_simulados();
        self.espelho.limpar_tudo();
        self.som.panico();
    }

    pub fn executar_timbres(&mut self, pedidos: Vec<AcaoTimbres>) {
        let salvar = pedidos
            .iter()
            .any(|p| !matches!(p, AcaoTimbres::Panico | AcaoTimbres::ReabrirSaida));
        self.executar_timbres_sem_salvar(pedidos);
        if salvar {
            self.salvar_preferencias();
        }
    }

    fn executar_timbres_sem_salvar(&mut self, pedidos: Vec<AcaoTimbres>) {
        for p in pedidos {
            match p {
                AcaoTimbres::CarregarSf2(caminho) if caminho.is_empty() => {
                    self.timbres.erro_timbre = Some("Informe o caminho de um arquivo .sf2.".into());
                }
                AcaoTimbres::CarregarSf2(caminho) => {
                    self.timbres.erro_timbre = None;
                    self.som.carregar_sf2(caminho);
                }
                AcaoTimbres::UsarPadrao => {
                    self.timbres.erro_timbre = None;
                    self.usar_sf2_padrao();
                }
                AcaoTimbres::Timbre { banco, programa } => {
                    self.timbres.erro_timbre = self.som.selecionar_timbre(banco, programa).err();
                }
                AcaoTimbres::Volume(v) => self.som.definir_volume(v),
                AcaoTimbres::Panico => self.panico(),
                AcaoTimbres::ReabrirSaida => self.som.reabrir_saida(),
                AcaoTimbres::ConfigTeclado(c) => {
                    self.timbres.erro_timbre = self.som.definir_config_teclado(c).err();
                }
            }
        }
    }
}

impl AppPianoTutor {
    /// Resumo para a página inicial.
    pub fn resumo(&self) -> Resumo {
        Resumo {
            midi: self.monitor.texto_status().to_owned(),
            midi_ok: matches!(self.monitor.estado(), EstadoConexao::Conectado { .. }),
            som: self.som.texto_status().to_owned(),
            som_ok: self.som.estado() == EstadoSom::Pronto,
            instrumento: self
                .som
                .nome_do_timbre()
                .map_or_else(|| "Sem instrumento".to_owned(), str::to_owned),
        }
    }

    /// O que a abertura mostra e se algo ainda carrega.
    fn texto_da_abertura(&self) -> (String, bool) {
        if let Some(t) = self
            .baixador_sf2
            .as_ref()
            .filter(|b| b.baixando())
            .and_then(BaixadorSf2::texto)
        {
            return (t, true);
        }
        match self.som.estado() {
            EstadoSom::Carregando => ("Carregando o SoundFont…".to_owned(), true),
            EstadoSom::Pronto => ("Pronto para tocar".to_owned(), false),
            _ => ("Preparando o som e o MIDI…".to_owned(), false),
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

/// Arquivos na pasta de dados (`~/.local/share/piano-tutor/`).
pub const ARQUIVO_PERFIL: &str = "teclado.txt";
pub const ARQUIVO_RELATORIO: &str = "rev-as-calibracao.md";
pub const ARQUIVO_PREFERENCIAS: &str = "preferencias.txt";

/// Data de hoje (UTC) em AAAA-MM-DD, sem dependências.
pub fn data_de_hoje() -> String {
    let dias = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400);
    data_civil(i64::try_from(dias).unwrap_or(0))
}

/// Dias desde 1970-01-01 para AAAA-MM-DD (algoritmo de Howard Hinnant).
pub fn data_civil(dias: i64) -> String {
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Seletor do Ribbon: ícone de mão e lista com as três nomenclaturas; abaixo (fora do modo
/// compacto) a legenda polegar/mínimo da escolhida. Devolve a nova escolha.
fn seletor_dedos(
    ui: &mut egui::Ui,
    atual: NomenclaturaDedos,
    compacto: bool,
) -> Option<NomenclaturaDedos> {
    let mut escolhida = atual;
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::icones::glifo("dedos")).size(18.0))
                .on_hover_text("Como o dedo aparece nas bolinhas do teclado");
            egui::ComboBox::from_id_salt("piano.seletor-dedos")
                .width(if compacto { 96.0 } else { 112.0 })
                .selected_text(egui::RichText::new(atual.curto()).monospace())
                .show_ui(ui, |ui| {
                    for n in NOMENCLATURAS {
                        ui.selectable_value(&mut escolhida, n, n.curto())
                            .on_hover_text(n.dica());
                    }
                })
                .response
                .on_hover_text(atual.dica());
        });
        if !compacto {
            let (p, m) = (atual.rotulo(1).unwrap_or(""), atual.rotulo(5).unwrap_or(""));
            ui.label(
                egui::RichText::new(format!("polegar {p} · mínimo {m}"))
                    .small()
                    .weak(),
            );
        }
    });
    (escolhida != atual).then_some(escolhida)
}

/// Abre o banco de perfis na pasta de dados (importando os `.txt` antigos na primeira vez);
/// sem pasta ou com erro, um banco na memória (o app funciona, só não guarda).
fn abrir_banco(pasta: Option<&Path>) -> BancoPreferencias {
    let aberto = pasta.map(|p| {
        BancoPreferencias::abrir(&p.join(ARQUIVO_BANCO)).and_then(|b| {
            b.importar_legado(p)?;
            Ok(b)
        })
    });
    match aberto {
        Some(Ok(b)) => b,
        other => {
            if let Some(Err(e)) = other {
                eprintln!("Piano Tutor: {e}; preferências só nesta sessão.");
            }
            BancoPreferencias::em_memoria().expect("SQLite em memória")
        }
    }
}

/// Preferências da janela guardadas no perfil ativo.
fn preferencias_da_janela(banco: &BancoPreferencias) -> Option<Preferencias> {
    serde_json::from_str(&banco.ler(secoes::CASCA).ok().flatten()?).ok()
}

/// Preferências da janela do perfil ativo, para o executável aplicar antes de abrir a janela.
pub fn preferencias_da_janela_salvas() -> Option<Preferencias> {
    let pasta = pasta_de_sessoes_do_ambiente()?.parent()?.to_path_buf();
    let banco = BancoPreferencias::abrir(&pasta.join(ARQUIVO_BANCO)).ok()?;
    preferencias_da_janela(&banco)
}

/// Texto de uma seção no perfil ativo (vazio = padrão).
fn ler_secao(banco: &BancoPreferencias, secao: &str) -> String {
    banco.ler(secao).ok().flatten().unwrap_or_default()
}

/// "Dedos em todas as teclas" (mapa para decorar) e a tônica da escala maior usada. Devolve a
/// escolha nova quando muda.
fn seletor_mapa_dedos(ui: &mut egui::Ui, atual: Option<u8>) -> Option<Option<u8>> {
    let mut ligado = atual.is_some();
    let mut tonica = atual.unwrap_or(0);
    ui.horizontal(|ui| {
        ui.checkbox(&mut ligado, "Em todas as teclas")
            .on_hover_text(
                "Mostra o dedo em todas as teclas da escala maior escolhida, para decorar o \
             dedilhado (mão esquerda abaixo do ponto de divisão ou do Dó central). Quando a \
             tela mostra um acorde, escala ou Lição, vale o dedilhado dela.",
            );
        if ligado {
            egui::ComboBox::from_id_salt("piano.mapa-dedos")
                .width(64.0)
                .selected_text(crate::dedos::TONICAS_MAPA[usize::from(tonica % 12)])
                .show_ui(ui, |ui| {
                    for (i, nome) in crate::dedos::TONICAS_MAPA.iter().enumerate() {
                        ui.selectable_value(&mut tonica, i as u8, format!("{nome} maior"));
                    }
                })
                .response
                .on_hover_text("Escala maior cujo dedilhado aparece nas teclas");
        }
    });
    let nova = ligado.then_some(tonica);
    (nova != atual).then_some(nova)
}

/// Configuração da casca: título, tokens, temas e tema inicial.
pub fn config() -> ConfigCasca {
    let mut config = ConfigCasca::nova(TITULO, TOKENS_JSON, TEMAS_JSON);
    config.tema_inicial = TEMAS[0].id.to_owned();
    config.icones = crate::icones::familia();
    config.icone_janela = Some(crate::icones::icone_janela());
    config.id_app = Some("piano-tutor".to_owned());
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
        vec![
            GrupoNavegacao::novo(
                "Piano Tutor",
                vec![
                    ItemNavegacao::novo(ROTA_INICIO, "Início", "inicio"),
                    ItemNavegacao::novo(ROTA_MONITOR, "Monitor MIDI", "midi"),
                    ItemNavegacao::novo(ROTA_CALIBRACAO, "Calibrar teclado", "calibrar"),
                    ItemNavegacao::novo(ROTA_TECLADO, "Teclado", "teclado"),
                    ItemNavegacao::novo(ROTA_EXERCICIOS, "Exercícios", "exercicios"),
                    ItemNavegacao::novo(ROTA_ACORDES, "Acordes e escalas", "acordes"),
                    ItemNavegacao::novo(ROTA_RITMOS, "Ritmos", "ritmos"),
                    ItemNavegacao::novo(ROTA_GRAVACAO, "Gravar e ouvir", "gravar"),
                    ItemNavegacao::novo(ROTA_PARTITURA, "Partitura", "partitura"),
                    ItemNavegacao::novo(ROTA_TIMBRES, "Sons e timbres", "timbres"),
                    ItemNavegacao::novo(ROTA_SOBRE, "Sobre", "sobre"),
                ],
            ),
            GrupoNavegacao::novo(
                "Cursos",
                vec![ItemNavegacao::novo(ROTA_CURSOS, "Cursos", "exercicios")],
            ),
            GrupoNavegacao::novo(
                "Tutor",
                vec![ItemNavegacao::novo(
                    ROTA_REVISAO,
                    "Revisão do tutor",
                    "partitura",
                )],
            ),
        ]
    }

    /// A rota de `PIANO_TUTOR_ROTA` (por exemplo `exercicios`, para capturas e demonstração); sem
    /// ela, a última tela aberta no perfil; sem nenhuma das duas, o Início. Valor desconhecido
    /// vale como ausente.
    fn rota_inicial(&self) -> Rota {
        let pedida = std::env::var("PIANO_TUTOR_ROTA").unwrap_or_default();
        let gravada =
            crate::retratos::rota_gravada(&ler_secao(&self.banco, crate::retratos::secoes::APP))
                .unwrap_or_default();
        // A Calibração só abre por pedido explícito (demonstração), nunca pela lembrança.
        let pedida = if pedida == ROTA_CALIBRACAO {
            Some(ROTA_CALIBRACAO)
        } else {
            rota_lembrada(&pedida)
        };
        pedida
            .or_else(|| rota_lembrada(&gravada))
            .map_or_else(|| Rota::nova(ROTA_INICIO), Rota::nova)
    }

    fn trilha(&self, rota: &Rota) -> Vec<Elo> {
        let inicio = || Elo::link("Início", ROTA_INICIO);
        match rota.como_texto() {
            ROTA_SOBRE => vec![inicio(), Elo::texto("Sobre")],
            ROTA_TECLADO => vec![inicio(), Elo::texto("Teclado")],
            ROTA_MONITOR => vec![inicio(), Elo::texto("Monitor MIDI")],
            ROTA_TIMBRES => vec![inicio(), Elo::texto("Sons e timbres")],
            ROTA_CALIBRACAO => vec![inicio(), Elo::texto("Calibrar teclado")],
            ROTA_RITMOS => vec![inicio(), Elo::texto("Ritmos")],
            ROTA_ACORDES => vec![inicio(), Elo::texto("Acordes e escalas")],
            ROTA_EXERCICIOS => vec![inicio(), Elo::texto("Exercícios")],
            ROTA_GRAVACAO => vec![inicio(), Elo::texto("Gravar e ouvir")],
            ROTA_PARTITURA => vec![inicio(), Elo::texto("Partitura")],
            ROTA_CURSOS => vec![inicio(), Elo::texto("Cursos")],
            ROTA_REVISAO => vec![inicio(), Elo::texto("Revisão do tutor")],
            _ => vec![Elo::texto("Início")],
        }
    }

    fn desenhar_pagina(&mut self, ui: &mut egui::Ui, rota: &Rota, _local: Local) {
        self.processar_quadro(ui.ctx());
        let anterior = self.rota_atual.clone();
        rota.como_texto().clone_into(&mut self.rota_atual);
        self.ao_mudar_de_rota(&anterior);
        let com_teclado = [ROTA_TECLADO, ROTA_ACORDES, ROTA_EXERCICIOS, ROTA_CURSOS];
        if rota.como_texto() != ROTA_ACORDES && rota.como_texto() != ROTA_EXERCICIOS {
            self.espelho.limpar_guias();
        }
        if !com_teclado.contains(&rota.como_texto()) {
            // Fora da tela Espelho ninguém recebe o "solta": não deixa nota presa.
            self.espelho.liberar_simulacao();
        }
        if rota.como_texto() != ROTA_REVISAO {
            // Fora da revisão nada continua tocando nem falando.
            self.revisao.ao_sair(&mut self.som);
        }
        match rota.como_texto() {
            ROTA_REVISAO => tela_revisao::desenhar_tela(ui, &mut self.revisao, &mut self.som),
            ROTA_TECLADO => tela_espelho::desenhar_tela(ui, &mut self.espelho),
            ROTA_MONITOR => {
                tela_monitor::desenhar_tela(ui, &mut self.monitor);
            }
            ROTA_TIMBRES => {
                let pedidos = tela_timbres::desenhar_tela(ui, &self.som, &mut self.timbres);
                self.executar_timbres(pedidos);
            }
            ROTA_PARTITURA => {
                tela_partitura::desenhar_tela(ui, &mut self.partitura, &mut self.som);
            }
            ROTA_GRAVACAO => tela_gravacao::desenhar_tela(ui, &mut self.gravacao, &mut self.som),
            ROTA_CURSOS => {
                let acoes = tela_cursos::desenhar_tela(
                    ui,
                    &mut self.cursos,
                    &mut self.som,
                    &mut self.espelho,
                    &tela_cursos::ContextoCursos {
                        professor: &self.professor,
                        chamada: self.chamada.as_ref(),
                        fabrica: &self.exercicios,
                    },
                );
                self.executar_acoes_cursos(ui.ctx(), acoes);
            }
            ROTA_EXERCICIOS => {
                let acoes = tela_exercicios::desenhar_tela(
                    ui,
                    &mut self.exercicios,
                    &mut self.espelho,
                    &mut self.som,
                    &mut self.ritmo,
                    &self.professor,
                    self.chamada.as_ref(),
                );
                self.executar_acoes_cursos(ui.ctx(), acoes);
            }
            ROTA_RITMOS => tela_ritmos::desenhar_tela(ui, &mut self.ritmo, &mut self.som),
            ROTA_ACORDES => {
                tela_acordes::desenhar_tela(
                    ui,
                    &mut self.acordes,
                    &mut self.espelho,
                    &mut self.som,
                    &mut self.ritmo,
                );
            }
            ROTA_CALIBRACAO => {
                let conectado = match self.monitor.gerenciador().estado() {
                    EstadoConexao::Conectado { .. } => {
                        self.monitor.gerenciador().porta_selecionada()
                    }
                    _ => None,
                };
                if let Some(c) = conectado
                    && self.calibracao.dispositivo != c
                {
                    c.clone_into(&mut self.calibracao.dispositivo);
                }
                let ctx = ContextoCalibracao {
                    conectado,
                    perfil_em_uso: self.perfil.as_ref().map(|p| {
                        format!(
                            "MIDI {} a {}{}",
                            p.grave,
                            p.aguda(),
                            if p.dispositivo.is_empty() {
                                String::new()
                            } else {
                                format!(" ({})", p.dispositivo)
                            }
                        )
                    }),
                    mensagem: self.mensagem_calibracao.as_deref(),
                };
                let pedidos = tela_calibracao::desenhar_tela(ui, &mut self.calibracao, &ctx);
                self.executar_calibracao(pedidos);
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
                let resumo = self.resumo();
                tela_inicio::desenhar_tela(ui, &resumo);
            }
        }
        // Notas da simulação tocam já neste quadro (sem esperar o próximo).
        self.encaminhar_simulacao();
        if self.abertura.visivel() {
            let (status, carregando) = self.texto_da_abertura();
            abertura::desenhar(ui.ctx(), &mut self.abertura, &status, carregando);
        }
    }

    fn acesso_rapido(&self) -> Vec<String> {
        vec![ids::PANICO.to_owned(), ids::TECLADO_61.to_owned()]
    }

    fn desenhar_controle_ribbon(&mut self, ui: &mut egui::Ui, id: &str, compacto: bool) {
        if id == acoes::CONTROLE_PERFIL {
            for p in self.desenhar_seletor_perfil(ui, compacto) {
                pedir(ui.ctx(), p);
            }
            return;
        }
        if id == acoes::CONTROLE_DEDOS {
            let atual = self.estado.nomenclatura_dedos();
            if let Some(n) = seletor_dedos(ui, atual, compacto) {
                self.trocar_dedos(n);
            }
            if let Some(m) = seletor_mapa_dedos(ui, self.espelho.mapa_dedos) {
                self.trocar_mapa_dedos(m);
            }
            if self.espelho.mapa_dedos.is_some() {
                let atual = self.espelho.modo_mapa;
                let mut modo = atual;
                egui::ComboBox::from_id_salt("piano.modo-mapa")
                    .width(if compacto { 120.0 } else { 190.0 })
                    .selected_text(modo.nome())
                    .show_ui(ui, |ui| {
                        for m in crate::dedos::MODOS_MAPA {
                            ui.selectable_value(&mut modo, m, m.nome());
                        }
                    })
                    .response
                    .on_hover_text(
                        "Escala em todas as oitavas: dedilhado contínuo (o 5 só entra no fim da \
                         escala; as teclas fora dela mostram o dedo da escala cromática, num \
                         círculo mais claro). Uma oitava: cada mão faz a escala de uma oitava, \
                         com o 5 na ponta. Cinco dedos: um dedo por tecla na tônica.",
                    );
                if modo != atual {
                    self.espelho.modo_mapa = modo;
                    self.gravar_exibicao();
                }
            }
            return;
        }
        if id != acoes::CONTROLE_INSTRUMENTO {
            return;
        }
        if let Some((banco, programa)) =
            seletor_instrumento::desenhar(ui, &self.som, &mut self.seletor, compacto)
        {
            self.executar_timbres(vec![AcaoTimbres::Timbre { banco, programa }]);
        }
        if !compacto {
            let (novo, soltou) = ui
                .horizontal(|ui| {
                    ui.label(crate::icones::glifo("volume"));
                    seletor_instrumento::volume(ui, self.som.volume(), 200.0)
                })
                .inner;
            if let Some(v) = novo {
                self.som.definir_volume(v);
            }
            if soltou {
                self.salvar_preferencias();
            }
        }
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
        let baixando = self
            .baixador_sf2
            .as_ref()
            .filter(|b| b.baixando())
            .and_then(BaixadorSf2::texto);
        let (tom_audio, dica_audio) = match self.som.estado() {
            _ if baixando.is_some() => (Tom::Info, baixando.clone().unwrap_or_default()),
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
                    .map(str::to_owned)
                    .or_else(|| self.baixador_sf2.as_ref().and_then(BaixadorSf2::texto))
                    .unwrap_or_else(|| "Sem som".to_owned()),
            ),
        };
        vec![
            SegmentoStatus::novo("versao", versao),
            SegmentoStatus::novo("midi", self.monitor.texto_status())
                .icone("midi")
                .tom(tom_midi)
                .dica(dica_midi)
                .clicavel(),
            SegmentoStatus::novo(
                "audio",
                if baixando.is_some() {
                    "Áudio: baixando SoundFont…"
                } else {
                    self.som.texto_status()
                },
            )
            .icone("volume")
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
            Some(Efeito::TrocarDedos(n)) => {
                self.trocar_dedos(n);
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
        if self.tema_sincronizado {
            let atual = self.banco.ler(secoes::TEMA).ok().flatten();
            let fabrica = self.banco.ativo().is_ok_and(|p| p.de_fabrica);
            // No de fábrica, o tema padrão não é mudança.
            let padrao = fabrica && atual.is_none() && preferencias.tema == TEMAS[0].id;
            if !padrao && atual.as_deref() != Some(preferencias.tema.as_str()) {
                let tema = preferencias.tema.clone();
                self.gravar_secao(secoes::TEMA, &tema);
            }
            // Ribbon recolhido, densidade, escala, painéis e tela cheia, na hora.
            if let Ok(json) = serde_json::to_string(preferencias) {
                let antes = self.banco.ler(secoes::CASCA).ok().flatten();
                if antes.as_deref() != Some(json.as_str()) {
                    self.gravar_secao(secoes::CASCA, &json);
                }
            }
        }
        self.tema_sincronizado = true;
    }
}

/// Ao fechar a janela, gravações não salvas vão para `gravacoes/recuperacao/` (F06).
impl Drop for AppPianoTutor {
    fn drop(&mut self) {
        if self.gravacao.tem_dados_nao_salvos() {
            self.gravacao.salvar_recuperacao();
        }
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

    #[test]
    fn perfis_guardam_e_trocam_dedilhado_e_partitura_e_o_de_fabrica_volta_ao_padrao() {
        let mut b = bancada();
        let (_, ativo) = b.app.perfis();
        let fabrica = ativo.expect("ativo");
        assert!(fabrica.de_fabrica);
        b.app.trocar_dedos(NomenclaturaDedos::LetrasPt);
        b.app.trocar_mapa_dedos(Some(7));
        let (perfis, ativo) = b.app.perfis();
        let meu = ativo.expect("ativo");
        assert_eq!(
            meu.nome,
            crate::perfis::NOME_PESSOAL,
            "mudança sobre o de fábrica"
        );
        assert_eq!(perfis.len(), 2);
        b.app.trocar_perfil(fabrica.id);
        assert_eq!(b.app.espelho.nomenclatura, NomenclaturaDedos::Numeros);
        assert_eq!(b.app.espelho.mapa_dedos, None);
        b.app.trocar_perfil(meu.id);
        assert_eq!(b.app.espelho.nomenclatura, NomenclaturaDedos::LetrasPt);
        assert_eq!(b.app.espelho.mapa_dedos, Some(7));
        b.app.criar_perfil("Aula");
        let (_, ativo) = b.app.perfis();
        assert_eq!(ativo.expect("ativo").nome, "Aula");
        assert_eq!(b.app.espelho.mapa_dedos, Some(7), "cópia do atual");
        b.app.apagar_perfil();
        assert!(b.app.perfis().1.expect("ativo").de_fabrica);
        assert_eq!(b.app.espelho.mapa_dedos, None);
    }

    #[test]
    fn opcoes_do_player_cursos_revisao_partitura_e_timbres_voltam_com_o_perfil() {
        use piano_tutor_core::pratica::Retomada;
        let mut b = bancada();
        let fabrica = b.app.perfis().1.expect("ativo");
        b.app.partitura.tolerancia_pratica_ms = 222.0;
        b.app.partitura.retomada_pratica = Retomada::Duas;
        b.app.cursos.opcoes_player.velocidade = 0.75;
        b.app.cursos.opcoes_player.video_a_direita = true;
        b.app.cursos.opcoes_player.modo = crate::player_aula::ModoAcompanhamento::SegueNoTempo;
        b.app.revisao.aba = crate::tela_revisao::AbaRevisao::Roteiro;
        b.app.revisao.ajustar_pagina = false;
        b.app.seletor.familia = Some(2);
        b.app.gravar_retratos();
        let meu = b.app.perfis().1.expect("ativo");
        assert!(!meu.de_fabrica);

        b.app.trocar_perfil(fabrica.id);
        assert_eq!(b.app.partitura.retomada_pratica, Retomada::Zero);
        assert_eq!(b.app.cursos.opcoes_player.velocidade, 1.0);
        assert!(!b.app.cursos.opcoes_player.video_a_direita);
        assert_eq!(
            b.app.revisao.aba,
            crate::tela_revisao::AbaRevisao::Partituras
        );
        assert!(b.app.revisao.ajustar_pagina);
        assert_eq!(b.app.seletor.familia, None);

        b.app.trocar_perfil(meu.id);
        assert_eq!(b.app.partitura.tolerancia_pratica_ms, 222.0);
        assert_eq!(b.app.partitura.retomada_pratica, Retomada::Duas);
        assert_eq!(b.app.cursos.opcoes_player.velocidade, 0.75);
        assert!(b.app.cursos.opcoes_player.video_a_direita);
        assert_eq!(
            b.app.cursos.opcoes_player.modo,
            crate::player_aula::ModoAcompanhamento::SegueNoTempo
        );
        assert_eq!(b.app.revisao.aba, crate::tela_revisao::AbaRevisao::Roteiro);
        assert!(!b.app.revisao.ajustar_pagina);
        assert_eq!(b.app.seletor.familia, Some(2));
    }

    #[test]
    fn a_ultima_tela_aberta_e_lembrada_no_perfil_e_o_de_fabrica_nao_muda() {
        let mut b = bancada();
        let fabrica = b.app.perfis().1.expect("ativo");
        // Perfil vazio: abre no Início (e, com o de fábrica, navegar não cria perfil).
        assert_eq!(b.app.rota_inicial().como_texto(), ROTA_INICIO);
        b.app.rota_atual = ROTA_PARTITURA.to_owned();
        b.app.gravar_retratos();
        assert!(
            b.app.perfis().1.expect("ativo").de_fabrica,
            "só navegar não cria Meu perfil"
        );
        assert_eq!(b.app.rota_inicial().como_texto(), ROTA_INICIO);

        // Com um perfil pessoal (uma opção mudou), a tela em que estava vai junto.
        b.app.acordes.de_ouvido = true;
        b.app.gravar_retratos();
        let meu = b.app.perfis().1.expect("ativo");
        assert!(!meu.de_fabrica);
        assert_eq!(b.app.rota_inicial().como_texto(), ROTA_PARTITURA);

        // Cada navegação seguinte é gravada na hora; Calibração e Sobre não são lembradas.
        b.app.rota_atual = ROTA_REVISAO.to_owned();
        b.app.gravar_retratos();
        assert_eq!(b.app.rota_inicial().como_texto(), ROTA_REVISAO);
        b.app.rota_atual = ROTA_CALIBRACAO.to_owned();
        b.app.gravar_retratos();
        assert_eq!(b.app.rota_inicial().como_texto(), ROTA_REVISAO);
        b.app.rota_atual = ROTA_SOBRE.to_owned();
        b.app.gravar_retratos();
        assert_eq!(b.app.rota_inicial().como_texto(), ROTA_REVISAO);
        b.app.rota_atual = ROTA_CURSOS.to_owned();
        b.app.gravar_retratos();
        assert_eq!(b.app.rota_inicial().como_texto(), ROTA_CURSOS);

        // Cada perfil tem a sua; trocar de perfil não muda a tela aberta.
        b.app.trocar_perfil(fabrica.id);
        assert_eq!(b.app.rota_inicial().como_texto(), ROTA_INICIO);
        assert_eq!(b.app.rota_atual, ROTA_CURSOS);
        b.app.trocar_perfil(meu.id);
        assert_eq!(b.app.rota_inicial().como_texto(), ROTA_CURSOS);

        // Rota desconhecida no banco: Início.
        b.app
            .gravar_secao(crate::retratos::secoes::APP, "rota=nao-existe\n");
        assert_eq!(b.app.rota_inicial().como_texto(), ROTA_INICIO);
    }

    #[test]
    fn o_timbre_das_demonstracoes_mudado_pela_tela_e_gravado_no_perfil() {
        use piano_tutor_audio::sf2_sintetico::PROGRAMA_QUADRADA;
        let mut b = bancada();
        // Primeiro quadro: o app aplica as preferências do perfil e anota a configuração, sem
        // gravar nada.
        b.app.aplicar_preferencias();
        b.app.gravar_retratos();
        assert!(b.app.perfis().1.expect("ativo").de_fabrica);
        // A tela Gravação muda o timbre das demonstrações direto no som.
        let quadrada = crate::som::Timbre {
            banco: 0,
            programa: PROGRAMA_QUADRADA,
        };
        let mut c = b.app.som.config_teclado();
        c.timbre_esquerda = quadrada;
        c.timbre_demonstracao = Some(quadrada);
        b.app.som.definir_config_teclado(c).expect("preset do SF2");
        b.app.gravar_retratos();
        assert!(!b.app.perfis().1.expect("ativo").de_fabrica);
        let gravado = PreferenciasSom::de_texto(&ler_secao(&b.app.banco, secoes::SOM));
        assert_eq!(gravado.teclado.timbre_demonstracao, c.timbre_demonstracao);
    }

    #[test]
    fn toda_opcao_de_tela_mudada_e_gravada_na_hora_e_volta_com_o_perfil() {
        use crate::teclado_visual::Rotulos;
        let mut b = bancada();
        let fabrica = b.app.perfis().1.expect("ativo");
        b.app.espelho.opcoes_tela.ajustar_a_largura = true;
        b.app.espelho.opcoes_tela.rotulos = Rotulos::Todas;
        b.app.espelho.definir_zoom(1.5);
        b.app.acordes.de_ouvido = true;
        b.app.acordes.treino.config.app_toca = false;
        b.app.ritmo.config.bateria = true;
        b.app.ritmo.definir_bpm(137);
        b.app.ritmo.pulso_salvo = (80, -12);
        b.app.gravacao.bpm_contagem = 90;
        b.app.partitura.modo_pratica = piano_tutor_core::pratica::ModoPratica::Tempo;
        b.app.gravar_retratos();
        let meu = b.app.perfis().1.expect("ativo");
        assert!(
            !meu.de_fabrica,
            "mudança sobre o de fábrica cria Meu perfil"
        );
        b.app.trocar_perfil(fabrica.id);
        assert!(!b.app.espelho.opcoes_tela.ajustar_a_largura);
        assert!(!b.app.acordes.de_ouvido);
        assert_ne!(b.app.ritmo.bpm(), 137);
        b.app.trocar_perfil(meu.id);
        assert!(b.app.espelho.opcoes_tela.ajustar_a_largura);
        assert_eq!(b.app.espelho.opcoes_tela.rotulos, Rotulos::Todas);
        assert!((b.app.espelho.zoom() - 1.5).abs() < 1e-6);
        assert!(b.app.acordes.de_ouvido);
        assert!(!b.app.acordes.treino.config.app_toca);
        assert!(b.app.ritmo.config.bateria);
        assert_eq!(b.app.ritmo.bpm(), 137);
        assert_eq!(b.app.ritmo.pulso_salvo, (80, -12));
        assert_eq!(b.app.gravacao.bpm_contagem, 90);
        assert_eq!(
            b.app.partitura.modo_pratica,
            piano_tutor_core::pratica::ModoPratica::Tempo
        );
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
            baixador_sf2: None,
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

    fn bends(r: &[piano_tutor_audio::sintetizador::RegistroFalso]) -> Vec<(u8, i16)> {
        r.iter()
            .filter_map(|e| match *e {
                piano_tutor_audio::sintetizador::RegistroFalso::PitchBend {
                    canal, valor, ..
                } => Some((canal, valor)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn roda_virtual_envia_bend_ao_motor_e_panico_centra() {
        let mut b = bancada();
        let ctx = egui::Context::default();
        // Um quadro vazio para o contexto ter foco e relógio.
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        b.app.tick();
        b.motor.renderizar(BLOCO, 0);
        let antes = bends(&b.motor.registro()).len();
        b.app.espelho.roda_bend.girar(3.0, 0.0);
        b.app.atualizar_roda_bend(&ctx);
        b.motor.renderizar(BLOCO, 0);
        let novos = bends(&b.motor.registro())[antes..].to_vec();
        assert_eq!(novos.len(), 3, "um por canal do teclado");
        assert!(
            novos.iter().all(|(_, v)| (*v - 4096).abs() <= 1),
            "{novos:?}"
        );
        assert!((b.app.som.pitch_bend_roda() - 4096).abs() <= 1);
        b.app.panico();
        b.motor.renderizar(BLOCO, 0);
        assert!(b.app.espelho.roda_bend.centrada());
        assert_eq!(b.app.som.pitch_bend_roda(), 0);
        let r = b.motor.registro();
        assert!(bends(&r).iter().rev().take(3).all(|(_, v)| *v == 0));
    }

    #[test]
    fn pitch_bend_do_teclado_fisico_chega_ao_motor() {
        let mut b = bancada();
        b.app.tick();
        b.app
            .monitor
            .executar(crate::monitor::AcaoMonitor::Conectar);
        b.app.tick();
        b.motor.renderizar(BLOCO, 0);
        let antes = bends(&b.motor.registro()).len();
        assert!(b.fonte.injetar(&[0xE0, 0x00, 0x60]));
        b.app.tick();
        b.motor.renderizar(BLOCO * 8, 0);
        let novos = bends(&b.motor.registro())[antes..].to_vec();
        // 0x60 << 7 = 12288 - 8192 = 4096 em cada canal do teclado.
        assert_eq!(novos, vec![(0, 4096), (1, 4096), (2, 4096)]);
        // O indicador da tela acompanha a roda do teclado físico.
        assert_eq!(b.app.espelho.bend_fisico, 4096);
        let (fracao, fisico) = b.app.espelho.bend_exibido();
        assert!(fisico && (fracao - 0.5).abs() < 0.01, "{fracao}");
    }

    #[test]
    fn sf2_baixado_em_segundo_plano_e_carregado_e_tela_mostra_progresso() {
        use crate::baixar_sf2::{AbridorDownload, BaixadorSf2, FonteSf2};
        use sha2::{Digest, Sha256};
        let bytes = piano_tutor_audio::sf2_sintetico::gerar();
        let sha: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let fonte = FonteSf2 {
            nome: "Baixado.sf2",
            url: "https://exemplo.invalido/Baixado.sf2",
            tamanho: bytes.len() as u64,
            sha256: Box::leak(sha.into_boxed_str()),
        };
        let abridor: AbridorDownload = Arc::new(move |_| {
            Ok(Box::new(std::io::Cursor::new(bytes.clone())) as Box<dyn std::io::Read + Send>)
        });
        let destino = std::env::temp_dir()
            .join(format!("pt-app-baixado-{}", std::process::id()))
            .join("Baixado.sf2");
        let _ = std::fs::remove_file(&destino);
        let mut baixador = BaixadorSf2::novo(fonte, abridor);
        baixador.iniciar(Some(destino.clone()));
        let mut b = bancada();
        b.app.baixador_sf2 = Some(baixador);
        b.app.tick();
        let fim = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while b.app.som.caminho_sf2() != Some(destino.as_path()) {
            assert!(
                std::time::Instant::now() < fim,
                "SF2 baixado não foi carregado"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
            b.app.tick();
        }
        assert_eq!(b.app.timbres.download_texto, None);
        assert_eq!(b.app.timbres.campo_caminho, destino.display().to_string());
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
    fn a_fonte_da_janela_tem_os_simbolos_da_tela_de_revisao() {
        let mut c = casca();
        let ctx = Context::default();
        quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
        quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
        let fonte = egui::FontId::proportional(14.0);
        let usados: String = crate::revisao::ESTADOS
            .iter()
            .map(|e| crate::revisao::simbolo_do_estado(*e))
            .collect::<String>()
            + "[!]—·";
        let faltam: String = usados
            .chars()
            .filter(|ch| !ctx.fonts_mut(|f| f.has_glyph(&fonte, *ch)))
            .collect();
        assert!(faltam.is_empty(), "símbolos ausentes: {faltam:?}");
    }

    #[test]
    fn a_tela_de_revisao_abre_pela_navegacao_com_o_aviso_do_modo_tutor() {
        let mut c = casca();
        let ctx = Context::default();
        quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
        let itens: Vec<(String, ItemNavegacao)> = c
            .app
            .navegacao()
            .into_iter()
            .flat_map(|g| g.itens.into_iter().map(move |i| (g.rotulo.clone(), i)))
            .collect();
        let (grupo, item) = itens
            .iter()
            .find(|(_, i)| i.rota.como_texto() == ROTA_REVISAO)
            .expect("item de revisão na navegação");
        assert_eq!(grupo, "Tutor");
        assert!(item.indisponivel.is_none());
        abrir(&ctx, ROTA_REVISAO);
        for _ in 0..4 {
            quadro(&mut c, &ctx, Vec::new(), Modifiers::NONE);
        }
        assert_eq!(c.estado.historico.atual().como_texto(), ROTA_REVISAO);
        assert!(c.estado.erros.is_empty(), "{:?}", c.estado.erros);
        assert!(!c.app.revisao.tocando());
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
        assert_eq!(c.tema().tema.id, "grafite");

        apertar(&mut c, &ctx, Key::T, Modifiers::COMMAND);
        assert_eq!(c.tema().tema.id, "executivo");
        assert_eq!(c.app.estado.tema_atual().id, "executivo");
        assert_eq!(c.app.estado.textos_de_status()[2], "Tema: Executivo");

        for _ in 0..4 {
            apertar(&mut c, &ctx, Key::T, Modifiers::COMMAND);
        }
        assert_eq!(c.tema().tema.id, "grafite", "volta ao primeiro");
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
