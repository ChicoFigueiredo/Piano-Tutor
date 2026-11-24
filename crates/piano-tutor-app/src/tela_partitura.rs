//! Tela Partitura (F07, somente leitura: ADR-0007): escolhe uma das partituras embutidas (ou
//! um `.musicxml` digitado), mostra em duas pautas (sol/fá) com o componente
//! [`crate::partitura_visual`], toca pelo mesmo motor de som (agendamento antecipado como o
//! ritmo), acompanha a execução com cursor e exporta o MusicXML carregado.
//!
//! O app chama [`EstadoPartitura::atualizar`] a cada quadro (agenda as notas até
//! `agora + ANTECEDENCIA`) e [`EstadoPartitura::tocando`] para saber se há reprodução.

use crate::partitura_visual::{
    Destaque, EstadoVisual, Mao, MapaResultados, Marcas, ModoRolagem, OpcoesVisuais, ResultadoNota,
    ZOOM_MAX, ZOOM_MIN, desenhar_partitura,
};
use crate::preferencias::{ARQUIVO_PREFERENCIAS_PARTITURA, PreferenciasPartitura};
use crate::som::{EstadoSom, ServicoSom};
use bancada_casca::egui::{self, Button, ComboBox, Key, RichText, Slider, TextEdit, Ui};
use piano_tutor_core::NotaMidi;
use piano_tutor_core::comparador::EventoEsperado;
use piano_tutor_core::mxl::{MXL_TAMANHO_MAXIMO_BYTES, extrair_musicxml};
use piano_tutor_core::partitura::{
    ErroPartitura, EventoReproducao, MapaAndamento, Partitura, SinalClave, TAMANHO_MAXIMO_BYTES,
};
use piano_tutor_core::pratica::{
    ConfigPratica, EstadoEvento, ModoPratica, Pratica, Resumo, Veredito,
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

/// Dados musicais derivados da partitura, calculados uma vez por carga.
#[derive(Clone, Debug)]
pub struct Musica {
    pub eventos: Vec<EventoReproducao>,
    pub mapa: MapaAndamento,
    /// Início de cada compasso tocado (semínimas na execução) e seu índice.
    pub execucao: Vec<(f64, usize)>,
    pub duracao_q: f64,
    pub bpm_arquivo: Option<f64>,
}

impl Musica {
    pub fn de(p: &Partitura) -> Self {
        Self {
            eventos: p.eventos_de_reproducao(),
            mapa: p.mapa_de_andamento(),
            execucao: p
                .compassos_na_execucao()
                .into_iter()
                .map(|(q, c)| (q.como_f64(), c))
                .collect(),
            duracao_q: p.duracao_da_execucao().como_f64(),
            bpm_arquivo: p.andamento_inicial(),
        }
    }

    /// Segundos do início da execução até `q` semínimas, no andamento `bpm_alvo`.
    pub fn segundos(&self, q: f64, bpm_alvo: f64) -> f64 {
        let base = self.bpm_arquivo.unwrap_or(bpm_alvo).max(1.0);
        self.mapa.segundos(q, base, bpm_alvo / base)
    }

    /// Instante (semínimas na execução) do primeiro compasso `c` tocado.
    pub fn inicio_do_compasso(&self, c: usize) -> Option<f64> {
        self.execucao.iter().find(|(_, i)| *i == c).map(|(q, _)| *q)
    }

    /// Compasso e tempo dentro dele para a posição `q` (semínimas na execução).
    pub fn compasso_em(&self, q: f64) -> Option<(usize, f64)> {
        self.execucao
            .iter()
            .rev()
            .find(|(ini, _)| *ini <= q + 1e-9)
            .map(|(ini, c)| (*c, (q - ini).max(0.0)))
    }
}

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

/// Contagem de entrada do modo Tempo, em semínimas (antes do primeiro compasso tocado).
const BATIDAS_DE_ENTRADA: f64 = 2.0;
/// Tolerância inicial do modo Tempo, em ms.
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
    /// Relógio do modo Tempo, só para converter instante em posição (`notas` fica vazio).
    relogio: Reproducao,
    mapa: MapaResultados,
    sujo: bool,
    resumo_final: Option<Resumo>,
}

