//! Do caminho do DTW ao instante de cada nota: tempo pelo caminho, refinamento para o onset
//! mais próximo, confiança por nota e recorte do trecho tocado.
//!
//! 1. Notas com o mesmo início na partitura formam um **grupo** (acorde) e compartilham tempo
//!    e confiança.
//! 2. O tempo de um grupo pelo caminho é a primeira coluna (quadro do áudio) em que o caminho
//!    passa pela linha do início do grupo.
//! 3. Os onsets do áudio dentro de ±`janela_onset_s` do tempo do caminho são candidatos; uma
//!    atribuição monótona (programação dinâmica) liga cada grupo a no máximo um onset, sem que
//!    dois grupos dividam o mesmo onset. Quem fica sem onset mantém o tempo do caminho.
//! 4. Confiança (0–1) = qualidade do custo local do croma × presença do onset.

use crate::croma::Croma;
use crate::dtw::CaminhoDtw;
use crate::onsets::Onset;
use piano_tutor_core::comparador::EventoEsperado;

/// Uma nota da partitura com seu instante no áudio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NotaAlinhada {
    /// Posição na lista de esperados (ordem de `esperados_da_partitura`).
    pub indice: usize,
    /// Índice do compasso (ordem escrita, base 0) da nota.
    pub compasso: usize,
    pub midi: u8,
    pub t_partitura_s: f64,
    pub duracao_partitura_s: f64,
    /// Instante no áudio analisado (segundos desde o início dele).
    pub t_audio_s: f64,
    /// 0 a 1.
    pub confianca: f32,
    /// A nota foi refinada para um onset detectado.
    pub com_onset: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConfigMapeamento {
    /// Distância máxima entre o tempo do caminho e o onset adotado.
    pub janela_onset_s: f64,
    /// Só os primeiros segundos de cada nota entram no custo local (o resto é cauda).
    pub trecho_confianca_s: f64,
    /// Custo médio do croma a partir do qual a qualidade vira zero.
    pub custo_ruim: f32,
    /// Multiplicador da confiança quando não há onset.
    pub fator_sem_onset: f32,
}

impl Default for ConfigMapeamento {
    fn default() -> Self {
        Self {
            janela_onset_s: 0.08,
            trecho_confianca_s: 0.4,
            custo_ruim: 0.8,
            fator_sem_onset: 0.5,
        }
    }
}

struct Grupo {
    membros: Vec<usize>,
    t_partitura_s: f64,
    linha: usize,
    linha_seguinte: usize,
}

