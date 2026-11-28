//! Comando tipado de edição do serviço de documento (ticket 14, E01, seam S1): o contrato
//! `ComandoDeEdicao` para `ResultadoDaEdicao`, sem janela, sem teclado e sem áudio. A tela só
//! traduz tecla em comando e chama [`DocumentoAberto::aplicar`]; o resultado da tecla é, por
//! construção, o do comando chamado direto (critério A12).
//!
//! # Contrato em linguagem de domínio
//!
//! - **Comando é valor.** [`ComandoDeEdicao`] é um dado (`Clone`, `Eq`, `Debug`): pode ser
//!   guardado, comparado e repetido. Existem dois níveis:
//!   - [`ComandoDeEdicao::MudarAltura`]: a **intenção** (um semitom, um grau da escala ou uma
//!     oitava, para cima ou para baixo) sobre uma nota por [`IdNota`]; o serviço decide a grafia
//!     (letra, alteração e acidente impresso) pela armadura e pelo compasso;
//!   - [`ComandoDeEdicao::DefinirAltura`]: a **forma resolvida** (altura e acidente exatos). É o
//!     que o serviço devolve como `aplicado` e como `inverso`.
//! - **Reversível.** Todo resultado traz o `inverso`: aplicá-lo devolve a nota à altura e ao
//!   acidente de antes (o texto volta idêntico quando a edição só mexeu em `step`, `octave`,
//!   `alter` e `accidental` sem atributos). O desfazer e o refazer (ticket 15, módulo
//!   `historico`) não dependem desse valor: voltam o compasso inteiro pelo instantâneo.
//! - **Validação antes de tocar em qualquer coisa.** Nota inexistente, sem altura (pausa) ou
//!   altura fora dos limites ([`LimitesDeAltura`]: MIDI 0 a 127, a oitava 0 a 9 do MusicXML e o
//!   teclado de referência) são recusadas com [`ErroDeEdicao`] e **nada muda**: o documento, o
//!   modelo de tocar e as identidades ficam como estavam.
//! - **Nenhum comando injeta marcação (N7; a API vai ser exposta por MCP).** Todo texto livre
//!   que entra no XML por comando passa por validação estrita **antes** de qualquer mudança:
//!   o `acidente`/grafia de [`ComandoDeEdicao::DefinirAltura`], [`ComandoDeEdicao::InserirNota`],
//!   [`ComandoDeEdicao::AdicionarAltura`] e [`ComandoDeEdicao::RestaurarAltura`] (lista fechada de
//!   nomes `accidental-value` e de atributos do `<accidental>`; valor entre aspas, sem `<`, com
//!   `&` só em entidade; `>` dentro do valor vale) e a `voz` e a `pauta` de
//!   [`ComandoDeEdicao::InserirNota`] e [`ComandoDeEdicao::InserirPausa`] (letras, dígitos, `-`,
//!   `_`, `.`). Os demais comandos só carregam números, enums e `char` de `A` a `G`. Grafia
//!   inválida, inclusive a que vem do próprio arquivo (atributo fora do esquema), é recusada com
//!   [`ErroDeEdicao`] e nada muda. A grafia que o `inverso` guarda preserva os bytes da marca de
//!   abertura do `<accidental>` (espaços atípicos incluídos; ver `SEPARADOR_BRUTO` em `grafia`).
//! - **`DefinirAltura` sem efeito não é edição.** Mesma altura e a mesma grafia efetiva (um nome
//!   só vale para o mesmo nome, com os atributos que o elemento já tinha) devolve `alterou =
//!   false`, não suja o compasso e não cria entrada de histórico (N8).
//! - **Só o compasso da nota é sujo e regenerado.** O resultado lista o compasso afetado; o
//!   modelo de tocar é atualizado só na nota editada e é igual ao que sairia de reabrir o
//!   texto exportado (conferido em teste no corpus).
//! - **Consequências declaradas, nunca silenciosas.** O serviço não mexe em nenhuma outra nota.
//!   Quando a edição deixa algo inconsistente no desenho, devolve um [`AvisoDeEdicao`]:
//!   ligadura de prolongamento que passa a ligar alturas diferentes, ou nota seguinte do
//!   compasso que dependia do acidente da nota editada e ficou sem o sinal devido. Ligadura de
//!   expressão, viga, letra, cifra e dedilhado ficam intocados (não dependem da altura).
//! - **A identidade da nota não muda:** o mesmo [`IdNota`] continua apontando para a nota
//!   editada.
//! - **Estrutura (ticket 17).** [`ComandoDeEdicao::InserirNota`], [`ComandoDeEdicao::InserirPausa`]
//!   e [`ComandoDeEdicao::RemoverNota`] mexem na estrutura do compasso; o contrato está em
//!   `inserir_remover`. A posição de inserção é a coluna do layout (o início da nota no
//!   compasso, em semínimas, `Fracao` exata), nunca um pixel.
//! - **Acorde (ticket 18).** [`ComandoDeEdicao::AcrescentarLetra`],
//!   [`ComandoDeEdicao::AcrescentarIntervalo`], [`ComandoDeEdicao::AdicionarAltura`] e
//!   [`ComandoDeEdicao::RetirarAltura`] mexem nas alturas de um acorde; o contrato está em
//!   `acorde`.
//! - **Nada é gravado.** A edição vive em memória; exportar e gravar arquivo é decisão do
//!   chamador (autosave e Versão são o ticket 21).

