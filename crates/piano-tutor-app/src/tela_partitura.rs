//! Tela Partitura (F07, somente leitura: ADR-0007): escolhe uma das partituras embutidas (ou
//! um `.musicxml` digitado), mostra em duas pautas (sol/fá) com o componente
//! [`crate::partitura_visual`], toca pelo mesmo motor de som (agendamento antecipado como o
//! ritmo), acompanha a execução com cursor e exporta o MusicXML carregado.
//!
//! O app chama [`EstadoPartitura::atualizar`] a cada quadro (agenda as notas até
//! `agora + ANTECEDENCIA`) e [`EstadoPartitura::tocando`] para saber se há reprodução.

pub use crate::atalhos_partitura::{AcaoAtalho, Destino, MapaAtalhos};
use crate::documento_partitura::Musica;
use crate::documento_partitura::{DocumentoPartitura, destino_de_exportacao};
use crate::geometria::IdNota;
use crate::partitura_visual::{
    Acerto, Destaque, EstadoVisual, Mao, MapaResultados, MarcaSelecao, Marcas, ModoRolagem,
    OpcoesVisuais, ResultadoNota, ZOOM_MAX, ZOOM_MIN, desenhar_partitura,
};
use crate::preferencias::PreferenciasPartitura;
use crate::selecao_partitura::{Comando, EfeitoDoClique, Resultado, SelecaoPartitura, descrever};
use crate::som::{EstadoSom, ServicoSom};
use bancada_casca::Atalho;
use bancada_casca::egui::{self, Button, ComboBox, Event, Key, RichText, Slider, TextEdit, Ui};
use piano_tutor_core::NotaMidi;
use piano_tutor_core::comparador::EventoEsperado;
use piano_tutor_core::documento::ErroServico;
use piano_tutor_core::partitura::{Partitura, SinalClave, TAMANHO_MAXIMO_BYTES};
use piano_tutor_core::pratica::{
    BATIDAS_RETOMADA, ConfigPratica, EstadoEvento, ModoPratica, Pratica, Resumo, Retomada, Veredito,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Antecedência de agendamento (igual à do ritmo).
pub const ANTECEDENCIA: Duration = Duration::from_millis(150);
/// Folga entre apertar Tocar e a primeira nota.
const ATRASO_INICIO: Duration = Duration::from_millis(120);
/// Andamento mínimo e máximo do controle, em semínimas por minuto.
pub const BPM_MIN: f32 = 30.0;
pub const BPM_MAX: f32 = 240.0;
const BPM_PADRAO: f32 = 80.0;
/// Máximo de partituras listadas da pasta de assets.
const MAX_ARQUIVOS_PASTA: usize = 64;

/// Partituras empacotadas no executável (CC0, ver `assets/partituras/LEIA-ME.md`).
pub const EMBUTIDAS: [(&str, &str); 5] = [
    (
        "Brahms: Wiegenlied, Op. 49 nº 4 (3/4)",
        include_str!("../../../assets/partituras/brahms-wiegenlied-op49-4.musicxml"),
    ),
    (
        "Schubert: Heidenröslein, D. 257 (2/4)",
        include_str!("../../../assets/partituras/schubert-heidenroeslein-d257.musicxml"),
    ),
    (
        "Fauré: Barcarolle, Op. 7 nº 3 (6/8)",
        include_str!("../../../assets/partituras/faure-barcarolle-op7-3.musicxml"),
    ),
    (
        "Beethoven: Ode à Alegria, tema (4/4)",
        include_str!("../../../assets/partituras/beethoven-ode-a-alegria-tema.musicxml"),
    ),
    (
        "Tradicional: Brilha, brilha, estrelinha (4/4, Sol maior)",
        include_str!("../../../assets/partituras/tradicional-brilha-brilha-estrelinha.musicxml"),
    ),
];

// --- Plano de reprodução (puro) ------------------------------------------------------------

/// Uma nota a tocar: instante e duração no relógio comum, em nanossegundos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotaAgendada {
    pub t_ns: u64,
    pub dur_ns: u64,
    pub midi: u8,
}

fn para_ns(segundos: f64) -> u64 {
    if segundos.is_finite() && segundos > 0.0 {
        (segundos * 1e9).round() as u64
    } else {
        0
    }
}

/// Notas que começam em `q0` ou depois, com `q0` tocando em `t0_ns`, ordenadas por instante.
pub fn plano_de_reproducao(m: &Musica, bpm_alvo: f64, q0: f64, t0_ns: u64) -> Vec<NotaAgendada> {
    let s0 = m.segundos(q0, bpm_alvo);
    let mut v: Vec<NotaAgendada> = m
        .eventos
        .iter()
        .filter(|e| e.inicio.como_f64() >= q0 - 1e-9)
        .map(|e| {
            let qi = e.inicio.como_f64();
            let qf = qi + e.duracao.como_f64();
            let (si, sf) = (m.segundos(qi, bpm_alvo), m.segundos(qf, bpm_alvo));
            // Corta 4% para as repetições de uma mesma nota soarem separadas.
            let dur = ((sf - si) * 0.96).max(0.04);
            NotaAgendada {
                t_ns: t0_ns.saturating_add(para_ns(si - s0)),
                dur_ns: para_ns(dur),
                midi: e.midi,
            }
        })
        .collect();
    v.sort_by_key(|n| (n.t_ns, n.midi));
    v
}

// --- Estado --------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Fonte {
    nome: String,
    /// `None` = embutida; `Some` = arquivo da pasta de assets.
    caminho: Option<PathBuf>,
    indice_embutida: usize,
}

struct Reproducao {
    notas: Vec<NotaAgendada>,
    proxima: usize,
    /// Instante do relógio comum em que `q0` toca, e `q0` em semínimas.
    t0_ns: u64,
    q0: f64,
    bpm: f64,
    fim_ns: u64,
}

/// Contagem de entrada dos modos com andamento (Espera tocar e Seguir o andamento), em
/// semínimas (antes do primeiro compasso tocado).
const BATIDAS_DE_ENTRADA: f64 = 2.0;
/// Tolerância inicial dos modos com andamento, em ms.
pub const TOLERANCIA_PRATICA_PADRAO_MS: f32 = 150.0;
pub const TOLERANCIA_PRATICA_MIN_MS: f32 = 30.0;
pub const TOLERANCIA_PRATICA_MAX_MS: f32 = 500.0;

/// Onde cada nota esperada está na partitura (para pintar e posicionar o cursor).
#[derive(Clone, Copy, Debug)]
struct PosicaoNota {
    compasso: usize,
    /// Início dentro do compasso, em semínimas.
    tempo: f64,
    mao: Mao,
}

/// Uma prática em andamento (ou terminada, com o resumo na tela até a próxima).
struct SessaoPratica {
    logica: Pratica,
    posicoes: Vec<PosicaoNota>,
    /// Instante do relógio comum que corresponde a `q0` com a contagem de entrada já descontada.
    t0_ns: u64,
    /// Espera tocar: segundos somados ao tempo da prática depois de cada retomada (o relógio
    /// volta no tempo para contar os dois tempos antes de recomeçar). Zero até a primeira.
    deslocamento_s: f64,
    /// Tempo da prática (s) abaixo do qual o cursor não anda: zero, ou o ponto de onde a última
    /// retomada recomeça. Enquanto o tempo da prática está abaixo dele, é a contagem.
    piso_s: f64,
    /// Relógio dos modos com andamento, só para converter instante em posição (`notas` fica
    /// vazio).
    relogio: Reproducao,
    mapa: MapaResultados,
    sujo: bool,
    resumo_final: Option<Resumo>,
}

impl SessaoPratica {
    /// Tempo da prática (s, negativo na contagem de entrada) no instante `instante_ns`.
    fn segundos(&self, instante_ns: u64) -> f64 {
        (instante_ns as f64 - self.t0_ns as f64) / 1e9 + self.deslocamento_s
    }

    /// Espera tocar: se o aluno completou o passo parado, volta o relógio para contar
    /// [`BATIDAS_RETOMADA`] tempos e chegar ao ponto de retomada no andamento da prática.
    fn aplicar_retomada(&mut self, agora_ns: u64) {
        let Some(t_alvo) = self.logica.tomar_retomada() else {
            return;
        };
        let contagem_s = BATIDAS_RETOMADA * 60.0 / self.relogio.bpm;
        let desejado = t_alvo - contagem_s;
        let sem_deslocamento = (agora_ns as f64 - self.t0_ns as f64) / 1e9;
        self.deslocamento_s = desejado - sem_deslocamento;
        self.piso_s = t_alvo;
        self.sujo = true;
    }

    /// Batidas que ainda faltam na contagem (entrada ou retomada) no instante `agora_ns`.
    fn batidas_de_contagem(&self, agora_ns: u64) -> Option<u32> {
        if !self.logica.config().modo.tem_andamento() || self.logica.parada().is_some() {
            return None;
        }
        let falta_s = self.piso_s - self.segundos(agora_ns);
        if falta_s <= 1e-9 {
            return None;
        }
        Some((falta_s * self.relogio.bpm / 60.0 - 1e-6).ceil().max(1.0) as u32)
    }

    fn reconstruir_mapa(&mut self) {
        let mut mapa = MapaResultados::default();
        // Espera tocar parado: as notas que faltam já aparecem como falta (vermelhas).
        let da_parada = self.logica.notas_da_parada();
        for (i, (pos, est)) in self.posicoes.iter().zip(self.logica.estados()).enumerate() {
            let r = match est {
                _ if da_parada.contains(&i) => Some(ResultadoNota::Errou),
                EstadoEvento::Pendente => None,
                EstadoEvento::Acertou => Some(ResultadoNota::Acertou),
                EstadoEvento::Errou => Some(ResultadoNota::Errou),
            };
            mapa.registrar(pos.compasso, pos.tempo, pos.mao, r);
        }
        self.mapa = mapa;
        self.sujo = false;
    }

    /// Fecha a sessão quando a lógica terminou.
    fn conferir_fim(&mut self) {
        if self.resumo_final.is_none() && self.logica.concluida() {
            self.resumo_final = Some(self.logica.resumo());
            self.sujo = true;
        }
    }
}

/// Mão (clave de fá = esquerda) de cada nota esperada, pela clave vigente na pauta.
fn mao_pela_clave(p: &Partitura, parte: usize, pauta: u8, compasso: usize, tempo: f64) -> Mao {
    let mut sinal = None;
    if let Some(parte) = p.partes.get(parte) {
        for (c, comp) in parte.compassos.iter().enumerate().take(compasso + 1) {
            for m in comp.claves.iter().filter(|m| m.pauta == pauta) {
                if c < compasso || m.posicao.como_f64() <= tempo + 1e-9 {
                    sinal = Some(m.clave.sinal);
                }
            }
        }
    }
    if sinal == Some(SinalClave::Fa) {
        Mao::Esquerda
    } else {
        Mao::Direita
    }
}

/// Estado da tela Partitura.
pub struct EstadoPartitura {
    fontes: Vec<Fonte>,
    selecionada: usize,
    campo_caminho: String,
    /// Documento carregado (texto XML, modelo e dados musicais), fora da interface.
    documento: Option<DocumentoPartitura>,
    pub erro: Option<String>,
    pub aviso: Option<String>,
    pub zoom: f32,
    pub bpm: f32,
    pub seguir: bool,
    /// Como a vista acompanha a execução (sistema a sistema ou fita contínua).
    pub modo_rolagem: ModoRolagem,
    /// Papel branco com tinta escura mesmo no tema Noite.
    pub papel_branco: bool,
    /// Seleção de nota em alto contraste (preferência do perfil).
    pub alto_contraste: bool,
    /// Nota selecionada e navegação por teclado (ticket 12): fora do documento e do egui.
    selecao: SelecaoPartitura,
    /// Mapa de atalhos da Oficina (ticket 13), gravado no perfil (`tela.atalhos`).
    pub atalhos: MapaAtalhos,
    /// Ação cujo atalho está sendo capturado na lista de atalhos (a próxima tecla vale).
    capturando_atalho: Option<AcaoAtalho>,
    /// Último aviso ou recusa de atalho (ação pendente, conflito, combinação recusada).
    aviso_atalho: Option<String>,
    /// Identidade do widget da vista no último quadro (foco do teclado; testes).
    pub(crate) id_da_vista: Option<egui::Id>,
    /// Arquivo onde as preferências de exibição são gravadas (sem pasta de dados, não grava).
    /// Últimas preferências entregues ao app (ele as grava no perfil ativo).
    preferencias_gravadas: PreferenciasPartitura,
    /// Preferências mudadas esperando o app gravar ([`EstadoPartitura::tomar_preferencias_mudadas`]).
    preferencias_mudadas: Option<PreferenciasPartitura>,
    visual: EstadoVisual,
    compasso_inicial: Option<usize>,
    reproducao: Option<Reproducao>,
    campo_exportar: String,
    sobrescrever: bool,
    /// Modo Praticar: como a prática avança.
    pub modo_pratica: ModoPratica,
    /// Janela de acerto do modo Tempo, em ms.
    pub tolerancia_pratica_ms: f32,
    /// Espera tocar: quanto voltar ao retomar depois de uma parada.
    pub retomada_pratica: Retomada,
    pratica: Option<SessaoPratica>,
}

