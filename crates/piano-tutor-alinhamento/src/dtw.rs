//! DTW (dynamic time warping) com passos (1,1), (1,0) e (0,1), pesos, banda de Sakoe-Chiba e
//! modo "subsequência".
//!
//! Convenção: linhas `i` = sequência de referência (a partitura, `n` quadros); colunas `j` = a
//! sequência longa (o áudio, `m` quadros). O custo local vem de uma função `custo(i, j)`, então
//! nenhuma matriz de custo é guardada; só as direções (1 byte por célula) e duas linhas de
//! custo acumulado.
//!
//! - passo diagonal `(i-1, j-1) -> (i, j)`: soma `peso_diagonal * custo(i, j)`;
//! - passo vertical `(i-1, j) -> (i, j)` (a partitura avança, o áudio não): `peso_vertical`;
//! - passo horizontal `(i, j-1) -> (i, j)` (o áudio avança, a partitura não): `peso_horizontal`.
//!
//! Os passos vertical e horizontal ainda somam `penalidade_lateral` cada um.
//!
//! Modo global: o caminho vai de `(0, 0)` a `(n-1, m-1)`. Modo subsequência: o caminho começa
//! em qualquer coluna da linha 0 e termina na coluna de menor custo da última linha; assim a
//! partitura inteira é encontrada **dentro** de um áudio maior (fala antes, silêncio depois).

use std::fmt;

/// Limite de células `n * m` (1 byte cada) para não estourar a memória.
pub const MAX_CELULAS: usize = 800_000_000;

const INF: f32 = f32::INFINITY;

#[derive(Clone, Debug, PartialEq)]
pub struct ConfigDtw {
    pub peso_diagonal: f32,
    pub peso_vertical: f32,
    pub peso_horizontal: f32,
    /// Penalidade somada a cada passo vertical ou horizontal. Em trechos de custo quase zero
    /// (nota sustentada) ela faz o caminho andar na diagonal em vez de ficar parado numa
    /// coluna, o que deixaria o instante do início da nota arbitrário.
    pub penalidade_lateral: f32,
    /// Banda de Sakoe-Chiba em colunas em torno da diagonal `i * (m-1)/(n-1)`; só no modo global.
    pub banda: Option<usize>,
    /// Início e fim livres na sequência longa.
    pub subsequencia: bool,
}

