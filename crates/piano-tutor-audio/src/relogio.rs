//! Relógio mestre de áudio (ADR-0005, spec §3.4).
//!
//! A **fonte única de tempo** do motor é a contagem de frames de amostra
//! entregues pelo callback de áudio. Metrônomo (F08), playback (F06) e pads
//! (F09) devem agendar em frames deste relógio, nunca em relógio de parede.
//!
//! Para tocar eventos MIDI capturados com timestamp monotônico, o callback
//! mantém uma **âncora** que relaciona frames a nanossegundos do relógio
//! monotônico ([`RelogioMonotonico`]). A âncora é suavizada por média móvel
//! exponencial para que o atraso de despertar do callback não vire jitter de
//! posicionamento. Toda a lógica aqui é pura e testada por unidade.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::motor::EstadoMotor;

/// Nanossegundos por segundo.
pub const NS_POR_SEGUNDO: u64 = 1_000_000_000;

/// Taxa de amostragem em Hz, nunca zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaxaAmostragem(NonZeroU32);

impl TaxaAmostragem {
    /// `None` quando `hz == 0`.
    pub const fn nova(hz: u32) -> Option<Self> {
        match NonZeroU32::new(hz) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    pub const fn hz(self) -> u32 {
        self.0.get()
    }
}

/// Divisão inteira arredondada ao mais próximo (meio para cima), `d > 0`.
fn div_arredondada(n: i128, d: i128) -> i128 {
    (2 * n + d).div_euclid(2 * d)
}

/// Converte frames em nanossegundos, arredondando ao mais próximo.
pub fn frames_para_ns(frames: u64, taxa: TaxaAmostragem) -> u64 {
    let ns = div_arredondada(
        i128::from(frames) * i128::from(NS_POR_SEGUNDO),
        i128::from(taxa.hz()),
    );
    u64::try_from(ns).unwrap_or(u64::MAX)
}

/// Converte nanossegundos em frames, arredondando ao mais próximo.
pub fn ns_para_frames(ns: u64, taxa: TaxaAmostragem) -> u64 {
    let frames = div_arredondada(
        i128::from(ns) * i128::from(taxa.hz()),
        i128::from(NS_POR_SEGUNDO),
    );
    u64::try_from(frames).unwrap_or(u64::MAX)
}

/// Versão com sinal de [`ns_para_frames`], usada pela âncora.
pub fn ns_para_frames_com_sinal(ns: i128, taxa: TaxaAmostragem) -> i128 {
    div_arredondada(ns * i128::from(taxa.hz()), i128::from(NS_POR_SEGUNDO))
}

/// Converte milissegundos em frames (para latência alvo configurável).
pub fn ms_para_frames(ms: f64, taxa: TaxaAmostragem) -> u64 {
    if !ms.is_finite() || ms <= 0.0 {
        return 0;
    }
    (ms * f64::from(taxa.hz()) / 1000.0).round() as u64
}

/// Converte frames em milissegundos (relatórios).
pub fn frames_para_ms(frames: f64, taxa: TaxaAmostragem) -> f64 {
    frames * 1000.0 / f64::from(taxa.hz())
}

/// Relógio monotônico compartilhado entre captura MIDI e callback de áudio.
///
/// **Contrato:** o timestamp `timestamp_monotonico_ns` dos eventos MIDI que
/// serão agendados precisa vir deste mesmo relógio (mesma época). O timestamp
/// próprio do `midir` usa outra origem e não pode ser misturado.
#[derive(Clone, Copy, Debug)]
pub struct RelogioMonotonico {
    epoca: Instant,
}

impl RelogioMonotonico {
    pub fn novo() -> Self {
        Self {
            epoca: Instant::now(),
        }
    }

    /// Nanossegundos desde a época. Sem alocação nem lock (seguro no callback).
    pub fn agora_ns(&self) -> u64 {
        u64::try_from(self.epoca.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }
}

impl Default for RelogioMonotonico {
    fn default() -> Self {
        Self::novo()
    }
}

/// Parâmetros da âncora frames ↔ relógio monotônico.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConfigAncora {
    /// Peso da nova medida na média móvel exponencial (0 < alfa ≤ 1).
    /// `1.0` desliga a suavização (âncora crua do último callback).
    pub alfa: f64,
    /// Desvio acima do qual a âncora é refeita do zero (xrun, suspensão,
    /// troca de dispositivo), em nanossegundos.
    pub limite_reancoragem_ns: u64,
}

impl Default for ConfigAncora {
    fn default() -> Self {
        Self {
            alfa: 1.0 / 32.0,
            limite_reancoragem_ns: 20_000_000,
        }
    }
}

/// Estimador da relação `instante_ns = deslocamento_ns + frames_para_ns(frame)`.
///
/// Cada callback informa o frame inicial do bloco e o instante monotônico em
/// que acordou. O instante de despertar oscila (escalonador do SO); a média
/// móvel exponencial reduz essa oscilação. Saltos grandes refazem a âncora.
#[derive(Clone, Debug)]
pub struct EstimadorAncora {
    taxa: TaxaAmostragem,
    config: ConfigAncora,
    deslocamento_ns: Option<f64>,
    reancoragens: u64,
}

impl EstimadorAncora {
    pub fn novo(taxa: TaxaAmostragem, config: ConfigAncora) -> Self {
        let alfa = if config.alfa.is_finite() && config.alfa > 0.0 {
            config.alfa.min(1.0)
        } else {
            1.0
        };
        Self {
            taxa,
            config: ConfigAncora { alfa, ..config },
            deslocamento_ns: None,
            reancoragens: 0,
        }
    }