impl EstadoPartitura {
    /// Cria o estado com as partituras embutidas e, se existir, as da pasta de assets
    /// (`<pasta>/partituras/*.musicxml`). A primeira é carregada.
    pub fn novo(pasta_assets: Option<PathBuf>) -> Self {
        let mut fontes: Vec<Fonte> = EMBUTIDAS
            .iter()
            .enumerate()
            .map(|(i, (nome, _))| Fonte {
                nome: (*nome).to_owned(),
                caminho: None,
                indice_embutida: i,
            })
            .collect();
        // As preferências vêm do perfil ativo (o app chama `definir_preferencias`).
        let prefs = PreferenciasPartitura::default();
        if let Some(pasta) = pasta_assets {
            let alvo = if pasta.join("partituras").is_dir() {
                pasta.join("partituras")
            } else {
                pasta
            };
            if let Ok(rd) = std::fs::read_dir(&alvo) {
                let mut achados: Vec<PathBuf> = rd
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| {
                        p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                            matches!(e.to_lowercase().as_str(), "musicxml" | "xml" | "mxl")
                        })
                    })
                    .collect();
                achados.sort();
                for p in achados.into_iter().take(MAX_ARQUIVOS_PASTA) {
                    let nome = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    // Já embutida com o mesmo arquivo? Não duplica.
                    let repetida = EMBUTIDAS.iter().any(|(_, t)| {
                        std::fs::metadata(&p).is_ok_and(|m| m.len() == t.len() as u64)
                    });
                    if !repetida {
                        fontes.push(Fonte {
                            nome: format!("{nome} (pasta de assets)"),
                            caminho: Some(p),
                            indice_embutida: 0,
                        });
                    }
                }
            }
        }
        let mut e = Self {
            fontes,
            selecionada: 0,
            campo_caminho: String::new(),
            documento: None,
            erro: None,
            aviso: None,
            zoom: prefs.zoom,
            bpm: BPM_PADRAO,
            seguir: prefs.seguir,
            modo_rolagem: prefs.modo,
            papel_branco: prefs.papel_branco,
            alto_contraste: prefs.alto_contraste,
            selecao: SelecaoPartitura::default(),
            atalhos: MapaAtalhos::padrao(),
            capturando_atalho: None,
            aviso_atalho: None,
            id_da_vista: None,
            preferencias_gravadas: prefs,
            preferencias_mudadas: None,
            visual: EstadoVisual::default(),
            compasso_inicial: None,
            reproducao: None,
            campo_exportar: String::new(),
            sobrescrever: false,
            modo_pratica: ModoPratica::NotaANota,
            tolerancia_pratica_ms: TOLERANCIA_PRATICA_PADRAO_MS,
            retomada_pratica: Retomada::Zero,
            pratica: None,
        };
        e.escolher(0);
        e
    }

    /// Há reprodução em andamento.
    pub fn tocando(&self) -> bool {
        self.reproducao.is_some()
    }

    pub fn partitura(&self) -> Option<&Partitura> {
        self.documento.as_ref().map(DocumentoPartitura::partitura)
    }

    fn musica(&self) -> Option<&Musica> {
        self.documento.as_ref().map(|d| &d.musica)
    }

    pub fn nome(&self) -> &str {
        self.documento.as_ref().map_or("", |d| d.nome.as_str())
    }

    fn carregar_texto(&mut self, nome: String, texto: String) -> Result<(), ErroServico> {
        let d = DocumentoPartitura::de_texto(nome, texto)?;
        self.carregar_documento(d);
        Ok(())
    }

    /// Passa a mostrar um documento já aberto pelo serviço (texto, arquivo ou `.mxl`).
    fn carregar_documento(&mut self, d: DocumentoPartitura) {
        if let Some(b) = d.musica.bpm_arquivo {
            self.bpm = (b as f32).clamp(BPM_MIN, BPM_MAX);
        }
        self.campo_exportar = format!("{}.musicxml", nome_de_arquivo(&d.nome));
        self.documento = Some(d);
        self.compasso_inicial = None;
        // Outro documento, outras identidades de nota: a seleção anterior não vale mais.
        self.selecao.limpar();
        self.pratica = None;
        self.visual.invalidar();
    }

    /// Nota selecionada (por identidade), se houver.
    pub fn nota_selecionada(&self) -> Option<IdNota> {
        self.selecao.nota()
    }

    /// Estado da seleção (nota e último aviso).
    pub fn selecao(&self) -> &SelecaoPartitura {
        &self.selecao
    }

    /// Quantas vozes ou pautas com notas há para o `Tab` alternar.
    pub fn trilhas_de_leitura(&self) -> usize {
        self.documento
            .as_ref()
            .map_or(0, |d| d.leitura.trilhas().len())
    }

    /// Remapeia a ação para `atalho` (vale já) e guarda a mensagem de recusa ou de conflito.
    pub fn remapear_atalho(&mut self, acao: AcaoAtalho, atalho: Atalho) {
        self.capturando_atalho = None;
        self.aviso_atalho = Some(match self.atalhos.remapear(acao, atalho) {
            Ok(None) => format!("«{}» agora é {atalho}.", acao.rotulo()),
            Ok(Some(aviso)) => format!("«{}» agora é {atalho}. Atenção: {aviso}", acao.rotulo()),
            Err(recusa) => recusa.to_string(),
        });
    }

    /// Avisa que a ação do atalho apertado ainda não tem comando (fatias seguintes).
    pub fn avisar_pendente(&mut self, acao: AcaoAtalho) {
        let atalho = self
            .atalhos
            .atalho(acao)
            .map_or_else(String::new, |a| format!(" ({a})"));
        self.aviso_atalho = Some(format!(
            "«{}»{atalho} ainda não tem comando: a edição chega nas próximas etapas.",
            acao.rotulo()
        ));
    }

    /// Último aviso ou recusa de atalho, para a linha de estado e os testes.
    pub fn aviso_de_atalho(&self) -> Option<&str> {
        self.aviso_atalho.as_deref()
    }

    /// Executa um comando de navegação sobre a partitura aberta. Não edita nem suja o documento.
    pub fn navegar(&mut self, cmd: Comando) -> Option<Resultado> {
        let d = self.documento.as_ref()?;
        Some(self.selecao.executar(&d.leitura, cmd))
    }

    /// Traduz o acerto de um clique: nota seleciona a nota; fora de nota, alterna o compasso de
    /// partida (comportamento de sempre) e mantém a nota selecionada; fora de tudo, nada muda.
    pub fn clicar_na_partitura(&mut self, acerto: Option<Acerto>) -> EfeitoDoClique {
        let Some(d) = self.documento.as_ref() else {
            return EfeitoDoClique::Nenhum;
        };
        let efeito = self.selecao.clicar(&d.leitura, acerto);
        if let EfeitoDoClique::Compasso(c) = efeito {
            self.compasso_inicial = if self.compasso_inicial == Some(c) {
                None
            } else {
                Some(c)
            };
        }
        efeito
    }

    /// Descrição em texto da nota selecionada (altura, compasso, pauta e voz).
    pub fn descricao_da_selecao(&self) -> Option<String> {
        let d = self.documento.as_ref()?;
        descrever(d.partitura(), &d.leitura, self.selecao.nota()?)
    }

    fn preferencias_visuais(&self) -> PreferenciasPartitura {
        PreferenciasPartitura {
            modo: self.modo_rolagem,
            papel_branco: self.papel_branco,
            seguir: self.seguir,
            zoom: self.zoom,
            alto_contraste: self.alto_contraste,
        }
    }

    /// Marca as preferências de exibição para gravar quando mudam.
    fn salvar_preferencias(&mut self) {
        let atual = self.preferencias_visuais();
        if atual == self.preferencias_gravadas {
            return;
        }
        self.preferencias_mudadas = Some(atual);
        self.preferencias_gravadas = atual;
    }

    /// Preferências que mudaram desde a última chamada, para o app gravar no perfil.
    pub fn tomar_preferencias_mudadas(&mut self) -> Option<PreferenciasPartitura> {
        self.preferencias_mudadas.take()
    }

    /// Aplica as preferências do perfil (ao abrir e ao trocar de perfil), sem marcar mudança.
    pub fn definir_preferencias(&mut self, p: PreferenciasPartitura) {
        self.zoom = p.zoom;
        self.seguir = p.seguir;
        self.modo_rolagem = p.modo;
        self.papel_branco = p.papel_branco;
        self.alto_contraste = p.alto_contraste;
        self.preferencias_gravadas = p;
        self.preferencias_mudadas = None;
        self.visual.invalidar();
    }

    /// Escolhe uma fonte da lista (para a reprodução, se houver).
    pub fn escolher(&mut self, i: usize) {
        let Some(f) = self.fontes.get(i).cloned() else {
            return;
        };
        self.parar_sem_som();
        let aberto = match &f.caminho {
            None => DocumentoPartitura::de_texto(
                f.nome.clone(),
                EMBUTIDAS[f.indice_embutida].1.to_owned(),
            )
            .map_err(|e| e.to_string()),
            Some(c) => DocumentoPartitura::de_arquivo(c, &f.nome),
        };
        match aberto {
            Ok(d) => {
                self.carregar_documento(d);
                self.selecionada = i;
                self.erro = None;
            }
            Err(e) => self.erro = Some(format!("{}: {e}", f.nome)),
        }
    }

    /// Abre um arquivo digitado (`.musicxml`, `.xml` ou `.mxl`).
    pub fn abrir_caminho(&mut self, caminho: &str) {
        let c = Path::new(caminho.trim());
        self.parar_sem_som();
        match DocumentoPartitura::de_arquivo(c, caminho) {
            Ok(d) => {
                self.carregar_documento(d);
                self.erro = None;
            }
            Err(e) => self.erro = Some(e),
        }
    }

    fn parar_sem_som(&mut self) {
        self.reproducao = None;
    }

    /// Começa a tocar a partir do compasso escolhido (ou do início).
    pub fn tocar(&mut self, som: &mut ServicoSom) {
        let Some(m) = self.musica() else {
            return;
        };
        let q0 = self
            .compasso_inicial
            .and_then(|c| m.inicio_do_compasso(c))
            .unwrap_or(0.0);
        let t0 = som
            .agora_ns()
            .saturating_add(para_ns(ATRASO_INICIO.as_secs_f64()));
        let bpm = f64::from(self.bpm);
        let notas = plano_de_reproducao(m, bpm, q0, t0);
        let fim_ns = t0.saturating_add(para_ns(
            m.segundos(m.duracao_q, bpm) - m.segundos(q0, bpm) + 0.5,
        ));
        som.panico();
        self.pratica = None;
        self.reproducao = Some(Reproducao {
            notas,
            proxima: 0,
            t0_ns: t0,
            q0,
            bpm,
            fim_ns,
        });
        self.aviso = if som.estado() == EstadoSom::Pronto {
            None
        } else {
            Some(
                "O som ainda não está pronto: o cursor anda, mas não há áudio (aba Timbres)."
                    .into(),
            )
        };
    }

    /// Para a reprodução e corta o que está soando.
    pub fn parar(&mut self, som: &mut ServicoSom) {
        if self.reproducao.take().is_some() {
            som.panico();
        }
    }

    /// Muda o andamento; se estiver tocando, continua do ponto ainda não agendado.
    pub fn definir_bpm(&mut self, bpm: f32) {
        self.bpm = bpm.clamp(BPM_MIN, BPM_MAX);
        let (Some(r), Some(m)) = (
            self.reproducao.as_mut(),
            self.documento.as_ref().map(|d| &d.musica),
        ) else {
            return;
        };
        let t_h = r
            .notas
            .get(r.proxima)
            .map_or(r.fim_ns, |n| n.t_ns)
            .max(r.t0_ns);
        // Ponto (semínimas) de onde seguir: o instante do próximo evento ainda não enviado.
        let q_h = q_em_ns(m, r, t_h);
        let bpm = f64::from(self.bpm);
        let notas = plano_de_reproducao(m, bpm, q_h, t_h);
        let fim_ns = t_h.saturating_add(para_ns(
            m.segundos(m.duracao_q, bpm) - m.segundos(q_h, bpm) + 0.5,
        ));
        *r = Reproducao {
            notas,
            proxima: 0,
            t0_ns: t_h,
            q0: q_h,
            bpm,
            fim_ns,
        };
    }

    /// Chamar a cada quadro: agenda o que falta até `agora + ANTECEDENCIA`.
    pub fn atualizar(&mut self, som: &mut ServicoSom) {
        let Some(r) = self.reproducao.as_mut() else {
            return;
        };
        let agora = som.agora_ns();
        let horizonte = agora.saturating_add(para_ns(ANTECEDENCIA.as_secs_f64()));
        while let Some(n) = r.notas.get(r.proxima).copied() {
            if n.t_ns > horizonte {
                break;
            }
            if let Some(nota) = NotaMidi::nova(n.midi) {
                som.demonstrar(&[nota], n.t_ns, Duration::from_nanos(n.dur_ns));
            }
            r.proxima += 1;
        }
        if r.proxima >= r.notas.len() && agora > r.fim_ns {
            self.reproducao = None;
        }
    }

    /// Posição atual em semínimas na execução, se estiver tocando.
    fn posicao_q(&self, agora_ns: u64) -> Option<f64> {
        let (r, m) = (self.reproducao.as_ref()?, self.musica()?);
        Some(q_em_ns(m, r, agora_ns))
    }

    /// Cursor da execução (compasso e tempo dentro dele), se estiver tocando.
    pub fn destaque(&self, agora_ns: u64) -> Option<Destaque> {
        if let Some(s) = self.pratica.as_ref() {
            if s.resumo_final.is_some() {
                return None;
            }
            return match s.logica.config().modo {
                ModoPratica::NotaANota => {
                    let pos = s.posicoes.get(s.logica.evento_do_passo_atual()?)?;
                    Some(Destaque {
                        compasso: pos.compasso,
                        tempo: pos.tempo,
                    })
                }
                ModoPratica::Tempo | ModoPratica::EsperaTocar => {
                    // Parado (Espera tocar): o cursor fica na nota que falta.
                    if let Some(&i) = s.logica.notas_da_parada().first() {
                        let pos = s.posicoes.get(i)?;
                        return Some(Destaque {
                            compasso: pos.compasso,
                            tempo: pos.tempo,
                        });
                    }
                    let t = s.segundos(agora_ns).max(s.piso_s);
                    let m = self.musica()?;
                    let q = q_em_s(m, &s.relogio, t);
                    let (compasso, tempo) = m.compasso_em(q)?;
                    Some(Destaque { compasso, tempo })
                }
            };
        }
        let q = self.posicao_q(agora_ns)?;
        let (compasso, tempo) = self.musica()?.compasso_em(q)?;
        Some(Destaque { compasso, tempo })
    }

    /// Há prática em andamento (ainda não terminou).
    pub fn praticando(&self) -> bool {
        self.pratica
            .as_ref()
            .is_some_and(|s| s.resumo_final.is_none())
    }

    /// Modo da prática em andamento ou do último resumo, se houver.
    pub fn modo_da_pratica(&self) -> Option<ModoPratica> {
        Some(self.pratica.as_ref()?.logica.config().modo)
    }

    /// Espera tocar: a prática está parada esperando a nota que falta.
    pub fn pratica_parada(&self) -> bool {
        self.praticando()
            && self
                .pratica
                .as_ref()
                .is_some_and(|s| s.logica.parada().is_some())
    }

    /// Indicação curta do que a prática faz agora: "Parado: toque a nota que falta" (Espera
    /// tocar) ou "Contagem: N" (entrada ou retomada, N = batidas que faltam).
    pub fn aviso_da_pratica(&self, agora_ns: u64) -> Option<String> {
        let s = self.pratica.as_ref()?;
        if s.resumo_final.is_some() {
            return None;
        }
        if s.logica.parada().is_some() {
            let n = s.logica.notas_da_parada().len();
            return Some(if n > 1 {
                "Parado: toque as notas que faltam".into()
            } else {
                "Parado: toque a nota que falta".into()
            });
        }
        s.batidas_de_contagem(agora_ns)
            .map(|n| format!("Contagem: {n}"))
    }

    /// Resumo da última prática (terminada ou parcial), se houver.
    pub fn resumo_pratica(&self) -> Option<Resumo> {
        let s = self.pratica.as_ref()?;
        Some(s.resumo_final.unwrap_or_else(|| s.logica.resumo()))
    }

    /// Começa a praticar a partir do compasso escolhido (ou do início), no modo e na tolerância
    /// configurados. `agora_ns` é o relógio comum (`ServicoSom::agora_ns`). Para a reprodução.
    /// Nos modos com andamento (Espera tocar e Seguir o andamento) há uma contagem de entrada de
    /// duas semínimas antes do primeiro compasso.
    pub fn iniciar_pratica(&mut self, agora_ns: u64) {
        let (Some(m), Some(p)) = (self.musica(), self.partitura()) else {
            return;
        };
        let q0 = self
            .compasso_inicial
            .and_then(|c| m.inicio_do_compasso(c))
            .unwrap_or(0.0);
        let bpm = f64::from(self.bpm);
        let s0 = m.segundos(q0, bpm);
        let mut esperados = Vec::new();
        let mut posicoes = Vec::new();
        for e in m
            .eventos
            .iter()
            .filter(|e| e.inicio.como_f64() >= q0 - 1e-9)
        {
            let qi = e.inicio.como_f64();
            let Some((compasso, tempo)) = m.compasso_em(qi) else {
                continue;
            };
            let si = m.segundos(qi, bpm) - s0;
            esperados.push(EventoEsperado {
                midi: e.midi,
                inicio_s: si,
                duracao_s: m.segundos(qi + e.duracao.como_f64(), bpm) - s0 - si,
                parte: e.parte,
                pauta: e.pauta,
                compasso: e.compasso,
            });
            posicoes.push(PosicaoNota {
                compasso,
                tempo,
                mao: mao_pela_clave(p, e.parte, e.pauta, compasso, tempo),
            });
        }
        let config = ConfigPratica {
            modo: self.modo_pratica,
            tolerancia_ms: f64::from(self.tolerancia_pratica_ms),
            retomada: self.retomada_pratica,
        };
        let entrada_s = match config.modo {
            ModoPratica::Tempo | ModoPratica::EsperaTocar => BATIDAS_DE_ENTRADA * 60.0 / bpm,
            ModoPratica::NotaANota => 0.0,
        };
        let t0_ns = agora_ns.saturating_add(para_ns(entrada_s));
        let mut s = SessaoPratica {
            logica: Pratica::nova(&esperados, config),
            posicoes,
            t0_ns,
            deslocamento_s: 0.0,
            piso_s: 0.0,
            relogio: Reproducao {
                notas: Vec::new(),
                proxima: 0,
                t0_ns,
                q0,
                bpm,
                fim_ns: 0,
            },
            mapa: MapaResultados::default(),
            sujo: true,
            resumo_final: None,
        };
        s.conferir_fim();
        self.reproducao = None;
        self.aviso = None;
        self.pratica = Some(s);
    }

    /// Encerra a prática: o que não foi tocado conta como falta e o resumo fica na tela.
    pub fn parar_pratica(&mut self) {
        if let Some(s) = self.pratica.as_mut()
            && s.resumo_final.is_none()
        {
            s.logica.encerrar();
            s.conferir_fim();
            s.sujo = true;
        }
    }

    /// Tira a prática e o resumo da tela.
    pub fn limpar_pratica(&mut self) {
        self.pratica = None;
    }

    /// Entrada de nota para o modo Praticar: `ligada` = note-on (note-off é ignorado),
    /// `instante_ns` no relógio comum (`ServicoSom::agora_ns`). Sem prática em andamento não faz
    /// nada. Serve ao teclado físico e à simulação.
    pub fn nota_tocada(&mut self, ligada: bool, nota: NotaMidi, instante_ns: u64) {
        if !ligada {
            return;
        }
        let Some(s) = self.pratica.as_mut() else {
            return;
        };
        if s.resumo_final.is_some() {
            return;
        }
        // Parado (Espera tocar), a nota vale no instante congelado.
        let t = s.logica.parada().unwrap_or_else(|| s.segundos(instante_ns));
        if s.logica.nota_tocada(nota.numero(), t) != Veredito::Ignorada {
            s.sujo = true;
        }
        s.aplicar_retomada(instante_ns);
        s.conferir_fim();
    }

    /// Chamar a cada quadro: faz o relógio dos modos com andamento andar (esperadas sem nota
    /// viram falta ao fim da janela, ou param a prática no Espera tocar) e fecha a prática
    /// quando tudo foi resolvido.
    pub fn atualizar_pratica(&mut self, agora_ns: u64) {
        let Some(s) = self.pratica.as_mut() else {
            return;
        };
        if s.resumo_final.is_none() {
            let t = s.segundos(agora_ns);
            if s.logica.avancar_tempo(t) {
                s.sujo = true;
            }
            s.aplicar_retomada(agora_ns);
            s.conferir_fim();
        }
        if s.sujo {
            s.reconstruir_mapa();
        }
    }

    /// Abre um MusicXML que já está na memória (Exercício de Curso). Para a reprodução e a
    /// prática; em caso de erro a partitura anterior continua e o texto vai para `erro`.
    pub fn abrir_xml(&mut self, nome: &str, xml: String) -> Result<(), String> {
        self.parar_sem_som();
        let r = if xml.len() > TAMANHO_MAXIMO_BYTES {
            Err(format!(
                "partitura grande demais ({} bytes; máximo {TAMANHO_MAXIMO_BYTES})",
                xml.len()
            ))
        } else {
            self.carregar_texto(nome.to_owned(), xml)
                .map_err(|e| e.to_string())
        };
        match r {
            Ok(()) => {
                self.erro = None;
                Ok(())
            }
            Err(e) => {
                let m = format!("{nome}: {e}");
                self.erro = Some(m.clone());
                Err(m)
            }
        }
    }

    /// Quantas notas a partitura tem (a ordem é a de `esperados_da_partitura`, a mesma do
    /// `indice` do Alinhamento do Pacote de Curso).
    pub fn total_de_notas(&self) -> usize {
        self.musica().map_or(0, |m| m.eventos.len())
    }

    /// Índice da nota que a prática espera agora (na ordem do Alinhamento), se houver prática
    /// em andamento. No modo Nota a nota é a nota do passo atual; nos modos com andamento, a
    /// primeira ainda pendente (no Espera tocar parado, a que falta).
    pub fn indice_da_pratica(&self) -> Option<usize> {
        let s = self.pratica.as_ref()?;
        if s.resumo_final.is_some() {
            return None;
        }
        let local = match s.logica.config().modo {
            ModoPratica::NotaANota => s.logica.evento_do_passo_atual()?,
            ModoPratica::Tempo | ModoPratica::EsperaTocar => s
                .logica
                .estados()
                .iter()
                .position(|e| *e == EstadoEvento::Pendente)?,
        };
        // A prática pode começar num compasso escolhido: as notas anteriores ficam de fora.
        let fora = self.total_de_notas().saturating_sub(s.posicoes.len());
        Some(fora + local)
    }

    /// Resumo da prática que terminou (não o parcial de uma em andamento).
    pub fn resumo_final_da_pratica(&self) -> Option<Resumo> {
        self.pratica.as_ref()?.resumo_final
    }

    /// Onde o cursor fica quando a nota `indice` soa (compasso e tempo dentro dele).
    pub fn destaque_da_nota(&self, indice: usize) -> Option<Destaque> {
        let m = self.musica()?;
        let e = m.eventos.get(indice)?;
        let (compasso, tempo) = m.compasso_em(e.inicio.como_f64())?;
        Some(Destaque { compasso, tempo })
    }

    /// Notas esperadas no andamento atual, para o modo Chamada e resposta. `mao` filtra pela
    /// clave (fá = esquerda); `None` leva as duas mãos.
    pub fn esperados_para_chamada(&self, mao: Option<Mao>) -> Vec<EventoEsperado> {
        let (Some(m), Some(p)) = (self.musica(), self.partitura()) else {
            return Vec::new();
        };
        let bpm = f64::from(self.bpm);
        m.eventos
            .iter()
            .filter_map(|e| {
                let qi = e.inicio.como_f64();
                let (compasso, tempo) = m.compasso_em(qi)?;
                if let Some(quer) = mao
                    && mao_pela_clave(p, e.parte, e.pauta, compasso, tempo) != quer
                {
                    return None;
                }
                let si = m.segundos(qi, bpm);
                Some(EventoEsperado {
                    midi: e.midi,
                    inicio_s: si,
                    duracao_s: m.segundos(qi + e.duracao.como_f64(), bpm) - si,
                    parte: e.parte,
                    pauta: e.pauta,
                    compasso: e.compasso,
                })
            })
            .collect()
    }

    /// Grava o MusicXML carregado, sem alterar nada, e confere relendo o arquivo.
    pub fn exportar(&mut self, caminho: &str) -> Result<PathBuf, String> {
        let c = destino_de_exportacao(caminho)?;
        let d = self
            .documento
            .as_ref()
            .ok_or_else(|| "nenhuma partitura carregada".to_owned())?;
        d.gravar(&c, self.sobrescrever)?;
        Ok(c)
    }
}

