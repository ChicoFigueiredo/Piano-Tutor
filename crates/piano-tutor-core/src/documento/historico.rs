//! Transação e pilha de desfazer/refazer do serviço de documento (ticket 15, E01, seam S1), sem
//! janela e sem teclado. A tela só traduz Ctrl+Z e Ctrl+Shift+Z em [`DocumentoAberto::desfazer`]
//! e [`DocumentoAberto::refazer`].
//!
//! # Contrato em linguagem de domínio
//!
//! - **Uma edição que mudou algo é uma transação.** [`DocumentoAberto::aplicar`] guarda o
//!   **instantâneo do compasso** afetado (o compasso do documento com a origem por faixa e o bit
//!   sujo, o compasso do modelo de tocar e as identidades das notas) **antes** de editar. Desfazer
//!   troca o compasso de volta pelo instantâneo e guarda o que estava lá para refazer; os dois
//!   caminhos são, portanto, a mesma troca. Como o compasso volta inteiro (inclusive o bit sujo),
//!   desfazer tudo exporta o arquivo original **byte a byte** (critério A5).
//! - **Recusa não deixa rastro.** Comando recusado, ou que não muda nada, não cria entrada, não
//!   descarta o ramo de refazer e não mexe em mais nada (pré-requisito do critério A4).
//! - **Editar depois de desfazer descarta o ramo de refazer.**
//! - **Gesto = grupo.** [`DocumentoAberto::iniciar_grupo`] e [`DocumentoAberto::encerrar_grupo`]
//!   juntam as edições bem-sucedidas entre os dois numa única entrada: um desfazer as desfaz
//!   todas. Grupo sem edição não cria entrada. Dentro de um grupo aberto não se desfaz nem refaz.
//! - **Limite de memória.** O histórico tem um orçamento em bytes
//!   ([`LIMITE_PADRAO_DO_HISTORICO`], 16 MiB, mudável por
//!   [`DocumentoAberto::definir_limite_do_historico`]). É uma **estimativa** do tamanho dos
//!   instantâneos guardados (o texto novo dos compassos, as notas e as identidades), não uma
//!   medida do alocador. Passando do limite, a edição **mais antiga** sai da pilha de desfazer, com
//!   o [`AvisoDeEdicao::EdicaoMaisAntigaDescartada`] no resultado da edição que provocou o
//!   descarte; as demais continuam desfazendo normalmente (nunca há corrupção: só se perde a
//!   possibilidade de voltar mais longe). A entrada mais recente fica sempre, mesmo que sozinha
//!   passe do limite: a última edição sempre pode ser desfeita.
//! - **Modificado.** [`DocumentoAberto::modificado`] diz se o documento difere do **estado
//!   salvo** ([`DocumentoAberto::marcar_salvo`]; ao abrir, o estado salvo é o aberto). Desfazer
//!   ou refazer até o estado salvo o deixa limpo; se o estado salvo saiu da pilha (ramo de refazer
//!   descartado ou entrada mais antiga descartada), nunca mais fica limpo por engano.
//! - **Seleção.** O resultado de desfazer/refazer traz as notas tocadas e o `foco`: a nota da
//!   primeira edição da transação. A tela reancora a seleção por [`IdNota`] (a identidade da nota
//!   não muda ao desfazer).
//!
//! # Fora de escopo
//!
//! Lote atômico entre compassos distantes e diff musical completo ficam para a E06; o
//! instantâneo por compasso é o preço de simplicidade desta fase.

use super::servico::DocumentoAberto;
use super::{
    AvisoDeEdicao, ComandoDeEdicao, Compasso, ErroDeEdicao, IdNota, PosCompasso, ResultadoDaEdicao,
    texto,
};
use crate::partitura;
use std::collections::VecDeque;
use std::mem::size_of;

/// Orçamento padrão do histórico: 16 MiB de instantâneos.
pub const LIMITE_PADRAO_DO_HISTORICO: usize = 16 * 1024 * 1024;

/// O que desfazer ou refazer mexeu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultadoDoHistorico {
    /// Compassos que a troca devolveu a outro estado.
    pub compassos_afetados: Vec<PosCompasso>,
    /// Notas editadas pela transação, na ordem das edições, sem repetição.
    pub notas: Vec<IdNota>,
    /// A nota a selecionar: a da primeira edição da transação.
    pub foco: Option<IdNota>,
}

