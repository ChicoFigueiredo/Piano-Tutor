//! Modo **Chamada e resposta** (M4, etapa 7): o professor anuncia e toca um trecho e o aluno o
//! repete no teclado; a resposta é avaliada com o [`comparador`](crate::comparador).
//!
//! Núcleo puro: sem relógio do sistema, sem som e sem voz. Os instantes entram como parâmetro
//! (`agora_s`, em segundos, numa base qualquer e monotônica escolhida pela camada de app) e a
//! máquina devolve [`Acao`]s que a camada de app executa (falar, tocar notas, mostrar feedback).
//!
//! ```text
//!            iniciar                 fala_terminou / estimativa
//!  Pronto ───────────► Anunciando ─────────────────────────────► ProfessorTocando
//!                          ▲  (sem anúncio: pula)                       │ trecho + folga
//!                          │                                            ▼
//!                          │                                     AguardandoAluno ──┐ sem 1ª nota
//!                          │                                            │          │ em `espera_inicio_s`
//!                          │                         silêncio após notas│          │
//!                          │                                            ▼          │
//!                          │                                        Avaliando ◄────┘
//!                          │                                     (transitório)
//!                          │                              acertou │       │ errou
//!                          │                                      ▼       ▼
//!                          │   próxima chamada ◄──────────── Acertou     Errou ──► tentativas restantes:
//!                          └──────────────────────────────────────┘          │      volta a ProfessorTocando
//!                                                                            │      (mesma chamada, sem anúncio)
//!            sem mais chamadas ► Concluido ◄─────────────────────────────────┘ esgotou: segue à próxima
//! ```
//!
//! Regras:
//!
//! - o aluno começa quando quiser, até `espera_inicio_s` depois que o professor termina; o
//!   tempo das notas da resposta é medido a partir da **primeira nota do aluno**;
//! - a resposta termina por **silêncio** depois da última nota (`silencio_fim_s`, ampliado
//!   para 1,5 vez a maior pausa do trecho esperado) ou, se o aluno já tocou notas suficientes,
//!   por um silêncio mais curto (`silencio_completo_s`);
//! - `Avaliando` é transitório: a avaliação roda no mesmo passo em que o silêncio é detectado,
//!   então quem consulta [`Sessao::estado`] vê `Acertou` ou `Errou` (o feedback fica em pausa
//!   por `pausa_feedback_s`, para a voz poder falar);
//! - errou: repete a chamada (o professor toca de novo) até `max_tentativas`; esgotadas, a
//!   chamada conta como não resolvida e a sessão **segue** (o aluno nunca fica preso);
//! - nada aqui mede latência de MIDI ou áudio: **NÃO VERIFICADO** com teclado real; só a
//!   lógica é testada.

use crate::NotaMidi;
use crate::comparador::{
    Comparacao, ConfigComparacao, ErroComparacao, EventoEsperado, NotaTocada, comparar,
};
use crate::teoria::{Acorde, ClasseAltura, Qualidade};
use std::fmt;

// --- Nomes em português ---------------------------------------------------------------------

/// Nome falado da classe de altura ("Mi bemol", "Dó sustenido"): a voz não lê `b` nem `#`.
pub const fn nome_falado_da_classe(c: ClasseAltura) -> &'static str {
    const N: [&str; 12] = [
        "Dó",
        "Dó sustenido",
        "Ré",
        "Mi bemol",
        "Mi",
        "Fá",
        "Fá sustenido",
        "Sol",
        "Sol sustenido",
        "Lá",
        "Si bemol",
        "Si",
    ];
    N[c.valor() as usize]
}

/// Nome da nota na **convenção do curso do Luciano**: o Dó central (MIDI 60) é **Dó3**, uma
/// oitava abaixo da científica (C4). Usa `#` na grafia (a fonte da tela não tem ♯).
pub fn nome_da_nota_no_curso(midi: u8) -> String {
    let Some(nota) = NotaMidi::nova(midi) else {
        return format!("MIDI {midi}");
    };
    format!(
        "{}{}",
        nota.nome_pt().replace('♯', "#"),
        i16::from(nota.oitava()) - 1
    )
}

/// Nome do acorde para a fala: o maior é só a fundamental ("acorde de Lá"); os demais levam a
/// qualidade ("acorde de Lá menor").
pub fn nome_falado_do_acorde(acorde: &Acorde) -> String {
    let raiz = nome_falado_da_classe(acorde.fundamental);
    match acorde.qualidade {
        Qualidade::Maior => raiz.to_owned(),
        outra => format!("{raiz} {}", outra.nome().replace(" (dominante)", "")),
    }
}

// --- Chamada --------------------------------------------------------------------------------

/// Como a resposta do aluno é conferida.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ModoResposta {
    /// Alturas MIDI exatas.
    #[default]
    Exato,
    /// Vale qualquer oitava: cada nota do aluno é levada à nota esperada de mesma classe de
    /// altura mais próxima (uma nota esperada só casa uma nota tocada; a dobra vira extra).
    PorClasse,
}

/// Uma chamada: o que o professor diz, o que toca e o que espera de volta.
///
/// Os tempos de `trecho` e `resposta` são relativos (segundos); só importa a diferença entre
/// eles, pois a máquina desloca cada lista para começar em zero.
#[derive(Clone, Debug, PartialEq)]
pub struct Chamada {
    /// Fala de abertura (por exemplo "Agora o acorde de Lá."); `None` = sem anúncio.
    pub anuncio: Option<String>,
    /// O que o professor toca.
    pub trecho: Vec<EventoEsperado>,
    /// O que o aluno deve tocar; `None` = o mesmo trecho.
    pub resposta: Option<Vec<EventoEsperado>>,
    pub modo: ModoResposta,
}

fn evento(midi: u8, inicio_s: f64, duracao_s: f64, compasso: usize) -> EventoEsperado {
    EventoEsperado {
        midi,
        inicio_s,
        duracao_s,
        parte: 0,
        pauta: 1,
        compasso,
    }
}

impl Chamada {
    /// Chamada em que a resposta é o próprio trecho, sem anúncio.
    pub fn nova(trecho: Vec<EventoEsperado>) -> Self {
        Self {
            anuncio: None,
            trecho,
            resposta: None,
            modo: ModoResposta::Exato,
        }
    }

    /// Notas em sequência `(altura MIDI, duração em s)`, uma depois da outra.
    pub fn de_notas(notas: &[(u8, f64)]) -> Self {
        let mut t = 0.0;
        let trecho = notas
            .iter()
            .map(|&(midi, dur)| {
                let e = evento(midi, t, dur, 0);
                t += dur;
                e
            })
            .collect();
        Self::nova(trecho)
    }

    /// Notas simultâneas (acorde) com a mesma duração.
    pub fn de_acorde(notas: &[u8], duracao_s: f64) -> Self {
        Self::nova(
            notas
                .iter()
                .map(|&m| evento(m, 0.0, duracao_s, 0))
                .collect(),
        )
    }

    pub fn com_anuncio(mut self, texto: impl Into<String>) -> Self {
        self.anuncio = Some(texto.into());
        self
    }

    pub fn com_resposta(mut self, resposta: Vec<EventoEsperado>) -> Self {
        self.resposta = Some(resposta);
        self
    }

    pub fn por_classe(mut self) -> Self {
        self.modo = ModoResposta::PorClasse;
        self
    }

    /// Resposta esperada (o trecho, se nenhuma foi dada).
    pub fn resposta_esperada(&self) -> &[EventoEsperado] {
        self.resposta.as_deref().unwrap_or(&self.trecho)
    }
}