    /// Registra o despertar de um callback cujo bloco começa em `frame`.
    pub fn atualizar(&mut self, frame: u64, agora_ns: u64) {
        let medida = agora_ns as f64 - frames_para_ns(frame, self.taxa) as f64;
        self.deslocamento_ns = Some(match self.deslocamento_ns {
            None => medida,
            Some(atual) => {
                let desvio = medida - atual;
                if desvio.abs() > self.config.limite_reancoragem_ns as f64 {
                    self.reancoragens += 1;
                    medida
                } else {
                    atual + self.config.alfa * desvio
                }
            }
        });
    }

    /// Deslocamento atual em ns (`None` antes do primeiro callback).
    pub fn deslocamento_ns(&self) -> Option<i64> {
        self.deslocamento_ns.map(|d| d.round() as i64)
    }

    /// Quantas vezes a âncora foi refeita por salto grande.
    pub fn reancoragens(&self) -> u64 {
        self.reancoragens
    }

    /// Frame do relógio mestre correspondente a um instante monotônico
    /// (sem latência). Pode ser negativo para instantes anteriores à âncora.
    pub fn frame_de_instante(&self, instante_ns: u64) -> Option<i64> {
        let desloc = self.deslocamento_ns()?;
        let relativo = i128::from(instante_ns) - i128::from(desloc);
        let frame = ns_para_frames_com_sinal(relativo, self.taxa);
        Some(i64::try_from(frame).unwrap_or(if frame < 0 { i64::MIN } else { i64::MAX }))
    }

    /// Frame de destino de um evento capturado em `instante_ns`, somando a
    /// latência alvo. Satura em zero.
    pub fn frame_destino(&self, instante_ns: u64, latencia_frames: u64) -> Option<u64> {
        let base = self.frame_de_instante(instante_ns)?;
        let destino = i128::from(base) + i128::from(latencia_frames);
        Some(u64::try_from(destino.max(0)).unwrap_or(u64::MAX))
    }
}

/// Leitura do relógio mestre por outras threads (UI, metrônomo, playback).
///
/// Lê atômicos publicados pelo callback; não bloqueia o callback.
#[derive(Clone, Debug)]
pub struct LeitorRelogio {
    estado: Arc<EstadoMotor>,
    taxa: TaxaAmostragem,
}

impl LeitorRelogio {
    pub(crate) fn novo(estado: Arc<EstadoMotor>, taxa: TaxaAmostragem) -> Self {
        Self { estado, taxa }
    }

    pub fn taxa(&self) -> TaxaAmostragem {
        self.taxa
    }

    /// Frames já renderizados pelo callback (posição do relógio mestre).
    pub fn frames(&self) -> u64 {
        self.estado.frames_renderizados.load(Ordering::Acquire)
    }

    /// Tempo de áudio decorrido, em ns, derivado só dos frames.
    pub fn tempo_audio_ns(&self) -> u64 {
        frames_para_ns(self.frames(), self.taxa)
    }

