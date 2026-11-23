//! Professor assistido (F05): 10 exercícios progressivos de fábrica, classificador de
//! acertos e erros, feedback acessível (texto + símbolo, nunca só cor), progressão por
//! pré-requisito e progresso salvo em texto (`progresso.txt`).
//!
//! Regras do classificador:
//!
//! - cada **passo** do exercício é uma nota ou um conjunto de notas (intervalo, acorde);
//! - um conjunto é aceito quando todas as notas foram tocadas dentro da **janela temporal**
//!   configurável (padrão 150 ms); notas fora da janela não formam o conjunto;
//! - sequências e arpejos exigem a **ordem** dos passos;
//! - `NoteOff` (inclusive extra ou sem `NoteOn`) nunca penaliza;
//! - nota errada conta um erro, mas o aluno corrige e continua no mesmo passo;
//! - sem limite de tempo por padrão (o limite é opcional) e com pausa.
//!
//! Feedback de erro (F05-5):
//!
//! - **replay**: [`EstadoExercicios::tentativa_como_gravacao`] converte o registro da tentativa
//!   em uma `Gravacao` da F06 (o que o aluno tocou, com os instantes relativos à primeira
//!   nota); a tela a agenda no motor de som para o aluno "ouvir de novo";
//! - **compasso do erro**: [`EstadoExercicios::erros_localizados`] posiciona cada nota errada
//!   na grade de compasso do núcleo (`piano_tutor_core::ritmo::Agendador`, 4/4, andamento
//!   configurável, compasso 1 = instante da primeira nota tocada na tentativa).

use crate::gravacao::{EventoGravado, Gravacao, Mensagem};
use crate::licoes::{Licao, Origem, Situacao};
use piano_tutor_core::NotaMidi;
use piano_tutor_core::ritmo::{Agendador, BPM_MAX, BPM_MIN, Formula};
use piano_tutor_core::teclado::{ConfigNomes, rotulo};
use std::fmt::Write as _;
use std::ops::Range;
use std::path::PathBuf;

/// Janela padrão para aceitar um conjunto de notas como "juntas".
pub const JANELA_PADRAO_MS: u32 = 150;
/// Limites aceitos para a janela.
pub const JANELA_MIN_MS: u32 = 10;
pub const JANELA_MAX_MS: u32 = 2000;
/// Andamento padrão da grade que localiza o compasso do erro (bpm).
pub const BPM_GRADE_PADRAO: u16 = 60;
/// Fórmula de compasso da grade do feedback de erro.
pub const FORMULA_GRADE: Formula = Formula::QuatroQuatro;
/// Duração máxima de cada nota no replay da tentativa, em ns.
const NOTA_REPLAY_NS: u64 = 400_000_000;
/// Menor duração de uma nota do replay (a menos que a próxima venha antes), em ns.
const NOTA_REPLAY_MIN_NS: u64 = 1_000_000;
/// Velocidade das notas do replay da tentativa.
const VELOCIDADE_REPLAY: u8 = 80;
/// Teto do registro de eventos de uma tentativa.
const MAX_REGISTRO: usize = 2000;
/// Nome do arquivo de progresso dentro da pasta de dados.
pub const ARQUIVO_PROGRESSO: &str = "progresso.txt";
/// Nome do relatório exportado.
pub const ARQUIVO_EXPORTADO: &str = "progresso-exportado.txt";

fn nota_de(x: u8) -> NotaMidi {
    NotaMidi::nova(x.min(127)).expect("número MIDI limitado a 127")
}

/// Nome da nota para a tela (a fonte não tem ♯/♭: usa `#` e `b`).
pub fn nome_nota(n: NotaMidi) -> String {
    rotulo(n, ConfigNomes::default())
        .replace('♯', "#")
        .replace('♭', "b")
}

fn nomes(notas: &[NotaMidi]) -> String {
    notas
        .iter()
        .map(|n| nome_nota(*n))
        .collect::<Vec<_>>()
        .join(" ")
}

fn mascara(notas: &[NotaMidi]) -> u16 {
    notas.iter().fold(0u16, |m, n| m | 1 << (n.numero() % 12))
}

/// Tipo do exercício (rótulo e comportamento da tela).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tipo {
    Localizar,
    Sequencia,
    Intervalos,
    Escala,
    Acorde,
    Arpejo,
    OuvirERepetir,
}

impl Tipo {
    pub const fn texto(self) -> &'static str {
        match self {
            Tipo::Localizar => "localizar nota",
            Tipo::Sequencia => "notas em ordem",
            Tipo::Intervalos => "intervalos",
            Tipo::Escala => "escala",
            Tipo::Acorde => "acordes",
            Tipo::Arpejo => "arpejo",
            Tipo::OuvirERepetir => "ouvir e repetir",
        }
    }
}

/// Um passo do gabarito: uma nota ou um conjunto de notas a tocar juntas.
#[derive(Clone, Debug, PartialEq)]
pub struct Passo {
    pub notas: Vec<NotaMidi>,
    /// `true`: vale qualquer inversão e dobra de oitava (compara classes de altura).
    pub por_classe: bool,
    /// Nome do passo (por exemplo "C maior"); vazio usa os nomes das notas.
    pub legenda: String,
}

impl Passo {
    pub fn nota(x: u8) -> Self {
        Self {
            notas: vec![nota_de(x)],
            por_classe: false,
            legenda: String::new(),
        }
    }

    /// Conjunto de notas exatas (intervalos).
    pub fn exato(xs: &[u8], legenda: &str) -> Self {
        Self {
            notas: xs.iter().map(|x| nota_de(*x)).collect(),
            por_classe: false,
            legenda: legenda.to_owned(),
        }
    }

    /// Acorde: vale qualquer inversão e dobras de oitava.
    pub fn acorde(xs: &[u8], legenda: &str) -> Self {
        Self {
            por_classe: true,
            ..Self::exato(xs, legenda)
        }
    }

    pub fn descricao(&self) -> String {
        if self.legenda.is_empty() {
            nomes(&self.notas)
        } else {
            format!("{} ({})", self.legenda, nomes(&self.notas))
        }
    }

    fn valida(&self, n: NotaMidi) -> bool {
        if self.por_classe {
            mascara(&self.notas) & (1 << (n.numero() % 12)) != 0
        } else {
            self.notas.contains(&n)
        }
    }

    /// Confere as notas tocadas dentro da janela contra o passo.
    fn avaliar(&self, grupo: &[NotaMidi]) -> Avaliacao {
        if grupo.iter().any(|n| !self.valida(*n)) {
            return Avaliacao::Errada;
        }
        let faltam: Vec<NotaMidi> = if self.por_classe {
            let tocadas = mascara(grupo);
            let mut vistas = 0u16;
            self.notas
                .iter()
                .copied()
                .filter(|n| {
                    let b = 1u16 << (n.numero() % 12);
                    let falta = tocadas & b == 0 && vistas & b == 0;
                    vistas |= b;
                    falta
                })
                .collect()
        } else {
            self.notas
                .iter()
                .copied()
                .filter(|n| !grupo.contains(n))
                .collect()
        };
        if faltam.is_empty() {
            Avaliacao::Completo
        } else {
            Avaliacao::Parcial(faltam)
        }
    }
}

enum Avaliacao {
    Errada,
    Parcial(Vec<NotaMidi>),
    Completo,
}

/// Exercício: objetivo, instruções, gabarito, pré-requisito e feedback. Vem de uma Lição
/// em arquivo (ADR-0009, módulo [`crate::licoes`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Exercicio {
    /// Identificador estável `licao/exercicio` (usado no arquivo de progresso).
    pub id: String,
    /// Posição 1..=N dentro da Lição.
    pub numero: u16,
    /// Índice da Lição em [`EstadoExercicios::licoes`].
    pub licao: usize,
    pub titulo: String,
    pub tipo: Tipo,
    /// Mão indicada (`direita`, `esquerda`, `ambas`); vazio quando não se aplica.
    pub mao: String,
    pub objetivo: String,
    pub instrucoes: String,
    /// Dedilhado sugerido; vazio quando não se aplica.
    pub dedilhado: String,
    /// Gabarito: passos na ordem exigida.
    pub passos: Vec<Passo>,
    /// Como fazer, passo a passo (pode ser vazio: a tela monta um roteiro padrão).
    pub roteiro: Vec<String>,
    /// Exercício (id completo) que precisa estar concluído antes.
    pub prerequisito: Option<String>,
    /// Mensagem a cada passo certo.
    pub feedback_acerto: String,
    /// Orientação a cada nota errada.
    pub feedback_erro: String,
    /// Mensagem ao concluir.
    pub conclusao: String,
}

impl Exercicio {
    /// Gabarito em texto.
    pub fn gabarito_texto(&self) -> String {
        self.passos
            .iter()
            .map(Passo::descricao)
            .collect::<Vec<_>>()
            .join("  >  ")
    }

    /// `true` quando o exercício esconde as teclas-guia (só de ouvido).
    pub fn de_ouvido(&self) -> bool {
        self.tipo == Tipo::OuvirERepetir
    }
}

/// Id da Lição com os 10 exercícios originais da F05.
pub const LICAO_INICIAL: &str = "primeiros-passos";

/// Os 10 exercícios progressivos originais (Lição `primeiros-passos`, embutida).
pub fn exercicios_de_fabrica() -> Vec<Exercicio> {
    let mut erros = Vec::new();
    crate::licoes::licoes_de_fabrica(&mut erros)
        .into_iter()
        .find(|l| l.id == LICAO_INICIAL)
        .map(|l| l.exercicios)
        .unwrap_or_default()
}

