//! Inserir nota ou pausa numa posição do compasso e remover nota (ticket 17, E01, seam S1), sem
//! janela e sem teclado. A tela só traduz uma tecla em [`ComandoDeEdicao::InserirNota`],
//! [`ComandoDeEdicao::InserirPausa`] ou [`ComandoDeEdicao::RemoverNota`] e chama
//! [`DocumentoAberto::aplicar`].
//!
//! # Contrato em linguagem de domínio
//!
//! - **A posição é a coluna do layout.** `posicao` é o início da nota no compasso, em semínimas
//!   (`Fracao` exata), o mesmo número que o modelo de tocar guarda em `Nota::posicao`; a voz e a
//!   pauta dizem em qual fio do compasso. Nada de pixel.
//! - **O compasso continua fechando.** A soma das durações de cada voz **nunca muda** por inserir
//!   ou remover:
//!   - **inserir nota** só vale sobre **pausas livres da mesma voz**, a partir de `posicao`: a nota
//!     ocupa o lugar delas, e o que sobrar de cada ponta continua pausa de duração exata (várias
//!     pausas, se o resto não for uma figura só), com o aviso [`AvisoDeEdicao::PausaCriada`].
//!     Onde há nota, onde não há tempo da voz, ou quando a figura passa das pausas livres, é
//!     recusado com [`ErroDeEdicao::SemEspacoParaInserir`] (mensagem em português, nada muda, o
//!     histórico não ganha entrada);
//!   - **inserir pausa** segue a mesma regra e regrupa as pausas livres. **Pausa em cima de nota
//!     é recusada** ([`ErroDeEdicao::PausaSobreNota`], decisão registrada no ticket 17): a nota só
//!     some por remoção explícita, que a transforma em pausa. Uma nova pausa idêntica à que já
//!     está ali não muda nada;
//!   - **remover nota** a transforma em **pausa de mesma duração, posição, voz e pauta** (a nota
//!     mantém o `IdNota`; nenhuma outra nota se move). Remover a última nota de uma voz deixa a
//!     voz com pausas até completar o compasso. Remover uma pausa é recusado
//!     ([`ErroDeEdicao::PausaNaoRemovivel`]). Nota de **acorde** que não é a última do acorde, e
//!     nota de **adorno**, simplesmente saem (o acorde sem ela continua soando; se a que saiu
//!     era a principal, a seguinte passa a ser a principal).
//! - **Quiáltera.** A nota inserida herda a quiáltera das pausas que ocupa (3:2, 5:4...), com
//!   `Fracao` exata; as pausas que sobram herdam a da pausa de que vieram. Pausas de quiálteras
//!   diferentes na mesma inserção são recusadas.
//! - **`divisions` não muda.** Duração que não é múltipla de `1/divisions` é
//!   [`ErroDeEdicao::DuracaoExigeMudarDivisions`] (nunca se arredonda).
//! - **Nada se perde em silêncio.** Quando a nota removida tinha **letra, dedilhado, ornamento,
//!   articulação ou ligadura**, o resultado traz um aviso para cada item
//!   ([`AvisoDeEdicao::LetraRemovida`], [`AvisoDeEdicao::DedilhadoRemovido`],
//!   [`AvisoDeEdicao::OrnamentoRemovido`], [`AvisoDeEdicao::LigaduraDeValorRemovida`],
//!   [`AvisoDeEdicao::LigaduraDeExpressaoRemovida`]). A marca de tupla (`<tuplet>`) fica, para
//!   o grupo não se desfazer. A cifra (`<harmony>`) é do compasso, não da nota: não some. Desfazer
//!   devolve tudo, byte a byte.
//! - **Atômico e só um compasso.** Tudo é calculado numa cópia do compasso; só depois de tudo
//!   certo o compasso (documento, modelo de tocar e identidades) é trocado, e só ele fica sujo.
//!   As vigas são conferidas como no ticket 16 ([`AvisoDeEdicao::VigasRefeitas`]). Se há outro
//!   elemento (direção, cifra, outra voz) **entre** as pausas que a inserção ocuparia, ela é
//!   recusada em vez de reordenar o compasso.
//! - **Identidade.** A nota e as pausas criadas ganham `IdNota` novo (monotônico, nunca
//!   reutilizado); as pausas ocupadas deixam de existir. O arquivo lateral de ids
//!   ([`DocumentoAberto::tabela_ids_para`]) guarda isso para a releitura.
//! - **Inverso.** O `inverso` de inserir nota é [`ComandoDeEdicao::RemoverNota`] e o de remover
//!   uma nota que vira pausa é [`ComandoDeEdicao::InserirNota`]. Nos demais casos (pausa,
//!   membro de acorde, adorno) o `inverso` é o próprio `aplicado` e quem desfaz é o histórico.
//!
//! # Fora de escopo
//!
//! Inserir nota **em acorde** (sobre outra nota), em compasso novo, fora de pausas ou criando voz
//! nova, e remover de uma vez uma seleção de várias notas.