/// Desloca a lista para que a primeira nota comece em zero (ordenada pelo início), fundindo
/// uníssonos (mesma altura, mesmo instante).
fn normalizar(eventos: &[EventoEsperado]) -> Vec<EventoEsperado> {
    let zero = eventos
        .iter()
        .map(|e| e.inicio_s)
        .fold(f64::INFINITY, f64::min);
    let mut v: Vec<EventoEsperado> = eventos
        .iter()
        .map(|e| EventoEsperado {
            inicio_s: e.inicio_s - zero,
            ..*e
        })
        .collect();
    v.sort_by(|a, b| a.inicio_s.total_cmp(&b.inicio_s).then(a.midi.cmp(&b.midi)));
    // Uníssono entre mãos: uma tecla só, então um evento só.
    crate::comparador::fundir_unissonos(&v)
}

/// Instante em que o último som do trecho acaba (após normalizar).
fn duracao_do_trecho(eventos: &[EventoEsperado]) -> f64 {
    eventos
        .iter()
        .map(|e| e.inicio_s + e.duracao_s.max(0.0))
        .fold(0.0, f64::max)
}

/// Maior intervalo entre inícios consecutivos (0 com menos de dois inícios distintos).
fn maior_intervalo_entre_inicios(eventos: &[EventoEsperado]) -> f64 {
    let mut inicios: Vec<f64> = eventos.iter().map(|e| e.inicio_s).collect();
    inicios.sort_by(f64::total_cmp);
    inicios.windows(2).map(|w| w[1] - w[0]).fold(0.0, f64::max)
}

/// Estimativa grosseira da duração de uma fala, para quando a camada de app não avisa o fim
/// dela (voz muda): 0,8 s + 0,06 s por caractere.
pub fn estimar_duracao_da_fala_s(texto: &str) -> f64 {
    0.8 + 0.06 * texto.chars().count() as f64
}

// --- Configuração ---------------------------------------------------------------------------

/// Parâmetros da sessão. Todos os tempos em segundos.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConfigChamada {
    /// Tentativas por chamada, a primeira incluída (>= 1).
    pub max_tentativas: u32,
    /// Tolerância e compensação do comparador; os tempos são relativos à primeira nota do aluno.
    pub comparacao: ConfigComparacao,
    /// Quanto esperar a primeira nota do aluno depois que o professor termina.
    pub espera_inicio_s: f64,
    /// Silêncio depois da última nota que encerra a resposta.
    pub silencio_fim_s: f64,
    /// Silêncio que encerra a resposta quando o aluno já tocou notas suficientes.
    pub silencio_completo_s: f64,
    /// Folga entre o fim do som do professor e o início da espera pelo aluno.
    pub folga_apos_professor_s: f64,
    /// Pausa depois do feedback, antes da repetição ou da próxima chamada.
    pub pausa_feedback_s: f64,
    /// `true`: o anúncio termina pela estimativa de duração; `false`: só quando a camada de app
    /// chama [`Sessao::fala_terminou`] (use com voz que avisa o fim).
    pub anuncio_por_estimativa: bool,
    /// `true`: notas a mais não reprovam a resposta (continuam listadas nos detalhes).
    pub aceita_notas_extras: bool,
}

impl Default for ConfigChamada {
    fn default() -> Self {
        Self {
            max_tentativas: 3,
            comparacao: ConfigComparacao {
                tolerancia_ms: 250.0,
                compensacao_ms: 0.0,
            },
            espera_inicio_s: 15.0,
            silencio_fim_s: 1.5,
            silencio_completo_s: 0.6,
            folga_apos_professor_s: 0.4,
            pausa_feedback_s: 1.5,
            anuncio_por_estimativa: true,
            aceita_notas_extras: false,
        }
    }
}

/// Configuração ou chamada inválida (nunca pânico).
#[derive(Clone, Debug, PartialEq)]
pub enum ErroChamada {
    /// `max_tentativas` zero.
    SemTentativas,
    /// Algum tempo da configuração é negativo ou não finito.
    TempoInvalido(&'static str),
    Comparacao(ErroComparacao),
    /// A chamada (índice) não tem notas a tocar ou a esperar.
    ChamadaVazia(usize),
    /// Algum instante ou duração da chamada (índice) não é finito.
    ChamadaComTempoInvalido(usize),
}

impl fmt::Display for ErroChamada {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemTentativas => write!(f, "o número de tentativas deve ser pelo menos 1"),
            Self::TempoInvalido(q) => write!(f, "tempo inválido na configuração: {q}"),
            Self::Comparacao(e) => write!(f, "comparação inválida: {e}"),
            Self::ChamadaVazia(i) => write!(f, "a chamada {} não tem notas", i + 1),
            Self::ChamadaComTempoInvalido(i) => {
                write!(f, "a chamada {} tem tempo não finito", i + 1)
            }
        }
    }
}

impl std::error::Error for ErroChamada {}

impl ConfigChamada {
    fn validar(&self) -> Result<(), ErroChamada> {
        if self.max_tentativas == 0 {
            return Err(ErroChamada::SemTentativas);
        }
        let tempos = [
            ("espera_inicio_s", self.espera_inicio_s),
            ("silencio_fim_s", self.silencio_fim_s),
            ("silencio_completo_s", self.silencio_completo_s),
            ("folga_apos_professor_s", self.folga_apos_professor_s),
            ("pausa_feedback_s", self.pausa_feedback_s),
        ];
        for (nome, v) in tempos {
            if !v.is_finite() || v < 0.0 {
                return Err(ErroChamada::TempoInvalido(nome));
            }
        }
        comparar(&[], &[], self.comparacao).map_err(ErroChamada::Comparacao)?;
        Ok(())
    }
}

// --- Saídas ---------------------------------------------------------------------------------

/// Por que a resposta não foi aceita.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Motivo {
    /// Resposta aceita.
    Acertou,
    /// O aluno não tocou nada no tempo de espera.
    NaoTocou,
    /// Faltaram notas (e não sobrou nenhuma).
    NotasFaltando,
    /// Todas as notas esperadas vieram, mas houve nota a mais.
    NotaExtra,
    /// Tocou uma altura que não era a esperada.
    NotaErrada,
    /// Tocou as alturas certas, mas fora da janela de tempo (por exemplo, acorde espalhado).
    ForaDoTempo,
}

impl Motivo {
    /// Texto curto, com símbolo, para a tela (nunca só cor).
    pub const fn texto(self) -> &'static str {
        match self {
            Motivo::Acertou => "[OK] Resposta certa.",
            Motivo::NaoTocou => "[!] Não ouvi você tocar.",
            Motivo::NotasFaltando => "[X] Faltaram notas.",
            Motivo::NotaExtra => "[X] Tocou uma nota a mais.",
            Motivo::NotaErrada => "[X] Alguma nota não era a pedida.",
            Motivo::ForaDoTempo => "[!] As notas certas vieram fora do tempo.",
        }
    }
}

/// Resultado detalhado de uma avaliação.
#[derive(Clone, Debug, PartialEq)]
pub struct Detalhes {
    /// Índice (0-based) da chamada.
    pub chamada: usize,
    /// Tentativa avaliada (1-based).
    pub tentativa: u32,
    pub max_tentativas: u32,
    pub motivo: Motivo,
    pub esperadas: usize,
    pub tocadas: usize,
    pub acertos: usize,
    /// Alturas esperadas que não foram tocadas na janela.
    pub faltaram: Vec<u8>,
    /// Alturas tocadas que não casaram com nenhuma esperada.
    pub sobraram: Vec<u8>,
    pub desvio_medio_ms: Option<f64>,
    /// Errou e a chamada vai se repetir.
    pub vai_repetir: bool,
    /// Errou e não há mais tentativas: a sessão segue para a próxima chamada.
    pub esgotou: bool,
}

