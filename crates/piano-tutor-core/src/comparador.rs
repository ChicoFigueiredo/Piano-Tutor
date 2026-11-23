//! Comparador entre notas tocadas (MIDI) e eventos esperados da partitura (F07-4).
//!
//! Função pura, sem relógio, UI nem hardware: recebe listas de `(altura MIDI, início em
//! segundos)` e uma tolerância em milissegundos e devolve acertos, faltas e extras. A noção de
//! **evento esperado** (o que deve soar, com ligaduras fundidas e repetições expandidas, ver
//! [`Partitura::eventos_de_reproducao`]) é separada da representação visual (geometria da
//! pauta): este módulo nunca olha para o desenho.
//!
//! Pareamento: para cada altura MIDI, as duas listas são ordenadas pelo tempo e percorridas
//! em paralelo; uma nota tocada acerta um esperado se a diferença for no máximo a tolerância.
//! Esse varrimento guloso é ótimo (máximo de acertos) porque a janela tem a mesma largura para
//! todos os esperados. Cada nota esperada e cada nota tocada participa de no máximo um acerto.
//!
//! Não mede latência de MIDI/áudio nem verifica velocity, duração ou pedal: **NÃO VERIFICADO**
//! com teclado real; só o cálculo é testado.

use crate::partitura::Partitura;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

/// Tolerância máxima aceita (uma semínima a 30 bpm seria 2 s; acima disso a comparação perde o sentido).
pub const TOLERANCIA_MAXIMA_MS: f64 = 2_000.0;

/// Nota esperada, em segundos desde o início da execução.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EventoEsperado {
    pub midi: u8,
    pub inicio_s: f64,
    pub duracao_s: f64,
    /// Parte (instrumento) de origem na partitura.
    pub parte: usize,
    /// Pauta (1 = de cima) dentro da parte.
    pub pauta: u8,
    /// Compasso (ordem escrita) onde o evento começa.
    pub compasso: usize,
}

/// Nota tocada: altura MIDI e instante do note-on, em segundos na mesma base de tempo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NotaTocada {
    pub midi: u8,
    pub inicio_s: f64,
}

/// Configuração da comparação.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConfigComparacao {
    /// Diferença máxima de tempo para contar como acerto, em ms (≥ 0).
    pub tolerancia_ms: f64,
    /// Atraso sistemático somado ao instante tocado antes de comparar (calibração), em ms.
    /// Positivo = o teclado chega atrasado, então o instante tocado é adiantado.
    pub compensacao_ms: f64,
}

impl Default for ConfigComparacao {
    fn default() -> Self {
        Self {
            tolerancia_ms: 150.0,
            compensacao_ms: 0.0,
        }
    }
}

/// Erro de entrada inválida (nunca de pânico).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErroComparacao {
    /// A tolerância é negativa, não finita ou acima de [`TOLERANCIA_MAXIMA_MS`].
    ToleranciaInvalida,
    /// A compensação não é finita.
    CompensacaoInvalida,
    /// Algum instante é NaN ou infinito (qual lista).
    TempoInvalido(&'static str),
}

impl fmt::Display for ErroComparacao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ToleranciaInvalida => write!(
                f,
                "tolerância inválida (use de 0 a {TOLERANCIA_MAXIMA_MS} ms)"
            ),
            Self::CompensacaoInvalida => write!(f, "compensação de tempo inválida"),
            Self::TempoInvalido(q) => write!(f, "instante não finito em {q}"),
        }
    }
}

impl std::error::Error for ErroComparacao {}

/// Um esperado casado com uma nota tocada.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Acerto {
    /// Índice na lista de esperados recebida.
    pub esperado: usize,
    /// Índice na lista de tocadas recebida.
    pub tocada: usize,
    /// Tocada menos esperada, em ms (negativo = adiantada), já com a compensação.
    pub desvio_ms: f64,
}

