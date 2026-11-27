//! Partitura na tela (F07, somente leitura): diagramação própria e desenho com o `Painter` do
//! egui, sem fonte musical. O layout usa só os tipos de [`crate::geometria`] (sem egui); o egui
//! entra apenas em `adaptador_egui`. Tudo é medido em **espaços de pauta** (sp; 1 sp = distância
//! entre duas linhas): a escala (zoom) só converte sp em pixels, de modo que a diagramação é
//! proporcional em qualquer tamanho. Regras de gravura em `docs/referencias/COMPONENTES-VISUAIS.md`:
//! linha de pauta 0,13 sp, haste 3,5 sp (0,12 sp de espessura), viga 0,5 sp com no máximo 1 sp
//! de desnível, linha suplementar 0,16 sp que avança 0,4 sp além da cabeça, espaçamento
//! horizontal pelo logaritmo da duração, coluna de acidentes e quebra de linha sempre em
//! compasso inteiro.
//!
//! Etapas: [`calcular_layout`] (puro, testável) gera [`Layout`] com sistemas e primitivas em sp;
//! [`desenhar_partitura`] pinta esse layout, destaca o compasso e as notas que estão soando e
//! acompanha a execução com rolagem. Mão direita âmbar, mão esquerda azul.

// Os laços por índice percorrem vetores paralelos (pautas, posições y, marcas): iterar um só
// deles deixaria o código menos claro.
#![allow(clippy::needless_range_loop)]

use crate::geometria::{Ancora, IdNota, Ponto, Retangulo, ponto, vetor};
use piano_tutor_core::documento::{Exibicao, IdsPorNota};
use piano_tutor_core::partitura::{
    Acidente, Altura, Armadura, Barra, Clave, Compasso, DirecaoHaste, EstadoViga, Figura, Fracao,
    InfoCompasso, MarcaLigadura, Nota, Partitura, SimboloForma, SinalClave,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Pixels por espaço de pauta com zoom 1,0 (100%).
pub const ESP_BASE_PX: f32 = 9.5;
/// Zoom mínimo e máximo aceitos.
pub const ZOOM_MIN: f32 = 0.5;
pub const ZOOM_MAX: f32 = 3.0;

// --- Constantes de gravura (em sp) ---------------------------------------------------------

pub const LINHA_PAUTA: f32 = 0.13;
pub const HASTE_ESPESSURA: f32 = 0.12;
pub const HASTE_COMPRIMENTO: f32 = 3.5;
pub const VIGA_ESPESSURA: f32 = 0.5;
pub const VIGA_ESPACO: f32 = 0.25;
pub const VIGA_DESNIVEL_MAX: f32 = 1.0;
pub const SUPLEMENTAR_ESPESSURA: f32 = 0.16;
pub const SUPLEMENTAR_EXTENSAO: f32 = 0.4;
pub const ALTURA_PAUTA: f32 = 4.0;

const MEIA_CABECA: f32 = 0.59;
const MEIA_SEMIBREVE: f32 = 0.84;
const MARGEM_ESQUERDA: f32 = 3.2;
const MARGEM_DIREITA: f32 = 1.0;
const LARGURA_MINIMA: f32 = 45.0;
const SEMINIMA_LARGURA: f32 = 4.8;
const POR_DOBRO: f32 = 1.5;
const ESTICAR_MAX: f32 = 1.9;
const ESTICAR_ULTIMO: f32 = 1.25;
/// Folga (sp) ao redor da região visível antes de descartar uma primitiva (textos largos).
const MARGEM_CORTE_SP: f32 = 8.0;
/// Fração da largura visível onde fica o cursor na fita contínua.
pub const FRACAO_CURSOR_FITA: f32 = 1.0 / 3.0;

// --- Paleta --------------------------------------------------------------------------------

pub mod adaptador_egui;
pub mod bravura;
mod textos;

#[cfg(test)]
mod captura_testes;
#[cfg(test)]
mod textos_testes;
pub use adaptador_egui::*;

// Spike do ticket 09 (ADR-0018): protótipo de render com a Bravura, só em testes.
#[cfg(test)]
mod spike_bravura;

// --- Primitivas ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mao {
    Direita,
    Esquerda,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cor {
    Tinta,
    Suave,
}

/// Vínculo de uma primitiva com uma nota (para destacar enquanto soa).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Alvo {
    pub compasso: usize,
    /// Início e fim dentro do compasso, em semínimas.
    pub inicio: f64,
    pub fim: f64,
    pub mao: Mao,
}

/// Forma geométrica em sp.
#[derive(Clone, Debug, PartialEq)]
pub enum Forma {
    Linha {
        a: Ponto,
        b: Ponto,
        espessura: f32,
    },
    /// Polígono convexo preenchido.
    Preenchido {
        pontos: Vec<Ponto>,
    },
    Caminho {
        pontos: Vec<Ponto>,
        espessura: f32,
    },
    /// Cabeça de nota: elipse inclinada (oca para mínima e semibreve).
    Cabeca {
        centro: Ponto,
        rx: f32,
        ry: f32,
        rotacao: f32,
        oca: bool,
        rotacao_oco: f32,
    },
    Disco {
        centro: Ponto,
        raio: f32,
    },
    Texto {
        pos: Ponto,
        texto: String,
        altura: f32,
        ancora: Ancora,
    },
    /// Glifo da fonte musical Bravura (SMuFL): `pos` é a origem na linha de base, à esquerda do
    /// glifo; `escala` 1,0 é o corpo de 4 sp do SMuFL. Hoje só o dedilhado o usa.
    Glifo {
        pos: Ponto,
        caractere: char,
        escala: f32,
    },
}

impl Forma {
    fn deslocar(&mut self, dx: f32, dy: f32) {
        let d = vetor(dx, dy);
        match self {
            Forma::Linha { a, b, .. } => {
                *a += d;
                *b += d;
            }
            Forma::Preenchido { pontos } | Forma::Caminho { pontos, .. } => {
                for p in pontos {
                    *p += d;
                }
            }
            Forma::Cabeca { centro, .. } | Forma::Disco { centro, .. } => *centro += d,
            Forma::Texto { pos, .. } | Forma::Glifo { pos, .. } => *pos += d,
        }
    }

    /// Retângulo envolvente (aproximado), em sp.
    fn limites(&self) -> Retangulo {
        match self {
            Forma::Linha { a, b, .. } => Retangulo::from_two_pos(*a, *b),
            Forma::Preenchido { pontos } | Forma::Caminho { pontos, .. } => {
                let mut r = Retangulo::NOTHING;
                for p in pontos {
                    r.extend_with(*p);
                }
                r
            }
            Forma::Cabeca { centro, rx, .. } => {
                Retangulo::from_center_size(*centro, vetor(*rx, *rx) * 2.0)
            }
            Forma::Disco { centro, raio } => {
                Retangulo::from_center_size(*centro, vetor(*raio, *raio) * 2.0)
            }
            Forma::Texto { pos, altura, .. } => {
                Retangulo::from_center_size(*pos, vetor(*altura, *altura))
            }
            Forma::Glifo {
                pos,
                caractere,
                escala,
            } => Retangulo::from_min_max(
                ponto(pos.x, pos.y - bravura::ALTURA_DO_ALGARISMO * escala),
                ponto(
                    pos.x + bravura::avanco_do_glifo(*caractere) * escala,
                    pos.y + 0.05 * escala,
                ),
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub forma: Forma,
    pub cor: Cor,
    pub alvo: Option<Alvo>,
    /// Identidade estável da nota a que a primitiva pertence (cabeça, ponto de aumento,
    /// acidente e, desde o ticket 19, a letra e o dedilhado da nota). O serviço de documento a
    /// preenche (ticket 10); enquanto for `None` vale o vínculo antigo por compasso (`alvo`).
    /// Só a cabeça é alvo de clique (`Layout::nota_em`).
    pub nota: Option<IdNota>,
}

fn item(forma: Forma) -> Item {
    Item {
        forma,
        cor: Cor::Tinta,
        alvo: None,
        nota: None,
    }
}

fn linha(a: Ponto, b: Ponto, espessura: f32) -> Item {
    item(Forma::Linha { a, b, espessura })
}

// --- Saída do layout -----------------------------------------------------------------------

/// Um sistema (linha de pautas) já posicionado, com primitivas em sp absolutos.
#[derive(Clone, Debug)]
pub struct Sistema {
    pub y_topo: f32,
    pub y_base: f32,
    pub x0: f32,
    pub x1: f32,
    pub itens: Vec<Item>,
    pub primeiro: usize,
    /// Um depois do último compasso.
    pub fim: usize,
}

/// Posição de um compasso no layout.
#[derive(Clone, Debug)]
pub struct CompassoLay {
    pub sistema: usize,
    pub x0: f32,
    pub x1: f32,
    /// (tempo no compasso em semínimas, x do centro da coluna).
    pub colunas: Vec<(f64, f32)>,
    pub duracao: f64,
}

/// Resultado de [`Layout::acertar`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acerto {
    Nota(IdNota),
    Compasso(usize),
}

/// Tolerância padrão do hit test de nota, em sp (folga ao redor da cabeça).
pub const TOLERANCIA_NOTA_SP: f32 = 0.25;
/// Folga vertical (sp) fora do sistema onde ainda se procuram notas (linhas suplementares).
const MARGEM_NOTA_SP: f32 = 4.0;

#[derive(Clone, Debug)]
pub struct Layout {
    pub largura: f32,
    pub altura: f32,
    pub sistemas: Vec<Sistema>,
    pub compassos: Vec<CompassoLay>,
}

impl Layout {
    /// Posição x do cursor no compasso `c` no instante `t` (semínimas desde o início do
    /// compasso), interpolando entre colunas.
    pub fn x_no_compasso(&self, c: usize, t: f64) -> Option<f32> {
        let cl = self.compassos.get(c)?;
        let mut anterior: Option<(f64, f32)> = None;
        for (p, x) in &cl.colunas {
            if *p > t + 1e-9 {
                return Some(match anterior {
                    Some((p0, x0)) if *p > p0 => x0 + (x - x0) * ((t - p0) / (p - p0)) as f32,
                    _ => *x,
                });
            }
            anterior = Some((*p, *x));
        }
        let (p0, x0) = anterior?;
        let fim = cl.x1 - 0.8;
        if cl.duracao > p0 {
            Some(x0 + (fim - x0).max(0.0) * (((t - p0) / (cl.duracao - p0)).clamp(0.0, 1.0) as f32))
        } else {
            Some(x0)
        }
    }

    /// Compasso sob o ponto `(x, y)` em sp, se houver.
    pub fn compasso_em(&self, x: f32, y: f32) -> Option<usize> {
        let s = self
            .sistemas
            .iter()
            .position(|s| y >= s.y_topo - 3.0 && y <= s.y_base + 3.0)?;
        (self.sistemas[s].primeiro..self.sistemas[s].fim)
            .find(|c| x >= self.compassos[*c].x0 && x <= self.compassos[*c].x1)
    }

    /// Nota (por identidade) sob o ponto `p` em sp: a cabeça cuja elipse, alargada em `tol` sp,
    /// contém o ponto; havendo várias, a de menor distância relativa ao centro. Primitivas sem
    /// identidade nunca são devolvidas.
    pub fn nota_em(&self, p: Ponto, tol: f32) -> Option<IdNota> {
        let mut melhor: Option<(f32, IdNota)> = None;
        for s in &self.sistemas {
            if p.y < s.y_topo - MARGEM_NOTA_SP || p.y > s.y_base + MARGEM_NOTA_SP {
                continue;
            }
            for it in &s.itens {
                let (
                    Some(id),
                    Forma::Cabeca {
                        centro,
                        rx,
                        ry,
                        rotacao,
                        ..
                    },
                ) = (it.nota, &it.forma)
                else {
                    continue;
                };
                // Ponto no referencial da elipse (girada por `rotacao`).
                let (dx, dy) = (p.x - centro.x, p.y - centro.y);
                let (sen, cos) = rotacao.sin_cos();
                let (u, v) = (dx * cos + dy * sen, -dx * sen + dy * cos);
                let d = (u / (rx + tol)).powi(2) + (v / (ry + tol)).powi(2);
                if d <= 1.0 && melhor.is_none_or(|(m, _)| d < m) {
                    melhor = Some((d, id));
                }
            }
        }
        melhor.map(|(_, id)| id)
    }

    /// Sistema e centro (em sp) da cabeça da nota `id`, se o layout a tiver (ticket 12: anel de
    /// seleção e rolagem até a nota).
    pub fn cabeca_da_nota(&self, id: IdNota) -> Option<(usize, Ponto)> {
        self.sistemas.iter().enumerate().find_map(|(i, s)| {
            s.itens.iter().find_map(|it| match (it.nota, &it.forma) {
                (Some(n), Forma::Cabeca { centro, .. }) if n == id => Some((i, *centro)),
                _ => None,
            })
        })
    }

    /// Hit test completo: a nota sob o ponto, se alguma primitiva com identidade o contiver;
    /// senão o compasso (comportamento antigo, o único possível sem identidades).
    pub fn acertar(&self, p: Ponto, tol: f32) -> Option<Acerto> {
        if let Some(id) = self.nota_em(p, tol) {
            return Some(Acerto::Nota(id));
        }
        self.compasso_em(p.x, p.y).map(Acerto::Compasso)
    }
}

// --- Análise de uma pauta em um compasso ---------------------------------------------------

#[derive(Clone, Debug)]
struct NotaL {
    /// Identidade da nota no documento (`None` quando o layout é calculado sem identidades).
    id: Option<IdNota>,
    /// Meios espaços acima da linha inferior (0 = linha inferior, 8 = superior).
    s: i32,
    acc: Option<Acidente>,
    acc_col: u8,
    /// -1: à esquerda da haste; 0: normal; 1: à direita.
    desloc: i8,
    midi: Option<u8>,
    liga_prox: bool,
    liga_ant: bool,
    ligaduras: Vec<MarcaLigadura>,
}

#[derive(Clone, Debug)]
struct Evento {
    voz: u8,
    pos: Fracao,
    dur: Fracao,
    figura: Figura,
    pontos: u8,
    pausa: bool,
    pausa_compasso: bool,
    adorno: bool,
    notas: Vec<NotaL>,
    cima: bool,
    grupo: Option<usize>,
    vigas: Vec<EstadoViga>,
    tuplo: Option<u32>,
    inicio_tuplo: bool,
    fim_tuplo: bool,
    n_vozes: usize,
    rank: usize,
    mao: Mao,
    haste_dica: Option<DirecaoHaste>,
}

impl Evento {
    fn meia_cabeca(&self) -> f32 {
        if matches!(self.figura, Figura::Semibreve | Figura::Breve) {
            MEIA_SEMIBREVE
        } else {
            MEIA_CABECA
        }
    }
}

#[derive(Clone, Debug)]
struct GrupoViga {
    eventos: Vec<usize>,
    cima: bool,
}

#[derive(Clone, Debug, Default)]
struct AnalisePauta {
    eventos: Vec<Evento>,
    grupos: Vec<GrupoViga>,
    /// Colunas de acidentes por posição.
    acc_cols: Vec<(Fracao, u8)>,
    /// Trocas de clave dentro do compasso (posição > 0).
    trocas: Vec<(Fracao, Clave)>,
}

fn s_de(altura: Altura, clave: Clave) -> i32 {
    altura.diatonica() - clave.linha_inferior()
}

fn clave_em(ini: Clave, trocas: &[(Fracao, Clave)], pos: Fracao) -> Clave {
    trocas
        .iter()
        .rfind(|(p, _)| *p <= pos)
        .map_or(ini, |(_, c)| *c)
}

fn cima_natural(ev: &Evento) -> bool {
    match (ev.notas.first(), ev.notas.last()) {
        (Some(b), Some(t)) => (4 - b.s) > (t.s - 4),
        _ => true,
    }
}

/// Notas com acidente numa mesma posição: (evento, nota, meios espaços).
type PorPosicao = (Fracao, Vec<(usize, usize, i32)>);

fn analisar_pauta(
    comp: &Compasso,
    pauta: u8,
    clave_ini: Clave,
    trocas: &[(Fracao, Clave)],
    ids: &dyn Fn(usize) -> Option<IdNota>,
) -> AnalisePauta {
    let notas: Vec<(usize, &Nota)> = comp
        .notas
        .iter()
        .enumerate()
        .filter(|(_, n)| n.pauta == pauta)
        .collect();
    let mut vozes: Vec<u8> = notas.iter().map(|(_, n)| n.voz).collect();
    vozes.sort_unstable();
    vozes.dedup();
    let n_vozes = vozes.len();
    let mut a = AnalisePauta {
        trocas: trocas.to_vec(),
        ..Default::default()
    };
    for (rank, voz) in vozes.iter().enumerate() {
        let primeiro_idx = a.eventos.len();
        for &(ordinal, n) in notas.iter().filter(|(_, n)| n.voz == *voz) {
            let clave = clave_em(clave_ini, trocas, n.posicao);
            let nl = n.altura.map(|alt| NotaL {
                id: ids(ordinal),
                s: s_de(alt, clave),
                acc: n.acidente,
                acc_col: 0,
                desloc: 0,
                midi: n.midi,
                liga_prox: n.ligada_ao_proximo,
                liga_ant: n.ligada_ao_anterior,
                ligaduras: n.ligaduras.clone(),
            });
            if n.acorde
                && a.eventos.len() > primeiro_idx
                && let Some(ev) = a.eventos.last_mut()
            {
                if let Some(nl) = nl {
                    ev.notas.push(nl);
                }
                continue;
            }
            a.eventos.push(Evento {
                voz: *voz,
                pos: n.posicao,
                dur: n.duracao,
                figura: n.figura,
                pontos: n.pontos,
                pausa: n.altura.is_none(),
                pausa_compasso: n.pausa_de_compasso,
                adorno: n.adorno,
                notas: nl.into_iter().collect(),
                cima: true,
                grupo: None,
                vigas: n.vigas.clone(),
                tuplo: n.tuplo.map(|(x, _)| x),
                inicio_tuplo: n.inicio_tuplo,
                fim_tuplo: n.fim_tuplo,
                n_vozes,
                rank,
                mao: if clave.sinal == SinalClave::Fa {
                    Mao::Esquerda
                } else {
                    Mao::Direita
                },
                haste_dica: n.haste,
            });
        }
        // Grupos de viga desta voz.
        let fim_idx = a.eventos.len();
        let mut aberto: Option<Vec<usize>> = None;
        for i in primeiro_idx..fim_idx {
            match a.eventos[i].vigas.first().copied() {
                Some(EstadoViga::Inicio) => aberto = Some(vec![i]),
                Some(EstadoViga::Continua) => {
                    if let Some(g) = aberto.as_mut() {
                        g.push(i);
                    }
                }
                Some(EstadoViga::Fim) => {
                    if let Some(mut g) = aberto.take() {
                        g.push(i);
                        if g.len() >= 2 {
                            let id = a.grupos.len();
                            for k in &g {
                                a.eventos[*k].grupo = Some(id);
                            }
                            a.grupos.push(GrupoViga {
                                eventos: g,
                                cima: true,
                            });
                        }
                    }
                }
                _ => aberto = None,
            }
        }
    }
    for ev in &mut a.eventos {
        ev.notas.sort_by_key(|n| n.s);
    }
    // Direção das hastes.
    for i in 0..a.eventos.len() {
        let ev = &a.eventos[i];
        let cima = if ev.n_vozes >= 2 {
            ev.rank == 0
        } else if let Some(d) = ev.haste_dica {
            d == DirecaoHaste::Cima
        } else {
            cima_natural(ev)
        };
        a.eventos[i].cima = cima;
    }
    for g in 0..a.grupos.len() {
        let idx = a.grupos[g].eventos.clone();
        let primeiro = &a.eventos[idx[0]];
        let cima = if primeiro.n_vozes >= 2 {
            primeiro.rank == 0
        } else if let Some(d) = primeiro.haste_dica {
            d == DirecaoHaste::Cima
        } else {
            let soma: i32 = idx
                .iter()
                .filter_map(|k| {
                    let ev = &a.eventos[*k];
                    let (b, t) = (ev.notas.first()?, ev.notas.last()?);
                    Some(if (t.s - 4).abs() >= (4 - b.s).abs() {
                        t.s - 4
                    } else {
                        b.s - 4
                    })
                })
                .sum();
            soma < 0
        };
        a.grupos[g].cima = cima;
        for k in idx {
            a.eventos[k].cima = cima;
        }
    }
    // Cabeças deslocadas (segundas) e colunas de acidentes.
    for ev in &mut a.eventos {
        let n = ev.notas.len();
        if n < 2 {
            continue;
        }
        if ev.cima {
            for i in 1..n {
                if ev.notas[i].s - ev.notas[i - 1].s == 1 && ev.notas[i - 1].desloc == 0 {
                    ev.notas[i].desloc = 1;
                }
            }
        } else {
            for i in (0..n - 1).rev() {
                if ev.notas[i + 1].s - ev.notas[i].s == 1 && ev.notas[i + 1].desloc == 0 {
                    ev.notas[i].desloc = -1;
                }
            }
        }
    }
    let mut por_pos: Vec<PorPosicao> = Vec::new();
    for (ie, ev) in a.eventos.iter().enumerate() {
        for (inota, n) in ev.notas.iter().enumerate() {
            if n.acc.is_some() {
                match por_pos.iter_mut().find(|(p, _)| *p == ev.pos) {
                    Some((_, v)) => v.push((ie, inota, n.s)),
                    None => por_pos.push((ev.pos, vec![(ie, inota, n.s)])),
                }
            }
        }
    }
    for (pos, mut v) in por_pos {
        v.sort_by_key(|(_, _, s)| -s);
        let mut colocados: Vec<(u8, i32)> = Vec::new();
        let mut maximo = 0u8;
        for (ie, inota, s) in v {
            let mut col = 0u8;
            while colocados
                .iter()
                .any(|(c, sp)| *c == col && (sp - s).abs() < 6)
            {
                col += 1;
            }
            colocados.push((col, s));
            maximo = maximo.max(col + 1);
            a.eventos[ie].notas[inota].acc_col = col;
        }
        a.acc_cols.push((pos, maximo));
    }
    a
}

// --- Plano horizontal de um compasso -------------------------------------------------------

struct PlanoCompasso {
    pos: Vec<Fracao>,
    /// Centro de cada coluna relativo ao início do conteúdo.
    dx: Vec<f32>,
    dx0: f32,
    largura: f32,
    /// Espaço à esquerda de cada coluna (acidentes, adornos, troca de clave).
    sala: Vec<f32>,
}

fn largura_da_duracao(d: f64) -> f32 {
    if d <= 0.0 {
        return 2.6;
    }
    (SEMINIMA_LARGURA + POR_DOBRO * (d.log2() as f32)).max(2.6)
}

fn largura_acidente(a: Acidente) -> f32 {
    match a {
        Acidente::Sustenido | Acidente::Bequadro => 1.0,
        Acidente::Bemol => 0.95,
        Acidente::DobradoSustenido => 1.0,
        Acidente::DobradoBemol => 1.7,
    }
}

fn planejar_compasso(
    analises: &[AnalisePauta],
    info: &InfoCompasso,
    textos: &[(Fracao, f32)],
) -> PlanoCompasso {
    let mut pos: Vec<Fracao> = analises
        .iter()
        .flat_map(|a| a.eventos.iter().map(|e| e.pos))
        .collect();
    pos.sort();
    pos.dedup();
    if pos.is_empty() {
        pos.push(Fracao::ZERO);
    }
    let n = pos.len();
    let mut sala = vec![0.0f32; n];
    let mut pontos = vec![0.0f32; n];
    let mut meia = vec![MEIA_CABECA; n];
    for a in analises {
        for (k, p) in pos.iter().enumerate() {
            let mut s = 0.0f32;
            if let Some((_, cols)) = a.acc_cols.iter().find(|(q, _)| q == p) {
                s += f32::from(*cols) * 1.2;
            }
            let adornos = a.eventos.iter().filter(|e| e.adorno && e.pos == *p).count();
            if adornos > 0 {
                s += 1.7 * adornos as f32;
            }
            if a.eventos
                .iter()
                .any(|e| e.pos == *p && e.notas.iter().any(|n| n.desloc < 0))
            {
                s += 1.25;
            }
            if k > 0 && a.trocas.iter().any(|(q, _)| q == p) {
                s += 2.8;
            }
            sala[k] = sala[k].max(s);
            for e in a.eventos.iter().filter(|e| e.pos == *p && !e.adorno) {
                if e.pontos > 0 {
                    pontos[k] = pontos[k].max(0.9 + 0.5 * f32::from(e.pontos));
                }
                meia[k] = meia[k].max(e.meia_cabeca() * 1.08);
            }
        }
    }
    // Largura do texto (letra e cifra) de cada coluna: sem textos, tudo zero e nada muda.
    let larg: Vec<f32> = pos
        .iter()
        .map(|p| {
            textos
                .iter()
                .filter(|(q, _)| q == p)
                .fold(0.0f32, |m, (_, w)| m.max(*w))
        })
        .collect();
    let rep_ini = if info.repeticao_inicio { 1.8 } else { 0.0 };
    let rep_fim = if info.repeticao_fim { 1.8 } else { 0.0 };
    let dx0 = (1.0 + rep_ini + sala[0]).max(larg[0] / 2.0 + 0.3);
    let dur = info.duracao.como_f64();
    let mut dx = vec![dx0];
    for k in 0..n - 1 {
        let d = (pos[k + 1] - pos[k]).como_f64();
        let natural = largura_da_duracao(d);
        let minimo = meia[k] + meia[k + 1] + pontos[k] + sala[k + 1] + 0.4;
        let anterior = dx[k];
        let texto = (larg[k] + larg[k + 1]) / 2.0 + 0.5;
        dx.push(anterior + natural.max(minimo).max(texto));
    }
    let d_ultimo = (dur - pos[n - 1].como_f64()).max(0.25);
    let cauda = (0.85 * largura_da_duracao(d_ultimo))
        .max(meia[n - 1] + pontos[n - 1] + 0.9)
        .max(larg[n - 1] / 2.0 + 0.9)
        + rep_fim;
    let largura = dx[n - 1] + cauda;
    PlanoCompasso {
        pos,
        dx,
        dx0,
        largura,
        sala,
    }
}

// --- Prefixos (claves, armadura, fôrma) ----------------------------------------------------

fn largura_clave(c: Clave) -> f32 {
    match c.sinal {
        SinalClave::Sol => 2.9,
        SinalClave::Fa => 3.0,
        SinalClave::Do => 2.9,
        _ => 2.0,
    }
}

fn largura_armadura(a: Armadura) -> f32 {
    if a.quintas == 0 {
        0.0
    } else {
        f32::from(a.quintas.unsigned_abs()) * 1.1 + 0.7
    }
}

const LARGURA_FORMA: f32 = 3.0;

// --- Primitivas de glifos ------------------------------------------------------------------

fn cubica(p0: Ponto, p1: Ponto, p2: Ponto, p3: Ponto, n: usize) -> Vec<Ponto> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            ponto(
                a * p0.x + b * p1.x + c * p2.x + d * p3.x,
                a * p0.y + b * p1.y + c * p2.y + d * p3.y,
            )
        })
        .collect()
}

