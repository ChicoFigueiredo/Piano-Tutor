//! Medição de jitter **offline** do relógio mestre e do agendador.
//!
//! Metodologia (render offline, sem dispositivo):
//!
//! 1. Um dispositivo simulado consome frames a `taxa × (1 + deriva)`. O
//!    callback `k` deveria acordar em `t0 + k·bloco/taxa_real`, mas acorda
//!    atrasado por um valor pseudoaleatório (uniforme em `[0, J]`, com picos
//!    ocasionais), modelando o escalonador do SO. **Esses atrasos são um
//!    modelo, não uma medição do PC de referência.**
//! 2. Eventos MIDI com timestamp monotônico conhecido `e_i` são entregues à
//!    fila com atraso de entrega pseudoaleatório e passam pelo caminho real:
//!    [`ControleAudio`] → fila `rtrb` → [`MotorAudio`] → sintetizador.
//! 3. O [`SintetizadorFalso`] emite um impulso no frame em que recebeu a nota;
//!    a saída renderizada é varrida e o frame de cada impulso é comparado com
//!    o frame ideal `round((e_i − t0)·taxa_real) + latência_alvo`.
//!
//! O erro em frames mede só o posicionamento pelo relógio mestre. Não inclui
//! a latência do driver/conversor nem o caminho USB do teclado: a medição
//! MIDI→áudio real (p95 ≤ 50 ms) é feita por captura no PC de referência.

use std::fmt::Write as _;

use piano_tutor_core::NotaMidi;
use piano_tutor_core::midi::{CanalMidi, EventoMidi};

use crate::comando::{Acao, ComandoAudio, Quando};
use crate::motor::{ConfigMotor, MotorAudio};
use crate::relogio::{
    ConfigAncora, NS_POR_SEGUNDO, TaxaAmostragem, frames_para_ms, ms_para_frames,
};
use crate::rusty::{ConfigRusty, SintetizadorRusty, ler_soundfont};
use crate::sf2_sintetico;
use crate::sintetizador::{Sintetizador, SintetizadorFalso};

/// Como o evento é posicionado.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModoAgendamento {
    /// Pelo timestamp + latência alvo ([`Quando::Instante`]).
    Agendado,
    /// No início do próximo bloco ([`Quando::Imediato`]), para comparação.
    Imediato,
}

/// Um cenário de simulação.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CenarioJitter {
    pub nome: &'static str,
    pub taxa_hz: u32,
    pub bloco: usize,
    pub latencia_alvo_frames: u64,
    pub modo: ModoAgendamento,
    pub suavizar_ancora: bool,
    /// Atraso máximo de despertar do callback (uniforme em [0, J]).
    pub jitter_despertar_max_ns: u64,
    /// Atraso extra ocasional (pico) e sua probabilidade por callback, em %.
    pub pico_despertar_ns: u64,
    pub prob_pico_pct: u32,
    /// Atraso máximo entre a captura do evento e o envio à fila.
    pub atraso_entrega_max_ns: u64,
    /// Deriva do cristal do dispositivo em relação ao relógio monotônico.
    pub deriva_ppm: f64,
    pub eventos: usize,
    pub semente: u64,
}

impl CenarioJitter {
    /// Folga de latência: `latência − (bloco + J + pico + entrega)` em frames.
    /// Negativa = latência alvo curta demais, eventos podem chegar atrasados.
    pub fn folga_frames(&self) -> i64 {
        let Some(taxa) = TaxaAmostragem::nova(self.taxa_hz) else {
            return i64::MIN;
        };
        let pior_ns =
            self.jitter_despertar_max_ns + self.pico_despertar_ns + self.atraso_entrega_max_ns;
        self.latencia_alvo_frames as i64
            - self.bloco as i64
            - ms_para_frames(pior_ns as f64 / 1e6, taxa) as i64
    }

    /// Cenário que deve manter erro ≤ 1 bloco e nenhum atraso.
    pub fn dentro_da_especificacao(&self) -> bool {
        self.modo == ModoAgendamento::Agendado && self.folga_frames() >= 0
    }
}

/// Média, p95 e máximo.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Estatistica {
    pub media: f64,
    pub p95: f64,
    pub max: f64,
}