    /// Instante monotônico estimado em que o callback trataria `frame`.
    /// `None` antes do primeiro callback.
    pub fn instante_de_frame(&self, frame: u64) -> Option<u64> {
        let desloc = self.estado.deslocamento_ns.load(Ordering::Acquire);
        if desloc == EstadoMotor::SEM_ANCORA {
            return None;
        }
        let ns = i128::from(desloc) + i128::from(frames_para_ns(frame, self.taxa));
        u64::try_from(ns.max(0)).ok()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn taxa(hz: u32) -> TaxaAmostragem {
        TaxaAmostragem::nova(hz).expect("taxa não nula")
    }

    #[test]
    fn taxa_zero_e_rejeitada() {
        assert!(TaxaAmostragem::nova(0).is_none());
        assert_eq!(taxa(48_000).hz(), 48_000);
    }

    #[test]
    fn conversoes_exatas_em_48k() {
        let t = taxa(48_000);
        assert_eq!(frames_para_ns(48_000, t), NS_POR_SEGUNDO);
        assert_eq!(frames_para_ns(48, t), 1_000_000);
        assert_eq!(ns_para_frames(NS_POR_SEGUNDO, t), 48_000);
        assert_eq!(ns_para_frames(1_000_000, t), 48);
        assert_eq!(ms_para_frames(10.0, t), 480);
        assert_eq!(ms_para_frames(-1.0, t), 0);
        assert_eq!(ms_para_frames(f64::NAN, t), 0);
    }

    #[test]
    fn conversoes_arredondam_em_44k1() {
        let t = taxa(44_100);
        // 1 frame = 22675.736... ns
        assert_eq!(frames_para_ns(1, t), 22_676);
        assert_eq!(ns_para_frames(22_675, t), 1);
        assert_eq!(ns_para_frames(11_337, t), 0);
        assert_eq!(ns_para_frames(11_338, t), 1);
    }

    #[test]
    fn ida_e_volta_frames_ns_sem_deriva() {
        for hz in [44_100, 48_000, 96_000] {
            let t = taxa(hz);
            for frames in [0_u64, 1, 63, 64, 1_000, 48_000, 10_000_000_000] {
                assert_eq!(ns_para_frames(frames_para_ns(frames, t), t), frames);
            }
        }
    }

    #[test]
    fn horas_de_audio_nao_estouram() {
        let t = taxa(192_000);
        let dez_horas = 192_000_u64 * 3600 * 10;
        assert_eq!(frames_para_ns(dez_horas, t), 36_000 * NS_POR_SEGUNDO);
        assert_eq!(frames_para_ns(u64::MAX, t), u64::MAX);
    }

    #[test]
    fn ancora_crua_mapeia_instante_para_frame() {
        let t = taxa(48_000);
        let mut est = EstimadorAncora::novo(
            t,
            ConfigAncora {
                alfa: 1.0,
                ..ConfigAncora::default()
            },
        );
        assert_eq!(est.frame_de_instante(0), None);
        // Bloco que começa no frame 4800 tratado no instante 1,1 s:
        // frame 0 ↔ 1,0 s.
        est.atualizar(4_800, 1_100_000_000);
        assert_eq!(est.deslocamento_ns(), Some(1_000_000_000));
        assert_eq!(est.frame_de_instante(1_000_000_000), Some(0));
        assert_eq!(est.frame_de_instante(1_200_000_000), Some(9_600));
        assert_eq!(est.frame_de_instante(999_000_000), Some(-48));
        assert_eq!(est.frame_destino(1_200_000_000, 480), Some(10_080));
        assert_eq!(est.frame_destino(0, 480), Some(0));
    }

    #[test]
    fn ancora_suavizada_reduz_oscilacao_de_despertar() {
        let t = taxa(48_000);
        let mut est = EstimadorAncora::novo(t, ConfigAncora::default());
        // Despertares alternando 0 e +1 ms em torno do ideal.
        for k in 0..2_000_u64 {
            let frame = k * 256;
            let ideal = frames_para_ns(frame, t);
            let atraso = if k % 2 == 0 { 0 } else { 1_000_000 };
            est.atualizar(frame, 5_000_000_000 + ideal + atraso);
        }
        let desloc = est.deslocamento_ns().expect("ancorado");
        // Converge para a média (0,5 ms), com oscilação residual pequena.
        let erro = (desloc - 5_000_500_000).abs();
        assert!(erro < 40_000, "deslocamento {desloc}");
        assert_eq!(est.reancoragens(), 0);
    }

    #[test]
    fn salto_grande_refaz_ancora() {
        let t = taxa(48_000);
        let mut est = EstimadorAncora::novo(t, ConfigAncora::default());
        est.atualizar(0, 1_000_000_000);
        est.atualizar(256, 1_000_000_000 + frames_para_ns(256, t) + 100_000_000);
        assert_eq!(est.reancoragens(), 1);
        assert_eq!(est.deslocamento_ns(), Some(1_100_000_000));
    }

    #[test]
    fn alfa_invalido_vira_ancora_crua() {
        let t = taxa(48_000);
        let mut est = EstimadorAncora::novo(
            t,
            ConfigAncora {
                alfa: f64::NAN,
                ..ConfigAncora::default()
            },
        );
        est.atualizar(0, 1_000);
        est.atualizar(0, 2_000);
        assert_eq!(est.deslocamento_ns(), Some(2_000));
    }

    #[test]
    fn relogio_monotonico_avanca() {
        let r = RelogioMonotonico::novo();
        let a = r.agora_ns();
        let b = r.agora_ns();
        assert!(b >= a);
    }
}
