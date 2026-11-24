//! Estado do painel Monitor MIDI (F01, #32), sem janela: o [`GerenciadorMidi`], os últimos
//! eventos, a seleção de porta e o log NDJSON da sessão.
//!
//! A tela (`tela_monitor`) só lê este estado e devolve [`AcaoMonitor`]; o app chama
//! [`EstadoMonitor::atualizar`] a cada quadro e roteia as [`SaidaMidi`] devolvidas.

use piano_tutor_core::midi::EventoMidi;
use piano_tutor_core::teclado::{ConfigNomes, rotulo};
use piano_tutor_midi::{
    Capacidades, Contadores, EstadoConexao, FontePortas, GerenciadorMidi, Observacao, Relogio,
    SaidaMidi, criar_arquivo_sessao,
};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// Quantos eventos a tabela guarda (os mais recentes).
pub const MAX_EVENTOS: usize = 200;

/// Texto mostrado para uma capacidade não observada (F01: não inventar valores).
pub const NAO_SUPORTADO: &str = "não suportado pelo dispositivo";

/// Uma linha da tabela de últimos eventos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinhaEvento {
    /// Nanossegundos do relógio comum (mesma base do áudio).
    pub ts_ns: u64,
    pub tipo: &'static str,
    /// Canal como exibido (1–16).
    pub canal: Option<u8>,
    pub dado: String,
    pub valor: String,
}

impl LinhaEvento {
    pub fn de_evento(e: &EventoMidi) -> Self {
        let ts_ns = e.timestamp_monotonico_ns();
        let nome = |n| texto_sem_glifos(&rotulo(n, ConfigNomes::default()));
        match *e {
            EventoMidi::NotaLigada {
                canal,
                nota,
                velocidade,
                ..
            } => Self {
                ts_ns,
                tipo: "Nota ligada",
                canal: Some(canal.exibicao()),
                dado: format!("{} ({})", nota.numero(), nome(nota)),
                valor: format!("vel. {velocidade}"),
            },
            EventoMidi::NotaDesligada {
                canal,
                nota,
                velocidade,
                ..
            } => Self {
                ts_ns,
                tipo: "Nota desligada",
                canal: Some(canal.exibicao()),
                dado: format!("{} ({})", nota.numero(), nome(nota)),
                valor: format!("vel. {velocidade}"),
            },
            EventoMidi::ControleMudou {
                canal,
                controle,
                valor,
                ..
            } => Self {
                ts_ns,
                tipo: "Controle",
                canal: Some(canal.exibicao()),
                dado: if controle == 64 {
                    "CC64 (sustain)".to_owned()
                } else {
                    format!("CC{controle}")
                },
                valor: valor.to_string(),
            },
            EventoMidi::ProgramaMudou {
                canal, programa, ..
            } => Self {
                ts_ns,
                tipo: "Programa",
                canal: Some(canal.exibicao()),
                dado: format!("programa {programa}"),
                valor: String::new(),
            },
            EventoMidi::PitchBend { canal, valor, .. } => Self {
                ts_ns,
                tipo: "Pitch bend",
                canal: Some(canal.exibicao()),
                dado: "roda de afinação".to_owned(),
                valor: format!("{valor:+}"),
            },
            EventoMidi::Desconhecido { .. } => Self {
                ts_ns,
                tipo: "Outro",
                canal: None,
                dado: "mensagem fora do escopo".to_owned(),
                valor: String::new(),
            },
        }
    }

    /// Tempo em segundos com vírgula ("12,345 s").
    pub fn tempo(&self) -> String {
        format!("{:.3} s", self.ts_ns as f64 / 1e9).replace('.', ",")
    }
}

/// O que a tela pediu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AcaoMonitor {
    AtualizarLista,
    Escolher(String),
    Conectar,
    Desconectar,
    Reconectar,
    IniciarLog,
    PararLog,
    LimparEventos,
}

/// Uma linha da tabela de capacidades.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinhaCapacidade {
    pub nome: &'static str,
    pub observada: bool,
    pub texto: String,
}

/// Estado do Monitor MIDI.
pub struct EstadoMonitor {
    gerenciador: GerenciadorMidi,
    eventos: VecDeque<LinhaEvento>,
    selecao: Option<String>,
    pasta_sessoes: Option<PathBuf>,
    arquivo_log: Option<PathBuf>,
    mensagem_log: Option<String>,
}