impl SessaoPratica {
    fn segundos(&self, instante_ns: u64) -> f64 {
        (instante_ns as f64 - self.t0_ns as f64) / 1e9
    }

    fn reconstruir_mapa(&mut self) {
        let mut mapa = MapaResultados::default();
        for (pos, est) in self.posicoes.iter().zip(self.logica.estados()) {
            let r = match est {
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
    partitura: Option<Partitura>,
    musica: Option<Musica>,
    /// Texto XML carregado, gravado de volta na exportação (nada é descartado).
    xml: String,
    nome: String,
    pub erro: Option<String>,
    pub aviso: Option<String>,
    pub zoom: f32,
    pub bpm: f32,
    pub seguir: bool,
    /// Como a vista acompanha a execução (sistema a sistema ou fita contínua).
    pub modo_rolagem: ModoRolagem,
    /// Papel branco com tinta escura mesmo no tema Noite.
    pub papel_branco: bool,
    /// Arquivo onde as preferências de exibição são gravadas (sem pasta de dados, não grava).
    arquivo_preferencias: Option<PathBuf>,
    preferencias_gravadas: PreferenciasPartitura,
    visual: EstadoVisual,
    compasso_inicial: Option<usize>,
    reproducao: Option<Reproducao>,
    campo_exportar: String,
    sobrescrever: bool,
    /// Modo Praticar: como a prática avança.
    pub modo_pratica: ModoPratica,
    /// Janela de acerto do modo Tempo, em ms.
    pub tolerancia_pratica_ms: f32,
    pratica: Option<SessaoPratica>,
}

fn ler_arquivo(caminho: &Path) -> Result<String, String> {
    let ext = caminho
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    if ext != "musicxml" && ext != "xml" && ext != "mxl" {
        return Err("use um arquivo .musicxml, .xml ou .mxl".into());
    }
    let meta = std::fs::metadata(caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
    if !meta.is_file() {
        return Err(format!("{} não é um arquivo", caminho.display()));
    }
    if ext == "mxl" {
        if meta.len() > MXL_TAMANHO_MAXIMO_BYTES as u64 {
            return Err(format!(
                "arquivo grande demais ({} bytes; máximo {MXL_TAMANHO_MAXIMO_BYTES})",
                meta.len()
            ));
        }
        let bytes = std::fs::read(caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
        return extrair_musicxml(&bytes).map_err(|e| e.to_string());
    }
    if meta.len() > TAMANHO_MAXIMO_BYTES as u64 {
        return Err(format!(
            "arquivo grande demais ({} bytes; máximo {TAMANHO_MAXIMO_BYTES})",
            meta.len()
        ));
    }
    std::fs::read_to_string(caminho).map_err(|e| format!("{}: {e}", caminho.display()))
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
        let arquivo_preferencias = pasta_assets
            .as_ref()
            .map(|p| p.join(ARQUIVO_PREFERENCIAS_PARTITURA));
        let prefs = arquivo_preferencias
            .as_deref()
            .and_then(PreferenciasPartitura::ler)
            .unwrap_or_default();
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
            partitura: None,
            musica: None,
            xml: String::new(),
            nome: String::new(),
            erro: None,
            aviso: None,
            zoom: prefs.zoom,
            bpm: BPM_PADRAO,
            seguir: prefs.seguir,
            modo_rolagem: prefs.modo,
            papel_branco: prefs.papel_branco,
            arquivo_preferencias,
            preferencias_gravadas: prefs,
            visual: EstadoVisual::default(),
            compasso_inicial: None,
            reproducao: None,
            campo_exportar: String::new(),
            sobrescrever: false,
            modo_pratica: ModoPratica::Espera,
            tolerancia_pratica_ms: TOLERANCIA_PRATICA_PADRAO_MS,
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
        self.partitura.as_ref()
    }

    pub fn nome(&self) -> &str {
        &self.nome
    }

    fn carregar_texto(&mut self, nome: String, texto: String) -> Result<(), ErroPartitura> {
        let p = Partitura::de_musicxml(&texto)?;
        let m = Musica::de(&p);
        if let Some(b) = m.bpm_arquivo {
            self.bpm = (b as f32).clamp(BPM_MIN, BPM_MAX);
        }
        self.campo_exportar = format!("{}.musicxml", nome_de_arquivo(&nome));
        self.nome = nome;
        self.xml = texto;
        self.partitura = Some(p);
        self.musica = Some(m);
        self.compasso_inicial = None;
        self.pratica = None;
        self.visual.invalidar();
        Ok(())
    }

    fn preferencias_visuais(&self) -> PreferenciasPartitura {
        PreferenciasPartitura {
            modo: self.modo_rolagem,
            papel_branco: self.papel_branco,
            seguir: self.seguir,
            zoom: self.zoom,
        }
    }

    /// Grava as preferências de exibição quando mudam (sem pasta de dados, não grava).
    fn salvar_preferencias(&mut self) {
        let atual = self.preferencias_visuais();
        if atual == self.preferencias_gravadas {
            return;
        }
        if let Some(c) = &self.arquivo_preferencias {
            let _ = atual.gravar(c);
        }
        self.preferencias_gravadas = atual;
    }

    /// Escolhe uma fonte da lista (para a reprodução, se houver).
    pub fn escolher(&mut self, i: usize) {
        let Some(f) = self.fontes.get(i).cloned() else {
            return;
        };
        self.parar_sem_som();
        let texto = match &f.caminho {
            None => Ok(EMBUTIDAS[f.indice_embutida].1.to_owned()),
            Some(c) => ler_arquivo(c),
        };
        match texto.and_then(|t| {
            self.carregar_texto(f.nome.clone(), t)
                .map_err(|e| e.to_string())
        }) {
            Ok(()) => {
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
        match ler_arquivo(c).and_then(|t| {
            let nome = c
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| caminho.to_owned());
            self.carregar_texto(nome, t).map_err(|e| e.to_string())
        }) {
            Ok(()) => self.erro = None,
            Err(e) => self.erro = Some(e),
        }
    }

    fn parar_sem_som(&mut self) {
        self.reproducao = None;
    }

    /// Começa a tocar a partir do compasso escolhido (ou do início).
    pub fn tocar(&mut self, som: &mut ServicoSom) {
        let Some(m) = &self.musica else {
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
        let (Some(r), Some(m)) = (self.reproducao.as_mut(), self.musica.as_ref()) else {
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
        let (r, m) = (self.reproducao.as_ref()?, self.musica.as_ref()?);
        Some(q_em_ns(m, r, agora_ns))
    }

    /// Cursor da execução (compasso e tempo dentro dele), se estiver tocando.
    pub fn destaque(&self, agora_ns: u64) -> Option<Destaque> {
        if let Some(s) = self.pratica.as_ref() {
            if s.resumo_final.is_some() {
                return None;
            }
            return match s.logica.config().modo {
                ModoPratica::Espera => {
                    let pos = s.posicoes.get(s.logica.evento_do_passo_atual()?)?;
                    Some(Destaque {
                        compasso: pos.compasso,
                        tempo: pos.tempo,
                    })
                }
                ModoPratica::Tempo => {
                    let q = q_em_ns(self.musica.as_ref()?, &s.relogio, agora_ns);
                    let (compasso, tempo) = self.musica.as_ref()?.compasso_em(q)?;
                    Some(Destaque { compasso, tempo })
                }
            };
        }
        let q = self.posicao_q(agora_ns)?;
        let (compasso, tempo) = self.musica.as_ref()?.compasso_em(q)?;
        Some(Destaque { compasso, tempo })
    }

    /// Há prática em andamento (ainda não terminou).
    pub fn praticando(&self) -> bool {
        self.pratica
            .as_ref()
            .is_some_and(|s| s.resumo_final.is_none())
    }

    /// Resumo da última prática (terminada ou parcial), se houver.
    pub fn resumo_pratica(&self) -> Option<Resumo> {
        let s = self.pratica.as_ref()?;
        Some(s.resumo_final.unwrap_or_else(|| s.logica.resumo()))
    }

    /// Começa a praticar a partir do compasso escolhido (ou do início), no modo e na tolerância
    /// configurados. `agora_ns` é o relógio comum (`ServicoSom::agora_ns`). Para a reprodução.
    /// No modo Tempo há uma contagem de entrada de duas semínimas antes do primeiro compasso.
    pub fn iniciar_pratica(&mut self, agora_ns: u64) {
        let (Some(m), Some(p)) = (self.musica.as_ref(), self.partitura.as_ref()) else {
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
        };
        let entrada_s = match config.modo {
            ModoPratica::Tempo => BATIDAS_DE_ENTRADA * 60.0 / bpm,
            ModoPratica::Espera => 0.0,
        };
        let t0_ns = agora_ns.saturating_add(para_ns(entrada_s));
        let mut s = SessaoPratica {
            logica: Pratica::nova(&esperados, config),
            posicoes,
            t0_ns,
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
        let t = s.segundos(instante_ns);
        if s.logica.nota_tocada(nota.numero(), t) != Veredito::Ignorada {
            s.sujo = true;
        }
        s.conferir_fim();
    }

    /// Chamar a cada quadro: faz o relógio do modo Tempo andar (esperadas sem nota viram falta
    /// ao fim da janela) e fecha a prática quando tudo foi resolvido.
    pub fn atualizar_pratica(&mut self, agora_ns: u64) {
        let Some(s) = self.pratica.as_mut() else {
            return;
        };
        if s.resumo_final.is_none() {
            let t = s.segundos(agora_ns);
            if s.logica.avancar_tempo(t) {
                s.sujo = true;
            }
            s.conferir_fim();
        }
        if s.sujo {
            s.reconstruir_mapa();
        }
    }

    /// Grava o MusicXML carregado, sem alterar nada, e confere relendo o arquivo.
    pub fn exportar(&mut self, caminho: &str) -> Result<PathBuf, String> {
        let t = caminho.trim();
        if t.is_empty() {
            return Err("digite o caminho do arquivo de destino".into());
        }
        let mut c = PathBuf::from(t);
        if c.extension().is_none() {
            c.set_extension("musicxml");
        }
        let ext = c
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase)
            .unwrap_or_default();
        if ext != "musicxml" && ext != "xml" {
            return Err("o destino deve terminar em .musicxml ou .xml".into());
        }
        if self.xml.is_empty() {
            return Err("nenhuma partitura carregada".into());
        }
        let mut aberto = std::fs::OpenOptions::new();
        aberto.write(true);
        if self.sobrescrever {
            aberto.create(true).truncate(true);
        } else {
            aberto.create_new(true);
        }
        use std::io::Write;
        let mut f = aberto.open(&c).map_err(|e| {
            format!(
                "{}: {e} (marque \"Sobrescrever\" para trocar um arquivo)",
                c.display()
            )
        })?;
        f.write_all(self.xml.as_bytes())
            .map_err(|e| format!("{}: {e}", c.display()))?;
        drop(f);
        let lido = std::fs::read_to_string(&c).map_err(|e| format!("{}: {e}", c.display()))?;
        let relido = Partitura::de_musicxml(&lido).map_err(|e| e.to_string())?;
        if Some(&relido) != self.partitura.as_ref() {
            return Err("a releitura do arquivo exportado difere da partitura carregada".into());
        }
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
    if t_ns <= r.t0_ns {
        return r.q0;
    }
    let alvo = (t_ns - r.t0_ns) as f64 / 1e9 + m.segundos(r.q0, r.bpm);
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

/// Desenha a tela inteira.
pub fn desenhar_tela(ui: &mut Ui, e: &mut EstadoPartitura, som: &mut ServicoSom) {
    ui.heading("Partitura");
    desenhar_origem(ui, e);
    e.atualizar_pratica(som.agora_ns());
    desenhar_transporte(ui, e, som);
    desenhar_pratica(ui, e, som);
    desenhar_exportacao(ui, e);
    if let Some(msg) = &e.erro {
        ui.label(RichText::new(msg).color(egui::Color32::from_rgb(232, 112, 106)));
    }
    if let Some(msg) = &e.aviso {
        ui.label(RichText::new(msg).weak());
    }
    if let Some(p) = &e.partitura {
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
    let destaque = e.destaque(agora);
    let Some(p) = e.partitura.take() else {
        ui.label("Nenhuma partitura carregada.");
        return;
    };
    let resposta = desenhar_partitura(
        ui,
        &p,
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
        },
    );
    e.partitura = Some(p);
    if let Some(c) = resposta.compasso_clicado {
        e.compasso_inicial = if e.compasso_inicial == Some(c) {
            None
        } else {
            Some(c)
        };
    }
    if e.tocando() || e.praticando() {
        ui.ctx().request_repaint();
    }
}

/// Só a vista da partitura (pautas, cursor e rolagem), sem a barra de comandos, para telas que
/// a embutem (a de revisão do tutor). Quem chama cuida de `atualizar` e dos botões.
pub fn desenhar_vista(ui: &mut Ui, e: &mut EstadoPartitura, som: &ServicoSom) {
    let destaque = e.destaque(som.agora_ns());
    let Some(p) = e.partitura.take() else {
        ui.label(RichText::new("Nenhuma partitura carregada.").weak());
        return;
    };
    desenhar_partitura(
        ui,
        &p,
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
        },
    );
    e.partitura = Some(p);
    if e.tocando() {
        ui.ctx().request_repaint();
    }
}

/// Texto do resumo da prática.
fn texto_do_resumo(r: &Resumo) -> String {
    let mut t = format!(
        "{} de {} notas certas ({:.0}%) · {} não tocadas · {} extras ou erradas",
        r.acertos,
        r.esperadas,
        r.precisao_pct(),
        r.faltas,
        r.extras
    );
    if let (Some(m), Some(a)) = (r.desvio_medio_ms, r.desvio_absoluto_ms) {
        t.push_str(&format!(
            " · tempo: desvio médio {m:+.0} ms (média absoluta {a:.0} ms)"
        ));
    }
    t
}

fn desenhar_pratica(ui: &mut Ui, e: &mut EstadoPartitura, som: &mut ServicoSom) {
    if e.partitura.is_none() {
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
                    "Esperar a nota certa: o cursor para até você tocar a nota (ou o acorde).\n\
                     Seguir o andamento: o cursor anda no andamento e cada nota vale se tocada \
                     dentro da tolerância.",
                );
            if e.modo_pratica == ModoPratica::Tempo {
                ui.add(
                    Slider::new(
                        &mut e.tolerancia_pratica_ms,
                        TOLERANCIA_PRATICA_MIN_MS..=TOLERANCIA_PRATICA_MAX_MS,
                    )
                    .text("tolerância (ms)")
                    .integer(),
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
        ui.label(RichText::new(format!("{rotulo}: {}", texto_do_resumo(&r))).strong());
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
            e.partitura
                .as_ref()
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
    use bancada_casca::egui::{Context, RawInput, Rect, pos2, vec2};
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
        let m = e.musica.as_ref().expect("musica");
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
        assert_eq!(e.xml, EMBUTIDAS[3].1);
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
        let m = e.musica.as_ref().expect("música");
        m.eventos
            .iter()
            .map(|x| (x.inicio.como_f64(), x.midi))
            .collect()
    }

    #[test]
    fn praticar_na_espera_cursor_para_ate_a_nota_certa_e_marca_verde_e_vermelho() {
        let mut e = ode();
        e.modo_pratica = ModoPratica::Espera;
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
}
