//! Subcomandos de medição de latência (F03-4 e F02-4): efeitos colaterais isolados aqui.
//!
//! - `latencia-listar`: lista dispositivos de entrada/saída de áudio e portas MIDI.
//! - `latencia-loopback`: toca cliques pela saída cpal e grava pela entrada cpal.
//! - `latencia-midi`: pareia cada NoteOn da porta MIDI com o onset gravado pela entrada.
//! - `latencia-quadros`: estatística de contagens de quadros de vídeo (F02-4).
//!
//! A conta (onset, pareamento, estatística, relatório) está em [`crate::latencia`] e é testada
//! sem hardware. O que está aqui só abre dispositivos, copia amostras e carimba instantes com o
//! relógio monotônico do núcleo; **não foi exercitado em hardware** nesta etapa (sem placa de
//! áudio, sem teclado). Os callbacks alocam (`Vec` por bloco) por simplicidade: é uma ferramenta
//! de diagnóstico, não o motor do app, e o instante é lido antes de qualquer alocação.

use crate::latencia::{
    ConfigOnset, DadosMedicao, GeradorCliques, GeradorTom, LIMITE_P95_AUDIO_MS, LIMITE_P95_TELA_MS,
    LinhaDoTempo, Modo, detectar_onsets, eh_nota_ligada, filtrar_isolados, ler_numeros,
    montar_relatorio_latencia, parear, quadros_para_ms, resumir,
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, FromSample, Sample, SampleFormat, SizedSample, StreamConfig};
use piano_tutor_core::tempo::RelogioMonotonico;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::time::{Duration, Instant};

/// Subcomandos de latência.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Subcomando {
    Listar,
    Loopback,
    Midi,
    Quadros,
}

/// Opções comuns da linha de comando.
#[derive(Clone, Debug, PartialEq)]
pub struct Opcoes {
    pub duracao_s: u32,
    pub buffer: u32,
    pub dispositivo_saida: Option<String>,
    pub dispositivo_entrada: Option<String>,
    pub porta_midi: Option<String>,
    /// Arquivo `.md` de saída (sem ele, imprime).
    pub saida_md: Option<String>,
    pub rotulo: Option<String>,
    /// Limiar mínimo (RMS) do detector de onset.
    pub limiar: f32,
    /// Amplitude (0 a 1) dos cliques e do tom gerado.
    pub volume: f32,
    pub gerar_som: bool,
    /// Atraso máximo aceito entre evento e onset.
    pub janela_max_ms: u64,
    pub fps: f64,
    pub arquivo: Option<String>,
}

impl Default for Opcoes {
    fn default() -> Self {
        Self {
            duracao_s: 30,
            buffer: 256,
            dispositivo_saida: None,
            dispositivo_entrada: None,
            porta_midi: None,
            saida_md: None,
            rotulo: None,
            limiar: 0.01,
            volume: 0.5,
            gerar_som: false,
            janela_max_ms: 400,
            fps: 240.0,
            arquivo: None,
        }
    }
}

pub const AJUDA: &str = "\
uso: piano-tutor-diag <subcomando> [opções]

  latencia-listar                    lista saídas, entradas e portas MIDI
  latencia-loopback                  cliques pela saída, gravados pela entrada (cabo ou microfone)
  latencia-midi                      NoteOn da porta MIDI contra o onset gravado
  latencia-quadros <arquivo>         contagens de quadros de vídeo (um número por linha)

