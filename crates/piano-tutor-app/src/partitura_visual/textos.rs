//! Letra, cifra e dedilhado no layout (ticket 19, E01, D13: só mostrar e preservar).
//!
//! O núcleo entrega a [`Exibicao`] (cada texto já ligado à `IdNota` da sua nota). Aqui o layout:
//!
//! - **reserva espaço vertical por estrofe**: cada estrofe da pauta ganha uma linha própria
//!   abaixo da pauta, **depois** de tudo o que a pauta tem embaixo (notas com linhas
//!   suplementares, ligaduras, dedilhado); a altura reservada entra na altura da pauta e, por
//!   isso, empurra a pauta de baixo. É a lacuna 1 do spike (ticket 09): a letra de 2 ou 3 versos
//!   invadia a pauta seguinte;
//! - põe a **cifra** acima da pauta, acima também do número do compasso e das casas de repetição;
//! - põe o **dedilhado** do lado oposto da haste, com glifos SMuFL da Bravura (1 a 5);
//! - soma a **largura do texto** ao espaçamento horizontal, para sílabas e cifras não se tocarem.
//!
//! Sem `Exibicao` (ou sem identidades de nota) nada daqui entra em ação e o layout é exatamente
//! o de antes: o golden de primitivas do ticket 06 não muda.

use super::*;
use crate::geometria::Alinha;
use piano_tutor_core::documento::{TipoDeSilaba, ordenar_estrofes};

/// Distância vertical entre duas estrofes, em sp (texto de 1,3 sp de altura de algarismo).
pub const PASSO_DE_ESTROFE: f32 = 2.1;
/// Altura de algarismo da letra, da cifra e do dedilhado em texto, em sp.
pub const ALTURA_LETRA: f32 = 1.3;
pub const ALTURA_CIFRA: f32 = 1.55;
const ALTURA_DEDO_TEXTO: f32 = 1.2;
/// Largura média de um caractere em fração da altura de algarismo (estimativa para o espaçamento).
const LARGURA_POR_CARACTERE: f32 = 0.66;
/// Folga entre a pauta (ou a última tinta embaixo dela) e a primeira estrofe, em sp.
const FOLGA_DA_LETRA: f32 = 0.9;
/// Mínimo abaixo da pauta onde a primeira estrofe começa, em sp.
const LETRA_MINIMA_ABAIXO: f32 = 1.8;
/// Folga entre a cifra e o que está embaixo dela, em sp.
const FOLGA_DA_CIFRA: f32 = 0.5;
/// Distância do centro do dedilhado à cabeça, em sp, e passo entre dedos de um acorde.
const DISTANCIA_DO_DEDO: f32 = 1.7;
const PASSO_DO_DEDO: f32 = 1.25;

/// Uma sílaba para desenhar numa linha de estrofe.
pub(super) struct LetraL {
    pub k: usize,
    pub pos: Fracao,
    pub linha: usize,
    pub texto: String,
    pub hifen: bool,
    pub nota: IdNota,
}

pub(super) struct CifraL {
    pub k: usize,
    pub pos: Fracao,
    pub texto: String,
}

pub(super) struct DedoL {
    pub k: usize,
    pub pos: Fracao,
    pub texto: String,
    pub nota: IdNota,
}

/// Tudo o que um compasso desenha além das notas.
#[derive(Default)]
pub(super) struct TextosCompasso {
    pub letras: Vec<LetraL>,
    pub cifras: Vec<CifraL>,
    pub dedos: Vec<DedoL>,
}

fn largura_estimada(texto: &str, altura: f32) -> f32 {
    texto.chars().count() as f32 * altura * LARGURA_POR_CARACTERE
}

impl TextosCompasso {
    /// Largura de texto por posição do compasso, para o espaçamento (a mais larga de cada uma).
    pub fn larguras(&self) -> Vec<(Fracao, f32)> {
        let mut v: Vec<(Fracao, f32)> = Vec::new();
        let mut somar = |pos: Fracao, w: f32| match v.iter_mut().find(|(p, _)| *p == pos) {
            Some((_, m)) => *m = m.max(w),
            None => v.push((pos, w)),
        };
        for l in &self.letras {
            // O hífen que segue a sílaba ocupa mais ~1 sp à direita.
            somar(
                l.pos,
                largura_estimada(&l.texto, ALTURA_LETRA) + 2.0 * f32::from(l.hifen),
            );
        }
        for c in &self.cifras {
            somar(c.pos, largura_estimada(&c.texto, ALTURA_CIFRA));
        }
        v
    }
}