use super::editar_duracao::MudancaDeDuracao;
use super::grafia::{
    Anterior, acidente_de, alter_do_acidente, alteracao_em_vigor, antes, de_partitura, midi,
    nome_da_altura, nome_do_acidente, nome_do_midi, para_partitura, por_grau, por_oitava,
    por_semitom,
};
use super::servico::DocumentoAberto;
use super::{Altura, ErroDocumento, IdNota, PosCompasso};
use crate::partitura::{self, Acidente, Figura, Fracao};
use std::fmt;

/// De quanto a altura muda.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnidadeDeAltura {
    /// Um semitom (a grafia depende da armadura e da direção).
    Semitom,
    /// Um grau da escala da armadura (a letra vizinha, com a alteração da armadura).
    GrauDaEscala,
    /// Uma oitava (mesma letra e alteração).
    Oitava,
}

/// Para onde a altura muda.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SentidoDaAltura {
    Acima,
    Abaixo,
}

/// Um comando de edição do documento. Ver o contrato do módulo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComandoDeEdicao {
    /// Muda a altura de `nota` (intenção; o serviço decide a grafia).
    MudarAltura {
        nota: IdNota,
        unidade: UnidadeDeAltura,
        sentido: SentidoDaAltura,
    },
    /// Põe `nota` exatamente em `altura`, com o `<accidental>` impresso dado (`None` = sem
    /// acidente impresso). Forma resolvida: é a do `aplicado` e a do `inverso`.
    DefinirAltura {
        nota: IdNota,
        altura: Altura,
        acidente: Option<String>,
    },
    /// Escolhe a figura de `nota` (as teclas 1 a 8 do perfil MuseScore 4), sem ponto: a
    /// **intenção** do usuário (ticket 16). Ver o contrato de duração em [`super::editar_duracao`].
    MudarDuracao { nota: IdNota, figura: Figura },
    /// Liga o ponto de aumento de `nota` (ou o desliga, se já tinha): a tecla `.`.
    AlternarPonto { nota: IdNota },
    /// Põe `nota` exatamente na figura e nos pontos dados (0 a 3): a **forma resolvida** de
    /// [`Self::MudarDuracao`] e [`Self::AlternarPonto`], a do `aplicado` e a do `inverso`.
    DefinirDuracao {
        nota: IdNota,
        figura: Figura,
        pontos: u8,
    },
    /// Insere uma nota na voz `voz` e pauta `pauta` do compasso `compasso`, começando em
    /// `posicao` (semínimas desde o início do compasso: a coluna do layout), com a `altura` e a
    /// figura dadas (ticket 17). O espaço tem de ser de pausas da mesma voz, que a nota substitui;
    /// o que sobrar delas continua pausa exata. `acidente` `None` = o serviço imprime o que a
    /// armadura e o compasso pedem. Quiáltera: herda a das pausas que ocupa.
    InserirNota {
        compasso: PosCompasso,
        voz: String,
        pauta: String,
        posicao: Fracao,
        altura: Altura,
        acidente: Option<String>,
        figura: Figura,
        pontos: u8,
    },
    /// Insere uma pausa (regrupa pausas livres; em cima de nota é recusado, ticket 17).
    InserirPausa {
        compasso: PosCompasso,
        voz: String,
        pauta: String,
        posicao: Fracao,
        figura: Figura,
        pontos: u8,
    },
    /// Remove `nota`: ela vira pausa de mesma duração e posição (ticket 17). Nota de acorde que
    /// não é a última do acorde e nota de adorno simplesmente saem.
    RemoverNota { nota: IdNota },
    /// Acrescenta ao acorde de `nota` a altura da `letra` (`A` a `G`) na oitava **mais próxima**
    /// de `nota` (a tecla Shift+letra; ticket 18). A alteração é a da armadura. A **intenção**:
    /// o serviço a resolve em [`Self::AdicionarAltura`].
    AcrescentarLetra { nota: IdNota, letra: char },
    /// Acrescenta ao acorde de `nota` a altura `graus` acima dela, na escala da armadura (2 =
    /// segunda ... 9 = nona; Alt+número no perfil MuseScore; ticket 18). A **intenção**.
    AcrescentarIntervalo { nota: IdNota, graus: u8 },
    /// Acrescenta ao acorde de `nota` exatamente `altura`, com o `<accidental>` impresso dado
    /// (`None` = o serviço imprime o que a armadura e o compasso pedem). A **forma resolvida**
    /// (ticket 18): a nova nota tem a mesma posição, duração, voz e pauta do acorde.
    AdicionarAltura {
        nota: IdNota,
        altura: Altura,
        acidente: Option<String>,
    },
    /// **Só como `inverso`** de [`Self::RemoverNota`] e [`Self::RetirarAltura`] num acorde de altura
    /// repetida (uníssono ou enarmônico): igual a [`Self::AdicionarAltura`], mas aceita a altura
    /// que já soa no acorde, porque o acorde que se restaura a tinha (N5 da segunda revisão da
    /// E01). Quem edita pela tela ou pela API usa `AdicionarAltura`, que continua recusando a
    /// altura repetida. A validação do acidente é a mesma.
    RestaurarAltura {
        nota: IdNota,
        altura: Altura,
        acidente: Option<String>,
    },
    /// Retira de um acorde a altura de `nota` (ticket 18). Com uma só altura (nota sem acorde),
    /// equivale a [`Self::RemoverNota`] (a nota vira pausa).
    RetirarAltura { nota: IdNota },
}