fn nome_de_arquivo(nome: &str) -> String {
    let base: String = nome
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let base = base.trim_matches('-').to_lowercase();
    if base.is_empty() {
        "partitura".into()
    } else {
        base.chars().take(48).collect()
    }
}

/// Semínimas na execução no instante `t_ns` do relógio comum (inverso do mapa de andamento).
fn q_em_ns(m: &Musica, r: &Reproducao, t_ns: u64) -> f64 {
    q_em_s(m, r, t_ns.saturating_sub(r.t0_ns) as f64 / 1e9)
}

/// Semínimas na execução `t_s` segundos depois de `r.q0` (zero ou negativo = `q0`).
fn q_em_s(m: &Musica, r: &Reproducao, t_s: f64) -> f64 {
    if t_s <= 0.0 {
        return r.q0;
    }
    let alvo = t_s + m.segundos(r.q0, r.bpm);
    let (mut lo, mut hi) = (r.q0, m.duracao_q.max(r.q0));
    if m.segundos(hi, r.bpm) <= alvo {
        return hi;
    }
    for _ in 0..48 {
        let meio = (lo + hi) / 2.0;
        if m.segundos(meio, r.bpm) < alvo {
            lo = meio;
        } else {
            hi = meio;
        }
    }
    (lo + hi) / 2.0
}

// --- Tela ----------------------------------------------------------------------------------

/// O que o corpo da tela mostra. A tela Partitura usa [`OpcoesTela::COMPLETA`]; o Exercício de
/// Curso esconde a escolha de arquivo e a exportação e pode mandar o cursor de fora (o áudio
/// do professor).
#[derive(Clone, Copy, Debug)]
pub struct OpcoesTela {
    pub com_titulo: bool,
    pub com_origem: bool,
    pub com_exportacao: bool,
    /// Seleção de nota e navegação por teclado (só a tela Partitura; a prática embutida do
    /// Exercício de Curso fica sem ela).
    pub com_selecao: bool,
    /// Se houver, manda no cursor (por exemplo, o do áudio do professor).
    pub cursor_externo: Option<Destaque>,
}

impl OpcoesTela {
    pub const COMPLETA: Self = Self {
        com_titulo: true,
        com_origem: true,
        com_exportacao: true,
        com_selecao: true,
        cursor_externo: None,
    };
}

/// Desenha a tela inteira.
pub fn desenhar_tela(ui: &mut Ui, e: &mut EstadoPartitura, som: &mut ServicoSom) {
    desenhar_corpo(ui, e, som, OpcoesTela::COMPLETA);
}

