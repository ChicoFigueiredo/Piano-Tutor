//! Saída de áudio com `cpal` 0.18.2. **Uma única saída serve o motor todo**
//! (spec §3.4): metrônomo, playback e pads futuros entram como comandos no
//! mesmo [`MotorAudio`], nunca como um segundo fluxo.
//!
//! O callback do `cpal` só chama [`MotorAudio::processar_intercalado`]; nada
//! de síntese na thread de UI. Falhas ao abrir viram [`ErroAudio`]; falhas
//! durante a execução (dispositivo removido, fluxo invalidado) são contadas e
//! sinalizadas em [`Telemetria::dispositivo_falhou`]. O app então dispara
//! pânico e reabre a saída, sem derrubar o processo.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, ErrorKind, FromSample, SampleFormat, SizedSample, StreamConfig};

use crate::comando::ControleAudio;
use crate::erro::ErroAudio;
use crate::motor::{ConfigMotor, EstadoMotor, MotorAudio, Telemetria};
use crate::relogio::{
    LeitorRelogio, RelogioMonotonico, TaxaAmostragem, frames_para_ns, ms_para_frames,
};
use crate::sintetizador::Sintetizador;

/// Pedido de configuração da saída.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConfigSaida {
    /// Buffer fixo em frames; `None` usa o padrão do host.
    pub buffer_frames: Option<u32>,
    /// Latência alvo para eventos com timestamp. O efetivo é no mínimo
    /// 2 × buffer (quando o buffer é conhecido).
    pub latencia_alvo_ms: f64,
    pub capacidade_fila: usize,
    /// Tempo máximo para o host montar o fluxo.
    pub tempo_limite: Option<Duration>,
}

impl Default for ConfigSaida {
    fn default() -> Self {
        Self {
            buffer_frames: Some(256),
            latencia_alvo_ms: 10.0,
            capacidade_fila: 1024,
            tempo_limite: Some(Duration::from_secs(3)),
        }
    }
}

/// O que foi efetivamente aberto (para diagnóstico de taxa/buffer).
#[derive(Clone, Debug, PartialEq)]
pub struct InfoSaida {
    pub host: String,
    pub dispositivo: String,
    pub taxa_hz: u32,
    pub canais: u16,
    pub formato: String,
    pub buffer_frames: Option<u32>,
    pub latencia_alvo_frames: u64,
}

/// Saída aberta. Ao ser descartada, o fluxo para.
pub struct SaidaAudio {
    _fluxo: cpal::Stream,
    controle: ControleAudio,
    estado: Arc<EstadoMotor>,
    leitor: LeitorRelogio,
    info: InfoSaida,
}

impl SaidaAudio {
    /// Abre o dispositivo de saída padrão. `criar_sintetizador` recebe a
    /// taxa do dispositivo e roda **nesta** thread (pode carregar SF2).
    pub fn abrir_padrao<S, F>(
        config: ConfigSaida,
        relogio: RelogioMonotonico,
        criar_sintetizador: F,
    ) -> Result<Self, ErroAudio>
    where
        S: Sintetizador + 'static,
        F: FnOnce(TaxaAmostragem) -> Result<S, ErroAudio>,
    {
        let host = cpal::default_host();
        let nome_host = host.id().name().to_string();
        let dispositivo = host
            .default_output_device()
            .ok_or(ErroAudio::SemDispositivo)?;
        let nome = dispositivo
            .description()
            .map(|d| d.to_string())
            .unwrap_or_else(|_| "desconhecido".to_string());
        let suportada = dispositivo.default_output_config().map_err(erro_cpal)?;
        let formato = suportada.sample_format();
        let taxa = TaxaAmostragem::nova(suportada.sample_rate()).ok_or(
            ErroAudio::ParametroInvalido("dispositivo informou taxa zero"),
        )?;
        let mut config_fluxo: StreamConfig = suportada.config();
        config_fluxo.buffer_size = match config.buffer_frames {
            Some(n) if n > 0 => BufferSize::Fixed(n),
            Some(_) => return Err(ErroAudio::ParametroInvalido("buffer_frames deve ser > 0")),
            None => BufferSize::Default,
        };

        let mut latencia = ms_para_frames(config.latencia_alvo_ms, taxa);
        if let Some(b) = config.buffer_frames {
            latencia = latencia.max(2 * u64::from(b));
        }
        let max_bloco = config.buffer_frames.map_or(4096, |b| (b as usize).max(64));
        let config_motor = ConfigMotor {
            latencia_alvo_frames: latencia,
            capacidade_fila: config.capacidade_fila,
            max_frames_bloco: max_bloco,
            ..ConfigMotor::padrao(taxa)
        };
        let sintetizador = criar_sintetizador(taxa)?;
        let (motor, controle) = MotorAudio::novo(config_motor, sintetizador)?;
        let estado = motor.estado();
        let leitor = motor.leitor_relogio();
        let canais = usize::from(config_fluxo.channels);

        let fluxo = match formato {
            SampleFormat::F32 => construir::<f32, S>(
                &dispositivo,
                config_fluxo,
                motor,
                canais,
                relogio,
                &estado,
                config.tempo_limite,
            ),
            SampleFormat::I16 => construir::<i16, S>(
                &dispositivo,
                config_fluxo,
                motor,
                canais,
                relogio,
                &estado,
                config.tempo_limite,
            ),
            SampleFormat::I32 => construir::<i32, S>(
                &dispositivo,
                config_fluxo,
                motor,
                canais,
                relogio,
                &estado,
                config.tempo_limite,
            ),
            SampleFormat::U16 => construir::<u16, S>(
                &dispositivo,
                config_fluxo,
                motor,
                canais,
                relogio,
                &estado,
                config.tempo_limite,
            ),
            outro => return Err(ErroAudio::FormatoNaoSuportado(format!("{outro:?}"))),
        }?;
        fluxo.play().map_err(erro_cpal)?;

        Ok(Self {
            _fluxo: fluxo,
            controle,
            estado,
            leitor,
            info: InfoSaida {
                host: nome_host,
                dispositivo: nome,
                taxa_hz: taxa.hz(),
                canais: config_fluxo.channels,
                formato: format!("{formato:?}"),
                buffer_frames: config.buffer_frames,
                latencia_alvo_frames: latencia,
            },
        })
    }

