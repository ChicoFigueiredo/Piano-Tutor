//! Identidade de nota e persistência: o `id` do MusicXML quando existir; senão a posição
//! (parte, compasso, ordinal), guardada num arquivo lateral ligado ao texto por impressão digital.

use super::erro::ErroDocumento;
use super::exportar::{Impressao, impressao_de};
use super::{Documento, IdNota};
use std::collections::BTreeMap;

/// Como uma nota é reencontrada ao reabrir o arquivo.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChaveNota {
    /// Atributo `id` da nota no MusicXML.
    Xml(String),
    /// Posição: parte, índice do compasso e ordinal da nota no compasso.
    Posicao {
        parte: String,
        compasso: usize,
        ordinal: usize,
    },
}

/// Tabela chave → `IdNota` mais o contador monotônico. O contador nunca volta: ids de notas que
/// sumiram não são reaproveitados.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabelaIds {
    pub(crate) proximo: u64,
    pub(crate) mapa: BTreeMap<ChaveNota, IdNota>,
    /// Texto para o qual a tabela vale (obrigatório em tabela gravada em arquivo lateral).
    pub(crate) base: Option<Impressao>,
}

impl Default for TabelaIds {
    fn default() -> Self {
        Self {
            proximo: 1,
            mapa: BTreeMap::new(),
            base: None,
        }
    }
}

const CABECALHO: &str = "piano-tutor-ids v1";

impl TabelaIds {
    pub fn proximo(&self) -> u64 {
        self.proximo
    }

    pub fn len(&self) -> usize {
        self.mapa.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mapa.is_empty()
    }

    pub(crate) fn alocar(&mut self) -> IdNota {
        let id = IdNota(self.proximo);
        self.proximo += 1;
        id
    }

    /// Arquivo lateral em texto simples (uma linha por nota).
    pub fn para_texto(&self) -> String {
        let mut s = format!("{CABECALHO}\nproximo={}\n", self.proximo);
        if let Some(b) = &self.base {
            s.push_str(&format!("base={}:{:016x}\n", b.tamanho, b.fnv));
        }
        for (k, id) in &self.mapa {
            match k {
                ChaveNota::Xml(x) => s.push_str(&format!("xml\t{x}\t{}\n", id.0)),
                ChaveNota::Posicao {
                    parte,
                    compasso,
                    ordinal,
                } => s.push_str(&format!("pos\t{parte}\t{compasso}\t{ordinal}\t{}\n", id.0)),
            }
        }
        s
    }

    pub fn de_texto(texto: &str) -> Result<Self, ErroDocumento> {
        let invalida = |m: &str| ErroDocumento::TabelaIdsInvalida(m.to_owned());
        let mut linhas = texto.lines();
        if linhas.next() != Some(CABECALHO) {
            return Err(invalida("cabeçalho desconhecido"));
        }
        let mut t = Self::default();
        let mut tem_proximo = false;
        for l in linhas {
            if let Some(v) = l.strip_prefix("proximo=") {
                t.proximo = v.parse().map_err(|_| invalida("proximo"))?;
                tem_proximo = true;
            } else if let Some(v) = l.strip_prefix("base=") {
                let (tam, h) = v.split_once(':').ok_or_else(|| invalida("base"))?;
                t.base = Some(Impressao {
                    tamanho: tam.parse().map_err(|_| invalida("base"))?,
                    fnv: u64::from_str_radix(h, 16).map_err(|_| invalida("base"))?,
                });
            } else {
                let c: Vec<&str> = l.split('\t').collect();
                let (chave, id) = match c.as_slice() {
                    ["xml", x, id] => (ChaveNota::Xml((*x).to_owned()), *id),
                    ["pos", p, c, o, id] => (
                        ChaveNota::Posicao {
                            parte: (*p).to_owned(),
                            compasso: c.parse().map_err(|_| invalida("compasso"))?,
                            ordinal: o.parse().map_err(|_| invalida("ordinal"))?,
                        },
                        *id,
                    ),
                    _ => return Err(invalida("linha")),
                };
                let id: u64 = id.parse().map_err(|_| invalida("id"))?;
                if id >= t.proximo && tem_proximo {
                    return Err(invalida("id maior ou igual ao contador"));
                }
                t.mapa.insert(chave, IdNota(id));
            }
        }
        if !tem_proximo {
            return Err(invalida("sem contador"));
        }
        Ok(t)
    }
}

impl Documento {
    /// Tabela de ids do documento (válida para o texto original aberto).
    pub fn tabela_ids(&self) -> TabelaIds {
        let mut t = self.ids.clone();
        t.base = Some(impressao_de(&self.original));
        t
    }

    /// Tabela para gravar ao lado de `texto_exportado`: as chaves por posição continuam valendo
    /// quando a edição não mudou a estrutura (altura, duração); a impressão é a do texto novo.
    pub fn tabela_ids_para(&self, texto_exportado: &str) -> TabelaIds {
        let mut t = self.ids.clone();
        t.base = Some(impressao_de(texto_exportado));
        t
    }
}
