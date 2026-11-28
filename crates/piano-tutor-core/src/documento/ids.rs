//! Identidade de nota e persistência: o `id` do MusicXML quando existir; senão a posição
//! (parte, compasso, ordinal), guardada num arquivo lateral ligado ao texto por impressão digital
//! (decisão do supervisor, ticket 17).
//!
//! # Arquivo lateral
//!
//! Texto simples, uma linha por nota: `piano-tutor-ids v1`, o contador `proximo=`, a amarra
//! `base=tamanho:fnv` (impressão digital do texto para o qual a tabela vale) e a `serie=`.
//! A **série** identifica a linhagem das identidades: nasce na primeira leitura de um texto sem
//! tabela (o FNV desse texto) e é herdada por toda abertura feita com a tabela de um texto
//! anterior. Dois documentos da mesma série têm `IdNota` comparáveis (o diff usa isso); de séries
//! diferentes (ou sem série), não. Ramos que divergem a partir de um ancestral comum guardam
//! ids novos independentes (o contador é o mesmo no ponto da divisão): entre ramos irmãos o
//! pareamento por id pode juntar notas inseridas diferentes; ancestral e descendente, não.

use super::erro::ErroDocumento;
use super::exportar::{Impressao, impressao_de};
use super::{Documento, IdNota};
use std::collections::{BTreeMap, BTreeSet};

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
    /// Série (linhagem) das identidades; ver o módulo. `None` em tabela antiga.
    pub(crate) serie: Option<u64>,
}

impl Default for TabelaIds {
    fn default() -> Self {
        Self {
            proximo: 1,
            mapa: BTreeMap::new(),
            base: None,
            serie: None,
        }
    }
}

const CABECALHO: &str = "piano-tutor-ids v1";

/// Ids novos que uma única edição pode criar, no máximo (folga larga: uma edição mexe num
/// compasso). Ver [`TabelaIds::primeiro_livre`].
const RESERVA_POR_EDICAO: u64 = 1 << 20;

/// Maior valor aceito para o contador `proximo` (2^62). Um contador legítimo é do tamanho do
/// número de notas já criadas, muito abaixo disto; o teto deixa metade do `u64` de folga, de modo
/// que somar os ids novos de uma edição nunca estoura, e um arquivo lateral hostil
/// (`proximo=18446744073709551615`) é recusado como tabela inválida em vez de derrubar o programa
/// (revisão independente da E01, achado R4).
pub const TETO_DO_CONTADOR_DE_IDS: u64 = 1 << 62;

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

    /// Série (linhagem) das identidades, se a tabela a conhece.
    pub fn serie(&self) -> Option<u64> {
        self.serie
    }

    /// Reserva o próximo id. Aritmética verificada: contador no teto vira erro, nunca pânico.
    pub(crate) fn alocar(&mut self) -> Result<IdNota, ErroDocumento> {
        let id = IdNota(self.proximo);
        self.proximo = self.proximo_apos(1)?;
        Ok(id)
    }

    /// O contador depois de reservar `quantos` ids, ou `Overflow` se passaria do teto.
    pub(crate) fn proximo_apos(&self, quantos: u64) -> Result<u64, ErroDocumento> {
        self.proximo
            .checked_add(quantos)
            .filter(|p| *p <= TETO_DO_CONTADOR_DE_IDS)
            .ok_or_else(|| {
                ErroDocumento::Overflow(format!(
                    "contador de identidades de nota no limite ({})",
                    self.proximo
                ))
            })
    }

    /// Primeiro id livre para uma edição que cria poucas notas (`RESERVA_POR_EDICAO`), conferindo
    /// antes que o contador comporta a reserva (as edições somam `novos` ao contador depois).
    pub(crate) fn primeiro_livre(&self) -> Result<u64, ErroDocumento> {
        self.proximo_apos(RESERVA_POR_EDICAO)?;
        Ok(self.proximo)
    }

    /// Arquivo lateral em texto simples (uma linha por nota).
    pub fn para_texto(&self) -> String {
        let mut s = format!("{CABECALHO}\nproximo={}\n", self.proximo);
        if let Some(b) = &self.base {
            s.push_str(&format!("base={}:{:016x}\n", b.tamanho, b.fnv));
        }
        if let Some(serie) = self.serie {
            s.push_str(&format!("serie={serie:016x}\n"));
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
        let mut usados: BTreeSet<u64> = BTreeSet::new();
        for l in linhas {
            if let Some(v) = l.strip_prefix("proximo=") {
                t.proximo = v.parse().map_err(|_| invalida("proximo"))?;
                if t.proximo > TETO_DO_CONTADOR_DE_IDS {
                    return Err(invalida("proximo acima do teto do contador"));
                }
                tem_proximo = true;
            } else if let Some(v) = l.strip_prefix("serie=") {
                t.serie = Some(u64::from_str_radix(v, 16).map_err(|_| invalida("serie"))?);
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
                // Dois ids iguais colapsariam em silêncio (só o último seria endereçável).
                if !usados.insert(id) {
                    return Err(invalida("id repetido"));
                }
                if t.mapa.insert(chave, IdNota(id)).is_some() {
                    return Err(invalida("chave repetida"));
                }
            }
        }
        if !tem_proximo {
            return Err(invalida("sem contador"));
        }
        // Valem todos os ids contra o contador final, em qualquer ordem das linhas.
        if usados.last().is_some_and(|maior| *maior >= t.proximo) {
            return Err(invalida("id maior ou igual ao contador"));
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

    /// Série (linhagem) das identidades deste documento (ver o módulo).
    pub fn serie_de_ids(&self) -> Option<u64> {
        self.ids.serie
    }

    /// As identidades dos dois documentos são comparáveis (mesma série)? Só então o mesmo
    /// [`IdNota`] nos dois lados é a mesma nota (o diff pareia por ele).
    pub fn ids_comparaveis(&self, outro: &Documento) -> bool {
        matches!((self.ids.serie, outro.ids.serie), (Some(a), Some(b)) if a == b)
    }

    /// Tabela para gravar ao lado de `texto_exportado`: refeita a partir do modelo atual, nota por
    /// nota, com a chave que a leitura usará no texto novo (o `id` do MusicXML quando único; senão
    /// a posição parte, compasso e ordinal **no texto exportado**). Assim vale também depois de
    /// editar o compasso criando ou tirando notas (ticket 17); o contador e a série continuam.
    pub fn tabela_ids_para(&self, texto_exportado: &str) -> TabelaIds {
        let mut t = TabelaIds {
            proximo: self.ids.proximo,
            mapa: BTreeMap::new(),
            base: Some(impressao_de(texto_exportado)),
            serie: self.ids.serie,
        };
        let mut usadas: BTreeSet<ChaveNota> = BTreeSet::new();
        for parte in &self.partes {
            for (ci, c) in parte.compassos.iter().enumerate() {
                for (ordinal, n) in c.notas.iter().enumerate() {
                    let por_posicao = ChaveNota::Posicao {
                        parte: parte.id.clone(),
                        compasso: ci,
                        ordinal,
                    };
                    let mut chave = match &n.id_xml {
                        Some(x) => ChaveNota::Xml(x.clone()),
                        None => por_posicao.clone(),
                    };
                    if !usadas.insert(chave.clone()) {
                        chave = por_posicao;
                        usadas.insert(chave.clone());
                    }
                    t.mapa.insert(chave, n.id);
                }
            }
        }
        t
    }
}
