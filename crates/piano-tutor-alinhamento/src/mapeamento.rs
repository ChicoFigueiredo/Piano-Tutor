//! Do caminho do DTW ao instante de cada nota: tempo pelo caminho, refinamento para o onset
//! mais próximo, confiança por nota e recorte do trecho tocado.
//!
//! 1. Notas com o mesmo início na partitura formam um **grupo** (acorde) e compartilham tempo
//!    e confiança.
//! 2. O tempo de um grupo pelo caminho é a primeira coluna (quadro do áudio) em que o caminho
//!    passa pela linha do início do grupo.
//!    Notas repetidas (mesma altura em sequência) têm croma idêntico e o caminho não as separa:
//!    o tempo de cada uma é interpolado entre o início da sequência e o do grupo seguinte
//!    (`regularizar_repetidas`). O caminho também costuma chegar ~75 ms antes do ataque com
//!    piano real (`vies_caminho_s`, medido nas Aulas do piloto M4; sem viés nas fixtures
//!    sintéticas, por isso só alarga a janela do lado tardio).
//! 3. Os onsets do áudio perto do tempo do caminho (±`janela_onset_s`, mais o viés do lado tardio) são candidatos; uma
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
    /// Idem para a nota que vem depois de silêncio (ou a primeira): o caminho anda num platô
    /// de custo baixo e localiza pior o início dela, então a janela é maior.
    pub janela_onset_apos_silencio_s: f64,
    /// Só os primeiros segundos de cada nota entram no custo local (o resto é cauda).
    pub trecho_confianca_s: f64,
    /// Custo médio do croma a partir do qual a qualidade vira zero.
    pub custo_ruim: f32,
    /// Multiplicador da confiança quando não há onset.
    pub fator_sem_onset: f32,
    /// Custo do croma abaixo do qual o áudio "já é" a nota (usado para achar o início do som).
    pub custo_som: f32,
    /// Quanto o início de uma nota após silêncio pode recuar (s) até o começo do som.
    pub recuo_max_s: f64,
    /// Quanto pode avançar (s) quando o caminho começa antes de a nota soar.
    pub avanco_max_s: f64,
    /// Correção sistemática (s) somada ao tempo pelo caminho de uma nota que não vem depois de
    /// silêncio: a janela longa do croma faz o caminho trocar de nota um pouco **antes** do
    /// ataque (com piano real, ~75 ms).
    pub vies_caminho_s: f64,
    /// Janela (s) de uma nota repetida cujo tempo foi interpolado entre as vizinhas.
    pub janela_onset_repetida_s: f64,
}