/// Estrofes de cada pauta, na ordem de exibição (a linha 0 é a primeira): fixas na peça inteira,
/// para que a estrofe 1 esteja sempre na mesma linha, em qualquer sistema.
pub(super) fn estrofes_das_pautas(
    p: &Partitura,
    ids: &IdsPorNota,
    exib: &Exibicao,
    pautas: &[PautaGlobal],
) -> Vec<Vec<String>> {
    pautas
        .iter()
        .map(|g| {
            let mut v: Vec<String> = Vec::new();
            for (m, c) in p.partes[g.parte].compassos.iter().enumerate() {
                for (ordinal, n) in c.notas.iter().enumerate() {
                    if n.pauta != g.pauta {
                        continue;
                    }
                    let Some(e) = ids.id(g.parte, m, ordinal).and_then(|id| exib.da_nota(id))
                    else {
                        continue;
                    };
                    for s in &e.silabas {
                        if !v.contains(&s.estrofe) {
                            v.push(s.estrofe.clone());
                        }
                    }
                }
            }
            ordenar_estrofes(&mut v);
            v
        })
        .collect()
}

/// Reúne letra, cifra e dedilhado do compasso `m`, por pauta global.
pub(super) fn coletar(
    p: &Partitura,
    ids: &IdsPorNota,
    exib: &Exibicao,
    pautas: &[PautaGlobal],
    estrofes: &[Vec<String>],
    m: usize,
) -> TextosCompasso {
    let mut t = TextosCompasso::default();
    for (k, g) in pautas.iter().enumerate() {
        let notas = &p.partes[g.parte].compassos[m].notas;
        for (ordinal, n) in notas.iter().enumerate() {
            if n.pauta != g.pauta {
                continue;
            }
            let Some(id) = ids.id(g.parte, m, ordinal) else {
                continue;
            };
            let Some(e) = exib.da_nota(id) else {
                continue;
            };
            for s in &e.silabas {
                let linha = estrofes[k]
                    .iter()
                    .position(|x| *x == s.estrofe)
                    .unwrap_or(0);
                t.letras.push(LetraL {
                    k,
                    pos: n.posicao,
                    linha,
                    texto: s.texto.clone(),
                    hifen: matches!(s.tipo, TipoDeSilaba::Inicio | TipoDeSilaba::Meio),
                    nota: id,
                });
            }
            if !e.dedos.is_empty() {
                t.dedos.push(DedoL {
                    k,
                    pos: n.posicao,
                    texto: e.dedilhado_em_texto(),
                    nota: id,
                });
            }
        }
    }
    for c in exib.cifras().iter().filter(|c| c.compasso == m) {
        // A cifra fica acima da primeira pauta da sua parte.
        let Some(k) = pautas.iter().position(|g| g.parte == c.parte) else {
            continue;
        };
        let pos = c
            .nota
            .and_then(|id| {
                p.partes[c.parte].compassos[m]
                    .notas
                    .iter()
                    .enumerate()
                    .find(|(o, _)| ids.id(c.parte, m, *o) == Some(id))
                    .map(|(_, n)| n.posicao)
            })
            .unwrap_or(Fracao::ZERO);
        t.cifras.push(CifraL {
            k,
            pos,
            texto: c.texto.clone(),
        });
    }
    t
}

/// Dedilhado da pauta `k` num compasso: itens (em y relativo ao topo da pauta) e quanto passam
/// acima e abaixo da pauta (sp positivos).
pub(super) fn itens_do_dedilhado(
    dedos: &[&DedoL],
    analise: &AnalisePauta,
    x_de: &dyn Fn(Fracao) -> f32,
) -> (Vec<Item>, f32, f32) {
    // (índice do evento, índice da nota no evento) de cada dedilhado, para empilhar os de acorde.
    let achados: Vec<Option<(usize, usize)>> = dedos
        .iter()
        .map(|d| {
            analise.eventos.iter().enumerate().find_map(|(ie, ev)| {
                ev.notas
                    .iter()
                    .position(|n| n.id == Some(d.nota))
                    .map(|i| (ie, i))
            })
        })
        .collect();
    let mut itens = Vec::new();
    let (mut acima, mut abaixo) = (0.0f32, 0.0f32);
    for (i, d) in dedos.iter().enumerate() {
        let xc = x_de(d.pos);
        let yc = match achados[i] {
            Some((ie, i_nota)) => {
                let ev = &analise.eventos[ie];
                // Notas do evento em ordem de altura (a mais grave primeiro). Haste para cima:
                // dedilhado embaixo, começando pela nota mais grave; haste para baixo: em cima,
                // começando pela mais aguda.
                let ordem = |a: usize, b: usize| if ev.cima { a < b } else { a > b };
                let mais_externas = achados
                    .iter()
                    .flatten()
                    .filter(|(e2, n2)| *e2 == ie && ordem(*n2, i_nota))
                    .count();
                let extremo = if ev.cima {
                    ev.notas.first()
                } else {
                    ev.notas.last()
                };
                let y_ext = y_de_s(extremo.map_or(4, |n| n.s));
                let desvio = DISTANCIA_DO_DEDO + mais_externas as f32 * PASSO_DO_DEDO;
                if ev.cima {
                    y_ext + desvio
                } else {
                    y_ext - desvio
                }
            }
            // Sem cabeça desenhada (identidade não achada): acima da pauta.
            None => -DISTANCIA_DO_DEDO,
        };
        let meia = bravura::ALTURA_DO_ALGARISMO * bravura::ESCALA_DEDILHADO / 2.0 + 0.1;
        acima = acima.max(-(yc - meia));
        abaixo = abaixo.max(yc + meia - ALTURA_PAUTA);
        itens.extend(desenho_do_dedilhado(d, xc, yc));
    }
    (itens, acima, abaixo)
}