fn juntar(mut a: Vec<Ponto>, b: Vec<Ponto>) -> Vec<Ponto> {
    a.extend(b.into_iter().skip(1));
    a
}

fn caminho(pontos: Vec<Ponto>, espessura: f32) -> Item {
    item(Forma::Caminho { pontos, espessura })
}

/// Clave de sol: `(x, y_g)` é o canto esquerdo na linha do Sol.
fn glifo_clave_sol(x: f32, y: f32, escala: f32) -> Vec<Item> {
    let e = escala;
    let p = |a: f32, b: f32| ponto(x + a * e, y + b * e);
    let mut tracado = cubica(
        p(1.55, -4.4),
        p(2.15, -4.2),
        p(2.2, -3.0),
        p(1.75, -2.1),
        10,
    );
    tracado = juntar(
        tracado,
        cubica(p(1.75, -2.1), p(1.45, -1.5), p(1.2, -0.3), p(1.2, 0.9), 10),
    );
    tracado = juntar(
        tracado,
        cubica(p(1.2, 0.9), p(1.2, 1.6), p(1.25, 2.0), p(1.1, 2.3), 6),
    );
    tracado = juntar(
        tracado,
        cubica(p(1.1, 2.3), p(0.95, 2.8), p(0.35, 2.9), p(0.3, 2.35), 8),
    );
    let mut espiral = cubica(p(1.35, -4.3), p(0.7, -3.9), p(0.7, -3.0), p(1.2, -2.2), 8);
    espiral = juntar(
        espiral,
        cubica(p(1.2, -2.2), p(1.8, -1.2), p(2.55, -0.7), p(2.5, 0.3), 10),
    );
    espiral = juntar(
        espiral,
        cubica(p(2.5, 0.3), p(2.45, 1.1), p(1.7, 1.5), p(1.0, 1.25), 10),
    );
    espiral = juntar(
        espiral,
        cubica(p(1.0, 1.25), p(0.3, 1.0), p(0.1, 0.1), p(0.6, -0.5), 10),
    );
    espiral = juntar(
        espiral,
        cubica(p(0.6, -0.5), p(1.0, -0.9), p(1.8, -0.7), p(1.85, -0.1), 8),
    );
    espiral = juntar(
        espiral,
        cubica(p(1.85, -0.1), p(1.85, 0.4), p(1.2, 0.55), p(0.95, 0.2), 6),
    );
    vec![
        caminho(tracado, 0.2 * e.max(0.5)),
        caminho(espiral, 0.2 * e.max(0.5)),
        item(Forma::Disco {
            centro: p(0.3, 2.3),
            raio: 0.26 * e,
        }),
    ]
}

/// Clave de fá: `(x, y_f)` é o canto esquerdo na linha do Fá.
fn glifo_clave_fa(x: f32, y: f32, escala: f32) -> Vec<Item> {
    let e = escala;
    let p = |a: f32, b: f32| ponto(x + a * e, y + b * e);
    let mut tracado = cubica(
        p(0.55, -0.55),
        p(1.5, -0.7),
        p(2.55, -0.1),
        p(2.35, 1.1),
        12,
    );
    tracado = juntar(
        tracado,
        cubica(p(2.35, 1.1), p(2.1, 2.1), p(1.2, 2.7), p(0.15, 3.1), 12),
    );
    vec![
        item(Forma::Disco {
            centro: p(0.55, 0.0),
            raio: 0.55 * e,
        }),
        caminho(tracado, 0.34 * e.max(0.5)),
        item(Forma::Disco {
            centro: p(2.85, -0.55),
            raio: 0.22 * e,
        }),
        item(Forma::Disco {
            centro: p(2.85, 0.55),
            raio: 0.22 * e,
        }),
    ]
}

/// Clave de dó: `(x, y_c)` é o canto esquerdo na linha do Dó.
fn glifo_clave_do(x: f32, y: f32, escala: f32) -> Vec<Item> {
    let e = escala;
    let p = |a: f32, b: f32| ponto(x + a * e, y + b * e);
    let mut cima = cubica(p(1.2, -0.1), p(1.6, -0.9), p(2.4, -1.1), p(2.6, -1.7), 8);
    cima = juntar(
        cima,
        cubica(p(2.6, -1.7), p(2.3, -2.2), p(1.4, -2.0), p(1.2, -1.2), 8),
    );
    let mut baixo = cubica(p(1.2, 0.1), p(1.6, 0.9), p(2.4, 1.1), p(2.6, 1.7), 8);
    baixo = juntar(
        baixo,
        cubica(p(2.6, 1.7), p(2.3, 2.2), p(1.4, 2.0), p(1.2, 1.2), 8),
    );
    vec![
        item(Forma::Preenchido {
            pontos: vec![p(0.0, -2.0), p(0.55, -2.0), p(0.55, 2.0), p(0.0, 2.0)],
        }),
        linha(p(0.85, -2.0), p(0.85, 2.0), 0.14 * e),
        caminho(cima, 0.3 * e.max(0.5)),
        caminho(baixo, 0.3 * e.max(0.5)),
    ]
}

fn glifo_clave(c: Clave, x: f32, y_topo: f32, escala: f32) -> Vec<Item> {
    let linha_y = |l: u8| y_topo + (4.0 - f32::from(l.clamp(1, 5)) + 1.0) * 1.0;
    let mut v = match c.sinal {
        SinalClave::Sol => glifo_clave_sol(x, linha_y(c.linha), escala),
        SinalClave::Fa => glifo_clave_fa(x, linha_y(c.linha), escala),
        SinalClave::Do => glifo_clave_do(x, linha_y(c.linha), escala),
        SinalClave::Percussao | SinalClave::Tablatura => {
            let y = y_topo + 2.0;
            vec![
                item(Forma::Preenchido {
                    pontos: vec![
                        ponto(x + 0.3, y - 0.9),
                        ponto(x + 0.7, y - 0.9),
                        ponto(x + 0.7, y + 0.9),
                        ponto(x + 0.3, y + 0.9),
                    ],
                }),
                item(Forma::Preenchido {
                    pontos: vec![
                        ponto(x + 1.1, y - 0.9),
                        ponto(x + 1.5, y - 0.9),
                        ponto(x + 1.5, y + 0.9),
                        ponto(x + 1.1, y + 0.9),
                    ],
                }),
            ]
        }
    };
    if c.oitava != 0 && matches!(c.sinal, SinalClave::Sol | SinalClave::Fa) {
        let (y, txt) = if c.oitava < 0 {
            (y_topo + 5.4, "8")
        } else {
            (y_topo - 1.4, "8")
        };
        v.push(item(Forma::Texto {
            pos: ponto(x + 1.4 * escala, y),
            texto: txt.into(),
            altura: 1.5,
            ancora: Ancora::CENTER_CENTER,
        }));
    }
    v
}