impl std::fmt::Debug for EstadoMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EstadoMonitor")
            .field("estado", self.gerenciador.estado())
            .field("selecao", &self.selecao)
            .field("eventos", &self.eventos.len())
            .finish_non_exhaustive()
    }
}

impl EstadoMonitor {
    /// `relogio` é o relógio comum (o mesmo da saída de áudio).
    pub fn novo(
        fonte: Box<dyn FontePortas>,
        relogio: Arc<dyn Relogio>,
        pasta_sessoes: Option<PathBuf>,
    ) -> Self {
        Self {
            gerenciador: GerenciadorMidi::novo(fonte, relogio),
            eventos: VecDeque::with_capacity(MAX_EVENTOS),
            selecao: None,
            pasta_sessoes,
            arquivo_log: None,
            mensagem_log: None,
        }
    }

    pub fn gerenciador(&self) -> &GerenciadorMidi {
        &self.gerenciador
    }

    pub fn gerenciador_mut(&mut self) -> &mut GerenciadorMidi {
        &mut self.gerenciador
    }

    /// Processa a entrada MIDI (chamar a cada quadro). Guarda os eventos na tabela e devolve as
    /// saídas para o app rotear (Espelho e som), na ordem.
    pub fn atualizar(&mut self) -> Vec<SaidaMidi> {
        let saidas = self.gerenciador.atualizar();
        for s in &saidas {
            if let SaidaMidi::Evento(e) = s {
                if self.eventos.len() == MAX_EVENTOS {
                    self.eventos.pop_front();
                }
                self.eventos.push_back(LinhaEvento::de_evento(e));
            }
        }
        if self.selecao.is_none() {
            self.selecao = self
                .gerenciador
                .porta_selecionada()
                .map(str::to_owned)
                .or_else(|| porta_sugerida(self.gerenciador.portas()).map(str::to_owned));
        }
        saidas
    }

    /// Tempo até a próxima varredura (a UI agenda o próximo quadro com isso).
    pub fn proxima_varredura_em(&self) -> Duration {
        self.gerenciador.proxima_varredura_em()
    }

    /// Executa um pedido da tela. As saídas resultantes (limpeza de notas, mudança de estado)
    /// saem no próximo [`EstadoMonitor::atualizar`].
    pub fn executar(&mut self, acao: AcaoMonitor) {
        match acao {
            AcaoMonitor::AtualizarLista => {
                let _ = self.gerenciador.listar_portas();
            }
            AcaoMonitor::Escolher(p) => self.selecao = Some(p),
            AcaoMonitor::Conectar => {
                if let Some(p) = self.selecao.clone() {
                    self.gerenciador.selecionar_porta(&p);
                }
            }
            AcaoMonitor::Desconectar => self.gerenciador.desconectar(),
            AcaoMonitor::Reconectar => self.gerenciador.reconectar(),
            AcaoMonitor::IniciarLog => self.iniciar_log(),
            AcaoMonitor::PararLog => self.parar_log(),
            AcaoMonitor::LimparEventos => self.eventos.clear(),
        }
    }

    fn iniciar_log(&mut self) {
        let Some(pasta) = self.pasta_sessoes.clone() else {
            self.mensagem_log = Some(
                "Pasta de dados do usuário desconhecida (HOME e XDG_DATA_HOME ausentes); \
                 log não iniciado."
                    .to_owned(),
            );
            return;
        };
        match criar_arquivo_sessao(&pasta, SystemTime::now()) {
            Ok((caminho, registro)) => {
                self.gerenciador
                    .iniciar_registro(Box::new(registro.em_saida()));
                self.mensagem_log = None;
                self.arquivo_log = Some(caminho);
            }
            Err(e) => {
                self.mensagem_log = Some(format!(
                    "Não foi possível criar o log em {}: {e}",
                    pasta.display()
                ));
            }
        }
    }

    fn parar_log(&mut self) {
        if let Some((linhas, falhas)) = self.gerenciador.parar_registro() {
            let arquivo = self
                .arquivo_log
                .as_deref()
                .map_or_else(String::new, |c| c.display().to_string());
            self.mensagem_log = Some(format!(
                "Log encerrado: {linhas} linhas, {falhas} falhas de escrita. {arquivo}"
            ));
        }
    }

    // --- Leitura -------------------------------------------------------------------------------

    pub fn estado(&self) -> &EstadoConexao {
        self.gerenciador.estado()
    }