impl ComandoDeEdicao {
    /// A nota que o comando edita; `None` nos comandos de inserção (a nota ainda não existe).
    pub fn nota(&self) -> Option<IdNota> {
        match self {
            Self::MudarAltura { nota, .. }
            | Self::DefinirAltura { nota, .. }
            | Self::MudarDuracao { nota, .. }
            | Self::AlternarPonto { nota }
            | Self::DefinirDuracao { nota, .. }
            | Self::RemoverNota { nota }
            | Self::AcrescentarLetra { nota, .. }
            | Self::AcrescentarIntervalo { nota, .. }
            | Self::AdicionarAltura { nota, .. }
            | Self::RestaurarAltura { nota, .. }
            | Self::RetirarAltura { nota } => Some(*nota),
            Self::InserirNota { .. } | Self::InserirPausa { .. } => None,
        }
    }

    /// O comando muda a estrutura do compasso (ticket 17): insere ou remove nota ou pausa.
    pub(super) fn e_de_estrutura(&self) -> bool {
        matches!(
            self,
            Self::InserirNota { .. } | Self::InserirPausa { .. } | Self::RemoverNota { .. }
        )
    }

    /// O comando mexe nas alturas de um acorde (ticket 18).
    pub(super) fn e_de_acorde(&self) -> bool {
        matches!(
            self,
            Self::AcrescentarLetra { .. }
                | Self::AcrescentarIntervalo { .. }
                | Self::AdicionarAltura { .. }
                | Self::RestaurarAltura { .. }
                | Self::RetirarAltura { .. }
        )
    }

    /// O comando muda a duração (ticket 16) em vez da altura?
    pub(super) fn e_de_duracao(&self) -> bool {
        matches!(
            self,
            Self::MudarDuracao { .. } | Self::AlternarPonto { .. } | Self::DefinirDuracao { .. }
        )
    }
}

/// Figura escolhida por uma tecla de duração do perfil MuseScore 4: `1` = semifusa (1/64),
/// `2` = fusa, `3` = semicolcheia, `4` = colcheia, `5` = semínima, `6` = mínima, `7` =
/// semibreve, `8` = breve. A `9` (longa) não existe no modelo de partitura: recusa explícita,
/// nunca uma figura trocada em silêncio.
pub fn figura_da_tecla(tecla: u8) -> Result<Figura, ErroDeEdicao> {
    Ok(match tecla {
        1 => Figura::Semifusa,
        2 => Figura::Fusa,
        3 => Figura::Semicolcheia,
        4 => Figura::Colcheia,
        5 => Figura::Seminima,
        6 => Figura::Minima,
        7 => Figura::Semibreve,
        8 => Figura::Breve,
        outra => return Err(ErroDeEdicao::FiguraNaoSuportada(outra)),
    })
}

/// Nome da figura em português, para as mensagens.
pub fn nome_da_figura(figura: Figura) -> &'static str {
    match figura {
        Figura::Breve => "breve",
        Figura::Semibreve => "semibreve",
        Figura::Minima => "mínima",
        Figura::Seminima => "semínima",
        Figura::Colcheia => "colcheia",
        Figura::Semicolcheia => "semicolcheia",
        Figura::Fusa => "fusa",
        Figura::Semifusa => "semifusa",
        Figura::Quasifusa => "quartifusa",
    }
}

/// «colcheia», «semínima pontuada», «mínima com 2 pontos»...
fn figura_com_pontos(figura: Figura, pontos: u8) -> String {
    match pontos {
        0 => nome_da_figura(figura).to_owned(),
        1 => format!("{} pontuada", nome_da_figura(figura)),
        n => format!("{} com {n} pontos", nome_da_figura(figura)),
    }
}

/// Número do compasso para mensagens (o rótulo impresso, ou a posição se não houver).
fn rotulo_do_compasso(p: &PosCompasso) -> String {
    if p.numero.is_empty() {
        (p.indice + 1).to_string()
    } else {
        p.numero.clone()
    }
}

/// Faixa de alturas que os comandos aceitam, em números MIDI (Dó4 = 60).
///
/// Valem sempre, além dos limites do usuário, o MIDI 0 a 127 e a oitava 0 a 9 do MusicXML
/// (Dó0 = 12 até Sol9 = 127).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimitesDeAltura {
    pub grave: u8,
    pub agudo: u8,
}

impl LimitesDeAltura {
    /// Tudo o que o MIDI e o MusicXML representam.
    pub const MIDI_COMPLETO: Self = Self {
        grave: 0,
        agudo: 127,
    };
    /// Piano de 88 teclas, de Lá0 (21) a Dó8 (108): o teclado de referência da notação. O
    /// REVAS de 61 teclas tem faixa calibrada por instrumento e não é usado como limite de
    /// partitura (um arquivo pode escrever mais grave ou mais agudo do que o teclado alcança).
    pub const PIANO_88: Self = Self {
        grave: 21,
        agudo: 108,
    };

    /// Limites efetivos: a interseção com o que o MusicXML representa (Dó0 a Sol9).
    fn efetivos(self) -> (i32, i32) {
        (
            i32::from(self.grave).max(12),
            i32::from(self.agudo).min(127),
        )
    }
}

impl Default for LimitesDeAltura {
    fn default() -> Self {
        Self::PIANO_88
    }
}