use super::comando::{
    ALTURA_NEUTRA, AvisoDeEdicao, ComandoDeEdicao, ErroDeEdicao, MudancaDeEstrutura,
    ResultadoDaEdicao, acidente_devido, sem_sinal_devido_em,
};
use super::edicao::{leaf, posto_nota, validar};
use super::editar_duracao::{
    Aplicacao, Planejado, decompor, derivar_e_vigar, duracao_real, grupo_do_acorde, inconsistente,
    inserir_nota_antes, montar_no, no_da_nota, no_da_nota_mut, no_de_pausa, remover_nota, unidades,
    valor_atual,
};
use super::erro::ErroDocumento;
use super::grafia::alter_do_acidente;
use super::servico::DocumentoAberto;
use super::texto::{No, Peca, Texto};
use super::{Altura, Compasso, Documento, IdNota, Nota, PosCompasso};
use crate::partitura::{self, Figura, Fracao, Partitura};
use roxmltree::Node;
use std::ops::Range;

impl DocumentoAberto {
    /// Aplica um comando de inserir ou remover (sem registrar no histórico). Ver o contrato do
    /// módulo.
    pub(super) fn aplicar_estrutura(
        &mut self,
        comando: &ComandoDeEdicao,
    ) -> Result<ResultadoDaEdicao, ErroDeEdicao> {
        let planejado = match comando {
            ComandoDeEdicao::InserirNota {
                compasso,
                voz,
                pauta,
                posicao,
                altura,
                acidente,
                figura,
                pontos,
            } => {
                validar(*altura)?;
                self.conferir_limites(*altura)?;
                if let Some(t) = acidente
                    && alter_do_acidente(t) != Some(altura.alter)
                {
                    return Err(ErroDeEdicao::Documento(ErroDocumento::AlturaInvalida(
                        format!(
                            "o acidente «{t}» não corresponde à alteração {}",
                            altura.alter
                        ),
                    )));
                }
                planejar_insercao(
                    &self.documento,
                    &self.partitura,
                    &Pedido {
                        compasso,
                        voz,
                        pauta,
                        posicao: *posicao,
                        nota: Some((*altura, acidente.as_deref())),
                        figura: *figura,
                        pontos: *pontos,
                    },
                )?
            }
            ComandoDeEdicao::InserirPausa {
                compasso,
                voz,
                pauta,
                posicao,
                figura,
                pontos,
            } => planejar_insercao(
                &self.documento,
                &self.partitura,
                &Pedido {
                    compasso,
                    voz,
                    pauta,
                    posicao: *posicao,
                    nota: None,
                    figura: *figura,
                    pontos: *pontos,
                },
            )?,
            ComandoDeEdicao::RemoverNota { nota } => {
                planejar_remocao(&self.documento, &self.partitura, *nota)?
            }
            _ => return Err(inconsistente("comando que não é de estrutura")),
        };
        match planejado {
            Planejado::SemMudanca(r) => Ok(*r),
            Planejado::Muda(ap) => Ok(self.trocar_compasso(*ap)),
        }
    }
}

// --- Localização ------------------------------------------------------------------------------

/// O compasso alvo nos dois modelos.
pub(super) struct Alvo<'a> {
    pub(super) pd: usize,
    pub(super) ci: usize,
    pub(super) pt: usize,
    pub(super) comp0: &'a Compasso,
    pub(super) tocar0: &'a partitura::Compasso,
    pub(super) info: &'a partitura::InfoCompasso,
    /// Posição com o rótulo impresso do compasso.
    pub(super) pos: PosCompasso,
}

pub(super) fn alvo_por_indice<'a>(
    doc: &'a Documento,
    partitura: &'a Partitura,
    pd: usize,
    ci: usize,
) -> Result<Alvo<'a>, ErroDeEdicao> {
    let parte = &doc.partes[pd];
    let comp0 = &parte.compassos[ci];
    let pt = doc.partes[..pd]
        .iter()
        .filter(|p| !p.compassos.is_empty())
        .count();
    let info = partitura
        .compassos
        .get(ci)
        .ok_or_else(|| inconsistente("compasso sem informação no modelo de tocar"))?;
    let tocar0 = partitura
        .partes
        .get(pt)
        .and_then(|p| p.compassos.get(ci))
        .ok_or_else(|| inconsistente("o compasso não existe no modelo de tocar"))?;
    if tocar0.notas.len() != comp0.notas.len() {
        return Err(inconsistente(
            "o modelo de tocar e o documento discordam sobre as notas do compasso",
        ));
    }
    Ok(Alvo {
        pd,
        ci,
        pt,
        comp0,
        tocar0,
        info,
        pos: PosCompasso {
            parte: parte.id.clone(),
            indice: ci,
            numero: comp0.numero.clone(),
        },
    })
}