opções:
  --duracao <s>               tempo de gravação (padrão 30)
  --buffer <frames>           buffer pedido ao áudio (padrão 256)
  --saida-dispositivo <nome>  trecho do nome da saída (padrão: padrão do sistema)
  --entrada-dispositivo <n>   trecho do nome da entrada
  --porta-midi <nome>         trecho do nome da porta MIDI (padrão: a primeira)
  --gerar-som                 (latencia-midi) este programa toca um tom a cada NoteOn
  --limiar <0-1>              limiar mínimo de onset (padrão 0.01)
  --volume <0-1>              amplitude dos cliques/tom (padrão 0.5)
  --janela-max-ms <ms>        atraso máximo aceito no pareamento (padrão 400)
  --fps <n>                   quadros por segundo da câmera (latencia-quadros, padrão 240)
  --saida <arquivo.md>        grava o relatório (sem isso, imprime)
  --rotulo <texto>            ambiente (ex.: \"cerebro, KB-330, USB\")
";

/// Lê o subcomando e as opções. `Ok(None)` quando o primeiro argumento não é de latência.
pub fn ler_comando(args: &[String]) -> Result<Option<(Subcomando, Opcoes)>, String> {
    let Some(primeiro) = args.first() else {
        return Ok(None);
    };
    let sub = match primeiro.as_str() {
        "latencia-listar" => Subcomando::Listar,
        "latencia-loopback" => Subcomando::Loopback,
        "latencia-midi" => Subcomando::Midi,
        "latencia-quadros" => Subcomando::Quadros,
        _ => return Ok(None),
    };
    let mut op = Opcoes::default();
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        let mut valor = |nome: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{nome} exige um valor"))
        };
        let numero = |v: String, nome: &str| -> Result<f64, String> {
            v.replace(',', ".")
                .parse::<f64>()
                .ok()
                .filter(|x| x.is_finite() && *x > 0.0)
                .ok_or_else(|| format!("{nome}: número positivo esperado, veio '{v}'"))
        };
        match a.as_str() {
            "--duracao" => op.duracao_s = numero(valor(a)?, a)? as u32,
            "--buffer" => op.buffer = numero(valor(a)?, a)? as u32,
            "--saida-dispositivo" => op.dispositivo_saida = Some(valor(a)?),
            "--entrada-dispositivo" => op.dispositivo_entrada = Some(valor(a)?),
            "--porta-midi" => op.porta_midi = Some(valor(a)?),
            "--saida" => op.saida_md = Some(valor(a)?),
            "--rotulo" => op.rotulo = Some(valor(a)?),
            "--limiar" => op.limiar = numero(valor(a)?, a)? as f32,
            "--volume" => op.volume = (numero(valor(a)?, a)? as f32).min(1.0),
            "--janela-max-ms" => op.janela_max_ms = numero(valor(a)?, a)? as u64,
            "--fps" => op.fps = numero(valor(a)?, a)?,
            "--gerar-som" => op.gerar_som = true,
            "--ajuda" | "-h" => return Err(AJUDA.to_owned()),
            outro if !outro.starts_with("--") && op.arquivo.is_none() => {
                op.arquivo = Some(outro.to_owned());
            }
            outro => return Err(format!("argumento desconhecido: {outro}\n\n{AJUDA}")),
        }
    }
    if op.duracao_s == 0 || op.buffer == 0 {
        return Err("duração e buffer devem ser maiores que zero".to_owned());
    }
    Ok(Some((sub, op)))
}

/// Executa o subcomando e devolve o texto (Markdown ou listagem).
pub fn executar(sub: Subcomando, op: &Opcoes) -> Result<String, String> {
    match sub {
        Subcomando::Listar => Ok(listar()),
        Subcomando::Loopback => loopback(op),
        Subcomando::Midi => midi(op),
        Subcomando::Quadros => quadros(op),
    }
}

// --- F02-4 ----------------------------------------------------------------------------------

fn quadros(op: &Opcoes) -> Result<String, String> {
    let Some(arq) = &op.arquivo else {
        return Err("latencia-quadros exige o arquivo com as contagens".to_owned());
    };
    let texto = std::fs::read_to_string(arq).map_err(|e| format!("{arq}: {e}"))?;
    let q = ler_numeros(&texto)?;
    let ms = quadros_para_ms(&q, op.fps);
    let d = DadosMedicao {
        modo: Some(Modo::Quadros),
        rotulo: op.rotulo.clone(),
        limite_p95_ms: LIMITE_P95_TELA_MS,
        fps: Some(op.fps),
        eventos: q.len(),
        resumo: resumir(&ms),
        ..DadosMedicao::default()
    };
    Ok(montar_relatorio_latencia(&d))
}