/// Posições (em meios espaços acima da linha inferior) dos acidentes da armadura.
fn posicoes_armadura(quintas: i8, clave: Clave) -> Vec<i32> {
    const SUST: [i32; 7] = [38, 35, 39, 36, 33, 37, 34];
    const BEM: [i32; 7] = [34, 37, 33, 36, 32, 35, 31];
    let base = clave.linha_inferior();
    let deslocamento = (f64::from(base - 30) / 7.0).round() as i32 * 7;
    let n = usize::from(quintas.unsigned_abs()).min(7);
    let tabela = if quintas > 0 { SUST } else { BEM };
    tabela[..n]
        .iter()
        .map(|d| d + deslocamento - base)
        .collect()
}

fn glifo_acidente(a: Acidente, x: f32, y: f32) -> Vec<Item> {
    let p = |dx: f32, dy: f32| ponto(x + dx, y + dy);
    match a {
        Acidente::Sustenido => vec![
            linha(p(0.3, -1.4), p(0.3, 1.5), 0.1),
            linha(p(0.68, -1.5), p(0.68, 1.4), 0.1),
            linha(p(0.0, 0.5), p(0.98, 0.15), 0.34),
            linha(p(0.0, -0.2), p(0.98, -0.55), 0.34),
        ],
        Acidente::Bequadro => vec![
            linha(p(0.15, -1.5), p(0.15, 0.6), 0.1),
            linha(p(0.72, -0.6), p(0.72, 1.5), 0.1),
            linha(p(0.1, 0.65), p(0.78, 0.4), 0.3),
            linha(p(0.1, -0.3), p(0.78, -0.55), 0.3),
        ],
        Acidente::Bemol => {
            let mut c = cubica(p(0.12, 0.6), p(0.55, 0.45), p(0.95, 0.0), p(0.8, -0.55), 8);
            c = juntar(
                c,
                cubica(
                    p(0.8, -0.55),
                    p(0.65, -0.95),
                    p(0.25, -0.5),
                    p(0.12, -0.15),
                    8,
                ),
            );
            vec![linha(p(0.12, -1.9), p(0.12, 0.62), 0.11), caminho(c, 0.2)]
        }
        Acidente::DobradoSustenido => vec![
            linha(p(0.05, -0.45), p(0.95, 0.45), 0.3),
            linha(p(0.05, 0.45), p(0.95, -0.45), 0.3),
        ],
        Acidente::DobradoBemol => {
            let mut v = glifo_acidente(Acidente::Bemol, x, y);
            v.extend(glifo_acidente(Acidente::Bemol, x + 0.8, y));
            v
        }
    }
}

fn elipse_pontos(c: Ponto, rx: f32, ry: f32, rot: f32, n: usize) -> Vec<Ponto> {
    let (s, co) = rot.sin_cos();
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32 * std::f32::consts::TAU;
            let (x, y) = (rx * t.cos(), ry * t.sin());
            ponto(c.x + x * co - y * s, c.y + x * s + y * co)
        })
        .collect()
}

fn glifo_pausa(fig: Figura, x: f32, y: f32) -> Vec<Item> {
    let p = |dx: f32, dy: f32| ponto(x + dx, y + dy);
    match fig {
        Figura::Semibreve | Figura::Breve => vec![item(Forma::Preenchido {
            pontos: vec![p(-0.6, -1.0), p(0.6, -1.0), p(0.6, -0.5), p(-0.6, -0.5)],
        })],
        Figura::Minima => vec![item(Forma::Preenchido {
            pontos: vec![p(-0.6, -0.5), p(0.6, -0.5), p(0.6, 0.0), p(-0.6, 0.0)],
        })],
        Figura::Seminima => vec![
            caminho(
                vec![
                    p(0.15, -1.5),
                    p(-0.2, -1.0),
                    p(0.3, -0.4),
                    p(-0.25, 0.25),
                    p(0.3, 0.85),
                ],
                0.4,
            ),
            caminho(
                juntar(
                    vec![p(0.3, 0.85)],
                    cubica(p(0.3, 0.85), p(-0.4, 0.7), p(-0.6, 1.3), p(-0.1, 1.6), 6),
                ),
                0.3,
            ),
        ],
        _ => {
            let n = usize::from(fig.bandeirolas().max(1));
            let (ax, ay) = (0.5f32, -0.75f32);
            let (bx, by) = (-0.1 - 0.35 * (n as f32 - 1.0), 0.95 + (n as f32 - 1.0));
            let x_da_haste = |yk: f32| ax + (bx - ax) * (yk - ay) / (by - ay);
            let mut v = vec![linha(p(ax, ay), p(bx, by), 0.15)];
            for k in 0..n {
                let yk = -0.45 + k as f32;
                let xh = x_da_haste(yk);
                v.push(item(Forma::Disco {
                    centro: p(xh - 0.5, yk),
                    raio: 0.28,
                }));
                v.push(caminho(
                    cubica(
                        p(xh - 0.5, yk),
                        p(xh - 0.25, yk + 0.2),
                        p(xh - 0.05, yk + 0.05),
                        p(xh + 0.05, yk - 0.3),
                        6,
                    ),
                    0.18,
                ));
            }
            v
        }
    }
}

/// Chave (brace) entre `y0` e `y1`, com a ponta direita em `x_dir`.
fn glifo_chave(x_dir: f32, y0: f32, y1: f32) -> Vec<Item> {
    let h = y1 - y0;
    let ym = (y0 + y1) / 2.0;
    let xl = x_dir - 1.35;
    let xr = x_dir - 0.15;
    let xv = x_dir - 0.75;
    let mut centro = cubica(
        ponto(xr, y0),
        ponto(xv - 0.35, y0 + 0.02 * h),
        ponto(xv, y0 + 0.1 * h),
        ponto(xv, y0 + 0.27 * h),
        10,
    );
    centro = juntar(
        centro,
        cubica(
            ponto(xv, y0 + 0.27 * h),
            ponto(xv, y0 + 0.40 * h),
            ponto(xl + 0.5, ym - 0.04 * h),
            ponto(xl, ym),
            10,
        ),
    );
    let mut baixo: Vec<Ponto> = centro
        .iter()
        .rev()
        .map(|p| ponto(p.x, 2.0 * ym - p.y))
        .collect();
    let mut total = centro;
    total.extend(baixo.drain(1..));
    let n = total.len();
    let espessura = |i: usize| -> f32 {
        let t = i as f32 / (n - 1) as f32;
        let meia = if t < 0.5 { t * 2.0 } else { (1.0 - t) * 2.0 };
        // sin(π) em f32 dá um negativo minúsculo e `powf` de negativo é NaN: limita em zero.
        0.05 + 0.5 * (meia * std::f32::consts::PI).sin().max(0.0).powf(0.9)
    };
    let mut itens = Vec::new();
    for i in 0..n - 1 {
        let (a, b) = (total[i], total[i + 1]);
        // Pontos repetidos da curva dariam normal NaN (0/0) e quebrariam a tesselação.
        if (b - a).length() < 1e-6 {
            continue;
        }
        let d = (b - a).normalized();
        let nrm = vetor(-d.y, d.x);
        let (wa, wb) = (espessura(i) / 2.0, espessura(i + 1) / 2.0);
        itens.push(item(Forma::Preenchido {
            pontos: vec![a + nrm * wa, b + nrm * wb, b - nrm * wb, a - nrm * wa],
        }));
    }
    itens
}

fn texto_item(pos: Ponto, t: &str, altura: f32, ancora: Ancora, cor: Cor) -> Item {
    Item {
        forma: Forma::Texto {
            pos,
            texto: t.to_owned(),
            altura,
            ancora,
        },
        cor,
        alvo: None,
        nota: None,
    }
}

// --- Emissão de uma pauta em um compasso ---------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct MarcaArco {
    /// 0 = ligadura de valor; 1 = de expressão.
    tipo: u8,
    chave: (u8, u8),
    inicio: bool,
    x: f32,
    y: f32,
    acima: bool,
    ordem: (usize, Fracao),
}

fn y_de_s(s: i32) -> f32 {
    ALTURA_PAUTA - s as f32 * 0.5
}

struct ContextoPauta<'a> {
    analise: &'a AnalisePauta,
    compasso: usize,
    pauta_idx: u8,
    x_de: &'a dyn Fn(Fracao) -> f32,
    x_centro_compasso: f32,
    itens: Vec<Item>,
    arcos: Vec<MarcaArco>,
}

fn alvo_de(ev: &Evento, compasso: usize) -> Alvo {
    Alvo {
        compasso,
        inicio: ev.pos.como_f64(),
        fim: (ev.pos + ev.dur).como_f64(),
        mao: ev.mao,
    }
}

fn emitir_cabeca(ctx: &mut ContextoPauta<'_>, ev: &Evento, n: &NotaL, x: f32, escala: f32) {
    let alvo = alvo_de(ev, ctx.compasso);
    let semibreve = matches!(ev.figura, Figura::Semibreve | Figura::Breve);
    let oca = semibreve || ev.figura == Figura::Minima;
    let hw = ev.meia_cabeca() * escala;
    let xc = x + f32::from(n.desloc) * 2.0 * hw * 0.98;
    let yc = y_de_s(n.s);
    let (rx, ry) = if semibreve {
        (0.84, 0.46)
    } else {
        (0.64, 0.45)
    };
    ctx.itens.push(Item {
        forma: Forma::Cabeca {
            centro: ponto(xc, yc),
            rx: rx * escala,
            ry: ry * escala,
            rotacao: if semibreve { 0.0 } else { -0.38 },
            oca,
            rotacao_oco: if semibreve { 1.35 } else { -0.55 },
        },
        cor: Cor::Tinta,
        alvo: Some(alvo),
        nota: n.id,
    });
    // Linhas suplementares.
    if escala >= 0.99 {
        let mut linhas = Vec::new();
        if n.s <= -2 {
            let mut sl = -2;
            while sl >= n.s {
                linhas.push(sl);
                sl -= 2;
            }
        } else if n.s >= 10 {
            let mut sl = 10;
            while sl <= n.s {
                linhas.push(sl);
                sl += 2;
            }
        }
        for sl in linhas {
            let y = y_de_s(sl);
            ctx.itens.push(linha(
                ponto(xc - hw - SUPLEMENTAR_EXTENSAO, y),
                ponto(xc + hw + SUPLEMENTAR_EXTENSAO, y),
                SUPLEMENTAR_ESPESSURA,
            ));
        }
    }
    // Pontos de aumento.
    for k in 0..ev.pontos {
        let dy = if n.s % 2 == 0 { -0.5 } else { 0.0 };
        let xd = x
            + MEIA_CABECA * escala
            + if n.desloc > 0 { 2.0 * hw } else { 0.0 }
            + 0.85
            + 0.5 * f32::from(k);
        ctx.itens.push(Item {
            forma: Forma::Disco {
                centro: ponto(xd, yc + dy),
                raio: 0.2 * escala,
            },
            cor: Cor::Tinta,
            alvo: Some(alvo),
            nota: n.id,
        });
    }
    // Acidente.
    if let Some(a) = n.acc {
        let borda = x
            - hw
            - 0.25
            - if ev.notas.iter().any(|o| o.desloc < 0) {
                1.25
            } else {
                0.0
            }
            - f32::from(n.acc_col) * 1.2;
        let larg = largura_acidente(a);
        for mut it in glifo_acidente(a, borda - larg, yc) {
            it.alvo = Some(alvo);
            it.nota = n.id;
            ctx.itens.push(it);
        }
    }
    // Marcas de ligaduras.
    let acima = if ev.n_vozes >= 2 { ev.cima } else { !ev.cima };
    let lado = if acima { -0.65 } else { 0.65 };
    let ordem = (ctx.compasso, ev.pos);
    if n.liga_prox {
        ctx.arcos.push(MarcaArco {
            tipo: 0,
            chave: (ctx.pauta_idx, n.midi.unwrap_or(0)),
            inicio: true,
            x: xc + hw + 0.2,
            y: yc + lado,
            acima,
            ordem,
        });
    }
    for m in &n.ligaduras {
        ctx.arcos.push(MarcaArco {
            tipo: 1,
            chave: (ctx.pauta_idx, m.numero),
            inicio: m.inicio,
            x: xc + if m.inicio { 0.2 } else { -0.2 },
            y: yc + lado * 1.4,
            acima,
            ordem,
        });
    }
    if n.liga_ant {
        ctx.arcos.push(MarcaArco {
            tipo: 0,
            chave: (ctx.pauta_idx, n.midi.unwrap_or(0)),
            inicio: false,
            x: xc - hw - 0.2,
            y: yc + lado,
            acima,
            ordem,
        });
    }
}

fn pontos_da_bandeirola(x: f32, y: f32, cima: bool) -> Vec<Ponto> {
    let sinal = if cima { 1.0 } else { -1.0 };
    let p = |dx: f32, dy: f32| ponto(x + dx, y + sinal * dy);
    juntar(
        cubica(p(0.0, 0.0), p(0.15, 1.2), p(1.5, 1.4), p(1.05, 3.1), 10),
        cubica(p(1.05, 3.1), p(1.25, 2.1), p(0.45, 1.45), p(0.0, 1.5), 8),
    )
}