/// Os dígitos `1` a `5` como glifos da Bravura (`1-2` com um traço entre eles); qualquer outro
/// texto (`p`, `i`, `m`...) como texto comum.
fn desenho_do_dedilhado(d: &DedoL, xc: f32, yc: f32) -> Vec<Item> {
    let glifos: Vec<char> = d
        .texto
        .split('-')
        .map_while(bravura::glifo_do_texto_de_dedo)
        .collect();
    let total = d.texto.split('-').count();
    let com_nota = |mut it: Item| {
        it.nota = Some(d.nota);
        it
    };
    if glifos.is_empty() || glifos.len() != total {
        return vec![com_nota(texto_item(
            ponto(xc, yc),
            &d.texto,
            ALTURA_DEDO_TEXTO,
            Ancora::CENTER_CENTER,
            Cor::Tinta,
        ))];
    }
    let esc = bravura::ESCALA_DEDILHADO;
    const TRACO: f32 = 0.45;
    const FOLGA: f32 = 0.15;
    let larguras: Vec<f32> = glifos
        .iter()
        .map(|c| bravura::avanco_do_glifo(*c) * esc)
        .collect();
    let largura_total: f32 =
        larguras.iter().sum::<f32>() + (glifos.len() - 1) as f32 * (TRACO + 2.0 * FOLGA);
    let base = yc + bravura::ALTURA_DO_ALGARISMO * esc / 2.0;
    let mut x = xc - largura_total / 2.0;
    let mut v = Vec::new();
    for (i, c) in glifos.iter().enumerate() {
        v.push(com_nota(item(Forma::Glifo {
            pos: ponto(x, base),
            caractere: *c,
            escala: esc,
        })));
        x += larguras[i];
        if i + 1 < glifos.len() {
            x += FOLGA;
            v.push(com_nota(linha(ponto(x, yc), ponto(x + TRACO, yc), 0.12)));
            x += TRACO + FOLGA;
        }
    }
    v
}

/// Letras da pauta `k` do sistema: itens em y relativo ao topo da pauta. `abaixo` é quanto a
/// pauta já ocupa abaixo da linha inferior (sp); devolve os itens e o novo `abaixo`.
pub(super) fn itens_da_letra(letras: &[(f32, &LetraL)], abaixo: f32) -> (Vec<Item>, f32) {
    if letras.is_empty() {
        return (Vec::new(), abaixo);
    }
    let linhas = letras
        .iter()
        .map(|(_, l)| l.linha)
        .max()
        .map_or(0, |m| m + 1);
    let topo = (ALTURA_PAUTA + LETRA_MINIMA_ABAIXO).max(ALTURA_PAUTA + abaixo + FOLGA_DA_LETRA);
    let mut itens = Vec::with_capacity(letras.len());
    for (x, l) in letras {
        let y = topo + l.linha as f32 * PASSO_DE_ESTROFE;
        let mut corpo = texto_item(
            ponto(*x, y),
            &l.texto,
            ALTURA_LETRA,
            Ancora([Alinha::Centro, Alinha::Min]),
            Cor::Tinta,
        );
        corpo.nota = Some(l.nota);
        itens.push(corpo);
        if l.hifen {
            let w = largura_estimada(&l.texto, ALTURA_LETRA);
            let mut h = texto_item(
                ponto(*x + w / 2.0 + 0.55, y),
                "-",
                ALTURA_LETRA,
                Ancora([Alinha::Centro, Alinha::Min]),
                Cor::Tinta,
            );
            h.nota = Some(l.nota);
            itens.push(h);
        }
    }
    let novo = topo - ALTURA_PAUTA + linhas as f32 * PASSO_DE_ESTROFE;
    (itens, abaixo.max(novo))
}

/// Cifras da pauta `k` do sistema: itens em y relativo ao topo da pauta. `acima` é quanto a pauta
/// já ocupa acima da linha superior (sp), com o mínimo `reserva` (número do compasso e casas);
/// devolve os itens e o novo `acima`.
pub(super) fn itens_da_cifra(
    cifras: &[(f32, &CifraL)],
    acima: f32,
    reserva: f32,
) -> (Vec<Item>, f32) {
    if cifras.is_empty() {
        return (Vec::new(), acima);
    }
    let base = acima.max(reserva) + FOLGA_DA_CIFRA;
    let itens = cifras
        .iter()
        .map(|(x, c)| {
            texto_item(
                ponto(*x, -base),
                &c.texto,
                ALTURA_CIFRA,
                Ancora::CENTER_BOTTOM,
                Cor::Tinta,
            )
        })
        .collect();
    (itens, acima.max(base + ALTURA_CIFRA * 1.3))
}