/// O que a camada de app deve fazer.
#[derive(Clone, Debug, PartialEq)]
pub enum Acao {
    /// Dizer o texto (voz) e mostrá-lo na tela.
    Falar(String),
    /// Tocar as notas agora; os instantes são relativos ao momento da ação (primeira nota em 0).
    TocarNotas(Vec<EventoEsperado>),
    /// Resultado da resposta do aluno.
    Feedback { acertou: bool, detalhes: Detalhes },
    /// Todas as chamadas foram vistas.
    Concluido(Resumo),
}

/// Estado visível da máquina.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Estado {
    /// Criada, ainda não iniciada.
    Pronto,
    Anunciando,
    ProfessorTocando,
    AguardandoAluno,
    /// Transitório: a avaliação roda no mesmo passo em que é disparada.
    Avaliando,
    /// Pausa depois de uma resposta certa.
    Acertou,
    /// Pausa depois de uma resposta errada (a repetição ou a próxima chamada vem a seguir).
    Errou,
    Concluido,
}

/// Desfecho de uma chamada.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResultadoChamada {
    pub acertou: bool,
    /// Tentativas gastas (1 = acertou de primeira).
    pub tentativas: u32,
}

/// Balanço da sessão (uma entrada por chamada já resolvida).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Resumo {
    pub resultados: Vec<ResultadoChamada>,
    /// Quantas chamadas a sessão tem no total.
    pub total: usize,
}

impl Resumo {
    pub fn acertadas(&self) -> usize {
        self.resultados.iter().filter(|r| r.acertou).count()
    }

    pub fn nao_resolvidas(&self) -> usize {
        self.resultados.iter().filter(|r| !r.acertou).count()
    }

    pub fn acertadas_de_primeira(&self) -> usize {
        self.resultados
            .iter()
            .filter(|r| r.acertou && r.tentativas == 1)
            .count()
    }

    pub fn tentativas_totais(&self) -> u32 {
        self.resultados.iter().map(|r| r.tentativas).sum()
    }
}

// --- Máquina de estados ---------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
enum Fase {
    Pronto,
    Anunciando { ate_s: Option<f64> },
    Professor { ate_s: f64 },
    Aguardando { desde_s: f64 },
    Acertou { ate_s: f64 },
    Errou { ate_s: f64, repete: bool },
    Concluido,
}

/// Sessão de Chamada e resposta: uma lista de chamadas, uma de cada vez.
#[derive(Clone, Debug)]
pub struct Sessao {
    chamadas: Vec<Chamada>,
    config: ConfigChamada,
    fase: Fase,
    atual: usize,
    /// Tentativa da chamada atual (1-based).
    tentativa: u32,
    /// Notas do aluno na espera atual: (altura, instante absoluto).
    tocadas: Vec<(u8, f64)>,
    resultados: Vec<ResultadoChamada>,
}

impl Sessao {
    pub fn nova(chamadas: Vec<Chamada>, config: ConfigChamada) -> Result<Self, ErroChamada> {
        config.validar()?;
        for (i, c) in chamadas.iter().enumerate() {
            if c.trecho.is_empty() || c.resposta_esperada().is_empty() {
                return Err(ErroChamada::ChamadaVazia(i));
            }
            let ruim = c
                .trecho
                .iter()
                .chain(c.resposta_esperada())
                .any(|e| !e.inicio_s.is_finite() || !e.duracao_s.is_finite());
            if ruim {
                return Err(ErroChamada::ChamadaComTempoInvalido(i));
            }
        }
        Ok(Self {
            chamadas,
            config,
            fase: Fase::Pronto,
            atual: 0,
            tentativa: 1,
            tocadas: Vec::new(),
            resultados: Vec::new(),
        })
    }

    pub fn config(&self) -> ConfigChamada {
        self.config
    }

    pub fn chamadas(&self) -> &[Chamada] {
        &self.chamadas
    }

    pub fn estado(&self) -> Estado {
        match self.fase {
            Fase::Pronto => Estado::Pronto,
            Fase::Anunciando { .. } => Estado::Anunciando,
            Fase::Professor { .. } => Estado::ProfessorTocando,
            Fase::Aguardando { .. } => Estado::AguardandoAluno,
            Fase::Acertou { .. } => Estado::Acertou,
            Fase::Errou { .. } => Estado::Errou,
            Fase::Concluido => Estado::Concluido,
        }
    }

    /// Índice (0-based) da chamada em curso; `None` antes de iniciar ou depois de concluir.
    pub fn chamada_atual(&self) -> Option<usize> {
        match self.fase {
            Fase::Pronto | Fase::Concluido => None,
            _ => Some(self.atual),
        }
    }

    /// Tentativa em curso da chamada atual (1-based).
    pub fn tentativa_atual(&self) -> u32 {
        self.tentativa
    }

    pub fn concluida(&self) -> bool {
        self.fase == Fase::Concluido
    }

    pub fn resumo(&self) -> Resumo {
        Resumo {
            resultados: self.resultados.clone(),
            total: self.chamadas.len(),
        }
    }

    /// Começa a sessão em `agora_s`. Chamar de novo é inofensivo (não faz nada).
    pub fn iniciar(&mut self, agora_s: f64) -> Vec<Acao> {
        let mut acoes = Vec::new();
        if self.fase != Fase::Pronto || !agora_s.is_finite() {
            return acoes;
        }
        self.atual = 0;
        if self.chamadas.is_empty() {
            self.fase = Fase::Concluido;
            acoes.push(Acao::Concluido(self.resumo()));
            return acoes;
        }
        self.entrar_na_chamada(agora_s, &mut acoes);
        self.processar(agora_s, &mut acoes);
        acoes
    }

    /// A camada de app avisa que a fala do anúncio acabou. Sem efeito fora de `Anunciando`.
    pub fn fala_terminou(&mut self, agora_s: f64) -> Vec<Acao> {
        let mut acoes = Vec::new();
        if !agora_s.is_finite() {
            return acoes;
        }
        if matches!(self.fase, Fase::Anunciando { .. }) {
            self.entrar_no_professor(agora_s, &mut acoes);
        }
        self.processar(agora_s, &mut acoes);
        acoes
    }

    /// O aluno tocou `midi` em `agora_s`. Só vale enquanto se espera a resposta; antes disso
    /// (anúncio, professor tocando) e durante a pausa de feedback, a nota é ignorada.
    pub fn nota_tocada(&mut self, midi: u8, agora_s: f64) -> Vec<Acao> {
        let mut acoes = Vec::new();
        if !agora_s.is_finite() {
            return acoes;
        }
        // Se o prazo de alguma etapa já passou, primeiro resolve (a nota pode ter chegado
        // depois do fim da espera, ou do silêncio que fecharia a resposta anterior).
        self.processar(agora_s, &mut acoes);
        if matches!(self.fase, Fase::Aguardando { .. }) {
            self.tocadas.push((midi, agora_s));
        }
        acoes
    }

    /// Faz o tempo andar até `agora_s` (chame a cada quadro).
    pub fn avancar(&mut self, agora_s: f64) -> Vec<Acao> {
        let mut acoes = Vec::new();
        if agora_s.is_finite() {
            self.processar(agora_s, &mut acoes);
        }
        acoes
    }

    // --- internos ---