/// Emite notas, hastes, vigas e bandeirolas, pausas e tercinas de uma pauta.
fn emitir_eventos(ctx: &mut ContextoPauta<'_>) {
    let a = ctx.analise;
    let mut pontas: Vec<Option<(f32, f32)>> = vec![None; a.eventos.len()];
    // Geometria de haste de cada evento.
    struct Haste {
        x: f32,
        base: f32,
        ponta: f32,
    }
    let mut hastes: Vec<Option<Haste>> = vec![];
    for ev in &a.eventos {
        if ev.pausa || ev.notas.is_empty() || !ev.figura.tem_haste() {
            hastes.push(None);
            continue;
        }
        let escala = if ev.adorno { 0.7 } else { 1.0 };
        let x = (ctx.x_de)(ev.pos) - if ev.adorno { adorno_recuo(a, ev) } else { 0.0 };
        let hw = ev.meia_cabeca() * escala;
        let (base_s, ponta_s) = (ev.notas[0].s, ev.notas[ev.notas.len() - 1].s);
        let comp = HASTE_COMPRIMENTO * escala.max(0.75);
        let h = if ev.cima {
            let ponta = (y_de_s(ponta_s) - comp).min(ALTURA_PAUTA / 2.0);
            Haste {
                x: x + hw - HASTE_ESPESSURA / 2.0,
                base: y_de_s(base_s),
                ponta,
            }
        } else {
            let ponta = (y_de_s(base_s) + comp).max(ALTURA_PAUTA / 2.0);
            Haste {
                x: x - hw + HASTE_ESPESSURA / 2.0,
                base: y_de_s(ponta_s),
                ponta,
            }
        };
        hastes.push(Some(h));
    }

    // Cabeças e acidentes.
    for ev in &a.eventos {
        let escala = if ev.adorno { 0.7 } else { 1.0 };
        let x = (ctx.x_de)(ev.pos) - if ev.adorno { adorno_recuo(a, ev) } else { 0.0 };
        if ev.pausa {
            let dy = if ev.n_vozes >= 2 {
                if ev.rank == 0 { -1.0 } else { 1.0 }
            } else {
                0.0
            };
            let xr = if ev.pausa_compasso {
                ctx.x_centro_compasso
            } else {
                x
            };
            let figura = if ev.pausa_compasso {
                Figura::Semibreve
            } else {
                ev.figura
            };
            let ycentro = ALTURA_PAUTA / 2.0 + dy;
            let gl = glifo_pausa(figura, xr, ycentro);
            ctx.itens.extend(gl);
            for k in 0..ev.pontos {
                ctx.itens.push(item(Forma::Disco {
                    centro: ponto(xr + 1.0 + 0.5 * f32::from(k), ycentro - 0.5),
                    raio: 0.2,
                }));
            }
            continue;
        }
        for n in &ev.notas {
            emitir_cabeca(ctx, ev, n, x, escala);
        }
    }

    // Hastes, vigas e bandeirolas.
    let mut feitos: HashSet<usize> = HashSet::new();
    for g in &a.grupos {
        let hs: Vec<&Haste> = g
            .eventos
            .iter()
            .filter_map(|i| hastes[*i].as_ref())
            .collect();
        if hs.len() != g.eventos.len() {
            continue;
        }
        let (x1, x2) = (hs[0].x, hs[hs.len() - 1].x);
        let t_de = |x: f32| if x2 > x1 { (x - x1) / (x2 - x1) } else { 0.0 };
        let sinal = if g.cima { 1.0 } else { -1.0 };
        // `y` cresce para baixo: para cima, a viga fica no menor y.
        let dy_bruto = hs[hs.len() - 1].ponta - hs[0].ponta;
        let mut dy = dy_bruto.clamp(-VIGA_DESNIVEL_MAX, VIGA_DESNIVEL_MAX);
        if hs.len() > 2 {
            let interior_extremo = hs[1..hs.len() - 1].iter().any(|h| {
                if g.cima {
                    h.ponta < hs[0].ponta.min(hs[hs.len() - 1].ponta)
                } else {
                    h.ponta > hs[0].ponta.max(hs[hs.len() - 1].ponta)
                }
            });
            if interior_extremo {
                dy = 0.0;
            }
        }
        let comp_min = HASTE_COMPRIMENTO
            + 0.75
                * (a.eventos[g.eventos[0]]
                    .figura
                    .bandeirolas()
                    .saturating_sub(2)) as f32;
        let extra = (comp_min - HASTE_COMPRIMENTO).max(0.0);
        let mut y1 = if g.cima { f32::MAX } else { f32::MIN };
        for h in &hs {
            let v = h.ponta - sinal * extra - dy * t_de(h.x);
            y1 = if g.cima { y1.min(v) } else { y1.max(v) };
        }
        let y_viga = |x: f32| y1 + dy * t_de(x);
        // Hastes até a viga.
        for (k, i) in g.eventos.iter().enumerate() {
            let h = hs[k];
            let ev = &a.eventos[*i];
            let ponta = y_viga(h.x);
            pontas[*i] = Some((h.x, ponta));
            ctx.itens.push(Item {
                forma: Forma::Linha {
                    a: ponto(h.x, h.base),
                    b: ponto(h.x, ponta),
                    espessura: HASTE_ESPESSURA,
                },
                cor: Cor::Tinta,
                alvo: Some(alvo_de(ev, ctx.compasso)),
                nota: None,
            });
            feitos.insert(*i);
        }
        // Vigas por nível.
        let max_nivel = g
            .eventos
            .iter()
            .map(|i| a.eventos[*i].vigas.len())
            .max()
            .unwrap_or(1);
        for nivel in 0..max_nivel {
            let deslocamento = nivel as f32 * (VIGA_ESPESSURA + VIGA_ESPACO);
            let mut inicio: Option<usize> = None;
            let mut segmentos: Vec<(f32, f32)> = Vec::new();
            for (k, i) in g.eventos.iter().enumerate() {
                match a.eventos[*i].vigas.get(nivel) {
                    Some(EstadoViga::Inicio) => inicio = Some(k),
                    Some(EstadoViga::Continua) => {}
                    Some(EstadoViga::Fim) => {
                        if let Some(s) = inicio.take() {
                            segmentos.push((hs[s].x, hs[k].x));
                        }
                    }
                    Some(EstadoViga::GanchoFrente) => {
                        segmentos.push((hs[k].x, (hs[k].x + 1.1).min(x2.max(hs[k].x + 0.6))))
                    }
                    Some(EstadoViga::GanchoAtras) => {
                        segmentos.push(((hs[k].x - 1.1).max(x1.min(hs[k].x - 0.6)), hs[k].x))
                    }
                    None => inicio = None,
                }
            }
            for (xa, xb) in segmentos {
                let (xa, xb) = (xa - HASTE_ESPESSURA / 2.0, xb + HASTE_ESPESSURA / 2.0);
                // Para cima os níveis seguintes ficam abaixo (y maior); para baixo, acima.
                let (ya, yb) = if g.cima {
                    (y_viga(xa) + deslocamento, y_viga(xb) + deslocamento)
                } else {
                    (y_viga(xa) - deslocamento, y_viga(xb) - deslocamento)
                };
                let esp = if g.cima {
                    VIGA_ESPESSURA
                } else {
                    -VIGA_ESPESSURA
                };
                ctx.itens.push(item(Forma::Preenchido {
                    pontos: vec![
                        ponto(xa, ya),
                        ponto(xb, yb),
                        ponto(xb, yb + esp),
                        ponto(xa, ya + esp),
                    ],
                }));
            }
        }
    }
    for (i, ev) in a.eventos.iter().enumerate() {
        if feitos.contains(&i) {
            continue;
        }
        let Some(h) = &hastes[i] else { continue };
        pontas[i] = Some((h.x, h.ponta));
        ctx.itens.push(Item {
            forma: Forma::Linha {
                a: ponto(h.x, h.base),
                b: ponto(h.x, h.ponta),
                espessura: HASTE_ESPESSURA,
            },
            cor: Cor::Tinta,
            alvo: Some(alvo_de(ev, ctx.compasso)),
            nota: None,
        });
        for k in 0..ev.figura.bandeirolas() {
            let desloc = f32::from(k) * 0.95 * if ev.cima { 1.0 } else { -1.0 };
            let mut it = caminho(
                pontos_da_bandeirola(h.x + HASTE_ESPESSURA / 2.0, h.ponta + desloc, ev.cima),
                0.32,
            );
            it.alvo = Some(alvo_de(ev, ctx.compasso));
            ctx.itens.push(it);
        }
    }
    emitir_tercinas(ctx, &pontas);
}

fn adorno_recuo(a: &AnalisePauta, ev: &Evento) -> f32 {
    let mesmos: Vec<usize> = a
        .eventos
        .iter()
        .enumerate()
        .filter(|(_, e)| e.adorno && e.pos == ev.pos)
        .map(|(i, _)| i)
        .collect();
    let minha = a
        .eventos
        .iter()
        .position(|e| std::ptr::eq(e, ev))
        .unwrap_or(0);
    let k = mesmos.iter().position(|i| *i == minha).unwrap_or(0);
    let total = mesmos.len();
    let acc = a
        .acc_cols
        .iter()
        .find(|(p, _)| *p == ev.pos)
        .map_or(0.0, |(_, c)| f32::from(*c) * 1.2);
    acc + 0.9 + 1.7 * (total - k) as f32 - 0.6
}

fn emitir_tercinas(ctx: &mut ContextoPauta<'_>, pontas: &[Option<(f32, f32)>]) {
    let a = ctx.analise;
    let mut i = 0;
    while i < a.eventos.len() {
        if a.eventos[i].inicio_tuplo {
            let voz = a.eventos[i].voz;
            let mut j = i;
            while j < a.eventos.len() && !(a.eventos[j].fim_tuplo && a.eventos[j].voz == voz) {
                j += 1;
            }
            if j >= a.eventos.len() {
                break;
            }
            let membros: Vec<usize> = (i..=j).filter(|k| a.eventos[*k].voz == voz).collect();
            let numero = a.eventos[i].tuplo.unwrap_or(3);
            let cima = a.eventos[i].cima;
            let xs: Vec<f32> = membros
                .iter()
                .map(|k| (ctx.x_de)(a.eventos[*k].pos))
                .collect();
            let (xa, xb) = (
                xs.iter().copied().fold(f32::MAX, f32::min),
                xs.iter().copied().fold(f32::MIN, f32::max),
            );
            let mut ys: Vec<f32> = membros
                .iter()
                .filter_map(|k| pontas[*k].map(|(_, y)| y))
                .collect();
            if ys.is_empty() {
                ys.push(if cima { 0.0 } else { ALTURA_PAUTA });
            }
            let extremo = if cima {
                ys.iter().copied().fold(f32::MAX, f32::min)
            } else {
                ys.iter().copied().fold(f32::MIN, f32::max)
            };
            let tudo_vigado = membros.iter().all(|k| a.eventos[*k].grupo.is_some());
            let y = extremo + if cima { -1.0 } else { 1.0 };
            let xm = (xa + xb) / 2.0;
            if !tudo_vigado && xb > xa + 1.0 {
                let tick = if cima { 0.5 } else { -0.5 };
                ctx.itens
                    .push(linha(ponto(xa, y + tick), ponto(xa, y), 0.1));
                ctx.itens.push(linha(ponto(xa, y), ponto(xm - 0.9, y), 0.1));
                ctx.itens.push(linha(ponto(xm + 0.9, y), ponto(xb, y), 0.1));
                ctx.itens
                    .push(linha(ponto(xb, y), ponto(xb, y + tick), 0.1));
            }
            ctx.itens.push(texto_item(
                ponto(xm, y),
                &numero.to_string(),
                1.5,
                Ancora::CENTER_CENTER,
                Cor::Tinta,
            ));
            i = j + 1;
        } else {
            i += 1;
        }
    }
}

// --- Arcos (ligaduras de valor e de expressão) ---------------------------------------------

fn arco(x1: f32, y1: f32, x2: f32, y2: f32, acima: bool, expressao: bool) -> Item {
    let dx = (x2 - x1).max(0.5);
    let altura = if expressao {
        (0.7 + dx * 0.07).min(2.6)
    } else {
        (0.4 + dx * 0.05).min(1.0)
    };
    let sinal = if acima { -1.0 } else { 1.0 };
    let c1 = ponto(x1 + dx * 0.28, y1 + sinal * altura * 1.33);
    let c2 = ponto(x2 - dx * 0.28, y2 + sinal * altura * 1.33);
    caminho(
        cubica(ponto(x1, y1), c1, c2, ponto(x2, y2), 16),
        if expressao { 0.13 } else { 0.2 },
    )
}

/// Casa as marcas de ligaduras em ordem de tempo e produz os arcos.
fn gerar_arcos(marcas: &mut [MarcaArco], x_ini: f32, x_fim: f32) -> Vec<Item> {
    marcas.sort_by(|a, b| a.ordem.cmp(&b.ordem).then(a.inicio.cmp(&b.inicio)));
    let mut abertas: HashMap<(u8, (u8, u8)), MarcaArco> = HashMap::new();
    let mut saida: Vec<Item> = Vec::new();
    for m in marcas.iter() {
        let chave = (m.tipo, m.chave);
        if m.inicio {
            if let Some(velha) = abertas.insert(chave, *m) {
                saida.push(arco(
                    velha.x,
                    velha.y,
                    x_fim,
                    velha.y,
                    velha.acima,
                    m.tipo == 1,
                ));
            }
        } else if let Some(i) = abertas.remove(&chave) {
            saida.push(arco(i.x, i.y, m.x, m.y, i.acima, m.tipo == 1));
        } else {
            saida.push(arco(x_ini, m.y, m.x, m.y, m.acima, m.tipo == 1));
        }
    }
    for (_, i) in abertas {
        saida.push(arco(i.x, i.y, x_fim, i.y, i.acima, i.tipo == 1));
    }
    saida
}

// --- Layout --------------------------------------------------------------------------------

struct PautaGlobal {
    parte: usize,
    pauta: u8,
}

struct EstadoClaves {
    /// Clave em vigor no início de cada compasso.
    inicio: Vec<Clave>,
    /// Há troca de clave exatamente no início do compasso (exceto o primeiro).
    troca_no_inicio: Vec<bool>,
    internas: Vec<Vec<(Fracao, Clave)>>,
}

fn claves_da_pauta(p: &Partitura, parte: usize, pauta: u8) -> EstadoClaves {
    let mut atual = Clave::SOL;
    let mut ini = Vec::new();
    let mut troca = Vec::new();
    let mut internas = Vec::new();
    for (m, comp) in p.partes[parte].compassos.iter().enumerate() {
        let mut interna = Vec::new();
        let mut mudou = false;
        for c in comp.claves.iter().filter(|c| c.pauta == pauta) {
            if c.posicao.e_zero() {
                atual = c.clave;
                mudou = m > 0;
            }
        }
        ini.push(atual);
        for c in comp.claves.iter().filter(|c| c.pauta == pauta) {
            if !c.posicao.e_zero() {
                interna.push((c.posicao, c.clave));
            }
        }
        interna.sort_by_key(|(p, _)| *p);
        if let Some((_, c)) = interna.last() {
            atual = *c;
        }
        troca.push(mudou);
        internas.push(interna);
    }
    EstadoClaves {
        inicio: ini,
        troca_no_inicio: troca,
        internas,
    }
}

fn desenhar_barra(itens: &mut Vec<Item>, x: f32, y0: f32, y1: f32, b: Barra, rep_fim: bool) {
    let fina = 0.16;
    if rep_fim {
        itens.push(linha(ponto(x - 1.0, y0), ponto(x - 1.0, y1), fina));
        itens.push(item(Forma::Preenchido {
            pontos: vec![
                ponto(x - 0.5, y0),
                ponto(x, y0),
                ponto(x, y1),
                ponto(x - 0.5, y1),
            ],
        }));
        return;
    }
    match b {
        Barra::Simples => itens.push(linha(ponto(x, y0), ponto(x, y1), fina)),
        Barra::Dupla => {
            itens.push(linha(ponto(x - 0.45, y0), ponto(x - 0.45, y1), fina));
            itens.push(linha(ponto(x, y0), ponto(x, y1), fina));
        }
        Barra::Final => {
            itens.push(linha(ponto(x - 0.95, y0), ponto(x - 0.95, y1), fina));
            itens.push(item(Forma::Preenchido {
                pontos: vec![
                    ponto(x - 0.5, y0),
                    ponto(x, y0),
                    ponto(x, y1),
                    ponto(x - 0.5, y1),
                ],
            }));
        }
        Barra::Nenhuma => {}
    }
}

fn pontos_de_repeticao(itens: &mut Vec<Item>, x: f32, y_topo: f32) {
    for dy in [1.5, 2.5] {
        itens.push(item(Forma::Disco {
            centro: ponto(x, y_topo + dy),
            raio: 0.25,
        }));
    }
}

struct SistemaTrabalho {
    primeiro: usize,
    fim: usize,
    esticar: f32,
}

/// Diagrama a partitura para uma largura disponível, em sp, quebrando em sistemas. Sem
/// identidades de nota: as primitivas da nota não carregam [`IdNota`] (`Item::nota` é `None`).
pub fn calcular_layout(p: &Partitura, largura_sp: f32) -> Layout {
    calcular_layout_com(p, None, None, largura_sp, false)
}

/// Como [`calcular_layout`], mas as primitivas de cada nota (cabeça, pontos de aumento e
/// acidente) carregam a identidade que o serviço de documento atribuiu a ela (ticket 10). A
/// geometria é a mesma: só `Item::nota` muda.
pub fn calcular_layout_com_ids(p: &Partitura, ids: &IdsPorNota, largura_sp: f32) -> Layout {
    calcular_layout_com(p, Some(ids), None, largura_sp, false)
}

/// Como [`calcular_layout_com_ids`], e ainda desenha a letra (uma linha por estrofe, com a
/// altura reservada), a cifra e o dedilhado que a [`Exibicao`] traz, junto das notas a que
/// pertencem (ticket 19). Cada texto carrega a identidade da sua nota em `Item::nota`.
pub fn calcular_layout_com_exibicao(
    p: &Partitura,
    ids: &IdsPorNota,
    exibicao: &Exibicao,
    largura_sp: f32,
) -> Layout {
    calcular_layout_com(p, Some(ids), Some(exibicao), largura_sp, false)
}

/// Diagrama a partitura como **fita contínua**: um único sistema sem quebras de linha, na
/// largura natural dos compassos (sem esticar). Clave e armadura só aparecem no início e onde
/// mudam. `Layout::largura` é a largura total da fita, em sp.
pub fn calcular_layout_fita(p: &Partitura) -> Layout {
    calcular_layout_com(p, None, None, LARGURA_MINIMA, true)
}

/// A fita contínua com as identidades de nota (ver [`calcular_layout_com_ids`]).
pub fn calcular_layout_fita_com_ids(p: &Partitura, ids: &IdsPorNota) -> Layout {
    calcular_layout_com(p, Some(ids), None, LARGURA_MINIMA, true)
}

/// A fita contínua com letra, cifra e dedilhado (ver [`calcular_layout_com_exibicao`]).
pub fn calcular_layout_fita_com_exibicao(
    p: &Partitura,
    ids: &IdsPorNota,
    exibicao: &Exibicao,
) -> Layout {
    calcular_layout_com(p, Some(ids), Some(exibicao), LARGURA_MINIMA, true)
}