fn alvo_por_posicao<'a>(
    doc: &'a Documento,
    partitura: &'a Partitura,
    pos: &PosCompasso,
) -> Result<Alvo<'a>, ErroDeEdicao> {
    let invalida = |motivo: String| ErroDeEdicao::PosicaoInvalida {
        compasso: pos.clone(),
        motivo,
    };
    let pd = doc
        .partes
        .iter()
        .position(|p| p.id == pos.parte)
        .ok_or_else(|| invalida(format!("a parte «{}» não existe", pos.parte)))?;
    let parte = &doc.partes[pd];
    if pos.indice >= parte.compassos.len() {
        return Err(invalida(format!(
            "a parte tem {} compasso(s)",
            parte.compassos.len()
        )));
    }
    alvo_por_indice(doc, partitura, pd, pos.indice)
}

// --- Inserir ----------------------------------------------------------------------------------

struct Pedido<'a> {
    compasso: &'a PosCompasso,
    voz: &'a str,
    pauta: &'a str,
    posicao: Fracao,
    /// `None` = pausa; `Some((altura, acidente pedido))` = nota.
    nota: Option<(Altura, Option<&'a str>)>,
    figura: Figura,
    pontos: u8,
}

/// Um evento da voz no compasso (nota, acorde como uma coisa só, ou pausa).
struct Evento {
    /// Ordinal da nota (a principal, no caso do acorde).
    k: usize,
    inicio: Fracao,
    dur: Fracao,
    pausa: bool,
    tuplo: Option<(u32, u32)>,
}

impl Evento {
    fn fim(&self) -> Fracao {
        self.inicio + self.dur
    }
}

fn eventos_da_voz(a: &Alvo<'_>, voz: &str) -> Vec<Evento> {
    a.comp0
        .notas
        .iter()
        .enumerate()
        .filter(|(_, n)| !n.graca && !n.acorde && n.voz == voz)
        .filter_map(|(k, n)| {
            let t = &a.tocar0.notas[k];
            (t.duracao > Fracao::ZERO).then_some(Evento {
                k,
                inicio: t.posicao,
                dur: t.duracao,
                pausa: n.pausa,
                tuplo: t.tuplo,
            })
        })
        .collect()
}

/// `<pitch>` decomposto (os filhos são elementos editáveis, como os de uma nota lida).
fn no_de_altura(a: Altura) -> No {
    let mut filhos = vec![leaf("step", &a.passo.to_string())];
    if a.alter != 0 {
        filhos.push(leaf("alter", &a.alter.to_string()));
    }
    filhos.push(leaf("octave", &a.oitava.to_string()));
    let mut pecas = vec![Peca::Txt(Texto::Novo("<pitch>".to_owned()))];
    for f in filhos {
        pecas.push(Peca::Txt(Texto::Novo(String::new())));
        pecas.push(Peca::Elem(f));
    }
    pecas.push(Peca::Txt(Texto::Novo("</pitch>".to_owned())));
    No {
        nome: "pitch".to_owned(),
        pecas,
    }
}

/// `<note>` novo com altura, na ordem do esquema, copiando voz, quiáltera e pauta do `modelo`.
#[allow(clippy::too_many_arguments)] // os campos de uma nota; agrupar só esconderia
fn no_de_nota(
    modelo: &No,
    base: &str,
    altura: Altura,
    acidente: Option<&str>,
    (figura, pontos): (Figura, u8),
    unidades: i64,
    (voz, pauta): (&str, &str),
    quialtera: bool,
) -> No {
    let mut filhos = vec![
        no_de_altura(altura),
        leaf("duration", &unidades.to_string()),
    ];
    match modelo.filho("voice") {
        Some(v) => filhos.push(v.clone()),
        None if voz != "1" => filhos.push(leaf("voice", voz)),
        None => {}
    }
    filhos.push(leaf("type", figura.texto_xml()));
    for _ in 0..pontos {
        filhos.push(No::novo("dot", "<dot/>".to_owned()));
    }
    if let Some(a) = acidente {
        filhos.push(leaf("accidental", a));
    }
    if quialtera && let Some(t) = modelo.filho("time-modification") {
        filhos.push(t.clone());
    }
    if pauta != "1" || modelo.filho("staff").is_some() {
        filhos.push(leaf("staff", pauta));
    }
    montar_no(modelo, base, filhos)
}

pub(super) fn nova_nota(
    id: IdNota,
    altura: Option<Altura>,
    unidades: i64,
    divisions: i64,
    (voz, pauta): (&str, &str),
) -> Nota {
    Nota {
        id,
        id_xml: None,
        altura,
        duracao: Some(Fracao::nova(unidades, divisions)),
        voz: voz.to_owned(),
        pauta: pauta.to_owned(),
        acorde: false,
        pausa: altura.is_none(),
        graca: false,
        divisions,
        idx: 0,
        faixa: 0..0,
    }
}