/// Classificação de uma nota tocada.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Classificacao {
    /// A nota completou o passo.
    PassoCompleto,
    /// A nota completou o último passo: exercício concluído.
    ExercicioCompleto,
    /// Nota certa de um conjunto ainda incompleto.
    ParteCerta,
    /// Nota que não pertence ao passo (conta um erro).
    Errada,
    /// Notas do conjunto separadas por mais que a janela (não conta erro).
    ForaDaJanela,
    /// `NoteOff`, nota com o exercício pausado ou já concluído.
    Ignorada,
}

/// Feedback mostrado ao aluno (sempre com texto e símbolo).
#[derive(Clone, Debug, PartialEq)]
pub enum Feedback {
    Nenhum,
    Certo,
    Parcial { faltam: Vec<NotaMidi> },
    Errou { tocada: NotaMidi },
    ForaDaJanela,
    Concluido,
    TempoEsgotado,
    Pausado,
    Repita,
}

/// Tom do feedback (a tela escolhe uma cor de apoio; o símbolo é obrigatório).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tom {
    Neutro,
    Bom,
    Ruim,
    Aviso,
}

impl Feedback {
    /// Símbolo em texto simples (a fonte da tela não tem ♯/♭ nem ícones).
    pub const fn simbolo(&self) -> &'static str {
        match self {
            Feedback::Nenhum => "",
            Feedback::Certo | Feedback::Concluido => "[OK]",
            Feedback::Parcial { .. } => "[...]",
            Feedback::Errou { .. } => "[X]",
            Feedback::ForaDaJanela | Feedback::TempoEsgotado => "[!]",
            Feedback::Pausado => "[||]",
            Feedback::Repita => "[<]",
        }
    }

    pub const fn tom(&self) -> Tom {
        match self {
            Feedback::Certo | Feedback::Concluido => Tom::Bom,
            Feedback::Errou { .. } => Tom::Ruim,
            Feedback::ForaDaJanela | Feedback::TempoEsgotado => Tom::Aviso,
            _ => Tom::Neutro,
        }
    }
}

/// Fase da tentativa atual.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fase {
    /// Nenhuma nota tocada ainda.
    Pronto,
    EmAndamento,
    Concluido,
}

/// Uma nota tocada e como foi classificada.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Evento {
    pub t_ns: u64,
    pub nota: NotaMidi,
    pub passo: usize,
    pub resultado: Classificacao,
}

/// Onde, na grade de compasso, uma nota foi tocada (contagem a partir de 1).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalNaGrade {
    pub compasso: u64,
    pub pulso: u64,
}

impl LocalNaGrade {
    pub fn texto(self) -> String {
        format!("compasso {}, tempo {}", self.compasso, self.pulso)
    }
}

/// Uma nota errada da tentativa, com o passo esperado e o lugar na grade.
#[derive(Clone, Debug, PartialEq)]
pub struct ErroLocalizado {
    pub nota: NotaMidi,
    /// Índice do passo (0-based) que estava esperado.
    pub passo: usize,
    /// Notas esperadas no passo.
    pub esperado: Vec<NotaMidi>,
    pub local: LocalNaGrade,
}

impl ErroLocalizado {
    /// Texto com símbolo `[X]`, nunca só cor.
    pub fn texto(&self) -> String {
        format!(
            "[X] {} no passo {} ({}); esperado: {}",
            nome_nota(self.nota),
            self.passo + 1,
            self.local.texto(),
            nomes(&self.esperado)
        )
    }
}

/// Tentativa em curso do exercício atual.
#[derive(Clone, Debug)]
pub struct Tentativa {
    pub passo: usize,
    pub acertos: u32,
    pub erros: u32,
    pub fase: Fase,
    pub feedback: Feedback,
    pub inicio_ns: Option<u64>,
    pub pausado_desde: Option<u64>,
    /// Registro das notas tocadas (base do replay da F06 e do compasso do erro da F08).
    pub registro: Vec<Evento>,
    grupo: Vec<(NotaMidi, u64)>,
}

impl Default for Tentativa {
    fn default() -> Self {
        Self {
            passo: 0,
            acertos: 0,
            erros: 0,
            fase: Fase::Pronto,
            feedback: Feedback::Nenhum,
            inicio_ns: None,
            pausado_desde: None,
            registro: Vec::new(),
            grupo: Vec::new(),
        }
    }
}

/// Contagens de acertos e erros.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Contagem {
    pub acertos: u32,
    pub erros: u32,
}

impl Contagem {
    /// Aproveitamento 0–100, ou `None` sem nenhuma nota.
    pub fn aproveitamento(&self) -> Option<u32> {
        let total = u64::from(self.acertos) + u64::from(self.erros);
        (total > 0).then(|| u32::try_from(u64::from(self.acertos) * 100 / total).unwrap_or(100))
    }
}

/// Progresso salvo de um exercício.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Progresso {
    pub acertos: u32,
    pub erros: u32,
    pub concluido: bool,
    /// Quantas vezes foi concluído.
    pub vezes: u32,
}

/// Configuração do professor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfigExercicios {
    /// Janela para aceitar um conjunto de notas como simultâneo.
    pub janela_ms: u32,
    /// Limite de tempo da tentativa em segundos; `0` = sem limite (padrão).
    pub limite_s: u32,
    /// Mostra as teclas-guia também nos exercícios de ouvido.
    pub mostrar_dica: bool,
    /// Andamento (bpm) da grade que indica o compasso do erro.
    pub bpm_grade: u16,
}

impl Default for ConfigExercicios {
    fn default() -> Self {
        Self {
            janela_ms: JANELA_PADRAO_MS,
            limite_s: 0,
            mostrar_dica: false,
            bpm_grade: BPM_GRADE_PADRAO,
        }
    }
}

/// Estado do professor assistido.
#[derive(Clone, Debug)]
pub struct EstadoExercicios {
    /// Lições em ordem de Trilha (os Exercícios ficam em `exercicios`, na mesma ordem).
    pub licoes: Vec<Licao>,
    /// Faixa de `exercicios` de cada Lição (paralelo a `licoes`).
    pub faixas: Vec<Range<usize>>,
    /// Problemas ao ler arquivos de Lição (mostrados na tela).
    pub erros_carga: Vec<String>,
    pub exercicios: Vec<Exercicio>,
    /// Progresso, paralelo a `exercicios`.
    pub progresso: Vec<Progresso>,
    pub config: ConfigExercicios,
    /// Estatísticas desta sessão (zeradas ao abrir o app).
    pub sessao: Contagem,
    pub sessao_concluidos: u32,
    pub tentativa: Tentativa,
    pub mensagem: Option<String>,
    /// Pasta de dados (`progresso.txt` fica nela).
    pub pasta: Option<PathBuf>,
    /// A tela pede confirmação antes de apagar o progresso.
    pub confirmar_apagar: bool,
    atual: usize,
    sujo: bool,
}

/// Id da versão 1 do progresso (`3`) para o id atual (`primeiros-passos/3`).
fn id_migrado(v: &str) -> String {
    if !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()) {
        format!("{LICAO_INICIAL}/{v}")
    } else {
        v.to_owned()
    }
}

impl EstadoExercicios {
    /// Todas as Lições (fábrica e `<pasta>/licoes/`) e o progresso salvo na pasta.
    pub fn novo(pasta: Option<PathBuf>) -> Self {
        let mut erros = Vec::new();
        let licoes = crate::licoes::carregar_todas(pasta.as_deref(), &mut erros);
        let mut e = Self::com_licoes(licoes, pasta);
        e.erros_carga = erros;
        e
    }

    /// Estado com exercícios soltos (testes): ficam numa Lição só.
    pub fn com_exercicios(exercicios: Vec<Exercicio>, pasta: Option<PathBuf>) -> Self {
        let licao = Licao {
            id: LICAO_INICIAL.to_owned(),
            trilha: "Exercícios".to_owned(),
            ordem_trilha: 0,
            ordem: 0,
            titulo: "Exercícios".to_owned(),
            descricao: String::new(),
            nivel: String::new(),
            situacao: Situacao::Proposta,
            fontes: Vec::new(),
            requer: Vec::new(),
            exercicios,
            origem: Origem::Fabrica("testes"),
        };
        Self::com_licoes(vec![licao], pasta)
    }

    /// Estado a partir de Lições já lidas. Lê o progresso da pasta.
    pub fn com_licoes(mut licoes: Vec<Licao>, pasta: Option<PathBuf>) -> Self {
        let mut exercicios = Vec::new();
        let mut faixas = Vec::with_capacity(licoes.len());
        for (i, l) in licoes.iter_mut().enumerate() {
            let inicio = exercicios.len();
            for mut x in std::mem::take(&mut l.exercicios) {
                x.licao = i;
                exercicios.push(x);
            }
            faixas.push(inicio..exercicios.len());
        }
        let n = exercicios.len();
        let mut e = Self {
            licoes,
            faixas,
            erros_carga: Vec::new(),
            exercicios,
            progresso: vec![Progresso::default(); n],
            config: ConfigExercicios::default(),
            sessao: Contagem::default(),
            sessao_concluidos: 0,
            tentativa: Tentativa::default(),
            mensagem: None,
            pasta,
            confirmar_apagar: false,
            atual: 0,
            sujo: false,
        };
        e.carregar();
        e
    }

    // ---------- consulta ----------

    /// Índice do exercício atual.
    pub fn indice(&self) -> usize {
        self.atual
    }

    pub fn exercicio(&self) -> Option<&Exercicio> {
        self.exercicios.get(self.atual)
    }

    pub fn passo_atual(&self) -> Option<&Passo> {
        self.exercicio()?.passos.get(self.tentativa.passo)
    }

    pub fn concluido(&self, indice: usize) -> bool {
        self.progresso.get(indice).is_some_and(|p| p.concluido)
    }

    /// Exercício liberado: sem pré-requisito ou com o pré-requisito concluído.
    pub fn desbloqueado(&self, indice: usize) -> bool {
        let Some(ex) = self.exercicios.get(indice) else {
            return false;
        };
        let exercicio_ok = match &ex.prerequisito {
            None => true,
            Some(id) => self.posicao(id).is_some_and(|i| self.concluido(i)),
        };
        exercicio_ok && self.licoes_requeridas_ok(ex.licao)
    }

