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
//! Pendente (fora desta entrega): replay do trecho com a Gravação (F06) e indicação do
//! compasso do erro com a grade (F08). O registro de eventos da tentativa
//! (`Tentativa::registro`) já guarda instante, nota, passo e classificação para essa ligação.

use piano_tutor_core::NotaMidi;
use piano_tutor_core::teclado::{ConfigNomes, rotulo};
use std::fmt::Write as _;
use std::path::PathBuf;

/// Janela padrão para aceitar um conjunto de notas como "juntas".
pub const JANELA_PADRAO_MS: u32 = 150;
/// Limites aceitos para a janela.
pub const JANELA_MIN_MS: u32 = 10;
pub const JANELA_MAX_MS: u32 = 2000;
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

/// Exercício de fábrica: objetivo, instruções, gabarito, pré-requisito e feedback.
#[derive(Clone, Debug, PartialEq)]
pub struct Exercicio {
    /// Número 1..=N, estável (usado no arquivo de progresso).
    pub id: u8,
    pub titulo: &'static str,
    pub tipo: Tipo,
    pub objetivo: &'static str,
    pub instrucoes: &'static str,
    /// Dedilhado sugerido (mão direita); vazio quando não se aplica.
    pub dedilhado: &'static str,
    /// Gabarito: passos na ordem exigida.
    pub passos: Vec<Passo>,
    /// Exercício que precisa estar concluído antes.
    pub prerequisito: Option<u8>,
    /// Mensagem a cada passo certo.
    pub feedback_acerto: &'static str,
    /// Orientação a cada nota errada.
    pub feedback_erro: &'static str,
    /// Mensagem ao concluir.
    pub conclusao: &'static str,
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

/// Os 10 exercícios progressivos de fábrica.
pub fn exercicios_de_fabrica() -> Vec<Exercicio> {
    let seq = |xs: &[u8]| xs.iter().map(|x| Passo::nota(*x)).collect::<Vec<_>>();
    vec![
        Exercicio {
            id: 1,
            titulo: "Encontrar o Dó central",
            tipo: Tipo::Localizar,
            objetivo: "Localizar o Dó central (Dó4), a nota de referência do teclado.",
            instrucoes: "O Dó central é a tecla branca logo à esquerda do grupo de duas teclas pretas, no meio do teclado. Toque essa tecla.",
            dedilhado: "",
            passos: seq(&[60]),
            prerequisito: None,
            feedback_acerto: "Esse é o Dó central, o Dó do meio do teclado.",
            feedback_erro: "Procure o grupo de duas teclas pretas perto do meio e toque a branca à esquerda dele.",
            conclusao: "Você achou o Dó central. Todas as outras notas se medem a partir dele.",
        },
        Exercicio {
            id: 2,
            titulo: "Notas brancas de Dó a Sol",
            tipo: Tipo::Sequencia,
            objetivo: "Tocar as cinco primeiras notas brancas, subindo e descendo.",
            instrucoes: "Toque Dó, Ré, Mi, Fá, Sol; depois Sol, Fá, Mi, Ré, Dó, uma tecla por vez, na ordem.",
            dedilhado: "Mão direita: 1-2-3-4-5-5-4-3-2-1",
            passos: seq(&[60, 62, 64, 65, 67, 67, 65, 64, 62, 60]),
            prerequisito: Some(1),
            feedback_acerto: "Nota certa, siga para a próxima.",
            feedback_erro: "Veja a tecla marcada no teclado e toque-a; a anterior continua valendo.",
            conclusao: "Dez notas na ordem certa: você domina as cinco notas brancas de Dó a Sol.",
        },
        Exercicio {
            id: 3,
            titulo: "Três notas em ordem",
            tipo: Tipo::Sequencia,
            objetivo: "Tocar três notas em ordem a partir de uma instrução escrita.",
            instrucoes: "Toque Mi, depois Dó, depois Sol (todas na região do Dó central).",
            dedilhado: "Mão direita: 3-1-5",
            passos: seq(&[64, 60, 67]),
            prerequisito: Some(2),
            feedback_acerto: "Certo, essa nota veio na ordem pedida.",
            feedback_erro: "A ordem é Mi, Dó, Sol. Toque a nota marcada; sem pressa.",
            conclusao: "Três notas na ordem certa.",
        },
        Exercicio {
            id: 4,
            titulo: "Sustenidos e bemóis",
            tipo: Tipo::Sequencia,
            objetivo: "Localizar as teclas pretas (sustenidos e bemóis).",
            instrucoes: "Toque Dó#, Mib, Fá#, Sib, Sol# (as teclas pretas). Dó# e Réb são a mesma tecla.",
            dedilhado: "",
            passos: seq(&[61, 63, 66, 70, 68]),
            prerequisito: Some(3),
            feedback_acerto: "Tecla preta certa.",
            feedback_erro: "Tecle a preta marcada: sustenido (#) fica à direita da branca, bemol (b) à esquerda da próxima.",
            conclusao: "Cinco teclas pretas encontradas.",
        },
        Exercicio {
            id: 5,
            titulo: "Intervalos: terças e quintas",
            tipo: Tipo::Intervalos,
            objetivo: "Tocar duas notas juntas e reconhecer terças e quintas.",
            instrucoes: "Toque as duas notas ao mesmo tempo: Dó-Mi (terça maior), Ré-Fá (terça menor), Dó-Sol (quinta justa), Sol-Ré (quinta justa).",
            dedilhado: "Mão direita: 1-3 para terças, 1-5 para quintas",
            passos: vec![
                Passo::exato(&[60, 64], "terça maior"),
                Passo::exato(&[62, 65], "terça menor"),
                Passo::exato(&[60, 67], "quinta justa"),
                Passo::exato(&[67, 74], "quinta justa"),
            ],
            prerequisito: Some(4),
            feedback_acerto: "Intervalo certo, as duas notas juntas.",
            feedback_erro: "Toque só as duas notas marcadas, juntas, como um golpe só.",
            conclusao: "Quatro intervalos (terças e quintas) tocados juntos.",
        },
        Exercicio {
            id: 6,
            titulo: "Escala de Dó maior subindo",
            tipo: Tipo::Escala,
            objetivo: "Tocar a escala de Dó maior, uma oitava, subindo.",
            instrucoes: "Toque Dó, Ré, Mi, Fá, Sol, Lá, Si, Dó (oitava acima), em ordem e sem pular nota.",
            dedilhado: "Mão direita: 1-2-3-1-2-3-4-5",
            passos: seq(&[60, 62, 64, 65, 67, 69, 71, 72]),
            prerequisito: Some(5),
            feedback_acerto: "Degrau certo da escala.",
            feedback_erro: "A escala de Dó maior só usa teclas brancas; toque a próxima marcada.",
            conclusao: "Escala de Dó maior completa, subindo.",
        },
        Exercicio {
            id: 7,
            titulo: "Tríade de Dó maior",
            tipo: Tipo::Acorde,
            objetivo: "Tocar Dó, Mi e Sol ao mesmo tempo, em três posições.",
            instrucoes: "Toque as três notas juntas (dentro da janela de tempo). Vale qualquer inversão e notas dobradas em outra oitava.",
            dedilhado: "Mão direita: 1-3-5",
            passos: vec![
                Passo::acorde(&[60, 64, 67], "Dó maior"),
                Passo::acorde(&[64, 67, 72], "Dó maior, 1ª inversão"),
                Passo::acorde(&[67, 72, 76], "Dó maior, 2ª inversão"),
            ],
            prerequisito: Some(6),
            feedback_acerto: "Acorde completo, as notas soaram juntas.",
            feedback_erro: "Dó maior é só Dó, Mi e Sol; toque as três ao mesmo tempo, sem nota a mais.",
            conclusao: "Tríade de Dó maior em três posições.",
        },
        Exercicio {
            id: 8,
            titulo: "Tríades maiores e menores",
            tipo: Tipo::Acorde,
            objetivo: "Diferenciar acordes maiores e menores.",
            instrucoes: "Toque Dó maior, Lá menor, Sol maior, Mi menor, Fá maior e Ré menor, cada um com as notas juntas.",
            dedilhado: "Mão direita: 1-3-5",
            passos: vec![
                Passo::acorde(&[60, 64, 67], "Dó maior"),
                Passo::acorde(&[57, 60, 64], "Lá menor"),
                Passo::acorde(&[55, 59, 62], "Sol maior"),
                Passo::acorde(&[52, 55, 59], "Mi menor"),
                Passo::acorde(&[53, 57, 60], "Fá maior"),
                Passo::acorde(&[50, 53, 57], "Ré menor"),
            ],
            prerequisito: Some(7),
            feedback_acerto: "Acorde certo.",
            feedback_erro: "Maior tem a terça maior (4 semitons); menor tem a terça menor (3 semitons). Confira a nota do meio.",
            conclusao: "Seis tríades, maiores e menores, tocadas corretamente.",
        },
        Exercicio {
            id: 9,
            titulo: "Arpejo de Dó maior",
            tipo: Tipo::Arpejo,
            objetivo: "Tocar as notas do acorde uma de cada vez, na sequência correta.",
            instrucoes: "Toque Dó, Mi, Sol, Dó (oitava acima) e desça Sol, Mi, Dó, uma nota de cada vez, na ordem.",
            dedilhado: "Mão direita subindo: 1-2-3-5; descendo: 5-3-2-1",
            passos: seq(&[60, 64, 67, 72, 67, 64, 60]),
            prerequisito: Some(8),
            feedback_acerto: "Nota do arpejo na ordem certa.",
            feedback_erro: "No arpejo a ordem importa: toque a nota marcada, uma de cada vez.",
            conclusao: "Arpejo de Dó maior completo, subindo e descendo.",
        },
        Exercicio {
            id: 10,
            titulo: "Ouvir e repetir",
            tipo: Tipo::OuvirERepetir,
            objetivo: "Repetir de ouvido uma frase curta tocada pelo app.",
            instrucoes: "Use Ouvir para escutar a frase de seis notas e toque-a no teclado. As teclas não são marcadas; ouça quantas vezes quiser. Use a dica se travar.",
            dedilhado: "",
            passos: seq(&[64, 64, 65, 67, 67, 65]),
            prerequisito: Some(9),
            feedback_acerto: "Você ouviu e acertou essa nota.",
            feedback_erro: "Essa nota não é a da frase. Ouça de novo (botão Ouvir) e tente outra.",
            conclusao: "Frase repetida de ouvido, sem ajuda visual.",
        },
    ]
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
    /// Registro das notas tocadas (base para o replay da F06 e o compasso da F08).
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
}

impl Default for ConfigExercicios {
    fn default() -> Self {
        Self {
            janela_ms: JANELA_PADRAO_MS,
            limite_s: 0,
            mostrar_dica: false,
        }
    }
}

/// Estado do professor assistido.
#[derive(Clone, Debug)]
pub struct EstadoExercicios {
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

impl EstadoExercicios {
    pub fn novo(pasta: Option<PathBuf>) -> Self {
        Self::com_exercicios(exercicios_de_fabrica(), pasta)
    }