impl Default for ConfigMapeamento {
    fn default() -> Self {
        Self {
            janela_onset_s: 0.08,
            janela_onset_apos_silencio_s: 0.2,
            trecho_confianca_s: 0.4,
            custo_ruim: 0.8,
            fator_sem_onset: 0.5,
            custo_som: 0.5,
            recuo_max_s: 0.25,
            avanco_max_s: 0.5,
            vies_caminho_s: 0.075,
            janela_onset_repetida_s: 0.15,
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
/// `croma_partitura` e o caminho usam a mesma taxa de quadros (`quadro_s`); `colunas_audio` é o
/// número de quadros do áudio analisado; `onsets` e os
/// instantes de saída estão na base de tempo do áudio analisado. `custo(i, j)` é o custo local
/// usado no DTW.
pub fn mapear_notas(
    esperados: &[EventoEsperado],
    croma_partitura: &Croma,
    caminho: &CaminhoDtw,
    colunas_audio: usize,
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

    // 2. Tempo pelo caminho. Numa nota que vem depois de silêncio (ou a primeira), o caminho
    // anda num platô de custo quase zero e o instante exato em que ela entra é arbitrário;
    // então o tempo vai para o começo do som que casa com a nota.
    let mut t_caminho: Vec<f64> = Vec::with_capacity(grupos.len());
    let mut janelas: Vec<f64> = Vec::with_capacity(grupos.len());
    for g in &grupos {
        let j = primeira[g.linha];
        let depois_de_silencio = g.linha == 0
            || croma_partitura.quadros[g.linha - 1]
                .iter()
                .all(|v| *v == 0.0);
        if depois_de_silencio {
            let j = inicio_do_som(g.linha, j, colunas_audio, quadro_s, custo, cfg);
            t_caminho.push(j as f64 * quadro_s);
            janelas.push(cfg.janela_onset_apos_silencio_s.max(cfg.janela_onset_s));
        } else {
            t_caminho.push(j as f64 * quadro_s);
            janelas.push(cfg.janela_onset_s);
        }
    }
    let mut usa_vies: Vec<bool> = janelas.iter().map(|w| *w <= cfg.janela_onset_s).collect();
    regularizar_repetidas(
        esperados,
        &grupos,
        &mut t_caminho,
        &mut janelas,
        &mut usa_vies,
        cfg,
    );

    // 3. Atribuição monótona de onsets. Como o caminho costuma chegar **antes** do ataque, a
    // janela de quem usa o viés vai de `t - janela` a `t + janela + viés` (alvo no meio dela);
    // o tempo de quem fica sem onset continua sendo o do caminho.
    let alvos: Vec<f64> = (0..grupos.len())
        .map(|g| {
            t_caminho[g]
                + if usa_vies[g] {
                    cfg.vies_caminho_s / 2.0
                } else {
                    0.0
                }
        })
        .collect();
    let janelas_busca: Vec<f64> = (0..grupos.len())
        .map(|g| {
            janelas[g]
                + if usa_vies[g] {
                    cfg.vies_caminho_s / 2.0
                } else {
                    0.0
                }
        })
        .collect();
    let atribuicao = atribuir_onsets(&alvos, onsets, &janelas_busca);

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
                let dt = (onsets[k].t_s - alvos[gi]).abs();
                let f = 1.0 - 0.3 * (dt / janelas_busca[gi]).min(1.0) as f32;
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

/// Quantas sequências à frente entram na medida do andamento usada para extrapolar.
const SEQUENCIAS_PARA_ANDAMENTO: usize = 8;

/// Notas repetidas (mesma altura, uma após a outra): o croma é idêntico, então o caminho do DTW
/// não distingue uma da outra (varia à toa dentro do platô e, no começo da peça, pode até
/// começar tarde). Só a **troca** de altura é confiável. Aqui o tempo de cada nota de uma
/// sequência repetida é interpolado, proporcionalmente à partitura, entre o início da sequência
/// e o início do grupo seguinte; os onsets fazem o resto. Se o início da sequência não é
/// confiável (primeira da peça ou depois de silêncio), ele é extrapolado com o andamento das
/// sequências seguintes.
fn regularizar_repetidas(
    esperados: &[EventoEsperado],
    grupos: &[Grupo],
    t_caminho: &mut [f64],
    janelas: &mut [f64],
    usa_vies: &mut [bool],
    cfg: &ConfigMapeamento,
) {
    let alturas = |g: &Grupo| {
        let mut m: Vec<u8> = g.membros.iter().map(|k| esperados[*k].midi).collect();
        m.sort_unstable();
        m
    };
    let chaves: Vec<Vec<u8>> = grupos.iter().map(alturas).collect();
    // Sequências: (primeiro grupo, número de grupos).
    let mut sequencias: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < grupos.len() {
        let mut j = i + 1;
        while j < grupos.len() && chaves[j] == chaves[i] {
            j += 1;
        }
        sequencias.push((i, j - i));
        i = j;
    }
    if !sequencias.iter().any(|&(_, r)| r >= 2) {
        return;
    }
    // Início confiável de cada sequência: ela não é a primeira nem vem logo após silêncio.
    let confiavel =
        |s: usize, indice_seq: usize| indice_seq > 0 && janelas[s] <= cfg.janela_onset_s;
    let mut inicios: Vec<f64> = sequencias.iter().map(|&(s, _)| t_caminho[s]).collect();
    let tp: Vec<f64> = sequencias
        .iter()
        .map(|&(s, _)| grupos[s].t_partitura_s)
        .collect();
    // Andamento local (s de áudio por s de partitura) entre inícios confiáveis vizinhos.
    for q in 0..sequencias.len() {
        let (s, _) = sequencias[q];
        if confiavel(s, q) {
            continue;
        }
        // Extrapola com o andamento medido entre a seguinte e a mais distante das próximas
        // sequências confiáveis (até `SEQUENCIAS_PARA_ANDAMENTO`): um intervalo só é ruidoso.
        let seguintes: Vec<usize> = ((q + 1)..sequencias.len())
            .filter(|&k| confiavel(sequencias[k].0, k))
            .take(SEQUENCIAS_PARA_ANDAMENTO)
            .collect();
        if let (Some(&k1), Some(&kn)) = (seguintes.first(), seguintes.last())
            && k1 == q + 1
            && tp[kn] > tp[k1]
            && inicios[kn] > inicios[k1]
        {
            let ritmo = (inicios[kn] - inicios[k1]) / (tp[kn] - tp[k1]);
            inicios[q] = inicios[k1] - (tp[k1] - tp[q]) * ritmo;
        }
    }
    for (q, &(s, r)) in sequencias.iter().enumerate() {
        if r < 2 || q + 1 >= sequencias.len() {
            continue;
        }
        let (a, b) = (inicios[q], t_caminho[sequencias[q + 1].0]);
        let (pa, pb) = (tp[q], tp[q + 1]);
        if b <= a || pb <= pa {
            continue;
        }
        for g in s..s + r {
            t_caminho[g] = a + (b - a) * (grupos[g].t_partitura_s - pa) / (pb - pa);
            janelas[g] = janelas[g].max(cfg.janela_onset_repetida_s);
            usa_vies[g] = true;
        }
    }
}

/// Procura o começo do som da nota da linha `i` perto da coluna `j` do caminho: volta enquanto
/// o croma do áudio ainda casa com a nota; se em `j` ainda não casa, avança até casar.
fn inicio_do_som(
    i: usize,
    j: usize,
    colunas: usize,
    quadro_s: f64,
    custo: &dyn Fn(usize, usize) -> f32,
    cfg: &ConfigMapeamento,
) -> usize {
    let recuo = (cfg.recuo_max_s / quadro_s).round() as usize;
    let avanco = (cfg.avanco_max_s / quadro_s).round() as usize;
    let casa = |c: usize| c < colunas && custo(i, c) < cfg.custo_som;
    let mut j = j;
    if !casa(j) {
        match (j + 1..=j + avanco).find(|c| casa(*c)) {
            Some(c) => j = c,
            None => return j,
        }
    }
    let mut voltou = 0;
    while j > 0 && voltou < recuo && casa(j - 1) {
        j -= 1;
        voltou += 1;
    }
    j
}

/// Liga cada tempo de `alvos` (crescente) a no máximo um onset (crescente), dentro de
/// ±`janelas[a]`, minimizando a soma das distâncias mais `janelas[a]` por alvo sem onset.
/// Devolve, por alvo, o índice do onset em `onsets`.
fn atribuir_onsets(alvos: &[f64], onsets: &[Onset], janelas: &[f64]) -> Vec<Option<usize>> {
    let g = alvos.len();
    let mut resultado = vec![None; g];
    if g == 0 || onsets.is_empty() {
        return resultado;
    }
    let folga = janelas.iter().copied().fold(0.0, f64::max);
    let primeiro = alvos.iter().copied().fold(f64::INFINITY, f64::min) - folga;
    let ultimo = alvos.iter().copied().fold(f64::NEG_INFINITY, f64::max) + folga;
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
        f[a * largura] = f[(a - 1) * largura] + janelas[a - 1];
        escolha[a * largura] = 1;
        for k in 1..=o {
            let dist = (onsets[candidatos[k - 1]].t_s - alvos[a - 1]).abs();
            let mut melhor = f[(a - 1) * largura + k] + janelas[a - 1];
            let mut e = 1;
            let pula = f[a * largura + k - 1];
            if pula < melhor {
                melhor = pula;
                e = 0;
            }
            if dist <= janelas[a - 1] {
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
        let r = atribuir_onsets(
            &[1.0, 2.0, 3.0],
            &[on(1.02), on(1.97), on(3.05)],
            &[0.08; 3],
        );
        assert_eq!(r, vec![Some(0), Some(1), Some(2)]);
    }

    #[test]
    fn atribuicao_deixa_o_alvo_sem_onset_quando_ninguem_esta_perto() {
        let r = atribuir_onsets(&[1.0, 2.0, 3.0], &[on(1.0), on(3.0)], &[0.08; 3]);
        assert_eq!(r, vec![Some(0), None, Some(1)]);
    }

    #[test]
    fn dois_alvos_nao_dividem_o_mesmo_onset() {
        // O alvo 1 (1.95) está a 0,05 do onset 2.0; o alvo 2 (2.01) está a 0,01: só o segundo leva.
        let r = atribuir_onsets(&[1.95, 2.01], &[on(2.0)], &[0.08; 2]);
        assert_eq!(r, vec![None, Some(0)]);
    }

    #[test]
    fn janela_maior_so_para_quem_tem_janela_maior() {
        // O onset está a 0,15 s de cada alvo; só o alvo com janela de 0,2 s o aceita.
        let r = atribuir_onsets(&[1.0], &[on(1.15)], &[0.08]);
        assert_eq!(r, vec![None]);
        let r = atribuir_onsets(&[1.0], &[on(1.15)], &[0.2]);
        assert_eq!(r, vec![Some(0)]);
    }

    #[test]
    fn atribuicao_vazia() {
        assert_eq!(atribuir_onsets(&[1.0], &[], &[0.08]), vec![None]);
        assert!(atribuir_onsets(&[], &[on(1.0)], &[]).is_empty());
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
