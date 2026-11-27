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
//! - **Pendentes**: ação sem comando ainda ([`AcaoAtalho::comando`] `None`) aparece em
//!   [`MapaAtalhos::pendentes`]; ao apertar a tecla, a tela avisa em vez de calar.
//! - **Persistência**: texto `id=Ctrl+Shift+Z` por linha (seção `tela.atalhos` do perfil,
//!   ADR-0010); `id=-` é "sem atalho". Texto ausente, com id desconhecido ou combinação
//!   inválida, reservada ou repetida cai no padrão (ou fica sem atalho se o padrão colidir).

use crate::espelho::MAPA_PC;
use crate::opcoes::Kv;
use crate::selecao_partitura::Comando;
use bancada_casca::Atalho;
use bancada_casca::egui::{Key, Modifiers};
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
}

/// Todas as ações, na ordem da lista de atalhos.
pub const ACOES: [AcaoAtalho; 31] = [
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
            Self::Ponto => "Aumentar a duração com ponto".into(),
            Self::Desfazer => "Desfazer".into(),
            Self::Refazer => "Refazer".into(),
            Self::Apagar => "Apagar a nota selecionada".into(),
            Self::Cifra => "Cifra (acorde cifrado)".into(),
            Self::AcrescentarNota(l) => {
                format!("Acrescentar ao acorde a nota {} ({l})", nome_da_letra(l))
            }
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
        }
    }

    /// Comando ligado à ação; `None` = ainda sem comando (pendente). Os comandos de edição
    /// entram nas fatias seguintes.
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
    /// Ação do perfil sem comando ainda: a tela avisa.
    Pendente(AcaoAtalho),
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
            .filter(|(a, _)| a.comando().is_none())
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
        Some(match acao.comando() {
            Some(c) => Destino::Comando(c),
            None => Destino::Pendente(acao),
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
            Some(Destino::Pendente(AcaoAtalho::Ponto))
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
        assert_eq!(p.len(), ACOES.len() - 4);
        for a in [
            AcaoAtalho::SemitomAcima,
            AcaoAtalho::Refazer,
            AcaoAtalho::Cifra,
            AcaoAtalho::Duracao(3),
        ] {
            assert!(p.contains(&a), "{a:?}");
        }
        assert!(!p.contains(&AcaoAtalho::NotaSeguinte));
        assert_eq!(
            m.rotear(Key::ArrowUp, Modifiers::NONE, true),
            Some(Destino::Pendente(AcaoAtalho::SemitomAcima))
        );
        assert_eq!(
            m.rotear(Key::Z, mods(true, false, true), true),
            Some(Destino::Pendente(AcaoAtalho::Refazer))
        );
        assert_eq!(
            m.rotear(Key::Num4, Modifiers::NONE, true),
            Some(Destino::Pendente(AcaoAtalho::Duracao(4)))
        );
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
            .remapear(AcaoAtalho::Ponto, Atalho::tecla(Key::A))
            .expect("ok");
        let aviso = r.expect("aviso da tela Teclado");
        assert_eq!(aviso.letra, "A");
        assert!(aviso.to_string().contains("tela Teclado"));
        // Com Ctrl não há conflito.
        assert_eq!(
            m.remapear(AcaoAtalho::Apagar, Atalho::ctrl(Key::A)),
            Ok(None)
        );
        // O padrão já sinaliza Shift+A, D, E, F, G (a tela Teclado ignora o Shift).
        let letras: Vec<&str> = MapaAtalhos::padrao()
            .avisos_tela_teclado()
            .iter()
            .map(|(_, a)| a.letra)
            .collect();
        assert_eq!(letras, ["A", "D", "E", "F", "G"]);
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
