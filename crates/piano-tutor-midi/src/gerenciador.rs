//! Gerenciador da entrada MIDI: seleção manual de porta, conexão, estado
//! observável, detecção de desconexão por varredura, limpeza de notas e
//! reconexão automática.
//!
//! Modelo de uso (sem threads próprias): a UI chama [`GerenciadorMidi::atualizar`]
//! a cada quadro, ou no mínimo a cada [`GerenciadorMidi::proxima_varredura_em`]
//! (≤ 1 s), e aplica as [`SaidaMidi`] devolvidas na ordem. A thread do backend
//! só copia bytes + timestamp para um canal; interpretação, capacidades e log
//! acontecem em `atualizar`.
//!
//! Garantia de tempo: com `atualizar` chamado pelo menos a cada intervalo de
//! varredura `I` (≤ 1 s), a perda da porta é percebida em até `2·I` (≤ 2 s),
//! dentro da meta de 3 s da F01.

use crate::capacidades::Capacidades;
use crate::fonte::{ConexaoAtiva, ErroFonte, FontePortas, chave_estavel, localizar_porta};
use crate::registro::{MotivoDesconexao, RegistroNdjson};
use crate::relogio::Relogio;
use piano_tutor_core::midi::{EventoMidi, Parser};
use piano_tutor_core::teclado::EstadoTeclas;
use std::io::Write;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

/// Intervalo máximo (e padrão) entre varreduras da lista de portas.
pub const INTERVALO_VARREDURA_MAXIMO: Duration = Duration::from_secs(1);

/// Estado da conexão, para a UI exibir.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EstadoConexao {
    Desconectado,
    Conectando { porta: String },
    Conectado { porta: String },
    Erro { mensagem: String },
}

/// Saídas de [`GerenciadorMidi::atualizar`], a aplicar **na ordem**.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SaidaMidi {
    /// Evento interpretado pelo [`Parser`] do núcleo.
    Evento(EventoMidi),
    /// Soltar todas as notas (chamar [`EstadoTeclas::limpar_tudo`] e silenciar vozes).
    LimparNotas,
    /// O estado da conexão mudou para este valor.
    EstadoMudou(EstadoConexao),
}

/// Contadores para o painel e para a evidência de "zero perdidos".
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Contadores {
    pub mensagens_recebidas: u64,
    pub eventos: u64,
    pub erros_de_interpretacao: u64,
    pub conexoes: u64,
    pub reconexoes: u64,
    pub desconexoes_detectadas: u64,
    pub falhas_de_listagem: u64,
}

struct Mensagem {
    ts_ns: u64,
    bytes: Vec<u8>,
}

struct Conexao {
    ativa: Box<dyn ConexaoAtiva>,
    porta: String,
    rx: Receiver<Mensagem>,
}

pub struct GerenciadorMidi {
    fonte: Box<dyn FontePortas>,
    relogio: Arc<dyn Relogio>,
    intervalo_ns: u64,
    estado: EstadoConexao,
    porta_desejada: Option<String>,
    conexao: Option<Conexao>,
    perdeu_porta: bool,
    parser: Parser,
    portas: Vec<String>,
    ultima_varredura_ns: Option<u64>,
    ultimo_erro_listagem: Option<String>,
    capacidades: Capacidades,
    contadores: Contadores,
    registro: Option<RegistroNdjson<Box<dyn Write + Send>>>,
    pendentes: Vec<SaidaMidi>,
}

impl GerenciadorMidi {
    /// `relogio` é a base única de tempo: a fonte carimba as mensagens com ele.
    pub fn novo(fonte: Box<dyn FontePortas>, relogio: Arc<dyn Relogio>) -> Self {
        Self {
            fonte,
            relogio,
            intervalo_ns: duracao_ns(INTERVALO_VARREDURA_MAXIMO),
            estado: EstadoConexao::Desconectado,
            porta_desejada: None,
            conexao: None,
            perdeu_porta: false,
            parser: Parser::novo(),
            portas: Vec::new(),
            ultima_varredura_ns: None,
            ultimo_erro_listagem: None,
            capacidades: Capacidades::novas(),
            contadores: Contadores::default(),
            registro: None,
            pendentes: Vec::new(),
        }
    }

    /// Define o intervalo de varredura, limitado a 1 ms–1 s.
    #[must_use]
    pub fn com_intervalo_varredura(mut self, intervalo: Duration) -> Self {
        let i = intervalo.clamp(Duration::from_millis(1), INTERVALO_VARREDURA_MAXIMO);
        self.intervalo_ns = duracao_ns(i);
        self
    }

    /// Liga o log NDJSON da sessão (grava a linha `sessao`).
    #[must_use]
    pub fn com_registro(mut self, saida: Box<dyn Write + Send>) -> Self {
        let mut r = RegistroNdjson::novo(saida);
        r.registrar_inicio_sessao(self.relogio.agora_ns());
        r.descarregar();
        self.registro = Some(r);
        self
    }

    pub fn estado(&self) -> &EstadoConexao {
        &self.estado
    }