/// Por que um comando foi recusado. Em todos os casos o documento fica como estava.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroDeEdicao {
    NotaInexistente(IdNota),
    /// Pausa, nota não afinada ou com microtom: não há altura para mudar.
    NotaSemAltura(IdNota),
    /// A altura pedida passa dos limites.
    ForaDosLimites {
        pedida: Altura,
        midi: i32,
        grave: i32,
        agudo: i32,
    },
    /// Outra recusa do documento (altura mal formada, texto inconsistente...).
    Documento(ErroDocumento),
    /// A nova duração faria a soma da voz passar do que o compasso comporta (A4). `ficaria` e
    /// `esperado` em semínimas, com `Fracao` exata.
    CompassoNaoFecha {
        compasso: PosCompasso,
        voz: String,
        ficaria: Fracao,
        esperado: Fracao,
    },
    /// Alongar a nota exigiria espaço livre (pausa) depois dela, e não há (`faltam` semínimas).
    SemPausaParaAlongar {
        nota: IdNota,
        faltam: Fracao,
    },
    /// A duração pedida não é múltipla de `1/divisions`: mudar `divisions` reescreveria a parte
    /// inteira e é recusado por padrão (nunca se arredonda).
    DuracaoExigeMudarDivisions {
        valor: Fracao,
        divisions: i64,
        necessarias: i64,
    },
    /// O resto de um encurtamento (ou de uma pausa consumida) não se escreve com figuras.
    PausaNaoRepresentavel(Fracao),
    /// Pausa: mudar a duração de pausas ainda não é suportado (a seleção só alcança notas).
    PausaNaoEditavel(IdNota),
    /// Nota de adorno não ocupa tempo: não há duração para mudar.
    NotaDeGracaSemDuracao(IdNota),
    /// A tecla de duração não tem figura no modelo (a `9`, longa, ou fora de `1..=9`).
    FiguraNaoSuportada(u8),
    /// Não se sabe a figura escrita da nota (sem `<type>` reconhecível nem dedução possível).
    ValorEscritoDesconhecido(IdNota),
    /// A posição pedida não existe: parte ou compasso inexistente, pauta fora das da parte,
    /// posição negativa ou depois do fim do compasso (ticket 17).
    PosicaoInvalida {
        compasso: PosCompasso,
        motivo: String,
    },
    /// Inserir não cabe ali: o lugar é de nota, ou a figura passa das pausas livres da voz
    /// (ticket 17). A soma do compasso não é tocada.
    SemEspacoParaInserir {
        compasso: PosCompasso,
        voz: String,
        posicao: Fracao,
        motivo: String,
    },
    /// Decisão do ticket 17: pausa sobre nota é recusada; a nota sai só por remoção explícita.
    PausaSobreNota {
        compasso: PosCompasso,
        voz: String,
        posicao: Fracao,
    },
    /// Remover uma pausa: não há o que remover (a pausa já é o vazio).
    PausaNaoRemovivel(IdNota),
    /// A altura pedida já soa no acorde (ticket 18): `nota` é a que já a tem.
    AlturaJaNoAcorde {
        nota: IdNota,
        altura: Altura,
    },
    /// Só nota que ocupa tempo forma acorde: pausa e nota de adorno são recusadas (ticket 18).
    AcordeSoEmNota(IdNota),
    /// Intervalo fora de 2 (segunda) a 9 (nona) (ticket 18).
    IntervaloInvalido(u8),
}

impl fmt::Display for ErroDeEdicao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotaInexistente(id) => write!(f, "A nota {} não existe mais.", id.0),
            Self::NotaSemAltura(_) => write!(
                f,
                "Esta nota não tem uma altura que possa ser mudada (pausa ou nota não afinada)."
            ),
            Self::ForaDosLimites {
                pedida,
                midi,
                grave,
                agudo,
            } => write!(
                f,
                "«{}» (MIDI {midi}) fica fora dos limites: de «{}» (MIDI {grave}) a «{}» \
                 (MIDI {agudo}). A nota não foi alterada.",
                nome_da_altura(*pedida),
                nome_do_midi(*grave),
                nome_do_midi(*agudo),
            ),
            Self::Documento(e) => write!(f, "{e}. A nota não foi alterada."),
            Self::CompassoNaoFecha {
                compasso,
                voz,
                ficaria,
                esperado,
            } => write!(
                f,
                "No compasso {}, a voz {voz} ficaria com {ficaria} tempo(s) de semínima, mas o \
                 compasso comporta {esperado}. A duração não foi alterada: encurte outra nota \
                 antes, ou escolha uma figura menor.",
                rotulo_do_compasso(compasso),
            ),
            Self::SemPausaParaAlongar { faltam, .. } => write!(
                f,
                "Não há pausa livre depois desta nota para alongá-la (faltam {faltam} tempo(s) de \
                 semínima). A duração não foi alterada."
            ),
            Self::DuracaoExigeMudarDivisions {
                valor,
                divisions,
                necessarias,
            } => write!(
                f,
                "Esta duração ({valor} de semínima) não cabe na grade de tempo do arquivo \
                 (divisions = {divisions}; seria preciso {necessarias}). Mudar divisions \
                 reescreve a parte inteira e é recusado por padrão. A duração não foi alterada."
            ),
            Self::PausaNaoRepresentavel(d) => write!(
                f,
                "O espaço que sobraria ({d} de semínima) não se escreve com uma pausa. A \
                 duração não foi alterada."
            ),
            Self::PausaNaoEditavel(_) => write!(
                f,
                "Mudar a duração de uma pausa ainda não é suportado: selecione uma nota."
            ),
            Self::NotaDeGracaSemDuracao(_) => write!(
                f,
                "Nota de adorno não ocupa tempo no compasso: não há duração para mudar."
            ),
            Self::FiguraNaoSuportada(9) => write!(
                f,
                "A longa (tecla 9) ainda não existe no modelo de partitura. Use as teclas 1 a 8."
            ),
            Self::FiguraNaoSuportada(t) => {
                write!(f, "A tecla {t} não escolhe uma duração (use 1 a 8).")
            }
            Self::ValorEscritoDesconhecido(_) => write!(
                f,
                "Não foi possível saber a figura escrita desta nota. A duração não foi alterada."
            ),
            Self::PosicaoInvalida { compasso, motivo } => write!(
                f,
                "Posição inválida no compasso {}: {motivo}. Nada foi alterado.",
                rotulo_do_compasso(compasso),
            ),
            Self::SemEspacoParaInserir {
                compasso,
                voz,
                posicao,
                motivo,
            } => write!(
                f,
                "Não há espaço para inserir no compasso {} (voz {voz}, tempo {posicao}): {motivo}. \
                 Nada foi alterado.",
                rotulo_do_compasso(compasso),
            ),
            Self::PausaSobreNota {
                compasso, posicao, ..
            } => write!(
                f,
                "No compasso {}, o tempo {posicao} é de uma nota: uma pausa não pode ser inserida \
                 em cima dela. Remova a nota antes (ela vira pausa). Nada foi alterado.",
                rotulo_do_compasso(compasso),
            ),
            Self::PausaNaoRemovivel(_) => write!(
                f,
                "Esta já é uma pausa: não há nota para remover. Nada foi alterado."
            ),
            Self::AlturaJaNoAcorde { altura, .. } => write!(
                f,
                "«{}» já está no acorde: uma altura repetida não é acrescentada. Nada foi \
                 alterado.",
                nome_da_altura(*altura),
            ),
            Self::AcordeSoEmNota(_) => write!(
                f,
                "Só uma nota que ocupa tempo recebe outra altura: pausa e nota de adorno não \
                 formam acorde. Nada foi alterado."
            ),
            Self::IntervaloInvalido(n) => write!(
                f,
                "O intervalo {n} não existe aqui: use de 2 (segunda) a 9 (nona). Nada foi alterado."
            ),
        }
    }
}