#[allow(clippy::too_many_lines)] // um fluxo só: planejar tudo antes de tocar em qualquer coisa
fn planejar_insercao(
    doc: &Documento,
    partitura: &Partitura,
    pedido: &Pedido<'_>,
) -> Result<Planejado, ErroDeEdicao> {
    let a = alvo_por_posicao(doc, partitura, pedido.compasso)?;
    let base = doc.original.as_str();
    let (voz, pauta, p) = (pedido.voz, pedido.pauta, pedido.posicao);
    let invalida = |motivo: String| ErroDeEdicao::PosicaoInvalida {
        compasso: a.pos.clone(),
        motivo,
    };
    let sem_espaco = |motivo: String| ErroDeEdicao::SemEspacoParaInserir {
        compasso: a.pos.clone(),
        voz: voz.to_owned(),
        posicao: p,
        motivo,
    };
    let pauta_n: u8 = pauta
        .trim()
        .parse()
        .ok()
        .filter(|n| *n >= 1)
        .ok_or_else(|| invalida(format!("a pauta «{pauta}» não é um número")))?;
    let pautas = partitura.partes[a.pt].pautas.max(1);
    if pauta_n > pautas {
        return Err(invalida(format!(
            "a parte tem {pautas} pauta(s) e foi pedida a {pauta_n}"
        )));
    }
    if voz.trim().is_empty() {
        return Err(invalida("a voz está vazia".to_owned()));
    }
    if p < Fracao::ZERO || p >= a.info.duracao {
        return Err(invalida(format!(
            "o tempo {p} está fora do compasso, que comporta {} tempo(s) de semínima",
            a.info.duracao
        )));
    }

    // O espaço: eventos da voz, em ordem do tempo, sem sobreposição.
    let eventos = eventos_da_voz(&a, voz);
    for par in eventos.windows(2) {
        if par[1].inicio < par[0].fim() {
            return Err(sem_espaco(
                "as notas desta voz não seguem a ordem do tempo no arquivo".to_owned(),
            ));
        }
    }
    let Some(i0) = eventos.iter().position(|e| e.inicio <= p && p < e.fim()) else {
        return Err(sem_espaco(
            "a voz não ocupa esse tempo (lacuna ou fim da voz)".to_owned(),
        ));
    };
    let e0 = &eventos[i0];
    if !e0.pausa {
        return Err(if pedido.nota.is_none() {
            ErroDeEdicao::PausaSobreNota {
                compasso: a.pos.clone(),
                voz: voz.to_owned(),
                posicao: p,
            }
        } else {
            sem_espaco(
                "esse tempo é de uma nota (só se insere sobre pausa livre; remova a nota antes)"
                    .to_owned(),
            )
        });
    }
    let tuplo = e0.tuplo;
    let d_novo = duracao_real(pedido.figura, pedido.pontos, tuplo);
    let fim_novo = p + d_novo;
    let mut ultimo = i0;
    let mut coberto_ate = e0.fim();
    while coberto_ate < fim_novo {
        let Some(prox) = eventos.get(ultimo + 1) else {
            return Err(sem_espaco(format!(
                "a figura ({d_novo} de semínima) passa do fim da voz; cabem {} tempo(s) de semínima",
                coberto_ate - p
            )));
        };
        if !prox.pausa || prox.inicio != coberto_ate {
            return Err(sem_espaco(format!(
                "a figura ({d_novo} de semínima) passa das pausas livres; cabem {} tempo(s) de \
                 semínima",
                coberto_ate - p
            )));
        }
        if prox.tuplo != tuplo {
            return Err(sem_espaco(
                "as pausas ocupadas são de quiálteras diferentes".to_owned(),
            ));
        }
        ultimo += 1;
        coberto_ate = prox.fim();
    }
    let consumidas: Vec<&Evento> = eventos[i0..=ultimo].iter().collect();
    let ks: Vec<usize> = consumidas.iter().map(|e| e.k).collect();
    let divisions = a.comp0.notas[e0.k].divisions;
    if ks.iter().any(|&k| a.comp0.notas[k].divisions != divisions) {
        return Err(sem_espaco(
            "as pausas ocupadas têm `divisions` diferentes".to_owned(),
        ));
    }
    // Nada entre as pausas ocupadas além delas mesmas: senão a inserção reordenaria o compasso.
    if ks.len() > 1 {
        let (de, ate) = (
            a.comp0.notas[ks[0]].idx,
            a.comp0.notas[ks[ks.len() - 1]].idx,
        );
        let permitido: Vec<usize> = ks.iter().map(|&k| a.comp0.notas[k].idx).collect();
        for (i, peca) in a.comp0.no.pecas.iter().enumerate().take(ate).skip(de + 1) {
            if matches!(peca, Peca::Elem(_)) && !permitido.contains(&i) {
                return Err(sem_espaco(
                    "há outro elemento do compasso (outra nota, direção ou cifra) entre as pausas \
                     que seriam ocupadas"
                        .to_owned(),
                ));
            }
        }
    }

    // Os restos de cada ponta, em pausas de duração exata.
    let esq = p - e0.inicio;
    let dir = coberto_ate - fim_novo;
    let tuplo_dir = consumidas[consumidas.len() - 1].tuplo;
    let figuras_esq = decompor(esq, e0.tuplo).ok_or(ErroDeEdicao::PausaNaoRepresentavel(esq))?;
    let figuras_dir = decompor(dir, tuplo_dir).ok_or(ErroDeEdicao::PausaNaoRepresentavel(dir))?;
    let u_novo = unidades(d_novo, divisions)?;
    let mut un_esq = Vec::new();
    for (f, pt) in &figuras_esq {
        un_esq.push(unidades(duracao_real(*f, *pt, e0.tuplo), divisions)?);
    }
    let mut un_dir = Vec::new();
    for (f, pt) in &figuras_dir {
        un_dir.push(unidades(duracao_real(*f, *pt, tuplo_dir), divisions)?);
    }

    // Pausa sobre uma pausa idêntica: nada a fazer.
    let id_alvo_atual = a.comp0.notas[e0.k].id;
    if pedido.nota.is_none()
        && ks.len() == 1
        && esq == Fracao::ZERO
        && dir == Fracao::ZERO
        && !a.tocar0.notas[e0.k].pausa_de_compasso
        && valor_atual(no_da_nota(a.comp0, e0.k)?, base, e0.dur, e0.tuplo)
            == Some((pedido.figura, pedido.pontos))
    {
        let r = resultado_base(
            ComandoDeEdicao::InserirPausa {
                compasso: a.pos.clone(),
                voz: voz.to_owned(),
                pauta: pauta.to_owned(),
                posicao: p,
                figura: pedido.figura,
                pontos: pedido.pontos,
            },
            id_alvo_atual,
            ALTURA_NEUTRA,
            false,
        );
        let mut r = r;
        r.alterou = false;
        r.estrutura = None;
        return Ok(Planejado::SemMudanca(Box::new(r)));
    }

    // O acidente devido, quando a nota é alterada (ou o que o chamador pediu).
    let acidente = pedido
        .nota
        .and_then(|(alt, pedido_acidente)| match pedido_acidente {
            Some(t) => Some(t.to_owned()),
            None => acidente_devido(a.tocar0, a.info.armadura.quintas, alt, pauta_n, p),
        });

    // --- Edita uma cópia do compasso ---------------------------------------------------------
    let mut comp = a.comp0.clone();
    let primeiro_novo = doc.ids.proximo;
    let mut novos: u64 = 0;
    let mut novo_id = || {
        let id = IdNota(primeiro_novo + novos);
        novos += 1;
        id
    };
    let modelo_esq = no_da_nota(a.comp0, e0.k)?.clone();
    let modelo_dir = no_da_nota(a.comp0, consumidas[consumidas.len() - 1].k)?.clone();
    let mut ref_ord = e0.k;
    let mut inseridas = 0usize;
    let mut criadas: Vec<(IdNota, Figura, u8)> = Vec::new();
    for ((f, pt), u) in figuras_esq.iter().zip(&un_esq) {
        let id = novo_id();
        criadas.push((id, *f, *pt));
        let no = no_de_pausa(&modelo_esq, base, *f, *pt, *u, e0.tuplo.is_some());
        let n = nova_nota(
            id,
            None,
            *u,
            divisions,
            (voz, &comp.notas[ref_ord].pauta.clone()),
        );
        inserir_nota_antes(&mut comp, ref_ord, no, n);
        ref_ord += 1;
        inseridas += 1;
    }
    // A nota (ou pausa) pedida.
    let id_novo = novo_id();
    let alvo_altura = pedido.nota.map(|(alt, _)| alt);
    let no_novo = match pedido.nota {
        Some((alt, _)) => no_de_nota(
            &modelo_esq,
            base,
            alt,
            acidente.as_deref(),
            (pedido.figura, pedido.pontos),
            u_novo,
            (voz, pauta),
            tuplo.is_some(),
        ),
        None => {
            // Pausa pedida: na pauta pedida, com a figura pedida.
            let mut n = no_de_pausa(
                &modelo_esq,
                base,
                pedido.figura,
                pedido.pontos,
                u_novo,
                tuplo.is_some(),
            );
            if pauta != "1" || modelo_esq.filho("staff").is_some() {
                if let Some(s) = n.filho_mut("staff") {
                    s.definir_conteudo(base, pauta)?;
                } else {
                    n.inserir_em_ordem(leaf("staff", pauta), &posto_nota)?;
                }
            }
            n
        }
    };
    inserir_nota_antes(
        &mut comp,
        ref_ord,
        no_novo,
        nova_nota(id_novo, alvo_altura, u_novo, divisions, (voz, pauta)),
    );
    ref_ord += 1;
    inseridas += 1;
    for ((f, pt), u) in figuras_dir.iter().zip(&un_dir) {
        let id = novo_id();
        criadas.push((id, *f, *pt));
        let no = no_de_pausa(&modelo_dir, base, *f, *pt, *u, tuplo_dir.is_some());
        let n = nova_nota(
            id,
            None,
            *u,
            divisions,
            (voz, &comp.notas[ref_ord].pauta.clone()),
        );
        inserir_nota_antes(&mut comp, ref_ord, no, n);
        ref_ord += 1;
        inseridas += 1;
    }
    // As pausas ocupadas saem (de trás para frente, para os ordinais valerem).
    let removidas: Vec<IdNota> = ks.iter().map(|&k| a.comp0.notas[k].id).collect();
    for &k in ks.iter().rev() {
        remover_nota(&mut comp, k + inseridas);
    }
    comp.sujo = true;

    // --- Modelo de tocar e vigas -----------------------------------------------------------------
    let (tocar, avisos_vigas) = derivar_e_vigar(
        &mut comp,
        base,
        divisions,
        a.info,
        a.tocar0,
        (a.tocar0.notas[e0.k].pauta, a.tocar0.notas[e0.k].voz),
        &a.pos,
    )?;

    // --- Avisos e resultado ----------------------------------------------------------------------
    let aplicado = match pedido.nota {
        Some((alt, _)) => ComandoDeEdicao::InserirNota {
            compasso: a.pos.clone(),
            voz: voz.to_owned(),
            pauta: pauta.to_owned(),
            posicao: p,
            altura: alt,
            acidente: acidente.clone(),
            figura: pedido.figura,
            pontos: pedido.pontos,
        },
        None => ComandoDeEdicao::InserirPausa {
            compasso: a.pos.clone(),
            voz: voz.to_owned(),
            pauta: pauta.to_owned(),
            posicao: p,
            figura: pedido.figura,
            pontos: pedido.pontos,
        },
    };
    let mut r = resultado_base(
        aplicado,
        id_novo,
        alvo_altura.unwrap_or(ALTURA_NEUTRA),
        alvo_altura.is_some(),
    );
    if alvo_altura.is_some() {
        r.inverso = ComandoDeEdicao::RemoverNota { nota: id_novo };
    }
    for (pausa, figura, pontos) in &criadas {
        r.avisos.push(AvisoDeEdicao::PausaCriada {
            pausa: *pausa,
            figura: *figura,
            pontos: *pontos,
        });
    }
    if alvo_altura.is_some() {
        r.avisos
            .push(AvisoDeEdicao::PausasConsumidas { duracao: d_novo });
    }
    if tuplo.is_some() {
        r.avisos
            .push(AvisoDeEdicao::TuplaPodeTerFicadoIncompleta { nota: id_novo });
    }
    r.avisos.extend(avisos_vigas);
    r.avisos
        .extend(avisos_de_acidente(&a, &comp, &tocar, Some(id_novo)));
    if let Some(e) = r.estrutura.as_mut() {
        e.criadas = criadas.iter().map(|(i, _, _)| *i).collect();
        e.removidas = removidas;
    }
    let ids: Vec<IdNota> = comp.notas.iter().map(|n| n.id).collect();
    Ok(Planejado::Muda(Box::new(Aplicacao {
        pd: a.pd,
        ci: a.ci,
        pt: a.pt,
        comp,
        tocar,
        ids,
        novos,
        resultado: Box::new(r),
    })))
}