impl Estatistica {
    pub fn de(valores: &[f64]) -> Self {
        if valores.is_empty() {
            return Self::default();
        }
        let mut v = valores.to_vec();
        v.sort_by(f64::total_cmp);
        let n = v.len();
        let idx95 = ((0.95 * n as f64).ceil() as usize).clamp(1, n) - 1;
        Self {
            media: v.iter().sum::<f64>() / n as f64,
            p95: v[idx95],
            max: v[n - 1],
        }
    }
}

/// Resultado de um cenário.
#[derive(Clone, Debug, PartialEq)]
pub struct ResultadoJitter {
    pub cenario: CenarioJitter,
    pub eventos_enviados: usize,
    pub impulsos_detectados: usize,
    pub comandos_atrasados: u64,
    /// Erro com sinal (frames) de cada evento: detectado − ideal.
    /// No modo agendado o ideal inclui a latência alvo; no imediato, não
    /// (o erro é a própria latência, variável).
    pub erros_frames: Vec<i64>,
}

impl ResultadoJitter {
    pub fn erro_medio(&self) -> f64 {
        if self.erros_frames.is_empty() {
            return 0.0;
        }
        self.erros_frames.iter().sum::<i64>() as f64 / self.erros_frames.len() as f64
    }

    /// |erro| em frames (contra o alvo ideal).
    pub fn erro_absoluto(&self) -> Estatistica {
        let v: Vec<f64> = self
            .erros_frames
            .iter()
            .map(|e| e.unsigned_abs() as f64)
            .collect();
        Estatistica::de(&v)
    }

    /// Jitter: |erro − erro médio| em frames (independe do viés constante).
    pub fn jitter(&self) -> Estatistica {
        let m = self.erro_medio();
        let v: Vec<f64> = self
            .erros_frames
            .iter()
            .map(|e| (*e as f64 - m).abs())
            .collect();
        Estatistica::de(&v)
    }

    pub fn todos_detectados(&self) -> bool {
        self.impulsos_detectados == self.eventos_enviados
    }
}

/// Gerador pseudoaleatório determinístico (xorshift64*).
#[derive(Clone, Debug)]
pub struct Pseudoaleatorio(u64);

impl Pseudoaleatorio {
    pub fn novo(semente: u64) -> Self {
        Self(semente.max(1))
    }

