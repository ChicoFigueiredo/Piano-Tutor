//! Mudar a duração de uma nota com validação do compasso (ticket 16, E01, seam S1), sem janela e
//! sem teclado. A tela só traduz `1` a `8` e `.` em [`ComandoDeEdicao::MudarDuracao`] e
//! [`ComandoDeEdicao::AlternarPonto`] e chama [`DocumentoAberto::aplicar`].
//!
//! # Contrato em linguagem de domínio
//!
//! - **O compasso continua fechando.** A soma das durações de cada voz do compasso (com
//!   `Fracao` exata, sem notas de acorde nem de adorno) **nunca muda** por uma edição de
//!   duração. Por isso:
//!   - **encurtar** a nota deixa um espaço, que o serviço preenche com **pausa da duração exata**
//!     logo depois dela (várias pausas, se o espaço não for uma figura só) e **avisa**
//!     ([`AvisoDeEdicao::PausaCriada`]). Nunca por arredondamento; nunca em silêncio;
//!   - **alongar** a nota só é possível sobre **pausas** que vêm logo depois dela na mesma voz:
//!     elas são consumidas (com [`AvisoDeEdicao::PausasConsumidas`]) e o que sobrar delas
//!     continua pausa exata. Faltando pausa, a edição é recusada: com
//!     [`ErroDeEdicao::CompassoNaoFecha`] se a voz passaria do que o compasso comporta (A4), ou
//!     com [`ErroDeEdicao::SemPausaParaAlongar`] se a voz estava incompleta.
//! - **Compasso incompleto sinalizado (anacruse)** continua válido: o que ele comporta é a sua
//!   duração de hoje (o serviço não muda o tamanho de nenhum compasso, então o início dos
//!   compassos seguintes não se mexe). Encurtar vale e cria a pausa; alongar vale só sobre pausa.
//! - **Tupla.** A nota mantém a sua quiáltera: a duração real é a nominal vezes `normais/reais`
//!   (3:2, 5:4...), em `Fracao` exata. As pausas de preenchimento herdam a quiáltera. O serviço
//!   avisa ([`AvisoDeEdicao::TuplaPodeTerFicadoIncompleta`]); não mexe nas marcas de tupla.
//! - **`divisions` não muda.** Valor que não é múltiplo de `1/divisions` (3 de uma tercina e 5 de
//!   uma quiáltera de cinco cabem; uma colcheia em `divisions = 3` não) é erro explícito
//!   ([`ErroDeEdicao::DuracaoExigeMudarDivisions`]), nunca arredondamento (decisão do supervisor:
//!   mudar `divisions` é recusado por padrão).
//! - **Acorde.** A duração vale para todas as notas do acorde; qualquer uma delas pode ser a
//!   selecionada.
//! - **Ligadura de valor, vigas.** A ligadura não é mexida (aviso
//!   [`AvisoDeEdicao::LigaduraDeValorMudouDuracao`]). As vigas da voz são conferidas: se a edição
//!   deixaria um grupo aberto, uma pausa vigada ou viga demais para a figura, o serviço as
//!   **refaz** (grupos por pulso, as mesmas regras da leitura) e avisa
//!   ([`AvisoDeEdicao::VigasRefeitas`]); vigas que já eram inconsistentes antes ficam como estão.
//! - **Atômico.** Tudo é calculado numa cópia do compasso; só depois de tudo certo o compasso
//!   (documento, modelo de tocar e identidades) é trocado. Em erro, nada muda e o histórico não
//!   ganha entrada. Só o compasso da nota é sujo. O modelo de tocar editado é igual ao que sairia
//!   de reabrir o texto exportado (conferido em teste no corpus).
//! - **Identidade.** A nota editada mantém o `IdNota`. Pausas criadas ganham `IdNota` novo
//!   (monotônico, nunca reutilizado); pausas consumidas deixam de existir. Pausa só parcialmente
//!   consumida é substituída por pausas novas com o resto. O arquivo lateral de ids (ticket 17,
//!   [`DocumentoAberto::tabela_ids_para`]) guarda quem é quem para a releitura, e a conferência da
//!   exportação confere as identidades por inteiro.
//!
//! # Fora de escopo
//!
//! Mudar a duração de uma pausa (a seleção só alcança notas), a longa (tecla 9, ausente do
//! modelo de partitura) e mudar `divisions` com confirmação do usuário.

use super::comando::{AvisoDeEdicao, ComandoDeEdicao, ErroDeEdicao, ResultadoDaEdicao};
use super::duracao::para_divisions;
use super::edicao::{leaf, posto_nota};
use super::erro::ErroDocumento;
use super::servico::DocumentoAberto;
use super::texto::{No, Peca, Texto};
use super::{Compasso, Documento, IdNota, Nota, PosCompasso};
use crate::partitura::{self, EstadoViga, Figura, Fracao, Partitura};