/// Compasso guardado: o do documento, o do modelo de tocar e as identidades das notas.
#[derive(Clone, Debug)]
struct Instantaneo {
    parte: usize,
    compasso: usize,
    doc: Compasso,
    /// Parte e compasso do modelo de tocar (a parte sem compasso não entra no modelo).
    tocar: Option<(usize, partitura::Compasso)>,
    ids: Vec<IdNota>,
}

impl Instantaneo {
    /// Estimativa do tamanho em memória, em bytes.
    fn bytes(&self) -> usize {
        let doc = size_of::<Compasso>()
            + self.doc.numero.len()
            + bytes_do_no(&self.doc.no)
            + self
                .doc
                .notas
                .iter()
                .map(|n| {
                    size_of::<super::Nota>()
                        + n.id_xml.as_ref().map_or(0, String::len)
                        + n.voz.len()
                        + n.pauta.len()
                })
                .sum::<usize>();
        let tocar = self.tocar.as_ref().map_or(0, |(_, c)| {
            size_of::<partitura::Compasso>()
                + c.notas.len() * size_of::<partitura::Nota>()
                + c.claves.len() * size_of::<partitura::MudancaClave>()
        });
        size_of::<Self>() + doc + tocar + self.ids.len() * size_of::<IdNota>()
    }
}

fn bytes_do_no(no: &texto::No) -> usize {
    no.nome.len()
        + no.pecas
            .iter()
            .map(|p| {
                size_of::<texto::Peca>()
                    + match p {
                        texto::Peca::Txt(texto::Texto::Novo(s)) => s.len(),
                        texto::Peca::Txt(texto::Texto::Original(_)) => 0,
                        texto::Peca::Elem(n) => bytes_do_no(n),
                    }
            })
            .sum::<usize>()
}

/// Uma entrada da pilha: os instantâneos dos compassos que o gesto mexeu.
#[derive(Clone, Debug)]
struct Transacao {
    instantaneos: Vec<Instantaneo>,
    notas: Vec<IdNota>,
    estado_antes: u64,
    estado_depois: u64,
    bytes: usize,
}

impl Transacao {
    fn recalcular_bytes(&mut self) {
        self.bytes = self.instantaneos.iter().map(Instantaneo::bytes).sum();
    }
}

/// Pilhas de desfazer e refazer, o gesto aberto e o controle de estado salvo.
#[derive(Clone, Debug)]
pub(super) struct Historico {
    desfazer: VecDeque<Transacao>,
    refazer: Vec<Transacao>,
    /// Gesto aberto (entre `iniciar_grupo` e `encerrar_grupo`).
    grupo: Option<Transacao>,
    profundidade_do_grupo: usize,
    limite: usize,
    descartadas: usize,
    /// Rótulo do estado atual do documento; cada edição ganha um rótulo novo, desfazer e refazer
    /// voltam ao rótulo guardado na transação.
    estado: u64,
    proximo_estado: u64,
    salvo: Option<u64>,
}

impl Default for Historico {
    fn default() -> Self {
        Self {
            desfazer: VecDeque::new(),
            refazer: Vec::new(),
            grupo: None,
            profundidade_do_grupo: 0,
            limite: LIMITE_PADRAO_DO_HISTORICO,
            descartadas: 0,
            estado: 0,
            proximo_estado: 1,
            salvo: Some(0),
        }
    }
}

impl Historico {
    fn bytes(&self) -> usize {
        self.desfazer.iter().map(|t| t.bytes).sum::<usize>()
            + self.refazer.iter().map(|t| t.bytes).sum::<usize>()
            + self.grupo.as_ref().map_or(0, |t| t.bytes)
    }

    fn novo_estado(&mut self) -> u64 {
        let e = self.proximo_estado;
        self.proximo_estado += 1;
        e
    }

    /// Tira da pilha de desfazer as mais antigas até caber no limite (a mais recente fica).
    /// Devolve quantas saíram.
    fn caber_no_limite(&mut self) -> usize {
        let mut fora = 0;
        while self.desfazer.len() > 1 && self.bytes() > self.limite {
            self.desfazer.pop_front();
            fora += 1;
        }
        self.descartadas += fora;
        fora
    }
}