    /// Índice do exercício de id `id`.
    pub fn posicao(&self, id: &str) -> Option<usize> {
        self.exercicios.iter().position(|x| x.id == id)
    }

    /// Índice da Lição de id `id`.
    pub fn posicao_licao(&self, id: &str) -> Option<usize> {
        self.licoes.iter().position(|l| l.id == id)
    }

    /// (concluídos, total) de uma Lição.
    pub fn andamento_licao(&self, licao: usize) -> (usize, usize) {
        let Some(f) = self.faixas.get(licao) else {
            return (0, 0);
        };
        (f.clone().filter(|i| self.concluido(*i)).count(), f.len())
    }

    /// Lição com todos os exercícios concluídos.
    pub fn licao_concluida(&self, licao: usize) -> bool {
        let (feitos, total) = self.andamento_licao(licao);
        total > 0 && feitos == total
    }

    fn licoes_requeridas_ok(&self, licao: usize) -> bool {
        self.licoes.get(licao).is_none_or(|l| {
            l.requer.iter().all(|r| {
                self.posicao_licao(r)
                    .is_none_or(|i| self.licao_concluida(i))
            })
        })
    }

    /// Lição do exercício atual.
    pub fn licao_atual(&self) -> Option<&Licao> {
        self.licoes.get(self.exercicio()?.licao)
    }

    /// Texto do bloqueio ("conclua antes ...").
    pub fn motivo_bloqueio(&self, indice: usize) -> Option<String> {
        if self.desbloqueado(indice) {
            return None;
        }
        let ex = self.exercicios.get(indice)?;
        if let Some(i) = ex.prerequisito.as_deref().and_then(|id| self.posicao(id))
            && !self.concluido(i)
        {
            let p = &self.exercicios[i];
            return Some(format!(
                "conclua antes o exercício {}: {}",
                p.numero, p.titulo
            ));
        }
        let l = self.licoes.get(ex.licao)?;
        let faltam: Vec<&str> = l
            .requer
            .iter()
            .filter_map(|r| self.posicao_licao(r))
            .filter(|i| !self.licao_concluida(*i))
            .map(|i| self.licoes[i].titulo.as_str())
            .collect();
        Some(format!("conclua antes a Lição {}", faltam.join(", ")))
    }

    pub fn janela_ns(&self) -> u64 {
        u64::from(self.config.janela_ms.clamp(JANELA_MIN_MS, JANELA_MAX_MS)) * 1_000_000
    }

    /// Teclas a destacar agora (vazio nos exercícios de ouvido, salvo com a dica).
    pub fn guias(&self) -> Vec<NotaMidi> {
        let Some(ex) = self.exercicio() else {
            return Vec::new();
        };
        if self.tentativa.fase == Fase::Concluido || (ex.de_ouvido() && !self.config.mostrar_dica) {
            return Vec::new();
        }
        self.passo_atual()
            .map(|p| p.notas.clone())
            .unwrap_or_default()
    }

    /// Passos do gabarito para o app tocar (cada passo é um grupo de notas).
    pub fn passos_para_ouvir(&self) -> Vec<Vec<NotaMidi>> {
        self.exercicio()
            .map(|e| e.passos.iter().map(|p| p.notas.clone()).collect())
            .unwrap_or_default()
    }

    /// Espaço entre os passos ao ouvir, em milissegundos.
    pub fn ouvir_passo_ms(&self) -> u64 {
        match self.exercicio().map(|e| e.tipo) {
            Some(Tipo::Acorde | Tipo::Intervalos) => 1100,
            Some(Tipo::Arpejo) => 320,
            _ => 420,
        }
    }

    /// Texto do feedback atual (inclui o símbolo).
    pub fn feedback_texto(&self) -> String {
        let t = &self.tentativa;
        let ex = self.exercicio();
        let corpo = match &t.feedback {
            Feedback::Nenhum => {
                return "Toque a nota marcada no teclado para começar. Não há limite de tempo."
                    .to_owned();
            }
            Feedback::Certo => format!(
                "Certo! {}",
                ex.map(|e| e.feedback_acerto.as_str()).unwrap_or_default()
            ),
            Feedback::Parcial { faltam } => {
                format!("Boa, faltam: {}. Toque juntas.", nomes(faltam))
            }
            Feedback::Errou { tocada } => {
                let ouvido = ex.is_some_and(Exercicio::de_ouvido) && !self.config.mostrar_dica;
                let esperado = self.passo_atual().map(|p| nomes(&p.notas));
                let dica = match (ouvido, esperado) {
                    (false, Some(e)) => {
                        format!(" Você tocou {}; esperado: {e}.", nome_nota(*tocada))
                    }
                    _ => format!(" Você tocou {}.", nome_nota(*tocada)),
                };
                format!(
                    "Nota errada.{dica} {} Pode tentar de novo.",
                    ex.map(|e| e.feedback_erro.as_str()).unwrap_or_default()
                )
            }
            Feedback::ForaDaJanela => format!(
                "As notas do conjunto ficaram separadas por mais de {} ms. Toque-as juntas.",
                self.config.janela_ms
            ),
            Feedback::Concluido => format!(
                "Exercício concluído: {} acertos, {} erros. {}",
                t.acertos,
                t.erros,
                ex.map(|e| e.conclusao.as_str()).unwrap_or_default()
            ),
            Feedback::TempoEsgotado => {
                "Tempo esgotado: a tentativa recomeçou. Desligue o limite se preferir sem pressa."
                    .to_owned()
            }
            Feedback::Pausado => "Pausado: as notas não contam até retomar.".to_owned(),
            Feedback::Repita => "Repita este passo; nada foi contado.".to_owned(),
        };
        format!("{} {corpo}", t.feedback.simbolo())
    }

    // ---------- classificador ----------

    /// Toda nota do teclado físico ou da simulação passa por aqui.
    pub fn nota(&mut self, ligar: bool, nota: NotaMidi, t_ns: u64) {
        self.classificar(ligar, nota, t_ns);
    }

    /// Como [`EstadoExercicios::nota`], devolvendo a classificação.
    pub fn classificar(&mut self, ligar: bool, nota: NotaMidi, t_ns: u64) -> Classificacao {
        if !ligar
            || self.tentativa.fase == Fase::Concluido
            || self.tentativa.pausado_desde.is_some()
        {
            return Classificacao::Ignorada;
        }
        self.verificar_tempo(t_ns);
        let janela = self.janela_ns();
        let Some(passo) = self.passo_atual().cloned() else {
            return Classificacao::Ignorada;
        };
        let t = &mut self.tentativa;
        t.inicio_ns.get_or_insert(t_ns);
        t.fase = Fase::EmAndamento;
        self.sujo = true;

        let antes = t.grupo.len();
        t.grupo.retain(|(_, ts)| t_ns.saturating_sub(*ts) <= janela);
        let expirou = t.grupo.len() < antes;
        t.grupo.retain(|(n, _)| *n != nota);
        t.grupo.push((nota, t_ns));
        let tocadas: Vec<NotaMidi> = t.grupo.iter().map(|(n, _)| *n).collect();

        let passo_idx = t.passo;
        let resultado = match passo.avaliar(&tocadas) {
            Avaliacao::Errada => {
                t.grupo.clear();
                t.erros = t.erros.saturating_add(1);
                t.feedback = Feedback::Errou { tocada: nota };
                self.sessao.erros = self.sessao.erros.saturating_add(1);
                if let Some(p) = self.progresso.get_mut(self.atual) {
                    p.erros = p.erros.saturating_add(1);
                }
                Classificacao::Errada
            }
            Avaliacao::Parcial(_) if expirou => {
                t.feedback = Feedback::ForaDaJanela;
                Classificacao::ForaDaJanela
            }
            Avaliacao::Parcial(faltam) => {
                t.feedback = Feedback::Parcial { faltam };
                Classificacao::ParteCerta
            }
            Avaliacao::Completo => {
                t.grupo.clear();
                t.acertos = t.acertos.saturating_add(1);
                t.passo += 1;
                self.sessao.acertos = self.sessao.acertos.saturating_add(1);
                if let Some(p) = self.progresso.get_mut(self.atual) {
                    p.acertos = p.acertos.saturating_add(1);
                }
                let total = self.exercicio().map_or(0, |e| e.passos.len());
                if self.tentativa.passo >= total {
                    self.concluir();
                    Classificacao::ExercicioCompleto
                } else {
                    self.tentativa.feedback = Feedback::Certo;
                    Classificacao::PassoCompleto
                }
            }
        };
        if self.tentativa.registro.len() < MAX_REGISTRO {
            self.tentativa.registro.push(Evento {
                t_ns,
                nota,
                passo: passo_idx,
                resultado,
            });
        }
        resultado
    }

    fn concluir(&mut self) {
        self.tentativa.fase = Fase::Concluido;
        self.tentativa.feedback = Feedback::Concluido;
        self.sessao_concluidos = self.sessao_concluidos.saturating_add(1);
        if let Some(p) = self.progresso.get_mut(self.atual) {
            p.concluido = true;
            p.vezes = p.vezes.saturating_add(1);
        }
        self.salvar();
    }

    // ---------- controle da tentativa ----------

    /// Recomeça o exercício atual do primeiro passo (estatísticas acumuladas são mantidas).
    pub fn reiniciar(&mut self) {
        self.tentativa = Tentativa::default();
    }

    /// Repete o passo atual: descarta as notas parciais, sem contar nada.
    pub fn repetir(&mut self) {
        if self.tentativa.fase == Fase::Concluido {
            self.reiniciar();
            return;
        }
        self.tentativa.grupo.clear();
        self.tentativa.feedback = Feedback::Repita;
    }