fn calcular_layout_com(
    p: &Partitura,
    ids: Option<&IdsPorNota>,
    exibicao: Option<&Exibicao>,
    largura_sp: f32,
    fita: bool,
) -> Layout {
    let largura = largura_sp.max(LARGURA_MINIMA);
    let n_comp = p.compassos.len();
    let pautas: Vec<PautaGlobal> = p
        .partes
        .iter()
        .enumerate()
        .flat_map(|(i, pt)| (1..=pt.pautas.max(1)).map(move |k| PautaGlobal { parte: i, pauta: k }))
        .collect();
    let claves: Vec<EstadoClaves> = pautas
        .iter()
        .map(|g| claves_da_pauta(p, g.parte, g.pauta))
        .collect();
    // Análises e planos.
    let analises: Vec<Vec<AnalisePauta>> = (0..n_comp)
        .map(|m| {
            pautas
                .iter()
                .enumerate()
                .map(|(i, g)| {
                    analisar_pauta(
                        &p.partes[g.parte].compassos[m],
                        g.pauta,
                        claves[i].inicio[m],
                        &claves[i].internas[m],
                        &|ordinal| ids.and_then(|t| t.id(g.parte, m, ordinal)),
                    )
                })
                .collect()
        })
        .collect();
    // Letra, cifra e dedilhado: só com identidades de nota e algo para mostrar.
    let extras = match (ids, exibicao) {
        (Some(i), Some(e)) if !e.vazia() => Some((i, e)),
        _ => None,
    };
    let estrofes = extras.map_or_else(Vec::new, |(i, e)| {
        textos::estrofes_das_pautas(p, i, e, &pautas)
    });
    let textos_de: Vec<textos::TextosCompasso> = (0..n_comp)
        .map(|m| {
            extras.map_or_else(Default::default, |(i, e)| {
                textos::coletar(p, i, e, &pautas, &estrofes, m)
            })
        })
        .collect();
    let planos: Vec<PlanoCompasso> = (0..n_comp)
        .map(|m| planejar_compasso(&analises[m], &p.compassos[m], &textos_de[m].larguras()))
        .collect();

    // Larguras de prefixo.
    let maior_clave = |m: usize| -> f32 {
        (0..pautas.len())
            .map(|i| largura_clave(claves[i].inicio[m]))
            .fold(0.0, f32::max)
    };
    let prefixo_primeiro = |m: usize, primeiro_sistema: bool| -> f32 {
        let inf = &p.compassos[m];
        let mut w = maior_clave(m) + 1.2 + largura_armadura(inf.armadura);
        if primeiro_sistema || inf.forma_mudou {
            w += LARGURA_FORMA;
        }
        w
    };
    let prefixo_meio = |m: usize| -> f32 {
        let inf = &p.compassos[m];
        let mut w = 0.0;
        if inf.armadura_mudou {
            w += largura_armadura(inf.armadura) + 0.5;
        }
        if inf.forma_mudou {
            w += LARGURA_FORMA;
        }
        if claves.iter().any(|c| c.troca_no_inicio[m]) {
            w += 2.6;
        }
        w
    };

    // Quebra de sistemas.
    let disponivel = if fita {
        f32::INFINITY
    } else {
        largura - MARGEM_ESQUERDA - MARGEM_DIREITA
    };
    let mut trabalho: Vec<SistemaTrabalho> = Vec::new();
    let mut i = 0;
    while i < n_comp {
        let primeiro_sistema = trabalho.is_empty();
        let mut soma = prefixo_primeiro(i, primeiro_sistema) + planos[i].largura;
        let mut j = i + 1;
        while j < n_comp {
            let w = prefixo_meio(j) + planos[j].largura;
            if soma + w > disponivel {
                break;
            }
            soma += w;
            j += 1;
        }
        // Estica ou comprime.
        let fixo: f32 = (i..j)
            .map(|m| {
                (if m == i {
                    prefixo_primeiro(m, primeiro_sistema)
                } else {
                    prefixo_meio(m)
                }) + planos[m].dx0
            })
            .sum();
        let esticavel: f32 = (i..j).map(|m| planos[m].largura - planos[m].dx0).sum();
        let mut r = if esticavel > 0.0 {
            (disponivel - fixo) / esticavel
        } else {
            1.0
        };
        r = if fita { 1.0 } else { r.clamp(0.6, ESTICAR_MAX) };
        trabalho.push(SistemaTrabalho {
            primeiro: i,
            fim: j,
            esticar: r,
        });
        i = j;
    }
    let ultimo = trabalho.len().saturating_sub(1);
    if let Some(u) = trabalho.get_mut(ultimo)
        && !fita
    {
        u.esticar = u.esticar.min(ESTICAR_ULTIMO);
    }

    // Emissão.
    let mut sistemas: Vec<Sistema> = Vec::new();
    let mut compassos_lay: Vec<Option<CompassoLay>> = vec![None; n_comp];
    let mut y_cursor = 2.5;
    let n_pautas = pautas.len();
    for (is, t) in trabalho.iter().enumerate() {
        let primeiro_sistema = is == 0;
        let mut por_pauta: Vec<Vec<Item>> = vec![Vec::new(); n_pautas];
        let mut marcas: Vec<Vec<MarcaArco>> = vec![Vec::new(); n_pautas];
        // Letra e cifra esperam o fim do sistema (a altura delas depende do resto da pauta);
        // o dedilhado já é desenhado junto das notas.
        let mut letras_pend: Vec<Vec<(f32, &textos::LetraL)>> = vec![Vec::new(); n_pautas];
        let mut cifras_pend: Vec<Vec<(f32, &textos::CifraL)>> = vec![Vec::new(); n_pautas];
        let mut dedo_acima = vec![0.0f32; n_pautas];
        let mut dedo_abaixo = vec![0.0f32; n_pautas];
        let mut x = MARGEM_ESQUERDA;
        let x_inicio = x;
        let mut barras_x: Vec<(f32, Barra, bool)> = Vec::new();
        let mut medidas: Vec<(usize, f32, f32)> = Vec::new();
        for m in t.primeiro..t.fim {
            let plano = &planos[m];
            let inf = &p.compassos[m];
            let eh_primeiro = m == t.primeiro;
            // Prefixo.
            let xp = if eh_primeiro {
                for k in 0..n_pautas {
                    let c = claves[k].inicio[m];
                    por_pauta[k].extend(glifo_clave(c, x + 0.4, 0.0, 1.0));
                }
                let xa = x + maior_clave(m) + 0.8;
                for k in 0..n_pautas {
                    por_pauta[k].extend(armadura_itens(inf.armadura, claves[k].inicio[m], xa));
                }
                if primeiro_sistema || inf.forma_mudou {
                    let xt = xa + largura_armadura(inf.armadura).max(0.0);
                    for k in 0..n_pautas {
                        por_pauta[k].extend(forma_itens(inf, xt));
                    }
                }
                x + prefixo_primeiro(m, primeiro_sistema)
            } else {
                let mut xq = x;
                if claves.iter().any(|c| c.troca_no_inicio[m]) {
                    for k in 0..n_pautas {
                        if claves[k].troca_no_inicio[m] {
                            por_pauta[k].extend(glifo_clave(
                                claves[k].inicio[m],
                                xq + 0.3,
                                0.0,
                                0.8,
                            ));
                        }
                    }
                    xq += 2.6;
                }
                if inf.armadura_mudou {
                    for k in 0..n_pautas {
                        por_pauta[k].extend(armadura_itens(
                            inf.armadura,
                            claves[k].inicio[m],
                            xq + 0.2,
                        ));
                    }
                    xq += largura_armadura(inf.armadura) + 0.5;
                }
                if inf.forma_mudou {
                    for k in 0..n_pautas {
                        por_pauta[k].extend(forma_itens(inf, xq));
                    }
                    xq += LARGURA_FORMA;
                }
                xq
            };
            let x_conteudo = xp;
            let r = t.esticar;
            let largura_m = plano.dx0 + (plano.largura - plano.dx0) * r;
            let x_fim = x_conteudo + largura_m;
            let centros: Vec<f32> = plano
                .dx
                .iter()
                .map(|d| x_conteudo + plano.dx0 + (d - plano.dx0) * r)
                .collect();
            let x_de = |pos: Fracao| -> f32 {
                match plano.pos.iter().position(|q| *q == pos) {
                    Some(k) => centros[k],
                    None => centros.first().copied().unwrap_or(x_conteudo),
                }
            };
            let x_centro = (x_conteudo + x_fim) / 2.0;
            for k in 0..n_pautas {
                let mut ctx = ContextoPauta {
                    analise: &analises[m][k],
                    compasso: m,
                    pauta_idx: k as u8,
                    x_de: &x_de,
                    x_centro_compasso: x_centro,
                    itens: Vec::new(),
                    arcos: Vec::new(),
                };
                emitir_eventos(&mut ctx);
                // Trocas de clave no meio do compasso.
                for (pos, c) in &analises[m][k].trocas {
                    let kx = plano.pos.iter().position(|q| q == pos);
                    if let Some(kx) = kx {
                        let xc = centros[kx] - plano.sala[kx] + 0.2;
                        ctx.itens.extend(glifo_clave(*c, xc, 0.0, 0.8));
                    }
                }
                por_pauta[k].extend(ctx.itens);
                marcas[k].extend(ctx.arcos);
                let t = &textos_de[m];
                let dedos: Vec<&textos::DedoL> = t.dedos.iter().filter(|d| d.k == k).collect();
                if !dedos.is_empty() {
                    let (itens, ac, ab) =
                        textos::itens_do_dedilhado(&dedos, &analises[m][k], &x_de);
                    por_pauta[k].extend(itens);
                    dedo_acima[k] = dedo_acima[k].max(ac);
                    dedo_abaixo[k] = dedo_abaixo[k].max(ab);
                }
                letras_pend[k].extend(
                    t.letras
                        .iter()
                        .filter(|l| l.k == k)
                        .map(|l| (x_de(l.pos), l)),
                );
                cifras_pend[k].extend(
                    t.cifras
                        .iter()
                        .filter(|c| c.k == k)
                        .map(|c| (x_de(c.pos), c)),
                );
            }
            barras_x.push((x_fim, inf.barra_final, inf.repeticao_fim));
            // Casas de repetição, números de compasso e colunas para o cursor.
            let colunas: Vec<(f64, f32)> = plano
                .pos
                .iter()
                .zip(&centros)
                .map(|(q, c)| (q.como_f64(), *c))
                .collect();
            compassos_lay[m] = Some(CompassoLay {
                sistema: is,
                x0: x,
                x1: x_fim,
                colunas,
                duracao: inf.duracao.como_f64(),
            });
            medidas.push((m, x_conteudo, x_fim));
            x = x_fim;
        }
        let x_fim_sistema = x;

        // Altura de cada pauta (extensões acima e abaixo).
        let mut acima = vec![0.0f32; n_pautas];
        let mut abaixo = vec![0.0f32; n_pautas];
        for k in 0..n_pautas {
            let mut ymin = 0.0f32;
            let mut ymax = ALTURA_PAUTA;
            for it in &por_pauta[k] {
                if matches!(it.forma, Forma::Texto { .. }) {
                    continue;
                }
                let r = it.forma.limites();
                ymin = ymin.min(r.min.y);
                ymax = ymax.max(r.max.y);
            }
            acima[k] = -ymin;
            abaixo[k] = ymax - ALTURA_PAUTA;
        }
        // Arcos (precisam do y relativo, e ampliam as extensões).
        for k in 0..n_pautas {
            let gerados = gerar_arcos(&mut marcas[k], x_inicio + 3.5, x_fim_sistema - 0.5);
            for it in gerados {
                let r = it.forma.limites();
                acima[k] = acima[k].max(-r.min.y);
                abaixo[k] = abaixo[k].max(r.max.y - ALTURA_PAUTA);
                por_pauta[k].push(it);
            }
        }
        let tem_casa = (t.primeiro..t.fim).any(|m| !p.compassos[m].casas.is_empty());
        let reserva_topo = if tem_casa { 3.6 } else { 2.6 };
        // Dedilhado, letra (uma linha por estrofe, abaixo de tudo o que a pauta já tem embaixo)
        // e cifra (acima de tudo o que a pauta já tem em cima, do número do compasso e das casas):
        // a altura reservada entra na da pauta, e a pauta de baixo é empurrada.
        for k in 0..n_pautas {
            acima[k] = acima[k].max(dedo_acima[k]);
            abaixo[k] = abaixo[k].max(dedo_abaixo[k]);
            let (itens, novo_abaixo) = textos::itens_da_letra(&letras_pend[k], abaixo[k]);
            por_pauta[k].extend(itens);
            abaixo[k] = novo_abaixo;
            // As pautas de baixo não têm número de compasso nem casas acima delas.
            let reserva = if k == 0 { reserva_topo } else { 1.0 };
            let (itens, novo_acima) = textos::itens_da_cifra(&cifras_pend[k], acima[k], reserva);
            por_pauta[k].extend(itens);
            acima[k] = novo_acima;
        }
        // Posição vertical.
        let y_topo_sistema = y_cursor;
        let mut ys: Vec<f32> = Vec::with_capacity(n_pautas);
        let mut y = y_topo_sistema + acima[0].max(reserva_topo);
        for k in 0..n_pautas {
            ys.push(y);
            if k + 1 < n_pautas {
                let base: f32 = if pautas[k].parte == pautas[k + 1].parte {
                    4.2
                } else {
                    5.5
                };
                let gap = base.max(abaixo[k] + acima[k + 1] + 1.0);
                y += ALTURA_PAUTA + gap;
            }
        }
        let y_base = ys[n_pautas - 1] + ALTURA_PAUTA;

        let mut itens: Vec<Item> = Vec::new();
        // Linhas de pauta.
        for k in 0..n_pautas {
            for l in 0..5 {
                itens.push(linha(
                    ponto(x_inicio, ys[k] + l as f32),
                    ponto(x_fim_sistema, ys[k] + l as f32),
                    LINHA_PAUTA,
                ));
            }
        }
        // Barra inicial, chaves e barras de compasso.
        itens.push(linha(ponto(x_inicio, ys[0]), ponto(x_inicio, y_base), 0.16));
        let mut k = 0;
        while k < n_pautas {
            let mut fim = k;
            while fim + 1 < n_pautas && pautas[fim + 1].parte == pautas[k].parte {
                fim += 1;
            }
            if fim > k {
                itens.extend(glifo_chave(x_inicio - 0.15, ys[k], ys[fim] + ALTURA_PAUTA));
            }
            for (xb, barra, rep) in &barras_x {
                desenhar_barra(&mut itens, *xb, ys[k], ys[fim] + ALTURA_PAUTA, *barra, *rep);
                if *rep {
                    pontos_de_repeticao(&mut itens, *xb - 1.6, ys[k]);
                    if fim > k {
                        pontos_de_repeticao(&mut itens, *xb - 1.6, ys[fim]);
                    }
                }
            }
            // Barra de repetição inicial atravessa o grupo.
            for (m, xc, _) in &medidas {
                if p.compassos[*m].repeticao_inicio {
                    itens.push(item(Forma::Preenchido {
                        pontos: vec![
                            ponto(xc + 0.2, ys[k]),
                            ponto(xc + 0.7, ys[k]),
                            ponto(xc + 0.7, ys[fim] + ALTURA_PAUTA),
                            ponto(xc + 0.2, ys[fim] + ALTURA_PAUTA),
                        ],
                    }));
                    itens.push(linha(
                        ponto(xc + 1.0, ys[k]),
                        ponto(xc + 1.0, ys[fim] + ALTURA_PAUTA),
                        0.16,
                    ));
                    pontos_de_repeticao(&mut itens, xc + 1.6, ys[k]);
                    if fim > k {
                        pontos_de_repeticao(&mut itens, xc + 1.6, ys[fim]);
                    }
                }
            }
            k = fim + 1;
        }
        // Itens de cada pauta, deslocados.
        for k in 0..n_pautas {
            let dy = ys[k];
            for mut it in std::mem::take(&mut por_pauta[k]) {
                it.forma.deslocar(0.0, dy);
                itens.push(it);
            }
        }
        // Números de compasso e casas.
        let (m0, x_primeiro, _) = medidas[0];
        let rotulo = &p.compassos[m0].numero;
        if rotulo != "0" {
            itens.push(texto_item(
                ponto(x_primeiro - 0.2, ys[0] - 1.1),
                rotulo,
                1.25,
                Ancora::LEFT_BOTTOM,
                Cor::Suave,
            ));
        }
        let mut c = 0;
        while c < medidas.len() {
            let (m, xc, _) = medidas[c];
            let casas = &p.compassos[m].casas;
            if casas.is_empty() {
                c += 1;
                continue;
            }
            let mut f = c;
            while f + 1 < medidas.len() && p.compassos[medidas[f + 1].0].casas == *casas {
                f += 1;
            }
            let (xa, xb) = (xc + 0.2, medidas[f].2 - 0.2);
            let yv = ys[0] - 2.4;
            itens.push(linha(ponto(xa, yv), ponto(xb, yv), 0.12));
            itens.push(linha(ponto(xa, yv), ponto(xa, yv + 1.2), 0.12));
            let nome: Vec<String> = casas.iter().map(|n| format!("{n}.")).collect();
            itens.push(texto_item(
                ponto(xa + 0.5, yv + 0.2),
                &nome.join(" "),
                1.25,
                Ancora::LEFT_TOP,
                Cor::Tinta,
            ));
            c = f + 1;
        }
        sistemas.push(Sistema {
            y_topo: ys[0],
            y_base,
            x0: x_inicio,
            x1: x_fim_sistema,
            itens,
            primeiro: t.primeiro,
            fim: t.fim,
        });
        let abaixo_ultimo = abaixo[n_pautas - 1].max(0.0);
        y_cursor = y_base + abaixo_ultimo + 3.5;
    }
    let compassos: Vec<CompassoLay> = compassos_lay
        .into_iter()
        .map(|c| {
            c.unwrap_or(CompassoLay {
                sistema: 0,
                x0: 0.0,
                x1: 0.0,
                colunas: Vec::new(),
                duracao: 0.0,
            })
        })
        .collect();
    let altura = sistemas.last().map_or(10.0, |s| s.y_base + 4.0);
    let largura = if fita {
        sistemas.last().map_or(largura, |s| s.x1 + MARGEM_DIREITA)
    } else {
        largura
    };
    Layout {
        largura,
        altura,
        sistemas,
        compassos,
    }
}