impl std::error::Error for ErroDeEdicao {}

impl From<ErroDocumento> for ErroDeEdicao {
    fn from(e: ErroDocumento) -> Self {
        match e {
            ErroDocumento::NotaInexistente(id) => Self::NotaInexistente(id),
            ErroDocumento::NotaSemAltura(id) => Self::NotaSemAltura(id),
            outro => Self::Documento(outro),
        }
    }
}

/// Consequência de uma edição que o usuário precisa saber (nunca é silenciosa).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AvisoDeEdicao {
    /// A nota estava numa ligadura de prolongamento: agora a ligadura liga alturas diferentes.
    /// O serviço não mexe na outra nota nem na ligadura.
    LigaduraDeValorComAlturasDiferentes { nota: IdNota },
    /// Esta nota, mais adiante no compasso, dependia de um acidente da nota editada e agora
    /// seria lida com outra alteração: falta um sinal nela.
    AcidenteDeNotaSeguinteMudou { nota: IdNota },
    /// O histórico passou do limite de memória: as `quantas` edições mais antigas saíram da pilha
    /// de desfazer (não dá mais para voltar até elas; nada foi corrompido).
    EdicaoMaisAntigaDescartada { quantas: usize },
    /// Encurtar a nota deixou um espaço, preenchido com uma pausa da duração exata (nunca por
    /// arredondamento): o aviso diz qual pausa nasceu.
    PausaCriada {
        pausa: IdNota,
        figura: Figura,
        pontos: u8,
    },
    /// Alongar a nota consumiu (`duracao`, em semínimas) as pausas que vinham depois dela.
    PausasConsumidas { duracao: Fracao },
    /// A nota está numa ligadura de prolongamento: o valor total da ligadura mudou.
    LigaduraDeValorMudouDuracao { nota: IdNota },
    /// A nota faz parte de uma tupla: o grupo pode não somar mais a tupla inteira.
    TuplaPodeTerFicadoIncompleta { nota: IdNota },
    /// As vigas do compasso ficariam inconsistentes e foram refeitas.
    VigasRefeitas { compasso: PosCompasso },
    /// A nota removida tinha letra: a sílaba (estrofe e texto) saiu junto com ela (ticket 17).
    LetraRemovida { nota: IdNota, texto: String },
    /// A nota removida tinha dedilhado.
    DedilhadoRemovido { nota: IdNota, texto: String },
    /// A nota removida tinha ornamento, articulação, fermata ou outra marca (`nome` no MusicXML).
    OrnamentoRemovido { nota: IdNota, nome: String },
    /// A nota removida estava numa ligadura de prolongamento: a outra nota ligada ficou com a
    /// ligadura solta.
    LigaduraDeValorRemovida { nota: IdNota },
    /// A nota removida abria ou fechava uma ligadura de expressão.
    LigaduraDeExpressaoRemovida { nota: IdNota },
    /// A nota do acorde estava ligada à seguinte (ligadura de prolongamento): a altura nova entra
    /// **sem** ligadura (a ligadura é por altura; ticket 18).
    AlturaNovaSemLigadura { nota: IdNota },
}