    /// Texto curto do estado ("Conectado", "Desconectado — aguardando a porta voltar"…).
    pub fn texto_estado(&self) -> String {
        let aguardando = self.gerenciador.aguardando_reconexao();
        match self.gerenciador.estado() {
            EstadoConexao::Desconectado if aguardando => {
                "Desconectado — aguardando a porta voltar".to_owned()
            }
            EstadoConexao::Desconectado => "Desconectado".to_owned(),
            EstadoConexao::Conectando { .. } => "Conectando…".to_owned(),
            EstadoConexao::Conectado { .. } => "Conectado".to_owned(),
            EstadoConexao::Erro { .. } if aguardando => {
                "Erro — aguardando a porta aparecer".to_owned()
            }
            EstadoConexao::Erro { .. } => "Erro".to_owned(),
        }
    }

    /// Texto da barra de status: "MIDI: <estado>".
    pub fn texto_status(&self) -> String {
        format!("MIDI: {}", self.texto_estado())
    }

    /// Detalhe do estado (porta ou mensagem de erro).
    pub fn detalhe_estado(&self) -> Option<String> {
        match self.gerenciador.estado() {
            EstadoConexao::Conectado { porta } | EstadoConexao::Conectando { porta } => {
                Some(porta.clone())
            }
            EstadoConexao::Erro { mensagem } => Some(mensagem.clone()),
            EstadoConexao::Desconectado => self.gerenciador.porta_selecionada().map(str::to_owned),
        }
    }

    pub fn portas(&self) -> &[String] {
        self.gerenciador.portas()
    }

    pub fn selecao(&self) -> Option<&str> {
        self.selecao.as_deref()
    }

    pub fn eventos(&self) -> impl DoubleEndedIterator<Item = &LinhaEvento> + ExactSizeIterator {
        self.eventos.iter()
    }

    pub fn contadores(&self) -> Contadores {
        self.gerenciador.contadores()
    }

    pub fn registrando(&self) -> bool {
        self.gerenciador.registrando()
    }

    pub fn arquivo_log(&self) -> Option<&Path> {
        self.arquivo_log.as_deref()
    }

    pub fn mensagem_log(&self) -> Option<&str> {
        self.mensagem_log.as_deref()
    }

    pub fn pasta_sessoes(&self) -> Option<&Path> {
        self.pasta_sessoes.as_deref()
    }

    /// Capacidades com o texto da UI: o que não foi observado aparece como
    /// [`NAO_SUPORTADO`], sem valor inventado.
    pub fn capacidades(&self) -> Vec<LinhaCapacidade> {
        linhas_de_capacidade(self.gerenciador.capacidades())
    }
}

/// Monta as linhas de capacidade a partir do que foi observado.
pub fn linhas_de_capacidade(c: &Capacidades) -> Vec<LinhaCapacidade> {
    let linha = |nome, obs: Observacao, detalhe: Option<String>| {
        let observada = obs == Observacao::Observado;
        let texto = if observada {
            detalhe.map_or_else(|| "observado".to_owned(), |d| format!("observado ({d})"))
        } else {
            NAO_SUPORTADO.to_owned()
        };
        LinhaCapacidade {
            nome,
            observada,
            texto,
        }
    };
    let canais: Vec<String> = c
        .canais_observados()
        .map(|c| c.exibicao().to_string())
        .collect();
    vec![
        linha(
            "Velocity variável",
            c.velocidade_variavel(),
            c.faixa_velocidade().map(|(a, b)| format!("{a}–{b}")),
        ),
        linha(
            "Pedal de sustentação (CC64)",
            c.pedal_sustain(),
            c.ultimo_valor_sustain()
                .map(|v| format!("último valor {v}")),
        ),
        linha(
            "Mudança de programa",
            c.mudanca_de_programa(),
            c.ultimo_programa().map(|p| format!("último {p}")),
        ),
        linha("Liberação de nota", c.liberacao_de_nota(), None),
        LinhaCapacidade {
            nome: "Canais em uso",
            observada: !canais.is_empty(),
            texto: if canais.is_empty() {
                "nenhuma mensagem de canal recebida".to_owned()
            } else {
                canais.join(", ")
            },
        },
    ]
}