// --- Listagem -------------------------------------------------------------------------------

fn nome_dispositivo(d: &cpal::Device) -> String {
    d.description()
        .map_or_else(|_| "desconhecido".to_owned(), |x| x.to_string())
}

fn listar() -> String {
    let mut s = String::new();
    let host = cpal::default_host();
    s.push_str("Saídas de áudio:\n");
    match host.output_devices() {
        Ok(it) => it.for_each(|d| s.push_str(&format!("  {}\n", nome_dispositivo(&d)))),
        Err(e) => s.push_str(&format!("  falha ao listar: {e}\n")),
    }
    s.push_str("Entradas de áudio:\n");
    match host.input_devices() {
        Ok(it) => it.for_each(|d| s.push_str(&format!("  {}\n", nome_dispositivo(&d)))),
        Err(e) => s.push_str(&format!("  falha ao listar: {e}\n")),
    }
    s.push_str("Portas MIDI de entrada:\n");
    match midir::MidiInput::new("piano-tutor-diag-listar") {
        Ok(m) => {
            for p in m.ports() {
                s.push_str(&format!("  {}\n", m.port_name(&p).unwrap_or_default()));
            }
        }
        Err(e) => s.push_str(&format!("  backend MIDI indisponível: {e}\n")),
    }
    s.push_str(
        "(nomes como o sistema os informa, para escolher com --*-dispositivo e --porta-midi)\n",
    );
    s
}

// --- Dispositivos de áudio ------------------------------------------------------------------

fn achar_dispositivo(entrada: bool, nome: Option<&str>) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    if let Some(n) = nome {
        let procurado = n.to_lowercase();
        let lista = if entrada {
            host.input_devices()
        } else {
            host.output_devices()
        }
        .map_err(|e| format!("falha ao listar dispositivos: {e}"))?;
        return lista
            .into_iter()
            .find(|d| nome_dispositivo(d).to_lowercase().contains(&procurado))
            .ok_or_else(|| format!("nenhum dispositivo com '{n}' (use latencia-listar)"));
    }
    let padrao = if entrada {
        host.default_input_device()
    } else {
        host.default_output_device()
    };
    padrao.ok_or_else(|| {
        format!(
            "sem dispositivo de {} padrão",
            if entrada { "entrada" } else { "saída" }
        )
    })
}

/// Soma das latências que o driver declara (via cpal), só para informação.
#[derive(Default)]
struct Declarada {
    soma_ns: AtomicU64,
    n: AtomicU64,
}