/// Figura, pontos e duração real (em semínimas) de uma nota.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValorDeDuracao {
    pub figura: Figura,
    pub pontos: u8,
    /// Duração real em semínimas (já com a quiáltera, se houver).
    pub duracao: Fracao,
}

/// O que um comando de duração mudou.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MudancaDeDuracao {
    pub antes: ValorDeDuracao,
    pub depois: ValorDeDuracao,
    /// Pausas que nasceram para preencher o espaço (na ordem do compasso).
    pub pausas_criadas: Vec<IdNota>,
    /// Pausas que deixaram de existir porque a nota ocupou o lugar delas.
    pub pausas_removidas: Vec<IdNota>,
}

/// Resultado do cálculo, antes de trocar o compasso.
pub(super) enum Planejado {
    /// Nada a mudar (a nota já estava assim).
    SemMudanca(Box<ResultadoDaEdicao>),
    Muda(Box<Aplicacao>),
}

/// O compasso novo, completo, pronto para trocar.
pub(super) struct Aplicacao {
    pub(super) pd: usize,
    pub(super) ci: usize,
    pub(super) pt: usize,
    pub(super) comp: Compasso,
    pub(super) tocar: partitura::Compasso,
    pub(super) ids: Vec<IdNota>,
    /// Quantos ids novos foram usados (o contador anda tantos).
    pub(super) novos: u64,
    pub(super) resultado: Box<ResultadoDaEdicao>,
}

pub(super) fn inconsistente(m: &str) -> ErroDeEdicao {
    ErroDeEdicao::Documento(ErroDocumento::FaixaInconsistente(m.to_owned()))
}

impl DocumentoAberto {
    /// Aplica um comando de duração (sem registrar no histórico). Ver o contrato do módulo.
    pub(super) fn aplicar_duracao(
        &mut self,
        comando: &ComandoDeEdicao,
    ) -> Result<ResultadoDaEdicao, ErroDeEdicao> {
        let ap = match planejar(&self.documento, &self.partitura, comando)? {
            Planejado::SemMudanca(r) => return Ok(*r),
            Planejado::Muda(ap) => *ap,
        };
        Ok(self.trocar_compasso(ap))
    }

    /// Tudo certo: troca o compasso (documento, índice, modelo de tocar e identidades) pelo
    /// planejado. Só aqui o documento muda, e a troca não falha.
    pub(super) fn trocar_compasso(&mut self, ap: Aplicacao) -> ResultadoDaEdicao {
        let Aplicacao {
            pd,
            ci,
            pt,
            comp,
            tocar,
            ids,
            novos,
            mut resultado,
        } = ap;
        let antigas: Vec<IdNota> = self.documento.partes[pd].compassos[ci]
            .notas
            .iter()
            .map(|n| n.id)
            .collect();
        for id in antigas {
            self.documento.indice.remove(&id);
        }
        for (i, n) in comp.notas.iter().enumerate() {
            self.documento.indice.insert(n.id, (pd, ci, i));
        }
        let numero = comp.numero.clone();
        self.documento.partes[pd].compassos[ci] = comp;
        self.documento.ids.proximo += novos;
        self.partitura.partes[pt].compassos[ci] = tocar;
        self.ids.definir_compasso(pt, ci, ids);
        resultado.compassos_afetados.push(PosCompasso {
            parte: self.documento.partes[pd].id.clone(),
            indice: ci,
            numero,
        });
        *resultado
    }
}

/// Valor escrito da nota (figura e pontos) como o arquivo diz; se não houver `<type>`
/// reconhecível, deduz da duração (desfazendo a quiáltera).
pub(super) fn valor_atual(
    no: &No,
    base: &str,
    duracao: Fracao,
    tuplo: Option<(u32, u32)>,
) -> Option<(Figura, u8)> {
    let tipo = no
        .filho("type")
        .and_then(|t| t.conteudo_folha(base))
        .and_then(|t| Figura::de_texto(t.trim()));
    if let Some(f) = tipo {
        let pontos = no.filhos().filter(|(_, n)| n.nome == "dot").count().min(3);
        return Some((f, u8::try_from(pontos).unwrap_or(3)));
    }
    let nominal = match tuplo {
        Some((a, n)) => duracao.vezes(i64::from(a), i64::from(n)),
        None => duracao,
    };
    Figura::por_duracao(nominal)
}

/// Duração real: nominal com pontos, vezes `normais/reais` da quiáltera.
pub(super) fn duracao_real(figura: Figura, pontos: u8, tuplo: Option<(u32, u32)>) -> Fracao {
    let nominal = figura.com_pontos(pontos);
    match tuplo {
        Some((a, n)) => nominal.vezes(i64::from(n), i64::from(a)),
        None => nominal,
    }
}

