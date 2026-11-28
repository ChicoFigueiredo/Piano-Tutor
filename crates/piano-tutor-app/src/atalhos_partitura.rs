//! Mapa de atalhos da Oficina de partitura (ticket 13, E01, seam S1): dados e regras **sem
//! janela**. O mapa liga cada [`AcaoAtalho`] a uma combinação de teclas ([`Atalho`] da casca);
//! a tela só traduz o evento de tecla em [`Destino`] e executa.
//!
//! # Contrato em linguagem de domínio
//!
//! - **Perfil padrão** no estilo do MuseScore 4 (decisões do titular): seta = semitom;
//!   `Alt+Shift+seta` = grau da escala; `Ctrl+seta` = oitava; durações `1` a `9`; `.` = ponto;
//!   `Shift+letra` = acrescentar nota ao acorde; `Ctrl+Z` desfaz e `Ctrl+Shift+Z` refaz;
//!   `Delete` apaga; cifra = `Ctrl+Shift+K` (o `Ctrl+K` é a paleta da casca).
//! - **Reservados** ([`reservado`]): `Ctrl+K` (paleta), `Ctrl+.` (Pânico) e `Esc` (prioridade da
//!   casca). Nunca são atribuídos e nunca são capturados pela Oficina: [`MapaAtalhos::rotear`]
//!   devolve `None` para eles, de modo que a casca os trata exatamente como antes.
//! - **Casca**: os outros atalhos da casca ([`ATALHOS_DA_CASCA`]) também são recusados.
//! - **Tela Teclado**: as teclas que tocam notas no teclado do PC
//!   ([`conflito_com_tela_teclado`]) não são recusadas (é outra tela), mas o conflito é
//!   **sinalizado** ao remapear e na lista.
//! - **Uso duplicado**: atribuir uma combinação que outra ação já usa é recusado, citando a
//!   ação. Para trocar duas, libere uma antes ([`MapaAtalhos::remover`]).
//! - **Altura** (ticket 14): as seis ações de altura ([`AcaoAtalho::altura`]) viram
//!   [`Destino::Altura`] (unidade e sentido); a tela monta o comando tipado do serviço de
//!   documento e o chama. A tecla nunca edita por conta própria.
//! - **Duração** (ticket 16): as teclas `1` a `9` ([`AcaoAtalho::Duracao`]) viram
//!   [`Destino::Duracao`] e o ponto ([`AcaoAtalho::Ponto`]) vira [`Destino::Ponto`]; a tela monta
//!   o comando tipado de duração do serviço de documento. A `9` (longa) chega ao serviço, que a
//!   recusa com o motivo (o modelo de partitura não tem a longa). Remapear continua valendo.
//! - **Apagar** (ticket 17): `Delete` ([`AcaoAtalho::Apagar`]) vira [`Destino::Apagar`]; a tela
//!   monta o comando tipado de remover do serviço de documento (a nota vira pausa de mesma
//!   duração).
//!   o comando existe no serviço (posição pela coluna do layout), sem atalho novo no mapa.
//! - **Acorde** (ticket 18): `Shift+letra` ([`AcaoAtalho::AcrescentarNota`]) acrescenta ao
//!   acorde a letra na oitava mais próxima da nota selecionada; `Alt+2` a `Alt+9`
//!   ([`AcaoAtalho::IntervaloAcima`], como no MuseScore) acrescentam o intervalo de segunda a
//!   nona acima dela; `Ctrl+Delete` ([`AcaoAtalho::RetirarAltura`]) retira a altura selecionada
//!   do acorde (a última vira pausa, como o `Delete`). Viram [`Destino::Acorde`]; a tela monta o
//!   comando tipado do serviço de documento. Tudo remapeável como o resto do mapa; nenhum
//!   atalho existente saiu.
//! - **Entrada de notas** (ticket 17b): as letras `A` a `G` ([`AcaoAtalho::InserirNota`]) viram
//!   [`Destino::InserirNota`] (a altura sai da letra, como no MuseScore, e a duração é a atual das
//!   teclas `1` a `8`) e `0` ([`AcaoAtalho::InserirPausa`]) vira [`Destino::InserirPausa`]; a tela
//!   monta o comando tipado de inserir do serviço de documento na posição do cursor de entrada
//!   (`cursor_entrada`). `Ctrl+←` e `Ctrl+→` movem o cursor pelas posições da voz (inclusive
//!   pausas) e `Alt+↑` e `Alt+↓` trocam de voz ou pauta ([`Destino::Entrada`]). São ações novas, no
//!   fim da lista: nenhum atalho existente saiu do mapa, e todas são remapeáveis como as demais.
//! - **Pendentes**: ação sem comando ainda ([`AcaoAtalho::pendente`]) aparece em
//!   [`MapaAtalhos::pendentes`]; ao apertar a tecla, a tela avisa em vez de calar.
//! - **Persistência**: texto `id=Ctrl+Shift+Z` por linha (seção `tela.atalhos` do perfil,
//!   ADR-0010); `id=-` é "sem atalho". Texto ausente, com id desconhecido ou combinação
//!   inválida, reservada ou repetida cai no padrão (ou fica sem atalho se o padrão colidir).

use crate::espelho::MAPA_PC;
use crate::opcoes::Kv;
use crate::selecao_partitura::Comando;
use bancada_casca::Atalho;
use bancada_casca::egui::{Key, Modifiers};
use piano_tutor_core::documento::{SentidoDaAltura, UnidadeDeAltura};
use std::fmt;

/// Uma ação da Oficina que pode ter atalho.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AcaoAtalho {
    NotaAnterior,
    NotaSeguinte,
    TrilhaAnterior,
    TrilhaSeguinte,
    SemitomAcima,
    SemitomAbaixo,
    GrauAcima,
    GrauAbaixo,
    OitavaAcima,
    OitavaAbaixo,
    /// Duração de `1` (semifusa) a `9` (longa), como no MuseScore.
    Duracao(u8),
    Ponto,
    Desfazer,
    Refazer,
    Apagar,
    Cifra,
    /// Acrescenta ao acorde a nota de letra `A` a `G`.
    AcrescentarNota(char),
    /// Acrescenta ao acorde a altura `2` (segunda) a `9` (nona) acima da nota selecionada
    /// (ticket 18).
    IntervaloAcima(u8),
    /// Retira do acorde a altura da nota selecionada (ticket 18).
    RetirarAltura,
    /// Insere, na posição do cursor de entrada, a nota de letra `A` a `G` (ticket 17b).
    InserirNota(char),
    /// Insere, na posição do cursor de entrada, uma pausa com a duração atual (ticket 17b).
    InserirPausa,
    /// Cursor de entrada para a posição anterior da voz, pausas incluídas (ticket 17b).
    EntradaAnterior,
    /// Cursor de entrada para a posição seguinte da voz, pausas incluídas (ticket 17b).
    EntradaSeguinte,
    /// Cursor de entrada para a voz ou pauta de cima (ticket 17b).
    EntradaAcima,
    /// Cursor de entrada para a voz ou pauta de baixo (ticket 17b).
    EntradaAbaixo,
}

/// Movimento do cursor de entrada (ticket 17b).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MovimentoDeEntrada {
    Anterior,
    Seguinte,
    Acima,
    Abaixo,
}