    /// Pausa: notas não contam e o limite de tempo não corre.
    pub fn pausar(&mut self, t_ns: u64) {
        if self.tentativa.fase != Fase::Concluido && self.tentativa.pausado_desde.is_none() {
            self.tentativa.pausado_desde = Some(t_ns);
            self.tentativa.grupo.clear();
            self.tentativa.feedback = Feedback::Pausado;
        }
    }

    pub fn retomar(&mut self, t_ns: u64) {
        if let Some(desde) = self.tentativa.pausado_desde.take() {
            if let Some(i) = self.tentativa.inicio_ns.as_mut() {
                *i = i.saturating_add(t_ns.saturating_sub(desde));
            }
            self.tentativa.feedback = Feedback::Nenhum;
        }
    }

    /// Aplica o limite de tempo opcional (a tela chama a cada quadro). Devolve `true` se
    /// a tentativa estourou o limite e foi reiniciada.
    pub fn verificar_tempo(&mut self, t_ns: u64) -> bool {
        let limite = u64::from(self.config.limite_s) * 1_000_000_000;
        let t = &self.tentativa;
        let estourou = limite > 0
            && t.fase == Fase::EmAndamento
            && t.pausado_desde.is_none()
            && t.inicio_ns.is_some_and(|i| t_ns.saturating_sub(i) > limite);
        if estourou {
            self.reiniciar();
            self.tentativa.feedback = Feedback::TempoEsgotado;
        }
        estourou
    }

    // ---------- feedback de erro: replay e compasso (F05-5) ----------

    /// Grade de compasso da tentativa: 4/4 no andamento configurado, com o compasso 1
    /// começando na primeira nota tocada. `None` se ainda não houve nota.
    pub fn grade(&self) -> Option<Agendador> {
        let t0 = self.tentativa.registro.first()?.t_ns;
        Some(Agendador::novo(t0, self.config.bpm_grade, FORMULA_GRADE))
    }

    /// Lugar de um instante na grade (compasso e tempo, contados a partir de 1).
    pub fn local_na_grade(&self, t_ns: u64) -> Option<LocalNaGrade> {
        let p = self.grade()?.posicao(t_ns)?;
        Some(LocalNaGrade {
            compasso: p.compasso + 1,
            pulso: p.pulso + 1,
        })
    }

    /// Notas erradas da tentativa, em ordem, cada uma com o passo esperado e o compasso.
    pub fn erros_localizados(&self) -> Vec<ErroLocalizado> {
        let Some(ex) = self.exercicio() else {
            return Vec::new();
        };
        self.tentativa
            .registro
            .iter()
            .filter(|ev| ev.resultado == Classificacao::Errada)
            .filter_map(|ev| {
                Some(ErroLocalizado {
                    nota: ev.nota,
                    passo: ev.passo,
                    esperado: ex.passos.get(ev.passo)?.notas.clone(),
                    local: self.local_na_grade(ev.t_ns)?,
                })
            })
            .collect()
    }

    /// O que o aluno tocou na tentativa, como `Gravacao` da F06 (para ouvir de novo).
    /// Cada nota dura 3/4 do intervalo até a próxima, no máximo 400 ms; notas ignoradas (pausa, depois de
    /// concluir) ficam de fora. `None` se não há nota registrada.
    pub fn tentativa_como_gravacao(&self) -> Option<Gravacao> {
        let toques: Vec<&Evento> = self
            .tentativa
            .registro
            .iter()
            .filter(|ev| ev.resultado != Classificacao::Ignorada)
            .collect();
        let t0 = toques.first()?.t_ns;
        let mut eventos: Vec<EventoGravado> = Vec::with_capacity(toques.len() * 2);
        let mut fim = 0u64;
        for (i, ev) in toques.iter().enumerate() {
            let inicio = ev.t_ns.saturating_sub(t0);
            // Solta antes da próxima nota (3/4 do intervalo), no máximo 400 ms.
            let dur = toques.get(i + 1).map_or(NOTA_REPLAY_NS, |p| {
                (p.t_ns.saturating_sub(ev.t_ns) / 4 * 3).clamp(NOTA_REPLAY_MIN_NS, NOTA_REPLAY_NS)
            });
            let (canal, nota) = (0, ev.nota.numero());
            eventos.push(EventoGravado {
                t_ns: inicio,
                msg: Mensagem::NotaLigada {
                    canal,
                    nota,
                    velocidade: VELOCIDADE_REPLAY,
                },
            });
            eventos.push(EventoGravado {
                t_ns: inicio + dur,
                msg: Mensagem::NotaDesligada {
                    canal,
                    nota,
                    velocidade: 64,
                },
            });
            fim = fim.max(inicio + dur);
        }
        eventos.sort_by_key(|e| e.t_ns);
        Some(Gravacao {
            nome: "Tentativa".to_owned(),
            eventos,
            tempos: Vec::new(),
            duracao_ns: fim,
        })
    }

    // ---------- progressão ----------

    /// Vai para o exercício `indice` se estiver liberado.
    pub fn selecionar(&mut self, indice: usize) -> bool {
        if !self.desbloqueado(indice) {
            return false;
        }
        if indice != self.atual {
            self.atual = indice;
            self.sujo = true;
        }
        self.reiniciar();
        self.confirmar_apagar = false;
        true
    }

    /// Abre a Lição `licao` no primeiro exercício liberado e ainda não concluído (ou no
    /// primeiro liberado). `false` se nenhum está liberado.
    pub fn abrir_licao(&mut self, licao: usize) -> bool {
        let Some(f) = self.faixas.get(licao).cloned() else {
            return false;
        };
        let alvo = f
            .clone()
            .find(|i| self.desbloqueado(*i) && !self.concluido(*i))
            .or_else(|| f.clone().find(|i| self.desbloqueado(*i)));
        alvo.is_some_and(|i| self.selecionar(i))
    }

    /// Próximo exercício, se existir e estiver liberado.
    pub fn avancar(&mut self) -> bool {
        self.selecionar(self.atual + 1)
    }

    /// Exercício anterior (sempre liberado, pois o pré-requisito vem antes).
    pub fn voltar(&mut self) -> bool {
        self.atual > 0 && self.selecionar(self.atual - 1)
    }

    /// Estatística do exercício atual (histórico salvo).
    pub fn historico_atual(&self) -> Progresso {
        self.progresso.get(self.atual).copied().unwrap_or_default()
    }

    // ---------- persistência ----------

    pub fn para_texto(&self) -> String {
        let mut s = String::from("# Piano Tutor: progresso dos exercícios v2\n");
        let _ = writeln!(s, "janela_ms={}", self.config.janela_ms);
        let _ = writeln!(s, "limite_s={}", self.config.limite_s);
        let _ = writeln!(s, "dica={}", u8::from(self.config.mostrar_dica));
        let _ = writeln!(s, "grade_bpm={}", self.config.bpm_grade);
        if let Some(e) = self.exercicio() {
            let _ = writeln!(s, "atual={}", e.id);
        }
        for (ex, p) in self.exercicios.iter().zip(&self.progresso) {
            if *p == Progresso::default() {
                continue;
            }
            let id = &ex.id;
            let _ = writeln!(s, "ex.{id}.acertos={}", p.acertos);
            let _ = writeln!(s, "ex.{id}.erros={}", p.erros);
            let _ = writeln!(s, "ex.{id}.concluido={}", u8::from(p.concluido));
            let _ = writeln!(s, "ex.{id}.vezes={}", p.vezes);
        }
        s
    }

