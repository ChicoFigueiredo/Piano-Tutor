//! Spike do ticket 08 (E01): modelo híbrido de documento MusicXML.
//!
//! Hipótese: um modelo semântico com [`IdNota`] estável, mais a **faixa de bytes de origem** de cada
//! parte, compasso, nota e altura, mais um **bit sujo** por compasso, permite exportar copiando
//! byte a byte tudo que não foi editado e regenerando só o compasso sujo (reaproveitando os filhos
//! desconhecidos). O texto original nunca muda: as faixas só valem para ele (`exportar` produz um
//! texto novo aplicando os patches uma única vez, de trás para frente).
//!
//! Este módulo vive no crate de fixtures (só para testes) de propósito: o ADR-0017 decide se o
//! código sobe para o núcleo no ticket 10. Nada aqui é UI.

mod duracao;
mod edicao;
mod erro;
mod exportar;
mod ids;
mod leitura;
mod texto;

pub use duracao::mmc_dos_denominadores;
pub use erro::ErroDocumento;
pub use exportar::{Impressao, Patch, Patches, impressao_de};
pub use ids::{ChaveNota, TabelaIds};

use crate::harness::PosCompasso;
use piano_tutor_core::partitura::Fracao;
use std::collections::HashMap;
use std::ops::Range;
use texto::No;

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