/// Pasta das sessões MIDI pela especificação XDG: `$XDG_DATA_HOME/piano-tutor/sessoes`, ou
/// `$HOME/.local/share/piano-tutor/sessoes`. `XDG_DATA_HOME` relativo é ignorado (a
/// especificação exige caminho absoluto).
pub fn pasta_de_sessoes(
    xdg_data_home: Option<OsString>,
    home: Option<OsString>,
) -> Option<PathBuf> {
    let base = xdg_data_home
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            home.map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|h| h.join(".local").join("share"))
        })?;
    Some(base.join("piano-tutor").join("sessoes"))
}

/// [`pasta_de_sessoes`] com as variáveis do ambiente atual.
pub fn pasta_de_sessoes_do_ambiente() -> Option<PathBuf> {
    pasta_de_sessoes(std::env::var_os("XDG_DATA_HOME"), std::env::var_os("HOME"))
}

/// A fonte embutida do egui não tem ♯ nem ♭.
fn texto_sem_glifos(s: &str) -> String {
    s.replace('♯', "#").replace('♭', "b")
}

/// Porta sugerida quando o usuário ainda não escolheu: a primeira que não seja a porta virtual
/// "Midi Through" do ALSA (que nunca recebe nada do teclado); se só houver ela, usa a primeira.
fn porta_sugerida(portas: &[String]) -> Option<&str> {
    portas
        .iter()
        .find(|p| !p.contains("Midi Through"))
        .or_else(|| portas.first())
        .map(String::as_str)
}

#[cfg(test)]
mod testes {
    #[test]
    fn sugestao_pula_midi_through() {
        let portas = vec![
            "Midi Through:Midi Through Port-0 14:0".to_string(),
            "USB AudioDevice:USB AudioDevice MIDI 1 24:0".to_string(),
        ];
        assert_eq!(
            super::porta_sugerida(&portas),
            Some("USB AudioDevice:USB AudioDevice MIDI 1 24:0")
        );
        assert_eq!(
            super::porta_sugerida(&portas[..1]),
            Some("Midi Through:Midi Through Port-0 14:0")
        );
        assert_eq!(super::porta_sugerida(&[]), None);
    }

    use super::*;
    use piano_tutor_midi::{FonteFalsa, RelogioManual};

    const KB: &str = "KB-330:KB-330 MIDI 1 20:0";

    fn monitor(pasta: Option<PathBuf>) -> (EstadoMonitor, FonteFalsa, RelogioManual) {
        let fonte = FonteFalsa::nova([KB]);
        let relogio = RelogioManual::novo();
        let m = EstadoMonitor::novo(Box::new(fonte.clone()), Arc::new(relogio.clone()), pasta);
        (m, fonte, relogio)
    }

    #[test]
    fn xdg_tem_prioridade_relativo_e_ignorado_e_home_e_o_reserva() {
        assert_eq!(
            pasta_de_sessoes(Some("/x/dados".into()), Some("/home/a".into())),
            Some(PathBuf::from("/x/dados/piano-tutor/sessoes"))
        );
        assert_eq!(
            pasta_de_sessoes(Some("relativo".into()), Some("/home/a".into())),
            Some(PathBuf::from("/home/a/.local/share/piano-tutor/sessoes"))
        );
        assert_eq!(pasta_de_sessoes(None, None), None);
    }

    #[test]
    fn primeira_porta_fica_pre_selecionada_e_conectar_usa_a_selecao() {
        let (mut m, fonte, _) = monitor(None);
        let _ = m.atualizar();
        assert_eq!(m.selecao(), Some(KB));
        assert_eq!(m.texto_status(), "MIDI: Desconectado");
        m.executar(AcaoMonitor::Conectar);
        let _ = m.atualizar();
        assert_eq!(m.texto_status(), "MIDI: Conectado");
        assert_eq!(m.detalhe_estado().as_deref(), Some(KB));
        assert_eq!(fonte.conexao_aberta().as_deref(), Some(KB));
    }

    #[test]
    fn desconexao_mostra_aguardando_e_reconexao_volta_a_conectado() {
        let (mut m, fonte, relogio) = monitor(None);
        let _ = m.atualizar();
        m.executar(AcaoMonitor::Conectar);
        let _ = m.atualizar();
        fonte.remover_porta(KB);
        relogio.avancar(Duration::from_secs(1));
        let saidas = m.atualizar();
        assert!(saidas.contains(&SaidaMidi::LimparNotas));
        assert_eq!(
            m.texto_status(),
            "MIDI: Desconectado — aguardando a porta voltar"
        );
        fonte.adicionar_porta(KB);
        relogio.avancar(Duration::from_secs(1));
        let _ = m.atualizar();
        assert_eq!(m.texto_estado(), "Conectado");
    }