    pub fn proximo(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniforme em [0, max].
    pub fn ate(&mut self, max: u64) -> u64 {
        if max == 0 {
            0
        } else {
            self.proximo() % (max + 1)
        }
    }
}

const T0_NS: u64 = 1_000_000_000;
const INTERVALO_NS: u64 = 21_337_000;
const DURACAO_NOTA_NS: u64 = 5_000_000;

#[derive(Clone, Copy)]
struct EventoSim {
    chegada_ns: u64,
    evento: EventoMidi,
}

/// Executa um cenário com o sintetizador falso.
pub fn medir_cenario(c: &CenarioJitter) -> ResultadoJitter {
    let vazio = ResultadoJitter {
        cenario: *c,
        eventos_enviados: 0,
        impulsos_detectados: 0,
        comandos_atrasados: 0,
        erros_frames: Vec::new(),
    };
    let Some(taxa) = TaxaAmostragem::nova(c.taxa_hz) else {
        return vazio;
    };
    let config = ConfigMotor {
        latencia_alvo_frames: c.latencia_alvo_frames,
        capacidade_fila: 4_096,
        capacidade_agendados: 4_096,
        max_frames_bloco: c.bloco.max(1),
        ancora: if c.suavizar_ancora {
            ConfigAncora::default()
        } else {
            ConfigAncora {
                alfa: 1.0,
                ..ConfigAncora::default()
            }
        },
        ..ConfigMotor::padrao(taxa)
    };
    let Ok((mut motor, mut controle)) = MotorAudio::novo(config, SintetizadorFalso::novo()) else {
        return vazio;
    };
    let mut rng = Pseudoaleatorio::novo(c.semente);
    let Some(canal) = CanalMidi::novo(0) else {
        return vazio;
    };

    // Eventos: nota ligada em e_i, desligada 5 ms depois.
    let mut instantes = Vec::with_capacity(c.eventos);
    let mut fila_sim: Vec<EventoSim> = Vec::with_capacity(c.eventos * 2);
    for i in 0..c.eventos {
        let e = T0_NS + 20_000_000 + i as u64 * INTERVALO_NS + rng.ate(INTERVALO_NS / 2);
        instantes.push(e);
        let Some(nota) = NotaMidi::nova(60 + (i % 12) as u8) else {
            continue;
        };
        fila_sim.push(EventoSim {
            chegada_ns: e + rng.ate(c.atraso_entrega_max_ns),
            evento: EventoMidi::NotaLigada {
                canal,
                nota,
                velocidade: 127,
                timestamp_monotonico_ns: e,
            },
        });
        let desl = e + DURACAO_NOTA_NS;
        fila_sim.push(EventoSim {
            chegada_ns: desl + rng.ate(c.atraso_entrega_max_ns),
            evento: EventoMidi::NotaDesligada {
                canal,
                nota,
                velocidade: 0,
                timestamp_monotonico_ns: desl,
            },
        });
    }
    fila_sim.sort_by_key(|s| s.chegada_ns);

    let taxa_real = f64::from(c.taxa_hz) * (1.0 + c.deriva_ppm * 1e-6);
    let periodo_ns = c.bloco as f64 * NS_POR_SEGUNDO as f64 / taxa_real;
    let ultimo = instantes.last().copied().unwrap_or(T0_NS) + 100_000_000;
    let mut saida = vec![0.0_f32; c.bloco * 2];
    let mut detectados: Vec<u64> = Vec::with_capacity(c.eventos);
    let mut proximo_evento = 0;
    let mut k = 0_u64;
    loop {
        let ideal = T0_NS as f64 + k as f64 * periodo_ns;
        if ideal > ultimo as f64 {
            break;
        }
        let mut atraso = rng.ate(c.jitter_despertar_max_ns);
        if c.prob_pico_pct > 0 && rng.ate(99) < u64::from(c.prob_pico_pct) {
            atraso += c.pico_despertar_ns;
        }
        let agora = ideal.round() as u64 + atraso;
        while proximo_evento < fila_sim.len() && fila_sim[proximo_evento].chegada_ns <= agora {
            let ev = fila_sim[proximo_evento].evento;
            let enviado = match c.modo {
                ModoAgendamento::Agendado => controle.enviar_evento_midi(&ev).is_ok(),
                ModoAgendamento::Imediato => ComandoAudio::de_evento_midi(&ev)
                    .map(|cmd| ComandoAudio::novo(Quando::Imediato, cmd.acao))
                    .is_some_and(|cmd| controle.enviar(cmd).is_ok()),
            };
            debug_assert!(enviado, "fila de simulação não deve encher");
            proximo_evento += 1;
        }
        let base = motor.frame_atual();
        motor.processar_intercalado(&mut saida, 2, agora);
        for (i, q) in saida.as_chunks::<2>().0.iter().enumerate() {
            if q[0] != 0.0 {
                detectados.push(base + i as u64);
            }
        }
        k += 1;
    }

    let lat = match c.modo {
        ModoAgendamento::Agendado => c.latencia_alvo_frames as i64,
        ModoAgendamento::Imediato => 0,
    };
    let erros_frames = instantes
        .iter()
        .zip(detectados.iter())
        .map(|(e, d)| {
            let ideal = ((e - T0_NS) as f64 * taxa_real / NS_POR_SEGUNDO as f64).round() as i64;
            *d as i64 - (ideal + lat)
        })
        .collect();
    ResultadoJitter {
        cenario: *c,
        eventos_enviados: instantes.len(),
        impulsos_detectados: detectados.len(),
        comandos_atrasados: motor.estado().telemetria().comandos_atrasados,
        erros_frames,
    }
}

/// Cenários publicados no relatório (48 kHz e 44,1 kHz).
pub fn cenarios_padrao() -> Vec<CenarioJitter> {
    let base = CenarioJitter {
        nome: "",
        taxa_hz: 48_000,
        bloco: 256,
        latencia_alvo_frames: 480,
        modo: ModoAgendamento::Agendado,
        suavizar_ancora: true,
        jitter_despertar_max_ns: 1_000_000,
        pico_despertar_ns: 0,
        prob_pico_pct: 0,
        atraso_entrega_max_ns: 500_000,
        deriva_ppm: 0.0,
        eventos: 2_000,
        semente: 0x5EED_0001,
    };
    vec![
        CenarioJitter {
            nome: "A: bloco 64, âncora crua",
            bloco: 64,
            latencia_alvo_frames: 256,
            suavizar_ancora: false,
            jitter_despertar_max_ns: 500_000,
            atraso_entrega_max_ns: 200_000,
            semente: 0x5EED_000A,
            ..base
        },
        CenarioJitter {
            nome: "B: bloco 128, âncora crua",
            bloco: 128,
            suavizar_ancora: false,
            semente: 0x5EED_000B,
            ..base
        },
        CenarioJitter {
            nome: "C: bloco 256, âncora crua, picos",
            latencia_alvo_frames: 720,
            suavizar_ancora: false,
            pico_despertar_ns: 3_000_000,
            prob_pico_pct: 1,
            semente: 0x5EED_000C,
            ..base
        },
        CenarioJitter {
            nome: "D: bloco 256, âncora suavizada, picos",
            latencia_alvo_frames: 720,
            pico_despertar_ns: 3_000_000,
            prob_pico_pct: 1,
            semente: 0x5EED_000C,
            ..base
        },
        CenarioJitter {
            nome: "E: bloco 512, suavizada, deriva +100 ppm",
            bloco: 512,
            latencia_alvo_frames: 1_440,
            jitter_despertar_max_ns: 2_000_000,
            atraso_entrega_max_ns: 1_000_000,
            deriva_ppm: 100.0,
            semente: 0x5EED_000E,
            ..base
        },
        CenarioJitter {
            nome: "F: 44,1 kHz, bloco 256, suavizada, deriva −50 ppm",
            taxa_hz: 44_100,
            latencia_alvo_frames: 441,
            deriva_ppm: -50.0,
            semente: 0x5EED_000F,
            ..base
        },
        CenarioJitter {
            nome: "G: bloco 256 sem agendamento (imediato)",
            modo: ModoAgendamento::Imediato,
            semente: 0x5EED_0010,
            ..base
        },
        CenarioJitter {
            nome: "H: bloco 256, latência alvo curta demais (128)",
            latencia_alvo_frames: 128,
            semente: 0x5EED_0011,
            ..base
        },
    ]
}

/// Resultado da medição de quantização do `rustysynth`.
#[derive(Clone, Debug, PartialEq)]
pub struct ResultadoRusty {
    pub bloco_interno: usize,
    pub taxa_hz: u32,
    pub eventos: usize,
    pub detectados: usize,
    /// Atraso (frames) entre o frame agendado e o primeiro frame audível.
    pub atrasos_frames: Vec<i64>,
}

impl ResultadoRusty {
    pub fn estatistica(&self) -> Estatistica {
        let v: Vec<f64> = self.atrasos_frames.iter().map(|x| *x as f64).collect();
        Estatistica::de(&v)
    }
}

/// Primeiros frames audíveis (|x| > limiar) após pelo menos `silencio` frames
/// silenciosos. `amostras` intercaladas em `canais`.
pub fn detectar_inicios(amostras: &[f32], canais: usize, limiar: f32, silencio: usize) -> Vec<u64> {
    let mut inicios = Vec::new();
    let mut quietos = silencio;
    for (i, q) in amostras.chunks_exact(canais.max(1)).enumerate() {
        let alto = q.iter().any(|x| x.abs() > limiar);
        if alto {
            if quietos >= silencio {
                inicios.push(i as u64);
            }
            quietos = 0;
        } else {
            quietos += 1;
        }
    }
    inicios
}

/// Mede o atraso de início de nota do `rustysynth` com um sintetizador já
/// criado (SF2 sintético ou real), com callbacks ideais de 256 frames e
/// eventos em frames absolutos conhecidos.
pub fn medir_inicio_rusty<S: Sintetizador>(
    sintetizador: S,
    taxa: TaxaAmostragem,
    bloco_interno: usize,
    eventos: usize,
    semente: u64,
) -> ResultadoRusty {
    let mut res = ResultadoRusty {
        bloco_interno,
        taxa_hz: taxa.hz(),
        eventos,
        detectados: 0,
        atrasos_frames: Vec::new(),
    };
    let config = ConfigMotor {
        capacidade_fila: eventos * 2 + 8,
        capacidade_agendados: eventos * 2 + 8,
        ..ConfigMotor::padrao(taxa)
    };
    let (Ok((mut motor, mut controle)), Some(canal), Some(nota)) = (
        MotorAudio::novo(config, sintetizador),
        CanalMidi::novo(0),
        NotaMidi::nova(69),
    ) else {
        return res;
    };
    let mut rng = Pseudoaleatorio::novo(semente);
    let espaco = ms_para_frames(250.0, taxa);
    let duracao = ms_para_frames(30.0, taxa);
    let mut alvos = Vec::with_capacity(eventos);
    for i in 0..eventos as u64 {
        let f = espaco + i * espaco + rng.ate(espaco / 4);
        alvos.push(f);
        let lig = ComandoAudio::novo(
            Quando::Frame(f),
            Acao::NotaLigada {
                canal,
                nota,
                velocidade: 100,
            },
        );
        // CC120 (All Sound Off): corta a voz sem release, para que o próximo
        // início parta de silêncio exato também com SF2 reais (release longo).
        let desl = ComandoAudio::novo(
            Quando::Frame(f + duracao),
            Acao::Controle {
                canal,
                controle: 120,
                valor: 0,
            },
        );
        if controle.enviar(lig).is_err() || controle.enviar(desl).is_err() {
            return res;
        }
    }
    let total = (espaco * (eventos as u64 + 2)) as usize;
    let saida = crate::offline::renderizar(&mut motor, total, 256, 2, T0_NS);
    let inicios = detectar_inicios(&saida, 2, 1e-5, ms_para_frames(50.0, taxa) as usize);
    res.detectados = inicios.len();
    res.atrasos_frames = alvos
        .iter()
        .zip(inicios.iter())
        .map(|(a, d)| *d as i64 - *a as i64)
        .collect();
    res
}

/// Mede o `rustysynth` com o SF2 sintético gerado em memória (sem reverb).
pub fn medir_rusty_sintetico(bloco_interno: usize, eventos: usize) -> Option<ResultadoRusty> {
    let taxa = TaxaAmostragem::nova(48_000)?;
    let bytes = sf2_sintetico::gerar();
    let sf = ler_soundfont(&mut bytes.as_slice()).ok()?;
    let sint = SintetizadorRusty::novo(
        &sf,
        taxa,
        ConfigRusty {
            bloco_interno,
            reverb_e_chorus: false,
            ..ConfigRusty::default()
        },
    )
    .ok()?;
    Some(medir_inicio_rusty(
        sint,
        taxa,
        bloco_interno,
        eventos,
        0x5EED_0100,
    ))
}

fn ms(frames: f64, taxa_hz: u32) -> f64 {
    TaxaAmostragem::nova(taxa_hz).map_or(0.0, |t| frames_para_ms(frames, t))
}

/// Relatório Markdown determinístico (sem data nem caminho local).
pub fn relatorio_markdown(resultados: &[ResultadoJitter], rusty: &[ResultadoRusty]) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# F03 — Jitter do relógio mestre (render offline)");
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "> **Ambiente:** RENDER OFFLINE NA NUVEM, sem dispositivo de áudio. Os atrasos de \
         despertar do callback são **modelados** (pseudoaleatórios, sementes fixas), não medidos \
         no PC de referência. Este relatório **não** comprova a latência MIDI→áudio real \
         (meta p95 ≤ 50 ms, ticket #39): **NÃO VERIFICADO**."
    );
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "Gerado por `cargo test -p piano-tutor-audio --test jitter_offline` \
         (com `PT_PUBLICAR_JITTER=1` para regravar). O teste falha se este arquivo divergir do \
         código ou se algum cenário dentro da especificação errar mais de 1 bloco."
    );
    let _ = writeln!(s);
    let _ = writeln!(s, "## Metodologia");
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "1. Dispositivo simulado consome frames a `taxa × (1 + deriva)`; o callback `k` acorda \
         em `t0 + k·bloco/taxa_real + atraso`, com atraso uniforme em `[0, J]` e, quando indicado, \
         picos de +3 ms em 1% dos callbacks."
    );
    let _ = writeln!(
        s,
        "2. 2.000 notas com timestamps monotônicos conhecidos (intervalo ≈ 21,3 ms + deslocamento \
         pseudoaleatório) chegam à fila com atraso de entrega uniforme em `[0, D]` e seguem o \
         caminho real: `ControleAudio` → fila `rtrb` → `MotorAudio` → sintetizador."
    );
    let _ = writeln!(
        s,
        "3. O sintetizador falso emite um impulso no frame em que recebe a nota; a saída é \
         varrida e cada impulso é comparado ao frame ideal `round((e − t0)·taxa_real) + latência \
         alvo`. Erro negativo = soou antes do ideal."
    );
    let _ = writeln!(
        s,
        "4. *Erro* é contra o alvo ideal (inclui o viés constante da âncora); *jitter* é \
         `|erro − erro médio|` (variação, sem o viés). Folga = latência − (bloco + J + pico + D)."
    );
    let _ = writeln!(s);
    let _ = writeln!(s, "## Resultados do agendador (sintetizador falso)");
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "| Cenário | Taxa | Bloco | Latência alvo | J / D | Folga | Detectados | Erro médio | \
         \\|Erro\\| p95 | \\|Erro\\| máx | Jitter p95 | Jitter máx | Atrasados | ≤ 1 bloco |"
    );
    let _ = writeln!(
        s,
        "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"
    );
    for r in resultados {
        let c = &r.cenario;
        let ea = r.erro_absoluto();
        let j = r.jitter();
        let lat = match c.modo {
            ModoAgendamento::Agendado => format!(
                "{} fr ({:.2} ms)",
                c.latencia_alvo_frames,
                ms(c.latencia_alvo_frames as f64, c.taxa_hz)
            ),
            ModoAgendamento::Imediato => "— (imediato)".to_string(),
        };
        let folga = match c.modo {
            ModoAgendamento::Agendado => format!("{} fr", c.folga_frames()),
            ModoAgendamento::Imediato => "—".to_string(),
        };
        let dentro = if c.modo == ModoAgendamento::Imediato {
            if j.max <= c.bloco as f64 {
                "sim (jitter)"
            } else {
                "não (jitter)"
            }
        } else if ea.max <= c.bloco as f64 {
            "sim"
        } else {
            "não"
        };
        let _ = writeln!(
            s,
            "| {} | {} Hz | {} | {} | {:.1} / {:.1} ms | {} | {}/{} | {:.2} fr ({:.3} ms) | {:.0} fr ({:.3} ms) | {:.0} fr ({:.3} ms) | {:.2} fr ({:.3} ms) | {:.2} fr ({:.3} ms) | {} | {} |",
            c.nome,
            c.taxa_hz,
            c.bloco,
            lat,
            c.jitter_despertar_max_ns as f64 / 1e6,
            c.atraso_entrega_max_ns as f64 / 1e6,
            folga,
            r.impulsos_detectados,
            r.eventos_enviados,
            r.erro_medio(),
            ms(r.erro_medio(), c.taxa_hz),
            ea.p95,
            ms(ea.p95, c.taxa_hz),
            ea.max,
            ms(ea.max, c.taxa_hz),
            j.p95,
            ms(j.p95, c.taxa_hz),
            j.max,
            ms(j.max, c.taxa_hz),
            r.comandos_atrasados,
            dentro
        );
    }
    let _ = writeln!(s);
    let _ = writeln!(s, "Leitura:");
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "- Com folga ≥ 0 (A–F), todo evento soa a menos de 1 bloco do alvo e nenhum chega \
         atrasado. O erro residual é o atraso de despertar do callback que a âncora não remove."
    );
    let _ = writeln!(
        s,
        "- A âncora suavizada (D) reduz o jitter em relação à crua (C) com os mesmos atrasos \
         (mesma semente), ao custo de um viés constante, que é só latência fixa."
    );
    let _ = writeln!(
        s,
        "- Sem agendamento (G), a latência é menor, mas varia em até ~1 bloco + J (jitter alto)."
    );
    let _ = writeln!(
        s,
        "- Com latência alvo menor que o bloco (H), parte dos eventos chega atrasada e é tocada \
         no início do bloco seguinte (contados em `comandos_atrasados`). Configuração não \
         recomendada; publicada para mostrar o limite."
    );
    let _ = writeln!(s);
    let _ = writeln!(s, "## Quantização interna do `rustysynth` (SF2 sintético)");
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "O `rustysynth` renderiza em blocos internos; um evento aplicado no meio de um bloco \
         interno só soa no início do bloco interno seguinte. Medido com um SF2 **sintético gerado \
         no teste** (onda quadrada própria, sem amostras de terceiros, sem reverb), callbacks \
         ideais de 256 frames e notas em frames absolutos conhecidos. Atraso = primeiro frame \
         audível − frame agendado (faixa esperada: 0 a bloco interno, pela quantização mais até 1 frame da \
         rampa de ganho da voz). O envelope do SF2 \
         sintético tem atraso e ataque mínimos; num SF2 real o envelope do preset soma-se a isso."
    );
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "| Bloco interno | Taxa | Detectados | Atraso médio | Atraso p95 | Atraso máx |"
    );
    let _ = writeln!(s, "|---|---|---|---|---|---|");
    for r in rusty {
        let e = r.estatistica();
        let _ = writeln!(
            s,
            "| {} | {} Hz | {}/{} | {:.2} fr ({:.3} ms) | {:.0} fr ({:.3} ms) | {:.0} fr ({:.3} ms) |",
            r.bloco_interno,
            r.taxa_hz,
            r.detectados,
            r.eventos,
            e.media,
            ms(e.media, r.taxa_hz),
            e.p95,
            ms(e.p95, r.taxa_hz),
            e.max,
            ms(e.max, r.taxa_hz)
        );
    }
    let _ = writeln!(s);
    let _ = writeln!(s, "## Limites e pendências");
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "- Atrasos de despertar, deriva e atraso de entrega são **modelos**. O jitter real do \
         callback ALSA/PipeWire no cerebro: **NÃO VERIFICADO**."
    );
    let _ = writeln!(
        s,
        "- Latência MIDI→áudio por captura/loopback (p95 ≤ 50 ms): **NÃO VERIFICADO** (ticket #39, \
         exige REVAS e PC de referência)."
    );
    let _ = writeln!(
        s,
        "- Contrato: o timestamp dos eventos MIDI deve vir do mesmo `RelogioMonotonico` usado no \
         callback; o timestamp próprio do `midir` tem outra origem."
    );
    s
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn estatistica_de_valores_conhecidos() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        let e = Estatistica::de(&v);
        assert_eq!(e.media, 50.5);
        assert_eq!(e.p95, 95.0);
        assert_eq!(e.max, 100.0);
        assert_eq!(Estatistica::de(&[]), Estatistica::default());
    }

    #[test]
    fn pseudoaleatorio_e_deterministico_e_limitado() {
        let mut a = Pseudoaleatorio::novo(7);
        let mut b = Pseudoaleatorio::novo(7);
        for _ in 0..1_000 {
            let x = a.ate(10);
            assert_eq!(x, b.ate(10));
            assert!(x <= 10);
        }
        assert_eq!(Pseudoaleatorio::novo(0).ate(0), 0);
    }

    #[test]
    fn sem_jitter_o_erro_e_zero() {
        let c = CenarioJitter {
            nome: "ideal",
            jitter_despertar_max_ns: 0,
            atraso_entrega_max_ns: 0,
            suavizar_ancora: false,
            eventos: 200,
            ..cenarios_padrao()[1]
        };
        let r = medir_cenario(&c);
        assert!(r.todos_detectados());
        assert_eq!(r.comandos_atrasados, 0);
        assert!(
            r.erros_frames.iter().all(|e| *e == 0),
            "{:?}",
            r.erros_frames
        );
    }

    #[test]
    fn folga_identifica_latencia_curta() {
        let cs = cenarios_padrao();
        assert!(
            cs.iter()
                .filter(|c| c.nome.starts_with('H'))
                .all(|c| !c.dentro_da_especificacao())
        );
        assert!(
            cs.iter()
                .filter(|c| c.nome.starts_with('A'))
                .all(|c| c.dentro_da_especificacao())
        );
    }

    #[test]
    fn detector_de_inicios() {
        let x = [0.0, 0.0, 0.5, 0.4, 0.0, 0.0, 0.0, 0.3, 0.0, 0.2];
        assert_eq!(detectar_inicios(&x, 1, 0.1, 2), vec![2, 7]);
    }
}