impl Declarada {
    fn somar(&self, d: Option<Duration>) {
        if let Some(d) = d {
            self.soma_ns
                .fetch_add(u64::try_from(d.as_nanos()).unwrap_or(0), Ordering::Relaxed);
            self.n.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn media_ms(&self) -> Option<f64> {
        let n = self.n.load(Ordering::Relaxed);
        (n > 0).then(|| self.soma_ns.load(Ordering::Relaxed) as f64 / n as f64 / 1e6)
    }
}

type BlocoGravado = (u64, u64, Vec<f32>);

struct FluxoAberto {
    _fluxo: cpal::Stream,
    taxa_hz: u32,
    nome: String,
    declarada: Arc<Declarada>,
}

fn config_do_fluxo(
    d: &cpal::Device,
    entrada: bool,
    buffer: u32,
) -> Result<(StreamConfig, SampleFormat), String> {
    let sup = if entrada {
        d.default_input_config()
    } else {
        d.default_output_config()
    }
    .map_err(|e| format!("configuração padrão indisponível: {e}"))?;
    let formato = sup.sample_format();
    let mut cfg = sup.config();
    cfg.buffer_size = BufferSize::Fixed(buffer);
    Ok((cfg, formato))
}

fn abrir_entrada(
    nome: Option<&str>,
    buffer: u32,
    relogio: RelogioMonotonico,
    tx: mpsc::Sender<BlocoGravado>,
) -> Result<FluxoAberto, String> {
    let dispositivo = achar_dispositivo(true, nome)?;
    let (cfg, formato) = config_do_fluxo(&dispositivo, true, buffer)?;
    let declarada = Arc::new(Declarada::default());
    let taxa_hz = cfg.sample_rate;
    let fluxo = match formato {
        SampleFormat::F32 => construir_entrada::<f32>(&dispositivo, cfg, relogio, tx, &declarada),
        SampleFormat::I16 => construir_entrada::<i16>(&dispositivo, cfg, relogio, tx, &declarada),
        SampleFormat::I32 => construir_entrada::<i32>(&dispositivo, cfg, relogio, tx, &declarada),
        SampleFormat::U16 => construir_entrada::<u16>(&dispositivo, cfg, relogio, tx, &declarada),
        outro => Err(format!("formato de amostra não suportado: {outro:?}")),
    }?;
    fluxo
        .play()
        .map_err(|e| format!("não foi possível iniciar a entrada: {e}"))?;
    Ok(FluxoAberto {
        _fluxo: fluxo,
        taxa_hz,
        nome: nome_dispositivo(&dispositivo),
        declarada,
    })
}

fn construir_entrada<T>(
    d: &cpal::Device,
    cfg: StreamConfig,
    relogio: RelogioMonotonico,
    tx: mpsc::Sender<BlocoGravado>,
    declarada: &Arc<Declarada>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let canais = usize::from(cfg.channels.max(1));
    let declarada = Arc::clone(declarada);
    let mut total = 0u64;
    d.build_input_stream(
        cfg,
        move |dados: &[T], info: &cpal::InputCallbackInfo| {
            // O instante é lido antes de qualquer alocação.
            let t = relogio.agora_ns();
            let ts = info.timestamp();
            declarada.somar(ts.callback.checked_duration_since(ts.capture));
            let mono: Vec<f32> = dados
                .chunks(canais)
                .map(|f| f.iter().map(|x| f32::from_sample(*x)).sum::<f32>() / canais as f32)
                .collect();
            total += mono.len() as u64;
            let _ = tx.send((t, total, mono));
        },
        |e| eprintln!("erro no fluxo de entrada: {e}"),
        Some(Duration::from_secs(3)),
    )
    .map_err(|e| format!("não foi possível abrir a entrada: {e}"))
}

/// Instantes (ns) em que cada clique foi entregue ao buffer de saída.
struct RegistroCliques {
    instantes: Vec<AtomicU64>,
    n: AtomicUsize,
}

impl RegistroCliques {
    fn novo() -> Self {
        Self {
            instantes: (0..8192).map(|_| AtomicU64::new(0)).collect(),
            n: AtomicUsize::new(0),
        }
    }

    /// Único produtor: o callback de saída.
    fn registrar(&self, t: u64) {
        let i = self.n.load(Ordering::Relaxed);
        if let Some(c) = self.instantes.get(i) {
            c.store(t, Ordering::Relaxed);
            self.n.store(i + 1, Ordering::Release);
        }
    }

    fn todos(&self) -> Vec<u64> {
        let n = self.n.load(Ordering::Acquire);
        self.instantes[..n]
            .iter()
            .map(|a| a.load(Ordering::Relaxed))
            .collect()
    }
}

enum Fonte {
    Cliques(Arc<RegistroCliques>, GeradorCliques),
    Tom(Arc<AtomicU32>, GeradorTom),
}

fn abrir_saida(
    nome: Option<&str>,
    buffer: u32,
    relogio: RelogioMonotonico,
    montar: impl FnOnce(u32) -> Fonte,
) -> Result<FluxoAberto, String> {
    let dispositivo = achar_dispositivo(false, nome)?;
    let (cfg, formato) = config_do_fluxo(&dispositivo, false, buffer)?;
    let declarada = Arc::new(Declarada::default());
    let taxa_hz = cfg.sample_rate;
    let fonte = montar(taxa_hz);
    let fluxo = match formato {
        SampleFormat::F32 => construir_saida::<f32>(&dispositivo, cfg, relogio, fonte, &declarada),
        SampleFormat::I16 => construir_saida::<i16>(&dispositivo, cfg, relogio, fonte, &declarada),
        SampleFormat::I32 => construir_saida::<i32>(&dispositivo, cfg, relogio, fonte, &declarada),
        SampleFormat::U16 => construir_saida::<u16>(&dispositivo, cfg, relogio, fonte, &declarada),
        outro => Err(format!("formato de amostra não suportado: {outro:?}")),
    }?;
    fluxo
        .play()
        .map_err(|e| format!("não foi possível iniciar a saída: {e}"))?;
    Ok(FluxoAberto {
        _fluxo: fluxo,
        taxa_hz,
        nome: nome_dispositivo(&dispositivo),
        declarada,
    })
}

fn construir_saida<T>(
    d: &cpal::Device,
    cfg: StreamConfig,
    relogio: RelogioMonotonico,
    mut fonte: Fonte,
    declarada: &Arc<Declarada>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let canais = usize::from(cfg.channels.max(1));
    let declarada = Arc::clone(declarada);
    let mut mono: Vec<f32> = Vec::new();
    d.build_output_stream(
        cfg,
        move |dados: &mut [T], info: &cpal::OutputCallbackInfo| {
            let t = relogio.agora_ns();
            let ts = info.timestamp();
            declarada.somar(ts.playback.checked_duration_since(ts.callback));
            mono.resize(dados.len() / canais, 0.0);
            match &mut fonte {
                Fonte::Cliques(reg, g) => g.preencher(&mut mono, t, &mut |i| reg.registrar(i)),
                Fonte::Tom(pedidos, g) => g.preencher(&mut mono, pedidos.load(Ordering::Acquire)),
            }
            for (frame, x) in dados.chunks_mut(canais).zip(&mono) {
                for s in frame {
                    *s = T::from_sample(*x);
                }
            }
        },
        |e| eprintln!("erro no fluxo de saída: {e}"),
        Some(Duration::from_secs(3)),
    )
    .map_err(|e| format!("não foi possível abrir a saída: {e}"))
}

// --- Análise comum --------------------------------------------------------------------------

struct Gravacao {
    amostras: Vec<f32>,
    linha: LinhaDoTempo,
}

fn juntar_gravacao(rx: &mpsc::Receiver<BlocoGravado>, taxa_hz: u32) -> Gravacao {
    let mut amostras = Vec::new();
    // Janela de 2 s para a origem local (absorve a deriva entre placa e relógio do sistema).
    let mut linha = LinhaDoTempo::nova(taxa_hz, 2.0);
    while let Ok((t, fim, mono)) = rx.try_recv() {
        linha.registrar_bloco(t, fim);
        amostras.extend(mono);
    }
    Gravacao { amostras, linha }
}

fn instantes_dos_onsets(g: &Gravacao, taxa_hz: u32, limiar: f32) -> Vec<u64> {
    let cfg = ConfigOnset {
        limiar_min: limiar,
        ..ConfigOnset::default()
    };
    detectar_onsets(&g.amostras, taxa_hz, &cfg)
        .iter()
        .filter_map(|i| g.linha.instante_ns(*i as u64))
        .collect()
}

fn esperar(segundos: u32, mut a_cada_segundo: impl FnMut(u32)) {
    let inicio = Instant::now();
    let total = Duration::from_secs(u64::from(segundos));
    let mut ultimo = 0;
    while inicio.elapsed() < total {
        std::thread::sleep(Duration::from_millis(100));
        let s = u32::try_from(inicio.elapsed().as_secs()).unwrap_or(u32::MAX);
        if s != ultimo {
            ultimo = s;
            a_cada_segundo(s);
        }
    }
}

// --- Loopback -------------------------------------------------------------------------------

fn loopback(op: &Opcoes) -> Result<String, String> {
    let relogio = RelogioMonotonico::novo();
    let (tx, rx) = mpsc::channel();
    let registro = Arc::new(RegistroCliques::novo());
    let entrada = abrir_entrada(op.dispositivo_entrada.as_deref(), op.buffer, relogio, tx)?;
    let reg = Arc::clone(&registro);
    let volume = op.volume;
    let saida = abrir_saida(
        op.dispositivo_saida.as_deref(),
        op.buffer,
        relogio,
        move |taxa| Fonte::Cliques(reg, GeradorCliques::novo(taxa, volume, 12_345, 1_000)),
    )?;
    eprintln!(
        "Gravando {} s: cliques pela saída, captura pela entrada. Não mexa nos cabos nem fale.",
        op.duracao_s
    );
    esperar(op.duracao_s, |s| eprintln!("  {s} s"));
    let (taxa_entrada, taxa_saida) = (entrada.taxa_hz, saida.taxa_hz);
    let (nome_e, nome_s) = (entrada.nome.clone(), saida.nome.clone());
    let (decl_e, decl_s) = (entrada.declarada.media_ms(), saida.declarada.media_ms());
    drop(saida);
    drop(entrada);

    let g = juntar_gravacao(&rx, taxa_entrada);
    let eventos = registro.todos();
    let onsets = instantes_dos_onsets(&g, taxa_entrada, op.limiar);
    let p = parear(&eventos, &onsets, 0, op.janela_max_ms * 1_000_000);
    let mut d = DadosMedicao {
        modo: Some(Modo::Loopback),
        rotulo: op.rotulo.clone(),
        limite_p95_ms: LIMITE_P95_AUDIO_MS,
        dispositivo_saida: Some(nome_s),
        dispositivo_entrada: Some(nome_e),
        taxa_saida_hz: Some(taxa_saida),
        taxa_entrada_hz: Some(taxa_entrada),
        buffer_frames: Some(op.buffer),
        duracao_s: f64::from(op.duracao_s),
        gerou_som: true,
        eventos: eventos.len(),
        onsets_detectados: onsets.len(),
        resumo: resumir(&p.atrasos_ms()),
        pareamento: p,
        lat_saida_declarada_ms: decl_s,
        lat_entrada_declarada_ms: decl_e,
        ..DadosMedicao::default()
    };
    if d.onsets_detectados == 0 {
        d.erros.push(
            "nenhum onset detectado: confira o cabo/microfone, o volume e --limiar".to_owned(),
        );
    }
    Ok(montar_relatorio_latencia(&d))
}

// --- MIDI → áudio ---------------------------------------------------------------------------

fn midi(op: &Opcoes) -> Result<String, String> {
    let relogio = RelogioMonotonico::novo();
    let entrada_midi =
        midir::MidiInput::new("piano-tutor-diag-latencia").map_err(|e| format!("MIDI: {e}"))?;
    let procurado = op.porta_midi.as_deref().map(str::to_lowercase);
    let porta = entrada_midi
        .ports()
        .into_iter()
        .find(|p| {
            procurado.as_ref().is_none_or(|n| {
                entrada_midi
                    .port_name(p)
                    .is_ok_and(|x| x.to_lowercase().contains(n))
            })
        })
        .ok_or("nenhuma porta MIDI de entrada encontrada (use latencia-listar)")?;
    let nome_porta = entrada_midi.port_name(&porta).unwrap_or_default();

    let eventos: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::new()));
    let pedidos = Arc::new(AtomicU32::new(0));
    let (tx, rx) = mpsc::channel();
    let entrada_audio = abrir_entrada(op.dispositivo_entrada.as_deref(), op.buffer, relogio, tx)?;
    let saida = if op.gerar_som {
        let ped = Arc::clone(&pedidos);
        let volume = op.volume;
        Some(abrir_saida(
            op.dispositivo_saida.as_deref(),
            op.buffer,
            relogio,
            move |taxa| Fonte::Tom(ped, GeradorTom::novo(taxa, volume)),
        )?)
    } else {
        None
    };
    // A conexão MIDI é a última a abrir: só depois disso as notas contam.
    let ev = Arc::clone(&eventos);
    let ped = Arc::clone(&pedidos);
    let _conexao = entrada_midi
        .connect(
            &porta,
            "piano-tutor-diag-latencia-entrada",
            move |_stamp, msg, ()| {
                if eh_nota_ligada(msg) {
                    let t = relogio.agora_ns();
                    ev.lock().unwrap_or_else(PoisonError::into_inner).push(t);
                    ped.fetch_add(1, Ordering::Release);
                }
            },
            (),
        )
        .map_err(|e| format!("não foi possível abrir a porta MIDI: {e}"))?;