    #[test]
    fn tabela_guarda_os_ultimos_eventos_com_limite() {
        let (mut m, fonte, _) = monitor(None);
        let _ = m.atualizar();
        m.executar(AcaoMonitor::Conectar);
        for i in 0..(MAX_EVENTOS + 10) {
            assert!(fonte.injetar(&[0x90, 36 + (i % 61) as u8, 100]));
        }
        assert!(fonte.injetar(&[0xB0, 64, 127]));
        let _ = m.atualizar();
        assert_eq!(m.eventos().len(), MAX_EVENTOS);
        let ultimo = m.eventos().last().expect("há eventos");
        assert_eq!(ultimo.tipo, "Controle");
        assert_eq!(ultimo.dado, "CC64 (sustain)");
        assert_eq!(ultimo.canal, Some(1));
        m.executar(AcaoMonitor::LimparEventos);
        assert_eq!(m.eventos().len(), 0);
    }

    #[test]
    fn capacidade_nao_observada_aparece_como_nao_suportada() {
        let (mut m, fonte, _) = monitor(None);
        let _ = m.atualizar();
        m.executar(AcaoMonitor::Conectar);
        let linhas = m.capacidades();
        assert!(
            linhas
                .iter()
                .take(4)
                .all(|l| !l.observada && l.texto == NAO_SUPORTADO),
            "{linhas:?}"
        );
        // Velocity fixa continua "não suportada"; liberação de nota passa a observada.
        assert!(fonte.injetar(&[0x90, 60, 100]));
        assert!(fonte.injetar(&[0x90, 60, 0]));
        let _ = m.atualizar();
        let linhas = m.capacidades();
        assert_eq!(linhas[0].texto, NAO_SUPORTADO);
        assert!(linhas[3].observada);
        assert_eq!(linhas[4].texto, "1");
    }

    #[test]
    fn log_ndjson_e_criado_na_pasta_de_sessoes_e_encerrado() {
        let pasta = std::env::temp_dir()
            .join(format!("piano-tutor-monitor-{}", std::process::id()))
            .join("sessoes");
        let (mut m, fonte, _) = monitor(Some(pasta.clone()));
        let _ = m.atualizar();
        m.executar(AcaoMonitor::Conectar);
        m.executar(AcaoMonitor::IniciarLog);
        assert!(m.registrando(), "{:?}", m.mensagem_log());
        let arquivo = m.arquivo_log().expect("arquivo criado").to_path_buf();
        assert!(arquivo.starts_with(&pasta));
        assert!(fonte.injetar(&[0x90, 60, 100]));
        let _ = m.atualizar();
        m.executar(AcaoMonitor::PararLog);
        assert!(!m.registrando());
        assert!(m.mensagem_log().is_some_and(|t| t.contains("3 linhas")));
        let texto = std::fs::read_to_string(&arquivo).expect("log legível");
        let linhas: Vec<&str> = texto.lines().collect();
        assert_eq!(linhas.len(), 3, "{texto}");
        assert!(linhas[0].contains("\"tipo\":\"sessao\""));
        assert!(linhas[2].contains("\"tipo\":\"nota_ligada\""));
        let _ = std::fs::remove_dir_all(pasta.parent().unwrap_or(&pasta));
    }

    #[test]
    fn log_sem_pasta_de_dados_e_erro_na_tela_sem_panico() {
        let (mut m, _, _) = monitor(None);
        m.executar(AcaoMonitor::IniciarLog);
        assert!(!m.registrando());
        assert!(
            m.mensagem_log()
                .is_some_and(|t| t.contains("log não iniciado"))
        );
    }

    #[test]
    fn linha_de_evento_usa_nome_sem_glifo_e_tempo_com_virgula() {
        let e = piano_tutor_core::midi::interpretar(&[0x91, 61, 90], 1_500_000_000)
            .expect("mensagem válida");
        let l = LinhaEvento::de_evento(&e);
        assert_eq!(l.tipo, "Nota ligada");
        assert_eq!(l.canal, Some(2));
        assert!(l.dado.starts_with("61 ("), "{}", l.dado);
        assert!(!l.dado.contains('♯'));
        assert_eq!(l.tempo(), "1,500 s");
    }
}