/// Resultado da comparação. Os índices apontam para as listas recebidas.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Comparacao {
    pub acertos: Vec<Acerto>,
    /// Esperados que ninguém tocou na janela.
    pub faltas: Vec<usize>,
    /// Tocadas que não correspondem a nenhum esperado (altura errada ou fora da janela).
    pub extras: Vec<usize>,
}

impl Comparacao {
    /// Fração de esperados acertados (1,0 se não havia nada esperado).
    pub fn taxa_de_acerto(&self) -> f64 {
        let total = self.acertos.len() + self.faltas.len();
        if total == 0 {
            1.0
        } else {
            self.acertos.len() as f64 / total as f64
        }
    }

    /// Desvio médio (com sinal) dos acertos, em ms; `None` sem acertos.
    pub fn desvio_medio_ms(&self) -> Option<f64> {
        if self.acertos.is_empty() {
            None
        } else {
            Some(self.acertos.iter().map(|a| a.desvio_ms).sum::<f64>() / self.acertos.len() as f64)
        }
    }

    /// Não houve falta nem extra.
    pub fn perfeita(&self) -> bool {
        self.faltas.is_empty() && self.extras.is_empty()
    }
}

/// Converte os eventos de reprodução da partitura em esperados em segundos, com o mapa de
/// andamento do arquivo. `bpm_padrao` vale onde o arquivo não declara andamento; `escala`
/// multiplica o andamento (1,0 = como escrito).
pub fn esperados_da_partitura(p: &Partitura, bpm_padrao: f64, escala: f64) -> Vec<EventoEsperado> {
    let mapa = p.mapa_de_andamento();
    p.eventos_de_reproducao()
        .iter()
        .map(|e| {
            let q0 = e.inicio.como_f64();
            let q1 = (e.inicio + e.duracao).como_f64();
            let t0 = mapa.segundos(q0, bpm_padrao, escala);
            EventoEsperado {
                midi: e.midi,
                inicio_s: t0,
                duracao_s: mapa.segundos(q1, bpm_padrao, escala) - t0,
                parte: e.parte,
                pauta: e.pauta,
                compasso: e.compasso,
            }
        })
        .collect()
}