/// Todas as ações, na ordem da lista de atalhos.
pub const ACOES: [AcaoAtalho; 52] = [
    AcaoAtalho::NotaAnterior,
    AcaoAtalho::NotaSeguinte,
    AcaoAtalho::TrilhaAnterior,
    AcaoAtalho::TrilhaSeguinte,
    AcaoAtalho::SemitomAcima,
    AcaoAtalho::SemitomAbaixo,
    AcaoAtalho::GrauAcima,
    AcaoAtalho::GrauAbaixo,
    AcaoAtalho::OitavaAcima,
    AcaoAtalho::OitavaAbaixo,
    AcaoAtalho::Duracao(1),
    AcaoAtalho::Duracao(2),
    AcaoAtalho::Duracao(3),
    AcaoAtalho::Duracao(4),
    AcaoAtalho::Duracao(5),
    AcaoAtalho::Duracao(6),
    AcaoAtalho::Duracao(7),
    AcaoAtalho::Duracao(8),
    AcaoAtalho::Duracao(9),
    AcaoAtalho::Ponto,
    AcaoAtalho::Desfazer,
    AcaoAtalho::Refazer,
    AcaoAtalho::Apagar,
    AcaoAtalho::Cifra,
    AcaoAtalho::AcrescentarNota('A'),
    AcaoAtalho::AcrescentarNota('B'),
    AcaoAtalho::AcrescentarNota('C'),
    AcaoAtalho::AcrescentarNota('D'),
    AcaoAtalho::AcrescentarNota('E'),
    AcaoAtalho::AcrescentarNota('F'),
    AcaoAtalho::AcrescentarNota('G'),
    AcaoAtalho::IntervaloAcima(2),
    AcaoAtalho::IntervaloAcima(3),
    AcaoAtalho::IntervaloAcima(4),
    AcaoAtalho::IntervaloAcima(5),
    AcaoAtalho::IntervaloAcima(6),
    AcaoAtalho::IntervaloAcima(7),
    AcaoAtalho::IntervaloAcima(8),
    AcaoAtalho::IntervaloAcima(9),
    AcaoAtalho::RetirarAltura,
    AcaoAtalho::InserirNota('A'),
    AcaoAtalho::InserirNota('B'),
    AcaoAtalho::InserirNota('C'),
    AcaoAtalho::InserirNota('D'),
    AcaoAtalho::InserirNota('E'),
    AcaoAtalho::InserirNota('F'),
    AcaoAtalho::InserirNota('G'),
    AcaoAtalho::InserirPausa,
    AcaoAtalho::EntradaAnterior,
    AcaoAtalho::EntradaSeguinte,
    AcaoAtalho::EntradaAcima,
    AcaoAtalho::EntradaAbaixo,
];

fn nome_da_duracao(n: u8) -> &'static str {
    match n {
        1 => "semifusa (1/64)",
        2 => "fusa (1/32)",
        3 => "semicolcheia (1/16)",
        4 => "colcheia (1/8)",
        5 => "semínima (1/4)",
        6 => "mínima (1/2)",
        7 => "semibreve (1)",
        8 => "breve (2)",
        _ => "longa (4)",
    }
}

fn nome_do_intervalo(n: u8) -> &'static str {
    match n {
        2 => "segunda",
        3 => "terça",
        4 => "quarta",
        5 => "quinta",
        6 => "sexta",
        7 => "sétima",
        8 => "oitava",
        _ => "nona",
    }
}

fn nome_da_letra(l: char) -> &'static str {
    match l {
        'A' => "Lá",
        'B' => "Si",
        'C' => "Dó",
        'D' => "Ré",
        'E' => "Mi",
        'F' => "Fá",
        _ => "Sol",
    }
}

fn tecla_numerica(n: u8) -> Key {
    match n {
        1 => Key::Num1,
        2 => Key::Num2,
        3 => Key::Num3,
        4 => Key::Num4,
        5 => Key::Num5,
        6 => Key::Num6,
        7 => Key::Num7,
        8 => Key::Num8,
        _ => Key::Num9,
    }
}

fn tecla_da_letra(l: char) -> Key {
    match l {
        'A' => Key::A,
        'B' => Key::B,
        'C' => Key::C,
        'D' => Key::D,
        'E' => Key::E,
        'F' => Key::F,
        _ => Key::G,
    }
}

impl AcaoAtalho {
    /// Identificador estável, gravado no perfil.
    pub fn id(self) -> String {
        match self {
            Self::NotaAnterior => "nota_anterior".into(),
            Self::NotaSeguinte => "nota_seguinte".into(),
            Self::TrilhaAnterior => "trilha_anterior".into(),
            Self::TrilhaSeguinte => "trilha_seguinte".into(),
            Self::SemitomAcima => "semitom_acima".into(),
            Self::SemitomAbaixo => "semitom_abaixo".into(),
            Self::GrauAcima => "grau_acima".into(),
            Self::GrauAbaixo => "grau_abaixo".into(),
            Self::OitavaAcima => "oitava_acima".into(),
            Self::OitavaAbaixo => "oitava_abaixo".into(),
            Self::Duracao(n) => format!("duracao_{n}"),
            Self::Ponto => "ponto".into(),
            Self::Desfazer => "desfazer".into(),
            Self::Refazer => "refazer".into(),
            Self::Apagar => "apagar".into(),
            Self::Cifra => "cifra".into(),
            Self::AcrescentarNota(l) => format!("acorde_{}", l.to_ascii_lowercase()),
            Self::IntervaloAcima(n) => format!("intervalo_acima_{n}"),
            Self::RetirarAltura => "retirar_altura".into(),
            Self::InserirNota(l) => format!("inserir_{}", l.to_ascii_lowercase()),
            Self::InserirPausa => "inserir_pausa".into(),
            Self::EntradaAnterior => "entrada_anterior".into(),
            Self::EntradaSeguinte => "entrada_seguinte".into(),
            Self::EntradaAcima => "entrada_acima".into(),
            Self::EntradaAbaixo => "entrada_abaixo".into(),
        }
    }

    /// Rótulo em PT-BR mostrado na lista de atalhos.
    pub fn rotulo(self) -> String {
        match self {
            Self::NotaAnterior => "Nota anterior".into(),
            Self::NotaSeguinte => "Próxima nota".into(),
            Self::TrilhaAnterior => "Voz ou pauta anterior".into(),
            Self::TrilhaSeguinte => "Próxima voz ou pauta".into(),
            Self::SemitomAcima => "Subir um semitom".into(),
            Self::SemitomAbaixo => "Descer um semitom".into(),
            Self::GrauAcima => "Subir um grau da escala".into(),
            Self::GrauAbaixo => "Descer um grau da escala".into(),
            Self::OitavaAcima => "Subir uma oitava".into(),
            Self::OitavaAbaixo => "Descer uma oitava".into(),
            Self::Duracao(n) => format!("Duração: {}", nome_da_duracao(n)),
            Self::Ponto => "Ponto de aumento (liga e desliga)".into(),
            Self::Desfazer => "Desfazer".into(),
            Self::Refazer => "Refazer".into(),
            Self::Apagar => "Apagar a nota selecionada".into(),
            Self::Cifra => "Cifra (acorde cifrado)".into(),
            Self::AcrescentarNota(l) => {
                format!("Acrescentar ao acorde a nota {} ({l})", nome_da_letra(l))
            }
            Self::IntervaloAcima(n) => {
                format!("Acrescentar ao acorde a {} acima", nome_do_intervalo(n))
            }
            Self::RetirarAltura => "Retirar a altura selecionada do acorde".into(),
            Self::InserirNota(l) => {
                format!(
                    "Inserir a nota {} ({l}) na posição de entrada",
                    nome_da_letra(l)
                )
            }
            Self::InserirPausa => "Inserir uma pausa na posição de entrada".into(),
            Self::EntradaAnterior => "Posição de entrada anterior (pausas incluídas)".into(),
            Self::EntradaSeguinte => "Próxima posição de entrada (pausas incluídas)".into(),
            Self::EntradaAcima => "Posição de entrada na voz ou pauta de cima".into(),
            Self::EntradaAbaixo => "Posição de entrada na voz ou pauta de baixo".into(),
        }
    }