/// Resultado de estrutura ainda sem avisos nem criadas e removidas.
pub(super) fn resultado_base(
    aplicado: ComandoDeEdicao,
    alvo: IdNota,
    altura: Altura,
    soa: bool,
) -> ResultadoDaEdicao {
    ResultadoDaEdicao {
        inverso: aplicado.clone(),
        aplicado,
        nota: alvo,
        antes: altura,
        depois: altura,
        alterou: true,
        compassos_afetados: Vec::new(),
        avisos: Vec::new(),
        duracao: None,
        estrutura: Some(MudancaDeEstrutura {
            alvo,
            alvo_soa: soa,
            criadas: Vec::new(),
            removidas: Vec::new(),
        }),
    }
}

/// Notas que passaram a ficar sem o sinal devido (acidente) por causa da edição.
pub(super) fn avisos_de_acidente(
    a: &Alvo<'_>,
    comp: &Compasso,
    tocar: &partitura::Compasso,
    ignorar: Option<IdNota>,
) -> Vec<AvisoDeEdicao> {
    let quintas = a.info.armadura.quintas;
    let antes: Vec<IdNota> = sem_sinal_devido_em(a.tocar0, quintas)
        .into_iter()
        .map(|i| a.comp0.notas[i].id)
        .collect();
    sem_sinal_devido_em(tocar, quintas)
        .into_iter()
        .filter_map(|i| comp.notas.get(i).map(|n| n.id))
        .filter(|id| Some(*id) != ignorar && !antes.contains(id))
        .map(|nota| AvisoDeEdicao::AcidenteDeNotaSeguinteMudou { nota })
        .collect()
}