/// Desenha transporte, prática e partitura, com as partes que `opcoes` pedir.
pub fn desenhar_corpo(
    ui: &mut Ui,
    e: &mut EstadoPartitura,
    som: &mut ServicoSom,
    opcoes: OpcoesTela,
) {
    if opcoes.com_titulo {
        ui.heading("Partitura");
    }
    if opcoes.com_origem {
        desenhar_origem(ui, e);
    }
    e.atualizar_pratica(som.agora_ns());
    desenhar_transporte(ui, e, som);
    desenhar_pratica(ui, e, som);
    if opcoes.com_exportacao {
        desenhar_exportacao(ui, e);
    }
    if let Some(msg) = &e.erro {
        ui.label(RichText::new(msg).color(egui::Color32::from_rgb(232, 112, 106)));
    }
    if let Some(msg) = &e.aviso {
        ui.label(RichText::new(msg).weak());
    }
    if let Some(p) = e.partitura() {
        let mut partes = Vec::new();
        if let Some(t) = &p.titulo {
            partes.push(t.clone());
        }
        if let Some(c) = &p.compositor {
            partes.push(c.clone());
        }
        partes.push(format!(
            "{} compassos, {} pautas",
            p.compassos.len(),
            p.total_de_pautas()
        ));
        if let Some(d) = &p.direitos {
            partes.push(format!("licença: {}", d.trim()));
        }
        ui.label(RichText::new(partes.join(" · ")).weak());
    }
    ui.add_space(4.0);
    let agora = som.agora_ns();
    let destaque = opcoes.cursor_externo.or_else(|| e.destaque(agora));
    if opcoes.com_selecao {
        desenhar_estado_da_selecao(ui, e);
    }
    let Some(d) = e.documento.take() else {
        ui.label("Nenhuma partitura carregada.");
        return;
    };
    let descricao = if opcoes.com_selecao {
        e.descricao_da_selecao().unwrap_or_default()
    } else {
        String::new()
    };
    let rolar_para = if opcoes.com_selecao {
        e.selecao.tomar_rolagem()
    } else {
        None
    };
    let resposta = desenhar_partitura(
        ui,
        d.partitura(),
        &mut e.visual,
        OpcoesVisuais {
            zoom: e.zoom,
            seguir: e.seguir,
            modo: e.modo_rolagem,
            papel_branco: e.papel_branco,
        },
        Marcas {
            destaque,
            selecionado: e.compasso_inicial,
            resultados: e.pratica.as_ref().map(|s| &s.mapa),
            ids: Some(d.ids()),
            selecao: opcoes.com_selecao.then(|| MarcaSelecao {
                nota: e.selecao.nota(),
                rolar_para,
                captura_tab: d.leitura.trilhas().len() > 1,
                alto_contraste: e.alto_contraste,
                descricao: &descricao,
            }),
        },
    );
    e.documento = Some(d);
    if opcoes.com_selecao {
        e.id_da_vista = resposta.id;
        if resposta.com_foco && !ui.ctx().text_edit_focused() {
            let tab_ativo = e.trilhas_de_leitura() > 1;
            let destinos = ler_comandos_de_teclado(ui, &e.atalhos, tab_ativo);
            if !destinos.is_empty() {
                for d in destinos {
                    match d {
                        Destino::Comando(c) => {
                            e.navegar(c);
                        }
                        Destino::Pendente(a) => e.avisar_pendente(a),
                    }
                }
                ui.ctx().request_repaint();
            }
        }
        if e.capturando_atalho.is_some() && !ui.ctx().text_edit_focused() {
            capturar_atalho(ui, e);
        }
        if resposta.acerto.is_some() {
            e.clicar_na_partitura(resposta.acerto);
        }
    } else if let Some(c) = resposta.compasso_clicado {
        e.compasso_inicial = if e.compasso_inicial == Some(c) {
            None
        } else {
            Some(c)
        };
    }
    if e.tocando() || e.praticando() || opcoes.cursor_externo.is_some() {
        ui.ctx().request_repaint();
    }
}

/// Lê (e consome) as teclas da Oficina do quadro, roteadas pelo mapa de atalhos (ticket 13).
/// Só deve ser chamada com a vista da partitura em foco: com foco em campo de texto ou em outro
/// controle, as teclas não são tocadas. Ctrl+K, Ctrl+. e Esc nunca são consumidos: a casca os
/// trata (ver [`MapaAtalhos::rotear`]).
fn ler_comandos_de_teclado(ui: &Ui, mapa: &MapaAtalhos, tab_ativo: bool) -> Vec<Destino> {
    ui.ctx().input_mut(|i| {
        let mut destinos = Vec::new();
        i.events.retain(|ev| {
            if let Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = ev
                && let Some(d) = mapa.rotear(*key, *modifiers, tab_ativo)
            {
                destinos.push(d);
                return false;
            }
            true
        });
        destinos
    })
}

/// Captura a próxima combinação apertada para a ação em remapeamento. Esc cancela (e segue para
/// a casca); só modificador sozinho é ignorado.
fn capturar_atalho(ui: &Ui, e: &mut EstadoPartitura) {
    let Some(acao) = e.capturando_atalho else {
        return;
    };
    let apertada = ui.ctx().input_mut(|i| {
        if i.events.iter().any(|ev| {
            matches!(
                ev,
                Event::Key {
                    key: Key::Escape,
                    pressed: true,
                    ..
                }
            )
        }) {
            return Some(None);
        }
        let pos = i.events.iter().position(|ev| {
            matches!(
                ev,
                Event::Key {
                    pressed: true,
                    repeat: false,
                    ..
                }
            )
        })?;
        match i.events.remove(pos) {
            Event::Key { key, modifiers, .. } => Some(Some(Atalho {
                ctrl: modifiers.command,
                alt: modifiers.alt,
                shift: modifiers.shift,
                tecla: key,
            })),
            _ => None,
        }
    });
    match apertada {
        Some(None) => {
            e.capturando_atalho = None;
            e.aviso_atalho = Some("Remapeamento cancelado.".to_owned());
        }
        Some(Some(atalho)) => e.remapear_atalho(acao, atalho),
        None => {}
    }
}

/// Linha de estado da seleção: nota selecionada em texto (além da cor e do anel), aviso discreto
/// de limite e a dica das teclas.
fn desenhar_estado_da_selecao(ui: &mut Ui, e: &mut EstadoPartitura) {
    let mut partes = Vec::new();
    match e.descricao_da_selecao() {
        Some(t) => partes.push(format!("Nota selecionada: {t}")),
        None => partes.push(
            "Clique numa nota (ou ative a partitura com Tab) e use as setas esquerda e direita; \
             Tab troca de voz ou pauta; Esc sai."
                .to_owned(),
        ),
    }
    if let Some(a) = e.selecao.aviso() {
        partes.push(a.texto().to_owned());
    }
    if let Some(a) = e.aviso_de_atalho() {
        partes.push(a.to_owned());
    }
    ui.label(RichText::new(partes.join(" · ")).weak());
    desenhar_lista_de_atalhos(ui, e);
}

/// Lista de atalhos da Oficina: rótulo em PT-BR, combinação, situação (pendente, conflito com a
/// tela Teclado) e os botões de remapear. Ctrl+K, Ctrl+. e Esc são listados como reservados.
fn desenhar_lista_de_atalhos(ui: &mut Ui, e: &mut EstadoPartitura) {
    egui::CollapsingHeader::new("Atalhos de teclado da Oficina")
        .id_salt("atalhos_da_oficina")
        .show(ui, |ui| {
            ui.label(
                RichText::new(
                    "Reservados e fora do alcance da Oficina: Ctrl+K (paleta), Ctrl+. (Pânico) \
                     e Esc (casca).",
                )
                .weak(),
            );
            let avisos = e.atalhos.avisos_tela_teclado();
            let linhas: Vec<_> = e.atalhos.linhas().collect();
            let mut mudar = None;
            let mut remover = None;
            egui::Grid::new("grade_de_atalhos")
                .num_columns(4)
                .striped(true)
                .show(ui, |ui| {
                    for (acao, atalho) in linhas {
                        ui.label(acao.rotulo());
                        let texto = if e.capturando_atalho == Some(acao) {
                            "Aperte a nova combinação (Esc cancela)".to_owned()
                        } else {
                            atalho.map_or_else(|| "sem atalho".to_owned(), |a| a.to_string())
                        };
                        ui.monospace(texto);
                        let mut situacao = Vec::new();
                        if acao.comando().is_none() {
                            situacao.push("pendente: ainda sem comando".to_owned());
                        }
                        if let Some((_, a)) = avisos.iter().find(|(x, _)| *x == acao) {
                            situacao.push(format!("conflita com a tela Teclado ({})", a.letra));
                        }
                        ui.label(RichText::new(situacao.join("; ")).weak());
                        ui.horizontal(|ui| {
                            if ui.button("Mudar").clicked() {
                                mudar = Some(acao);
                            }
                            if ui.button("Sem atalho").clicked() {
                                remover = Some(acao);
                            }
                        });
                        ui.end_row();
                    }
                });
            if let Some(a) = mudar {
                e.capturando_atalho = Some(a);
                e.aviso_atalho = None;
            }
            if let Some(a) = remover {
                e.atalhos.remover(a);
            }
            if ui.button("Restaurar atalhos padrão").clicked() {
                e.atalhos.restaurar_padrao();
                e.capturando_atalho = None;
                e.aviso_atalho = Some("Atalhos restaurados ao padrão.".to_owned());
            }
        });
}

/// Só a vista da partitura (pautas, cursor e rolagem), sem a barra de comandos, para telas que
/// a embutem (a de revisão do tutor). Quem chama cuida de `atualizar` e dos botões.
pub fn desenhar_vista(ui: &mut Ui, e: &mut EstadoPartitura, som: &ServicoSom) {
    let destaque = e.destaque(som.agora_ns());
    desenhar_vista_com_cursor(ui, e, destaque);
}

/// A vista da partitura com o cursor dado por fora (o Player de Aula segue o vídeo).
pub fn desenhar_vista_com_cursor(ui: &mut Ui, e: &mut EstadoPartitura, destaque: Option<Destaque>) {
    let Some(d) = e.documento.take() else {
        ui.label(RichText::new("Nenhuma partitura carregada.").weak());
        return;
    };
    desenhar_partitura(
        ui,
        d.partitura(),
        &mut e.visual,
        OpcoesVisuais {
            zoom: e.zoom,
            seguir: e.seguir,
            modo: e.modo_rolagem,
            papel_branco: e.papel_branco,
        },
        Marcas {
            destaque,
            selecionado: None,
            resultados: None,
            ids: Some(d.ids()),
            selecao: None,
        },
    );
    e.documento = Some(d);
    if e.tocando() || destaque.is_some() {
        ui.ctx().request_repaint();
    }
}

/// Texto do resumo da prática (as paradas aparecem quando houve alguma).
#[cfg(test)]
fn texto_do_resumo(r: &Resumo) -> String {
    texto_do_resumo_com(r, r.paradas > 0)
}

/// Texto do resumo; `com_paradas` mostra as paradas do Espera tocar mesmo quando são zero.
fn texto_do_resumo_com(r: &Resumo, com_paradas: bool) -> String {
    let mut t = format!(
        "{} de {} notas certas ({:.0}%) · {} não tocadas · {} extras ou erradas",
        r.acertos,
        r.esperadas,
        r.precisao_pct(),
        r.faltas,
        r.extras
    );
    if com_paradas || r.paradas > 0 {
        t.push_str(&format!(
            " · {} {}",
            r.paradas,
            if r.paradas == 1 { "parada" } else { "paradas" }
        ));
    }
    if let (Some(m), Some(a)) = (r.desvio_medio_ms, r.desvio_absoluto_ms) {
        t.push_str(&format!(
            " · tempo: desvio médio {m:+.0} ms (média absoluta {a:.0} ms)"
        ));
    }
    t
}

fn desenhar_pratica(ui: &mut Ui, e: &mut EstadoPartitura, som: &mut ServicoSom) {
    if e.documento.is_none() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        if e.praticando() {
            if ui
                .button("Parar prática")
                .on_hover_text("Encerra e mostra o resumo")
                .clicked()
            {
                e.parar_pratica();
            }
        } else if ui
            .button("Praticar")
            .on_hover_text(
                "Toque no teclado (ou no teclado simulado) as notas da partitura; certas ficam \
                 verdes e erradas, vermelhas. Começa pelo compasso escolhido.",
            )
            .clicked()
        {
            som.panico();
            e.iniciar_pratica(som.agora_ns());
        }
        ui.add_enabled_ui(!e.praticando(), |ui| {
            ComboBox::from_id_salt("modo-pratica")
                .selected_text(e.modo_pratica.rotulo())
                .show_ui(ui, |ui| {
                    for m in ModoPratica::TODOS {
                        ui.selectable_value(&mut e.modo_pratica, m, m.rotulo());
                    }
                })
                .response
                .on_hover_text(
                    "Nota a nota: o cursor para em cada nota até você tocar a certa (ou o acorde).\n\
                     Espera tocar: o cursor anda no andamento e só para quando você deixa passar \
                     uma nota; ao tocá-la, conta dois tempos e retoma.\n\
                     Seguir o andamento: o cursor anda no andamento e cada nota vale se tocada \
                     dentro da tolerância.",
                );
            if e.modo_pratica.tem_andamento() {
                ui.add(
                    Slider::new(
                        &mut e.tolerancia_pratica_ms,
                        TOLERANCIA_PRATICA_MIN_MS..=TOLERANCIA_PRATICA_MAX_MS,
                    )
                    .text("tolerância (ms)")
                    .integer(),
                );
            }
            if e.modo_pratica == ModoPratica::EsperaTocar {
                ui.label("Ao retomar:");
                ComboBox::from_id_salt("retomada-pratica")
                    .selected_text(e.retomada_pratica.rotulo())
                    .show_ui(ui, |ui| {
                        for r in Retomada::TODAS {
                            ui.selectable_value(&mut e.retomada_pratica, r, r.rotulo());
                        }
                    })
                    .response
                    .on_hover_text(
                        "Depois de acertar a nota em que parou, quanto voltar antes de seguir \
                         (as notas voltadas são tocadas e conferidas de novo).",
                    );
            }
        });
        if e.pratica.is_some() && !e.praticando() && ui.small_button("Limpar").clicked() {
            e.limpar_pratica();
        }
    });
    if let Some(r) = e.resumo_pratica() {
        let rotulo = if e.praticando() {
            "Prática"
        } else {
            "Resumo da prática"
        };
        let com_paradas = e.modo_da_pratica() == Some(ModoPratica::EsperaTocar);
        ui.label(
            RichText::new(format!(
                "{rotulo}: {}",
                texto_do_resumo_com(&r, com_paradas)
            ))
            .strong(),
        );
    }
    if let Some(aviso) = e.aviso_da_pratica(som.agora_ns()) {
        ui.label(RichText::new(aviso).strong());
    }
}