    /// Porta escolhida pelo usuário (mantida enquanto aguarda reconexão).
    pub fn porta_selecionada(&self) -> Option<&str> {
        self.porta_desejada.as_deref()
    }

    /// Verdadeiro quando a porta escolhida sumiu e o gerenciador aguarda a volta.
    pub fn aguardando_reconexao(&self) -> bool {
        self.conexao.is_none() && self.porta_desejada.is_some()
    }

    /// Última lista de portas obtida.
    pub fn portas(&self) -> &[String] {
        &self.portas
    }

    pub fn ultimo_erro_de_listagem(&self) -> Option<&str> {
        self.ultimo_erro_listagem.as_deref()
    }

    pub fn capacidades(&self) -> &Capacidades {
        &self.capacidades
    }

    pub fn contadores(&self) -> Contadores {
        self.contadores
    }

    /// Linhas gravadas e falhas do log, se ligado.
    pub fn registro_linhas_e_falhas(&self) -> Option<(u64, u64)> {
        self.registro.as_ref().map(|r| (r.linhas(), r.falhas()))
    }

    /// Tempo até a próxima varredura (para agendar o próximo `atualizar`).
    pub fn proxima_varredura_em(&self) -> Duration {
        let agora = self.relogio.agora_ns();
        let falta = self.ultima_varredura_ns.map_or(0, |u| {
            (u.saturating_add(self.intervalo_ns)).saturating_sub(agora)
        });
        Duration::from_nanos(falta)
    }

    /// Força nova listagem de portas agora.
    pub fn listar_portas(&mut self) -> Result<&[String], ErroFonte> {
        self.varrer()?;
        Ok(&self.portas)
    }

    /// Seleção manual. Fecha a conexão atual (com limpeza de notas) se for
    /// outra porta e tenta conectar já; se a porta não estiver presente, o
    /// estado vira `Erro` e a conexão acontece sozinha quando ela aparecer.
    pub fn selecionar_porta(&mut self, nome: &str) {
        if self.conexao.as_ref().is_some_and(|c| c.porta == nome) {
            return;
        }
        let mesma_chave = self
            .porta_desejada
            .as_deref()
            .is_some_and(|p| chave_estavel(p) == chave_estavel(nome));
        self.drenar();
        self.encerrar(MotivoDesconexao::TrocaDePorta);
        if !mesma_chave {
            self.capacidades = Capacidades::novas();
        }
        self.porta_desejada = Some(nome.to_string());
        self.perdeu_porta = false;
        match self.varrer() {
            Ok(()) => self.tentar_reconectar(true),
            Err(e) => self.mudar_estado(EstadoConexao::Erro {
                mensagem: e.to_string(),
            }),
        }
        self.descarregar_registro();
    }

    /// Desconexão manual: esquece a porta escolhida e limpa as notas.
    pub fn desconectar(&mut self) {
        self.drenar();
        self.encerrar(MotivoDesconexao::Manual);
        self.porta_desejada = None;
        self.perdeu_porta = false;
        self.mudar_estado(EstadoConexao::Desconectado);
        self.descarregar_registro();
    }

    /// Fecha e reabre a conexão com a porta escolhida (útil se o dispositivo
    /// for replugado mais rápido que a varredura e mantiver o mesmo nome).
    pub fn reconectar(&mut self) {
        let Some(porta) = self.porta_desejada.clone() else {
            return;
        };
        self.drenar();
        self.encerrar(MotivoDesconexao::ReconexaoForcada);
        self.porta_desejada = Some(porta);
        if self.varrer().is_ok() {
            self.tentar_reconectar(true);
        }
        self.descarregar_registro();
    }

    /// Processa mensagens recebidas e, quando vence o intervalo, varre as
    /// portas (detecção de desconexão e reconexão). Nunca entra em pânico.
    pub fn atualizar(&mut self) -> Vec<SaidaMidi> {
        self.drenar();
        let vencida = self
            .ultima_varredura_ns
            .is_none_or(|u| self.relogio.agora_ns().saturating_sub(u) >= self.intervalo_ns);
        if vencida && self.varrer().is_ok() {
            let presente = self
                .conexao
                .as_ref()
                .is_some_and(|c| self.portas.contains(&c.porta));
            if self.conexao.is_some() && !presente {
                self.drenar();
                self.encerrar(MotivoDesconexao::PortaAusente);
                self.contadores.desconexoes_detectadas += 1;
                self.perdeu_porta = true;
                self.mudar_estado(EstadoConexao::Desconectado);
            }
            if self.conexao.is_none() && self.porta_desejada.is_some() {
                self.tentar_reconectar(false);
            }
        }
        self.descarregar_registro();
        std::mem::take(&mut self.pendentes)
    }

    fn varrer(&mut self) -> Result<(), ErroFonte> {
        self.ultima_varredura_ns = Some(self.relogio.agora_ns());
        match self.fonte.listar() {
            Ok(p) => {
                self.portas = p;
                self.ultimo_erro_listagem = None;
                Ok(())
            }
            Err(e) => {
                // Falha de listagem não derruba uma conexão viva (ver riscos).
                self.contadores.falhas_de_listagem += 1;
                self.ultimo_erro_listagem = Some(e.to_string());
                Err(e)
            }
        }
    }