// --- Remover ----------------------------------------------------------------------------------

/// O que a nota removida tinha e vai sumir com ela.
#[derive(Default)]
struct Marcas {
    letras: Vec<String>,
    dedos: Vec<String>,
    ornamentos: Vec<String>,
    ligadura_de_valor: bool,
    ligadura_de_expressao: bool,
}

fn texto_dos_filhos(n: Node<'_, '_>) -> String {
    n.descendants()
        .filter(|d| d.is_text())
        .filter_map(|d| d.text())
        .collect::<String>()
        .trim()
        .to_owned()
}

fn analisar_marcas(texto_da_nota: &str) -> Result<Marcas, ErroDeEdicao> {
    let doc = crate::xml_seguro::analisar(texto_da_nota)
        .map_err(|e| ErroDeEdicao::Documento(ErroDocumento::from(e)))?;
    let mut m = Marcas::default();
    for c in doc.root_element().children().filter(Node::is_element) {
        match c.tag_name().name() {
            "lyric" => {
                let numero = c.attribute("number").unwrap_or("1");
                let texto: String = c
                    .children()
                    .filter(|t| t.is_element() && t.tag_name().name() == "text")
                    .map(texto_dos_filhos)
                    .collect();
                m.letras.push(format!("{numero}: {texto}"));
            }
            "tie" => m.ligadura_de_valor = true,
            "notations" => {
                for n in c.children().filter(Node::is_element) {
                    match n.tag_name().name() {
                        "tied" => m.ligadura_de_valor = true,
                        "slur" => m.ligadura_de_expressao = true,
                        "tuplet" => {}
                        "technical" => {
                            for t in n.children().filter(Node::is_element) {
                                if t.tag_name().name() == "fingering" {
                                    m.dedos.push(texto_dos_filhos(t));
                                } else {
                                    m.ornamentos.push(t.tag_name().name().to_owned());
                                }
                            }
                        }
                        "ornaments" | "articulations" => {
                            for t in n.children().filter(Node::is_element) {
                                m.ornamentos.push(t.tag_name().name().to_owned());
                            }
                        }
                        outro => m.ornamentos.push(outro.to_owned()),
                    }
                }
            }
            _ => {}
        }
    }
    Ok(m)
}