fn desenhar_origem(ui: &mut Ui, e: &mut EstadoPartitura) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Partitura:");
        let atual = e
            .fontes
            .get(e.selecionada)
            .map_or_else(String::new, |f| f.nome.clone());
        let mut escolha = None;
        ComboBox::from_id_salt("partitura-fonte")
            .selected_text(atual)
            .width(320.0)
            .show_ui(ui, |ui| {
                for (i, f) in e.fontes.iter().enumerate() {
                    if ui.selectable_label(i == e.selecionada, &f.nome).clicked() {
                        escolha = Some(i);
                    }
                }
            });
        if let Some(i) = escolha {
            e.escolher(i);
        }
        ui.separator();
        let r = ui.add(
            TextEdit::singleline(&mut e.campo_caminho)
                .hint_text("/caminho/arquivo.musicxml")
                .desired_width(280.0),
        );
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if ui.button("Abrir arquivo").clicked() || enter {
            let c = e.campo_caminho.clone();
            e.abrir_caminho(&c);
        }
    });
}

fn desenhar_transporte(ui: &mut Ui, e: &mut EstadoPartitura, som: &mut ServicoSom) {
    ui.horizontal_wrapped(|ui| {
        if e.tocando() {
            if ui.add(Button::new("Parar")).clicked() {
                e.parar(som);
            }
        } else if ui.add(Button::new("Tocar")).clicked() {
            e.tocar(som);
        }
        if !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(Key::Space)) {
            if e.tocando() {
                e.parar(som);
            } else {
                e.tocar(som);
            }
        }
        match e.compasso_inicial.and_then(|c| {
            e.partitura()
                .and_then(|p| p.compassos.get(c).map(|i| i.numero.clone()))
        }) {
            Some(n) => {
                ui.label(format!("começa no compasso {n}"));
                if ui.small_button("do início").clicked() {
                    e.compasso_inicial = None;
                }
            }
            None => {
                ui.label(
                    RichText::new("do início (clique num compasso para escolher onde começar)")
                        .weak(),
                );
            }
        }
        ui.separator();
        let mut bpm = e.bpm;
        if ui
            .add(
                Slider::new(&mut bpm, BPM_MIN..=BPM_MAX)
                    .text("semínima/min")
                    .integer(),
            )
            .changed()
        {
            e.definir_bpm(bpm);
        }
        ui.separator();
        let mut zoom = e.zoom;
        if ui
            .add(
                Slider::new(&mut zoom, ZOOM_MIN..=ZOOM_MAX)
                    .text("zoom")
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                    .custom_parser(|s| {
                        s.trim()
                            .trim_end_matches('%')
                            .parse::<f64>()
                            .ok()
                            .map(|v| v / 100.0)
                    }),
            )
            .changed()
        {
            e.zoom = zoom.clamp(ZOOM_MIN, ZOOM_MAX);
        }
        if ui.small_button("100%").clicked() {
            e.zoom = 1.0;
        }
        if ui.small_button("150%").clicked() {
            e.zoom = 1.5;
        }
        ui.checkbox(&mut e.seguir, "Seguir a execução");
        ui.separator();
        ComboBox::from_id_salt("modo-rolagem")
            .selected_text(format!("Rolagem: {}", e.modo_rolagem.rotulo()))
            .show_ui(ui, |ui| {
                for m in ModoRolagem::TODOS {
                    ui.selectable_value(&mut e.modo_rolagem, m, m.rotulo());
                }
            })
            .response
            .on_hover_text(
                "Sistema a sistema: a vista salta para o sistema atual (no topo, com o seguinte \
                 logo abaixo).\nFita contínua: a peça vira uma linha só, que rola na horizontal \
                 com o cursor parado a 1/3 da largura.",
            );
        ui.checkbox(&mut e.papel_branco, "Papel branco")
            .on_hover_text("Partitura sempre em papel branco, mesmo com o tema Noite");
        ui.checkbox(&mut e.alto_contraste, "Alto contraste na seleção")
            .on_hover_text(
                "Nota selecionada e foco do teclado em amarelo (tema Noite) ou azul puro \
                 (papel branco), com contraste de pelo menos 7:1 contra o papel",
            );
    });
    // Não grava a cada quadro enquanto o slider de zoom é arrastado.
    if !ui.input(|i| i.pointer.any_down()) {
        e.salvar_preferencias();
    }
}

fn desenhar_exportacao(ui: &mut Ui, e: &mut EstadoPartitura) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Exportar MusicXML:");
        ui.add(TextEdit::singleline(&mut e.campo_exportar).desired_width(300.0));
        ui.checkbox(&mut e.sobrescrever, "Sobrescrever");
        if ui
            .button("Exportar")
            .on_hover_text("Grava o arquivo carregado sem alterações e confere relendo (somente leitura: sem edição de notas)")
            .clicked()
        {
            let destino = e.campo_exportar.clone();
            match e.exportar(&destino) {
                Ok(c) => {
                    e.aviso = Some(format!("Exportado e conferido: {}", c.display()));
                    e.erro = None;
                }
                Err(m) => e.erro = Some(m),
            }
        }
    });
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::partitura_visual::Alvo;
    use crate::som::apoio_testes::servico_pronto;
    use bancada_casca::egui::{Context, Modifiers, RawInput, Rect, pos2, vec2};
    use piano_tutor_audio::RelogioManual;
    use piano_tutor_audio::sintetizador::RegistroFalso;
    use piano_tutor_core::mxl::empacotar_mxl;
    use piano_tutor_core::tempo::Relogio;
    use std::sync::Arc;

    fn quadro(ctx: &Context, e: &mut EstadoPartitura, som: &mut ServicoSom) {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
            ..Default::default()
        };
        ctx.run_ui(entrada, |ui| desenhar_tela(ui, e, som))
            .drop_without_applying_deltas();
    }

    const PEQUENA: &str = r#"<score-partwise><part-list><score-part id="P1"><part-name>x</part-name></score-part></part-list>