    eprintln!(
        "Escutando {} s. Toque UMA nota por vez, com pelo menos 1 s entre elas (30 notas ou mais).",
        op.duracao_s
    );
    if !op.gerar_som {
        eprintln!("O som deve vir do app (ou do programa em teste), captado pelo microfone.");
    }
    let mut vistos = 0usize;
    esperar(op.duracao_s, |s| {
        let n = eventos.lock().unwrap_or_else(PoisonError::into_inner).len();
        if n != vistos {
            vistos = n;
            eprintln!("  {s} s: {n} notas");
        }
    });
    let taxa_entrada = entrada_audio.taxa_hz;
    let nome_e = entrada_audio.nome.clone();
    let decl_e = entrada_audio.declarada.media_ms();
    let (nome_s, taxa_s, decl_s) = saida.as_ref().map_or((None, None, None), |s| {
        (
            Some(s.nome.clone()),
            Some(s.taxa_hz),
            s.declarada.media_ms(),
        )
    });
    drop(_conexao);
    drop(saida);
    drop(entrada_audio);

    let g = juntar_gravacao(&rx, taxa_entrada);
    let todos = eventos
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    // Isolamento = janela de pareamento: um segundo NoteOn dentro da janela contaminaria o par.
    let (isolados, descartados) = filtrar_isolados(&todos, op.janela_max_ms * 1_000_000);
    let onsets = instantes_dos_onsets(&g, taxa_entrada, op.limiar);
    let p = parear(&isolados, &onsets, 0, op.janela_max_ms * 1_000_000);
    let mut d = DadosMedicao {
        modo: Some(Modo::MidiAudio),
        rotulo: op.rotulo.clone(),
        limite_p95_ms: LIMITE_P95_AUDIO_MS,
        dispositivo_saida: nome_s,
        dispositivo_entrada: Some(nome_e),
        porta_midi: Some(nome_porta),
        taxa_saida_hz: taxa_s,
        taxa_entrada_hz: Some(taxa_entrada),
        buffer_frames: Some(op.buffer),
        duracao_s: f64::from(op.duracao_s),
        gerou_som: op.gerar_som,
        eventos: todos.len(),
        descartados_por_proximidade: descartados,
        onsets_detectados: onsets.len(),
        resumo: resumir(&p.atrasos_ms()),
        pareamento: p,
        lat_saida_declarada_ms: decl_s,
        lat_entrada_declarada_ms: decl_e,
        ..DadosMedicao::default()
    };
    if todos.is_empty() {
        d.erros
            .push("nenhum NoteOn recebido: confira a porta MIDI".to_owned());
    }
    if d.onsets_detectados == 0 {
        d.erros.push(
            "nenhum onset detectado: confira o microfone, o volume do som e --limiar".to_owned(),
        );
    }
    Ok(montar_relatorio_latencia(&d))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn comando_que_nao_e_de_latencia_nao_e_consumido() {
        assert_eq!(ler_comando(&args("--saida x.md")), Ok(None));
        assert_eq!(ler_comando(&[]), Ok(None));
    }