    /// Combinação do perfil padrão (MuseScore 4, com as decisões do titular).
    pub fn padrao(self) -> Atalho {
        match self {
            Self::NotaAnterior => Atalho::tecla(Key::ArrowLeft),
            Self::NotaSeguinte => Atalho::tecla(Key::ArrowRight),
            Self::TrilhaAnterior => Atalho::tecla(Key::Tab).com_shift(),
            Self::TrilhaSeguinte => Atalho::tecla(Key::Tab),
            Self::SemitomAcima => Atalho::tecla(Key::ArrowUp),
            Self::SemitomAbaixo => Atalho::tecla(Key::ArrowDown),
            Self::GrauAcima => Atalho::alt(Key::ArrowUp).com_shift(),
            Self::GrauAbaixo => Atalho::alt(Key::ArrowDown).com_shift(),
            Self::OitavaAcima => Atalho::ctrl(Key::ArrowUp),
            Self::OitavaAbaixo => Atalho::ctrl(Key::ArrowDown),
            Self::Duracao(n) => Atalho::tecla(tecla_numerica(n)),
            Self::Ponto => Atalho::tecla(Key::Period),
            Self::Desfazer => Atalho::ctrl(Key::Z),
            Self::Refazer => Atalho::ctrl(Key::Z).com_shift(),
            Self::Apagar => Atalho::tecla(Key::Delete),
            Self::Cifra => Atalho::ctrl(Key::K).com_shift(),
            Self::AcrescentarNota(l) => Atalho::tecla(tecla_da_letra(l)).com_shift(),
            Self::IntervaloAcima(n) => Atalho::alt(tecla_numerica(n)),
            Self::RetirarAltura => Atalho::ctrl(Key::Delete),
            Self::InserirNota(l) => Atalho::tecla(tecla_da_letra(l)),
            Self::InserirPausa => Atalho::tecla(Key::Num0),
            Self::EntradaAnterior => Atalho::ctrl(Key::ArrowLeft),
            Self::EntradaSeguinte => Atalho::ctrl(Key::ArrowRight),
            Self::EntradaAcima => Atalho::alt(Key::ArrowUp),
            Self::EntradaAbaixo => Atalho::alt(Key::ArrowDown),
        }
    }

    /// Comando de **edição de altura** ligado à ação (ticket 14): a tecla só traduz a ação em
    /// unidade e sentido; quem edita é o comando tipado do serviço de documento.
    pub fn altura(self) -> Option<(UnidadeDeAltura, SentidoDaAltura)> {
        use SentidoDaAltura::{Abaixo, Acima};
        use UnidadeDeAltura::{GrauDaEscala, Oitava, Semitom};
        match self {
            Self::SemitomAcima => Some((Semitom, Acima)),
            Self::SemitomAbaixo => Some((Semitom, Abaixo)),
            Self::GrauAcima => Some((GrauDaEscala, Acima)),
            Self::GrauAbaixo => Some((GrauDaEscala, Abaixo)),
            Self::OitavaAcima => Some((Oitava, Acima)),
            Self::OitavaAbaixo => Some((Oitava, Abaixo)),
            _ => None,
        }
    }

    /// Ação do histórico (ticket 15): desfazer e refazer chamam o serviço de documento.
    pub fn historico(self) -> Option<Destino> {
        match self {
            Self::Desfazer => Some(Destino::Desfazer),
            Self::Refazer => Some(Destino::Refazer),
            _ => None,
        }
    }

    /// Ação de **duração** (ticket 16): as teclas `1` a `9` escolhem a figura e o ponto alterna.
    pub fn duracao(self) -> Option<Destino> {
        match self {
            Self::Duracao(n) => Some(Destino::Duracao(n)),
            Self::Ponto => Some(Destino::Ponto),
            _ => None,
        }
    }

    /// Ação de **apagar** (ticket 17): remove a nota selecionada (ela vira pausa).
    pub fn apagar(self) -> Option<Destino> {
        match self {
            Self::Apagar => Some(Destino::Apagar),
            _ => None,
        }
    }

    /// Ação de **acorde** (ticket 18): acrescentar uma altura ou retirá-la.
    pub fn acorde(self) -> Option<Destino> {
        match self {
            Self::AcrescentarNota(l) => Some(Destino::Acorde(PedidoDeAcorde::Letra(l))),
            Self::IntervaloAcima(n) => Some(Destino::Acorde(PedidoDeAcorde::Intervalo(n))),
            Self::RetirarAltura => Some(Destino::Acorde(PedidoDeAcorde::Retirar)),
            _ => None,
        }
    }

    /// Ação de **entrada de notas** (ticket 17b): inserir nota ou pausa e mover o cursor de
    /// entrada.
    pub fn entrada(self) -> Option<Destino> {
        match self {
            Self::InserirNota(l) => Some(Destino::InserirNota(l)),
            Self::InserirPausa => Some(Destino::InserirPausa),
            Self::EntradaAnterior => Some(Destino::Entrada(MovimentoDeEntrada::Anterior)),
            Self::EntradaSeguinte => Some(Destino::Entrada(MovimentoDeEntrada::Seguinte)),
            Self::EntradaAcima => Some(Destino::Entrada(MovimentoDeEntrada::Acima)),
            Self::EntradaAbaixo => Some(Destino::Entrada(MovimentoDeEntrada::Abaixo)),
            _ => None,
        }
    }

    /// Ação com atalho mas ainda sem comando nenhum (nem de navegação, nem de edição).
    pub fn pendente(self) -> bool {
        self.comando().is_none()
            && self.altura().is_none()
            && self.historico().is_none()
            && self.duracao().is_none()
            && self.apagar().is_none()
            && self.acorde().is_none()
            && self.entrada().is_none()
    }

    /// Comando de navegação ligado à ação; `None` = não é navegação (edição ou ainda pendente).
    /// Os demais comandos de edição entram nas fatias seguintes.
    pub fn comando(self) -> Option<Comando> {
        match self {
            Self::NotaAnterior => Some(Comando::Anterior),
            Self::NotaSeguinte => Some(Comando::Seguinte),
            Self::TrilhaAnterior => Some(Comando::TrilhaAnterior),
            Self::TrilhaSeguinte => Some(Comando::TrilhaSeguinte),
            _ => None,
        }
    }

    #[cfg(test)]
    fn de_id(id: &str) -> Option<Self> {
        ACOES.iter().copied().find(|a| a.id() == id)
    }
}

/// Atalhos que a casca e o app usam e a Oficina jamais captura, com o motivo. Os três primeiros
/// são os **reservados** (A11); os demais são recusados no remapeamento por colidirem com a casca.
pub const ATALHOS_DA_CASCA: [(Atalho, &str, bool); 9] = [
    (Atalho::ctrl(Key::K), "a paleta de ações da casca", true),
    (
        Atalho::ctrl(Key::Period),
        "o Pânico (corta todo o som)",
        true,
    ),
    (
        Atalho::tecla(Key::Escape),
        "o Esc da casca (fecha paleta, bastidores e KeyTips)",
        true,
    ),
    (Atalho::ctrl(Key::T), "alternar o tema", false),
    (
        Atalho::ctrl(Key::F1),
        "recolher ou expandir o Ribbon",
        false,
    ),
    (
        Atalho::ctrl(Key::Slash),
        "o mapa de atalhos da casca",
        false,
    ),
    (Atalho::tecla(Key::F10), "o modo foco", false),
    (Atalho::tecla(Key::F11), "a tela cheia", false),
    (
        Atalho::alt(Key::ArrowLeft),
        "voltar (histórico da casca)",
        false,
    ),
];