/// Unidades de `1/divisions`; erro explícito se não for inteiro (nunca arredonda).
pub(super) fn unidades(valor: Fracao, divisions: i64) -> Result<i64, ErroDeEdicao> {
    para_divisions(valor, divisions).map_err(|e| match e {
        ErroDocumento::DuracaoNaoRepresentavel {
            valor,
            divisions,
            necessarias,
        } => ErroDeEdicao::DuracaoExigeMudarDivisions {
            valor,
            divisions,
            necessarias,
        },
        outro => ErroDeEdicao::Documento(outro),
    })
}

/// Parte `real` em figuras (com no máximo um ponto), as maiores primeiro. As figuras são
/// nominais (a quiáltera, se houver, é aplicada por quem escreve a pausa). `None` se o espaço não
/// se escreve com figuras.
pub(super) fn decompor(real: Fracao, tuplo: Option<(u32, u32)>) -> Option<Vec<(Figura, u8)>> {
    let mut resto = match tuplo {
        Some((a, n)) => real.vezes(i64::from(a), i64::from(n)),
        None => real,
    };
    let mut v = Vec::new();
    while resto > Fracao::ZERO {
        let mut melhor: Option<(Figura, u8, Fracao)> = None;
        for f in Figura::TODAS {
            for pontos in 0..=1u8 {
                let valor = f.com_pontos(pontos);
                if valor <= resto && melhor.is_none_or(|(_, _, m)| valor > m) {
                    melhor = Some((f, pontos, valor));
                }
            }
        }
        let (f, p, valor) = melhor?;
        v.push((f, p));
        resto = resto - valor;
        if v.len() > 16 {
            return None;
        }
    }
    Some(v)
}

pub(super) fn texto_da_peca(p: &Peca, base: &str) -> String {
    match p {
        Peca::Txt(Texto::Original(r)) => base[r.clone()].to_owned(),
        Peca::Txt(Texto::Novo(s)) => s.clone(),
        Peca::Elem(n) => n.texto(base),
    }
}

/// `<note>` novo, já decomposto em peças (para ser editável depois), com a formatação do `modelo`
/// (o espaço entre os filhos e o fecho).
pub(super) fn montar_no(modelo: &No, base: &str, filhos: Vec<No>) -> No {
    let espaco = modelo.pecas.get(1).map(|p| texto_da_peca(p, base));
    let fecho = modelo
        .pecas
        .last()
        .map_or_else(|| "</note>".to_owned(), |p| texto_da_peca(p, base));
    let espaco = espaco.filter(|e| e.trim().is_empty()).unwrap_or_default();
    let mut pecas = vec![Peca::Txt(Texto::Novo("<note>".to_owned()))];
    for f in filhos {
        pecas.push(Peca::Txt(Texto::Novo(espaco.clone())));
        pecas.push(Peca::Elem(f));
    }
    pecas.push(Peca::Txt(Texto::Novo(fecho)));
    No {
        nome: "note".to_owned(),
        pecas,
    }
}

/// Pausa nova: filhos na ordem do esquema (`rest`, `duration`, `voice`, `type`, `dot`...,
/// `time-modification`, `staff`), copiando voz, pauta e quiáltera da nota `modelo`.
pub(super) fn no_de_pausa(
    modelo: &No,
    base: &str,
    figura: Figura,
    pontos: u8,
    unidades: i64,
    quialtera: bool,
) -> No {
    let mut filhos = vec![
        No::novo("rest", "<rest/>".to_owned()),
        leaf("duration", &unidades.to_string()),
    ];
    if let Some(v) = modelo.filho("voice") {
        filhos.push(v.clone());
    }
    filhos.push(leaf("type", figura.texto_xml()));
    for _ in 0..pontos {
        filhos.push(No::novo("dot", "<dot/>".to_owned()));
    }
    if quialtera && let Some(t) = modelo.filho("time-modification") {
        filhos.push(t.clone());
    }
    if let Some(s) = modelo.filho("staff") {
        filhos.push(s.clone());
    }
    montar_no(modelo, base, filhos)
}

pub(super) fn no_da_nota(comp: &Compasso, ordinal: usize) -> Result<&No, ErroDeEdicao> {
    match comp.no.pecas.get(comp.notas[ordinal].idx) {
        Some(Peca::Elem(e)) => Ok(e),
        _ => Err(inconsistente("peça da nota")),
    }
}

pub(super) fn no_da_nota_mut(comp: &mut Compasso, ordinal: usize) -> Result<&mut No, ErroDeEdicao> {
    let idx = comp.notas[ordinal].idx;
    match comp.no.pecas.get_mut(idx) {
        Some(Peca::Elem(e)) => Ok(e),
        _ => Err(inconsistente("peça da nota")),
    }
}