    fn processar(&mut self, agora_s: f64, acoes: &mut Vec<Acao>) {
        loop {
            match self.fase {
                Fase::Anunciando { ate_s: Some(ate) } if agora_s >= ate => {
                    self.entrar_no_professor(ate, acoes);
                }
                Fase::Professor { ate_s } if agora_s >= ate_s => {
                    self.tocadas.clear();
                    self.fase = Fase::Aguardando { desde_s: ate_s };
                }
                Fase::Aguardando { desde_s } => match self.fim_da_resposta(desde_s, agora_s) {
                    Some(t) => self.avaliar(t, acoes),
                    None => break,
                },
                Fase::Acertou { ate_s } if agora_s >= ate_s => {
                    self.passar_para_a_proxima(ate_s, acoes);
                }
                Fase::Errou { ate_s, repete } if agora_s >= ate_s => {
                    if repete {
                        self.tentativa += 1;
                        self.entrar_no_professor(ate_s, acoes);
                    } else {
                        self.passar_para_a_proxima(ate_s, acoes);
                    }
                }
                _ => break,
            }
        }
    }

    /// Entra na chamada `self.atual`, na primeira tentativa, com anúncio se houver.
    fn entrar_na_chamada(&mut self, t: f64, acoes: &mut Vec<Acao>) {
        self.tentativa = 1;
        match self.chamadas[self.atual].anuncio.clone() {
            Some(texto) if !texto.trim().is_empty() => {
                let ate_s = self
                    .config
                    .anuncio_por_estimativa
                    .then(|| t + estimar_duracao_da_fala_s(&texto));
                acoes.push(Acao::Falar(texto));
                self.fase = Fase::Anunciando { ate_s };
            }
            _ => self.entrar_no_professor(t, acoes),
        }
    }

    fn entrar_no_professor(&mut self, t: f64, acoes: &mut Vec<Acao>) {
        let notas = normalizar(&self.chamadas[self.atual].trecho);
        let ate_s = t + duracao_do_trecho(&notas) + self.config.folga_apos_professor_s;
        acoes.push(Acao::TocarNotas(notas));
        self.fase = Fase::Professor { ate_s };
    }

    /// Instante (nominal) em que a resposta termina, se já terminou até `agora_s`.
    fn fim_da_resposta(&self, desde_s: f64, agora_s: f64) -> Option<f64> {
        let cfg = &self.config;
        let Some(&(_, primeira)) = self.tocadas.first() else {
            let limite = desde_s + cfg.espera_inicio_s;
            return (agora_s >= limite).then_some(limite);
        };
        let ultima = self.tocadas.iter().map(|t| t.1).fold(primeira, f64::max);
        let esperada = normalizar(self.chamadas[self.atual].resposta_esperada());
        let silencio = cfg
            .silencio_fim_s
            .max(1.5 * maior_intervalo_entre_inicios(&esperada));
        let completa = self.tocadas.len() >= esperada.len();
        let limite_silencio = if completa {
            silencio.min(cfg.silencio_completo_s)
        } else {
            silencio
        };
        // Teto: ninguém leva mais que o dobro do trecho (mais 5 s) para responder.
        let teto = primeira + 2.0 * duracao_do_trecho(&esperada) + 5.0;
        let fim = (ultima + limite_silencio).min(teto);
        (agora_s >= fim).then_some(fim)
    }

    fn avaliar(&mut self, t: f64, acoes: &mut Vec<Acao>) {
        let chamada = &self.chamadas[self.atual];
        let esperados = normalizar(chamada.resposta_esperada());
        let primeira = self.tocadas.first().map_or(0.0, |x| x.1);
        let mut tocadas: Vec<NotaTocada> = self
            .tocadas
            .iter()
            .map(|&(midi, inicio)| NotaTocada {
                midi,
                inicio_s: inicio - primeira,
            })
            .collect();
        if chamada.modo == ModoResposta::PorClasse {
            for n in &mut tocadas {
                n.midi = nota_esperada_da_mesma_classe(&esperados, n.midi);
            }
        }
        let comparacao = comparar(&esperados, &tocadas, self.config.comparacao)
            .unwrap_or_else(|_| tudo_faltou(esperados.len(), tocadas.len()));
        let motivo = classificar(&esperados, &tocadas, &comparacao);
        let tolera_extras = self.config.aceita_notas_extras
            && comparacao.faltas.is_empty()
            && !comparacao.extras.is_empty();
        let acertou = !tocadas.is_empty() && (comparacao.perfeita() || tolera_extras);
        let motivo = if acertou { Motivo::Acertou } else { motivo };
        let repete = !acertou && self.tentativa < self.config.max_tentativas;
        let detalhes = Detalhes {
            chamada: self.atual,
            tentativa: self.tentativa,
            max_tentativas: self.config.max_tentativas,
            motivo,
            esperadas: esperados.len(),
            tocadas: tocadas.len(),
            acertos: comparacao.acertos.len(),
            faltaram: comparacao
                .faltas
                .iter()
                .map(|&i| esperados[i].midi)
                .collect(),
            sobraram: comparacao.extras.iter().map(|&i| tocadas[i].midi).collect(),
            desvio_medio_ms: comparacao.desvio_medio_ms(),
            vai_repetir: repete,
            esgotou: !acertou && !repete,
        };
        acoes.push(Acao::Feedback { acertou, detalhes });
        let ate_s = t + self.config.pausa_feedback_s;
        self.tocadas.clear();
        if acertou {
            self.resultados.push(ResultadoChamada {
                acertou: true,
                tentativas: self.tentativa,
            });
            self.fase = Fase::Acertou { ate_s };
        } else {
            if !repete {
                self.resultados.push(ResultadoChamada {
                    acertou: false,
                    tentativas: self.tentativa,
                });
            }
            self.fase = Fase::Errou { ate_s, repete };
        }
    }

    fn passar_para_a_proxima(&mut self, t: f64, acoes: &mut Vec<Acao>) {
        if self.atual + 1 < self.chamadas.len() {
            self.atual += 1;
            self.entrar_na_chamada(t, acoes);
        } else {
            self.fase = Fase::Concluido;
            acoes.push(Acao::Concluido(self.resumo()));
        }
    }
}

fn tudo_faltou(esperados: usize, tocadas: usize) -> Comparacao {
    Comparacao {
        acertos: Vec::new(),
        faltas: (0..esperados).collect(),
        extras: (0..tocadas).collect(),
    }
}

/// Leva `midi` à nota esperada de mesma classe de altura mais próxima (ou o próprio `midi`).
fn nota_esperada_da_mesma_classe(esperados: &[EventoEsperado], midi: u8) -> u8 {
    esperados
        .iter()
        .filter(|e| e.midi % 12 == midi % 12)
        .min_by_key(|e| e.midi.abs_diff(midi))
        .map_or(midi, |e| e.midi)
}

fn classificar(esperados: &[EventoEsperado], tocadas: &[NotaTocada], c: &Comparacao) -> Motivo {
    if tocadas.is_empty() {
        return Motivo::NaoTocou;
    }
    match (c.faltas.is_empty(), c.extras.is_empty()) {
        (true, true) => Motivo::Acertou,
        (false, true) => Motivo::NotasFaltando,
        (true, false) => Motivo::NotaExtra,
        (false, false) => {
            let fora_do_tempo = c.faltas.iter().any(|&f| {
                c.extras
                    .iter()
                    .any(|&x| tocadas[x].midi == esperados[f].midi)
            });
            if fora_do_tempo {
                Motivo::ForaDoTempo
            } else {
                Motivo::NotaErrada
            }
        }
    }
}

// --- Geradores ------------------------------------------------------------------------------