    /// Aplica um texto de progresso. Linhas inválidas, chaves desconhecidas e valores fora
    /// de faixa são ignorados (ficam no padrão); nunca falha.
    pub fn aplicar_texto(&mut self, texto: &str) {
        self.progresso = vec![Progresso::default(); self.exercicios.len()];
        self.config = ConfigExercicios::default();
        let mut atual_id: Option<String> = None;
        for linha in texto.lines() {
            let Some((k, v)) = linha.trim().split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "janela_ms" => {
                    if let Ok(x) = v.parse::<u32>()
                        && (JANELA_MIN_MS..=JANELA_MAX_MS).contains(&x)
                    {
                        self.config.janela_ms = x;
                    }
                }
                "limite_s" => {
                    if let Ok(x) = v.parse::<u32>() {
                        self.config.limite_s = x.min(3600);
                    }
                }
                "dica" => self.config.mostrar_dica = v == "1",
                "grade_bpm" => {
                    if let Ok(x) = v.parse::<u16>()
                        && (BPM_MIN..=BPM_MAX).contains(&x)
                    {
                        self.config.bpm_grade = x;
                    }
                }
                "atual" => atual_id = Some(id_migrado(v)),
                _ => self.aplicar_chave_exercicio(k, v),
            }
        }
        // Coerência: um exercício só vale como concluído se o pré-requisito também foi.
        for i in 0..self.exercicios.len() {
            if self.progresso[i].concluido && !self.desbloqueado(i) {
                self.progresso[i].concluido = false;
            }
        }
        self.atual = atual_id
            .and_then(|id| self.posicao(&id))
            .filter(|i| self.desbloqueado(*i))
            .unwrap_or(0);
        self.tentativa = Tentativa::default();
    }

    /// Chaves `ex.<licao>/<exercicio>.<campo>` (v2) e `ex<N>.<campo>` (v1, migra para
    /// `primeiros-passos/N`).
    fn aplicar_chave_exercicio(&mut self, k: &str, v: &str) {
        let (id, campo) = if let Some(resto) = k.strip_prefix("ex.") {
            let Some((id, campo)) = resto.rsplit_once('.') else {
                return;
            };
            (id.to_owned(), campo)
        } else if let Some(resto) = k.strip_prefix("ex") {
            let Some((n, campo)) = resto.split_once('.') else {
                return;
            };
            if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
                return;
            }
            (id_migrado(n), campo)
        } else {
            return;
        };
        let Some(i) = self.posicao(&id) else {
            return;
        };
        let p = &mut self.progresso[i];
        match campo {
            "acertos" => p.acertos = v.parse().unwrap_or(0),
            "erros" => p.erros = v.parse().unwrap_or(0),
            "concluido" => p.concluido = v == "1",
            "vezes" => p.vezes = v.parse().unwrap_or(0),
            _ => {}
        }
    }

    fn caminho(&self) -> Option<PathBuf> {
        self.pasta.as_ref().map(|p| p.join(ARQUIVO_PROGRESSO))
    }

    /// Lê `progresso.txt` (arquivo ausente ou corrompido: padrões, sem pânico).
    fn carregar(&mut self) {
        let Some(c) = self.caminho() else {
            return;
        };
        match std::fs::read(&c) {
            Ok(b) => {
                self.aplicar_texto(&String::from_utf8_lossy(&b));
                self.sujo = false;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => self.mensagem = Some(format!("Não foi possível ler o progresso: {e}")),
        }
    }

    /// Grava `progresso.txt`.
    pub fn salvar(&mut self) {
        let Some(c) = self.caminho() else {
            return;
        };
        let tmp = c.with_extension("txt.tmp");
        let r = c
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&tmp, self.para_texto()))
            .and_then(|()| std::fs::rename(&tmp, &c));
        match r {
            Ok(()) => self.sujo = false,
            Err(e) => self.mensagem = Some(format!("Não foi possível salvar o progresso: {e}")),
        }
    }

    /// Salva se algo mudou desde a última gravação.
    pub fn salvar_se_sujo(&mut self) {
        if self.sujo {
            self.salvar();
        }
    }

    /// Comando explícito: apaga todo o progresso (memória e arquivo).
    pub fn apagar_progresso(&mut self) {
        self.progresso = vec![Progresso::default(); self.exercicios.len()];
        self.sessao = Contagem::default();
        self.sessao_concluidos = 0;
        self.atual = 0;
        self.tentativa = Tentativa::default();
        self.confirmar_apagar = false;
        self.sujo = false;
        self.mensagem = Some(match self.caminho() {
            Some(c) => match std::fs::remove_file(&c) {
                Ok(()) => "Progresso apagado.".to_owned(),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    "Progresso apagado.".to_owned()
                }
                Err(e) => format!("Progresso zerado, mas o arquivo não pôde ser removido: {e}"),
            },
            None => "Progresso apagado (sem pasta de dados).".to_owned(),
        });
    }

    /// Relatório legível do progresso.
    pub fn relatorio(&self) -> String {
        let mut s = String::from("Piano Tutor: relatório de progresso dos exercícios\n\n");
        for (i, (ex, p)) in self.exercicios.iter().zip(&self.progresso).enumerate() {
            let situacao = if p.concluido {
                format!("concluído {} vez(es)", p.vezes)
            } else if self.desbloqueado(i) {
                "liberado, ainda não concluído".to_owned()
            } else {
                "bloqueado".to_owned()
            };
            let aprov = Contagem {
                acertos: p.acertos,
                erros: p.erros,
            }
            .aproveitamento()
            .map_or("sem notas".to_owned(), |a| format!("{a}%"));
            let licao = self.licoes.get(ex.licao).map_or("", |l| l.titulo.as_str());
            let _ = writeln!(
                s,
                "{licao} {}. {} [{}]: {situacao}; acertos {}, erros {}, aproveitamento {aprov}",
                ex.numero,
                ex.titulo,
                ex.tipo.texto(),
                p.acertos,
                p.erros
            );
        }
        let feitos = self.progresso.iter().filter(|p| p.concluido).count();
        let _ = writeln!(
            s,
            "\nConcluídos: {feitos} de {}. Janela de conjunto: {} ms.",
            self.exercicios.len(),
            self.config.janela_ms
        );
        s
    }

    /// Comando explícito: grava o relatório em `progresso-exportado.txt`.
    pub fn exportar(&mut self) -> Result<PathBuf, String> {
        let Some(pasta) = self.pasta.clone() else {
            let m = "Sem pasta de dados para exportar.".to_owned();
            self.mensagem = Some(m.clone());
            return Err(m);
        };
        let c = pasta.join(ARQUIVO_EXPORTADO);
        let r = std::fs::create_dir_all(&pasta).and_then(|()| std::fs::write(&c, self.relatorio()));
        match r {
            Ok(()) => {
                self.mensagem = Some(format!("Relatório exportado em {}", c.display()));
                Ok(c)
            }
            Err(e) => {
                let m = format!("Não foi possível exportar: {e}");
                self.mensagem = Some(m.clone());
                Err(m)
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    const MS: u64 = 1_000_000;

    fn n(x: u8) -> NotaMidi {
        NotaMidi::nova(x).expect("nota")
    }

    fn pasta_unica(rotulo: &str) -> PathBuf {
        static C: AtomicU32 = AtomicU32::new(0);
        std::env::temp_dir().join(format!(
            "pt-exerc-{}-{}-{rotulo}",
            std::process::id(),
            C.fetch_add(1, Ordering::Relaxed)
        ))
    }

    /// Estado já posicionado no exercício `id` (conclui os anteriores).
    fn em(id: u8) -> EstadoExercicios {
        let mut e = EstadoExercicios::novo(None);
        for p in e.progresso.iter_mut().take(usize::from(id) - 1) {
            p.concluido = true;
        }
        assert!(e.selecionar(usize::from(id) - 1));
        e
    }

    fn toca(
        e: &mut EstadoExercicios,
        notas: &[u8],
        t0_ms: u64,
        passo_ms: u64,
    ) -> Vec<Classificacao> {
        notas
            .iter()
            .enumerate()
            .map(|(i, x)| e.classificar(true, n(*x), (t0_ms + i as u64 * passo_ms) * MS))
            .collect()
    }

    #[test]
    fn fabrica_tem_dez_exercicios_completos_e_encadeados() {
        let ex = exercicios_de_fabrica();
        assert_eq!(ex.len(), 10);
        for (i, x) in ex.iter().enumerate() {
            assert_eq!(usize::from(x.numero), i + 1);
            for t in [
                &x.titulo,
                &x.objetivo,
                &x.instrucoes,
                &x.feedback_acerto,
                &x.feedback_erro,
                &x.conclusao,
            ] {
                assert!(!t.is_empty(), "exercício {} com texto vazio", x.id);
            }
            assert!(!x.passos.is_empty());
            assert!(x.passos.iter().all(|p| !p.notas.is_empty()));
            assert_eq!(
                x.prerequisito,
                (i > 0).then(|| format!("primeiros-passos/{i}"))
            );
        }
        assert_eq!(ex[1].passos.len(), 10, "o exercício 2 tem 10 notas");
    }

    /// F05-4: os 10 exercícios progressivos de fábrica existem, com todos os campos, e o
    /// gabarito de cada um é jogável (tocar o gabarito conclui o exercício).
    #[test]
    fn dez_exercicios_progressivos_tem_campos_completos_e_gabarito_jogavel() {
        let ex = exercicios_de_fabrica();
        assert_eq!(ex.len(), 10, "são exatamente 10 exercícios de fábrica");

        let tipos_esperados = [
            Tipo::Localizar,
            Tipo::Sequencia,
            Tipo::Sequencia,
            Tipo::Sequencia,
            Tipo::Intervalos,
            Tipo::Escala,
            Tipo::Acorde,
            Tipo::Acorde,
            Tipo::Arpejo,
            Tipo::OuvirERepetir,
        ];
        let mut titulos: Vec<&str> = ex.iter().map(|x| x.titulo.as_str()).collect();
        titulos.sort_unstable();
        titulos.dedup();
        assert_eq!(titulos.len(), 10, "títulos únicos");

        for (i, x) in ex.iter().enumerate() {
            let rotulo = format!("exercício {} ({})", x.id, x.titulo);
            assert_eq!(usize::from(x.numero), i + 1, "{rotulo}: id sequencial");
            assert_eq!(x.id, format!("primeiros-passos/{}", i + 1));
            assert_eq!(x.tipo, tipos_esperados[i], "{rotulo}: tipo");
            // Nome, objetivo, instruções, feedback de acerto/erro e conclusão.
            for (campo, texto) in [
                ("titulo", &x.titulo),
                ("objetivo", &x.objetivo),
                ("instrucoes", &x.instrucoes),
                ("feedback_acerto", &x.feedback_acerto),
                ("feedback_erro", &x.feedback_erro),
                ("conclusao", &x.conclusao),
            ] {
                assert!(
                    texto.trim().len() >= 10,
                    "{rotulo}: campo {campo} vazio ou curto demais"
                );
            }
            assert!(!x.tipo.texto().is_empty(), "{rotulo}: rótulo do tipo");
            // Gabarito: notas dentro das 61 teclas (Dó2..Dó7 = 36..96), sem conjunto vazio.
            assert!(!x.passos.is_empty(), "{rotulo}: sem gabarito");
            for p in &x.passos {
                assert!(!p.notas.is_empty(), "{rotulo}: passo vazio");
                for nota in &p.notas {
                    assert!(
                        (36..=96).contains(&nota.numero()),
                        "{rotulo}: nota {} fora das 61 teclas",
                        nota.numero()
                    );
                }
            }
            assert_eq!(
                x.gabarito_texto().split("  >  ").count(),
                x.passos.len(),
                "{rotulo}: gabarito em texto com um trecho por passo"
            );
            // Pré-requisito: o primeiro é livre; os demais exigem o anterior.
            assert_eq!(
                x.prerequisito,
                (i > 0).then(|| format!("primeiros-passos/{i}")),
                "{rotulo}: pré-requisito"
            );
            // Só o último é de ouvido (guias escondidas).
            assert_eq!(x.de_ouvido(), i == 9, "{rotulo}: de_ouvido");
            // Forma do gabarito conforme o tipo.
            match x.tipo {
                Tipo::Localizar | Tipo::Sequencia | Tipo::Escala | Tipo::Arpejo => {
                    assert!(
                        x.passos.iter().all(|p| p.notas.len() == 1 && !p.por_classe),
                        "{rotulo}: passos de uma nota exata"
                    );
                }
                Tipo::Intervalos => {
                    assert!(
                        x.passos.iter().all(|p| p.notas.len() == 2 && !p.por_classe),
                        "{rotulo}: intervalos de duas notas exatas"
                    );
                }
                Tipo::Acorde => {
                    assert!(
                        x.passos.iter().all(|p| p.notas.len() == 3 && p.por_classe),
                        "{rotulo}: tríades por classe de altura"
                    );
                }
                Tipo::OuvirERepetir => {
                    assert!(x.passos.len() >= 4, "{rotulo}: frase curta");
                }
            }
            // Dedilhado sugerido nos exercícios em que se aplica.
            if matches!(x.numero, 2 | 3 | 5 | 6 | 7 | 8 | 9) {
                assert!(!x.dedilhado.is_empty(), "{rotulo}: dedilhado sugerido");
            }
        }

        // Progressão: a tarefa fica mais complexa ao longo da lista (de 1 nota a 3 notas juntas).
        assert_eq!(ex[0].passos.len(), 1);
        assert_eq!(ex[1].passos.len(), 10);
        assert_eq!(ex[5].passos.len(), 8, "escala de uma oitava");
        assert_eq!(ex[8].passos.len(), 7, "arpejo subindo e descendo");

        // Jogabilidade: tocar o gabarito (notas de cada passo dentro da janela) conclui cada um.
        for x in &ex {
            let mut e = em(x.numero as u8);
            let mut t_ms = 0u64;
            let mut ultima = Classificacao::Ignorada;
            for p in &x.passos {
                for nota in &p.notas {
                    ultima = e.classificar(true, *nota, t_ms * MS);
                    t_ms += 5;
                }
                t_ms += 500;
            }
            assert_eq!(
                ultima,
                Classificacao::ExercicioCompleto,
                "exercício {}: o gabarito deve concluir",
                x.id
            );
            assert_eq!(e.tentativa.erros, 0, "exercício {}: sem erros", x.id);
            assert_eq!(
                e.tentativa.acertos as usize,
                x.passos.len(),
                "exercício {}: um acerto por passo",
                x.id
            );
            assert!(e.concluido(usize::from(x.numero) - 1));
        }
    }

    #[test]
    fn exercicio_de_dez_notas_com_acertos_e_erros_injetados_classifica_100_por_cento() {
        let mut e = em(2);
        use Classificacao::{Errada, ExercicioCompleto, PassoCompleto};
        // (nota tocada, classificação esperada pelo gabarito C D E F G G F E D C)
        let roteiro: [(u8, Classificacao); 14] = [
            (60, PassoCompleto),
            (62, PassoCompleto),
            (66, Errada), // erro injetado: esperava Mi
            (64, PassoCompleto),
            (65, PassoCompleto),
            (67, PassoCompleto),
            (65, Errada), // erro injetado: esperava Sol de novo
            (67, PassoCompleto),
            (65, PassoCompleto),
            (64, PassoCompleto),
            (61, Errada), // erro injetado: esperava Ré
            (62, PassoCompleto),
            (59, Errada), // erro injetado: esperava Dó
            (60, ExercicioCompleto),
        ];
        for (i, (x, esperado)) in roteiro.iter().enumerate() {
            let c = e.classificar(true, n(*x), i as u64 * 400 * MS);
            assert_eq!(c, *esperado, "evento {i}: nota {x}");
        }
        assert_eq!(e.tentativa.acertos, 10);
        assert_eq!(e.tentativa.erros, 4);
        assert_eq!(e.tentativa.fase, Fase::Concluido);
        assert_eq!(e.historico_atual().acertos, 10);
        assert_eq!(e.historico_atual().erros, 4);
        assert!(e.concluido(1));
        assert_eq!(
            e.sessao,
            Contagem {
                acertos: 10,
                erros: 4
            }
        );
        assert_eq!(e.sessao.aproveitamento(), Some(71));
        assert_eq!(e.tentativa.registro.len(), 14);
        assert!(e.feedback_texto().starts_with("[OK]"));
    }

    #[test]
    fn nota_errada_nao_avanca_e_permite_corrigir() {
        let mut e = em(3);
        assert_eq!(e.classificar(true, n(60), 0), Classificacao::Errada);
        assert_eq!(e.tentativa.passo, 0);
        assert!(e.feedback_texto().starts_with("[X]"));
        assert!(e.feedback_texto().contains("Mi4"), "{}", e.feedback_texto());
        assert_eq!(
            e.classificar(true, n(64), 10 * MS),
            Classificacao::PassoCompleto
        );
        assert_eq!(e.tentativa.passo, 1);
    }

    #[test]
    fn noteoff_extra_nunca_penaliza() {
        let mut e = em(3);
        // NoteOff antes de qualquer NoteOn, repetido, de notas que nunca ligaram.
        for t in 0..5 {
            assert_eq!(e.classificar(false, n(72), t * MS), Classificacao::Ignorada);
            assert_eq!(e.classificar(false, n(64), t * MS), Classificacao::Ignorada);
        }
        assert_eq!(e.tentativa.erros, 0);
        assert_eq!(e.tentativa.fase, Fase::Pronto);
        let c = toca(&mut e, &[64], 100, 0);
        assert_eq!(c, vec![Classificacao::PassoCompleto]);
        e.nota(false, n(64), 200 * MS);
        e.nota(false, n(64), 210 * MS);
        e.nota(false, n(99), 220 * MS);
        assert_eq!(e.tentativa.erros, 0);
        assert_eq!(e.tentativa.passo, 1);
        // Depois de concluir, NoteOn e NoteOff ficam ignorados.
        let c = toca(&mut e, &[60, 67], 300, 100);
        assert_eq!(c[1], Classificacao::ExercicioCompleto);
        e.nota(false, n(60), 900 * MS);
        assert_eq!(e.classificar(true, n(1), 950 * MS), Classificacao::Ignorada);
        assert_eq!(e.tentativa.erros, 0);
    }

    #[test]
    fn acorde_dentro_da_janela_e_aceito() {
        let mut e = em(7);
        assert_eq!(e.config.janela_ms, 150);
        let c = [
            e.classificar(true, n(60), 1_000 * MS),
            e.classificar(true, n(64), 1_100 * MS),
            e.classificar(true, n(67), 1_150 * MS), // exatamente 150 ms: vale
        ];
        assert_eq!(
            c,
            [
                Classificacao::ParteCerta,
                Classificacao::ParteCerta,
                Classificacao::PassoCompleto
            ]
        );
        assert_eq!(e.tentativa.passo, 1);
    }

    #[test]
    fn acorde_fora_da_janela_nao_e_aceito_nem_conta_erro() {
        let mut e = em(7);
        let c = [
            e.classificar(true, n(60), 0),
            e.classificar(true, n(64), 100 * MS),
            e.classificar(true, n(67), 400 * MS), // 400 ms depois do Dó
        ];
        assert_eq!(c[2], Classificacao::ForaDaJanela);
        assert_eq!(e.tentativa.passo, 0);
        assert_eq!(e.tentativa.erros, 0);
        assert!(e.feedback_texto().starts_with("[!]"));
        // O aluno tenta de novo, agora junto.
        let c = toca(&mut e, &[60, 64, 67], 1_000, 30);
        assert_eq!(
            c[2],
            Classificacao::PassoCompleto,
            "o conjunto anterior não conta"
        );
    }

    #[test]
    fn janela_e_configuravel() {
        let mut e = em(7);
        e.config.janela_ms = 500;
        let c = toca(&mut e, &[60, 64, 67], 0, 200);
        assert_eq!(c[2], Classificacao::PassoCompleto, "400 ms cabe em 500 ms");
        let mut e = em(7);
        e.config.janela_ms = 50;
        let c = toca(&mut e, &[60, 64, 67], 0, 40);
        assert_eq!(c[2], Classificacao::ForaDaJanela, "80 ms não cabe em 50 ms");
    }

    #[test]
    fn acorde_aceita_inversao_dobra_e_rejeita_nota_extra_ou_errada() {
        let mut e = em(7);
        let c = toca(&mut e, &[60, 72, 64, 67], 0, 20); // Dó, Dó dobrado, Mi, Sol
        assert_eq!(c[3], Classificacao::PassoCompleto);
        // Passo 2: nota fora do acorde conta um erro e limpa o grupo.
        assert_eq!(
            e.classificar(true, n(64), 1_000 * MS),
            Classificacao::ParteCerta
        );
        assert_eq!(
            e.classificar(true, n(63), 1_010 * MS),
            Classificacao::Errada
        );
        assert_eq!(e.tentativa.erros, 1);
        let c = toca(&mut e, &[64, 67, 60], 2_000, 10);
        assert_eq!(c[2], Classificacao::PassoCompleto);
    }

    #[test]
    fn tercas_e_quintas_exigem_as_notas_exatas() {
        let mut e = em(5);
        // Dó-Sol no lugar de Dó-Mi: Sol não pertence ao passo.
        assert_eq!(e.classificar(true, n(60), 0), Classificacao::ParteCerta);
        assert_eq!(e.classificar(true, n(67), 10 * MS), Classificacao::Errada);
        let c = toca(&mut e, &[64, 60], 1_000, 20);
        assert_eq!(c[1], Classificacao::PassoCompleto);
        // Uma sexta (Mi acima) não vale como terça: a oitava é exata.
        assert_eq!(
            e.classificar(true, n(77), 2_000 * MS),
            Classificacao::Errada
        );
    }

    #[test]
    fn arpejo_exige_a_ordem_correta() {
        let mut e = em(9);
        // Mi antes de Dó: errado; depois a ordem correta Dó Mi Sol Dó Sol Mi Dó.
        assert_eq!(e.classificar(true, n(64), 0), Classificacao::Errada);
        let c = toca(&mut e, &[60, 64, 67, 72, 67, 64, 60], 100, 300);
        assert!(c[..6].iter().all(|x| *x == Classificacao::PassoCompleto));
        assert_eq!(c[6], Classificacao::ExercicioCompleto);
        assert_eq!(e.tentativa.erros, 1);

        // Ordem trocada no meio: Sol antes de Mi.
        let mut e = em(9);
        let c = toca(&mut e, &[60, 67, 64, 67, 72], 0, 300);
        assert_eq!(
            c,
            vec![
                Classificacao::PassoCompleto,
                Classificacao::Errada,
                Classificacao::PassoCompleto,
                Classificacao::PassoCompleto,
                Classificacao::PassoCompleto
            ]
        );
    }

    #[test]
    fn ouvir_e_repetir_esconde_as_guias_e_a_dica_as_mostra() {
        let mut e = em(10);
        assert!(e.guias().is_empty());
        assert_eq!(e.passos_para_ouvir().len(), 6);
        assert_eq!(e.classificar(true, n(60), 0), Classificacao::Errada);
        assert!(
            !e.feedback_texto().contains("esperado"),
            "o ouvido não revela a nota"
        );
        e.config.mostrar_dica = true;
        assert_eq!(e.guias(), vec![n(64)]);
        // Nos demais exercícios as guias aparecem sempre.
        let mut e = em(6);
        assert_eq!(e.guias(), vec![n(60)]);
        toca(&mut e, &[60], 0, 0);
        assert_eq!(e.guias(), vec![n(62)]);
    }

    #[test]
    fn progressao_avanca_retrocede_e_bloqueia() {
        let mut e = EstadoExercicios::novo(None);
        assert!(e.desbloqueado(0));
        assert!(!e.desbloqueado(1));
        assert!(!e.desbloqueado(99_999));
        assert_eq!(
            e.motivo_bloqueio(1).as_deref(),
            Some("conclua antes o exercício 1: Encontrar o Dó central")
        );
        assert!(!e.avancar(), "bloqueado até concluir o 1");
        assert!(!e.selecionar(5));
        assert!(!e.voltar());
        assert_eq!(e.indice(), 0);
        toca(&mut e, &[60], 0, 0);
        assert!(e.concluido(0));
        assert!(e.desbloqueado(1));
        assert!(!e.desbloqueado(2));
        assert!(e.avancar());
        assert_eq!(e.indice(), 1);
        assert_eq!(
            e.tentativa.fase,
            Fase::Pronto,
            "avançar reinicia a tentativa"
        );
        assert!(!e.avancar(), "o 3 só abre após concluir o 2");
        assert!(e.voltar());
        assert_eq!(e.indice(), 0);
        assert!(e.concluido(0), "voltar não perde o progresso");
        // Concluir tudo libera todos; avançar para depois do último falha.
        for p in &mut e.progresso {
            p.concluido = true;
        }
        let ultimo = e.exercicios.len() - 1;
        assert!(e.selecionar(ultimo));
        assert!(!e.avancar());
    }

    #[test]
    fn reiniciar_e_repetir() {
        let mut e = em(3);
        toca(&mut e, &[64, 60], 0, 10);
        assert_eq!(e.tentativa.passo, 2);
        e.repetir();
        assert_eq!(e.tentativa.passo, 2, "repetir mantém o passo");
        assert_eq!(e.tentativa.erros, 0);
        assert!(e.feedback_texto().starts_with("[<]"));
        e.reiniciar();
        assert_eq!(e.tentativa.passo, 0);
        assert_eq!(e.tentativa.acertos, 0);
        assert_eq!(e.historico_atual().acertos, 2, "histórico acumulado fica");
        // Concluído: repetir recomeça.
        toca(&mut e, &[64, 60, 67], 100, 10);
        assert_eq!(e.tentativa.fase, Fase::Concluido);
        assert!(e.guias().is_empty());
        e.repetir();
        assert_eq!(e.tentativa.fase, Fase::Pronto);
        assert_eq!(e.historico_atual().vezes, 1);
    }

    #[test]
    fn pausa_e_limite_de_tempo_opcionais() {
        let mut e = em(3);
        // Padrão: sem limite, por mais longa que seja a espera.
        assert_eq!(e.config.limite_s, 0);
        toca(&mut e, &[64], 0, 0);
        assert!(!e.verificar_tempo(10_000_000 * MS));
        // Pausa: notas não contam.
        e.pausar(1_000 * MS);
        assert_eq!(
            e.classificar(true, n(1), 1_100 * MS),
            Classificacao::Ignorada
        );
        assert_eq!(e.tentativa.erros, 0);
        e.retomar(1_500 * MS);
        // Limite de 5 s, descontando a pausa.
        e.config.limite_s = 5;
        assert!(!e.verificar_tempo(5_400 * MS));
        assert!(e.verificar_tempo(6_600 * MS));
        assert_eq!(e.tentativa.passo, 0);
        assert!(e.feedback_texto().starts_with("[!]"));
    }

    #[test]
    fn progresso_ida_e_volta_em_texto() {
        let mut e = em(4);
        e.config.janela_ms = 220;
        e.config.limite_s = 30;
        e.config.mostrar_dica = true;
        toca(&mut e, &[61, 63, 66, 70, 68], 0, 10);
        e.nota(true, n(1), 999 * MS);
        let texto = e.para_texto();
        let mut b = EstadoExercicios::novo(None);
        b.aplicar_texto(&texto);
        assert_eq!(b.progresso, e.progresso);
        assert_eq!(b.config, e.config);
        assert_eq!(b.indice(), 3);
        assert_eq!(b.para_texto(), texto);
        assert!(b.concluido(3));
        assert!(b.desbloqueado(4));
    }

    #[test]
    fn arquivo_corrompido_volta_aos_padroes_sem_panico() {
        let lixos: [&str; 8] = [
            "",
            "\0\0\0 isto não é progresso \u{fffd}\u{fffd}",
            "ex1.acertos=abc\nex1.erros=-4\nex99.acertos=7\njanela_ms=999999\natual=zzz\n===\n=\nex.=1\nex1.=",
            "ex3.concluido=1\nex2.concluido=0\natual=3",
            "ex1.acertos=99999999999999999999999\nlimite_s=9999999999",
            "janela_ms=0\njanela_ms=\ndica",
            "atual=10\nex10.concluido=1",
            "ex1.concluido=1\natual=7",
        ];
        for l in lixos {
            let mut e = EstadoExercicios::novo(None);
            e.aplicar_texto(l);
            assert!(e.indice() < 10);
            assert!(e.desbloqueado(e.indice()), "atual bloqueado em {l:?}");
            assert!((JANELA_MIN_MS..=JANELA_MAX_MS).contains(&e.config.janela_ms));
            assert!(e.config.limite_s <= 3600);
            // Nunca concluído sem o pré-requisito.
            for i in 1..10 {
                assert!(!e.concluido(i) || e.concluido(i - 1));
            }
        }
        let mut e = EstadoExercicios::novo(None);
        e.aplicar_texto("ex3.concluido=1\natual=3");
        assert!(!e.concluido(2));
        assert_eq!(e.indice(), 0);
    }

    #[test]
    fn arquivo_binario_no_disco_usa_padroes() {
        let pasta = pasta_unica("binario");
        std::fs::create_dir_all(&pasta).expect("pasta");
        std::fs::write(
            pasta.join(ARQUIVO_PROGRESSO),
            [
                0xff, 0xfe, 0x00, 0x9f, b'=', 0xc0, 0x80, b'\n', b'e', b'x', b'1',
            ],
        )
        .expect("escrever");
        let e = EstadoExercicios::novo(Some(pasta.clone()));
        assert_eq!(e.indice(), 0);
        assert!(e.progresso.iter().all(|p| *p == Progresso::default()));
        assert_eq!(e.config, ConfigExercicios::default());
        let _ = std::fs::remove_dir_all(pasta);
    }

    #[test]
    fn progresso_sobrevive_a_reinicializacao_e_pode_ser_apagado_e_exportado() {
        let pasta = pasta_unica("disco");
        let mut e = EstadoExercicios::novo(Some(pasta.clone()));
        toca(&mut e, &[60], 0, 0); // conclui o 1 e salva sozinho
        assert!(e.avancar());
        e.nota(true, n(62), 10 * MS); // erro no exercício 2
        e.salvar_se_sujo();
        assert!(pasta.join(ARQUIVO_PROGRESSO).exists());

        let b = EstadoExercicios::novo(Some(pasta.clone()));
        assert_eq!(b.progresso, e.progresso);
        assert_eq!(b.indice(), 1);
        assert!(b.concluido(0));

        let rel = e.exportar().expect("exportar");
        assert_eq!(rel, pasta.join(ARQUIVO_EXPORTADO));
        let txt = std::fs::read_to_string(&rel).expect("ler relatório");
        assert!(txt.contains("1. Encontrar o Dó central"));
        assert!(txt.contains(&format!("Concluídos: 1 de {}", e.exercicios.len())));
        assert!(txt.contains("bloqueado"));

        e.apagar_progresso();
        assert!(!pasta.join(ARQUIVO_PROGRESSO).exists());
        assert!(e.progresso.iter().all(|p| *p == Progresso::default()));
        assert_eq!(e.indice(), 0);
        assert_eq!(e.sessao, Contagem::default());
        let c = EstadoExercicios::novo(Some(pasta.clone()));
        assert!(!c.concluido(0));
        let _ = std::fs::remove_dir_all(pasta);
    }

    #[test]
    fn exportar_sem_pasta_informa_em_vez_de_falhar() {
        let mut e = EstadoExercicios::novo(None);
        assert!(e.exportar().is_err());
        assert!(e.mensagem.is_some());
        e.apagar_progresso();
    }

    #[test]
    fn gabaritos_batem_com_a_teoria() {
        let ex = exercicios_de_fabrica();
        // Cada tríade do exercício 8 é identificada pela teoria como o acorde do nome.
        for p in &ex[7].passos {
            let a = piano_tutor_core::teoria::identificar_acorde(&p.notas).expect("acorde");
            let maior = a.qualidade == piano_tutor_core::teoria::Qualidade::Maior;
            let menor = a.qualidade == piano_tutor_core::teoria::Qualidade::Menor;
            assert!(
                (maior && p.legenda.ends_with("maior")) || (menor && p.legenda.ends_with("menor")),
                "{} x {}",
                p.legenda,
                a.cifra()
            );
        }
        // Escala do exercício 6 = escala de Dó maior da teoria.
        let escala: Vec<NotaMidi> = piano_tutor_core::teoria::TipoEscala::Maior.notas(n(60));
        let passos: Vec<NotaMidi> = ex[5].passos.iter().map(|p| p.notas[0]).collect();
        assert_eq!(passos, escala);
    }
    // --- F05-5: compasso do erro e replay da tentativa ---------------------------------------

    /// Exercício 2 (Dó Ré Mi Fá Sol ...), grade a `bpm` bpm. Toques `(nota, instante em ms)`.
    fn tentativa_com_erros(bpm: u16) -> EstadoExercicios {
        let mut e = em(2);
        e.config.bpm_grade = bpm;
        for (x, t_ms) in [
            (60u8, 5000u64), // certo, compasso 1 tempo 1 (início da grade)
            (62, 6000),      // certo
            (66, 7200),      // erro (esperava Mi)
            (64, 8000),      // certo
            (65, 9000),      // certo
            (61, 10_500),    // erro (esperava Sol)
        ] {
            e.classificar(true, n(x), t_ms * MS);
        }
        e
    }

    #[test]
    fn erro_e_localizado_no_compasso_e_tempo_da_grade_4_por_4() {
        // 60 bpm: 1 s por tempo, 4 s por compasso, contando da primeira nota (5 s).
        let e = tentativa_com_erros(60);
        let erros = e.erros_localizados();
        assert_eq!(erros.len(), 2);
        assert_eq!(erros[0].nota, n(66));
        assert_eq!(erros[0].passo, 2);
        assert_eq!(erros[0].esperado, vec![n(64)]);
        // 7,2 s - 5 s = 2,2 s: tempo 3 do compasso 1.
        assert_eq!(
            erros[0].local,
            LocalNaGrade {
                compasso: 1,
                pulso: 3
            }
        );
        // 10,5 s - 5 s = 5,5 s: tempo 2 do compasso 2.
        assert_eq!(erros[1].nota, n(61));
        assert_eq!(erros[1].passo, 4);
        assert_eq!(erros[1].esperado, vec![n(67)]);
        assert_eq!(
            erros[1].local,
            LocalNaGrade {
                compasso: 2,
                pulso: 2
            }
        );
        let t = erros[1].texto();
        assert!(t.starts_with("[X]"), "{t}");
        assert!(t.contains("compasso 2, tempo 2"), "{t}");
        assert!(t.contains("passo 5"), "{t}");
        assert!(t.contains("esperado: Sol4"), "{t}");
        // Os acertos não aparecem como erro.
        assert!(erros.iter().all(|x| x.nota != n(62)));
    }

    #[test]
    fn andamento_da_grade_muda_o_compasso_do_erro() {
        // 120 bpm: 0,5 s por tempo, 2 s por compasso. 2,2 s depois da primeira nota = tempo 1
        // do compasso 2; 5,5 s = tempo 4 do compasso 3.
        let e = tentativa_com_erros(120);
        let l: Vec<LocalNaGrade> = e.erros_localizados().iter().map(|x| x.local).collect();
        assert_eq!(
            l,
            vec![
                LocalNaGrade {
                    compasso: 2,
                    pulso: 1
                },
                LocalNaGrade {
                    compasso: 3,
                    pulso: 4
                },
            ]
        );
    }

    #[test]
    fn sem_toques_nao_ha_grade_nem_erros_nem_replay() {
        let e = em(2);
        assert!(e.grade().is_none());
        assert!(e.local_na_grade(1).is_none());
        assert!(e.erros_localizados().is_empty());
        assert!(e.tentativa_como_gravacao().is_none());
        // Só acertos: sem erro listado.
        let mut e = em(2);
        toca(&mut e, &[60, 62], 0, 400);
        assert!(e.erros_localizados().is_empty());
    }

    #[test]
    fn bpm_da_grade_e_salvo_e_valores_fora_da_faixa_sao_ignorados() {
        let mut e = em(1);
        e.config.bpm_grade = 90;
        let texto = e.para_texto();
        assert!(texto.contains("grade_bpm=90"));
        let mut outro = em(1);
        outro.aplicar_texto(&texto);
        assert_eq!(outro.config.bpm_grade, 90);
        for ruim in ["grade_bpm=5", "grade_bpm=900", "grade_bpm=abc"] {
            outro.aplicar_texto(ruim);
            assert_eq!(outro.config.bpm_grade, BPM_GRADE_PADRAO, "{ruim}");
        }
    }

    #[test]
    fn tentativa_vira_gravacao_da_f06_com_o_que_o_aluno_tocou() {
        let mut e = tentativa_com_erros(60);
        // NoteOff e nota depois de pausar não entram.
        e.classificar(false, n(60), 11_000 * MS);
        let g = e.tentativa_como_gravacao().expect("gravação");
        assert_eq!(g.total_notas(), 6, "seis toques, certos e errados");
        let ligadas: Vec<(u8, u64)> = g
            .eventos
            .iter()
            .filter_map(|x| match x.msg {
                Mensagem::NotaLigada { nota, .. } => Some((nota, x.t_ns / MS)),
                _ => None,
            })
            .collect();
        // Ordem e instantes relativos à primeira nota (5 s).
        assert_eq!(
            ligadas,
            vec![
                (60, 0),
                (62, 1000),
                (66, 2200),
                (64, 3000),
                (65, 4000),
                (61, 5500)
            ]
        );
        // Cada nota solta antes da próxima, e a última em até 400 ms.
        for par in g.notas().windows(2) {
            assert!(par[0].fim_ns > par[0].inicio_ns);
            assert!(par[0].fim_ns < par[1].inicio_ns);
        }
        let ultima = g.notas().pop().expect("nota");
        assert_eq!(ultima.fim_ns - ultima.inicio_ns, 400 * MS);
        assert_eq!(g.duracao_ns, 5900 * MS);
        // É uma gravação F06 de verdade: exporta para .mid e reimporta com as mesmas notas.
        let de_novo = Gravacao::de_smf(&g.para_smf().expect("exporta"), "t").expect("lê");
        let m = |x: &Gravacao| x.eventos.iter().map(|v| v.msg).collect::<Vec<_>>();
        assert_eq!(m(&g), m(&de_novo));
    }
    #[test]
    fn progresso_v1_migra_para_ids_da_licao_e_v2_volta_igual() {
        let mut e = EstadoExercicios::novo(None);
        e.aplicar_texto("ex1.concluido=1\nex1.acertos=3\nex2.concluido=1\nex2.erros=2\natual=3");
        assert!(e.concluido(0) && e.concluido(1));
        assert_eq!(e.progresso[0].acertos, 3);
        assert_eq!(e.progresso[1].erros, 2);
        assert_eq!(
            e.exercicio().map(|x| x.id.as_str()),
            Some("primeiros-passos/3")
        );
        let texto = e.para_texto();
        assert!(texto.contains("ex.primeiros-passos/1.acertos=3"), "{texto}");
        assert!(texto.contains("atual=primeiros-passos/3"));
        assert!(
            !texto.contains("ex.primeiros-passos/4."),
            "exercício sem progresso não é gravado"
        );
        let mut b = EstadoExercicios::novo(None);
        b.aplicar_texto(&texto);
        assert_eq!(b.progresso, e.progresso);
        assert_eq!(b.indice(), e.indice());
    }

    #[test]
    fn licao_que_requer_outra_fica_bloqueada_ate_concluir_a_outra() {
        let mut avisos = Vec::new();
        let base = |id: &str, requer: &str| {
            format!(
                "[licao]\nid = \"{id}\"\ntrilha = \"T\"\ntitulo = \"{id}\"\nrequer = [{requer}]\n\n\
                 [[exercicio]]\nid = \"a\"\ntitulo = \"A\"\ntipo = \"sequencia\"\n\
                 objetivo = \"o\"\ninstrucoes = \"i\"\npassos = \"C4\"\n"
            )
        };
        let l1 = crate::licoes::ler_licao(&base("um", ""), Origem::Fabrica("1"), &mut avisos)
            .expect("um");
        let l2 =
            crate::licoes::ler_licao(&base("dois", "\"um\""), Origem::Fabrica("2"), &mut avisos)
                .expect("dois");
        let mut e = EstadoExercicios::com_licoes(vec![l1, l2], None);
        assert!(e.desbloqueado(0));
        assert!(!e.desbloqueado(1));
        assert_eq!(
            e.motivo_bloqueio(1).as_deref(),
            Some("conclua antes a Lição um")
        );
        toca(&mut e, &[60], 0, 0);
        assert!(e.licao_concluida(0));
        assert_eq!(e.andamento_licao(0), (1, 1));
        assert!(e.desbloqueado(1));
        assert!(e.avancar());
        assert_eq!(e.licao_atual().map(|l| l.id.as_str()), Some("dois"));
    }
}