    pub fn controle(&mut self) -> &mut ControleAudio {
        &mut self.controle
    }

    pub fn leitor_relogio(&self) -> LeitorRelogio {
        self.leitor.clone()
    }

    pub fn info(&self) -> &InfoSaida {
        &self.info
    }

    pub fn telemetria(&self) -> Telemetria {
        self.estado.telemetria()
    }
}

fn construir<T, S>(
    dispositivo: &cpal::Device,
    config: StreamConfig,
    mut motor: MotorAudio<S>,
    canais: usize,
    relogio: RelogioMonotonico,
    estado: &Arc<EstadoMotor>,
    tempo_limite: Option<Duration>,
) -> Result<cpal::Stream, ErroAudio>
where
    T: SizedSample + FromSample<f32>,
    S: Sintetizador + 'static,
{
    // Buffer de conversão pré-alocado; blocos maiores são feitos em partes.
    let mut temporario = vec![0.0_f32; motor.config().max_frames_bloco * canais.max(1)];
    let estado_erro = Arc::clone(estado);
    let dados = move |saida: &mut [T], _info: &cpal::OutputCallbackInfo| {
        let agora = relogio.agora_ns();
        if canais == 0 {
            return;
        }
        let passo = (temporario.len() / canais) * canais;
        if passo == 0 {
            return;
        }
        let taxa = motor.config().taxa;
        let mut feitos = 0_u64;
        for parte in saida.chunks_mut(passo) {
            let tmp = &mut temporario[..parte.len()];
            // Cada parte começa `feitos` frames depois do despertar.
            motor.processar_intercalado(tmp, canais, agora + frames_para_ns(feitos, taxa));
            feitos += (parte.len() / canais) as u64;
            for (d, s) in parte.iter_mut().zip(tmp.iter()) {
                *d = T::from_sample(*s);
            }
        }
    };
    let erro = move |e: cpal::Error| {
        estado_erro
            .erros_dispositivo
            .fetch_add(1, Ordering::Relaxed);
        match e.kind() {
            ErrorKind::Xrun => {
                estado_erro.xruns.fetch_add(1, Ordering::Relaxed);
            }
            ErrorKind::DeviceNotAvailable | ErrorKind::StreamInvalidated => {
                estado_erro
                    .dispositivo_falhou
                    .store(true, Ordering::Release);
            }
            _ => {}
        }
    };
    dispositivo
        .build_output_stream(config, dados, erro, tempo_limite)
        .map_err(erro_cpal)
}

/// Classifica um erro do `cpal` em [`ErroAudio`] recuperável.
fn erro_cpal(e: cpal::Error) -> ErroAudio {
    match e.kind() {
        ErrorKind::DeviceNotAvailable | ErrorKind::HostUnavailable => ErroAudio::SemDispositivo,
        ErrorKind::UnsupportedConfig | ErrorKind::InvalidInput => {
            ErroAudio::ConfiguracaoNaoSuportada(e.to_string())
        }
        _ => ErroAudio::Dispositivo(e.to_string()),
    }
}