<part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
<direction><sound tempo="60"/></direction>
<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note>
<note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note>
<note><chord/><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note>
<note><rest/><duration>1</duration><type>quarter</type></note>
<note><pitch><step>C</step><octave>5</octave></pitch><duration>1</duration><type>quarter</type></note>
</measure></part></score-partwise>"#;

    #[test]
    fn novo_lista_as_embutidas_e_carrega_a_primeira() {
        let e = EstadoPartitura::novo(None);
        assert_eq!(e.fontes.len(), EMBUTIDAS.len());
        assert_eq!(e.fontes.len(), 5);
        assert!(e.erro.is_none(), "{:?}", e.erro);
        let p = e.partitura().expect("carregada");
        assert_eq!(p.compassos.len(), 19);
        assert!(!e.tocando());
        assert_eq!(e.bpm, 75.0);
    }

    #[test]
    fn plano_respeita_ordem_tempo_e_andamento() {
        let p = Partitura::de_musicxml(PEQUENA).expect("válida");
        let m = Musica::de(&p);
        // 60 bpm: uma semínima = 1 s.
        let plano = plano_de_reproducao(&m, 60.0, 0.0, 1_000);
        assert_eq!(plano.len(), 4);
        let seg = |n: &NotaAgendada| (n.t_ns - 1_000) as f64 / 1e9;
        assert!((seg(&plano[0]) - 0.0).abs() < 1e-9 && plano[0].midi == 60);
        assert!((seg(&plano[1]) - 1.0).abs() < 1e-9 && plano[1].midi == 64);
        assert!((seg(&plano[2]) - 1.0).abs() < 1e-9 && plano[2].midi == 67);
        assert!((seg(&plano[3]) - 3.0).abs() < 1e-9 && plano[3].midi == 72);
        for w in plano.windows(2) {
            assert!(w[0].t_ns <= w[1].t_ns);
        }
        assert!(
            plano
                .iter()
                .all(|n| n.dur_ns > 0 && n.dur_ns < 1_000_000_000)
        );
        // Dobro do andamento: metade do tempo.
        let rapido = plano_de_reproducao(&m, 120.0, 0.0, 0);
        assert!((rapido[3].t_ns as f64 / 1e9 - 1.5).abs() < 1e-9);
        // Começando em 1 semínima: a primeira nota do plano cai em t0.
        let meio = plano_de_reproducao(&m, 60.0, 1.0, 5_000);
        assert_eq!(meio.len(), 3);
        assert_eq!(meio[0].t_ns, 5_000);
    }

    #[test]
    fn atualizar_agenda_com_antecedencia_e_toca_na_ordem() {
        let relogio = RelogioManual::novo();
        let (mut som, motor) = servico_pronto(Arc::new(relogio.clone()) as Arc<dyn Relogio>);
        motor.renderizar(512, 0);
        let mut e = EstadoPartitura::novo(None);
        e.carregar_texto("pequena".into(), PEQUENA.into())
            .expect("ok");
        assert_eq!(e.bpm, 60.0);
        e.tocar(&mut som);
        assert!(e.tocando());
        // Logo após Tocar só entra o que cai dentro da antecedência (a 1ª nota, em 120 ms);
        // a 2ª (em 1 s) ainda não.
        e.atualizar(&mut som);
        assert_eq!(e.reproducao.as_ref().map(|r| r.proxima), Some(1));
        for _ in 0..350 {
            relogio.avancar(Duration::from_millis(16));
            e.atualizar(&mut som);
        }
        assert!(!e.tocando(), "terminou depois de ~4,5 s");
        motor.renderizar_no_relogio(48_000 * 8);
        let ligadas: Vec<u8> = motor
            .registro()
            .iter()
            .filter_map(|r| match *r {
                RegistroFalso::Ligada { nota, .. } => Some(nota),
                _ => None,
            })
            .collect();
        assert_eq!(ligadas, vec![60, 64, 67, 72]);
    }

    #[test]
    fn cursor_acompanha_a_reproducao_e_parar_limpa() {
        let relogio = RelogioManual::novo();
        let (mut som, _motor) = servico_pronto(Arc::new(relogio.clone()) as Arc<dyn Relogio>);
        let mut e = EstadoPartitura::novo(None);
        e.carregar_texto("pequena".into(), PEQUENA.into())
            .expect("ok");
        assert!(e.destaque(0).is_none());
        e.tocar(&mut som);
        let t0 = e.reproducao.as_ref().map(|r| r.t0_ns).expect("t0");
        let d = e.destaque(t0 + 2_500_000_000).expect("destaque");
        assert_eq!(d.compasso, 0);
        assert!((d.tempo - 2.5).abs() < 1e-3, "{d:?}");
        // Mudar o andamento no meio continua do ponto certo.
        e.definir_bpm(120.0);
        assert!(e.tocando());
        e.parar(&mut som);
        assert!(!e.tocando());
        assert!(e.destaque(t0).is_none());
    }

    #[test]
    fn comecar_do_compasso_escolhido() {
        let mut e = EstadoPartitura::novo(None);
        let relogio = RelogioManual::novo();
        let (mut som, _m) = servico_pronto(Arc::new(relogio) as Arc<dyn Relogio>);
        e.compasso_inicial = Some(2);
        e.tocar(&mut som);
        let r = e.reproducao.as_ref().expect("tocando");
        let m = e.musica().expect("musica");
        assert_eq!(Some(r.q0), m.inicio_do_compasso(2));
        assert!(r.q0 > 0.0);
    }

    #[test]
    fn desenha_cada_embutida_a_100_e_150_sem_panico() {
        let relogio = RelogioManual::novo();
        let (mut som, _m) = servico_pronto(Arc::new(relogio) as Arc<dyn Relogio>);
        let ctx = Context::default();
        let mut e = EstadoPartitura::novo(None);
        for i in 0..3 {
            e.escolher(i);
            assert!(e.erro.is_none(), "{:?}", e.erro);
            for z in [1.0, 1.5] {
                e.zoom = z;
                quadro(&ctx, &mut e, &mut som);
                quadro(&ctx, &mut e, &mut som);
            }
        }
    }

    #[test]
    fn exporta_sem_perda_e_nao_sobrescreve_sem_pedir() {
        let pasta = std::env::temp_dir().join(format!("pt-partitura-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).expect("pasta");
        let mut e = EstadoPartitura::novo(None);
        let destino = pasta.join("saida.musicxml");
        let d = destino.to_string_lossy().into_owned();
        let c = e.exportar(&d).expect("exporta");
        assert_eq!(std::fs::read_to_string(&c).expect("lê"), EMBUTIDAS[0].1);
        assert!(e.exportar(&d).is_err(), "não sobrescreve sem pedir");
        e.sobrescrever = true;
        assert!(e.exportar(&d).is_ok());
        assert!(e.exportar("").is_err());
        assert!(e.exportar(&pasta.join("x.txt").to_string_lossy()).is_err());
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn abre_arquivo_xml_e_mxl_mostra_com_identidades_e_exporta_identico() {
        let pasta = std::env::temp_dir().join(format!("pt-partitura-ident-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).expect("pasta");
        let origem = EMBUTIDAS[3].1;
        let xml = pasta.join("ode.musicxml");
        std::fs::write(&xml, origem).expect("grava");
        let mxl = pasta.join("ode.mxl");
        std::fs::write(&mxl, empacotar_mxl("ode.musicxml", origem).expect("zip")).expect("grava");
        let relogio = RelogioManual::novo();
        let (mut som, _m) = servico_pronto(Arc::new(relogio) as Arc<dyn Relogio>);
        for (i, arq) in [&xml, &mxl].into_iter().enumerate() {
            let mut e = EstadoPartitura::novo(None);
            e.abrir_caminho(&arq.to_string_lossy());
            assert!(e.erro.is_none(), "{:?}", e.erro);
            // A tela desenha o documento aberto e o layout carrega a identidade das notas.
            let ctx = Context::default();
            quadro_tesselado(&ctx, &mut e, &mut som, (1280.0, 800.0));
            let layout = e.visual.ultimo_layout.clone().expect("layout desenhado");
            let cabecas: Vec<_> = layout
                .sistemas
                .iter()
                .flat_map(|s| &s.itens)
                .filter(|it| matches!(it.forma, crate::partitura_visual::Forma::Cabeca { .. }))
                .collect();
            assert!(!cabecas.is_empty());
            assert!(cabecas.iter().all(|it| it.nota.is_some()), "arquivo {i}");
            // Exportar grava o mesmo texto do original (o .mxl entrega o XML de dentro).
            let destino = pasta.join(format!("saida-{i}.musicxml"));
            e.campo_exportar = destino.to_string_lossy().into_owned();
            let alvo = e.campo_exportar.clone();
            e.exportar(&alvo).expect("exporta");
            assert_eq!(std::fs::read(&destino).expect("lê"), origem.as_bytes());
            assert!(
                e.exportar(&alvo).is_err(),
                "pede confirmação para sobrescrever"
            );
            e.sobrescrever = true;
            assert!(e.exportar(&alvo).is_ok());
        }
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn abrir_caminho_valida_extensao_tamanho_e_conteudo() {
        let pasta = std::env::temp_dir().join(format!("pt-partitura-abrir-{}", std::process::id()));
        std::fs::create_dir_all(&pasta).expect("pasta");
        let mut e = EstadoPartitura::novo(None);
        let ruim = pasta.join("ruim.musicxml");
        std::fs::write(&ruim, "isto não é xml").expect("grava");
        e.abrir_caminho(&ruim.to_string_lossy());
        assert!(
            e.erro
                .as_deref()
                .is_some_and(|m| m.contains("XML inválido"))
        );
        // .mxl que não existe, que não é zip e uma bomba: recusados com mensagem, sem pânico.
        e.abrir_caminho(&pasta.join("a.mxl").to_string_lossy());
        assert!(e.erro.as_deref().is_some_and(|m| m.contains(".mxl")));
        let falso = pasta.join("falso.mxl");
        std::fs::write(&falso, "isto não é zip").expect("grava");
        e.abrir_caminho(&falso.to_string_lossy());
        assert!(
            e.erro
                .as_deref()
                .is_some_and(|m| m.contains("arquivo .mxl recusado")),
            "{:?}",
            e.erro
        );
        let bomba = empacotar_mxl("score.xml", &" ".repeat(9 * 1024 * 1024)).expect("zip");
        let arq_bomba = pasta.join("bomba.mxl");
        std::fs::write(&arq_bomba, bomba).expect("grava");
        e.abrir_caminho(&arq_bomba.to_string_lossy());
        assert!(
            e.erro
                .as_deref()
                .is_some_and(|m| m.contains("limite de segurança")),
            "{:?}",
            e.erro
        );
        // .mxl válido abre, com o mesmo conteúdo do .musicxml embutido, e exporta o XML.
        let bom_mxl = pasta.join("ode.mxl");
        std::fs::write(
            &bom_mxl,
            empacotar_mxl("ode.musicxml", EMBUTIDAS[3].1).expect("zip"),
        )
        .expect("grava");
        e.abrir_caminho(&bom_mxl.to_string_lossy());
        assert!(e.erro.is_none(), "{:?}", e.erro);
        assert_eq!(e.partitura().map(|p| p.compassos.len()), Some(16));
        assert_eq!(
            e.documento.as_ref().map(|d| d.xml()).unwrap_or_default(),
            EMBUTIDAS[3].1
        );
        e.abrir_caminho(&pasta.join("a.txt").to_string_lossy());
        assert!(e.erro.is_some());
        let boa = pasta.join("boa.musicxml");
        std::fs::write(&boa, PEQUENA).expect("grava");
        e.abrir_caminho(&boa.to_string_lossy());
        assert!(e.erro.is_none(), "{:?}", e.erro);
        assert_eq!(e.partitura().map(|p| p.compassos.len()), Some(1));
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn pasta_de_assets_acrescenta_partituras_novas() {
        let pasta =
            std::env::temp_dir().join(format!("pt-partitura-assets-{}", std::process::id()));
        std::fs::create_dir_all(pasta.join("partituras")).expect("pasta");
        std::fs::write(pasta.join("partituras/pequena.musicxml"), PEQUENA).expect("grava");
        let e = EstadoPartitura::novo(Some(pasta.clone()));
        assert_eq!(e.fontes.len(), EMBUTIDAS.len() + 1);
        let _ = std::fs::remove_dir_all(&pasta);
    }

    /// Desenha a tela e tesselou as formas, como a janela real faz (o epaint só entra em pânico,
    /// em depuração, na tesselação).
    fn quadro_tesselado(
        ctx: &Context,
        e: &mut EstadoPartitura,
        som: &mut ServicoSom,
        tam: (f32, f32),
    ) {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(tam.0, tam.1))),
            ..Default::default()
        };
        let saida = ctx.run_ui(entrada, |ui| desenhar_tela(ui, e, som));
        let _ = ctx.tessellate(saida.shapes.clone(), saida.pixels_per_point);
        saida.drop_without_applying_deltas();
    }

    #[test]
    fn desenha_e_tessela_em_tamanhos_minimos_e_largura_zero_sem_panico() {
        let relogio = RelogioManual::novo();
        let (mut som, _m) = servico_pronto(Arc::new(relogio) as Arc<dyn Relogio>);
        let tamanhos = [
            (0.0, 0.0),
            (0.0, 800.0),
            (1280.0, 0.0),
            (1.0, 1.0),
            (10.0, 10.0),
            (50.0, 30.0),
            (120.0, 80.0),
            (320.0, 200.0),
            (1280.0, 800.0),
        ];
        for modo in ModoRolagem::TODOS {
            for i in 0..EMBUTIDAS.len() {
                let mut e = EstadoPartitura::novo(None);
                e.escolher(i);
                e.modo_rolagem = modo;
                let ctx = Context::default();
                for zoom in [ZOOM_MIN, 1.5, ZOOM_MAX] {
                    e.zoom = zoom;
                    for t in tamanhos {
                        quadro_tesselado(&ctx, &mut e, &mut som, t);
                    }
                }
            }
        }
    }

    fn ode() -> EstadoPartitura {
        let mut e = EstadoPartitura::novo(None);
        e.escolher(3);
        assert_eq!(e.partitura().map(|p| p.compassos.len()), Some(16));
        e
    }

    fn nota(n: u8) -> NotaMidi {
        NotaMidi::nova(n).expect("nota válida")
    }

    /// Notas esperadas (instante em semínimas, altura) da partitura carregada.
    fn eventos_de(e: &EstadoPartitura) -> Vec<(f64, u8)> {
        let m = e.musica().expect("música");
        m.eventos
            .iter()
            .map(|x| (x.inicio.como_f64(), x.midi))
            .collect()
    }

    #[test]
    fn praticar_nota_a_nota_cursor_para_ate_a_nota_certa_e_marca_verde_e_vermelho() {
        let mut e = ode();
        e.modo_pratica = ModoPratica::NotaANota;
        assert!(!e.praticando());
        e.nota_tocada(true, nota(60), 0); // sem prática: ignorada
        e.iniciar_pratica(1_000);
        assert!(e.praticando());
        let ev = eventos_de(&e);
        // Primeiro passo: Mi4 (64) na melodia e Dó3 (48) no baixo; cursor no compasso 0, tempo 0.
        let d = e.destaque(5_000).expect("cursor");
        assert_eq!((d.compasso, d.tempo), (0, 0.0));
        // Nota errada: cursor fica, marca vermelha, extra contado.
        e.nota_tocada(true, nota(61), 2_000);
        e.atualizar_pratica(3_000);
        let s = e.pratica.as_ref().expect("sessão");
        assert!(!s.mapa.vazio());
        let alvo = |mao| Alvo {
            compasso: 0,
            inicio: 0.0,
            fim: 1.0,
            mao,
        };
        assert_eq!(s.mapa.de(&alvo(Mao::Direita)), Some(ResultadoNota::Errou));
        assert_eq!(s.mapa.de(&alvo(Mao::Esquerda)), Some(ResultadoNota::Errou));
        assert_eq!(e.destaque(9_000).map(|d| d.tempo), Some(0.0));
        // note-off é ignorado; as certas deixam verde e o cursor anda.
        e.nota_tocada(false, nota(64), 3_000);
        e.nota_tocada(true, nota(64), 3_000);
        e.nota_tocada(true, nota(48), 3_100);
        e.atualizar_pratica(4_000);
        let s = e.pratica.as_ref().expect("sessão");
        assert_eq!(s.mapa.de(&alvo(Mao::Direita)), Some(ResultadoNota::Acertou));
        assert_eq!(
            s.mapa.de(&alvo(Mao::Esquerda)),
            Some(ResultadoNota::Acertou)
        );
        assert_eq!(e.destaque(9_000).map(|d| d.tempo), Some(1.0));
        // Toca o resto na ordem, passo a passo: termina com resumo.
        for (q, midi) in ev.iter().filter(|(q, _)| *q > 0.0) {
            let _ = q;
            e.nota_tocada(true, nota(*midi), 10_000);
        }
        e.atualizar_pratica(20_000);
        assert!(!e.praticando());
        let r = e.resumo_pratica().expect("resumo");
        assert_eq!((r.esperadas, r.acertos, r.faltas, r.extras), (94, 94, 0, 1));
        assert!(texto_do_resumo(&r).contains("94 de 94 notas certas (100%)"));
        assert!(e.destaque(0).is_none());
        // Depois do fim, notas são ignoradas; limpar remove o resumo.
        e.nota_tocada(true, nota(60), 30_000);
        assert_eq!(e.resumo_pratica().map(|r| r.extras), Some(1));
        e.limpar_pratica();
        assert!(e.resumo_pratica().is_none());
    }

    #[test]
    fn praticar_no_tempo_segue_o_andamento_com_tolerancia_e_conta_faltas() {
        let mut e = ode();
        e.modo_pratica = ModoPratica::Tempo;
        e.tolerancia_pratica_ms = 100.0;
        e.definir_bpm(100.0);
        let agora = 1_000_000_000u64;
        e.iniciar_pratica(agora);
        // Contagem de entrada de 2 semínimas a 100 bpm = 1,2 s.
        let t0 = agora + 1_200_000_000;
        assert_eq!(e.pratica.as_ref().map(|s| s.t0_ns), Some(t0));
        let d = e.destaque(agora + 100).expect("cursor");
        assert_eq!((d.compasso, d.tempo), (0, 0.0), "cursor espera na entrada");
        // 2 s depois do início: 2 s = 3,33 semínimas, ainda no compasso 0.
        let d = e.destaque(t0 + 2_000_000_000).expect("cursor");
        assert_eq!(d.compasso, 0);
        assert!((d.tempo - 3.333).abs() < 0.01, "{}", d.tempo);
        // Toca tudo no tempo, menos o baixo do compasso 2 (índices pelo instante e altura).
        let ev = eventos_de(&e);
        let mut omitidas = 0;
        // Uma nota fora da partitura logo no começo.
        e.nota_tocada(true, nota(61), t0 + 3_000_000_000);
        for (q, midi) in &ev {
            if (4.0..8.0).contains(q) && *midi < 60 {
                omitidas += 1;
                continue;
            }
            // 20 ms atrasada.
            let ns = t0 + (q * 0.6 * 1e9) as u64 + 20_000_000;
            e.nota_tocada(true, nota(*midi), ns);
        }
        assert!(omitidas > 0);
        e.atualizar_pratica(t0 + 60_000_000_000);
        assert!(!e.praticando());
        let r = e.resumo_pratica().expect("resumo");
        assert_eq!(r.esperadas, 94);
        assert_eq!(
            (r.acertos, r.faltas, r.extras),
            (94 - omitidas, omitidas, 1)
        );
        let med = r.desvio_medio_ms.expect("desvio");
        assert!((med - 20.0).abs() < 0.5, "{med}");
        assert!(texto_do_resumo(&r).contains("desvio médio +20 ms"));
        // Vermelho nas omitidas (compasso 1 = tempos 4..8, mão esquerda).
        let s = e.pratica.as_ref().expect("sessão");
        let alvo = Alvo {
            compasso: 1,
            inicio: 0.0,
            fim: 2.0,
            mao: Mao::Esquerda,
        };
        assert_eq!(s.mapa.de(&alvo), Some(ResultadoNota::Errou));
        let alvo = Alvo {
            mao: Mao::Direita,
            ..alvo
        };
        assert_eq!(s.mapa.de(&alvo), Some(ResultadoNota::Acertou));
    }

    /// Instante (semínimas) e alturas do segundo passo da Ode (o primeiro com `q > 0`).
    fn segundo_passo(e: &EstadoPartitura) -> (f64, Vec<u8>) {
        let ev = eventos_de(e);
        let q = ev
            .iter()
            .map(|(q, _)| *q)
            .filter(|q| *q > 0.0)
            .fold(f64::INFINITY, f64::min);
        let alturas = ev
            .iter()
            .filter(|(x, _)| (*x - q).abs() < 1e-9)
            .map(|(_, m)| *m)
            .collect();
        (q, alturas)
    }

    /// Ode a 120 bpm (semínima = 0,5 s), Espera tocar com janela de 100 ms, com a contagem de
    /// entrada de 1 s a partir de 1 s: o tempo da prática 0 cai em 2 s.
    fn espera_tocar(retomada: Retomada) -> (EstadoPartitura, u64) {
        let mut e = ode();
        e.modo_pratica = ModoPratica::EsperaTocar;
        e.tolerancia_pratica_ms = 100.0;
        e.retomada_pratica = retomada;
        e.definir_bpm(120.0);
        e.iniciar_pratica(1_000_000_000);
        let t0 = 2_000_000_000;
        assert_eq!(e.pratica.as_ref().map(|s| s.t0_ns), Some(t0));
        (e, t0)
    }

    const S: u64 = 1_000_000_000;
    const MS: u64 = 1_000_000;

    #[test]
    fn espera_tocar_para_ao_fim_da_janela_congela_o_cursor_e_nota_errada_nao_para() {
        let (mut e, t0) = espera_tocar(Retomada::Zero);
        let (q1, alturas) = segundo_passo(&e);
        assert!(!alturas.is_empty());
        // Nota errada no começo: vira extra, não para.
        e.nota_tocada(true, nota(61), t0 + 5 * MS);
        e.atualizar_pratica(t0 + 50 * MS);
        assert!(!e.pratica_parada());
        assert_eq!(e.resumo_pratica().map(|r| r.extras), Some(1));
        // Primeiro passo tocado na hora; a janela do segundo (0,5 s + 100 ms) ainda não venceu.
        e.nota_tocada(true, nota(64), t0 + 10 * MS);
        e.nota_tocada(true, nota(48), t0 + 12 * MS);
        e.atualizar_pratica(t0 + 590 * MS);
        assert!(!e.pratica_parada());
        assert!(e.aviso_da_pratica(t0 + 590 * MS).is_none());
        // Vence a janela: para, o cursor fica na nota que falta e não anda mais.
        e.atualizar_pratica(t0 + 700 * MS);
        assert!(e.pratica_parada());
        let esperado = e.musica().and_then(|m| m.compasso_em(q1)).expect("posição");
        for agora in [t0 + 700 * MS, t0 + 5 * S, t0 + 60 * S] {
            e.atualizar_pratica(agora);
            let d = e.destaque(agora).expect("cursor");
            assert_eq!((d.compasso, d.tempo), esperado, "cursor congelado");
        }
        assert_eq!(
            e.aviso_da_pratica(t0 + 5 * S)
                .as_deref()
                .map(|a| a.starts_with("Parado:")),
            Some(true)
        );
        // Nota errada parado: continua parado e conta extra.
        e.nota_tocada(true, nota(61), t0 + 6 * S);
        assert!(e.pratica_parada());
        let r = e.resumo_pratica().expect("resumo");
        assert_eq!((r.paradas, r.extras), (1, 2));
        // A nota que falta fica vermelha no mapa.
        let s = e.pratica.as_ref().expect("sessão");
        assert!(!s.mapa.vazio());
        assert_eq!(e.indice_da_pratica().map(|i| eventos_de(&e)[i].0), Some(q1));
    }

    #[test]
    fn espera_tocar_retoma_da_nota_parada_com_contagem_de_dois_tempos() {
        let (mut e, t0) = espera_tocar(Retomada::Zero);
        let (q1, alturas) = segundo_passo(&e);
        e.nota_tocada(true, nota(64), t0 + 10 * MS);
        e.nota_tocada(true, nota(48), t0 + 12 * MS);
        e.atualizar_pratica(t0 + 700 * MS);
        assert!(e.pratica_parada());
        // O aluno toca 20 s depois (relógio da tela seguiu, o da prática não).
        let t = t0 + 20 * S;
        for m in &alturas {
            e.nota_tocada(true, nota(*m), t);
        }
        assert!(!e.pratica_parada());
        assert!(e.praticando());
        let pos = |e: &EstadoPartitura, q: f64| {
            e.musica().and_then(|m| m.compasso_em(q)).expect("posição")
        };
        // Contagem de dois tempos (1 s a 120 bpm) com o cursor no ponto de retomada.
        assert_eq!(e.aviso_da_pratica(t).as_deref(), Some("Contagem: 2"));
        assert_eq!(
            e.aviso_da_pratica(t + 500 * MS).as_deref(),
            Some("Contagem: 1")
        );
        assert_eq!(e.aviso_da_pratica(t + S), None);
        for agora in [t, t + 400 * MS, t + S] {
            let d = e.destaque(agora).expect("cursor");
            let (c, q) = pos(&e, q1);
            assert_eq!(d.compasso, c, "cursor na retomada");
            assert!((d.tempo - q).abs() < 1e-6, "{} != {q}", d.tempo);
        }
        // Depois da contagem o relógio anda: 0,25 s depois, meia semínima à frente.
        let d = e.destaque(t + S + 250 * MS).expect("cursor");
        let (c, q) = pos(&e, q1 + 0.5);
        assert_eq!(d.compasso, c);
        assert!((d.tempo - q).abs() < 1e-6, "{} != {q}", d.tempo);
        // O resto da prática segue: a parada não repete, o passo é conferido uma vez só.
        e.atualizar_pratica(t + S + 300 * MS);
        assert!(!e.pratica_parada());
        let r = e.resumo_pratica().expect("resumo");
        assert_eq!((r.paradas, r.acertos), (1, 2 + alturas.len()));
    }

    #[test]
    fn espera_tocar_volta_uma_nota_e_confere_de_novo() {
        let (mut e, t0) = espera_tocar(Retomada::Uma);
        let (q1, alturas) = segundo_passo(&e);
        e.nota_tocada(true, nota(64), t0 + 10 * MS);
        e.nota_tocada(true, nota(48), t0 + 12 * MS);
        e.atualizar_pratica(t0 + 700 * MS);
        assert!(e.pratica_parada());
        let t = t0 + 10 * S;
        for m in &alturas {
            e.nota_tocada(true, nota(*m), t);
        }
        assert!(!e.pratica_parada());
        // Volta ao primeiro passo (q = 0): contagem de 1 s e o cursor no início.
        assert_eq!(e.aviso_da_pratica(t).as_deref(), Some("Contagem: 2"));
        let d = e.destaque(t + 100 * MS).expect("cursor");
        assert_eq!((d.compasso, d.tempo), (0, 0.0));
        // Os passos voltados voltam a ser conferidos: nada acertado ainda.
        assert_eq!(e.resumo_pratica().map(|r| r.acertos), Some(0));
        // Toca o primeiro passo de novo no tempo certo (t + 1 s = tempo 0 da prática).
        e.nota_tocada(true, nota(64), t + S + 10 * MS);
        e.nota_tocada(true, nota(48), t + S + 12 * MS);
        assert_eq!(e.resumo_pratica().map(|r| r.acertos), Some(2));
        // Segundo passo no tempo (0,5 s de q1 = 1 depois do tempo 0).
        let t1 = t + S + (q1 * 0.5 * 1e9) as u64 + 10 * MS;
        for m in &alturas {
            e.nota_tocada(true, nota(*m), t1);
        }
        e.atualizar_pratica(t1 + 50 * MS);
        assert!(!e.pratica_parada(), "sem nova parada");
        let r = e.resumo_pratica().expect("resumo");
        assert_eq!((r.paradas, r.acertos), (1, 2 + alturas.len()));
    }

    #[test]
    fn espera_tocar_o_resumo_inclui_as_paradas() {
        let (mut e, t0) = espera_tocar(Retomada::Zero);
        e.atualizar_pratica(t0 + 700 * MS); // para no primeiro passo
        assert!(e.pratica_parada());
        e.parar_pratica();
        let r = e.resumo_final_da_pratica().expect("resumo");
        assert_eq!(r.paradas, 1);
        assert_eq!(r.faltas, 94 + 1);
        assert!(texto_do_resumo(&r).contains("1 parada"));
        assert!(!texto_do_resumo(&r).contains("paradas"));
        let sem = Resumo { paradas: 0, ..r };
        assert!(!texto_do_resumo(&sem).contains("parada"));
        assert!(texto_do_resumo_com(&sem, true).contains("0 paradas"));
        assert!(texto_do_resumo(&Resumo { paradas: 3, ..r }).contains("3 paradas"));
        assert_eq!(e.modo_da_pratica(), Some(ModoPratica::EsperaTocar));
        assert!(e.aviso_da_pratica(t0).is_none());
    }

    #[test]
    fn espera_tocar_desenha_parado_e_em_contagem_sem_panico() {
        let (mut e, _t0) = espera_tocar(Retomada::Duas);
        let (mut som, _m) = servico_pronto(Arc::new(RelogioManual::novo()) as Arc<dyn Relogio>);
        let ctx = Context::default();
        quadro_tesselado(&ctx, &mut e, &mut som, (1280.0, 800.0));
        e.parar_pratica();
        quadro_tesselado(&ctx, &mut e, &mut som, (1280.0, 800.0));
    }

    #[test]
    fn praticar_parar_encerra_com_faltas_e_tocar_cancela_a_pratica() {
        let mut e = ode();
        e.iniciar_pratica(0);
        e.nota_tocada(true, nota(64), 10);
        e.parar_pratica();
        assert!(!e.praticando());
        let r = e.resumo_pratica().expect("resumo");
        assert_eq!((r.acertos, r.faltas), (1, 93));
        // Começa no compasso escolhido (compasso 4): só as notas dali em diante.
        e.compasso_inicial = Some(4);
        e.iniciar_pratica(0);
        assert_eq!(
            e.pratica.as_ref().map(|s| s.logica.esperados().len()),
            Some(eventos_de(&e).iter().filter(|(q, _)| *q >= 16.0).count())
        );
        let (mut som, _m) = servico_pronto(Arc::new(RelogioManual::novo()) as Arc<dyn Relogio>);
        e.tocar(&mut som);
        assert!(!e.praticando() && e.resumo_pratica().is_none() && e.tocando());
        // Sem partitura de prática nada quebra.
        e.nota_tocada(true, nota(60), 0);
        e.atualizar_pratica(0);
    }

    #[test]
    fn praticar_desenha_com_resultados_sem_panico() {
        let mut e = ode();
        let (mut som, _m) = servico_pronto(Arc::new(RelogioManual::novo()) as Arc<dyn Relogio>);
        e.iniciar_pratica(som.agora_ns());
        e.nota_tocada(true, nota(61), som.agora_ns());
        e.nota_tocada(true, nota(64), som.agora_ns());
        let ctx = Context::default();
        quadro_tesselado(&ctx, &mut e, &mut som, (1280.0, 800.0));
        e.parar_pratica();
        quadro_tesselado(&ctx, &mut e, &mut som, (1280.0, 800.0));
    }

    // --- Seleção e navegação (ticket 12) ------------------------------------------------------

    /// Duas vozes (pautas 1 e 2) para o `Tab`.
    const DUAS_VOZES: &str = r#"<score-partwise><part-list><score-part id="P1"><part-name>x</part-name></score-part></part-list>
<part id="P1"><measure number="1"><attributes><divisions>1</divisions><staves>2</staves>
<clef number="1"><sign>G</sign><line>2</line></clef><clef number="2"><sign>F</sign><line>4</line></clef></attributes>
<note><pitch><step>C</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>half</type><staff>1</staff></note>
<note><pitch><step>E</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>half</type><staff>1</staff></note>
<backup><duration>4</duration></backup>
<note><pitch><step>C</step><octave>3</octave></pitch><duration>4</duration><voice>5</voice><type>whole</type><staff>2</staff></note>
</measure></part></score-partwise>"#;

    fn tecla(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn quadro_com(
        ctx: &Context,
        e: &mut EstadoPartitura,
        som: &mut ServicoSom,
        eventos: Vec<Event>,
        modifiers: Modifiers,
    ) {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
            events: std::iter::once(Event::ModifiersChanged(modifiers))
                .chain(eventos)
                .collect(),
            ..Default::default()
        };
        ctx.run_ui(entrada, |ui| desenhar_tela(ui, e, som))
            .drop_without_applying_deltas();
    }

    /// Desenha, dá o foco do teclado à vista da partitura e deixa o filtro de teclas assentar.
    fn partitura_em_foco(ctx: &Context, e: &mut EstadoPartitura, som: &mut ServicoSom) -> egui::Id {
        quadro(ctx, e, som);
        let id = e.id_da_vista.expect("a vista foi desenhada");
        ctx.memory_mut(|m| m.request_focus(id));
        quadro(ctx, e, som);
        quadro(ctx, e, som);
        assert!(ctx.memory(|m| m.has_focus(id)), "a vista deveria ter foco");
        id
    }

    fn apertar(
        ctx: &Context,
        e: &mut EstadoPartitura,
        som: &mut ServicoSom,
        key: Key,
        modifiers: Modifiers,
    ) {
        quadro_com(ctx, e, som, vec![tecla(key, modifiers)], modifiers);
        quadro(ctx, e, som);
    }

    fn banco() -> (ServicoSom, Context) {
        let (som, _m) = servico_pronto(Arc::new(RelogioManual::novo()) as Arc<dyn Relogio>);
        (som, Context::default())
    }

    /// O comando de navegação que o perfil padrão liga à tecla.
    fn comando_da_tecla(tecla: Key, mods: Modifiers, tab_ativo: bool) -> Option<Comando> {
        match MapaAtalhos::padrao().rotear(tecla, mods, tab_ativo) {
            Some(Destino::Comando(c)) => Some(c),
            _ => None,
        }
    }

    #[test]
    fn tabela_de_teclas_so_vale_sem_ctrl_alt_cmd_e_tab_so_com_mais_de_uma_voz() {
        let nenhum = Modifiers::NONE;
        assert_eq!(
            comando_da_tecla(Key::ArrowLeft, nenhum, false),
            Some(Comando::Anterior)
        );
        assert_eq!(
            comando_da_tecla(Key::ArrowRight, nenhum, false),
            Some(Comando::Seguinte)
        );
        assert_eq!(
            comando_da_tecla(Key::Tab, nenhum, true),
            Some(Comando::TrilhaSeguinte)
        );
        assert_eq!(
            comando_da_tecla(Key::Tab, Modifiers::SHIFT, true),
            Some(Comando::TrilhaAnterior)
        );
        // Uma voz só: Tab é da janela (passa o foco adiante), não da partitura.
        assert_eq!(comando_da_tecla(Key::Tab, nenhum, false), None);
        // Atalhos da casca e do sistema nunca são tomados.
        for m in [
            Modifiers::COMMAND,
            Modifiers::CTRL,
            Modifiers::ALT,
            Modifiers::SHIFT,
        ] {
            assert_eq!(comando_da_tecla(Key::ArrowLeft, m, true), None, "{m:?}");
            assert_eq!(comando_da_tecla(Key::ArrowRight, m, true), None, "{m:?}");
        }
        assert_eq!(comando_da_tecla(Key::Tab, Modifiers::COMMAND, true), None);
        assert_eq!(comando_da_tecla(Key::K, Modifiers::COMMAND, true), None);
        assert_eq!(
            comando_da_tecla(Key::Period, Modifiers::COMMAND, true),
            None
        );
        assert_eq!(comando_da_tecla(Key::A, nenhum, true), None);
        assert_eq!(comando_da_tecla(Key::Space, nenhum, true), None);
    }

    #[test]
    fn a_oficina_nunca_consome_ctrl_k_ctrl_ponto_nem_esc_e_so_trata_o_que_a_casca_entrega() {
        let ctx = Context::default();
        let mapa = MapaAtalhos::padrao();
        let eventos = vec![
            tecla(Key::K, Modifiers::COMMAND),
            tecla(Key::Period, Modifiers::COMMAND),
            tecla(Key::Escape, Modifiers::NONE),
            tecla(Key::ArrowRight, Modifiers::NONE),
        ];
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))),
            events: eventos,
            ..Default::default()
        };
        let mut destinos = Vec::new();
        let mut restantes = Vec::new();
        ctx.run_ui(entrada, |ui| {
            destinos = ler_comandos_de_teclado(ui, &mapa, true);
            restantes = ui.input(|i| {
                i.events
                    .iter()
                    .filter_map(|ev| match ev {
                        Event::Key { key, .. } => Some(*key),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            });
        })
        .drop_without_applying_deltas();
        assert_eq!(destinos, vec![Destino::Comando(Comando::Seguinte)]);
        // Os três reservados seguem no quadro para a casca; só a seta foi consumida.
        assert_eq!(restantes, vec![Key::K, Key::Period, Key::Escape]);
    }

    #[test]
    fn atalho_de_acao_pendente_avisa_em_vez_de_calar_e_nao_muda_a_selecao() {
        let (mut som, ctx) = banco();
        let mut e = ode();
        partitura_em_foco(&ctx, &mut e, &mut som);
        apertar(&ctx, &mut e, &mut som, Key::ArrowRight, Modifiers::NONE);
        let sel = e.nota_selecionada();
        apertar(&ctx, &mut e, &mut som, Key::ArrowUp, Modifiers::NONE);
        let aviso = e.aviso_de_atalho().expect("aviso de pendente");
        assert!(aviso.contains("Subir um semitom"), "{aviso}");
        assert!(aviso.contains("ainda não tem comando"), "{aviso}");
        assert_eq!(e.nota_selecionada(), sel);
        assert!(!e.documento.as_ref().expect("doc").editado());
    }

    #[test]
    fn remapear_vale_sem_reiniciar_e_recusa_reservado_e_em_uso_com_mensagem() {
        let (mut som, ctx) = banco();
        let mut e = ode();
        partitura_em_foco(&ctx, &mut e, &mut som);
        // Ctrl+J passa a ser "próxima nota"; → deixa de andar.
        e.remapear_atalho(AcaoAtalho::NotaSeguinte, Atalho::ctrl(Key::J));
        apertar(&ctx, &mut e, &mut som, Key::ArrowRight, Modifiers::NONE);
        assert_eq!(e.nota_selecionada(), None);
        apertar(&ctx, &mut e, &mut som, Key::J, Modifiers::COMMAND);
        assert!(e.nota_selecionada().is_some());
        // Recusas.
        e.remapear_atalho(AcaoAtalho::Cifra, Atalho::ctrl(Key::K));
        assert!(e.aviso_de_atalho().expect("recusa").contains("reservado"));
        e.remapear_atalho(AcaoAtalho::Cifra, Atalho::ctrl(Key::Z));
        assert!(e.aviso_de_atalho().expect("recusa").contains("Desfazer"));
        assert_eq!(
            e.atalhos.atalho(AcaoAtalho::Cifra),
            Some(AcaoAtalho::Cifra.padrao())
        );
    }

    #[test]
    fn captura_da_lista_de_atalhos_aplica_a_proxima_tecla() {
        let (mut som, ctx) = banco();
        let mut e = ode();
        quadro(&ctx, &mut e, &mut som);
        e.capturando_atalho = Some(AcaoAtalho::Ponto);
        quadro_com(
            &ctx,
            &mut e,
            &mut som,
            vec![tecla(Key::P, Modifiers::COMMAND)],
            Modifiers::COMMAND,
        );
        assert_eq!(e.capturando_atalho, None);
        assert_eq!(
            e.atalhos.atalho(AcaoAtalho::Ponto),
            Some(Atalho::ctrl(Key::P))
        );
        // Esc cancela a captura sem mudar nada.
        e.capturando_atalho = Some(AcaoAtalho::Ponto);
        quadro_com(
            &ctx,
            &mut e,
            &mut som,
            vec![tecla(Key::Escape, Modifiers::NONE)],
            Modifiers::NONE,
        );
        assert_eq!(e.capturando_atalho, None);
        assert_eq!(
            e.atalhos.atalho(AcaoAtalho::Ponto),
            Some(Atalho::ctrl(Key::P))
        );
    }

    #[test]
    fn setas_so_agem_com_a_partitura_em_foco_e_esc_devolve_o_foco() {
        let (mut som, ctx) = banco();
        let mut e = ode();
        quadro(&ctx, &mut e, &mut som);
        // Sem foco na partitura, as setas não fazem nada.
        apertar(&ctx, &mut e, &mut som, Key::ArrowRight, Modifiers::NONE);
        assert_eq!(e.nota_selecionada(), None);
        // Com foco: → seleciona a primeira nota, depois anda.
        partitura_em_foco(&ctx, &mut e, &mut som);
        apertar(&ctx, &mut e, &mut som, Key::ArrowRight, Modifiers::NONE);
        let primeira = e.nota_selecionada().expect("selecionou a primeira");
        apertar(&ctx, &mut e, &mut som, Key::ArrowRight, Modifiers::NONE);
        let segunda = e.nota_selecionada().expect("andou");
        assert_ne!(primeira, segunda);
        apertar(&ctx, &mut e, &mut som, Key::ArrowLeft, Modifiers::NONE);
        assert_eq!(e.nota_selecionada(), Some(primeira));
        // Na primeira nota, ← fica e avisa.
        apertar(&ctx, &mut e, &mut som, Key::ArrowLeft, Modifiers::NONE);
        assert_eq!(e.nota_selecionada(), Some(primeira));
        assert!(e.selecao().aviso().is_some());
        // Esc devolve o foco à janela; a seleção fica e as setas voltam a não fazer nada.
        apertar(&ctx, &mut e, &mut som, Key::Escape, Modifiers::NONE);
        let id = e.id_da_vista.expect("id");
        assert!(!ctx.memory(|m| m.has_focus(id)));
        apertar(&ctx, &mut e, &mut som, Key::ArrowRight, Modifiers::NONE);
        assert_eq!(e.nota_selecionada(), Some(primeira));
        // Navegar nunca suja o documento.
        assert!(!e.documento.as_ref().expect("doc").editado());
    }

    #[test]
    fn ctrl_e_alt_com_setas_nao_movem_a_selecao_mesmo_com_foco() {
        let (mut som, ctx) = banco();
        let mut e = ode();
        partitura_em_foco(&ctx, &mut e, &mut som);
        apertar(&ctx, &mut e, &mut som, Key::ArrowRight, Modifiers::COMMAND);
        apertar(&ctx, &mut e, &mut som, Key::ArrowRight, Modifiers::ALT);
        apertar(&ctx, &mut e, &mut som, Key::ArrowRight, Modifiers::SHIFT);
        assert_eq!(e.nota_selecionada(), None);
    }

    #[test]
    fn digitar_em_campo_de_texto_nao_move_a_selecao() {
        let (mut som, ctx) = banco();
        let mut e = ode();
        partitura_em_foco(&ctx, &mut e, &mut som);
        apertar(&ctx, &mut e, &mut som, Key::ArrowRight, Modifiers::NONE);
        let antes = e.nota_selecionada().expect("selecionada");
        // O foco passa a um campo de texto ao lado da partitura.
        let campo = egui::Id::new("campo-de-texto-da-casca");
        let mut texto = String::new();
        let mut frame = |eventos: Vec<Event>, e: &mut EstadoPartitura, som: &mut ServicoSom| {
            let entrada = RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
                events: eventos,
                ..Default::default()
            };
            ctx.run_ui(entrada, |ui| {
                ui.add(TextEdit::singleline(&mut texto).id(campo));
                desenhar_corpo(ui, e, som, OpcoesTela::COMPLETA);
            })
            .drop_without_applying_deltas();
        };
        frame(Vec::new(), &mut e, &mut som);
        ctx.memory_mut(|m| m.request_focus(campo));
        frame(Vec::new(), &mut e, &mut som);
        frame(Vec::new(), &mut e, &mut som);
        assert!(ctx.memory(|m| m.has_focus(campo)));
        let id = e.id_da_vista.expect("id");
        assert!(!ctx.memory(|m| m.has_focus(id)));
        // Digitar e andar o cursor do texto: a seleção da partitura não muda.
        frame(
            vec![
                Event::Text("abc".into()),
                tecla(Key::ArrowRight, Modifiers::NONE),
                tecla(Key::ArrowLeft, Modifiers::NONE),
            ],
            &mut e,
            &mut som,
        );
        frame(Vec::new(), &mut e, &mut som);
        assert_eq!(e.nota_selecionada(), Some(antes));
        assert!(e.selecao().aviso().is_none());
    }

    #[test]
    fn tab_troca_de_voz_e_so_e_capturado_havendo_mais_de_uma() {
        let (mut som, ctx) = banco();
        let mut e = EstadoPartitura::novo(None);
        e.abrir_xml("duas vozes", DUAS_VOZES.to_owned())
            .expect("abre");
        assert_eq!(e.trilhas_de_leitura(), 2);
        let id = partitura_em_foco(&ctx, &mut e, &mut som);
        apertar(&ctx, &mut e, &mut som, Key::Tab, Modifiers::NONE);
        let em_cima = e.nota_selecionada().expect("começou pela primeira voz");
        apertar(&ctx, &mut e, &mut som, Key::Tab, Modifiers::NONE);
        let embaixo = e.nota_selecionada().expect("trocou de pauta");
        assert_ne!(em_cima, embaixo);
        // O foco continua na partitura (Tab não o levou a outro controle).
        assert!(ctx.memory(|m| m.has_focus(id)));
        apertar(&ctx, &mut e, &mut som, Key::Tab, Modifiers::SHIFT);
        assert_eq!(e.nota_selecionada(), Some(em_cima));

        // Com uma voz só, a partitura não captura o Tab: nada é selecionado nem avisado.
        let mut u = EstadoPartitura::novo(None);
        u.abrir_xml("uma voz", PEQUENA.to_owned()).expect("abre");
        assert_eq!(u.trilhas_de_leitura(), 1);
        partitura_em_foco(&ctx, &mut u, &mut som);
        apertar(&ctx, &mut u, &mut som, Key::Tab, Modifiers::NONE);
        assert_eq!(u.nota_selecionada(), None);
        assert!(u.selecao().aviso().is_none());
    }

    #[test]
    fn clique_em_nota_seleciona_e_clique_fora_alterna_o_compasso_sem_perder_a_nota() {
        let (mut som, ctx) = banco();
        let mut e = ode();
        quadro(&ctx, &mut e, &mut som);
        let layout = e.visual.ultimo_layout.clone().expect("layout");
        let (centro, id) = layout
            .sistemas
            .iter()
            .flat_map(|s| &s.itens)
            .find_map(|it| match (&it.forma, it.nota) {
                (crate::partitura_visual::Forma::Cabeca { centro, .. }, Some(id)) => {
                    Some((*centro, id))
                }
                _ => None,
            })
            .expect("há cabeça com identidade");
        let sobre = layout.nota_em(centro, crate::partitura_visual::TOLERANCIA_NOTA_SP);
        assert_eq!(sobre, Some(id));
        // Clique na nota: seleciona e informa a mesma identidade do hit test; o compasso de
        // partida não muda.
        let efeito = e.clicar_na_partitura(layout.acertar(centro, 0.25));
        assert_eq!(efeito, EfeitoDoClique::NotaSelecionada(id));
        assert_eq!(e.nota_selecionada(), Some(id));
        assert_eq!(e.compasso_inicial, None);
        // Clique fora de nota (dentro do compasso): alterna o compasso de partida, a nota fica.
        let efeito = e.clicar_na_partitura(Some(Acerto::Compasso(2)));
        assert_eq!(efeito, EfeitoDoClique::Compasso(2));
        assert_eq!(e.compasso_inicial, Some(2));
        assert_eq!(e.nota_selecionada(), Some(id));
        e.clicar_na_partitura(Some(Acerto::Compasso(2)));
        assert_eq!(e.compasso_inicial, None);
        // Clique fora de tudo: nada muda.
        assert_eq!(e.clicar_na_partitura(None), EfeitoDoClique::Nenhum);
        assert_eq!(e.nota_selecionada(), Some(id));
        // Trocar de partitura limpa a seleção (outras identidades).
        e.escolher(0);
        assert_eq!(e.nota_selecionada(), None);
    }

    #[test]
    fn desenha_com_selecao_foco_e_alto_contraste_sem_panico() {
        let (mut som, ctx) = banco();
        let mut e = ode();
        partitura_em_foco(&ctx, &mut e, &mut som);
        apertar(&ctx, &mut e, &mut som, Key::ArrowRight, Modifiers::NONE);
        for alto in [false, true] {
            e.alto_contraste = alto;
            for modo in ModoRolagem::TODOS {
                e.modo_rolagem = modo;
                quadro_tesselado(&ctx, &mut e, &mut som, (1280.0, 800.0));
                quadro_tesselado(&ctx, &mut e, &mut som, (120.0, 80.0));
            }
        }
        // O alto contraste é preferência do perfil: sai e volta pelas preferências.
        e.alto_contraste = true;
        e.salvar_preferencias();
        let p = e.tomar_preferencias_mudadas().expect("mudou");
        assert!(p.alto_contraste);
        let mut outro = EstadoPartitura::novo(None);
        outro.definir_preferencias(p);
        assert!(outro.alto_contraste);
    }
}