/// Põe o novo `<note>` logo depois da nota `ordinal_ref`; devolve o ordinal da nova.
pub(super) fn inserir_nota_apos(
    comp: &mut Compasso,
    ordinal_ref: usize,
    no: No,
    nota: Nota,
) -> usize {
    let idx_ref = comp.notas[ordinal_ref].idx;
    let espaco = match comp.no.pecas.get(idx_ref.wrapping_sub(1)) {
        Some(Peca::Txt(t)) => t.clone(),
        _ => Texto::Novo(String::new()),
    };
    comp.no.pecas.insert(idx_ref + 1, Peca::Txt(espaco));
    comp.no.pecas.insert(idx_ref + 2, Peca::Elem(no));
    for n in &mut comp.notas {
        if n.idx > idx_ref {
            n.idx += 2;
        }
    }
    let mut nota = nota;
    nota.idx = idx_ref + 2;
    comp.notas.insert(ordinal_ref + 1, nota);
    ordinal_ref + 1
}

/// Põe o novo `<note>` logo **antes** da nota `ordinal_ref` (que passa a ter o ordinal seguinte);
/// devolve o ordinal da nova, que é o `ordinal_ref` de antes.
pub(super) fn inserir_nota_antes(
    comp: &mut Compasso,
    ordinal_ref: usize,
    no: No,
    nota: Nota,
) -> usize {
    let idx_ref = comp.notas[ordinal_ref].idx;
    let espaco = match comp.no.pecas.get(idx_ref.wrapping_sub(1)) {
        Some(Peca::Txt(t)) => t.clone(),
        _ => Texto::Novo(String::new()),
    };
    comp.no.pecas.insert(idx_ref, Peca::Elem(no));
    comp.no.pecas.insert(idx_ref + 1, Peca::Txt(espaco));
    for n in &mut comp.notas {
        if n.idx >= idx_ref {
            n.idx += 2;
        }
    }
    let mut nota = nota;
    nota.idx = idx_ref;
    comp.notas.insert(ordinal_ref, nota);
    ordinal_ref
}

/// Tira o `<note>` (com o espaço que o precedia).
pub(super) fn remover_nota(comp: &mut Compasso, ordinal: usize) {
    let idx = comp.notas[ordinal].idx;
    comp.no.pecas.remove(idx);
    comp.no.pecas.remove(idx - 1);
    comp.notas.remove(ordinal);
    for n in &mut comp.notas {
        if n.idx > idx {
            n.idx -= 2;
        }
    }
}

/// Há `<backup>` ou `<forward>` entre as peças `de` e `ate` (exclusive)?
pub(super) fn ha_salto_entre(comp: &Compasso, de: usize, ate: usize) -> bool {
    comp.no.pecas[de + 1..ate]
        .iter()
        .any(|p| matches!(p, Peca::Elem(e) if e.nome == "backup" || e.nome == "forward"))
}

/// Soma das durações da voz no compasso (notas fora de acorde e de adorno, mais os
/// `<forward>` dela), em semínimas.
fn total_da_voz(comp: &Compasso, voz: &str, divisions: i64, base: &str) -> Fracao {
    let mut total = Fracao::ZERO;
    let mut atual = String::new();
    let mut k = 0;
    for p in &comp.no.pecas {
        let Peca::Elem(e) = p else { continue };
        match e.nome.as_str() {
            "note" => {
                let n = &comp.notas[k];
                k += 1;
                atual.clone_from(&n.voz);
                if n.voz == voz
                    && !n.acorde
                    && !n.graca
                    && let Some(d) = n.duracao
                {
                    total = total + d;
                }
            }
            "forward" => {
                let da_voz = e
                    .filho("voice")
                    .and_then(|v| v.conteudo_folha(base))
                    .map_or_else(|| atual.clone(), |v| v.trim().to_owned());
                if da_voz == voz
                    && let Some(u) = e
                        .filho("duration")
                        .and_then(|d| d.conteudo_folha(base))
                        .and_then(|d| d.trim().parse::<i64>().ok())
                {
                    total = total + Fracao::nova(u, divisions);
                }
            }
            _ => {}
        }
    }
    total
}

/// Grupo de notas que soam juntas (acorde) em torno de `ordinal`: primeira e última.
pub(super) fn grupo_do_acorde(comp: &Compasso, ordinal: usize) -> (usize, usize) {
    let mut p0 = ordinal;
    while p0 > 0 && comp.notas[p0].acorde {
        p0 -= 1;
    }
    let mut q = p0;
    while q + 1 < comp.notas.len() && comp.notas[q + 1].acorde {
        q += 1;
    }
    (p0, q)
}