impl AvisoDeEdicao {
    /// Texto em português para a tela.
    pub fn texto(&self) -> String {
        match self {
            Self::LigaduraDeValorComAlturasDiferentes { .. } => {
                "A nota está numa ligadura de prolongamento: agora ela liga alturas diferentes. \
                 Ajuste a outra nota ou remova a ligadura."
                    .to_owned()
            }
            Self::AcidenteDeNotaSeguinteMudou { .. } => {
                "Uma nota mais adiante neste compasso dependia do acidente desta e ficou sem o \
                 sinal devido. Confira o compasso."
                    .to_owned()
            }
            Self::EdicaoMaisAntigaDescartada { quantas } => format!(
                "O histórico de desfazer está cheio: {quantas} edição(ões) mais antiga(s) não \
                 poderá(ão) mais ser desfeita(s)."
            ),
            Self::PausaCriada { figura, pontos, .. } => format!(
                "Foi criada uma pausa de {} depois da nota, para manter o compasso fechado.",
                figura_com_pontos(*figura, *pontos)
            ),
            Self::PausasConsumidas { duracao } => format!(
                "A nota ocupou {duracao} tempo(s) de semínima de pausa que vinha depois dela."
            ),
            Self::LigaduraDeValorMudouDuracao { .. } => {
                "A nota está numa ligadura de prolongamento: o valor total da ligadura mudou. \
                 Confira a outra nota ligada."
                    .to_owned()
            }
            Self::TuplaPodeTerFicadoIncompleta { .. } => {
                "A nota faz parte de uma tupla: confira se o grupo continua completo.".to_owned()
            }
            Self::VigasRefeitas { compasso } => format!(
                "As vigas do compasso {} ficariam inconsistentes e foram refeitas.",
                rotulo_do_compasso(compasso)
            ),
            Self::LetraRemovida { texto, .. } => format!(
                "A nota removida tinha letra («{texto}»): a sílaba saiu junto com ela. Desfazer a \
                 devolve."
            ),
            Self::DedilhadoRemovido { texto, .. } => format!(
                "A nota removida tinha dedilhado ({texto}): o dedilhado saiu junto com ela."
            ),
            Self::OrnamentoRemovido { nome, .. } => {
                format!("A nota removida tinha uma marca ({nome}): ela saiu junto com a nota.")
            }
            Self::LigaduraDeValorRemovida { .. } => {
                "A nota removida estava numa ligadura de prolongamento: a outra nota ligada ficou \
                 com a ligadura solta. Confira."
                    .to_owned()
            }
            Self::LigaduraDeExpressaoRemovida { .. } => {
                "A nota removida abria ou fechava uma ligadura de expressão: confira a outra ponta."
                    .to_owned()
            }
            Self::AlturaNovaSemLigadura { .. } => {
                "O acorde estava ligado ao seguinte: a altura nova entra sem ligadura (ela só \
                 dura este acorde). Ligue-a à seguinte se for o caso."
                    .to_owned()
            }
        }
    }
}

/// O que um comando fez.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultadoDaEdicao {
    /// A forma resolvida do que foi aplicado (`DefinirAltura` ou `DefinirDuracao`).
    pub aplicado: ComandoDeEdicao,
    /// O comando que desfaz este (`DefinirAltura` ou `DefinirDuracao`).
    pub inverso: ComandoDeEdicao,
    /// A nota editada (a mesma identidade de antes).
    pub nota: IdNota,
    pub antes: Altura,
    pub depois: Altura,
    /// Algo mudou? Pôr a nota onde ela já está não suja nada.
    pub alterou: bool,
    /// Compassos que este comando sujou (zero ou um).
    pub compassos_afetados: Vec<PosCompasso>,
    pub avisos: Vec<AvisoDeEdicao>,
    /// O que mudou na duração (`Some` só nos comandos de duração, ticket 16). Nesses comandos
    /// `antes` e `depois` são a altura da nota, que não muda.
    pub duracao: Option<MudancaDeDuracao>,
    /// O que mudou na estrutura (`Some` só nos comandos de inserir e remover, ticket 17). Neles
    /// `antes` e `depois` são a altura da nota (Dó4 neutro quando o alvo é uma pausa).
    pub estrutura: Option<MudancaDeEstrutura>,
}

/// O que um comando de inserir ou remover mudou (ticket 17).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MudancaDeEstrutura {
    /// A nota ou pausa inserida; na remoção, a nota que virou pausa (a mesma identidade) ou que
    /// saiu (nota de acorde ou de adorno).
    pub alvo: IdNota,
    /// O alvo soa (é uma nota inserida)? Pausa e nota removida não soam.
    pub alvo_soa: bool,
    /// Pausas que nasceram para preencher o espaço que sobrou (na ordem do compasso).
    pub criadas: Vec<IdNota>,
    /// Identidades que deixaram de existir: as pausas que a nota ocupou, ou a própria nota que
    /// saiu sem virar pausa.
    pub removidas: Vec<IdNota>,
}

impl ResultadoDaEdicao {
    /// Número MIDI da altura nova, para ouvir a nota editada (`None` se o resultado não soa).
    pub fn midi_depois(&self) -> Option<u8> {
        if self.estrutura.as_ref().is_some_and(|e| !e.alvo_soa) {
            return None;
        }
        self.depois.numero_midi().and_then(|m| u8::try_from(m).ok())
    }
}

/// Altura de enfeite dos resultados cujo alvo é uma pausa (nunca soa: ver `midi_depois`).
pub(super) const ALTURA_NEUTRA: Altura = Altura {
    passo: 'C',
    alter: 0,
    oitava: 4,
};

/// Onde a nota está nos dois modelos.
#[derive(Clone, Copy)]
struct Local {
    /// Índice do compasso (igual nos dois modelos).
    compasso: usize,
    /// Ordinal da nota no compasso (igual nos dois modelos).
    ordinal: usize,
    /// Índice da parte no modelo de tocar (partes sem compasso não entram).
    parte: usize,
    quintas: i8,
}