/// Tira de cada `<notations>` da nota tudo, menos as marcas de tupla (`<tuplet>`); o que ficar
/// vazio some.
fn limpar_notations(no: &mut No, base: &str) -> Result<(), ErroDeEdicao> {
    let mut i = 0;
    while i < no.pecas.len() {
        let eh_notations = matches!(&no.pecas[i], Peca::Elem(n) if n.nome == "notations");
        if !eh_notations {
            i += 1;
            continue;
        }
        let texto = match &no.pecas[i] {
            Peca::Elem(n) => n.texto(base),
            Peca::Txt(_) => String::new(),
        };
        let doc = crate::xml_seguro::analisar(&texto)
            .map_err(|e| ErroDeEdicao::Documento(ErroDocumento::from(e)))?;
        let mut cortes: Vec<Range<usize>> = Vec::new();
        let mut ficam = 0;
        for c in doc.root_element().children().filter(Node::is_element) {
            if c.tag_name().name() == "tuplet" {
                ficam += 1;
                continue;
            }
            let r = c.range();
            let mut ini = r.start;
            while ini > 0 && texto.as_bytes()[ini - 1].is_ascii_whitespace() {
                ini -= 1;
            }
            cortes.push(ini..r.end);
        }
        if ficam == 0 {
            // Some o elemento inteiro, com o espaço que o precedia.
            no.pecas.remove(i);
            no.pecas.remove(i - 1);
            i -= 1;
        } else {
            let mut novo = texto.clone();
            for r in cortes.into_iter().rev() {
                novo.replace_range(r, "");
            }
            no.pecas[i] = Peca::Elem(No::novo("notations", novo));
            i += 1;
        }
    }
    Ok(())
}