fn armadura_itens(a: Armadura, clave: Clave, x: f32) -> Vec<Item> {
    let mut v = Vec::new();
    let acc = if a.quintas > 0 {
        Acidente::Sustenido
    } else {
        Acidente::Bemol
    };
    for (i, s) in posicoes_armadura(a.quintas, clave).iter().enumerate() {
        v.extend(glifo_acidente(acc, x + i as f32 * 1.1, y_de_s(*s)));
    }
    v
}

fn forma_itens(inf: &InfoCompasso, x: f32) -> Vec<Item> {
    let f = inf.forma;
    let centro = x + 1.3;
    match f.simbolo {
        Some(SimboloForma::Comum) => vec![texto_item(
            ponto(centro, 2.0),
            "C",
            3.6,
            Ancora::CENTER_CENTER,
            Cor::Tinta,
        )],
        Some(SimboloForma::Cortado) => vec![
            texto_item(
                ponto(centro, 2.0),
                "C",
                3.6,
                Ancora::CENTER_CENTER,
                Cor::Tinta,
            ),
            linha(ponto(centro, 0.2), ponto(centro, 3.8), 0.14),
        ],
        None => vec![
            texto_item(
                ponto(centro, 1.0),
                &f.batidas.to_string(),
                2.9,
                Ancora::CENTER_CENTER,
                Cor::Tinta,
            ),
            texto_item(
                ponto(centro, 3.0),
                &f.unidade.to_string(),
                2.9,
                Ancora::CENTER_CENTER,
                Cor::Tinta,
            ),
        ],
    }
}

// --- Pintura -------------------------------------------------------------------------------

/// Instante de execução a destacar: compasso e tempo dentro dele (semínimas).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Destaque {
    pub compasso: usize,
    pub tempo: f64,
}

/// Resultado de uma nota na prática (verde ou vermelho na pauta).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultadoNota {
    Acertou,
    Errou,
}

fn chave_resultado(compasso: usize, inicio: f64, mao: Mao) -> (usize, i64, bool) {
    (
        compasso,
        (inicio * 1000.0).round() as i64,
        mao == Mao::Esquerda,
    )
}

/// Resultados da prática por posição na partitura: (compasso, início dentro do compasso, mão).
/// Notas de uma mesma mão que começam juntas (acorde) compartilham a marca: vermelha se alguma
/// errou, verde só quando todas acertaram.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MapaResultados {
    mapa: HashMap<(usize, i64, bool), (u32, u32, u32)>,
}

impl MapaResultados {
    /// Registra o estado de uma nota: `None` = ainda pendente.
    pub fn registrar(
        &mut self,
        compasso: usize,
        inicio: f64,
        mao: Mao,
        resultado: Option<ResultadoNota>,
    ) {
        let e = self
            .mapa
            .entry(chave_resultado(compasso, inicio, mao))
            .or_default();
        e.0 += 1;
        match resultado {
            Some(ResultadoNota::Acertou) => e.1 += 1,
            Some(ResultadoNota::Errou) => e.2 += 1,
            None => {}
        }
    }

    pub fn vazio(&self) -> bool {
        self.mapa.is_empty()
    }

    /// Marca da primitiva `alvo`, se houver.
    pub fn de(&self, alvo: &Alvo) -> Option<ResultadoNota> {
        let (total, ok, erro) =
            *self
                .mapa
                .get(&chave_resultado(alvo.compasso, alvo.inicio, alvo.mao))?;
        if erro > 0 {
            Some(ResultadoNota::Errou)
        } else if ok == total && total > 0 {
            Some(ResultadoNota::Acertou)
        } else {
            None
        }
    }
}

fn forma_finita(f: &Forma) -> bool {
    match f {
        Forma::Linha { a, b, espessura } => a.is_finite() && b.is_finite() && espessura.is_finite(),
        Forma::Preenchido { pontos } => pontos.iter().all(|p| p.is_finite()),
        Forma::Caminho { pontos, espessura } => {
            espessura.is_finite() && pontos.iter().all(|p| p.is_finite())
        }
        Forma::Cabeca {
            centro,
            rx,
            ry,
            rotacao,
            ..
        } => centro.is_finite() && rx.is_finite() && ry.is_finite() && rotacao.is_finite(),
        Forma::Disco { centro, raio } => centro.is_finite() && raio.is_finite(),
        Forma::Texto { pos, altura, .. } => pos.is_finite() && altura.is_finite(),
        Forma::Glifo { pos, escala, .. } => pos.is_finite() && escala.is_finite(),
    }
}

#[cfg(test)]
mod testes {
    use super::adaptador_egui::{cor_do_item, cor_do_item_com};
    use super::*;
    use bancada_casca::egui::{self, Context, RawInput, Shape};
    use piano_tutor_core::partitura::Partitura;

    const MINIMA: &str = r#"<score-partwise><part-list><score-part id="P1"><part-name>x</part-name></score-part></part-list>
<part id="P1"><measure number="1"><attributes><divisions>2</divisions><key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes>
<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>
<note><pitch><step>B</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>
<note><pitch><step>E</step><octave>5</octave></pitch><duration>1</duration><type>eighth</type><beam number="1">begin</beam></note>
<note><pitch><step>D</step><octave>5</octave></pitch><duration>1</duration><type>eighth</type><beam number="1">end</beam></note>
<note><rest/><duration>2</duration><type>quarter</type></note>
</measure></part></score-partwise>"#;