impl DocumentoAberto {
    /// Limites de altura dos comandos de edição.
    pub fn limites(&self) -> LimitesDeAltura {
        self.limites
    }

    pub fn definir_limites(&mut self, limites: LimitesDeAltura) {
        self.limites = limites;
    }

    /// Aplica o comando sem registrar no histórico (quem registra é `aplicar`, em `historico`).
    /// Em erro, nada muda; em sucesso, o documento (compasso sujo), o modelo de tocar e o texto
    /// exportável já refletem a edição. Ver o contrato do módulo.
    pub(super) fn aplicar_sem_historico(
        &mut self,
        comando: ComandoDeEdicao,
    ) -> Result<ResultadoDaEdicao, ErroDeEdicao> {
        if comando.e_de_duracao() {
            return self.aplicar_duracao(&comando);
        }
        if comando.e_de_estrutura() {
            return self.aplicar_estrutura(&comando);
        }
        if comando.e_de_acorde() {
            return self.aplicar_acorde(&comando);
        }
        let id = comando.nota().ok_or_else(|| {
            ErroDeEdicao::Documento(ErroDocumento::NaoSuportado(
                "comando sem nota fora do caminho de estrutura".into(),
            ))
        })?;
        let local = self.local_da_nota(id)?;
        let antes_altura = self
            .documento
            .nota(id)
            .and_then(|n| n.altura)
            .ok_or(ErroDeEdicao::NotaSemAltura(id))?;
        let acidente_antes = self.documento.acidente_da_nota(id)?;
        let (altura, acidente) = match comando {
            ComandoDeEdicao::MudarAltura {
                unidade, sentido, ..
            } => {
                let acima = sentido == SentidoDaAltura::Acima;
                let q = local.quintas;
                let nova = match unidade {
                    UnidadeDeAltura::Semitom => por_semitom(antes_altura, acima, q),
                    UnidadeDeAltura::GrauDaEscala => por_grau(antes_altura, acima, q),
                    UnidadeDeAltura::Oitava => por_oitava(antes_altura, acima),
                }
                .ok_or_else(|| {
                    ErroDeEdicao::Documento(ErroDocumento::AlturaInvalida(format!(
                        "passo '{}'",
                        antes_altura.passo
                    )))
                })?;
                self.conferir_limites(nova)?;
                let acidente = self.acidente_a_imprimir(local, nova, acidente_antes.as_deref());
                (nova, acidente)
            }
            ComandoDeEdicao::DefinirAltura {
                altura, acidente, ..
            } => {
                self.conferir_limites(altura)?;
                (altura, acidente)
            }
            // Já despachado no começo da função.
            ComandoDeEdicao::MudarDuracao { .. }
            | ComandoDeEdicao::AlternarPonto { .. }
            | ComandoDeEdicao::DefinirDuracao { .. }
            | ComandoDeEdicao::InserirNota { .. }
            | ComandoDeEdicao::InserirPausa { .. }
            | ComandoDeEdicao::RemoverNota { .. }
            | ComandoDeEdicao::AcrescentarLetra { .. }
            | ComandoDeEdicao::AcrescentarIntervalo { .. }
            | ComandoDeEdicao::AdicionarAltura { .. }
            | ComandoDeEdicao::RestaurarAltura { .. }
            | ComandoDeEdicao::RetirarAltura { .. } => {
                return Err(ErroDeEdicao::Documento(ErroDocumento::NaoSuportado(
                    "comando de duração, de estrutura ou de acorde fora do caminho dele".into(),
                )));
            }
        };
        let sem_sinal_antes = self.sem_sinal_devido(local);
        let alterou = self
            .documento
            .definir_altura_e_acidente(id, altura, acidente.as_deref())?;
        // A grafia que ficou (um nome só mantém os atributos que o elemento já tinha).
        let acidente = self.documento.acidente_da_nota(id)?;
        let mut resultado = ResultadoDaEdicao {
            aplicado: ComandoDeEdicao::DefinirAltura {
                nota: id,
                altura,
                acidente: acidente.clone(),
            },
            inverso: ComandoDeEdicao::DefinirAltura {
                nota: id,
                altura: antes_altura,
                acidente: acidente_antes,
            },
            nota: id,
            antes: antes_altura,
            depois: altura,
            alterou,
            compassos_afetados: Vec::new(),
            avisos: Vec::new(),
            duracao: None,
            estrutura: None,
        };
        if !alterou {
            return Ok(resultado);
        }
        self.atualizar_modelo_de_tocar(local, altura, acidente.as_deref());
        resultado
            .compassos_afetados
            .extend(self.documento.posicao_da_nota(id));
        if midi(altura) != midi(antes_altura) && self.documento.tem_ligadura_de_valor(id)? {
            resultado
                .avisos
                .push(AvisoDeEdicao::LigaduraDeValorComAlturasDiferentes { nota: id });
        }
        for ordinal in self.sem_sinal_devido(local) {
            if ordinal == local.ordinal || sem_sinal_antes.contains(&ordinal) {
                continue;
            }
            if let Some(nota) = self.ids.id(local.parte, local.compasso, ordinal) {
                resultado
                    .avisos
                    .push(AvisoDeEdicao::AcidenteDeNotaSeguinteMudou { nota });
            }
        }
        Ok(resultado)
    }

