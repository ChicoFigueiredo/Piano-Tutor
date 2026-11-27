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
//!   `alter` e `accidental` sem atributos). O desfazer (ticket 15) usa esse valor e o instantâneo
//!   do compasso; este ticket não o implementa.
//! - **Validação antes de tocar em qualquer coisa.** Nota inexistente, sem altura (pausa) ou
//!   altura fora dos limites ([`LimitesDeAltura`]: MIDI 0 a 127, a oitava 0 a 9 do MusicXML e o
//!   teclado de referência) são recusadas com [`ErroDeEdicao`] e **nada muda**: o documento, o
//!   modelo de tocar e as identidades ficam como estavam.
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
//! - **Nada é gravado.** A edição vive em memória; exportar e gravar arquivo é decisão do
//!   chamador (autosave e Versão são o ticket 21).

use super::grafia::{
    Anterior, acidente_de, alter_do_acidente, alteracao_em_vigor, antes, de_partitura, midi,
    nome_da_altura, nome_do_midi, para_partitura, por_grau, por_oitava, por_semitom,
};
use super::servico::DocumentoAberto;
use super::{Altura, ErroDocumento, IdNota, PosCompasso};
use crate::partitura::Acidente;
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
}

impl ComandoDeEdicao {
    /// A nota que o comando edita.
    pub fn nota(&self) -> IdNota {
        match self {
            Self::MudarAltura { nota, .. } | Self::DefinirAltura { nota, .. } => *nota,
        }
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
        }
    }
}

/// O que um comando fez.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultadoDaEdicao {
    /// A forma resolvida do que foi aplicado (sempre `DefinirAltura`).
    pub aplicado: ComandoDeEdicao,
    /// O comando que desfaz este (sempre `DefinirAltura`).
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
}

impl ResultadoDaEdicao {
    /// Número MIDI da altura nova, para ouvir a nota editada.
    pub fn midi_depois(&self) -> Option<u8> {
        self.depois.numero_midi().and_then(|m| u8::try_from(m).ok())
    }
}

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

    /// Aplica o comando. Em erro, nada muda; em sucesso, o documento (compasso sujo), o modelo
    /// de tocar e o texto exportável já refletem a edição. Ver o contrato do módulo.
    pub fn aplicar(&mut self, comando: ComandoDeEdicao) -> Result<ResultadoDaEdicao, ErroDeEdicao> {
        let id = comando.nota();
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
        };
        let sem_sinal_antes = self.sem_sinal_devido(local);
        let alterou = self
            .documento
            .definir_altura_e_acidente(id, altura, acidente.as_deref())?;
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

    fn conferir_limites(&self, altura: Altura) -> Result<(), ErroDeEdicao> {
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
    fn anteriores(
        &self,
        l: Local,
        excluir: usize,
        pauta: u8,
        ate: crate::partitura::Fracao,
    ) -> Vec<Anterior> {
        let comp = &self.partitura.partes[l.parte].compassos[l.compasso];
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
        let comp = &self.partitura.partes[l.parte].compassos[l.compasso];
        comp.notas
            .iter()
            .enumerate()
            .filter_map(|(i, n)| {
                let a = n.altura?;
                if n.acidente.is_some() {
                    return None;
                }
                let anteriores = self.anteriores(l, i, n.pauta, n.posicao);
                let vigor =
                    alteracao_em_vigor(l.quintas, de_partitura(a).passo, a.oitava, &anteriores);
                (a.alteracao != vigor).then_some(i)
            })
            .collect()
    }

    /// Reflete a edição no modelo de tocar, só na nota editada.
    fn atualizar_modelo_de_tocar(&mut self, l: Local, altura: Altura, acidente: Option<&str>) {
        let Some(pa) = para_partitura(altura) else {
            return;
        };
        let nota = &mut self.partitura.partes[l.parte].compassos[l.compasso].notas[l.ordinal];
        nota.altura = Some(pa);
        nota.midi = pa.numero_midi();
        nota.acidente = acidente.and_then(Acidente::de_texto);
    }
}