    fn arq(i: usize) -> &'static str {
        [
            include_str!("../../../assets/partituras/brahms-wiegenlied-op49-4.musicxml"),
            include_str!("../../../assets/partituras/schubert-heidenroeslein-d257.musicxml"),
            include_str!("../../../assets/partituras/faure-barcarolle-op7-3.musicxml"),
        ][i]
    }

    fn cabecas(l: &Layout) -> Vec<(Ponto, bool)> {
        l.sistemas
            .iter()
            .flat_map(|s| &s.itens)
            .filter_map(|i| match &i.forma {
                Forma::Cabeca { centro, oca, .. } => Some((*centro, *oca)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn nota_simples_fica_na_posicao_certa_com_haste_de_3_5_sp() {
        let p = Partitura::de_musicxml(MINIMA).expect("válida");
        let l = calcular_layout(&p, 60.0);
        let c = cabecas(&l);
        assert_eq!(c.len(), 4);
        let s = &l.sistemas[0];
        let y_topo = s.y_topo;
        // Dó4 (1ª linha suplementar abaixo): y = topo + 4 + 1 = 5.
        assert!(
            (c[0].0.y - (y_topo + 5.0)).abs() < 1e-4,
            "{:?} {y_topo}",
            c[0]
        );
        // Si4 (3ª linha): y = topo + 2.
        assert!((c[1].0.y - (y_topo + 2.0)).abs() < 1e-4);
        // Haste do Dó4 (semínima para cima): 3,5 sp acima da nota, na borda direita da cabeça.
        let x_haste = c[0].0.x + MEIA_CABECA - HASTE_ESPESSURA / 2.0;
        let (a, b) = s
            .itens
            .iter()
            .find_map(|i| match &i.forma {
                Forma::Linha { a, b, espessura }
                    if (*espessura - HASTE_ESPESSURA).abs() < 1e-6
                        && (a.x - x_haste).abs() < 1e-3 =>
                {
                    Some((*a, *b))
                }
                _ => None,
            })
            .expect("haste do Dó4");
        assert!(((a.y - b.y).abs() - 3.5).abs() < 1e-3, "haste {a:?} {b:?}");
        assert!(b.y < a.y, "haste do Dó4 sobe");
        // Si4 está na linha do meio: haste para baixo, também de 3,5 sp.
        let x_baixo = c[1].0.x - MEIA_CABECA + HASTE_ESPESSURA / 2.0;
        let (a, b) = s
            .itens
            .iter()
            .find_map(|i| match &i.forma {
                Forma::Linha { a, b, espessura }
                    if (*espessura - HASTE_ESPESSURA).abs() < 1e-6
                        && (a.x - x_baixo).abs() < 1e-3 =>
                {
                    Some((*a, *b))
                }
                _ => None,
            })
            .expect("haste do Si4");
        assert!(((b.y - a.y) - 3.5).abs() < 1e-3, "haste {a:?} {b:?}");
    }

    #[test]
    fn linha_suplementar_avanca_0_4_sp_alem_da_cabeca() {
        let p = Partitura::de_musicxml(MINIMA).expect("válida");
        let l = calcular_layout(&p, 60.0);
        let s = &l.sistemas[0];
        let c = cabecas(&l)[0].0;
        let supl = s
            .itens
            .iter()
            .find_map(|i| match &i.forma {
                Forma::Linha { a, b, espessura }
                    if (*espessura - SUPLEMENTAR_ESPESSURA).abs() < 1e-6
                        && (a.y - b.y).abs() < 1e-6
                        && (a.y - c.y).abs() < 1e-4 =>
                {
                    Some((*a, *b))
                }
                _ => None,
            })
            .expect("linha suplementar do Dó4");
        let meia = (supl.1.x - supl.0.x) / 2.0;
        assert!((meia - (MEIA_CABECA + SUPLEMENTAR_EXTENSAO)).abs() < 1e-4);
        assert!(((supl.0.x + supl.1.x) / 2.0 - c.x).abs() < 1e-4);
    }

    #[test]
    fn colcheias_ligadas_tem_viga_de_0_5_sp_e_haste_na_viga() {
        let p = Partitura::de_musicxml(MINIMA).expect("válida");
        let l = calcular_layout(&p, 60.0);
        let s = &l.sistemas[0];
        let vigas: Vec<&Vec<Ponto>> = s
            .itens
            .iter()
            .filter_map(|i| match &i.forma {
                Forma::Preenchido { pontos } if pontos.len() == 4 => Some(pontos),
                _ => None,
            })
            .filter(|p| {
                let h = (p[3].y - p[0].y).abs();
                (h - VIGA_ESPESSURA).abs() < 1e-4 && (p[1].x - p[0].x) > 1.0
            })
            .collect();
        assert_eq!(vigas.len(), 1);
        let v = vigas[0];
        assert!((v[1].y - v[0].y).abs() <= VIGA_DESNIVEL_MAX + 1e-4);
    }

    #[test]
    fn quebra_de_linha_so_em_compasso_inteiro_e_cabe_na_largura() {
        for i in 0..3 {
            let p = Partitura::de_musicxml(arq(i)).expect("válida");
            for largura in [60.0f32, 90.0, 140.0] {
                let l = calcular_layout(&p, largura);
                assert!(l.sistemas.len() >= 2);
                let mut visto = vec![0usize; p.compassos.len()];
                for s in &l.sistemas {
                    assert!(s.primeiro < s.fim);
                    for c in s.primeiro..s.fim {
                        visto[c] += 1;
                        let cl = &l.compassos[c];
                        assert_eq!(
                            cl.sistema,
                            l.sistemas
                                .iter()
                                .position(|q| std::ptr::eq(q, s))
                                .unwrap_or(99)
                        );
                        assert!(cl.x0 >= s.x0 - 1e-3 && cl.x1 <= s.x1 + 1e-3);
                        assert!(cl.x1 > cl.x0);
                    }
                    // Cabe na largura (exceto compasso único mais largo que a linha).
                    if s.fim - s.primeiro > 1 {
                        assert!(
                            s.x1 <= largura - MARGEM_DIREITA + 1e-2,
                            "{i} {largura}: {}",
                            s.x1
                        );
                    }
                }
                assert!(
                    visto.iter().all(|v| *v == 1),
                    "cada compasso em exatamente um sistema"
                );
            }
        }
    }

    #[test]
    fn larguras_maiores_cabem_mais_compassos_por_sistema() {
        let p = Partitura::de_musicxml(arq(0)).expect("válida");
        let estreito = calcular_layout(&p, 60.0);
        let largo = calcular_layout(&p, 160.0);
        assert!(largo.sistemas.len() < estreito.sistemas.len());
    }

    #[test]
    fn cada_nota_de_altura_vira_uma_cabeca() {
        for i in 0..3 {
            let p = Partitura::de_musicxml(arq(i)).expect("válida");
            let esperado: usize = p
                .partes
                .iter()
                .flat_map(|x| &x.compassos)
                .flat_map(|c| &c.notas)
                .filter(|n| n.altura.is_some())
                .count();
            let l = calcular_layout(&p, 100.0);
            assert_eq!(cabecas(&l).len(), esperado, "arquivo {i}");
        }
    }

    #[test]
    fn cursor_acompanha_o_tempo_dentro_do_compasso() {
        let p = Partitura::de_musicxml(arq(1)).expect("válida");
        let l = calcular_layout(&p, 100.0);
        let x0 = l.x_no_compasso(2, 0.0).expect("x");
        let x1 = l.x_no_compasso(2, 1.0).expect("x");
        let x2 = l.x_no_compasso(2, 1.5).expect("x");
        assert!(x0 < x2 && x2 <= x1 + 20.0);
        assert!(x1 > x0);
        let cl = &l.compassos[2];
        assert!(x0 >= cl.x0 && x1 <= cl.x1);
        assert!(l.x_no_compasso(9999, 0.0).is_none());
    }

    #[test]
    fn clique_acha_o_compasso() {
        let p = Partitura::de_musicxml(arq(1)).expect("válida");
        let l = calcular_layout(&p, 100.0);
        let cl = &l.compassos[3];
        let s = &l.sistemas[cl.sistema];
        let achado = l.compasso_em((cl.x0 + cl.x1) / 2.0, (s.y_topo + s.y_base) / 2.0);
        assert_eq!(achado, Some(3));
        assert_eq!(l.compasso_em(-5.0, -5.0), None);
    }

    #[test]
    fn minimas_sao_ocas_e_seminimas_cheias() {
        let xml = MINIMA.replace(
            "<type>quarter</type></note>\n<note><pitch><step>B",
            "<type>half</type></note>\n<note><pitch><step>B",
        );
        let p = Partitura::de_musicxml(&xml).expect("válida");
        let c = cabecas(&calcular_layout(&p, 60.0));
        assert!(c[0].1);
        assert!(!c[1].1);
    }

    fn quadro(
        ctx: &Context,
        p: &Partitura,
        est: &mut EstadoVisual,
        zoom: f32,
        destaque: Option<Destaque>,
    ) -> usize {
        let entrada = RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1280.0, 800.0),
            )),
            ..Default::default()
        };
        let mut n = 0;
        let saida = ctx.run_ui(entrada, |ui| {
            desenhar_partitura(
                ui,
                p,
                est,
                OpcoesVisuais {
                    zoom,
                    ..OpcoesVisuais::default()
                },
                Marcas {
                    destaque,
                    selecionado: Some(1),
                    ..Marcas::default()
                },
            );
        });
        n += saida.shapes.len();
        saida.drop_without_applying_deltas();
        n
    }

    #[test]
    fn desenha_cada_partitura_a_100_e_150_por_cento_sem_panico() {
        for i in 0..3 {
            let p = Partitura::de_musicxml(arq(i)).expect("válida");
            let ctx = Context::default();
            let mut est = EstadoVisual::default();
            for zoom in [1.0f32, 1.5] {
                let a = quadro(&ctx, &p, &mut est, zoom, None);
                assert!(a > 100, "arquivo {i} zoom {zoom}: {a} formas");
                let d = Destaque {
                    compasso: 3,
                    tempo: 0.5,
                };
                quadro(&ctx, &p, &mut est, zoom, Some(d));
            }
        }
    }

    #[test]
    fn fita_e_um_unico_sistema_com_todos_os_compassos_em_x_crescente() {
        for i in 0..3 {
            let p = Partitura::de_musicxml(arq(i)).expect("válida");
            let l = calcular_layout_fita(&p);
            assert_eq!(l.sistemas.len(), 1, "arquivo {i}");
            let s = &l.sistemas[0];
            assert_eq!((s.primeiro, s.fim), (0, p.compassos.len()));
            assert_eq!(l.compassos.len(), p.compassos.len());
            let mut x = s.x0;
            for (m, cl) in l.compassos.iter().enumerate() {
                assert_eq!(cl.sistema, 0);
                assert!(cl.x1 > cl.x0, "arquivo {i} compasso {m}");
                assert!(cl.x0 >= x - 1e-3, "arquivo {i} compasso {m}: x não cresce");
                x = cl.x1;
                // Colunas dentro do compasso e em ordem crescente.
                assert!(cl.colunas.windows(2).all(|c| c[0].1 <= c[1].1));
                assert!(
                    cl.colunas
                        .iter()
                        .all(|(_, cx)| *cx >= cl.x0 && *cx <= cl.x1)
                );
            }
            assert!((s.x1 - x).abs() < 1e-3);
            assert!((l.largura - (s.x1 + MARGEM_DIREITA)).abs() < 1e-3);
            // Todas as notas com altura continuam presentes.
            let esperado: usize = p
                .partes
                .iter()
                .flat_map(|x| &x.compassos)
                .flat_map(|c| &c.notas)
                .filter(|n| n.altura.is_some())
                .count();
            assert_eq!(cabecas(&l).len(), esperado, "arquivo {i}");
        }
    }

    #[test]
    fn fita_e_mais_larga_que_a_quebra_em_sistemas() {
        let p = Partitura::de_musicxml(arq(0)).expect("válida");
        let fita = calcular_layout_fita(&p);
        let largo = calcular_layout(&p, 160.0);
        assert!(fita.largura > largo.largura);
    }

    #[test]
    fn fita_vazia_ou_minima_nao_quebra_e_cursor_e_clique_funcionam() {
        let p = Partitura::de_musicxml(MINIMA).expect("válida");
        let l = calcular_layout_fita(&p);
        assert_eq!(l.sistemas.len(), 1);
        let p = Partitura::de_musicxml(arq(1)).expect("válida");
        let l = calcular_layout_fita(&p);
        let x0 = l.x_no_compasso(2, 0.0).expect("x");
        let x1 = l.x_no_compasso(2, 1.0).expect("x");
        assert!(x1 > x0);
        let cl = &l.compassos[5];
        let s = &l.sistemas[0];
        let achado = l.compasso_em((cl.x0 + cl.x1) / 2.0, (s.y_topo + s.y_base) / 2.0);
        assert_eq!(achado, Some(5));
    }

    fn quadro_com(
        escuro: bool,
        p: &Partitura,
        est: &mut EstadoVisual,
        opc: OpcoesVisuais,
        destaque: Option<Destaque>,
    ) -> Vec<Shape> {
        let ctx = Context::default();
        let entrada = RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1280.0, 800.0),
            )),
            ..Default::default()
        };
        let mut saida = ctx.run_ui(entrada, |ui| {
            ui.visuals_mut().dark_mode = escuro;
            desenhar_partitura(
                ui,
                p,
                est,
                opc,
                Marcas {
                    destaque,
                    selecionado: Some(1),
                    ..Marcas::default()
                },
            );
        });
        let formas = std::mem::take(&mut saida.shapes)
            .into_iter()
            .map(|c| c.shape)
            .collect();
        saida.drop_without_applying_deltas();
        formas
    }

    fn cor_do_papel(formas: &[Shape], pal: &Paleta) -> bool {
        formas
            .iter()
            .any(|f| matches!(f, Shape::Rect(r) if r.fill == pal.papel))
    }

    #[test]
    fn papel_branco_usa_a_paleta_clara_mesmo_no_tema_noite() {
        let p = Partitura::de_musicxml(arq(0)).expect("válida");
        let (escura, clara) = (Paleta::para_tema(true), Paleta::para_tema(false));
        assert_eq!(clara.papel, egui::Color32::WHITE);
        let mut est = EstadoVisual::default();
        let normal = quadro_com(true, &p, &mut est, OpcoesVisuais::default(), None);
        assert!(cor_do_papel(&normal, &escura));
        assert!(!cor_do_papel(&normal, &clara));
        let branco = OpcoesVisuais {
            papel_branco: true,
            ..OpcoesVisuais::default()
        };
        let f = quadro_com(true, &p, &mut est, branco, None);
        assert!(cor_do_papel(&f, &clara));
        assert!(!cor_do_papel(&f, &escura));
        // No tema Claro o papel branco não muda nada.
        let f = quadro_com(false, &p, &mut est, branco, None);
        assert!(cor_do_papel(&f, &clara));
    }

    #[test]
    fn sobreposicoes_de_selecao_e_cursor_tem_contraste_no_papel_branco() {
        // A seleção (azul) e o cursor (âmbar) saem da paleta clara quando o papel é branco,
        // com opacidade baixa: sobre branco escurecem pouco e a tinta continua legível.
        let pal = Paleta::para_tema(false);
        for (cor, alfa) in [(pal.esquerda, 26u8), (pal.direita, 38)] {
            let a = f32::from(alfa) / 255.0;
            let mistura = |c: u8| (f32::from(c) * a + 255.0 * (1.0 - a)).round() as u8;
            let fundo = (mistura(cor.r()), mistura(cor.g()), mistura(cor.b()));
            // Ainda claro (luminância alta) e bem distante da tinta.
            let lum = 0.2126 * f32::from(fundo.0)
                + 0.7152 * f32::from(fundo.1)
                + 0.0722 * f32::from(fundo.2);
            assert!(lum > 190.0, "fundo da sobreposição muito escuro: {lum}");
        }
    }

    #[test]
    fn modos_de_rolagem_desenham_e_o_cache_distingue_o_modo() {
        let p = Partitura::de_musicxml(arq(1)).expect("válida");
        let mut est = EstadoVisual::default();
        let d = Destaque {
            compasso: 5,
            tempo: 0.5,
        };
        let sistemas = quadro_com(false, &p, &mut est, OpcoesVisuais::default(), Some(d));
        assert!(sistemas.len() > 10);
        let n_sistemas = est.ultimo_layout.as_ref().expect("layout").sistemas.len();
        assert!(n_sistemas > 1);
        let fita = OpcoesVisuais {
            modo: ModoRolagem::Fita,
            ..OpcoesVisuais::default()
        };
        let f = quadro_com(false, &p, &mut est, fita, Some(d));
        assert!(f.len() > 10);
        assert_eq!(
            est.ultimo_layout.as_ref().expect("layout").sistemas.len(),
            1
        );
        // Voltando ao modo anterior o layout com sistemas é recalculado.
        quadro_com(false, &p, &mut est, OpcoesVisuais::default(), Some(d));
        assert_eq!(
            est.ultimo_layout.as_ref().expect("layout").sistemas.len(),
            n_sistemas
        );
    }

    #[test]
    fn modo_de_rolagem_ida_e_volta_pela_chave() {
        for m in ModoRolagem::TODOS {
            assert_eq!(ModoRolagem::de_chave(m.chave()), Some(m));
        }
        assert_eq!(ModoRolagem::de_chave("outro"), None);
        assert_eq!(ModoRolagem::default(), ModoRolagem::Sistemas);
    }

    #[test]
    fn destaque_pinta_as_notas_que_soam_com_a_cor_da_mao() {
        let p = Partitura::de_musicxml(MINIMA).expect("válida");
        let l = calcular_layout(&p, 60.0);
        let pal = Paleta::para_tema(true);
        let it = l.sistemas[0]
            .itens
            .iter()
            .find(|i| matches!(i.forma, Forma::Cabeca { .. }))
            .expect("cabeça");
        let sem = cor_do_item(it, &pal, None);
        let tocando = cor_do_item(
            it,
            &pal,
            Some(Destaque {
                compasso: 0,
                tempo: 0.5,
            }),
        );
        let depois = cor_do_item(
            it,
            &pal,
            Some(Destaque {
                compasso: 0,
                tempo: 1.5,
            }),
        );
        let outro = cor_do_item(
            it,
            &pal,
            Some(Destaque {
                compasso: 1,
                tempo: 0.5,
            }),
        );
        assert_eq!(sem, pal.tinta);
        assert_eq!(tocando, pal.direita);
        assert_eq!(depois, pal.tinta);
        assert_eq!(outro, pal.tinta);
    }

    #[test]
    fn mao_esquerda_usa_a_cor_azul_na_clave_de_fa() {
        let p = Partitura::de_musicxml(arq(0)).expect("válida");
        let l = calcular_layout(&p, 100.0);
        let alvos: Vec<Mao> = l
            .sistemas
            .iter()
            .flat_map(|s| &s.itens)
            .filter_map(|i| i.alvo.map(|a| a.mao))
            .collect();
        assert!(alvos.contains(&Mao::Esquerda));
        assert!(alvos.contains(&Mao::Direita));
    }

    #[test]
    fn armadura_tem_posicoes_corretas_em_sol_e_fa() {
        // Sol maior: Fá♯ na linha superior (s = 8). Fá menor/bemóis: Si♭ na linha do meio (s = 4).
        assert_eq!(posicoes_armadura(1, Clave::SOL), vec![8]);
        assert_eq!(posicoes_armadura(-1, Clave::SOL), vec![4]);
        assert_eq!(posicoes_armadura(1, Clave::FA), vec![6]);
        assert_eq!(posicoes_armadura(-2, Clave::FA), vec![2, 5]);
        assert_eq!(posicoes_armadura(3, Clave::SOL), vec![8, 5, 9]);
    }

    #[test]
    fn pauta_vazia_ou_partitura_minima_nao_quebra() {
        let xml = r#"<score-partwise><part-list><score-part id="P1"><part-name>x</part-name></score-part></part-list><part id="P1"><measure number="1"/></part></score-partwise>"#;
        let p = Partitura::de_musicxml(xml).expect("válida");
        let l = calcular_layout(&p, 50.0);
        assert_eq!(l.sistemas.len(), 1);
        let ctx = Context::default();
        quadro(&ctx, &p, &mut EstadoVisual::default(), 1.0, None);
    }

    /// Despeja o quadro como triângulos em JSON, para inspeção visual fora do app. Só roda
    /// com `PT_PARTITURA_DUMP=<pasta>`; sem a variável, não faz nada.
    #[test]
    fn despeja_quadros_para_inspecao_visual() {
        let Ok(pasta) = std::env::var("PT_PARTITURA_DUMP") else {
            return;
        };
        for (i, nome) in ["wiegenlied", "heidenroeslein", "barcarolle"]
            .iter()
            .enumerate()
        {
            let p = Partitura::de_musicxml(arq(i)).expect("válida");
            for zoom in [1.0f32, 1.5] {
                let ctx = Context::default();
                let mut est = EstadoVisual::default();
                let entrada = RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::pos2(0.0, 0.0),
                        egui::vec2(1100.0, 4000.0),
                    )),
                    ..Default::default()
                };
                let mut saida = ctx.run_ui(entrada, |ui| {
                    ui.visuals_mut().dark_mode = std::env::var("PT_PARTITURA_ESCURO").is_ok();
                    desenhar_partitura(
                        ui,
                        &p,
                        &mut est,
                        OpcoesVisuais {
                            zoom,
                            seguir: false,
                            papel_branco: std::env::var("PT_PARTITURA_PAPEL_BRANCO").is_ok(),
                            ..OpcoesVisuais::default()
                        },
                        Marcas {
                            destaque: Some(Destaque {
                                compasso: 3,
                                tempo: 0.5,
                            }),
                            selecionado: None,
                            ..Marcas::default()
                        },
                    );
                });
                saida.shapes.retain(|c| !matches!(c.shape, Shape::Text(_)));
                let meshes = ctx.tessellate(std::mem::take(&mut saida.shapes), 1.0);
                let mut tris: Vec<[f32; 10]> = Vec::new();
                for m in &meshes {
                    if let bancada_casca::egui::epaint::Primitive::Mesh(mesh) = &m.primitive {
                        for t in mesh.indices.chunks(3) {
                            let v: Vec<_> = t.iter().map(|k| mesh.vertices[*k as usize]).collect();
                            let c = v[0].color;
                            tris.push([
                                v[0].pos.x,
                                v[0].pos.y,
                                v[1].pos.x,
                                v[1].pos.y,
                                v[2].pos.x,
                                v[2].pos.y,
                                f32::from(c.r()),
                                f32::from(c.g()),
                                f32::from(c.b()),
                                f32::from(c.a()),
                            ]);
                        }
                    }
                }
                let layout = est.ultimo_layout.clone().expect("layout");
                let esp = ESP_BASE_PX * zoom;
                let mut textos: Vec<(f32, f32, String, f32)> = Vec::new();
                for s in &layout.sistemas {
                    for it in &s.itens {
                        if let Forma::Texto {
                            pos, texto, altura, ..
                        } = &it.forma
                        {
                            textos.push((pos.x * esp, pos.y * esp, texto.clone(), *altura * esp));
                        }
                    }
                }
                let json = format!(
                    "{{\"tris\":{:?},\"textos\":{:?},\"altura\":{}}}",
                    tris,
                    textos,
                    layout.altura * esp
                );
                let json = json.replace('(', "[").replace(')', "]");
                std::fs::write(
                    format!("{pasta}/{nome}-{}.json", (zoom * 100.0) as u32),
                    json,
                )
                .expect("gravar");
            }
        }
    }

    // --- Golden de primitivas (ticket 06) ---------------------------------------------------
    // Assinaturas FNV-1a do layout (formas em sp, por bits) e dos triângulos tesselados do egui
    // (sem texto), das partituras embutidas a 100% e 150%. Capturadas ANTES da migração para a
    // camada geométrica própria: qualquer mudança no desenho muda a assinatura.

    struct Fnv(u64);
    impl Fnv {
        fn novo() -> Self {
            Self(0xcbf2_9ce4_8422_2325)
        }
        fn u64(&mut self, v: u64) {
            for b in v.to_le_bytes() {
                self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        fn f(&mut self, v: f32) {
            self.u64(u64::from(v.to_bits()));
        }
        fn p(&mut self, p: Ponto) {
            self.f(p.x);
            self.f(p.y);
        }
    }

    fn ancora_id(a: Ancora) -> u64 {
        if a == Ancora::CENTER_CENTER {
            1
        } else if a == Ancora::LEFT_BOTTOM {
            2
        } else if a == Ancora::LEFT_TOP {
            3
        } else {
            99
        }
    }

    fn assinatura_item(it: &Item) -> u64 {
        let mut h = Fnv::novo();
        h.u64(match it.cor {
            Cor::Tinta => 1,
            Cor::Suave => 2,
        });
        match &it.alvo {
            None => h.u64(0),
            Some(a) => {
                h.u64(1);
                h.u64(a.compasso as u64);
                h.u64(a.inicio.to_bits());
                h.u64(a.fim.to_bits());
                h.u64(match a.mao {
                    Mao::Direita => 1,
                    Mao::Esquerda => 2,
                });
            }
        }
        match &it.forma {
            Forma::Linha { a, b, espessura } => {
                h.u64(10);
                h.p(*a);
                h.p(*b);
                h.f(*espessura);
            }
            Forma::Preenchido { pontos } => {
                h.u64(11);
                pontos.iter().for_each(|q| h.p(*q));
            }
            Forma::Caminho { pontos, espessura } => {
                h.u64(12);
                pontos.iter().for_each(|q| h.p(*q));
                h.f(*espessura);
            }
            Forma::Cabeca {
                centro,
                rx,
                ry,
                rotacao,
                oca,
                rotacao_oco,
            } => {
                h.u64(13);
                h.p(*centro);
                h.f(*rx);
                h.f(*ry);
                h.f(*rotacao);
                h.u64(u64::from(*oca));
                h.f(*rotacao_oco);
            }
            Forma::Disco { centro, raio } => {
                h.u64(14);
                h.p(*centro);
                h.f(*raio);
            }
            Forma::Texto {
                pos,
                texto,
                altura,
                ancora,
            } => {
                h.u64(15);
                h.p(*pos);
                texto.bytes().for_each(|b| h.u64(u64::from(b)));
                h.f(*altura);
                h.u64(ancora_id(*ancora));
            }
            Forma::Glifo {
                pos,
                caractere,
                escala,
            } => {
                h.u64(16);
                h.p(*pos);
                h.u64(u64::from(*caractere as u32));
                h.f(*escala);
            }
        }
        h.0
    }

    fn assinatura_layout(l: &Layout) -> u64 {
        let mut h = Fnv::novo();
        h.f(l.largura);
        h.f(l.altura);
        for s in &l.sistemas {
            h.f(s.y_topo);
            h.f(s.y_base);
            h.f(s.x0);
            h.f(s.x1);
            h.u64(s.primeiro as u64);
            h.u64(s.fim as u64);
            // Soma comutativa: independe da ordem de desenho de itens equivalentes.
            let mut soma = 0u64;
            for it in &s.itens {
                soma = soma.wrapping_add(assinatura_item(it));
            }
            h.u64(soma);
        }
        for c in &l.compassos {
            h.u64(c.sistema as u64);
            h.f(c.x0);
            h.f(c.x1);
            h.u64(c.duracao.to_bits());
            for (t, x) in &c.colunas {
                h.u64(t.to_bits());
                h.f(*x);
            }
        }
        h.0
    }

    fn assinatura_malha(p: &Partitura, zoom: f32) -> (u64, usize) {
        let ctx = Context::default();
        let mut est = EstadoVisual::default();
        let entrada = RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1100.0, 4000.0),
            )),
            ..Default::default()
        };
        let mut saida = ctx.run_ui(entrada, |ui| {
            ui.visuals_mut().dark_mode = false;
            desenhar_partitura(
                ui,
                p,
                &mut est,
                OpcoesVisuais {
                    zoom,
                    seguir: false,
                    ..OpcoesVisuais::default()
                },
                Marcas {
                    destaque: Some(Destaque {
                        compasso: 3,
                        tempo: 0.5,
                    }),
                    selecionado: None,
                    ..Marcas::default()
                },
            );
        });
        saida.shapes.retain(|c| !matches!(c.shape, Shape::Text(_)));
        saida.textures_delta.clear();
        let meshes = ctx.tessellate(std::mem::take(&mut saida.shapes), 1.0);
        let mut soma = 0u64;
        let mut n = 0;
        for m in &meshes {
            if let bancada_casca::egui::epaint::Primitive::Mesh(mesh) = &m.primitive {
                for t in mesh.indices.chunks(3) {
                    let mut h = Fnv::novo();
                    for i in t {
                        let v = mesh.vertices[*i as usize];
                        h.f(v.pos.x);
                        h.f(v.pos.y);
                        h.u64(u64::from(u32::from_le_bytes(v.color.to_array())));
                    }
                    soma = soma.wrapping_add(h.0);
                    n += 1;
                }
            }
        }
        let mut h = Fnv::novo();
        h.u64(soma);
        (h.0, n)
    }

    /// (layout 100%, malha 100%, layout 150%, malha 150%) por partitura embutida.
    const GOLDEN: [(u64, u64, u64, u64); 5] = [
        (
            1024020949450510958,
            15424379957843765398,
            16889023980630175934,
            11783567225301403596,
        ),
        (
            3092051816611255550,
            1702561511137555275,
            9294272831556289290,
            11106701511911798202,
        ),
        (
            1566174185879285142,
            11368826779549470146,
            5499259382719335217,
            7184184929903766057,
        ),
        (
            2199799701033366023,
            15406921808439581487,
            18010821381813141312,
            5940283737368385173,
        ),
        (
            18174550916654093645,
            17252801736928073200,
            17418052786301984108,
            6144444238221826126,
        ),
    ];

    #[test]
    fn golden_de_primitivas_das_partituras_embutidas() {
        let fontes = [
            arq(0),
            arq(1),
            arq(2),
            include_str!("../../../assets/partituras/beethoven-ode-a-alegria-tema.musicxml"),
            include_str!(
                "../../../assets/partituras/tradicional-brilha-brilha-estrelinha.musicxml"
            ),
        ];
        for (i, f) in fontes.iter().enumerate() {
            let p = Partitura::de_musicxml(f).expect("válida");
            let mut obtido = [0u64; 4];
            for (k, zoom) in [1.0f32, 1.5].into_iter().enumerate() {
                // Largura do layout em sp como o desenho a usa (1100 px menos margens).
                let esp = ESP_BASE_PX * zoom;
                obtido[k * 2] = assinatura_layout(&calcular_layout(&p, 1000.0 / esp));
                obtido[k * 2 + 1] = assinatura_malha(&p, zoom).0;
            }
            eprintln!("GOLDEN {i}: {obtido:?}");
            let g = GOLDEN[i];
            assert_eq!(obtido, [g.0, g.1, g.2, g.3], "partitura {i}");
        }
    }

    fn cabeca_com_id(centro: Ponto, id: u64) -> Item {
        Item {
            forma: Forma::Cabeca {
                centro,
                rx: 0.64,
                ry: 0.45,
                rotacao: -0.38,
                oca: false,
                rotacao_oco: -0.55,
            },
            cor: Cor::Tinta,
            alvo: None,
            nota: Some(IdNota(id)),
        }
    }

    fn layout_com_ids() -> Layout {
        let p = Partitura::de_musicxml(MINIMA).expect("válida");
        let mut l = calcular_layout(&p, 60.0);
        let y = l.sistemas[0].y_topo + 2.0;
        l.sistemas[0].itens.push(cabeca_com_id(ponto(20.0, y), 7));
        l.sistemas[0].itens.push(cabeca_com_id(ponto(21.2, y), 8));
        l
    }

    #[test]
    fn hit_test_acha_nota_por_identidade_dentro_e_fora_da_tolerancia() {
        let l = layout_com_ids();
        let y = l.sistemas[0].y_topo + 2.0;
        assert_eq!(l.nota_em(ponto(20.0, y), 0.0), Some(IdNota(7)));
        // Logo além do raio horizontal (0,64): só a tolerância o inclui.
        let fora = ponto(20.0 - 0.8, y);
        assert_eq!(l.nota_em(fora, 0.0), None);
        assert_eq!(l.nota_em(fora, TOLERANCIA_NOTA_SP), Some(IdNota(7)));
        // Longe de tudo.
        assert_eq!(l.nota_em(ponto(20.0, y + 3.0), TOLERANCIA_NOTA_SP), None);
    }

    #[test]
    fn hit_test_desempata_pela_mais_proxima() {
        let l = layout_com_ids();
        let y = l.sistemas[0].y_topo + 2.0;
        assert_eq!(l.nota_em(ponto(20.2, y), 0.5), Some(IdNota(7)));
        assert_eq!(l.nota_em(ponto(21.0, y), 0.5), Some(IdNota(8)));
    }

    #[test]
    fn layout_com_ids_carrega_a_identidade_de_cada_cabeca_de_nota() {
        use piano_tutor_core::documento::DocumentoAberto;
        let fontes = [
            MINIMA,
            arq(0),
            arq(1),
            arq(2),
            include_str!("../../../assets/partituras/beethoven-ode-a-alegria-tema.musicxml"),
            include_str!(
                "../../../assets/partituras/tradicional-brilha-brilha-estrelinha.musicxml"
            ),
        ];
        for (i, f) in fontes.iter().enumerate() {
            let d = DocumentoAberto::abrir_texto("t", f).expect("abre");
            for fita in [false, true] {
                let l = if fita {
                    calcular_layout_fita_com_ids(d.partitura(), d.ids())
                } else {
                    calcular_layout_com_ids(d.partitura(), d.ids(), 100.0)
                };
                let mut das_cabecas: Vec<IdNota> = Vec::new();
                for it in l.sistemas.iter().flat_map(|s| &s.itens) {
                    if matches!(it.forma, Forma::Cabeca { .. }) {
                        das_cabecas.push(it.nota.expect("toda cabeça carrega a identidade"));
                    }
                }
                let mut esperadas: Vec<IdNota> = Vec::new();
                for (pi, parte) in d.partitura().partes.iter().enumerate() {
                    for (ci, c) in parte.compassos.iter().enumerate() {
                        for (oi, n) in c.notas.iter().enumerate() {
                            if n.altura.is_some() {
                                esperadas.push(d.ids().id(pi, ci, oi).expect("id"));
                            }
                        }
                    }
                }
                das_cabecas.sort();
                esperadas.sort();
                assert_eq!(das_cabecas, esperadas, "fonte {i} fita {fita}");
                // O hit test devolve a identidade da cabeça sob o ponto.
                let cab = l
                    .sistemas
                    .iter()
                    .flat_map(|s| &s.itens)
                    .find_map(|it| match it.forma {
                        Forma::Cabeca { centro, .. } => Some((centro, it.nota)),
                        _ => None,
                    })
                    .expect("há cabeça");
                assert_eq!(l.nota_em(cab.0, 0.0), cab.1, "fonte {i}");
            }
        }
    }

    #[test]
    fn layout_com_ids_so_acrescenta_a_identidade_sem_mudar_a_geometria() {
        use piano_tutor_core::documento::DocumentoAberto;
        for f in [MINIMA, arq(0), arq(2)] {
            let d = DocumentoAberto::abrir_texto("t", f).expect("abre");
            let sem = calcular_layout(d.partitura(), 100.0);
            let com = calcular_layout_com_ids(d.partitura(), d.ids(), 100.0);
            assert_eq!(assinatura_layout(&sem), assinatura_layout(&com));
            assert_eq!(sem.largura, com.largura);
            assert_eq!(sem.altura, com.altura);
        }
    }

    #[test]
    fn sem_identidade_o_acerto_cai_no_compasso_como_antes() {
        let p = Partitura::de_musicxml(MINIMA).expect("válida");
        let l = calcular_layout(&p, 60.0);
        assert!(
            l.sistemas
                .iter()
                .flat_map(|s| &s.itens)
                .all(|i| i.nota.is_none())
        );
        let c = &l.compassos[0];
        let q = ponto((c.x0 + c.x1) / 2.0, l.sistemas[0].y_topo + 2.0);
        assert_eq!(l.nota_em(q, TOLERANCIA_NOTA_SP), None);
        assert_eq!(l.acertar(q, TOLERANCIA_NOTA_SP), Some(Acerto::Compasso(0)));
        assert_eq!(
            l.acertar(q, TOLERANCIA_NOTA_SP),
            l.compasso_em(q.x, q.y).map(Acerto::Compasso)
        );
        // Fora de qualquer sistema.
        assert_eq!(l.acertar(ponto(-50.0, -50.0), TOLERANCIA_NOTA_SP), None);
    }

    #[test]
    fn cor_da_selecao_tem_contraste_minimo_documentado_contra_o_papel_em_todos_os_temas() {
        use crate::selecao_partitura::{
            CONTRASTE_MINIMO, CONTRASTE_MINIMO_ALTO, razao_de_contraste,
        };
        let rgb = |c: bancada_casca::egui::Color32| [c.r(), c.g(), c.b()];
        for escuro in [true, false] {
            for alto in [false, true] {
                let pal = Paleta::para_tema_com(escuro, alto);
                let minimo = if alto {
                    CONTRASTE_MINIMO_ALTO
                } else {
                    CONTRASTE_MINIMO
                };
                let r = razao_de_contraste(rgb(pal.selecao), rgb(pal.papel));
                assert!(
                    r >= minimo,
                    "escuro={escuro} alto_contraste={alto}: contraste {r:.2} < {minimo}"
                );
                // A seleção não pode ser confundida com nenhuma outra cor de significado.
                for outra in [pal.direita, pal.esquerda, pal.acerto, pal.erro, pal.tinta] {
                    assert_ne!(pal.selecao, outra, "escuro={escuro} alto={alto}");
                }
            }
        }
        // O alto contraste só aumenta o contraste da seleção e não mexe no resto da paleta.
        for escuro in [true, false] {
            let (n, a) = (
                Paleta::para_tema(escuro),
                Paleta::para_tema_com(escuro, true),
            );
            assert!(
                razao_de_contraste(rgb(a.selecao), rgb(a.papel))
                    > razao_de_contraste(rgb(n.selecao), rgb(n.papel))
            );
            assert_eq!(
                (
                    n.papel, n.tinta, n.suave, n.direita, n.esquerda, n.acerto, n.erro
                ),
                (
                    a.papel, a.tinta, a.suave, a.direita, a.esquerda, a.acerto, a.erro
                )
            );
        }
        assert_eq!(Paleta::para_tema(true), Paleta::para_tema_com(true, false));
    }

    #[test]
    fn nota_selecionada_pinta_com_a_cor_da_selecao_e_so_ela() {
        use piano_tutor_core::documento::DocumentoAberto;
        let d = DocumentoAberto::abrir_texto("t", MINIMA).expect("abre");
        let l = calcular_layout_com_ids(d.partitura(), d.ids(), 80.0);
        let cabecas: Vec<&Item> = l
            .sistemas
            .iter()
            .flat_map(|s| &s.itens)
            .filter(|i| matches!(i.forma, Forma::Cabeca { .. }) && i.nota.is_some())
            .collect();
        assert!(cabecas.len() >= 2);
        let pal = Paleta::para_tema(true);
        let sel = cabecas[0].nota.expect("id");
        let marcas = Marcas {
            selecao: Some(MarcaSelecao {
                nota: Some(sel),
                ..MarcaSelecao::default()
            }),
            ..Marcas::default()
        };
        assert_eq!(cor_final(cabecas[0], &pal, marcas), pal.selecao);
        assert_ne!(cor_final(cabecas[1], &pal, marcas), pal.selecao);
        // Sem seleção, ninguém ganha a cor da seleção.
        assert_ne!(cor_final(cabecas[0], &pal, Marcas::default()), pal.selecao);
    }

    #[test]
    fn cabeca_da_nota_acha_o_sistema_e_o_centro_ou_nada() {
        let l = layout_com_ids();
        let y = l.sistemas[0].y_topo + 2.0;
        assert_eq!(l.cabeca_da_nota(IdNota(8)), Some((0, ponto(21.2, y))));
        assert_eq!(l.cabeca_da_nota(IdNota(404)), None);
        // Sem identidades no layout, nenhuma cabeça é achada.
        let p = Partitura::de_musicxml(MINIMA).expect("válida");
        assert_eq!(calcular_layout(&p, 60.0).cabeca_da_nota(IdNota(1)), None);
    }

    #[test]
    fn acerto_prefere_a_nota_ao_compasso() {
        let l = layout_com_ids();
        let y = l.sistemas[0].y_topo + 2.0;
        assert_eq!(
            l.acertar(ponto(20.0, y), TOLERANCIA_NOTA_SP),
            Some(Acerto::Nota(IdNota(7)))
        );
    }

    #[test]
    fn layout_nunca_produz_coordenada_nao_finita() {
        let fontes = [
            arq(0),
            arq(1),
            arq(2),
            include_str!("../../../assets/partituras/beethoven-ode-a-alegria-tema.musicxml"),
            include_str!(
                "../../../assets/partituras/tradicional-brilha-brilha-estrelinha.musicxml"
            ),
            MINIMA,
        ];
        for (i, f) in fontes.iter().enumerate() {
            let p = Partitura::de_musicxml(f).expect("válida");
            let mut layouts: Vec<Layout> = [0.0f32, 1.0, 10.0, 30.0, 60.0, 120.0, 400.0]
                .iter()
                .map(|w| calcular_layout(&p, *w))
                .collect();
            layouts.push(calcular_layout_fita(&p));
            for (k, l) in layouts.iter().enumerate() {
                for it in l.sistemas.iter().flat_map(|s| &s.itens) {
                    assert!(
                        forma_finita(&it.forma),
                        "fonte {i} layout {k}: {:?}",
                        it.forma
                    );
                }
                assert!(
                    l.largura.is_finite() && l.altura.is_finite(),
                    "fonte {i} layout {k}"
                );
            }
        }
    }

    #[test]
    fn resultados_da_pratica_pintam_verde_e_vermelho_e_acorde_so_fica_verde_se_todas_acertam() {
        let p = Partitura::de_musicxml(arq(0)).expect("válida");
        let l = calcular_layout(&p, 100.0);
        let pal = Paleta::para_tema(true);
        let it = l.sistemas[0]
            .itens
            .iter()
            .find(|i| matches!(i.forma, Forma::Cabeca { .. }) && i.alvo.is_some())
            .expect("cabeça");
        let a = it.alvo.expect("alvo");
        let mut m = MapaResultados::default();
        assert_eq!(cor_do_item_com(it, &pal, None, Some(&m)), pal.tinta);
        m.registrar(a.compasso, a.inicio, a.mao, Some(ResultadoNota::Acertou));
        assert_eq!(cor_do_item_com(it, &pal, None, Some(&m)), pal.acerto);
        // Resultado vence o destaque do cursor; sem resultado, o destaque segue valendo.
        let d = Destaque {
            compasso: a.compasso,
            tempo: a.inicio,
        };
        assert_eq!(cor_do_item_com(it, &pal, Some(d), Some(&m)), pal.acerto);
        assert_eq!(cor_do_item_com(it, &pal, Some(d), None), pal.mao(a.mao));
        // Segunda nota do mesmo acorde ainda pendente: sem marca; errada: vermelho.
        m.registrar(a.compasso, a.inicio, a.mao, None);
        assert_eq!(m.de(&a), None);
        m.registrar(a.compasso, a.inicio, a.mao, Some(ResultadoNota::Errou));
        assert_eq!(cor_do_item_com(it, &pal, None, Some(&m)), pal.erro);
        assert_ne!(pal.acerto, pal.erro);
        // Outra mão na mesma posição não é afetada.
        let outra = Alvo {
            mao: if a.mao == Mao::Direita {
                Mao::Esquerda
            } else {
                Mao::Direita
            },
            ..a
        };
        assert_eq!(m.de(&outra), None);
    }
}