/// Vigas de cada nota da voz formam grupos fechados, só em nota com bandeirola e nunca em pausa?
pub(super) fn vigas_consistentes(comp: &partitura::Compasso, pauta: u8, voz: u8) -> bool {
    let notas: Vec<_> = comp
        .notas
        .iter()
        .filter(|n| n.pauta == pauta && n.voz == voz && !n.acorde && !n.adorno)
        .collect();
    if notas.iter().any(|n| {
        (!n.vigas.is_empty() && n.e_pausa()) || n.vigas.len() > usize::from(n.figura.bandeirolas())
    }) {
        return false;
    }
    let niveis = notas.iter().map(|n| n.vigas.len()).max().unwrap_or(0);
    for nivel in 0..niveis {
        let mut aberto = false;
        for n in &notas {
            match n.vigas.get(nivel) {
                Some(EstadoViga::Inicio) => {
                    if aberto {
                        return false;
                    }
                    aberto = true;
                }
                Some(EstadoViga::Continua) => {
                    if !aberto {
                        return false;
                    }
                }
                Some(EstadoViga::Fim) => {
                    if !aberto {
                        return false;
                    }
                    aberto = false;
                }
                Some(EstadoViga::GanchoAtras | EstadoViga::GanchoFrente) => {}
                None => {
                    if aberto {
                        return false;
                    }
                }
            }
        }
        if aberto {
            return false;
        }
    }
    true
}

pub(super) fn texto_da_viga(e: EstadoViga) -> &'static str {
    match e {
        EstadoViga::Inicio => "begin",
        EstadoViga::Continua => "continue",
        EstadoViga::Fim => "end",
        EstadoViga::GanchoAtras => "backward hook",
        EstadoViga::GanchoFrente => "forward hook",
    }
}

/// O modelo de tocar do compasso, relido do texto que o compasso do documento produziria.
pub(super) fn derivar(
    comp: &Compasso,
    base: &str,
    divisions: i64,
    info: &partitura::InfoCompasso,
) -> Result<partitura::Compasso, ErroDeEdicao> {
    let mut texto = String::new();
    comp.no.escrever(base, &mut texto);
    let tocar = partitura::reler_compasso(&texto, divisions, info.armadura, info.forma)
        .map_err(|e| ErroDeEdicao::Documento(ErroDocumento::Xml(e.to_string())))?;
    if tocar.notas.len() != comp.notas.len() {
        return Err(inconsistente(
            "o modelo de tocar e o documento discordam sobre as notas do compasso",
        ));
    }
    Ok(tocar)
}

/// Relê o modelo de tocar do compasso editado e confere as vigas da voz `(pauta, voz)` que o
/// comando mexeu: arquivo sem nenhum `<beam>` ganha as vigas automáticas por pulso (como a
/// leitura); em arquivo com vigas, se a voz era consistente e deixou de ser (grupo aberto, pausa
/// vigada, viga demais para a figura), as vigas da voz são **refeitas** (grupos por pulso, as
/// mesmas regras da leitura) e escritas no texto, com o aviso [`AvisoDeEdicao::VigasRefeitas`].
/// Vigas que já eram inconsistentes antes ficam como estão.
pub(super) fn derivar_e_vigar(
    comp: &mut Compasso,
    base: &str,
    divisions: i64,
    info: &partitura::InfoCompasso,
    tocar0: &partitura::Compasso,
    (pauta, voz_n): (u8, u8),
    pos: &PosCompasso,
) -> Result<(partitura::Compasso, Vec<AvisoDeEdicao>), ErroDeEdicao> {
    let pulso = info.forma.duracao_do_pulso();
    // Arquivo sem nenhum `<beam>`: a leitura vigia sozinha por pulso (o modelo de tocar já traz
    // essas vigas automáticas, então a decisão olha o texto, não o modelo).
    let sem_vigas_no_arquivo = !base.contains("<beam");
    let mut tocar = derivar(comp, base, divisions, info)?;
    let mut avisos: Vec<AvisoDeEdicao> = Vec::new();
    if sem_vigas_no_arquivo {
        vigar_tudo(&mut tocar, pulso);
    } else if vigas_consistentes(tocar0, pauta, voz_n) && !vigas_consistentes(&tocar, pauta, voz_n)
    {
        for n in &mut tocar.notas {
            if n.pauta == pauta && n.voz == voz_n && !n.adorno {
                n.vigas.clear();
            }
        }
        if pulso.numerador() > 0 {
            partitura::vigar_voz(&mut tocar, pulso, pauta, voz_n);
        }
        for i in 0..comp.notas.len() {
            let (np, nv, ad) = (
                tocar.notas[i].pauta,
                tocar.notas[i].voz,
                tocar.notas[i].adorno,
            );
            if np != pauta || nv != voz_n || ad {
                continue;
            }
            let estados = tocar.notas[i].vigas.clone();
            let no = no_da_nota_mut(comp, i)?;
            while no.remover_filho("beam") {}
            for (nivel, e) in estados.iter().enumerate() {
                let t = format!(
                    "<beam number=\"{}\">{}</beam>",
                    nivel + 1,
                    texto_da_viga(*e)
                );
                no.inserir_em_ordem(No::novo("beam", t), &posto_nota)?;
            }
        }
        tocar = derivar(comp, base, divisions, info)?;
        avisos.push(AvisoDeEdicao::VigasRefeitas {
            compasso: pos.clone(),
        });
    }
    Ok((tocar, avisos))
}