/// O atalho é reservado (Ctrl+K, Ctrl+. ou Esc)? Devolve o motivo.
pub fn reservado(a: Atalho) -> Option<&'static str> {
    if a.tecla == Key::Escape {
        return Some(ATALHOS_DA_CASCA[2].1);
    }
    ATALHOS_DA_CASCA
        .iter()
        .find(|(c, _, res)| *res && *c == a)
        .map(|(_, m, _)| *m)
}

fn da_casca(a: Atalho) -> Option<&'static str> {
    if a == Atalho::alt(Key::ArrowRight) {
        return Some("avançar (histórico da casca)");
    }
    ATALHOS_DA_CASCA
        .iter()
        .find(|(c, _, res)| !*res && *c == a)
        .map(|(_, m, _)| *m)
}

/// Esta tecla, como o teclado do PC da tela Teclado a lê (com Shift ou sem modificador), toca
/// uma nota ou muda a oitava lá? Devolve a letra mostrada.
pub fn conflito_com_tela_teclado(a: Atalho) -> Option<&'static str> {
    if a.ctrl || a.alt {
        return None;
    }
    if matches!(a.tecla, Key::Z | Key::X) {
        return Some(if a.tecla == Key::Z { "Z" } else { "X" });
    }
    MAPA_PC
        .iter()
        .find(|(k, _, _)| *k == a.tecla)
        .map(|(_, _, letra)| *letra)
}

/// Texto canônico gravado no perfil (`Ctrl+Shift+Up`), ao contrário do `Display`, que usa setas.
pub fn texto_canonico(a: Atalho) -> String {
    let mut s = String::new();
    if a.ctrl {
        s.push_str("Ctrl+");
    }
    if a.alt {
        s.push_str("Alt+");
    }
    if a.shift {
        s.push_str("Shift+");
    }
    s.push_str(a.tecla.name());
    s
}

/// Por que um atalho foi recusado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recusa {
    /// Ctrl+K, Ctrl+. ou Esc: nunca remapeáveis.
    Reservado {
        atalho: Atalho,
        motivo: &'static str,
    },
    /// Atalho de outra parte da casca.
    DaCasca {
        atalho: Atalho,
        motivo: &'static str,
    },
    /// Outra ação da Oficina já usa a combinação.
    EmUso { atalho: Atalho, por: AcaoAtalho },
}

impl fmt::Display for Recusa {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Recusa::Reservado { atalho, motivo } => write!(
                f,
                "{atalho} é reservado para {motivo} e não pode ser remapeado."
            ),
            Recusa::DaCasca { atalho, motivo } => write!(
                f,
                "{atalho} conflita com um atalho da casca ({motivo}). Escolha outra combinação."
            ),
            Recusa::EmUso { atalho, por } => write!(
                f,
                "{atalho} já é usado por «{}». Libere esse atalho antes de reutilizá-lo.",
                por.rotulo()
            ),
        }
    }
}

impl std::error::Error for Recusa {}

/// Aviso (não recusa) de conflito com a tela Teclado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AvisoTelaTeclado {
    pub atalho: Atalho,
    pub letra: &'static str,
}

impl fmt::Display for AvisoTelaTeclado {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} conflita com a tela Teclado, onde a tecla {} toca nota. Só vale na Oficina.",
            self.atalho, self.letra
        )
    }
}

/// Para onde vai uma tecla roteada pela Oficina.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destino {
    /// Comando de navegação existente.
    Comando(Comando),
    /// Mudar a altura da nota selecionada (ticket 14): vira o comando tipado do serviço.
    Altura(UnidadeDeAltura, SentidoDaAltura),
    /// Desfazer a última edição (ticket 15): chama o histórico do serviço de documento.
    Desfazer,
    /// Refazer a edição desfeita (ticket 15).
    Refazer,
    /// Escolher a figura da nota selecionada com a tecla `1` a `9` (ticket 16).
    Duracao(u8),
    /// Alternar o ponto de aumento da nota selecionada (ticket 16).
    Ponto,
    /// Remover a nota selecionada, que vira pausa de mesma duração (ticket 17).
    Apagar,
    /// Acrescentar ao acorde uma altura da nota selecionada, ou retirá-la (ticket 18).
    Acorde(PedidoDeAcorde),
    /// Inserir a nota da letra `A` a `G` na posição do cursor de entrada (ticket 17b).
    InserirNota(char),
    /// Inserir uma pausa na posição do cursor de entrada (ticket 17b).
    InserirPausa,
    /// Mover o cursor de entrada (ticket 17b).
    Entrada(MovimentoDeEntrada),
    /// Ação do perfil sem comando ainda: a tela avisa.
    Pendente(AcaoAtalho),
}

/// O que um atalho de acorde pede ao serviço de documento (ticket 18).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PedidoDeAcorde {
    /// A letra `A` a `G` na oitava mais próxima da nota selecionada.
    Letra(char),
    /// O intervalo de `2` (segunda) a `9` (nona) acima da nota selecionada.
    Intervalo(u8),
    /// Retira a altura da nota selecionada do acorde.
    Retirar,
}

/// O mapa ação para combinação. `None` = ação sem atalho.
#[derive(Clone, Debug, PartialEq)]
pub struct MapaAtalhos {
    ligacoes: Vec<(AcaoAtalho, Option<Atalho>)>,
}

impl Default for MapaAtalhos {
    fn default() -> Self {
        Self::padrao()
    }
}

impl MapaAtalhos {
    /// O perfil padrão.
    pub fn padrao() -> Self {
        Self {
            ligacoes: ACOES.iter().map(|a| (*a, Some(a.padrao()))).collect(),
        }
    }

    /// Combinação atual da ação.
    pub fn atalho(&self, acao: AcaoAtalho) -> Option<Atalho> {
        self.ligacoes
            .iter()
            .find(|(a, _)| *a == acao)
            .and_then(|(_, c)| *c)
    }

    /// Ação que usa a combinação, se houver.
    pub fn acao_de(&self, atalho: Atalho) -> Option<AcaoAtalho> {
        self.ligacoes
            .iter()
            .find(|(_, c)| *c == Some(atalho))
            .map(|(a, _)| *a)
    }