    #[test]
    fn padroes_e_opcoes_do_loopback() {
        let (sub, op) = ler_comando(&args("latencia-loopback"))
            .expect("ok")
            .expect("é latência");
        assert_eq!(sub, Subcomando::Loopback);
        assert_eq!(op, Opcoes::default());
        let (_, op) = ler_comando(&args(
            "latencia-loopback --duracao 20 --buffer 128 --saida-dispositivo HDA --volume 3 \
             --rotulo cerebro --saida a.md --limiar 0,02",
        ))
        .expect("ok")
        .expect("é latência");
        assert_eq!((op.duracao_s, op.buffer), (20, 128));
        assert_eq!(op.dispositivo_saida.as_deref(), Some("HDA"));
        assert_eq!(op.volume, 1.0, "volume limitado a 1");
        assert_eq!(op.limiar, 0.02);
        assert_eq!(op.saida_md.as_deref(), Some("a.md"));
    }

    #[test]
    fn midi_e_quadros_aceitam_suas_opcoes() {
        let (sub, op) = ler_comando(&args(
            "latencia-midi --porta-midi KB-330 --gerar-som --janela-max-ms 300",
        ))
        .expect("ok")
        .expect("é latência");
        assert_eq!(sub, Subcomando::Midi);
        assert!(op.gerar_som);
        assert_eq!(
            (op.porta_midi.as_deref(), op.janela_max_ms),
            (Some("KB-330"), 300)
        );
        let (sub, op) = ler_comando(&args("latencia-quadros contagens.txt --fps 120"))
            .expect("ok")
            .expect("é latência");
        assert_eq!(sub, Subcomando::Quadros);
        assert_eq!(
            (op.arquivo.as_deref(), op.fps),
            (Some("contagens.txt"), 120.0)
        );
    }