/// Quebra os eventos esperados de uma partitura em **frases** de `compassos_por_frase`
/// compassos e faz de cada uma uma chamada (o professor toca a frase, o aluno a repete).
///
/// Os eventos vêm de [`crate::comparador::esperados_da_partitura`]; filtre antes por mão/parte
/// se quiser só uma. Compassos são lidos na ordem de execução (repetições expandidas contam
/// como compassos novos). O anúncio é "Frase N. Ouça e repita." e a primeira frase abre com
/// "Vamos tocar em chamada e resposta."; passe `anunciar = false` para chamadas mudas.
pub fn chamadas_de_frases(
    esperados: &[EventoEsperado],
    compassos_por_frase: usize,
    anunciar: bool,
) -> Vec<Chamada> {
    let por_frase = compassos_por_frase.max(1);
    let mut ordenados: Vec<EventoEsperado> = esperados.to_vec();
    ordenados.sort_by(|a, b| a.inicio_s.total_cmp(&b.inicio_s).then(a.midi.cmp(&b.midi)));
    // Corridas consecutivas do mesmo compasso (a ordem de execução, não o número escrito).
    let mut corridas: Vec<Vec<EventoEsperado>> = Vec::new();
    let mut ultimo: Option<usize> = None;
    for e in ordenados {
        if ultimo != Some(e.compasso) {
            corridas.push(Vec::new());
            ultimo = Some(e.compasso);
        }
        if let Some(c) = corridas.last_mut() {
            c.push(e);
        }
    }
    corridas
        .chunks(por_frase)
        .enumerate()
        .map(|(i, grupo)| {
            let trecho: Vec<EventoEsperado> = grupo.iter().flatten().copied().collect();
            let chamada = Chamada::nova(normalizar(&trecho));
            if !anunciar {
                chamada
            } else if i == 0 {
                chamada.com_anuncio("Vamos tocar em chamada e resposta. Frase 1. Ouça e repita.")
            } else {
                chamada.com_anuncio(format!("Frase {}. Ouça e repita.", i + 1))
            }
        })
        .collect()
}