/// Compara as notas tocadas com as esperadas. Entradas em qualquer ordem.
pub fn comparar(
    esperados: &[EventoEsperado],
    tocadas: &[NotaTocada],
    config: ConfigComparacao,
) -> Result<Comparacao, ErroComparacao> {
    if !(config.tolerancia_ms.is_finite()
        && (0.0..=TOLERANCIA_MAXIMA_MS).contains(&config.tolerancia_ms))
    {
        return Err(ErroComparacao::ToleranciaInvalida);
    }
    if !config.compensacao_ms.is_finite() {
        return Err(ErroComparacao::CompensacaoInvalida);
    }
    if esperados.iter().any(|e| !e.inicio_s.is_finite()) {
        return Err(ErroComparacao::TempoInvalido("esperados"));
    }
    if tocadas.iter().any(|t| !t.inicio_s.is_finite()) {
        return Err(ErroComparacao::TempoInvalido("tocadas"));
    }
    let tol = config.tolerancia_ms / 1000.0;
    let comp = config.compensacao_ms / 1000.0;
    // Agrupa por altura: (tempo, índice original), ordenado pelo tempo.
    let mut esp: BTreeMap<u8, Vec<(f64, usize)>> = BTreeMap::new();
    for (i, e) in esperados.iter().enumerate() {
        esp.entry(e.midi).or_default().push((e.inicio_s, i));
    }
    let mut toc: BTreeMap<u8, Vec<(f64, usize)>> = BTreeMap::new();
    for (i, t) in tocadas.iter().enumerate() {
        toc.entry(t.midi).or_default().push((t.inicio_s + comp, i));
    }
    let ordena = |v: &mut Vec<(f64, usize)>| {
        v.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(Ordering::Equal)
                .then(a.1.cmp(&b.1))
        });
    };
    esp.values_mut().for_each(ordena);
    toc.values_mut().for_each(ordena);

    let mut r = Comparacao::default();
    let vazio: Vec<(f64, usize)> = Vec::new();
    let alturas: std::collections::BTreeSet<u8> = esp.keys().chain(toc.keys()).copied().collect();
    for altura in alturas {
        let e = esp.get(&altura).unwrap_or(&vazio);
        let t = toc.get(&altura).unwrap_or(&vazio);
        let (mut i, mut j) = (0, 0);
        while i < e.len() && j < t.len() {
            let dif = t[j].0 - e[i].0;
            if dif.abs() <= tol + 1e-9 {
                r.acertos.push(Acerto {
                    esperado: e[i].1,
                    tocada: t[j].1,
                    desvio_ms: dif * 1000.0,
                });
                i += 1;
                j += 1;
            } else if dif < 0.0 {
                // Tocada cedo demais para este esperado (e para os seguintes): sobra.
                r.extras.push(t[j].1);
                j += 1;
            } else {
                // Esperado ficou para trás da janela: ninguém tocou.
                r.faltas.push(e[i].1);
                i += 1;
            }
        }
        r.faltas.extend(e[i..].iter().map(|x| x.1));
        r.extras.extend(t[j..].iter().map(|x| x.1));
    }
    r.acertos.sort_by_key(|a| a.esperado);
    r.faltas.sort_unstable();
    r.extras.sort_unstable();
    Ok(r)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn esp(midi: u8, t: f64) -> EventoEsperado {
        EventoEsperado {
            midi,
            inicio_s: t,
            duracao_s: 0.5,
            parte: 0,
            pauta: 1,
            compasso: 0,
        }
    }

    fn toc(midi: u8, t: f64) -> NotaTocada {
        NotaTocada { midi, inicio_s: t }
    }

    fn cfg(ms: f64) -> ConfigComparacao {
        ConfigComparacao {
            tolerancia_ms: ms,
            compensacao_ms: 0.0,
        }
    }

    #[test]
    fn execucao_exata_acerta_tudo() {
        let e = [esp(60, 0.0), esp(64, 0.5), esp(67, 1.0)];
        let t = [toc(60, 0.0), toc(64, 0.5), toc(67, 1.0)];
        let r = comparar(&e, &t, cfg(50.0)).expect("válida");
        assert_eq!(r.acertos.len(), 3);
        assert!(r.perfeita());
        assert_eq!(r.taxa_de_acerto(), 1.0);
        assert_eq!(r.desvio_medio_ms(), Some(0.0));
    }

    #[test]
    fn tolerancia_decide_entre_acerto_e_falta_mais_extra() {
        let e = [esp(60, 1.0)];
        let t = [toc(60, 1.080)];
        let r = comparar(&e, &t, cfg(100.0)).expect("válida");
        assert_eq!(r.acertos.len(), 1);
        assert!((r.acertos[0].desvio_ms - 80.0).abs() < 1e-6);
        let r = comparar(&e, &t, cfg(50.0)).expect("válida");
        assert!(r.acertos.is_empty());
        assert_eq!(r.faltas, [0]);
        assert_eq!(r.extras, [0]);
        assert_eq!(r.taxa_de_acerto(), 0.0);
        // Borda: exatamente na tolerância ainda acerta, e adiantado dá desvio negativo.
        let r = comparar(&e, &[toc(60, 0.9)], cfg(100.0)).expect("válida");
        assert_eq!(r.acertos.len(), 1);
        assert!(r.acertos[0].desvio_ms < 0.0);
    }

    #[test]
    fn altura_errada_vira_falta_e_extra() {
        let r = comparar(&[esp(60, 0.0)], &[toc(61, 0.0)], cfg(100.0)).expect("válida");
        assert_eq!(r.faltas, [0]);
        assert_eq!(r.extras, [0]);
        assert!(r.acertos.is_empty());
    }

    #[test]
    fn nota_a_menos_e_nota_a_mais() {
        let e = [esp(60, 0.0), esp(62, 0.5), esp(64, 1.0)];
        let t = [toc(60, 0.0), toc(64, 1.0), toc(72, 1.2)];
        let r = comparar(&e, &t, cfg(100.0)).expect("válida");
        assert_eq!(r.acertos.len(), 2);
        assert_eq!(r.faltas, [1]);
        assert_eq!(r.extras, [2]);
        assert!((r.taxa_de_acerto() - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn repeticao_da_mesma_altura_casa_uma_a_uma() {
        // Duas notas esperadas iguais; só uma tocada perto da primeira: a outra é falta.
        let e = [esp(60, 0.0), esp(60, 0.4)];
        let r = comparar(&e, &[toc(60, 0.05)], cfg(100.0)).expect("válida");
        assert_eq!(r.acertos.len(), 1);
        assert_eq!(r.acertos[0].esperado, 0);
        assert_eq!(r.faltas, [1]);
        // Duas tocadas para um só esperado: a segunda é extra.
        let r =
            comparar(&[esp(60, 0.0)], &[toc(60, 0.0), toc(60, 0.05)], cfg(100.0)).expect("válida");
        assert_eq!(r.acertos.len(), 1);
        assert_eq!(r.extras, [1]);
    }

    #[test]
    fn varrimento_e_otimo_com_janelas_sobrepostas() {
        // Tolerância 150 ms; esperados a 0,0 e 0,2; tocadas a 0,15 e 0,3. O casamento ótimo
        // é 0,0 com 0,15 e 0,2 com 0,3 (dois acertos).
        let e = [esp(60, 0.0), esp(60, 0.2)];
        let t = [toc(60, 0.15), toc(60, 0.3)];
        let r = comparar(&e, &t, cfg(150.0)).expect("válida");
        assert_eq!(r.acertos.len(), 2);
        assert!(r.perfeita());
    }

    #[test]
    fn aceita_entradas_fora_de_ordem() {
        let e = [esp(67, 1.0), esp(60, 0.0), esp(64, 0.5)];
        let t = [toc(64, 0.52), toc(67, 0.98), toc(60, 0.01)];
        let r = comparar(&e, &t, cfg(50.0)).expect("válida");
        assert_eq!(r.acertos.len(), 3);
        assert_eq!(r.acertos[0].esperado, 0);
        assert_eq!(r.acertos[0].tocada, 1);
    }

    #[test]
    fn compensacao_corrige_atraso_sistematico() {
        let e = [esp(60, 0.0), esp(62, 0.5)];
        // O teclado chega 120 ms atrasado.
        let t = [toc(60, 0.12), toc(62, 0.62)];
        let sem = comparar(&e, &t, cfg(50.0)).expect("válida");
        assert_eq!(sem.acertos.len(), 0);
        let com = comparar(
            &e,
            &t,
            ConfigComparacao {
                tolerancia_ms: 50.0,
                compensacao_ms: -120.0,
            },
        )
        .expect("válida");
        assert_eq!(com.acertos.len(), 2);
        assert!(com.desvio_medio_ms().is_some_and(|d| d.abs() < 1e-6));
    }

    #[test]
    fn listas_vazias_e_entradas_invalidas() {
        let r = comparar(&[], &[], cfg(50.0)).expect("válida");
        assert!(r.perfeita());
        assert_eq!(r.taxa_de_acerto(), 1.0);
        assert_eq!(r.desvio_medio_ms(), None);
        let r = comparar(&[esp(60, 0.0)], &[], cfg(50.0)).expect("válida");
        assert_eq!(r.faltas, [0]);
        let r = comparar(&[], &[toc(60, 0.0)], cfg(50.0)).expect("válida");
        assert_eq!(r.extras, [0]);
        for ruim in [-1.0, f64::NAN, f64::INFINITY, TOLERANCIA_MAXIMA_MS + 1.0] {
            assert_eq!(
                comparar(&[], &[], cfg(ruim)),
                Err(ErroComparacao::ToleranciaInvalida)
            );
        }
        assert_eq!(
            comparar(
                &[],
                &[],
                ConfigComparacao {
                    tolerancia_ms: 10.0,
                    compensacao_ms: f64::NAN
                }
            ),
            Err(ErroComparacao::CompensacaoInvalida)
        );
        assert_eq!(
            comparar(&[esp(60, f64::NAN)], &[], cfg(10.0)),
            Err(ErroComparacao::TempoInvalido("esperados"))
        );
        assert_eq!(
            comparar(&[], &[toc(60, f64::INFINITY)], cfg(10.0)),
            Err(ErroComparacao::TempoInvalido("tocadas"))
        );
        // Tolerância zero exige instante idêntico.
        let r = comparar(&[esp(60, 1.0)], &[toc(60, 1.0)], cfg(0.0)).expect("válida");
        assert_eq!(r.acertos.len(), 1);
    }

    #[test]
    fn esperados_da_partitura_usam_o_andamento_do_arquivo() {
        let ode = include_str!("../../../assets/partituras/beethoven-ode-a-alegria-tema.musicxml");
        let p = Partitura::de_musicxml(ode).expect("válida");
        let e = esperados_da_partitura(&p, 80.0, 1.0);
        assert_eq!(e.len(), 94);
        // 100 bpm: a semínima dura 0,6 s; o segundo compasso começa em 2,4 s.
        let primeiro_do_c2 = e
            .iter()
            .find(|x| x.compasso == 1 && x.pauta == 1)
            .expect("nota");
        assert!((primeiro_do_c2.inicio_s - 2.4).abs() < 1e-9);
        assert!((primeiro_do_c2.duracao_s - 0.6).abs() < 1e-9);
        // Dobrar o andamento (escala 2) corta os tempos pela metade.
        let rapido = esperados_da_partitura(&p, 80.0, 2.0);
        assert!((rapido[0].duracao_s * 2.0 - e[0].duracao_s).abs() < 1e-9);
    }

    #[test]
    fn execucao_perfeita_da_partitura_acerta_tudo_e_a_lenta_depende_da_tolerancia() {
        let ode = include_str!("../../../assets/partituras/beethoven-ode-a-alegria-tema.musicxml");
        let p = Partitura::de_musicxml(ode).expect("válida");
        let e = esperados_da_partitura(&p, 80.0, 1.0);
        let tocadas: Vec<NotaTocada> = e
            .iter()
            .map(|x| NotaTocada {
                midi: x.midi,
                inicio_s: x.inicio_s,
            })
            .collect();
        let r = comparar(&e, &tocadas, cfg(30.0)).expect("válida");
        assert_eq!(r.acertos.len(), 94);
        assert!(r.perfeita());
        // Aluno 10% mais lento: a tolerância de 50 ms perde as últimas, a de 5 s é recusada.
        let lentas: Vec<NotaTocada> = e
            .iter()
            .map(|x| NotaTocada {
                midi: x.midi,
                inicio_s: x.inicio_s * 1.10,
            })
            .collect();
        let r = comparar(&e, &lentas, cfg(50.0)).expect("válida");
        assert!(r.acertos.len() < 94 && !r.faltas.is_empty());
        assert_eq!(r.faltas.len(), r.extras.len());
        assert!(comparar(&e, &lentas, cfg(5_000.0)).is_err()); // acima do teto
        // Atraso constante de 120 ms: acerta tudo com 150 ms e nada com 100 ms.
        let atrasadas: Vec<NotaTocada> = e
            .iter()
            .map(|x| NotaTocada {
                midi: x.midi,
                inicio_s: x.inicio_s + 0.12,
            })
            .collect();
        let r = comparar(&e, &atrasadas, cfg(150.0)).expect("válida");
        assert_eq!(r.acertos.len(), 94);
        assert!((r.desvio_medio_ms().unwrap_or(0.0) - 120.0).abs() < 1e-6);
        let r = comparar(&e, &atrasadas, cfg(100.0)).expect("válida");
        assert!(r.acertos.is_empty());
    }
}