/// Mapeia as notas esperadas para instantes do áudio.
///
/// `croma_partitura` e o caminho usam a mesma taxa de quadros (`quadro_s`); `onsets` e os
/// instantes de saída estão na base de tempo do áudio analisado. `custo(i, j)` é o custo local
/// usado no DTW.
pub fn mapear_notas(
    esperados: &[EventoEsperado],
    croma_partitura: &Croma,
    caminho: &CaminhoDtw,
    onsets: &[Onset],
    custo: &dyn Fn(usize, usize) -> f32,
    cfg: &ConfigMapeamento,
) -> Vec<NotaAlinhada> {
    let quadro_s = croma_partitura.quadro_s;
    let n = croma_partitura.quadros.len();
    if esperados.is_empty() || n == 0 || caminho.passos.is_empty() {
        return Vec::new();
    }
    let primeira = caminho.primeira_coluna_por_linha(n);

    // 1. Grupos por início.
    let mut ordem: Vec<usize> = (0..esperados.len()).collect();
    ordem.sort_by(|a, b| esperados[*a].inicio_s.total_cmp(&esperados[*b].inicio_s));
    let mut grupos: Vec<Grupo> = Vec::new();
    for k in ordem {
        let t = esperados[k].inicio_s;
        match grupos.last_mut() {
            Some(g) if (t - g.t_partitura_s).abs() < 1e-6 => g.membros.push(k),
            _ => {
                let linha = (((t - croma_partitura.origem_s) / quadro_s).round().max(0.0) as usize)
                    .min(n - 1);
                grupos.push(Grupo {
                    membros: vec![k],
                    t_partitura_s: t,
                    linha,
                    linha_seguinte: n,
                });
            }
        }
    }
    for g in 1..grupos.len() {
        grupos[g - 1].linha_seguinte = grupos[g].linha.max(grupos[g - 1].linha + 1);
    }

    // 2. Tempo pelo caminho.
    let t_caminho: Vec<f64> = grupos
        .iter()
        .map(|g| primeira[g.linha] as f64 * quadro_s)
        .collect();

    // 3. Atribuição monótona de onsets.
    let atribuicao = atribuir_onsets(&t_caminho, onsets, cfg.janela_onset_s);

    // 4. Custo local por linha do caminho, e confiança.
    let mut soma = vec![0.0_f64; n];
    let mut conta = vec![0_u32; n];
    for &(i, j) in &caminho.passos {
        if i < n {
            soma[i] += f64::from(custo(i, j));
            conta[i] += 1;
        }
    }
    let quadros_trecho = ((cfg.trecho_confianca_s / quadro_s).ceil() as usize).max(1);

    let mut saida = vec![
        NotaAlinhada {
            indice: 0,
            compasso: 0,
            midi: 0,
            t_partitura_s: 0.0,
            duracao_partitura_s: 0.0,
            t_audio_s: 0.0,
            confianca: 0.0,
            com_onset: false,
        };
        esperados.len()
    ];
    let mut ultimo_t = 0.0_f64;
    for (gi, g) in grupos.iter().enumerate() {
        let fim = g.linha_seguinte.min(g.linha + quadros_trecho).min(n);
        let (mut s, mut c) = (0.0, 0_u32);
        for i in g.linha..fim {
            s += soma[i];
            c += conta[i];
        }
        let custo_local = if c > 0 {
            (s / f64::from(c)) as f32
        } else {
            1.0
        };
        let qualidade = (1.0 - custo_local / cfg.custo_ruim).clamp(0.0, 1.0);
        let (t, com_onset, fator) = match atribuicao[gi] {
            Some(k) => {
                let dt = (onsets[k].t_s - t_caminho[gi]).abs();
                let f = 1.0 - 0.3 * (dt / cfg.janela_onset_s).min(1.0) as f32;
                (onsets[k].t_s, true, f)
            }
            None => (t_caminho[gi], false, cfg.fator_sem_onset),
        };
        let t = t.max(ultimo_t);
        ultimo_t = t;
        for &k in &g.membros {
            saida[k] = NotaAlinhada {
                indice: k,
                compasso: esperados[k].compasso,
                midi: esperados[k].midi,
                t_partitura_s: esperados[k].inicio_s,
                duracao_partitura_s: esperados[k].duracao_s,
                t_audio_s: t,
                confianca: qualidade * fator,
                com_onset,
            };
        }
    }
    saida
}

/// Liga cada tempo de `alvos` (crescente) a no máximo um onset (crescente), dentro de
/// ±`janela`, minimizando a soma das distâncias mais `janela` por alvo sem onset.
/// Devolve, por alvo, o índice do onset em `onsets`.
fn atribuir_onsets(alvos: &[f64], onsets: &[Onset], janela: f64) -> Vec<Option<usize>> {
    let g = alvos.len();
    let mut resultado = vec![None; g];
    if g == 0 || onsets.is_empty() {
        return resultado;
    }
    let primeiro = alvos.iter().copied().fold(f64::INFINITY, f64::min) - janela;
    let ultimo = alvos.iter().copied().fold(f64::NEG_INFINITY, f64::max) + janela;
    let candidatos: Vec<usize> = (0..onsets.len())
        .filter(|k| onsets[*k].t_s >= primeiro && onsets[*k].t_s <= ultimo)
        .collect();
    let o = candidatos.len();
    if o == 0 {
        return resultado;
    }
    // f[a][k]: custo mínimo dos `a` primeiros alvos usando os `k` primeiros candidatos.
    let largura = o + 1;
    let mut f = vec![0.0_f64; (g + 1) * largura];
    let mut escolha = vec![0_u8; (g + 1) * largura]; // 0 pula onset, 1 alvo sem onset, 2 casa
    for a in 1..=g {
        f[a * largura] = a as f64 * janela;
        escolha[a * largura] = 1;
        for k in 1..=o {
            let dist = (onsets[candidatos[k - 1]].t_s - alvos[a - 1]).abs();
            let mut melhor = f[(a - 1) * largura + k] + janela;
            let mut e = 1;
            let pula = f[a * largura + k - 1];
            if pula < melhor {
                melhor = pula;
                e = 0;
            }
            if dist <= janela {
                let casa = f[(a - 1) * largura + k - 1] + dist;
                if casa < melhor {
                    melhor = casa;
                    e = 2;
                }
            }
            f[a * largura + k] = melhor;
            escolha[a * largura + k] = e;
        }
    }
    let (mut a, mut k) = (g, o);
    while a > 0 {
        match escolha[a * largura + k] {
            2 => {
                resultado[a - 1] = Some(candidatos[k - 1]);
                a -= 1;
                k -= 1;
            }
            0 => k -= 1,
            _ => a -= 1,
        }
    }
    resultado
}