fn avisos_das_marcas(nota: IdNota, m: &Marcas) -> Vec<AvisoDeEdicao> {
    let mut v = Vec::new();
    for texto in &m.letras {
        v.push(AvisoDeEdicao::LetraRemovida {
            nota,
            texto: texto.clone(),
        });
    }
    for texto in &m.dedos {
        v.push(AvisoDeEdicao::DedilhadoRemovido {
            nota,
            texto: texto.clone(),
        });
    }
    for nome in &m.ornamentos {
        v.push(AvisoDeEdicao::OrnamentoRemovido {
            nota,
            nome: nome.clone(),
        });
    }
    if m.ligadura_de_valor {
        v.push(AvisoDeEdicao::LigaduraDeValorRemovida { nota });
    }
    if m.ligadura_de_expressao {
        v.push(AvisoDeEdicao::LigaduraDeExpressaoRemovida { nota });
    }
    v
}

pub(super) fn planejar_remocao(
    doc: &Documento,
    partitura: &Partitura,
    id: IdNota,
) -> Result<Planejado, ErroDeEdicao> {
    let &(pd, ci, ord) = doc
        .indice
        .get(&id)
        .ok_or(ErroDeEdicao::NotaInexistente(id))?;
    let a = alvo_por_indice(doc, partitura, pd, ci)?;
    let base = doc.original.as_str();
    let sel = &a.comp0.notas[ord];
    if sel.pausa {
        return Err(ErroDeEdicao::PausaNaoRemovivel(id));
    }
    let no0 = no_da_nota(a.comp0, ord)?;
    let marcas = analisar_marcas(&no0.texto(base))?;
    let (p0, q) = grupo_do_acorde(a.comp0, ord);
    let sai_sem_virar_pausa = sel.graca || q > p0;
    let mut comp = a.comp0.clone();
    let tocar_da_nota = &a.tocar0.notas[ord];
    let altura_antes = sel.altura;
    let mut removidas: Vec<IdNota> = Vec::new();
    let mut inverso: Option<ComandoDeEdicao> = None;

    if sai_sem_virar_pausa {
        // A nota some; se era a principal de um acorde, a seguinte passa a ser a principal.
        remover_nota(&mut comp, ord);
        if !sel.acorde && comp.notas.get(ord).is_some_and(|n| n.acorde) {
            no_da_nota_mut(&mut comp, ord)?.remover_filho("chord");
            comp.notas[ord].acorde = false;
        }
        removidas.push(id);
    } else {
        // Vira pausa da mesma duração, voz e pauta: a identidade fica com a pausa.
        if let (Some(altura), Some((figura, pontos))) = (
            sel.altura,
            sel.duracao
                .and_then(|d| valor_atual(no0, base, d, tocar_da_nota.tuplo)),
        ) {
            inverso = Some(ComandoDeEdicao::InserirNota {
                compasso: a.pos.clone(),
                voz: sel.voz.clone(),
                pauta: sel.pauta.clone(),
                posicao: tocar_da_nota.posicao,
                altura,
                acidente: doc.acidente_da_nota(id).ok().flatten(),
                figura,
                pontos,
            });
        }
        let no = no_da_nota_mut(&mut comp, ord)?;
        for nome in [
            "pitch",
            "unpitched",
            "accidental",
            "tie",
            "stem",
            "notehead",
            "notehead-text",
            "beam",
            "lyric",
            "play",
            "listen",
        ] {
            while no.remover_filho(nome) {}
        }
        limpar_notations(no, base)?;
        no.inserir_em_ordem(No::novo("rest", "<rest/>".to_owned()), &posto_nota)?;
        comp.notas[ord].pausa = true;
        comp.notas[ord].altura = None;
    }
    comp.sujo = true;

    let (tocar, avisos_vigas) = derivar_e_vigar(
        &mut comp,
        base,
        sel.divisions,
        a.info,
        a.tocar0,
        (tocar_da_nota.pauta, tocar_da_nota.voz),
        &a.pos,
    )?;

    let aplicado = ComandoDeEdicao::RemoverNota { nota: id };
    let altura = altura_antes.unwrap_or(ALTURA_NEUTRA);
    let mut r = resultado_base(aplicado, id, altura, false);
    if let Some(i) = inverso {
        r.inverso = i;
    }
    r.avisos.extend(avisos_das_marcas(id, &marcas));
    r.avisos.extend(avisos_vigas);
    r.avisos.extend(avisos_de_acidente(&a, &comp, &tocar, None));
    if let Some(e) = r.estrutura.as_mut() {
        e.removidas = removidas;
    }
    let ids: Vec<IdNota> = comp.notas.iter().map(|n| n.id).collect();
    Ok(Planejado::Muda(Box::new(Aplicacao {
        pd: a.pd,
        ci: a.ci,
        pt: a.pt,
        comp,
        tocar,
        ids,
        novos: 0,
        resultado: Box::new(r),
    })))
}