impl Default for ConfigDtw {
    /// Passo diagonal conta em dobro (simétrico) e os passos laterais custam um pouco mais que
    /// metade disso, o que favorece a diagonal sem proibir andamentos diferentes.
    fn default() -> Self {
        Self {
            peso_diagonal: 2.0,
            peso_vertical: 1.2,
            peso_horizontal: 1.2,
            penalidade_lateral: 0.05,
            banda: None,
            subsequencia: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ErroDtw {
    SequenciaVazia,
    PesoInvalido,
    BandaComSubsequencia,
    MuitoGrande { celulas: usize },
    SemCaminho,
}

impl fmt::Display for ErroDtw {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SequenciaVazia => write!(f, "sequência vazia no DTW"),
            Self::PesoInvalido => write!(f, "os pesos do DTW precisam ser finitos e positivos"),
            Self::BandaComSubsequencia => {
                write!(f, "a banda de Sakoe-Chiba não vale no modo subsequência")
            }
            Self::MuitoGrande { celulas } => write!(
                f,
                "o DTW pediria {celulas} células (máximo {MAX_CELULAS}); restrinja o trecho do áudio"
            ),
            Self::SemCaminho => write!(f, "nenhum caminho viável (banda estreita demais?)"),
        }
    }
}

impl std::error::Error for ErroDtw {}

/// Caminho de alinhamento, do início ao fim.
#[derive(Clone, Debug, PartialEq)]
pub struct CaminhoDtw {
    /// Pares `(i, j)` em ordem; `i` e `j` nunca diminuem.
    pub passos: Vec<(usize, usize)>,
    /// Custo acumulado total (ponderado).
    pub custo: f32,
}

impl CaminhoDtw {
    /// Para cada linha `i` (0..n), a primeira coluna `j` do caminho nessa linha.
    pub fn primeira_coluna_por_linha(&self, n: usize) -> Vec<usize> {
        let mut r = vec![usize::MAX; n];
        for &(i, j) in &self.passos {
            if i < n && r[i] == usize::MAX {
                r[i] = j;
            }
        }
        r
    }
}

const DIAG: u8 = 0;
const VERT: u8 = 1;
const HORIZ: u8 = 2;
const INICIO: u8 = 3;

/// Calcula o caminho de menor custo entre `n` linhas e `m` colunas.
pub fn dtw(
    n: usize,
    m: usize,
    custo: impl Fn(usize, usize) -> f32,
    cfg: &ConfigDtw,
) -> Result<CaminhoDtw, ErroDtw> {
    if n == 0 || m == 0 {
        return Err(ErroDtw::SequenciaVazia);
    }
    let pesos = [cfg.peso_diagonal, cfg.peso_vertical, cfg.peso_horizontal];
    if pesos.iter().any(|p| !(p.is_finite() && *p > 0.0)) {
        return Err(ErroDtw::PesoInvalido);
    }
    if cfg.banda.is_some() && cfg.subsequencia {
        return Err(ErroDtw::BandaComSubsequencia);
    }
    let celulas = n.saturating_mul(m);
    if celulas > MAX_CELULAS {
        return Err(ErroDtw::MuitoGrande { celulas });
    }

    let mut dir = vec![INICIO; celulas];
    let mut anterior = vec![INF; m];
    let mut atual = vec![INF; m];
    for i in 0..n {
        let (j_min, j_max) = match cfg.banda {
            Some(b) => {
                let centro = if n > 1 {
                    i as f64 * (m - 1) as f64 / (n - 1) as f64
                } else {
                    0.0
                };
                let lo = (centro - b as f64).ceil().max(0.0) as usize;
                let hi = ((centro + b as f64).floor() as usize).min(m - 1);
                (lo, hi)
            }
            None => (0, m - 1),
        };
        atual.fill(INF);
        if j_min <= j_max {
            for j in j_min..=j_max {
                let c = custo(i, j);
                let mut melhor = INF;
                let mut d = INICIO;
                if i == 0 {
                    if cfg.subsequencia || j == 0 {
                        melhor = cfg.peso_diagonal * c;
                    }
                } else {
                    if j > 0 && anterior[j - 1].is_finite() {
                        melhor = anterior[j - 1] + cfg.peso_diagonal * c;
                        d = DIAG;
                    }
                    if anterior[j].is_finite() {
                        let v = anterior[j] + cfg.peso_vertical * c + cfg.penalidade_lateral;
                        if v < melhor {
                            melhor = v;
                            d = VERT;
                        }
                    }
                }
                if j > 0 && atual[j - 1].is_finite() {
                    let v = atual[j - 1] + cfg.peso_horizontal * c + cfg.penalidade_lateral;
                    if v < melhor {
                        melhor = v;
                        d = HORIZ;
                    }
                }
                atual[j] = melhor;
                dir[i * m + j] = d;
            }
        }
        std::mem::swap(&mut anterior, &mut atual);
    }
    // Depois do último swap, `anterior` guarda a última linha.
    let j_fim = if cfg.subsequencia {
        anterior
            .iter()
            .enumerate()
            .filter(|(_, v)| v.is_finite())
            .min_by(|a, b| a.1.total_cmp(b.1))
            .map(|(j, _)| j)
    } else if anterior[m - 1].is_finite() {
        Some(m - 1)
    } else {
        None
    };
    let j_fim = j_fim.ok_or(ErroDtw::SemCaminho)?;
    let total = anterior[j_fim];

    let (mut i, mut j) = (n - 1, j_fim);
    let mut passos = vec![(i, j)];
    loop {
        match dir[i * m + j] {
            DIAG => {
                i -= 1;
                j -= 1;
            }
            VERT => i -= 1,
            HORIZ => j -= 1,
            _ => break,
        }
        passos.push((i, j));
    }
    passos.reverse();
    Ok(CaminhoDtw {
        passos,
        custo: total,
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn custo_abs<'a>(a: &'a [f32], b: &'a [f32]) -> impl Fn(usize, usize) -> f32 + 'a {
        move |i, j| (a[i] - b[j]).abs()
    }

    fn simples() -> ConfigDtw {
        ConfigDtw {
            peso_diagonal: 1.0,
            peso_vertical: 1.0,
            peso_horizontal: 1.0,
            penalidade_lateral: 0.0,
            banda: None,
            subsequencia: false,
        }
    }

    #[test]
    fn sequencias_iguais_dao_a_diagonal_com_custo_zero() {
        let a = [1.0, 2.0, 3.0, 2.0];
        let r = dtw(4, 4, custo_abs(&a, &a), &simples()).expect("viável");
        assert_eq!(r.passos, vec![(0, 0), (1, 1), (2, 2), (3, 3)]);
        assert_eq!(r.custo, 0.0);
    }

    #[test]
    fn alonga_a_nota_repetida_a_mao() {
        // a = 1 2 3; b = 1 1 2 2 2 3: o caminho ótimo repete 1 e 2 no eixo b.
        let a = [1.0, 2.0, 3.0];
        let b = [1.0, 1.0, 2.0, 2.0, 2.0, 3.0];
        let r = dtw(3, 6, custo_abs(&a, &b), &simples()).expect("viável");
        assert_eq!(r.custo, 0.0);
        assert_eq!(r.passos.first(), Some(&(0, 0)));
        assert_eq!(r.passos.last(), Some(&(2, 5)));
        assert_eq!(r.primeira_coluna_por_linha(3), vec![0, 2, 5]);
        assert_eq!(r.passos.len(), 6);
    }

    #[test]
    fn comprime_quando_a_referencia_e_mais_longa() {
        let a = [1.0, 1.0, 2.0, 2.0, 3.0];
        let b = [1.0, 2.0, 3.0];
        let r = dtw(5, 3, custo_abs(&a, &b), &simples()).expect("viável");
        assert_eq!(r.custo, 0.0);
        assert_eq!(r.primeira_coluna_por_linha(5), vec![0, 0, 1, 1, 2]);
    }

    #[test]
    fn custo_com_diferenca_e_calculado_a_mao() {
        // a = 0 2, b = 1 3: unico caminho diagonal custa |0-1| + |2-3| = 2; as alternativas
        // passam por mais células e custam mais.
        let a = [0.0, 2.0];
        let b = [1.0, 3.0];
        let r = dtw(2, 2, custo_abs(&a, &b), &simples()).expect("viável");
        assert_eq!(r.custo, 2.0);
        assert_eq!(r.passos, vec![(0, 0), (1, 1)]);
    }

    #[test]
    fn subsequencia_acha_o_trecho_no_meio_de_uma_sequencia_longa() {
        let a = [5.0, 6.0, 7.0];
        let b = [0.0, 0.0, 9.0, 5.0, 6.0, 7.0, 0.0, 0.0];
        let cfg = ConfigDtw {
            subsequencia: true,
            ..simples()
        };
        let r = dtw(3, 8, custo_abs(&a, &b), &cfg).expect("viável");
        assert_eq!(r.passos, vec![(0, 3), (1, 4), (2, 5)]);
        assert_eq!(r.custo, 0.0);
        // No modo global o mesmo problema paga o preço dos extremos.
        let g = dtw(3, 8, custo_abs(&a, &b), &simples()).expect("viável");
        assert!(g.custo > 0.0);
    }

    #[test]
    fn subsequencia_com_tempo_mais_lento() {
        let a = [5.0, 6.0, 7.0];
        let b = [0.0, 5.0, 5.0, 6.0, 6.0, 6.0, 7.0, 0.0];
        let cfg = ConfigDtw {
            subsequencia: true,
            ..simples()
        };
        let r = dtw(3, 8, custo_abs(&a, &b), &cfg).expect("viável");
        assert_eq!(r.custo, 0.0);
        // A linha 0 casa com as colunas 1 e 2 (custo igual); o desempate fica com o início tardio.
        assert_eq!(r.primeira_coluna_por_linha(3), vec![2, 3, 6]);
    }

    #[test]
    fn banda_de_sakoe_chiba_restringe_o_caminho() {
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let b = [1.0, 1.0, 1.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 6.0];
        let livre = dtw(6, 10, custo_abs(&a, &b), &simples()).expect("viável");
        assert_eq!(livre.custo, 0.0);
        let estreita = ConfigDtw {
            banda: Some(1),
            ..simples()
        };
        let r = dtw(6, 10, custo_abs(&a, &b), &estreita).expect("viável");
        assert!(r.custo > 0.0, "a banda impede o desvio grande");
        for &(i, j) in &r.passos {
            let centro = i as f64 * 9.0 / 5.0;
            assert!((j as f64 - centro).abs() <= 1.0 + 1e-9, "({i},{j})");
        }
    }

    #[test]
    fn banda_estreita_demais_nao_tem_caminho() {
        let a = [1.0, 2.0];
        let b = [1.0; 30];
        let cfg = ConfigDtw {
            banda: Some(0),
            ..simples()
        };
        // Com banda 0 só a diagonal "esticada" existe, mas ela pula colunas: sem passo permitido.
        assert_eq!(
            dtw(2, 30, custo_abs(&a, &b), &cfg),
            Err(ErroDtw::SemCaminho)
        );
    }

    #[test]
    fn entradas_invalidas() {
        let f = |_: usize, _: usize| 0.0;
        assert_eq!(dtw(0, 3, f, &simples()), Err(ErroDtw::SequenciaVazia));
        let ruim = ConfigDtw {
            peso_diagonal: 0.0,
            ..simples()
        };
        assert_eq!(dtw(2, 2, f, &ruim), Err(ErroDtw::PesoInvalido));
        let mista = ConfigDtw {
            banda: Some(2),
            subsequencia: true,
            ..simples()
        };
        assert_eq!(dtw(2, 2, f, &mista), Err(ErroDtw::BandaComSubsequencia));
        assert!(matches!(
            dtw(100_000, 100_000, f, &simples()),
            Err(ErroDtw::MuitoGrande { .. })
        ));
    }

    #[test]
    fn prefere_a_diagonal_quando_o_custo_empata() {
        // Custo 0 na diagonal e 0,5 fora dela: o caminho é a diagonal.
        let r = dtw(2, 2, |i, j| if i == j { 0.0 } else { 0.5 }, &simples()).expect("viável");
        assert_eq!(r.passos, vec![(0, 0), (1, 1)]);
    }
}