    /// Linhas da lista de atalhos: ação, combinação e se está pendente de comando.
    pub fn linhas(&self) -> impl Iterator<Item = (AcaoAtalho, Option<Atalho>)> + '_ {
        self.ligacoes.iter().copied()
    }

    /// Ações com atalho mas ainda sem comando associado (não somem em silêncio).
    pub fn pendentes(&self) -> Vec<AcaoAtalho> {
        self.ligacoes
            .iter()
            .filter(|(a, _)| a.pendente())
            .map(|(a, _)| *a)
            .collect()
    }

    /// Conflitos com a tela Teclado das combinações atuais (sinalizados, não recusados).
    pub fn avisos_tela_teclado(&self) -> Vec<(AcaoAtalho, AvisoTelaTeclado)> {
        self.ligacoes
            .iter()
            .filter_map(|(a, c)| {
                let atalho = (*c)?;
                let letra = conflito_com_tela_teclado(atalho)?;
                Some((*a, AvisoTelaTeclado { atalho, letra }))
            })
            .collect()
    }

    fn checar(&self, acao: AcaoAtalho, atalho: Atalho) -> Result<(), Recusa> {
        if let Some(motivo) = reservado(atalho) {
            return Err(Recusa::Reservado { atalho, motivo });
        }
        if let Some(motivo) = da_casca(atalho) {
            return Err(Recusa::DaCasca { atalho, motivo });
        }
        match self.acao_de(atalho) {
            Some(por) if por != acao => Err(Recusa::EmUso { atalho, por }),
            _ => Ok(()),
        }
    }

    /// Atribui `atalho` à ação, valendo já. Recusa reservado, atalho da casca e atalho em uso;
    /// com a tela Teclado só avisa (o aviso vem no `Ok`).
    pub fn remapear(
        &mut self,
        acao: AcaoAtalho,
        atalho: Atalho,
    ) -> Result<Option<AvisoTelaTeclado>, Recusa> {
        self.checar(acao, atalho)?;
        if let Some(l) = self.ligacoes.iter_mut().find(|(a, _)| *a == acao) {
            l.1 = Some(atalho);
        }
        Ok(conflito_com_tela_teclado(atalho).map(|letra| AvisoTelaTeclado { atalho, letra }))
    }

    /// Deixa a ação sem atalho.
    pub fn remover(&mut self, acao: AcaoAtalho) {
        if let Some(l) = self.ligacoes.iter_mut().find(|(a, _)| *a == acao) {
            l.1 = None;
        }
    }

    /// Volta tudo ao perfil padrão.
    pub fn restaurar_padrao(&mut self) {
        *self = Self::padrao();
    }

    /// Roteia uma tecla apertada. Reservados (Ctrl+K, Ctrl+., Esc) e atalhos da casca nunca são
    /// capturados. `tab_ativo` é falso com uma voz só: aí `Tab` é da janela.
    pub fn rotear(&self, tecla: Key, mods: Modifiers, tab_ativo: bool) -> Option<Destino> {
        if tecla == Key::Escape {
            return None;
        }
        // Ctrl cru e Ctrl/Cmd "de comando" valem igual (a combinação guarda só "Ctrl").
        let mods = Modifiers {
            command: mods.command || mods.ctrl,
            ctrl: false,
            mac_cmd: false,
            ..mods
        };
        let acao = self
            .ligacoes
            .iter()
            .find(|(_, c)| c.is_some_and(|c| c.bate(tecla, mods)))
            .map(|(a, _)| *a)?;
        let atalho = self.atalho(acao)?;
        if reservado(atalho).is_some() || da_casca(atalho).is_some() {
            return None;
        }
        if tecla == Key::Tab && !tab_ativo {
            return None;
        }
        Some(match (acao.comando(), acao.altura()) {
            (Some(c), _) => Destino::Comando(c),
            (None, Some((unidade, sentido))) => Destino::Altura(unidade, sentido),
            (None, None) => acao
                .historico()
                .or_else(|| acao.duracao())
                .or_else(|| acao.apagar())
                .or_else(|| acao.acorde())
                .or_else(|| acao.entrada())
                .unwrap_or(Destino::Pendente(acao)),
        })
    }

    /// Texto para o perfil, uma linha `id=combinação` por ação.
    pub fn para_texto(&self) -> String {
        let mut kv = Kv::novo();
        for (a, c) in &self.ligacoes {
            kv.por(&a.id(), c.map_or_else(|| "-".to_owned(), texto_canonico));
        }
        kv.texto()
    }

    /// Lê o texto do perfil, tolerante: o que faltar ou não valer cai no padrão.
    pub fn de_texto(t: &str) -> Self {
        let kv = Kv::de_texto(t);
        let mut mapa = Self {
            ligacoes: ACOES.iter().map(|a| (*a, None)).collect(),
        };
        let candidatos: Vec<(AcaoAtalho, Option<Atalho>)> = ACOES
            .iter()
            .map(|acao| {
                let c = match kv.ler::<String>(&acao.id()) {
                    Some(v) if v == "-" => None,
                    Some(v) => Atalho::ler(&v).ok().or(Some(acao.padrao())),
                    None => Some(acao.padrao()),
                };
                (*acao, c)
            })
            .collect();
        // Primeiro quem ficou no padrão, depois os remapeados: um remapeamento não rouba o
        // padrão de outra ação. O que não vale volta ao padrão, se livre, ou fica sem atalho.
        let (padroes, outros): (Vec<_>, Vec<_>) = candidatos
            .into_iter()
            .partition(|(a, c)| *c == Some(a.padrao()));
        for (acao, c) in padroes.into_iter().chain(outros) {
            let escolhido = [c, Some(acao.padrao())]
                .into_iter()
                .flatten()
                .find(|x| mapa.checar(acao, *x).is_ok());
            if c.is_none() {
                continue;
            }
            if let Some(x) = escolhido
                && let Some(l) = mapa.ligacoes.iter_mut().find(|(a, _)| *a == acao)
            {
                l.1 = Some(x);
            }
        }
        mapa
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn mods(ctrl: bool, alt: bool, shift: bool) -> Modifiers {
        Modifiers {
            alt,
            ctrl: false,
            shift,
            mac_cmd: false,
            command: ctrl,
        }
    }

    #[test]
    fn padrao_segue_musescore_e_as_decisoes_do_titular() {
        let m = MapaAtalhos::padrao();
        let esperado = [
            (AcaoAtalho::SemitomAcima, "Up"),
            (AcaoAtalho::GrauAcima, "Alt+Shift+Up"),
            (AcaoAtalho::OitavaAcima, "Ctrl+Up"),
            (AcaoAtalho::Duracao(1), "1"),
            (AcaoAtalho::Duracao(9), "9"),
            (AcaoAtalho::Ponto, "Period"),
            (AcaoAtalho::Desfazer, "Ctrl+Z"),
            (AcaoAtalho::Refazer, "Ctrl+Shift+Z"),
            (AcaoAtalho::Cifra, "Ctrl+Shift+K"),
            (AcaoAtalho::Apagar, "Delete"),
            (AcaoAtalho::AcrescentarNota('C'), "Shift+C"),
            (AcaoAtalho::IntervaloAcima(3), "Alt+3"),
            (AcaoAtalho::RetirarAltura, "Ctrl+Delete"),
        ];
        for (a, txt) in esperado {
            assert_eq!(texto_canonico(m.atalho(a).expect("ok")), txt, "{a:?}");
        }
    }

    #[test]
    fn padrao_nao_tem_reservado_nem_atalho_da_casca_nem_repeticao() {
        let m = MapaAtalhos::padrao();
        let todos: Vec<Atalho> = m.linhas().filter_map(|(_, c)| c).collect();
        assert_eq!(todos.len(), ACOES.len());
        for (i, a) in todos.iter().enumerate() {
            assert!(reservado(*a).is_none(), "{a} é reservado");
            assert!(da_casca(*a).is_none(), "{a} é da casca");
            assert!(!todos[..i].contains(a), "{a} repetido");
        }
    }

    #[test]
    fn ids_sao_unicos_e_rotulos_em_ptbr_nao_vazios() {
        let mut ids: Vec<String> = ACOES.iter().map(|a| a.id()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), ACOES.len());
        for a in ACOES {
            assert!(a.rotulo().chars().count() > 3, "{a:?}");
            assert_eq!(AcaoAtalho::de_id(&a.id()), Some(a));
        }
        assert!(AcaoAtalho::Duracao(5).rotulo().contains("semínima"));
    }

    #[test]
    fn ctrl_k_ctrl_ponto_e_esc_nunca_sao_capturados_pela_oficina() {
        let m = MapaAtalhos::padrao();
        assert_eq!(m.rotear(Key::K, mods(true, false, false), true), None);
        assert_eq!(m.rotear(Key::Period, mods(true, false, false), true), None);
        assert_eq!(m.rotear(Key::Escape, Modifiers::NONE, true), None);
        assert_eq!(m.rotear(Key::Escape, Modifiers::SHIFT, true), None);
        // Cifra é Ctrl+Shift+K: outra combinação, roteada (pendente).
        assert_eq!(
            m.rotear(Key::K, mods(true, false, true), true),
            Some(Destino::Pendente(AcaoAtalho::Cifra))
        );
        // Ponto sem Ctrl é a duração pontuada, não o Pânico.
        assert_eq!(
            m.rotear(Key::Period, Modifiers::NONE, true),
            Some(Destino::Ponto)
        );
    }

    #[test]
    fn mesmo_forcando_um_mapa_corrompido_os_reservados_nao_sao_roteados() {
        let mut m = MapaAtalhos::padrao();
        m.ligacoes[0].1 = Some(Atalho::ctrl(Key::K));
        m.ligacoes[1].1 = Some(Atalho::ctrl(Key::Period));
        m.ligacoes[2].1 = Some(Atalho::tecla(Key::Escape));
        assert_eq!(m.rotear(Key::K, mods(true, false, false), true), None);
        assert_eq!(m.rotear(Key::Period, mods(true, false, false), true), None);
        assert_eq!(m.rotear(Key::Escape, Modifiers::NONE, true), None);
    }

    #[test]
    fn navegacao_padrao_e_tab_so_com_mais_de_uma_trilha() {
        let m = MapaAtalhos::padrao();
        let n = Modifiers::NONE;
        assert_eq!(
            m.rotear(Key::ArrowLeft, n, false),
            Some(Destino::Comando(Comando::Anterior))
        );
        assert_eq!(
            m.rotear(Key::ArrowRight, n, false),
            Some(Destino::Comando(Comando::Seguinte))
        );
        assert_eq!(
            m.rotear(Key::Tab, n, true),
            Some(Destino::Comando(Comando::TrilhaSeguinte))
        );
        assert_eq!(
            m.rotear(Key::Tab, Modifiers::SHIFT, true),
            Some(Destino::Comando(Comando::TrilhaAnterior))
        );
        assert_eq!(m.rotear(Key::Tab, n, false), None);
        assert_eq!(m.rotear(Key::Tab, Modifiers::SHIFT, false), None);
        // Teclas sem ação e atalhos da casca ficam com a casca.
        assert_eq!(m.rotear(Key::Space, n, true), None);
        assert_eq!(m.rotear(Key::T, mods(true, false, false), true), None);
        assert_eq!(
            m.rotear(Key::ArrowLeft, mods(false, true, false), true),
            None
        );
    }

    #[test]
    fn acoes_sem_comando_ficam_registradas_como_pendentes_e_roteiam_como_pendente() {
        let m = MapaAtalhos::padrao();
        let p = m.pendentes();
        // Navegação (4), altura (6), histórico (2), duração (9 teclas e o ponto), apagar (1),
        // acorde (Shift+letra, Alt+2 a Alt+9 e retirar altura: 7 + 8 + 1) e entrada de notas
        // (7 letras, a pausa e 4 movimentos do cursor) têm comando; só a cifra segue pendente.
        assert_eq!(p.len(), ACOES.len() - 4 - 6 - 2 - 10 - 1 - 7 - 8 - 1 - 12);
        assert_eq!(p.len(), 1);
        assert!(p.contains(&AcaoAtalho::Cifra));
        assert!(!p.contains(&AcaoAtalho::Apagar));
        assert!(!p.contains(&AcaoAtalho::Duracao(3)));
        assert!(!p.contains(&AcaoAtalho::Ponto));
        assert!(!p.contains(&AcaoAtalho::NotaSeguinte));
        assert!(!p.contains(&AcaoAtalho::SemitomAcima));
        assert!(!p.contains(&AcaoAtalho::Desfazer));
        assert!(!p.contains(&AcaoAtalho::Refazer));
        assert_eq!(
            m.rotear(Key::Z, mods(true, false, false), true),
            Some(Destino::Desfazer)
        );
        assert_eq!(
            m.rotear(Key::Z, mods(true, false, true), true),
            Some(Destino::Refazer)
        );
        assert_eq!(
            m.rotear(Key::Delete, Modifiers::NONE, true),
            Some(Destino::Apagar)
        );
        assert_eq!(
            m.rotear(Key::K, mods(true, false, true), true),
            Some(Destino::Pendente(AcaoAtalho::Cifra))
        );
    }

    #[test]
    fn as_teclas_um_a_nove_e_o_ponto_roteiam_para_a_duracao_do_servico() {
        let m = MapaAtalhos::padrao();
        let teclas = [
            (Key::Num1, 1),
            (Key::Num2, 2),
            (Key::Num3, 3),
            (Key::Num4, 4),
            (Key::Num5, 5),
            (Key::Num6, 6),
            (Key::Num7, 7),
            (Key::Num8, 8),
            (Key::Num9, 9),
        ];
        for (tecla, n) in teclas {
            assert_eq!(
                m.rotear(tecla, Modifiers::NONE, true),
                Some(Destino::Duracao(n)),
                "{tecla:?}"
            );
        }
        assert_eq!(
            m.rotear(Key::Period, Modifiers::NONE, true),
            Some(Destino::Ponto)
        );
        // Com Ctrl, `1`..`9` e `.` não são duração (e Ctrl+. é o Pânico, nunca capturado).
        assert_eq!(m.rotear(Key::Num4, mods(true, false, false), true), None);
        assert_eq!(m.rotear(Key::Period, mods(true, false, false), true), None);
        // Remapeada, a duração segue a nova tecla e a antiga fica livre.
        let mut m = MapaAtalhos::padrao();
        let novo = Atalho::ctrl(Key::J);
        assert_eq!(m.remapear(AcaoAtalho::Duracao(4), novo), Ok(None));
        assert_eq!(
            m.rotear(Key::J, Modifiers::COMMAND, true),
            Some(Destino::Duracao(4))
        );
        assert_eq!(m.rotear(Key::Num4, Modifiers::NONE, true), None);
        // Nenhum atalho existente saiu do mapa: as ações seguem com combinação própria.
        let padrao = MapaAtalhos::padrao();
        assert_eq!(padrao.linhas().count(), ACOES.len());
        assert!(padrao.linhas().all(|(_, c)| c.is_some()));
    }

    #[test]
    fn as_acoes_de_acorde_roteiam_e_sao_remapeaveis_sem_tirar_nenhum_atalho() {
        let m = MapaAtalhos::padrao();
        // Shift+letra: a letra na oitava mais próxima.
        assert_eq!(
            m.rotear(Key::G, Modifiers::SHIFT, true),
            Some(Destino::Acorde(PedidoDeAcorde::Letra('G')))
        );
        // Alt+2 a Alt+9: o intervalo acima, como no MuseScore.
        let alt = Modifiers {
            alt: true,
            ..Modifiers::NONE
        };
        for (tecla, n) in [
            (Key::Num2, 2),
            (Key::Num3, 3),
            (Key::Num5, 5),
            (Key::Num9, 9),
        ] {
            assert_eq!(
                m.rotear(tecla, alt, true),
                Some(Destino::Acorde(PedidoDeAcorde::Intervalo(n))),
                "Alt+{n}"
            );
        }
        // Ctrl+Delete retira a altura; o Delete continua apagando a nota.
        assert_eq!(
            m.rotear(Key::Delete, mods(true, false, false), true),
            Some(Destino::Acorde(PedidoDeAcorde::Retirar))
        );
        assert_eq!(
            m.rotear(Key::Delete, Modifiers::NONE, true),
            Some(Destino::Apagar)
        );
        // As durações `1` a `9` sem Alt seguem sendo duração.
        assert_eq!(
            m.rotear(Key::Num3, Modifiers::NONE, true),
            Some(Destino::Duracao(3))
        );
        // Nenhuma ação de acorde fica pendente nem colide com a casca ou a tela Teclado.
        assert!(m.pendentes().iter().all(|a| a.acorde().is_none()));
        assert!(
            m.avisos_tela_teclado().iter().all(|(a, _)| !matches!(
                a,
                AcaoAtalho::IntervaloAcima(_) | AcaoAtalho::RetirarAltura
            ))
        );
        // Remapear: a tecla nova passa a valer e a antiga fica livre.
        let mut m = MapaAtalhos::padrao();
        let nova = Atalho::ctrl(Key::J);
        assert_eq!(m.remapear(AcaoAtalho::RetirarAltura, nova), Ok(None));
        assert_eq!(
            m.rotear(Key::J, Modifiers::COMMAND, true),
            Some(Destino::Acorde(PedidoDeAcorde::Retirar))
        );
        assert_eq!(m.rotear(Key::Delete, mods(true, false, false), true), None);
        // Reutilizar um atalho em uso é recusado, citando a ação.
        let r = m.remapear(AcaoAtalho::IntervaloAcima(3), Atalho::tecla(Key::Delete));
        assert!(matches!(
            r,
            Err(Recusa::EmUso {
                por: AcaoAtalho::Apagar,
                ..
            })
        ));
        // Persistência: o remapeamento sobrevive ao texto do perfil.
        let relido = MapaAtalhos::de_texto(&m.para_texto());
        assert_eq!(relido.atalho(AcaoAtalho::RetirarAltura), Some(nova));
        assert_eq!(
            relido.atalho(AcaoAtalho::IntervaloAcima(4)),
            Some(AcaoAtalho::IntervaloAcima(4).padrao())
        );
    }

    #[test]
    fn as_letras_a_g_e_o_zero_roteiam_para_a_entrada_de_notas_sem_tirar_nenhum_atalho() {
        let m = MapaAtalhos::padrao();
        let letras = [
            (Key::A, 'A'),
            (Key::B, 'B'),
            (Key::C, 'C'),
            (Key::D, 'D'),
            (Key::E, 'E'),
            (Key::F, 'F'),
            (Key::G, 'G'),
        ];
        for (tecla, l) in letras {
            assert_eq!(
                m.rotear(tecla, Modifiers::NONE, true),
                Some(Destino::InserirNota(l)),
                "{tecla:?}"
            );
            // Shift+letra continua sendo "acrescentar ao acorde" (ticket 18).
            assert_eq!(
                m.rotear(tecla, Modifiers::SHIFT, true),
                Some(Destino::Acorde(PedidoDeAcorde::Letra(l))),
                "{tecla:?}"
            );
        }
        assert_eq!(
            m.rotear(Key::Num0, Modifiers::NONE, true),
            Some(Destino::InserirPausa)
        );
        use MovimentoDeEntrada::{Abaixo, Acima, Anterior, Seguinte};
        let movimentos = [
            (Key::ArrowLeft, mods(true, false, false), Anterior),
            (Key::ArrowRight, mods(true, false, false), Seguinte),
            (Key::ArrowUp, mods(false, true, false), Acima),
            (Key::ArrowDown, mods(false, true, false), Abaixo),
        ];
        for (tecla, m_, mov) in movimentos {
            assert_eq!(m.rotear(tecla, m_, true), Some(Destino::Entrada(mov)));
        }
        // As setas de antes seguem iguais (navegação por notas e altura).
        assert_eq!(
            m.rotear(Key::ArrowLeft, Modifiers::NONE, true),
            Some(Destino::Comando(Comando::Anterior))
        );
        assert_eq!(
            m.rotear(Key::ArrowUp, Modifiers::NONE, true),
            Some(Destino::Altura(
                UnidadeDeAltura::Semitom,
                SentidoDaAltura::Acima
            ))
        );
        // Todas as ações novas são remapeáveis e as antigas ficam onde estavam.
        let mut r = MapaAtalhos::padrao();
        let novo = Atalho::ctrl(Key::J);
        assert_eq!(r.remapear(AcaoAtalho::InserirNota('C'), novo), Ok(None));
        assert_eq!(
            r.rotear(Key::J, Modifiers::COMMAND, true),
            Some(Destino::InserirNota('C'))
        );
        assert_eq!(r.rotear(Key::C, Modifiers::NONE, true), None);
        assert!(
            r.remapear(AcaoAtalho::InserirPausa, Atalho::tecla(Key::R))
                .is_ok()
        );
        assert_eq!(
            r.rotear(Key::R, Modifiers::NONE, true),
            Some(Destino::InserirPausa)
        );
        // Uma combinação em uso continua sendo recusada, citando a outra ação.
        assert!(matches!(
            r.remapear(AcaoAtalho::EntradaAcima, Atalho::tecla(Key::Delete)),
            Err(Recusa::EmUso {
                por: AcaoAtalho::Apagar,
                ..
            })
        ));
        // Os 31 atalhos de antes do 17b continuam todos no mapa, com o mesmo padrão.
        let antigos: Vec<AcaoAtalho> = ACOES[..31].to_vec();
        for a in antigos {
            assert_eq!(MapaAtalhos::padrao().atalho(a), Some(a.padrao()), "{a:?}");
        }
        assert_eq!(ACOES.len(), 52);
    }

    #[test]
    fn as_seis_acoes_de_altura_roteiam_para_unidade_e_sentido_do_servico() {
        use SentidoDaAltura::{Abaixo, Acima};
        use UnidadeDeAltura::{GrauDaEscala, Oitava, Semitom};
        let m = MapaAtalhos::padrao();
        let casos = [
            (Key::ArrowUp, Modifiers::NONE, Semitom, Acima),
            (Key::ArrowDown, Modifiers::NONE, Semitom, Abaixo),
            (Key::ArrowUp, mods(false, true, true), GrauDaEscala, Acima),
            (
                Key::ArrowDown,
                mods(false, true, true),
                GrauDaEscala,
                Abaixo,
            ),
            (Key::ArrowUp, mods(true, false, false), Oitava, Acima),
            (Key::ArrowDown, mods(true, false, false), Oitava, Abaixo),
        ];
        for (tecla, mods, unidade, sentido) in casos {
            assert_eq!(
                m.rotear(tecla, mods, true),
                Some(Destino::Altura(unidade, sentido)),
                "{tecla:?} {mods:?}"
            );
        }
        // Alt sem Shift, Shift sem Alt e Ctrl+Shift não são ações de altura (Alt+↑ é, desde o
        // ticket 17b, a posição de entrada na voz de cima).
        assert_eq!(
            m.rotear(Key::ArrowUp, mods(false, true, false), true),
            Some(Destino::Entrada(MovimentoDeEntrada::Acima))
        );
        assert_eq!(m.rotear(Key::ArrowUp, mods(false, false, true), true), None);
        // Remapeada, a ação de altura segue a nova tecla.
        let mut m = MapaAtalhos::padrao();
        let novo = Atalho::ctrl(Key::J);
        assert_eq!(m.remapear(AcaoAtalho::SemitomAcima, novo), Ok(None));
        assert_eq!(
            m.rotear(Key::J, Modifiers::COMMAND, true),
            Some(Destino::Altura(Semitom, Acima))
        );
        assert_eq!(m.rotear(Key::ArrowUp, Modifiers::NONE, true), None);
    }

    #[test]
    fn remapear_vale_na_hora_sem_reiniciar() {
        let mut m = MapaAtalhos::padrao();
        let novo = Atalho::ctrl(Key::ArrowRight).com_shift();
        assert_eq!(m.remapear(AcaoAtalho::NotaSeguinte, novo), Ok(None));
        assert_eq!(m.atalho(AcaoAtalho::NotaSeguinte), Some(novo));
        assert_eq!(
            m.rotear(Key::ArrowRight, mods(true, false, true), true),
            Some(Destino::Comando(Comando::Seguinte))
        );
        assert_eq!(m.rotear(Key::ArrowRight, Modifiers::NONE, true), None);
        // A combinação antiga ficou livre.
        assert_eq!(
            m.remapear(AcaoAtalho::Ponto, Atalho::tecla(Key::ArrowRight)),
            Ok(None)
        );
    }

    #[test]
    fn atribuir_reservado_e_recusado_com_mensagem_que_cita_o_conflito() {
        let mut m = MapaAtalhos::padrao();
        let antes = m.clone();
        let e = m
            .remapear(AcaoAtalho::Cifra, Atalho::ctrl(Key::K))
            .expect_err("deveria recusar");
        assert!(matches!(e, Recusa::Reservado { .. }));
        assert!(e.to_string().contains("Ctrl+K"), "{e}");
        assert!(e.to_string().contains("paleta"), "{e}");
        let e = m
            .remapear(AcaoAtalho::Cifra, Atalho::ctrl(Key::Period))
            .expect_err("deveria recusar");
        assert!(e.to_string().contains("Pânico"), "{e}");
        let e = m
            .remapear(AcaoAtalho::Cifra, Atalho::tecla(Key::Escape))
            .expect_err("deveria recusar");
        assert!(e.to_string().contains("Esc"), "{e}");
        assert_eq!(m, antes, "recusa não muda o mapa");
    }

    #[test]
    fn atribuir_atalho_ja_usado_e_recusado_citando_a_outra_acao() {
        let mut m = MapaAtalhos::padrao();
        let e = m
            .remapear(AcaoAtalho::Ponto, Atalho::ctrl(Key::Z))
            .expect_err("deveria recusar");
        assert_eq!(
            e,
            Recusa::EmUso {
                atalho: Atalho::ctrl(Key::Z),
                por: AcaoAtalho::Desfazer
            }
        );
        assert!(e.to_string().contains("Desfazer"), "{e}");
        // Reatribuir a própria combinação é aceito.
        assert!(
            m.remapear(AcaoAtalho::Desfazer, Atalho::ctrl(Key::Z))
                .is_ok()
        );
        // Liberada, pode ser reutilizada.
        m.remover(AcaoAtalho::Desfazer);
        assert!(m.remapear(AcaoAtalho::Ponto, Atalho::ctrl(Key::Z)).is_ok());
    }

    #[test]
    fn atalho_da_casca_e_recusado() {
        let mut m = MapaAtalhos::padrao();
        for a in [
            Atalho::ctrl(Key::T),
            Atalho::ctrl(Key::F1),
            Atalho::tecla(Key::F11),
            Atalho::alt(Key::ArrowRight),
        ] {
            let e = m
                .remapear(AcaoAtalho::Ponto, a)
                .expect_err("deveria recusar");
            assert!(matches!(e, Recusa::DaCasca { .. }), "{a}");
            assert!(e.to_string().contains("casca"), "{e}");
        }
    }

    #[test]
    fn conflito_com_a_tela_teclado_e_sinalizado_mas_aceito() {
        let mut m = MapaAtalhos::padrao();
        let r = m
            .remapear(AcaoAtalho::Ponto, Atalho::tecla(Key::K))
            .expect("ok");
        let aviso = r.expect("aviso da tela Teclado");
        assert_eq!(aviso.letra, "K");
        assert!(aviso.to_string().contains("tela Teclado"));
        // Com Ctrl não há conflito.
        assert_eq!(
            m.remapear(AcaoAtalho::Apagar, Atalho::ctrl(Key::A)),
            Ok(None)
        );
        // O padrão já sinaliza Shift+A, D, E, F, G (a tela Teclado ignora o Shift) e, desde o
        // ticket 17b, as mesmas letras sem Shift (inserir nota).
        let letras: Vec<&str> = MapaAtalhos::padrao()
            .avisos_tela_teclado()
            .iter()
            .map(|(_, a)| a.letra)
            .collect();
        assert_eq!(letras, ["A", "D", "E", "F", "G", "A", "D", "E", "F", "G"]);
    }

    #[test]
    fn ida_e_volta_pelo_texto_do_perfil() {
        let mut m = MapaAtalhos::padrao();
        m.remapear(AcaoAtalho::NotaSeguinte, Atalho::ctrl(Key::J))
            .expect("ok");
        m.remover(AcaoAtalho::Apagar);
        let t = m.para_texto();
        assert!(t.contains("nota_seguinte=Ctrl+J"), "{t}");
        assert!(t.contains("apagar=-"), "{t}");
        assert!(t.contains("refazer=Ctrl+Shift+Z"), "{t}");
        let lido = MapaAtalhos::de_texto(&t);
        assert_eq!(lido, m);
        assert_eq!(lido.atalho(AcaoAtalho::Apagar), None);
        assert_eq!(MapaAtalhos::de_texto(&lido.para_texto()), lido);
    }

    #[test]
    fn texto_ausente_ou_estragado_cai_no_padrao() {
        assert_eq!(MapaAtalhos::de_texto(""), MapaAtalhos::padrao());
        let lixo = "nota_seguinte=Ctrl+Banana\ncifra=Ctrl+K\nrefazer=Ctrl+Shift+Z\n\
                    ponto=Ctrl+Z\nid_inexistente=A\n=\n";
        let m = MapaAtalhos::de_texto(lixo);
        assert_eq!(
            m.atalho(AcaoAtalho::NotaSeguinte),
            Some(AcaoAtalho::NotaSeguinte.padrao())
        );
        // Ctrl+K reservado: a cifra volta ao padrão (Ctrl+Shift+K), nunca fica com o reservado.
        assert_eq!(
            m.atalho(AcaoAtalho::Cifra),
            Some(AcaoAtalho::Cifra.padrao())
        );
        // Ctrl+Z é o padrão do Desfazer: o remapeamento do ponto não o rouba, e o ponto volta
        // ao seu próprio padrão.
        assert_eq!(m.atalho(AcaoAtalho::Desfazer), Some(Atalho::ctrl(Key::Z)));
        assert_eq!(
            m.atalho(AcaoAtalho::Ponto),
            Some(AcaoAtalho::Ponto.padrao())
        );
        assert_eq!(m.rotear(Key::K, mods(true, false, false), true), None);
    }

    #[test]
    fn todos_os_atalhos_padrao_sobrevivem_ao_texto_canonico() {
        for a in ACOES {
            let c = a.padrao();
            assert_eq!(Atalho::ler(&texto_canonico(c)), Ok(c), "{a:?}");
        }
    }

    #[test]
    fn a_lista_da_casca_cobre_o_atalho_do_panico_do_app() {
        assert_eq!(
            reservado(crate::acoes::ATALHO_PANICO).map(|m| m.contains("Pânico")),
            Some(true)
        );
    }
}