    /// Estado com exercícios próprios (testes e lições futuras). Lê o progresso da pasta.
    pub fn com_exercicios(exercicios: Vec<Exercicio>, pasta: Option<PathBuf>) -> Self {
        let n = exercicios.len();
        let mut e = Self {
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
        match ex.prerequisito {
            None => true,
            Some(id) => self
                .exercicios
                .iter()
                .position(|x| x.id == id)
                .is_some_and(|i| self.concluido(i)),
        }
    }

    /// Texto do bloqueio ("conclua o exercício N").
    pub fn motivo_bloqueio(&self, indice: usize) -> Option<String> {
        if self.desbloqueado(indice) {
            return None;
        }
        let id = self.exercicios.get(indice)?.prerequisito?;
        Some(format!("conclua antes o exercício {id}"))
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
                ex.map(|e| e.feedback_acerto).unwrap_or_default()
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
                    ex.map(|e| e.feedback_erro).unwrap_or_default()
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
                ex.map(|e| e.conclusao).unwrap_or_default()
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
        let mut s = String::from("# Piano Tutor: progresso dos exercícios v1\n");
        let _ = writeln!(s, "janela_ms={}", self.config.janela_ms);
        let _ = writeln!(s, "limite_s={}", self.config.limite_s);
        let _ = writeln!(s, "dica={}", u8::from(self.config.mostrar_dica));
        if let Some(e) = self.exercicio() {
            let _ = writeln!(s, "atual={}", e.id);
        }
        for (ex, p) in self.exercicios.iter().zip(&self.progresso) {
            let id = ex.id;
            let _ = writeln!(s, "ex{id}.acertos={}", p.acertos);
            let _ = writeln!(s, "ex{id}.erros={}", p.erros);
            let _ = writeln!(s, "ex{id}.concluido={}", u8::from(p.concluido));
            let _ = writeln!(s, "ex{id}.vezes={}", p.vezes);
        }
        s
    }

    /// Aplica um texto de progresso. Linhas inválidas, chaves desconhecidas e valores fora
    /// de faixa são ignorados (ficam no padrão); nunca falha.
    pub fn aplicar_texto(&mut self, texto: &str) {
        self.progresso = vec![Progresso::default(); self.exercicios.len()];
        self.config = ConfigExercicios::default();
        let mut atual_id: Option<u8> = None;
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
                "atual" => atual_id = v.parse().ok(),
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
            .and_then(|id| self.exercicios.iter().position(|e| e.id == id))
            .filter(|i| self.desbloqueado(*i))
            .unwrap_or(0);
        self.tentativa = Tentativa::default();
    }

    fn aplicar_chave_exercicio(&mut self, k: &str, v: &str) {
        let Some(resto) = k.strip_prefix("ex") else {
            return;
        };
        let Some((id, campo)) = resto.split_once('.') else {
            return;
        };
        let Some(i) = id
            .parse::<u8>()
            .ok()
            .and_then(|id| self.exercicios.iter().position(|e| e.id == id))
        else {
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
            let _ = writeln!(
                s,
                "{}. {} [{}]: {situacao}; acertos {}, erros {}, aproveitamento {aprov}",
                ex.id,
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
            assert_eq!(usize::from(x.id), i + 1);
            for t in [
                x.titulo,
                x.objetivo,
                x.instrucoes,
                x.feedback_acerto,
                x.feedback_erro,
                x.conclusao,
            ] {
                assert!(!t.is_empty(), "exercício {} com texto vazio", x.id);
            }
            assert!(!x.passos.is_empty());
            assert!(x.passos.iter().all(|p| !p.notas.is_empty()));
            assert_eq!(x.prerequisito, if i == 0 { None } else { Some(x.id - 1) });
        }
        assert_eq!(ex[1].passos.len(), 10, "o exercício 2 tem 10 notas");
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
        assert!(!e.desbloqueado(99));
        assert_eq!(
            e.motivo_bloqueio(1).as_deref(),
            Some("conclua antes o exercício 1")
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
        assert!(e.selecionar(9));
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
        assert!(txt.contains("Concluídos: 1 de 10"));
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
}