    #[test]
    fn opcoes_invalidas_viram_erro() {
        for a in [
            "latencia-midi --duracao",
            "latencia-midi --duracao zero",
            "latencia-midi --duracao -3",
            "latencia-midi --buffer 0",
            "latencia-midi --nao-existe",
            "latencia-quadros a.txt b.txt",
        ] {
            assert!(ler_comando(&args(a)).is_err(), "{a}");
        }
        assert!(
            ler_comando(&args("latencia-midi --ajuda"))
                .expect_err("ajuda")
                .contains("latencia-loopback")
        );
    }

    #[test]
    fn quadros_de_um_arquivo_geram_relatorio_de_tela() {
        let pasta = std::env::temp_dir().join(format!("pt-diag-quadros-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&pasta);
        let arq = pasta.join("contagens.txt");
        let linhas: String = (0..40).map(|i| format!("{}\n", 8 + i % 4)).collect();
        std::fs::write(&arq, linhas).expect("grava");
        let op = Opcoes {
            arquivo: Some(arq.to_string_lossy().into_owned()),
            rotulo: Some("teste sintético".into()),
            ..Opcoes::default()
        };
        let md = executar(Subcomando::Quadros, &op).expect("relatório");
        assert!(md.contains("F02-4") && md.contains("40"), "{md}");
        assert!(md.contains("quadros/s"));
        let _ = std::fs::remove_dir_all(pasta);
        // Sem arquivo ou com arquivo inexistente: erro, não relatório.
        assert!(executar(Subcomando::Quadros, &Opcoes::default()).is_err());
        let ruim = Opcoes {
            arquivo: Some("/nao/existe.txt".into()),
            ..Opcoes::default()
        };
        assert!(executar(Subcomando::Quadros, &ruim).is_err());
    }

    #[test]
    fn registro_de_cliques_guarda_em_ordem_e_ignora_excesso() {
        let r = RegistroCliques::novo();
        for t in 1..=9_000u64 {
            r.registrar(t);
        }
        let v = r.todos();
        assert_eq!(v.len(), 8192);
        assert_eq!((v[0], v[8191]), (1, 8192));
    }
}