/// Uma chamada por acorde: "Agora o acorde de Lá." O professor toca o acorde (todas as notas
/// juntas, `duracao_s`) a partir de `base` e o aluno o repete em **qualquer oitava e
/// inversão** (resposta por classe de altura).
///
/// Convenção do curso: o Dó central (MIDI 60) é o Dó3; use `base` = Dó3 (60) para o acorde
/// soar na região do Dó central. A primeira chamada abre com "Vamos tocar em chamada e resposta."
pub fn chamadas_de_acordes(
    acordes: &[Acorde],
    base: NotaMidi,
    duracao_s: f64,
    anunciar: bool,
) -> Vec<Chamada> {
    acordes
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let notas: Vec<u8> = a.notas(base, 0).iter().map(|n| n.numero()).collect();
            let chamada = Chamada::de_acorde(&notas, duracao_s).por_classe();
            if !anunciar {
                return chamada;
            }
            let fala = format!("Agora o acorde de {}.", nome_falado_do_acorde(a));
            if i == 0 {
                chamada.com_anuncio(format!("Vamos tocar em chamada e resposta. {fala}"))
            } else {
                chamada.com_anuncio(fala)
            }
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::teoria::ClasseAltura;

    const DO3: u8 = 60;
    const RE3: u8 = 62;
    const MI3: u8 = 64;
    const SOL3: u8 = 67;

    fn config() -> ConfigChamada {
        ConfigChamada {
            max_tentativas: 3,
            comparacao: ConfigComparacao {
                tolerancia_ms: 200.0,
                compensacao_ms: 0.0,
            },
            espera_inicio_s: 10.0,
            silencio_fim_s: 1.5,
            silencio_completo_s: 0.5,
            folga_apos_professor_s: 0.5,
            pausa_feedback_s: 1.0,
            anuncio_por_estimativa: false,
            aceita_notas_extras: false,
        }
    }

    /// Frase Dó–Mi–Sol, 0,5 s cada: o professor termina em 1,5 s (+0,5 de folga = 2,0 s).
    fn frase() -> Chamada {
        Chamada::de_notas(&[(DO3, 0.5), (MI3, 0.5), (SOL3, 0.5)]).com_anuncio("Ouça e repita.")
    }

    fn sessao(chamadas: Vec<Chamada>) -> Sessao {
        Sessao::nova(chamadas, config()).expect("válida")
    }

    /// Inicia no instante 0 e leva a sessão até `AguardandoAluno` (que começa em t = 2,0 s).
    fn ate_aguardar(s: &mut Sessao) -> Vec<Acao> {
        let mut acoes = s.iniciar(0.0);
        acoes.extend(s.fala_terminou(0.2));
        // Professor tocou de 0,2 s a 1,7 s; folga até 2,2 s.
        acoes.extend(s.avancar(2.3));
        assert_eq!(s.estado(), Estado::AguardandoAluno);
        acoes
    }

    fn feedbacks(acoes: &[Acao]) -> Vec<(bool, &Detalhes)> {
        acoes
            .iter()
            .filter_map(|a| match a {
                Acao::Feedback { acertou, detalhes } => Some((*acertou, detalhes)),
                _ => None,
            })
            .collect()
    }

    /// O aluno toca Dó–Mi–Sol a partir de `t0`, com `passo` s entre as notas.
    fn tocar_frase(s: &mut Sessao, t0: f64, passo: f64) -> Vec<Acao> {
        let mut acoes = Vec::new();
        for (i, m) in [DO3, MI3, SOL3].into_iter().enumerate() {
            acoes.extend(s.nota_tocada(m, t0 + passo * i as f64));
        }
        acoes
    }

    #[test]
    fn acerto_direto_percorre_todos_os_estados() {
        let mut s = sessao(vec![frase()]);
        assert_eq!(s.estado(), Estado::Pronto);
        assert_eq!(s.chamada_atual(), None);

        let acoes = s.iniciar(0.0);
        assert_eq!(acoes, vec![Acao::Falar("Ouça e repita.".to_owned())]);
        assert_eq!(s.estado(), Estado::Anunciando);

        // Sem estimativa: só avança quando a fala termina.
        assert!(s.avancar(30.0).is_empty());
        assert_eq!(s.estado(), Estado::Anunciando);
        let acoes = s.fala_terminou(0.2);
        assert_eq!(s.estado(), Estado::ProfessorTocando);
        let Acao::TocarNotas(notas) = &acoes[0] else {
            panic!("esperava TocarNotas, veio {acoes:?}");
        };
        assert_eq!(
            notas.iter().map(|n| n.midi).collect::<Vec<_>>(),
            [DO3, MI3, SOL3]
        );
        assert_eq!(notas[0].inicio_s, 0.0);
        assert!((notas[2].inicio_s - 1.0).abs() < 1e-9);

        // O aluno não pode atropelar o professor: a nota é ignorada.
        assert!(s.nota_tocada(DO3, 1.0).is_empty());
        assert_eq!(s.estado(), Estado::ProfessorTocando);

        s.avancar(2.3);
        assert_eq!(s.estado(), Estado::AguardandoAluno);
        // O aluno começa quando quiser: 4 s depois, andamento um pouco diferente do tempo
        // do professor? Não: a avaliação é relativa à primeira nota (0, 0,5, 1,0 s).
        tocar_frase(&mut s, 6.0, 0.5);
        assert_eq!(s.estado(), Estado::AguardandoAluno);
        // Notas suficientes: o silêncio curto (0,5 s) basta.
        let acoes = s.avancar(7.6);
        let fb = feedbacks(&acoes);
        assert_eq!(fb.len(), 1);
        assert!(fb[0].0);
        assert_eq!(fb[0].1.motivo, Motivo::Acertou);
        assert_eq!(fb[0].1.acertos, 3);
        assert_eq!(fb[0].1.tentativa, 1);
        assert!(!fb[0].1.vai_repetir && !fb[0].1.esgotou);
        assert_eq!(s.estado(), Estado::Acertou);

        // Depois da pausa, não há mais chamadas.
        let acoes = s.avancar(9.0);
        assert_eq!(s.estado(), Estado::Concluido);
        assert!(s.concluida());
        let Some(Acao::Concluido(r)) = acoes.last() else {
            panic!("esperava Concluido, veio {acoes:?}");
        };
        assert_eq!(r.acertadas(), 1);
        assert_eq!(r.acertadas_de_primeira(), 1);
        assert_eq!(r.total, 1);
    }

    #[test]
    fn tempo_da_resposta_e_relativo_a_primeira_nota_do_aluno() {
        // O aluno toca bem mais devagar que o professor: 0,5 s entre notas contra 0,5 do trecho
        // não muda; com 0,6 s a 3ª nota fica 200 ms atrasada (no limite) e ainda passa.
        let mut s = sessao(vec![frase()]);
        ate_aguardar(&mut s);
        tocar_frase(&mut s, 9.9, 0.6);
        let acoes = s.avancar(20.0);
        assert!(feedbacks(&acoes)[0].0);
        // Muito mais lento (0,8 s): a 3ª nota estoura a tolerância.
        let mut s = sessao(vec![frase()]);
        ate_aguardar(&mut s);
        tocar_frase(&mut s, 3.0, 0.8);
        let acoes = s.avancar(20.0);
        let fb = feedbacks(&acoes);
        assert!(!fb[0].0);
        assert_eq!(fb[0].1.motivo, Motivo::ForaDoTempo);
    }

    #[test]
    fn erro_repete_a_chamada_sem_repetir_o_anuncio() {
        let mut s = sessao(vec![frase()]);
        ate_aguardar(&mut s);
        // Nota errada no meio.
        s.nota_tocada(DO3, 3.0);
        s.nota_tocada(RE3, 3.5);
        s.nota_tocada(SOL3, 4.0);
        let acoes = s.avancar(5.0);
        let fb = feedbacks(&acoes);
        assert!(!fb[0].0);
        assert_eq!(fb[0].1.motivo, Motivo::NotaErrada);
        assert_eq!(fb[0].1.faltaram, [MI3]);
        assert_eq!(fb[0].1.sobraram, [RE3]);
        assert!(fb[0].1.vai_repetir && !fb[0].1.esgotou);
        assert_eq!(s.estado(), Estado::Errou);
        assert_eq!(s.tentativa_atual(), 1);

        // Depois da pausa de feedback, o professor toca de novo (sem Falar).
        let acoes = s.avancar(6.1);
        assert_eq!(s.estado(), Estado::ProfessorTocando);
        assert_eq!(s.tentativa_atual(), 2);
        assert!(matches!(acoes.as_slice(), [Acao::TocarNotas(_)]));

        // Segunda tentativa certa.
        s.avancar(9.0);
        assert_eq!(s.estado(), Estado::AguardandoAluno);
        tocar_frase(&mut s, 10.0, 0.5);
        let acoes = s.avancar(12.0);
        assert!(feedbacks(&acoes)[0].0);
        assert_eq!(feedbacks(&acoes)[0].1.tentativa, 2);
        s.avancar(14.0);
        let r = s.resumo();
        assert_eq!(r.acertadas(), 1);
        assert_eq!(r.acertadas_de_primeira(), 0);
        assert_eq!(r.tentativas_totais(), 2);
    }

    #[test]
    fn limite_de_tentativas_esgota_e_segue_para_a_proxima() {
        let outra = Chamada::de_notas(&[(RE3, 0.5)]).com_anuncio("Agora uma nota.");
        let mut cfg = config();
        cfg.max_tentativas = 2;
        let mut s = Sessao::nova(vec![frase(), outra], cfg).expect("válida");
        s.iniciar(0.0);
        s.fala_terminou(0.0);
        // Tentativa 1: professor até 2,0 s; o aluno toca só a primeira nota em 3,5 s; a
        // resposta fecha por silêncio em 5,0 s; a pausa de feedback vai até 6,0 s.
        s.avancar(2.1);
        assert_eq!(s.estado(), Estado::AguardandoAluno);
        s.nota_tocada(DO3, 3.5);
        let acoes = s.avancar(5.1);
        let fb = feedbacks(&acoes);
        assert_eq!(fb[0].1.motivo, Motivo::NotasFaltando);
        assert!(fb[0].1.vai_repetir && !fb[0].1.esgotou);
        let acoes = s.avancar(6.1);
        assert_eq!(s.tentativa_atual(), 2);
        assert!(matches!(acoes.as_slice(), [Acao::TocarNotas(_)]));
        // Tentativa 2: professor de 6,0 a 8,0 s; de novo só uma nota.
        s.avancar(8.1);
        assert_eq!(s.estado(), Estado::AguardandoAluno);
        s.nota_tocada(DO3, 8.5);
        let acoes = s.avancar(10.1);
        let fb = feedbacks(&acoes);
        assert!(!fb[0].1.vai_repetir && fb[0].1.esgotou);
        // Esgotou: vai para a chamada 2 (com anúncio), sem ficar presa.
        let acoes = s.avancar(11.1);
        assert_eq!(s.chamada_atual(), Some(1));
        assert_eq!(s.tentativa_atual(), 1);
        assert_eq!(s.estado(), Estado::Anunciando);
        assert_eq!(acoes, vec![Acao::Falar("Agora uma nota.".to_owned())]);
        let r = s.resumo();
        assert_eq!(r.nao_resolvidas(), 1);
        assert_eq!(r.resultados[0].tentativas, 2);
    }

    #[test]
    fn aluno_que_nao_toca_esgota_o_tempo_de_espera() {
        let mut s = sessao(vec![frase()]);
        ate_aguardar(&mut s); // a espera começa em 2,2 s
        assert!(s.avancar(12.0).is_empty()); // 9,8 s de espera: ainda dentro de 10 s
        let acoes = s.avancar(12.3);
        let fb = feedbacks(&acoes);
        assert_eq!(fb.len(), 1);
        assert!(!fb[0].0);
        assert_eq!(fb[0].1.motivo, Motivo::NaoTocou);
        assert_eq!(fb[0].1.tocadas, 0);
        assert_eq!(fb[0].1.faltaram, [DO3, MI3, SOL3]);
        assert!(fb[0].1.vai_repetir);
        assert_eq!(s.estado(), Estado::Errou);
    }

    #[test]
    fn nota_extra_reprova_a_menos_que_a_configuracao_aceite() {
        let mut s = sessao(vec![frase()]);
        ate_aguardar(&mut s);
        tocar_frase(&mut s, 3.0, 0.5);
        s.nota_tocada(72, 4.2); // nota a mais, logo depois
        let acoes = s.avancar(10.0);
        let fb = feedbacks(&acoes);
        assert!(!fb[0].0);
        assert_eq!(fb[0].1.motivo, Motivo::NotaExtra);
        assert_eq!(fb[0].1.sobraram, [72]);
        assert_eq!(fb[0].1.acertos, 3);

        let mut cfg = config();
        cfg.aceita_notas_extras = true;
        let mut s = Sessao::nova(vec![frase()], cfg).expect("válida");
        ate_aguardar(&mut s);
        tocar_frase(&mut s, 3.0, 0.5);
        s.nota_tocada(72, 4.2);
        let acoes = s.avancar(10.0);
        let fb = feedbacks(&acoes);
        assert!(fb[0].0);
        assert_eq!(fb[0].1.sobraram, [72]); // continua listada
    }

    #[test]
    fn acorde_espalhado_fora_da_janela_nao_vale() {
        let chamada =
            Chamada::de_acorde(&[DO3, MI3, SOL3], 1.0).com_anuncio("Agora o acorde de Dó.");
        let mut s = sessao(vec![chamada]);
        s.iniciar(0.0);
        s.fala_terminou(0.0);
        s.avancar(2.0); // 1,0 s de acorde + 0,5 de folga = 1,5 s
        assert_eq!(s.estado(), Estado::AguardandoAluno);
        // Juntas (dentro de 200 ms): passa.
        s.nota_tocada(DO3, 3.00);
        s.nota_tocada(MI3, 3.08);
        s.nota_tocada(SOL3, 3.15);
        let acoes = s.avancar(5.0);
        assert!(feedbacks(&acoes)[0].0);

        // Espalhadas (arpejo lento, 0,6 s entre notas): reprova por tempo.
        let chamada = Chamada::de_acorde(&[DO3, MI3, SOL3], 1.0);
        let mut s = sessao(vec![chamada]);
        s.iniciar(0.0);
        s.avancar(2.0);
        s.nota_tocada(DO3, 3.0);
        s.nota_tocada(MI3, 3.6);
        s.nota_tocada(SOL3, 4.2);
        let acoes = s.avancar(9.0);
        let fb = feedbacks(&acoes);
        assert!(!fb[0].0);
        assert_eq!(fb[0].1.motivo, Motivo::ForaDoTempo);
        assert_eq!(fb[0].1.acertos, 1);
        assert_eq!(fb[0].1.faltaram, [MI3, SOL3]);
    }

    #[test]
    fn resposta_por_classe_aceita_inversao_e_outra_oitava() {
        let chamada = Chamada::de_acorde(&[DO3, MI3, SOL3], 1.0).por_classe();
        let mut s = sessao(vec![chamada]);
        s.iniciar(0.0);
        s.avancar(2.0);
        // 1ª inversão uma oitava acima: Mi, Sol, Dó.
        s.nota_tocada(MI3 + 12, 3.0);
        s.nota_tocada(SOL3 + 12, 3.02);
        s.nota_tocada(DO3 + 24, 3.04);
        let acoes = s.avancar(6.0);
        assert!(feedbacks(&acoes)[0].0);
        // Exato não aceitaria.
        let chamada = Chamada::de_acorde(&[DO3, MI3, SOL3], 1.0);
        let mut s = sessao(vec![chamada]);
        s.iniciar(0.0);
        s.avancar(2.0);
        s.nota_tocada(MI3 + 12, 3.0);
        s.nota_tocada(SOL3 + 12, 3.02);
        s.nota_tocada(DO3 + 24, 3.04);
        let acoes = s.avancar(6.0);
        assert!(!feedbacks(&acoes)[0].0);
    }

    #[test]
    fn resposta_diferente_do_trecho_e_pausa_no_meio_nao_encerram_cedo() {
        // O professor toca Dó; o aluno deve responder Sol (resposta própria).
        let c = Chamada::de_notas(&[(DO3, 0.5)]).com_resposta(vec![evento(SOL3, 0.0, 0.5, 0)]);
        let mut s = sessao(vec![c]);
        s.iniciar(0.0);
        s.avancar(2.0);
        s.nota_tocada(SOL3, 2.5);
        let acoes = s.avancar(4.0);
        assert!(feedbacks(&acoes)[0].0);

        // Frase com pausa de 2 s entre as notas: o silêncio de 1,5 s não pode fechar a resposta
        // antes da 2ª nota (o limite sobe para 1,5 x 2 s = 3 s).
        let c = Chamada::nova(vec![evento(DO3, 0.0, 0.5, 0), evento(MI3, 2.0, 0.5, 0)]);
        let mut s = sessao(vec![c]);
        s.iniciar(0.0);
        s.avancar(4.0);
        assert_eq!(s.estado(), Estado::AguardandoAluno);
        s.nota_tocada(DO3, 5.0);
        assert!(s.avancar(7.5).is_empty()); // 2,5 s de silêncio, ainda esperando
        s.nota_tocada(MI3, 7.0);
        let acoes = s.avancar(9.0);
        assert!(feedbacks(&acoes)[0].0);
    }

    #[test]
    fn nota_que_chega_depois_do_prazo_nao_reabre_a_resposta_anterior() {
        let mut s = sessao(vec![frase()]);
        ate_aguardar(&mut s);
        tocar_frase(&mut s, 3.0, 0.5);
        // Ninguém chamou `avancar` por 20 s; a próxima nota chega depois do fim da resposta.
        let acoes = s.nota_tocada(DO3, 23.0);
        assert!(feedbacks(&acoes)[0].0); // a resposta anterior foi avaliada primeiro
        // A nota tardia não entra na resposta já fechada; a sessão segue o seu curso.
        assert_eq!(s.estado(), Estado::Concluido);
        assert_eq!(s.resumo().acertadas_de_primeira(), 1);
    }

    #[test]
    fn anuncio_por_estimativa_avanca_sem_aviso_da_voz() {
        let mut cfg = config();
        cfg.anuncio_por_estimativa = true;
        let mut s = Sessao::nova(vec![frase()], cfg).expect("válida");
        s.iniciar(0.0);
        assert_eq!(s.estado(), Estado::Anunciando);
        // "Ouça e repita." (14 caracteres): 0,8 + 14 x 0,06 = 1,64 s.
        assert!(s.avancar(1.5).is_empty());
        let acoes = s.avancar(1.7);
        assert!(matches!(acoes.as_slice(), [Acao::TocarNotas(_)]));
        assert_eq!(s.estado(), Estado::ProfessorTocando);
    }

    #[test]
    fn chamada_sem_anuncio_comeca_direto_no_professor() {
        let c = Chamada::de_notas(&[(DO3, 0.5)]);
        let mut s = sessao(vec![c]);
        let acoes = s.iniciar(0.0);
        assert!(matches!(acoes.as_slice(), [Acao::TocarNotas(_)]));
        assert_eq!(s.estado(), Estado::ProfessorTocando);
        // Iniciar de novo não faz nada.
        assert!(s.iniciar(5.0).is_empty());
    }

    #[test]
    fn sessao_sem_chamadas_conclui_ao_iniciar_e_entradas_invalidas_sao_recusadas() {
        let mut s = sessao(vec![]);
        let acoes = s.iniciar(0.0);
        assert_eq!(s.estado(), Estado::Concluido);
        assert!(matches!(acoes.as_slice(), [Acao::Concluido(r)] if r.total == 0));

        let mut cfg = config();
        cfg.max_tentativas = 0;
        assert_eq!(
            Sessao::nova(vec![frase()], cfg).err(),
            Some(ErroChamada::SemTentativas)
        );
        let mut cfg = config();
        cfg.espera_inicio_s = f64::NAN;
        assert!(matches!(
            Sessao::nova(vec![frase()], cfg).err(),
            Some(ErroChamada::TempoInvalido("espera_inicio_s"))
        ));
        let mut cfg = config();
        cfg.comparacao.tolerancia_ms = -1.0;
        assert!(matches!(
            Sessao::nova(vec![frase()], cfg).err(),
            Some(ErroChamada::Comparacao(_))
        ));
        assert_eq!(
            Sessao::nova(vec![Chamada::nova(vec![])], config()).err(),
            Some(ErroChamada::ChamadaVazia(0))
        );
        let ruim = Chamada::nova(vec![evento(DO3, f64::NAN, 0.5, 0)]);
        assert_eq!(
            Sessao::nova(vec![ruim], config()).err(),
            Some(ErroChamada::ChamadaComTempoInvalido(0))
        );
        // Tempo não finito nas entradas é ignorado.
        let mut s = sessao(vec![frase()]);
        assert!(s.iniciar(f64::NAN).is_empty());
        assert_eq!(s.estado(), Estado::Pronto);
    }

    #[test]
    fn varias_chamadas_seguem_em_ordem_e_o_resumo_conta() {
        let c2 = Chamada::de_notas(&[(RE3, 0.5)]).com_anuncio("Agora o Ré.");
        let mut s = sessao(vec![frase(), c2]);
        ate_aguardar(&mut s);
        tocar_frase(&mut s, 3.0, 0.5);
        s.avancar(5.0); // feedback certo em 4,5 s (+ pausa 1 s = 5,5 s)
        assert_eq!(s.estado(), Estado::Acertou);
        let acoes = s.avancar(5.6);
        assert_eq!(acoes, vec![Acao::Falar("Agora o Ré.".to_owned())]);
        assert_eq!(s.chamada_atual(), Some(1));
        s.fala_terminou(5.7);
        s.avancar(7.0);
        s.nota_tocada(RE3, 8.0);
        s.avancar(9.0);
        s.avancar(11.0);
        assert!(s.concluida());
        let r = s.resumo();
        assert_eq!((r.total, r.acertadas(), r.nao_resolvidas()), (2, 2, 0));
        assert_eq!(r.acertadas_de_primeira(), 2);
    }

    // --- nomes e geradores ---

    #[test]
    fn nomes_na_convencao_do_curso() {
        assert_eq!(nome_da_nota_no_curso(60), "Dó3");
        assert_eq!(nome_da_nota_no_curso(69), "Lá3");
        assert_eq!(nome_da_nota_no_curso(61), "Dó#3");
        assert_eq!(nome_da_nota_no_curso(48), "Dó2");
        assert_eq!(nome_da_nota_no_curso(200), "MIDI 200");
        assert_eq!(nome_falado_da_classe(ClasseAltura::nova(3)), "Mi bemol");
        assert_eq!(nome_falado_da_classe(ClasseAltura::nova(6)), "Fá sustenido");
    }

    #[test]
    fn gerador_de_acordes_anuncia_em_portugues() {
        let la = Acorde::novo(ClasseAltura::nova(9), Qualidade::Maior);
        let la_menor = Acorde::novo(ClasseAltura::nova(9), Qualidade::Menor);
        let mib7 = Acorde::novo(ClasseAltura::nova(3), Qualidade::SetimaDominante);
        let base = NotaMidi::nova(60).expect("MIDI válido");
        let c = chamadas_de_acordes(&[la, la_menor, mib7], base, 1.0, true);
        assert_eq!(c.len(), 3);
        assert_eq!(
            c[0].anuncio.as_deref(),
            Some("Vamos tocar em chamada e resposta. Agora o acorde de Lá.")
        );
        assert_eq!(c[1].anuncio.as_deref(), Some("Agora o acorde de Lá menor."));
        assert_eq!(
            c[2].anuncio.as_deref(),
            Some("Agora o acorde de Mi bemol com sétima.")
        );
        // Lá maior a partir do Dó3 (60): Lá 69, Dó# 73, Mi 76; todas juntas, por classe.
        let notas: Vec<u8> = c[0].trecho.iter().map(|e| e.midi).collect();
        assert_eq!(notas, [69, 73, 76]);
        assert!(c[0].trecho.iter().all(|e| e.inicio_s == 0.0));
        assert_eq!(c[0].modo, ModoResposta::PorClasse);
        // Mudas.
        let c = chamadas_de_acordes(&[la], base, 1.0, false);
        assert_eq!(c[0].anuncio, None);
    }

    fn evento_em(midi: u8, t: f64, compasso: usize) -> EventoEsperado {
        evento(midi, t, 0.4, compasso)
    }

    #[test]
    fn frases_fundem_unissono_entre_maos_e_resposta_pede_uma_tecla() {
        // Dó4 nas duas mãos no mesmo instante, mais um Mi4.
        let e = [
            evento_em(60, 0.0, 0),
            evento(60, 0.0, 2.0, 0),
            evento_em(64, 1.0, 0),
        ];
        let c = chamadas_de_frases(&e, 1, false);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].trecho.len(), 2);
        assert!((c[0].trecho[0].duracao_s - 2.0).abs() < 1e-12);
        assert_eq!(normalizar(&e).len(), 2);
    }

    #[test]
    fn gerador_de_frases_agrupa_por_compassos_e_normaliza_o_tempo() {
        // 4 compassos de 2 s com duas notas cada.
        let mut e = Vec::new();
        for c in 0..4usize {
            let t = c as f64 * 2.0;
            e.push(evento_em(60 + c as u8, t, c));
            e.push(evento_em(64 + c as u8, t + 1.0, c));
        }
        // Frases de 2 compassos: 2 chamadas.
        let c = chamadas_de_frases(&e, 2, true);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].trecho.len(), 4);
        assert_eq!(c[1].trecho.len(), 4);
        // A segunda frase começa em zero (o silêncio anterior não conta).
        assert_eq!(c[1].trecho[0].inicio_s, 0.0);
        assert_eq!(c[1].trecho[0].midi, 62);
        assert!((c[1].trecho[3].inicio_s - 3.0).abs() < 1e-9);
        assert_eq!(
            c[0].anuncio.as_deref(),
            Some("Vamos tocar em chamada e resposta. Frase 1. Ouça e repita.")
        );
        assert_eq!(c[1].anuncio.as_deref(), Some("Frase 2. Ouça e repita."));
        // Um compasso por frase, frase final incompleta, e 0 vale 1.
        assert_eq!(chamadas_de_frases(&e, 1, false).len(), 4);
        assert_eq!(chamadas_de_frases(&e, 3, false).len(), 2);
        assert_eq!(chamadas_de_frases(&e, 0, false).len(), 4);
        assert!(chamadas_de_frases(&e, 1, false)[0].anuncio.is_none());
        assert!(chamadas_de_frases(&[], 2, true).is_empty());
    }

    #[test]
    fn gerador_de_frases_com_partitura_real_e_sessao_completa() {
        use crate::comparador::esperados_da_partitura;
        use crate::partitura::Partitura;
        let ode = include_str!("../../../assets/partituras/beethoven-ode-a-alegria-tema.musicxml");
        let p = Partitura::de_musicxml(ode).expect("válida");
        let esperados = esperados_da_partitura(&p, 80.0, 1.0);
        let mao_direita: Vec<EventoEsperado> =
            esperados.iter().copied().filter(|e| e.pauta == 1).collect();
        let chamadas = chamadas_de_frases(&mao_direita, 2, true);
        assert!(chamadas.len() >= 2);
        let total_notas: usize = chamadas.iter().map(|c| c.trecho.len()).sum();
        assert_eq!(total_notas, mao_direita.len());

        // Um aluno perfeito acerta todas de primeira, na mesma cadência do professor.
        let n = chamadas.len();
        let esperada_por_chamada: Vec<Vec<EventoEsperado>> =
            chamadas.iter().map(|c| normalizar(&c.trecho)).collect();
        let mut s = Sessao::nova(chamadas, config()).expect("válida");
        let mut t = 0.0;
        s.iniciar(t);
        for resposta in &esperada_por_chamada {
            s.fala_terminou(t);
            t += duracao_do_trecho(resposta) + 1.0; // professor + folga
            s.avancar(t);
            assert_eq!(s.estado(), Estado::AguardandoAluno);
            t += 1.0;
            for e in resposta {
                s.nota_tocada(e.midi, t + e.inicio_s);
            }
            t += duracao_do_trecho(resposta) + 3.0;
            s.avancar(t);
            t += 2.0;
            s.avancar(t);
        }
        assert!(s.concluida(), "estado: {:?}", s.estado());
        let r = s.resumo();
        assert_eq!(r.acertadas_de_primeira(), n);
    }
}