/// Trecho tocado dentro do áudio analisado.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recorte {
    pub inicio_s: f64,
    pub fim_s: f64,
}

/// Recorte: primeira nota − margem até última nota + duração (na escala de andamento do
/// áudio) + margem, limitado a `[0, duracao_audio_s]`.
pub fn calcular_recorte(
    notas: &[NotaAlinhada],
    duracao_audio_s: f64,
    margem_s: f64,
) -> Option<Recorte> {
    let primeira = notas
        .iter()
        .min_by(|a, b| a.t_audio_s.total_cmp(&b.t_audio_s))?;
    let ultima = notas
        .iter()
        .max_by(|a, b| a.t_audio_s.total_cmp(&b.t_audio_s))?;
    let dp = ultima.t_partitura_s - primeira.t_partitura_s;
    let ratio = if dp > 1e-6 {
        ((ultima.t_audio_s - primeira.t_audio_s) / dp).clamp(0.5, 2.0)
    } else {
        1.0
    };
    let fim_nota = notas
        .iter()
        .map(|n| n.t_audio_s + n.duracao_partitura_s * ratio)
        .fold(f64::NEG_INFINITY, f64::max);
    Some(Recorte {
        inicio_s: (primeira.t_audio_s - margem_s).max(0.0),
        fim_s: (fim_nota + margem_s).min(duracao_audio_s.max(0.0)),
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    fn on(t: f64) -> Onset {
        Onset { t_s: t, forca: 1.0 }
    }

    #[test]
    fn atribuicao_casa_o_mais_proximo() {
        let r = atribuir_onsets(&[1.0, 2.0, 3.0], &[on(1.02), on(1.97), on(3.05)], 0.08);
        assert_eq!(r, vec![Some(0), Some(1), Some(2)]);
    }

    #[test]
    fn atribuicao_deixa_o_alvo_sem_onset_quando_ninguem_esta_perto() {
        let r = atribuir_onsets(&[1.0, 2.0, 3.0], &[on(1.0), on(3.0)], 0.08);
        assert_eq!(r, vec![Some(0), None, Some(1)]);
    }

    #[test]
    fn dois_alvos_nao_dividem_o_mesmo_onset() {
        // O alvo 1 (1.95) está a 0,05 do onset 2.0; o alvo 2 (2.01) está a 0,01: só o segundo leva.
        let r = atribuir_onsets(&[1.95, 2.01], &[on(2.0)], 0.08);
        assert_eq!(r, vec![None, Some(0)]);
    }

    #[test]
    fn atribuicao_vazia() {
        assert_eq!(atribuir_onsets(&[1.0], &[], 0.08), vec![None]);
        assert!(atribuir_onsets(&[], &[on(1.0)], 0.08).is_empty());
    }

    fn nota(t_part: f64, t_aud: f64, dur: f64) -> NotaAlinhada {
        NotaAlinhada {
            indice: 0,
            compasso: 0,
            midi: 60,
            t_partitura_s: t_part,
            duracao_partitura_s: dur,
            t_audio_s: t_aud,
            confianca: 1.0,
            com_onset: true,
        }
    }

    #[test]
    fn recorte_usa_margem_e_escala_de_andamento() {
        // Áudio 20% mais lento: a última nota (0,5 s na partitura) dura 0,6 s.
        let notas = [nota(0.0, 10.0, 0.5), nota(1.0, 11.2, 0.5)];
        let r = calcular_recorte(&notas, 60.0, 0.5).expect("há notas");
        assert!((r.inicio_s - 9.5).abs() < 1e-9);
        assert!((r.fim_s - (11.2 + 0.6 + 0.5)).abs() < 1e-9);
    }

    #[test]
    fn recorte_e_limitado_ao_audio() {
        let notas = [nota(0.0, 0.2, 2.0)];
        let r = calcular_recorte(&notas, 1.0, 0.5).expect("há notas");
        assert_eq!(r.inicio_s, 0.0);
        assert_eq!(r.fim_s, 1.0);
        assert!(calcular_recorte(&[], 1.0, 0.5).is_none());
    }
}
