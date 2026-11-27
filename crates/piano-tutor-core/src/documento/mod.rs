//! Modelo de documento híbrido do núcleo (ADR-0017): texto original imutável, modelo semântico
//! com [`IdNota`] estável, **faixa de bytes de origem** de cada parte, compasso, nota e altura, e
//! **bit sujo** por compasso.
//!
//! Exportar copia byte a byte tudo que não está sujo e regenera só os compassos sujos (peças:
//! texto original por faixa mais filhos desconhecidos preservados). O texto original nunca muda:
//! as faixas só valem para ele, e `exportar` produz um texto novo aplicando os patches uma única
//! vez, de trás para frente.
//!
//! A porta de entrada para telas e ferramentas é o [`servico`] ([`DocumentoAberto`]): abre
//! MusicXML ou MXL, entrega o [`Documento`], a [`crate::partitura::Partitura`] derivada e as
//! identidades de nota do layout, e exporta. Sem janela, áudio, rede nem disco: quem grava
//! arquivos é o chamador. Nada aqui é interface (seam S1).

mod comando;
mod duracao;
mod edicao;
mod editar_duracao;
mod erro;
mod exportar;
mod grafia;
mod historico;
mod ids;
mod leitura;
pub mod servico;
mod texto;

pub use comando::{
    AvisoDeEdicao, ComandoDeEdicao, ErroDeEdicao, LimitesDeAltura, ResultadoDaEdicao,
    SentidoDaAltura, UnidadeDeAltura, figura_da_tecla, nome_da_figura,
};
pub use duracao::mmc_dos_denominadores;
pub use editar_duracao::{MudancaDeDuracao, ValorDeDuracao};
pub use erro::ErroDocumento;
pub use exportar::{Impressao, Patch, Patches, impressao_de};
pub use grafia::{nome_da_altura, nome_do_midi};
pub use historico::{LIMITE_PADRAO_DO_HISTORICO, ResultadoDoHistorico};
pub use ids::{ChaveNota, TabelaIds};
pub use servico::{DocumentoAberto, ErroServico, Formato, IdsPorNota};

use crate::partitura::Fracao;
use std::collections::HashMap;
use std::fmt;
use std::ops::Range;
use texto::No;

/// Posição de um compasso: parte (id) e índice (0 = primeiro compasso da parte).
///
/// A igualdade considera só `parte` e `indice`; `numero` (o rótulo impresso) serve para mensagens.
#[derive(Debug, Clone, Eq)]
pub struct PosCompasso {
    pub parte: String,
    pub indice: usize,
    pub numero: String,
}

impl PosCompasso {
    /// Posição para listas de compassos editáveis (sem rótulo).
    pub fn nova(parte: &str, indice: usize) -> Self {
        Self {
            parte: parte.to_owned(),
            indice,
            numero: String::new(),
        }
    }
}

impl PartialEq for PosCompasso {
    fn eq(&self, o: &Self) -> bool {
        self.parte == o.parte && self.indice == o.indice
    }
}

impl fmt::Display for PosCompasso {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.parte, self.indice)?;
        if !self.numero.is_empty() {
            write!(f, " (compasso {})", self.numero)?;
        }
        Ok(())
    }
}

/// Identidade estável de uma nota: monotônica por documento, atribuída na leitura, nunca
/// reutilizada (o contador só cresce) e nunca derivada do índice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IdNota(pub u64);

/// Altura escrita (`<pitch>`): passo `A..=G`, alteração em semitons inteiros e oitava `0..=9`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Altura {
    pub passo: char,
    pub alter: i8,
    pub oitava: i8,
}

impl Altura {
    /// Número MIDI (Dó4 = 60), sem checar limites; `None` se o passo não é `A..=G`.
    pub fn numero_midi(self) -> Option<i32> {
        grafia::midi(self)
    }
}

/// Dados semânticos de uma nota, lidos na abertura (a verdade textual continua nas faixas).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nota {
    pub id: IdNota,
    /// Atributo `id` da nota no MusicXML, quando existir.
    pub id_xml: Option<String>,
    /// `None` em pausa ou quando a altura não é representável (alteração fracionária).
    pub altura: Option<Altura>,
    /// Em semínimas; `None` em nota de graça ou quando `<duration>` não é inteiro.
    pub duracao: Option<Fracao>,
    pub voz: String,
    pub pauta: String,
    pub acorde: bool,
    pub pausa: bool,
    pub graca: bool,
    /// `divisions` vigente quando a nota foi lida.
    pub divisions: i64,
    /// Posição da nota entre as peças do compasso (`texto::No::pecas`).
    pub(crate) idx: usize,
    /// Faixa da nota no texto original.
    pub faixa: Range<usize>,
}

/// Compasso: faixa de origem, bit sujo e a decomposição em peças.
#[derive(Clone, Debug)]
pub struct Compasso {
    pub numero: String,
    pub faixa: Range<usize>,
    pub sujo: bool,
    pub(crate) no: No,
    pub notas: Vec<Nota>,
}

/// Parte (`<part>`): faixa de origem e compassos.
#[derive(Clone, Debug)]
pub struct Parte {
    pub id: String,
    pub faixa: Range<usize>,
    pub compassos: Vec<Compasso>,
    /// Quantas vezes `<divisions>` é declarado na parte (a elevação só suporta uma).
    pub declaracoes_divisions: usize,
}

/// O documento: texto original imutável mais o modelo com origem por faixa.
#[derive(Clone, Debug)]
pub struct Documento {
    original: String,
    partes: Vec<Parte>,
    indice: HashMap<IdNota, (usize, usize, usize)>,
    ids: TabelaIds,
}

impl Documento {
    /// O texto original (imutável).
    pub fn original(&self) -> &str {
        &self.original
    }

    pub fn partes(&self) -> &[Parte] {
        &self.partes
    }

    /// Todas as notas, na ordem do documento.
    pub fn notas(&self) -> impl Iterator<Item = &Nota> {
        self.partes
            .iter()
            .flat_map(|p| p.compassos.iter().flat_map(|c| c.notas.iter()))
    }

    pub fn nota(&self, id: IdNota) -> Option<&Nota> {
        let &(p, c, n) = self.indice.get(&id)?;
        self.partes.get(p)?.compassos.get(c)?.notas.get(n)
    }

    /// Compasso (parte e índice) que contém a nota.
    pub fn posicao_da_nota(&self, id: IdNota) -> Option<PosCompasso> {
        let &(p, c, _) = self.indice.get(&id)?;
        let parte = self.partes.get(p)?;
        let comp = parte.compassos.get(c)?;
        Some(PosCompasso {
            parte: parte.id.clone(),
            indice: c,
            numero: comp.numero.clone(),
        })
    }

    /// Compassos com o bit sujo ligado.
    pub fn compassos_sujos(&self) -> Vec<PosCompasso> {
        let mut v = Vec::new();
        for p in &self.partes {
            for (i, c) in p.compassos.iter().enumerate() {
                if c.sujo {
                    v.push(PosCompasso {
                        parte: p.id.clone(),
                        indice: i,
                        numero: c.numero.clone(),
                    });
                }
            }
        }
        v
    }

    pub fn total_de_notas(&self) -> usize {
        self.indice.len()
    }
}