    /// Conecta à porta desejada se ela estiver na última listagem.
    /// `explicito`: o usuário acabou de pedir (porta ausente vira `Erro`).
    fn tentar_reconectar(&mut self, explicito: bool) {
        let Some(desejada) = self.porta_desejada.clone() else {
            return;
        };
        let Some(real) = localizar_porta(&self.portas, &desejada).map(str::to_string) else {
            if explicito {
                self.mudar_estado(EstadoConexao::Erro {
                    mensagem: ErroFonte::PortaNaoEncontrada(desejada).to_string(),
                });
            }
            return;
        };
        self.mudar_estado(EstadoConexao::Conectando {
            porta: real.clone(),
        });
        let (tx, rx) = mpsc::channel();
        let callback = Box::new(move |bytes: &[u8], ts_ns: u64| {
            // Receptor fechado = conexão encerrada; a mensagem é descartada.
            let _ = tx.send(Mensagem {
                ts_ns,
                bytes: bytes.to_vec(),
            });
        });
        let ts = self.relogio.agora_ns();
        match self
            .fonte
            .conectar(&real, Arc::clone(&self.relogio), callback)
        {
            Ok(ativa) => {
                let reconexao = self.perdeu_porta;
                self.perdeu_porta = false;
                self.parser.reiniciar();
                self.conexao = Some(Conexao {
                    ativa,
                    porta: real.clone(),
                    rx,
                });
                self.porta_desejada = Some(real.clone());
                self.contadores.conexoes += 1;
                if reconexao {
                    self.contadores.reconexoes += 1;
                }
                if let Some(r) = self.registro.as_mut() {
                    r.registrar_conexao(ts, &real, reconexao);
                }
                self.mudar_estado(EstadoConexao::Conectado { porta: real });
            }
            Err(e) => {
                let mensagem = e.to_string();
                if let Some(r) = self.registro.as_mut() {
                    r.registrar_erro_conexao(ts, &real, &mensagem);
                }
                self.mudar_estado(EstadoConexao::Erro { mensagem });
            }
        }
    }

    /// Fecha a conexão (se houver), processa o que já tinha chegado e emite
    /// `LimparNotas`. Não mexe em `porta_desejada` nem no estado.
    fn encerrar(&mut self, motivo: MotivoDesconexao) {
        let Some(c) = self.conexao.take() else {
            return;
        };
        let Conexao { ativa, porta, rx } = c;
        drop(ativa);
        while let Ok(m) = rx.try_recv() {
            self.processar(m);
        }
        let ts = self.relogio.agora_ns();
        if let Some(r) = self.registro.as_mut() {
            r.registrar_desconexao(ts, &porta, motivo);
        }
        self.parser.reiniciar();
        self.pendentes.push(SaidaMidi::LimparNotas);
    }

    fn drenar(&mut self) {
        let Some(c) = self.conexao.as_ref() else {
            return;
        };
        let lote: Vec<Mensagem> = c.rx.try_iter().collect();
        for m in lote {
            self.processar(m);
        }
    }

    fn processar(&mut self, m: Mensagem) {
        self.contadores.mensagens_recebidas += 1;
        match self.parser.interpretar(&m.bytes, m.ts_ns) {
            Ok(evento) => {
                self.contadores.eventos += 1;
                self.capacidades.registrar(&evento);
                if let Some(r) = self.registro.as_mut() {
                    r.registrar_evento(&evento, m.bytes.first().copied());
                }
                self.pendentes.push(SaidaMidi::Evento(evento));
            }
            Err(erro) => {
                self.contadores.erros_de_interpretacao += 1;
                if let Some(r) = self.registro.as_mut() {
                    r.registrar_erro_midi(m.ts_ns, &erro, m.bytes.len());
                }
            }
        }
    }

    fn mudar_estado(&mut self, novo: EstadoConexao) {
        if self.estado != novo {
            self.estado = novo.clone();
            self.pendentes.push(SaidaMidi::EstadoMudou(novo));
        }
    }

    fn descarregar_registro(&mut self) {
        if let Some(r) = self.registro.as_mut() {
            r.descarregar();
        }
    }
}

fn duracao_ns(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}

/// Aplica as saídas ao estado das teclas, na ordem. Devolve quantas notas
/// ficaram fora da faixa calibrada do teclado (ignoradas, sem pânico).
pub fn aplicar_em_teclas(saidas: &[SaidaMidi], teclas: &mut EstadoTeclas) -> usize {
    let mut fora = 0;
    for s in saidas {
        let r = match s {
            SaidaMidi::Evento(EventoMidi::NotaLigada { nota, .. }) => teclas.ligar(*nota),
            SaidaMidi::Evento(EventoMidi::NotaDesligada { nota, .. }) => teclas.desligar(*nota),
            SaidaMidi::LimparNotas => {
                teclas.limpar_tudo();
                Ok(())
            }
            SaidaMidi::Evento(_) | SaidaMidi::EstadoMudou(_) => Ok(()),
        };
        if r.is_err() {
            fora += 1;
        }
    }
    fora
}