impl DocumentoAberto {
    /// Aplica o comando como uma transação. Em erro ou sem mudança, nada muda e **não há
    /// entrada** no histórico. Em sucesso, a edição entra na pilha de desfazer (ou no grupo
    /// aberto) e o ramo de refazer é descartado. Ver o contrato do módulo.
    pub fn aplicar(&mut self, comando: ComandoDeEdicao) -> Result<ResultadoDaEdicao, ErroDeEdicao> {
        let nota = comando.nota();
        let antes = self.instantaneo_da_nota(nota);
        let mut resultado = self.aplicar_sem_historico(comando)?;
        if resultado.alterou {
            let fora = self.registrar(antes, resultado.nota);
            if fora > 0 {
                resultado
                    .avisos
                    .push(AvisoDeEdicao::EdicaoMaisAntigaDescartada { quantas: fora });
            }
        }
        Ok(resultado)
    }

    /// Há o que desfazer?
    pub fn pode_desfazer(&self) -> bool {
        !self.historico.desfazer.is_empty()
    }

    /// Há o que refazer?
    pub fn pode_refazer(&self) -> bool {
        !self.historico.refazer.is_empty()
    }

    /// Quantas entradas a pilha de desfazer tem.
    pub fn entradas_de_desfazer(&self) -> usize {
        self.historico.desfazer.len()
    }

    /// Quantas entradas a pilha de refazer tem.
    pub fn entradas_de_refazer(&self) -> usize {
        self.historico.refazer.len()
    }

    /// Desfaz a edição mais recente (ou o gesto inteiro). `None` se não há o que desfazer ou se
    /// há um grupo aberto.
    pub fn desfazer(&mut self) -> Option<ResultadoDoHistorico> {
        if self.historico.profundidade_do_grupo > 0 {
            return None;
        }
        let mut t = self.historico.desfazer.pop_back()?;
        let r = self.trocar(&mut t);
        self.historico.estado = t.estado_antes;
        self.historico.refazer.push(t);
        Some(r)
    }

    /// Refaz a edição desfeita mais recentemente. `None` se não há o que refazer ou se há um
    /// grupo aberto.
    pub fn refazer(&mut self) -> Option<ResultadoDoHistorico> {
        if self.historico.profundidade_do_grupo > 0 {
            return None;
        }
        let mut t = self.historico.refazer.pop()?;
        let r = self.trocar(&mut t);
        self.historico.estado = t.estado_depois;
        self.historico.desfazer.push_back(t);
        Some(r)
    }

    /// Começa um gesto: as edições até [`Self::encerrar_grupo`] viram uma única entrada. Grupos
    /// aninhados valem como um só (o mais externo manda).
    pub fn iniciar_grupo(&mut self) {
        self.historico.profundidade_do_grupo += 1;
    }

    /// Fecha o gesto. Devolve os avisos do histórico (descarte da edição mais antiga por
    /// limite). Grupo sem edição não cria entrada.
    pub fn encerrar_grupo(&mut self) -> Vec<AvisoDeEdicao> {
        let h = &mut self.historico;
        h.profundidade_do_grupo = h.profundidade_do_grupo.saturating_sub(1);
        if h.profundidade_do_grupo > 0 {
            return Vec::new();
        }
        let Some(t) = h.grupo.take() else {
            return Vec::new();
        };
        h.desfazer.push_back(t);
        match h.caber_no_limite() {
            0 => Vec::new(),
            quantas => vec![AvisoDeEdicao::EdicaoMaisAntigaDescartada { quantas }],
        }
    }

    /// O documento difere do estado salvo? Abrir é o estado salvo inicial; desfazer ou refazer
    /// até ele deixa limpo.
    pub fn modificado(&self) -> bool {
        self.historico.salvo != Some(self.historico.estado)
    }

    /// Registra o estado de agora como o salvo (quem grava o arquivo chama depois de gravar).
    pub fn marcar_salvo(&mut self) {
        self.historico.salvo = Some(self.historico.estado);
    }

    /// Orçamento do histórico, em bytes (estimativa; ver o contrato do módulo).
    pub fn limite_do_historico(&self) -> usize {
        self.historico.limite
    }

    /// Muda o orçamento e já descarta as edições mais antigas que não cabem. Devolve quantas.
    pub fn definir_limite_do_historico(&mut self, bytes: usize) -> usize {
        self.historico.limite = bytes;
        self.historico.caber_no_limite()
    }