/// Vigas automáticas por pulso em todas as vozes (só nos arquivos sem nenhuma viga).
pub(super) fn vigar_tudo(tocar: &mut partitura::Compasso, pulso: Fracao) {
    if pulso.numerador() <= 0 {
        return;
    }
    let mut chaves: Vec<(u8, u8)> = tocar.notas.iter().map(|n| (n.pauta, n.voz)).collect();
    chaves.sort_unstable();
    chaves.dedup();
    for (pauta, voz) in chaves {
        partitura::vigar_voz(tocar, pulso, pauta, voz);
    }
}

/// Uma pausa que a nota ocupa (inteira ou em parte).
struct Consumo {
    id: IdNota,
    /// Quanto da pausa a nota toma.
    toma: Fracao,
    /// Quiáltera da própria pausa.
    tuplo: Option<(u32, u32)>,
}

#[allow(clippy::too_many_lines)] // um fluxo só: planejar tudo antes de tocar em qualquer coisa
fn planejar(
    doc: &Documento,
    partitura: &Partitura,
    comando: &ComandoDeEdicao,
) -> Result<Planejado, ErroDeEdicao> {
    let id = comando
        .nota()
        .ok_or_else(|| inconsistente("comando de duração sem nota"))?;
    let &(pd, ci, ord) = doc
        .indice
        .get(&id)
        .ok_or(ErroDeEdicao::NotaInexistente(id))?;
    let pt = doc.partes[..pd]
        .iter()
        .filter(|p| !p.compassos.is_empty())
        .count();
    let base = doc.original.as_str();
    let comp0 = &doc.partes[pd].compassos[ci];
    let info = partitura
        .compassos
        .get(ci)
        .ok_or_else(|| inconsistente("compasso sem informação no modelo de tocar"))?;
    let tocar0 = partitura
        .partes
        .get(pt)
        .and_then(|p| p.compassos.get(ci))
        .ok_or_else(|| inconsistente("a nota não existe no modelo de tocar"))?;
    if tocar0.notas.len() != comp0.notas.len() {
        return Err(inconsistente(
            "o modelo de tocar e o documento discordam sobre as notas do compasso",
        ));
    }

    // A nota e o acorde dela.
    let sel = &comp0.notas[ord];
    if sel.graca {
        return Err(ErroDeEdicao::NotaDeGracaSemDuracao(id));
    }
    if sel.pausa || sel.altura.is_none() {
        return Err(ErroDeEdicao::PausaNaoEditavel(id));
    }
    let (p0, q) = grupo_do_acorde(comp0, ord);
    if comp0.notas[p0].acorde {
        return Err(ErroDeEdicao::ValorEscritoDesconhecido(id));
    }
    for n in &comp0.notas[p0..=q] {
        if n.altura.is_none() || n.pausa || n.graca || n.duracao.is_none() {
            return Err(ErroDeEdicao::PausaNaoEditavel(id));
        }
    }
    let principal = &comp0.notas[p0];
    let divisions = principal.divisions;
    let voz = principal.voz.clone();
    let d_antes = principal
        .duracao
        .ok_or(ErroDeEdicao::ValorEscritoDesconhecido(id))?;
    let no_p = no_da_nota(comp0, p0)?;
    let tuplo = tocar0.notas[p0].tuplo;
    let (fig_antes, pontos_antes) = valor_atual(no_p, base, d_antes, tuplo)
        .ok_or(ErroDeEdicao::ValorEscritoDesconhecido(id))?;
    let antes = ValorDeDuracao {
        figura: fig_antes,
        pontos: pontos_antes,
        duracao: d_antes,
    };

    // O valor pedido, na forma resolvida.
    let (figura, pontos) = match comando {
        ComandoDeEdicao::MudarDuracao { figura, .. } => (*figura, 0),
        ComandoDeEdicao::AlternarPonto { .. } => (fig_antes, u8::from(pontos_antes == 0)),
        ComandoDeEdicao::DefinirDuracao { figura, pontos, .. } => (*figura, *pontos),
        _ => return Err(inconsistente("comando que não é de duração")),
    };
    if pontos > 3 {
        return Err(ErroDeEdicao::Documento(ErroDocumento::DuracaoInvalida(
            format!("{pontos} pontos de aumento (o máximo é 3)"),
        )));
    }
    let d_novo = duracao_real(figura, pontos, tuplo);
    let depois = ValorDeDuracao {
        figura,
        pontos,
        duracao: d_novo,
    };
    let altura = sel.altura.ok_or(ErroDeEdicao::PausaNaoEditavel(id))?;
    let mut resultado = ResultadoDaEdicao {
        aplicado: ComandoDeEdicao::DefinirDuracao {
            nota: id,
            figura,
            pontos,
        },
        inverso: ComandoDeEdicao::DefinirDuracao {
            nota: id,
            figura: fig_antes,
            pontos: pontos_antes,
        },
        nota: id,
        antes: altura,
        depois: altura,
        alterou: false,
        compassos_afetados: Vec::new(),
        avisos: Vec::new(),
        duracao: Some(MudancaDeDuracao {
            antes,
            depois,
            pausas_criadas: Vec::new(),
            pausas_removidas: Vec::new(),
        }),
        estrutura: None,
    };
    if figura == fig_antes && pontos == pontos_antes && d_novo == d_antes {
        return Ok(Planejado::SemMudanca(Box::new(resultado)));
    }
    let pos = PosCompasso {
        parte: doc.partes[pd].id.clone(),
        indice: ci,
        numero: comp0.numero.clone(),
    };
    let limite = info.duracao;
    let total = total_da_voz(comp0, &voz, divisions, base);

    // O que fazer com o espaço: preencher (encurtar) ou tomar de pausas (alongar).
    let mut a_criar: Vec<(Figura, u8)> = Vec::new();
    let mut a_consumir: Vec<Consumo> = Vec::new();
    if d_novo < d_antes {
        let sobra = d_antes - d_novo;
        let figuras = decompor(sobra, tuplo).ok_or(ErroDeEdicao::PausaNaoRepresentavel(sobra))?;
        for (f, p) in figuras {
            a_criar.push((f, p));
        }
    } else if d_novo > d_antes {
        let falta = d_novo - d_antes;
        let mut tomado = Fracao::ZERO;
        let mut anterior = comp0.notas[q].idx;
        for k in q + 1..comp0.notas.len() {
            let n = &comp0.notas[k];
            if tomado >= falta || ha_salto_entre(comp0, anterior, n.idx) {
                break;
            }
            anterior = n.idx;
            if n.graca {
                continue;
            }
            let (Some(dr), true) = (n.duracao, n.pausa && !n.acorde && n.voz == voz) else {
                break;
            };
            if n.pauta != principal.pauta {
                break;
            }
            let toma = dr.min(falta - tomado);
            tomado = tomado + toma;
            a_consumir.push(Consumo {
                id: n.id,
                toma,
                tuplo: tocar0.notas[k].tuplo,
            });
        }
        let liquido = falta - tomado;
        if liquido > Fracao::ZERO {
            let ficaria = total + liquido;
            return Err(if ficaria > limite {
                ErroDeEdicao::CompassoNaoFecha {
                    compasso: pos,
                    voz,
                    ficaria,
                    esperado: limite,
                }
            } else {
                ErroDeEdicao::SemPausaParaAlongar {
                    nota: id,
                    faltam: liquido,
                }
            });
        }
    }

    let u_novo = unidades(d_novo, divisions)?;

    // --- Edita uma cópia do compasso ---------------------------------------------------------
    let mut comp = comp0.clone();
    let mut novos: u64 = 0;
    let primeiro_novo = doc.ids.proximo;
    let mut novo_id = || {
        let id = IdNota(primeiro_novo + novos);
        novos += 1;
        id
    };
    for k in p0..=q {
        let no = no_da_nota_mut(&mut comp, k)?;
        no.filho_mut("duration")
            .ok_or_else(|| {
                ErroDeEdicao::Documento(ErroDocumento::DuracaoInvalida("sem <duration>".into()))
            })?
            .definir_conteudo(base, &u_novo.to_string())?;
        match no.filho_mut("type") {
            Some(t) => t.definir_conteudo(base, figura.texto_xml())?,
            None => no.inserir_em_ordem(leaf("type", figura.texto_xml()), &posto_nota)?,
        }
        let existentes = no.filhos().filter(|(_, n)| n.nome == "dot").count();
        let querem = usize::from(pontos);
        for _ in querem..existentes {
            no.remover_filho("dot");
        }
        for _ in existentes..querem {
            no.inserir_em_ordem(No::novo("dot", "<dot/>".to_owned()), &posto_nota)?;
        }
        comp.notas[k].duracao = Some(Fracao::nova(u_novo, divisions));
    }

    let mut criadas: Vec<(IdNota, Figura, u8)> = Vec::new();
    let mut removidas: Vec<IdNota> = Vec::new();
    let modelo = no_da_nota(&comp, p0)?.clone();
    let nova_pausa = |comp: &mut Compasso,
                      apos: usize,
                      f: Figura,
                      p: u8,
                      u: i64,
                      quialtera: bool,
                      id: IdNota| {
        let no = no_de_pausa(&modelo, base, f, p, u, quialtera);
        let nota = Nota {
            id,
            id_xml: None,
            altura: None,
            duracao: Some(Fracao::nova(u, divisions)),
            voz: comp.notas[apos].voz.clone(),
            pauta: comp.notas[apos].pauta.clone(),
            acorde: false,
            pausa: true,
            graca: false,
            divisions,
            idx: 0,
            faixa: 0..0,
        };
        inserir_nota_apos(comp, apos, no, nota)
    };
    // Encurtar: as pausas exatas entram logo depois do acorde.
    let mut depois_de = q;
    for (f, p) in a_criar {
        let u = unidades(duracao_real(f, p, tuplo), divisions)?;
        let nid = novo_id();
        criadas.push((nid, f, p));
        depois_de = nova_pausa(&mut comp, depois_de, f, p, u, tuplo.is_some(), nid);
    }
    // Alongar: as pausas tomadas somem (ou ficam com o resto, também em pausas exatas).
    for c in &a_consumir {
        let k = comp
            .notas
            .iter()
            .position(|n| n.id == c.id)
            .ok_or_else(|| inconsistente("pausa a consumir sumiu"))?;
        let dr = comp.notas[k]
            .duracao
            .ok_or_else(|| inconsistente("pausa sem duração"))?;
        let resto = dr - c.toma;
        if resto > Fracao::ZERO {
            let tuplo_da_pausa = c.tuplo;
            let figuras = decompor(resto, tuplo_da_pausa)
                .ok_or(ErroDeEdicao::PausaNaoRepresentavel(resto))?;
            let mut apos = k;
            let modelo_da_pausa = no_da_nota(&comp, k)?.clone();
            for (f, p) in figuras {
                let u = unidades(duracao_real(f, p, tuplo_da_pausa), divisions)?;
                let nid = novo_id();
                let no = no_de_pausa(&modelo_da_pausa, base, f, p, u, tuplo_da_pausa.is_some());
                let nota = Nota {
                    id: nid,
                    id_xml: None,
                    altura: None,
                    duracao: Some(Fracao::nova(u, divisions)),
                    voz: comp.notas[k].voz.clone(),
                    pauta: comp.notas[k].pauta.clone(),
                    acorde: false,
                    pausa: true,
                    graca: false,
                    divisions,
                    idx: 0,
                    faixa: 0..0,
                };
                apos = inserir_nota_apos(&mut comp, apos, no, nota);
            }
        }
        remover_nota(&mut comp, k);
        removidas.push(c.id);
    }
    comp.sujo = true;

    // --- Modelo de tocar e vigas -----------------------------------------------------------------
    let (pauta, voz_n) = (tocar0.notas[p0].pauta, tocar0.notas[p0].voz);
    let (tocar, avisos) = derivar_e_vigar(
        &mut comp,
        base,
        divisions,
        info,
        tocar0,
        (pauta, voz_n),
        &pos,
    )?;

    // --- Avisos e resultado ----------------------------------------------------------------------
    for (pausa, figura, pontos) in &criadas {
        resultado.avisos.push(AvisoDeEdicao::PausaCriada {
            pausa: *pausa,
            figura: *figura,
            pontos: *pontos,
        });
    }
    if !a_consumir.is_empty() {
        let duracao = a_consumir.iter().fold(Fracao::ZERO, |s, c| s + c.toma);
        resultado
            .avisos
            .push(AvisoDeEdicao::PausasConsumidas { duracao });
    }
    for k in p0..=q {
        if doc.tem_ligadura_de_valor(comp0.notas[k].id)? {
            resultado
                .avisos
                .push(AvisoDeEdicao::LigaduraDeValorMudouDuracao { nota: id });
            break;
        }
    }
    if tuplo.is_some() {
        resultado
            .avisos
            .push(AvisoDeEdicao::TuplaPodeTerFicadoIncompleta { nota: id });
    }
    resultado.avisos.extend(avisos);
    if let Some(m) = resultado.duracao.as_mut() {
        m.pausas_criadas = criadas.iter().map(|(i, _, _)| *i).collect();
        m.pausas_removidas = removidas;
    }
    resultado.alterou = true;
    let ids: Vec<IdNota> = comp.notas.iter().map(|n| n.id).collect();
    Ok(Planejado::Muda(Box::new(Aplicacao {
        pd,
        ci,
        pt,
        comp,
        tocar,
        ids,
        novos,
        resultado: Box::new(resultado),
    })))
}