    fn local_da_nota(&self, id: IdNota) -> Result<Local, ErroDeEdicao> {
        let &(pd, compasso, ordinal) = self
            .documento
            .indice
            .get(&id)
            .ok_or(ErroDeEdicao::NotaInexistente(id))?;
        let parte = self.documento.partes[..pd]
            .iter()
            .filter(|p| !p.compassos.is_empty())
            .count();
        let existe = self
            .partitura
            .partes
            .get(parte)
            .and_then(|p| p.compassos.get(compasso))
            .is_some_and(|c| ordinal < c.notas.len());
        if !existe {
            return Err(ErroDeEdicao::Documento(ErroDocumento::FaixaInconsistente(
                "a nota não existe no modelo de tocar".into(),
            )));
        }
        let quintas = self
            .partitura
            .compassos
            .get(compasso)
            .map_or(0, |i| i.armadura.quintas);
        Ok(Local {
            compasso,
            ordinal,
            parte,
            quintas,
        })
    }

    pub(super) fn conferir_limites(&self, altura: Altura) -> Result<(), ErroDeEdicao> {
        let (grave, agudo) = self.limites.efetivos();
        let m = midi(altura).ok_or_else(|| {
            ErroDeEdicao::Documento(ErroDocumento::AlturaInvalida(format!(
                "passo '{}'",
                altura.passo
            )))
        })?;
        if m < grave || m > agudo {
            return Err(ErroDeEdicao::ForaDosLimites {
                pedida: altura,
                midi: m,
                grave,
                agudo,
            });
        }
        Ok(())
    }

    /// Notas da mesma pauta que começam antes de `ate` no compasso (menos a de ordinal
    /// `excluir`), com o que vale para o acidente em vigor.
    fn anteriores(&self, l: Local, excluir: usize, pauta: u8, ate: Fracao) -> Vec<Anterior> {
        anteriores_em(
            &self.partitura.partes[l.parte].compassos[l.compasso],
            excluir,
            pauta,
            ate,
        )
    }

    /// O `<accidental>` que a nota deve ter na altura nova: o que já tem, se significa a mesma
    /// alteração (não se mexe no que o arquivo escolheu); senão o devido (ou nenhum).
    fn acidente_a_imprimir(&self, l: Local, nova: Altura, atual: Option<&str>) -> Option<String> {
        let nota = &self.partitura.partes[l.parte].compassos[l.compasso].notas[l.ordinal];
        let anteriores = self.anteriores(l, l.ordinal, nota.pauta, nota.posicao);
        let em_vigor = alteracao_em_vigor(l.quintas, nova.passo, nova.oitava, &anteriores);
        match atual {
            Some(t) if alter_do_acidente(t) == Some(nova.alter) => Some(t.to_owned()),
            _ if nova.alter != em_vigor => Some(acidente_de(nova.alter).to_owned()),
            _ => None,
        }
    }

    /// Ordinais das notas do compasso que não têm acidente impresso e cuja alteração difere da
    /// que está em vigor naquele ponto (leitura errada na pauta).
    fn sem_sinal_devido(&self, l: Local) -> Vec<usize> {
        sem_sinal_devido_em(
            &self.partitura.partes[l.parte].compassos[l.compasso],
            l.quintas,
        )
    }

    /// Reflete a edição no modelo de tocar, só na nota editada.
    fn atualizar_modelo_de_tocar(&mut self, l: Local, altura: Altura, acidente: Option<&str>) {
        let Some(pa) = para_partitura(altura) else {
            return;
        };
        let nota = &mut self.partitura.partes[l.parte].compassos[l.compasso].notas[l.ordinal];
        nota.altura = Some(pa);
        nota.midi = pa.numero_midi();
        nota.acidente = acidente.and_then(|t| Acidente::de_texto(nome_do_acidente(t)));
    }
}

/// Notas da mesma pauta que começam antes de `ate` no compasso (menos a de ordinal `excluir`),
/// com o que vale para o acidente em vigor.
pub(super) fn anteriores_em(
    comp: &partitura::Compasso,
    excluir: usize,
    pauta: u8,
    ate: Fracao,
) -> Vec<Anterior> {
    comp.notas
        .iter()
        .enumerate()
        .filter(|(i, n)| *i != excluir && n.pauta == pauta && antes(n.posicao, ate))
        .filter_map(|(_, n)| {
            let a = n.altura?;
            Some(Anterior {
                letra: de_partitura(a).passo,
                oitava: a.oitava,
                alter: a.alteracao,
                posicao: n.posicao,
            })
        })
        .collect()
}

/// Ordinais das notas do compasso sem acidente impresso cuja alteração difere da que está em
/// vigor naquele ponto.
pub(super) fn sem_sinal_devido_em(comp: &partitura::Compasso, quintas: i8) -> Vec<usize> {
    comp.notas
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            let a = n.altura?;
            if n.acidente.is_some() {
                return None;
            }
            let anteriores = anteriores_em(comp, i, n.pauta, n.posicao);
            let vigor = alteracao_em_vigor(quintas, de_partitura(a).passo, a.oitava, &anteriores);
            (a.alteracao != vigor).then_some(i)
        })
        .collect()
}

/// O `<accidental>` que uma nota **nova** em `altura`, na pauta e posição dadas, precisa para ser
/// lida certo: o sinal devido (ou `None` se a armadura e as notas antes dela já o explicam).
pub(super) fn acidente_devido(
    comp: &partitura::Compasso,
    quintas: i8,
    altura: Altura,
    pauta: u8,
    posicao: Fracao,
) -> Option<String> {
    let anteriores = anteriores_em(comp, usize::MAX, pauta, posicao);
    let em_vigor = alteracao_em_vigor(quintas, altura.passo, altura.oitava, &anteriores);
    (altura.alter != em_vigor).then(|| acidente_de(altura.alter).to_owned())
}