    /// Tamanho estimado dos instantâneos guardados, em bytes.
    pub fn bytes_do_historico(&self) -> usize {
        self.historico.bytes()
    }

    /// Quantas edições saíram da pilha de desfazer por falta de espaço desde a abertura.
    pub fn edicoes_descartadas(&self) -> usize {
        self.historico.descartadas
    }

    // --- por dentro ---

    /// Guarda a transação da edição que acabou de mudar o documento. `antes` é o instantâneo
    /// tirado antes da edição; `None` quando o grupo aberto já guardou aquele compasso.
    /// Devolve quantas entradas antigas saíram por limite.
    fn registrar(&mut self, antes: Option<Instantaneo>, nota: IdNota) -> usize {
        let h = &mut self.historico;
        let estado_antes = h.estado;
        h.estado = h.novo_estado();
        h.refazer.clear();
        let t = h.grupo.get_or_insert_with(|| Transacao {
            instantaneos: Vec::new(),
            notas: Vec::new(),
            estado_antes,
            estado_depois: 0,
            bytes: 0,
        });
        if let Some(a) = antes
            && !t
                .instantaneos
                .iter()
                .any(|i| (i.parte, i.compasso) == (a.parte, a.compasso))
        {
            t.instantaneos.push(a);
        }
        if !t.notas.contains(&nota) {
            t.notas.push(nota);
        }
        t.estado_depois = h.estado;
        t.recalcular_bytes();
        if h.profundidade_do_grupo > 0 {
            return 0;
        }
        let Some(t) = h.grupo.take() else { return 0 };
        h.desfazer.push_back(t);
        h.caber_no_limite()
    }

    /// Instantâneo do compasso da nota, a menos que o grupo aberto já o tenha.
    fn instantaneo_da_nota(&self, nota: IdNota) -> Option<Instantaneo> {
        let &(parte, compasso, _) = self.documento.indice.get(&nota)?;
        let ja_guardado = self.historico.grupo.as_ref().is_some_and(|g| {
            g.instantaneos
                .iter()
                .any(|i| (i.parte, i.compasso) == (parte, compasso))
        });
        if ja_guardado {
            return None;
        }
        let doc = self.documento.partes.get(parte)?.compassos.get(compasso)?;
        let parte_tocar = self.documento.partes[..parte]
            .iter()
            .filter(|p| !p.compassos.is_empty())
            .count();
        let tocar = self
            .partitura
            .partes
            .get(parte_tocar)
            .and_then(|p| p.compassos.get(compasso))
            .map(|c| (parte_tocar, c.clone()));
        let ids = tocar
            .as_ref()
            .map(|_| self.ids.compasso(parte_tocar, compasso))
            .unwrap_or_default();
        Some(Instantaneo {
            parte,
            compasso,
            doc: doc.clone(),
            tocar,
            ids,
        })
    }

    /// Troca cada compasso da transação pelo que está guardado, guardando no lugar o que estava
    /// no documento (a mesma troca serve para desfazer e para refazer).
    fn trocar(&mut self, t: &mut Transacao) -> ResultadoDoHistorico {
        let mut compassos_afetados = Vec::with_capacity(t.instantaneos.len());
        for inst in &mut t.instantaneos {
            let (p, c) = (inst.parte, inst.compasso);
            let atual = &mut self.documento.partes[p].compassos[c];
            for n in &atual.notas {
                self.documento.indice.remove(&n.id);
            }
            std::mem::swap(atual, &mut inst.doc);
            for (i, n) in self.documento.partes[p].compassos[c]
                .notas
                .iter()
                .enumerate()
            {
                self.documento.indice.insert(n.id, (p, c, i));
            }
            if let Some((pt, guardado)) = &mut inst.tocar {
                std::mem::swap(&mut self.partitura.partes[*pt].compassos[c], guardado);
                let atuais = self.ids.compasso(*pt, c);
                let guardados = std::mem::replace(&mut inst.ids, atuais);
                self.ids.definir_compasso(*pt, c, guardados);
            }
            compassos_afetados.push(PosCompasso {
                parte: self.documento.partes[p].id.clone(),
                indice: c,
                numero: self.documento.partes[p].compassos[c].numero.clone(),
            });
        }
        t.recalcular_bytes();
        ResultadoDoHistorico {
            compassos_afetados,
            foco: t.notas.first().copied(),
            notas: t.notas.clone(),
        }
    }
}
