//! Medição de latência (F03-4: MIDI→áudio, p95 ≤ 50 ms; F02-4: MIDI→tela, p95 ≤ 60 ms).
//!
//! Este módulo é **lógica pura**, testada com sinais sintéticos e sem hardware: estatística,
//! detecção de onset por energia, linha do tempo da gravação, pareamento de eventos com onsets,
//! geradores de sinal e o relatório Markdown. O que toca dispositivos (cpal, midir) fica em
//! [`crate::medicao`].
//!
//! Nada aqui prova latência real: os testes só provam que a **conta** está certa. Um número
//! só vale como medição quando o relatório gerado por uma execução de verdade o traz, com o
//! ambiente rotulado.

use piano_tutor_core::sanitizar::sanitizar;
use std::fmt::Write as _;

/// Mínimo de pares evento→onset para o relatório emitir veredito (abaixo disso: inconclusivo).
pub const AMOSTRAS_MINIMAS: usize = 30;
/// Meta F03-4 (MIDI→áudio) e F02-4 (MIDI→tela), em ms, para o p95.
pub const LIMITE_P95_AUDIO_MS: f64 = 50.0;
pub const LIMITE_P95_TELA_MS: f64 = 60.0;

// --- Estatística ----------------------------------------------------------------------------

/// Resumo de uma série de atrasos, em ms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resumo {
    pub n: usize,
    pub media: f64,
    /// Desvio padrão (população).
    pub desvio: f64,
    pub min: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
    /// p95 de `|x − média|`: variação sem o viés constante (mesma definição do `jitter.rs`).
    pub jitter_p95: f64,
}

/// Percentil por posto mais próximo (`ceil(p·n)`), `ordenado` crescente e não vazio.
pub fn percentil(ordenado: &[f64], p: f64) -> f64 {
    let n = ordenado.len();
    if n == 0 {
        return f64::NAN;
    }
    let posto = ((p * n as f64).ceil() as usize).clamp(1, n);
    ordenado[posto - 1]
}

/// Resume `valores` (não finitos são ignorados). `None` se não sobrar nenhum.
pub fn resumir(valores: &[f64]) -> Option<Resumo> {
    let mut v: Vec<f64> = valores.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    let media = v.iter().sum::<f64>() / n as f64;
    let var = v.iter().map(|x| (x - media).powi(2)).sum::<f64>() / n as f64;
    let mut desvios: Vec<f64> = v.iter().map(|x| (x - media).abs()).collect();
    desvios.sort_by(f64::total_cmp);
    Some(Resumo {
        n,
        media,
        desvio: var.sqrt(),
        min: v[0],
        p50: percentil(&v, 0.50),
        p95: percentil(&v, 0.95),
        p99: percentil(&v, 0.99),
        max: v[n - 1],
        jitter_p95: percentil(&desvios, 0.95),
    })
}

/// Resultado frente à meta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Veredito {
    /// p95 ≤ meta, com amostras suficientes.
    Dentro,
    /// p95 > meta, com amostras suficientes.
    Acima,
    /// Poucas amostras (ou nenhuma): **NÃO VERIFICADO**.
    Inconclusivo,
}

pub fn veredito(r: Option<&Resumo>, limite_p95_ms: f64, n_min: usize) -> Veredito {
    match r {
        Some(r) if r.n >= n_min => {
            if r.p95 <= limite_p95_ms {
                Veredito::Dentro
            } else {
                Veredito::Acima
            }
        }
        _ => Veredito::Inconclusivo,
    }
}

// --- Detecção de onset ----------------------------------------------------------------------

/// Parâmetros do detector de onset por energia.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConfigOnset {
    /// Janela de energia (RMS), em ms.
    pub janela_ms: f64,
    /// Limiar = `fator × mediana(RMS das janelas)`, no mínimo `limiar_min`.
    pub fator: f32,
    /// Limiar mínimo absoluto (RMS, escala de −1 a 1).
    pub limiar_min: f32,
    /// Depois de um onset, ignora este tempo (ressonância, eco, cauda do som), em ms.
    pub refratario_ms: f64,
}

impl Default for ConfigOnset {
    fn default() -> Self {
        Self {
            janela_ms: 1.0,
            fator: 8.0,
            limiar_min: 0.01,
            refratario_ms: 120.0,
        }
    }
}

/// Índices (em amostras) dos onsets de `amostras` (mono, −1 a 1).
///
/// O onset é a **primeira amostra** da primeira janela acima do limiar cujo módulo passa de
/// metade do limiar. Sons de ataque lento são detectados um pouco depois do início perceptível:
/// o viés depende do limiar e é sempre para **mais** atraso (conservador).
pub fn detectar_onsets(amostras: &[f32], taxa_hz: u32, cfg: &ConfigOnset) -> Vec<usize> {
    let w = ((f64::from(taxa_hz) * cfg.janela_ms / 1000.0).round() as usize).max(1);
    if amostras.len() < w {
        return Vec::new();
    }
    let rms: Vec<f32> = amostras
        .chunks_exact(w)
        .map(|c| (c.iter().map(|x| x * x).sum::<f32>() / w as f32).sqrt())
        .collect();
    let mut ordenado = rms.clone();
    ordenado.sort_by(f32::total_cmp);
    let piso = ordenado[ordenado.len() / 2];
    let limiar = (cfg.fator * piso).max(cfg.limiar_min);
    let refratario = (f64::from(taxa_hz) * cfg.refratario_ms / 1000.0).round() as usize;

    let mut onsets = Vec::new();
    let mut i = 0;
    while i < rms.len() {
        if rms[i] >= limiar {
            let ini = i * w;
            let fim = ini + w;
            let amostra = amostras[ini..fim]
                .iter()
                .position(|x| x.abs() >= limiar * 0.5)
                .map_or(ini, |p| ini + p);
            onsets.push(amostra);
            i = (amostra + refratario.max(w)) / w + 1;
        } else {
            i += 1;
        }
    }
    onsets
}

// --- Linha do tempo da gravação -------------------------------------------------------------

/// Liga o índice de amostra da gravação ao relógio monotônico do processo.
///
/// Cada bloco entregue pelo callback informa `t_callback` (ns, relógio monotônico, lido no
/// início do callback) e quantos frames já foram entregues **até o fim do bloco**. Supondo que o
/// fim do bloco acabou de ser capturado, a amostra `j` está em `t_callback − (fim − j)/taxa`.
/// O atraso de despertar do callback só adianta o `t_callback`, nunca o atrasa, então o melhor
/// estimador da origem é o **menor** deslocamento numa janela local (a janela também absorve a
/// deriva entre o cristal da placa e o relógio do sistema, da ordem de dezenas de ppm).
///
/// Não inclui a latência de captura do conversor e do driver (desconhecida): o instante obtido
/// pode ser **anterior** ao verdadeiro por essa latência. O relatório declara isso.
#[derive(Clone, Debug)]
pub struct LinhaDoTempo {
    taxa_hz: u32,
    janela_frames: u64,
    /// `(frames até o fim do bloco, t_callback − fim·1e9/taxa)`.
    marcas: Vec<(u64, i128)>,
}

impl LinhaDoTempo {
    pub fn nova(taxa_hz: u32, janela_s: f64) -> Self {
        Self {
            taxa_hz: taxa_hz.max(1),
            janela_frames: (f64::from(taxa_hz) * janela_s) as u64,
            marcas: Vec::new(),
        }
    }

    fn ns_do_frame(&self, frame: u64) -> i128 {
        i128::from(frame) * 1_000_000_000 / i128::from(self.taxa_hz)
    }

    pub fn registrar_bloco(&mut self, t_callback_ns: u64, frames_ate_o_fim: u64) {
        let desloc = i128::from(t_callback_ns) - self.ns_do_frame(frames_ate_o_fim);
        self.marcas.push((frames_ate_o_fim, desloc));
    }

    /// Instante (ns) da amostra `frame`; `None` sem nenhum bloco registrado perto.
    pub fn instante_ns(&self, frame: u64) -> Option<u64> {
        let desloc = self
            .marcas
            .iter()
            .filter(|(fim, _)| fim.abs_diff(frame) <= self.janela_frames)
            .map(|(_, d)| *d)
            .min()?;
        u64::try_from(desloc + self.ns_do_frame(frame)).ok()
    }
}

// --- Pareamento -----------------------------------------------------------------------------

/// Um evento pareado com seu onset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Par {
    pub evento_ns: u64,
    pub onset_ns: u64,
}

impl Par {
    pub fn atraso_ms(&self) -> f64 {
        (self.onset_ns as f64 - self.evento_ns as f64) / 1e6
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Pareamento {
    pub pares: Vec<Par>,
    /// Eventos para os quais nenhum onset caiu na janela (nota perdida ou som inaudível).
    pub eventos_sem_onset: usize,
    /// Onsets sem evento correspondente (ruído, eco, som do próprio teclado).
    pub onsets_orfaos: usize,
}

impl Pareamento {
    pub fn atrasos_ms(&self) -> Vec<f64> {
        self.pares.iter().map(Par::atraso_ms).collect()
    }
}

/// Pareia cada onset com o evento **mais recente** que o precede de `min_ns` a `max_ns`;
/// um evento recebe no máximo um onset (o primeiro). `eventos` e `onsets` em ordem crescente.
pub fn parear(eventos: &[u64], onsets: &[u64], min_ns: u64, max_ns: u64) -> Pareamento {
    let mut usado = vec![false; eventos.len()];
    let mut r = Pareamento::default();
    for &o in onsets {
        let ate = o.saturating_sub(min_ns);
        let depois = eventos.partition_point(|e| *e <= ate);
        let candidato = depois.checked_sub(1);
        match candidato {
            Some(i) if !usado[i] && o - eventos[i] <= max_ns && o >= eventos[i] + min_ns => {
                usado[i] = true;
                r.pares.push(Par {
                    evento_ns: eventos[i],
                    onset_ns: o,
                });
            }
            _ => r.onsets_orfaos += 1,
        }
    }
    r.eventos_sem_onset = usado.iter().filter(|u| !**u).count();
    r
}

/// Descarta eventos que têm outro a menos de `isolamento_ns` (acordes, repique): o onset de um
/// seria atribuído ao outro. Devolve os isolados e quantos foram descartados.
pub fn filtrar_isolados(eventos: &[u64], isolamento_ns: u64) -> (Vec<u64>, usize) {
    let manter: Vec<u64> = eventos
        .iter()
        .enumerate()
        .filter(|(i, e)| {
            let antes = i.checked_sub(1).map(|k| **e - eventos[k]);
            let depois = eventos.get(i + 1).map(|p| *p - **e);
            antes.is_none_or(|d| d >= isolamento_ns) && depois.is_none_or(|d| d >= isolamento_ns)
        })
        .map(|(_, e)| *e)
        .collect();
    let descartados = eventos.len() - manter.len();
    (manter, descartados)
}

/// `true` para NoteOn com velocidade maior que zero (qualquer canal).
pub fn eh_nota_ligada(mensagem: &[u8]) -> bool {
    matches!(mensagem, [s, _, v, ..] if (0x90..=0x9F).contains(s) && *v > 0)
}

// --- Geradores de sinal ---------------------------------------------------------------------

/// Gerador pseudoaleatório mínimo e determinístico (LCG de 64 bits).
pub fn proximo_aleatorio(estado: &mut u64) -> u64 {
    *estado = estado
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *estado >> 33
}

/// Intervalo entre cliques (350 a 650 ms): irregular, para o pareamento não acertar por acaso.
pub fn proximo_intervalo_ms(estado: &mut u64) -> u64 {
    350 + proximo_aleatorio(estado) % 301
}

/// Gera cliques em instantes conhecidos e registra, para cada um, o instante em que o software
/// o entregou ao buffer (`t_callback + posição no bloco`).
#[derive(Clone, Debug)]
pub struct GeradorCliques {
    taxa_hz: u32,
    amplitude: f32,
    frame: u64,
    proximo: u64,
    pos: Option<usize>,
    rng: u64,
}

/// Duração do clique, em ms (seno de 1,8 kHz com decaimento rápido, ataque abrupto).
const CLIQUE_MS: f64 = 12.0;

impl GeradorCliques {
    pub fn novo(taxa_hz: u32, amplitude: f32, semente: u64, atraso_inicial_ms: u64) -> Self {
        Self {
            taxa_hz,
            amplitude,
            frame: 0,
            proximo: atraso_inicial_ms * u64::from(taxa_hz) / 1000,
            pos: None,
            rng: semente,
        }
    }

    fn amostra(&self, n: usize) -> f32 {
        let t = n as f64 / f64::from(self.taxa_hz);
        let envelope = (-t / 0.0015).exp();
        self.amplitude * ((2.0 * std::f64::consts::PI * 1800.0 * t).sin() * envelope) as f32
    }

    /// Preenche `mono` (um bloco do callback que começou em `t_callback_ns`). `registrar`
    /// recebe o instante de cada início de clique.
    pub fn preencher(
        &mut self,
        mono: &mut [f32],
        t_callback_ns: u64,
        registrar: &mut dyn FnMut(u64),
    ) {
        let duracao = (f64::from(self.taxa_hz) * CLIQUE_MS / 1000.0) as usize;
        for (i, saida) in mono.iter_mut().enumerate() {
            if self.frame == self.proximo {
                self.pos = Some(0);
                let ns = (i as u64) * 1_000_000_000 / u64::from(self.taxa_hz);
                registrar(t_callback_ns + ns);
                let intervalo = proximo_intervalo_ms(&mut self.rng);
                self.proximo += intervalo * u64::from(self.taxa_hz) / 1000;
            }
            *saida = match self.pos {
                Some(n) if n < duracao => {
                    self.pos = Some(n + 1);
                    self.amostra(n)
                }
                _ => {
                    self.pos = None;
                    0.0
                }
            };
            self.frame += 1;
        }
    }
}

/// Toca um tom curto a cada NoteOn recebido (modo `--gerar-som`): mede o piso do caminho
/// "callback MIDI → callback de áudio → captura", sem o motor do app.
#[derive(Clone, Debug)]
pub struct GeradorTom {
    taxa_hz: u32,
    amplitude: f32,
    pos: Option<usize>,
    atendidos: u32,
}

impl GeradorTom {
    pub fn novo(taxa_hz: u32, amplitude: f32) -> Self {
        Self {
            taxa_hz,
            amplitude,
            pos: None,
            atendidos: 0,
        }
    }

    /// `pedidos_totais` é o contador de NoteOn já recebidos; quando passa do já atendido, o tom
    /// começa no **primeiro frame deste bloco**.
    pub fn preencher(&mut self, mono: &mut [f32], pedidos_totais: u32) {
        if pedidos_totais != self.atendidos {
            self.atendidos = pedidos_totais;
            self.pos = Some(0);
        }
        let duracao = (f64::from(self.taxa_hz) * 0.25) as usize;
        for saida in mono.iter_mut() {
            *saida = match self.pos {
                Some(n) if n < duracao => {
                    self.pos = Some(n + 1);
                    let t = n as f64 / f64::from(self.taxa_hz);
                    let env = (-t / 0.08).exp();
                    self.amplitude * ((2.0 * std::f64::consts::PI * 880.0 * t).sin() * env) as f32
                }
                _ => {
                    self.pos = None;
                    0.0
                }
            };
        }
    }
}

// --- F02-4: contagem de quadros de vídeo ----------------------------------------------------

/// Lê um número por linha (vírgula decimal aceita); linhas vazias e `#` são ignoradas.
pub fn ler_numeros(texto: &str) -> Result<Vec<f64>, String> {
    let mut v = Vec::new();
    for (n, linha) in texto.lines().enumerate() {
        let l = linha.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let x: f64 = l
            .replace(',', ".")
            .parse()
            .map_err(|_| format!("linha {}: '{l}' não é número", n + 1))?;
        if !x.is_finite() || x < 0.0 {
            return Err(format!("linha {}: valor inválido '{l}'", n + 1));
        }
        v.push(x);
    }
    Ok(v)
}

/// Converte contagens de quadros de vídeo em ms.
pub fn quadros_para_ms(quadros: &[f64], fps: f64) -> Vec<f64> {
    quadros.iter().map(|q| q * 1000.0 / fps).collect()
}

// --- Relatório ------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Modo {
    /// Cliques gerados pelo programa, saída → entrada (F03-4, componente de áudio).
    Loopback,
    /// NoteOn da porta MIDI → onset gravado (F03-4).
    MidiAudio,
    /// Contagem de quadros de vídeo (F02-4).
    Quadros,
}

/// Dados de uma execução (tudo opcional que depende do modo).
#[derive(Clone, Debug, Default)]
pub struct DadosMedicao {
    pub modo: Option<Modo>,
    pub rotulo: Option<String>,
    pub limite_p95_ms: f64,
    pub dispositivo_saida: Option<String>,
    pub dispositivo_entrada: Option<String>,
    pub porta_midi: Option<String>,
    pub taxa_saida_hz: Option<u32>,
    pub taxa_entrada_hz: Option<u32>,
    pub buffer_frames: Option<u32>,
    pub duracao_s: f64,
    /// `true` quando o programa tocou o som (cliques ou `--gerar-som`).
    pub gerou_som: bool,
    pub fps: Option<f64>,
    pub eventos: usize,
    pub descartados_por_proximidade: usize,
    pub onsets_detectados: usize,
    pub pareamento: Pareamento,
    pub resumo: Option<Resumo>,
    /// Latências que o driver diz ter (via cpal); informativas, não medidas.
    pub lat_saida_declarada_ms: Option<f64>,
    pub lat_entrada_declarada_ms: Option<f64>,
    pub erros: Vec<String>,
}

fn opcional(v: &Option<String>) -> String {
    v.as_deref()
        .map_or_else(|| "não usado".to_owned(), sanitizar)
}

fn ms(x: f64) -> String {
    format!("{x:.2} ms")
}

/// Markdown do resultado. Texto sanitizado (nomes de dispositivos, rótulo e erros).
pub fn montar_relatorio_latencia(d: &DadosMedicao) -> String {
    let (titulo, meta_nome) = match d.modo {
        Some(Modo::Loopback) => ("Latência de áudio por loopback (saída → entrada)", "F03-4"),
        Some(Modo::MidiAudio) => ("Latência MIDI → áudio por captura", "F03-4"),
        Some(Modo::Quadros) => (
            "Latência MIDI → tela por contagem de quadros de vídeo",
            "F02-4",
        ),
        None => ("Medição de latência", "—"),
    };
    let v = veredito(d.resumo.as_ref(), d.limite_p95_ms, AMOSTRAS_MINIMAS);
    let mut md = format!("# {titulo}\n\n");
    if let Some(r) = &d.rotulo {
        let _ = writeln!(md, "> Ambiente: {}\n", sanitizar(r));
    }
    let estado = match v {
        Veredito::Dentro => format!(
            "p95 = {} ≤ meta de {} ({meta_nome}), com {} amostras. Vale **só** para o ambiente \
             rotulado acima e para este método (ver limitações).",
            ms(d.resumo.map_or(f64::NAN, |r| r.p95)),
            ms(d.limite_p95_ms),
            d.resumo.map_or(0, |r| r.n)
        ),
        Veredito::Acima => format!(
            "p95 = {} **acima** da meta de {} ({meta_nome}), com {} amostras. Documentar a \
             limitação e abrir correção, conforme a ficha.",
            ms(d.resumo.map_or(f64::NAN, |r| r.p95)),
            ms(d.limite_p95_ms),
            d.resumo.map_or(0, |r| r.n)
        ),
        Veredito::Inconclusivo => format!(
            "**NÃO VERIFICADO**: {} amostras pareadas (mínimo {AMOSTRAS_MINIMAS}). Sem veredito.",
            d.resumo.map_or(0, |r| r.n)
        ),
    };
    let _ = writeln!(md, "**Resultado:** {estado}\n");

    md.push_str("## Configuração\n\n");
    if d.modo != Some(Modo::Quadros) {
        let _ = writeln!(md, "- Saída de áudio: {}", opcional(&d.dispositivo_saida));
        let _ = writeln!(
            md,
            "- Entrada de áudio: {}",
            opcional(&d.dispositivo_entrada)
        );
        if d.modo == Some(Modo::MidiAudio) {
            let _ = writeln!(md, "- Porta MIDI: {}", opcional(&d.porta_midi));
        }
        let taxa = |t: Option<u32>| t.map_or_else(|| "n/d".to_owned(), |x| format!("{x} Hz"));
        let _ = writeln!(
            md,
            "- Taxa: saída {}, entrada {}",
            taxa(d.taxa_saida_hz),
            taxa(d.taxa_entrada_hz)
        );
        let _ = writeln!(
            md,
            "- Buffer pedido: {}",
            d.buffer_frames
                .map_or_else(|| "padrão do host".to_owned(), |b| format!("{b} frames"))
        );
        let _ = writeln!(
            md,
            "- Som tocado por este programa: {}",
            if d.gerou_som {
                "sim"
            } else {
                "não (mede o som de outro programa, ex.: o app)"
            }
        );
        let _ = writeln!(md, "- Duração da gravação: {:.0} s", d.duracao_s);
    } else if let Some(f) = d.fps {
        let _ = writeln!(
            md,
            "- Câmera: {f:.0} quadros/s (resolução de leitura: ±1 quadro = ±{:.2} ms)",
            1000.0 / f
        );
    }
    md.push('\n');

    md.push_str("## Contagens\n\n");
    let _ = writeln!(md, "- Eventos de referência: {}", d.eventos);
    if d.descartados_por_proximidade > 0 {
        let _ = writeln!(
            md,
            "- Descartados por proximidade (acordes ou repique): {}",
            d.descartados_por_proximidade
        );
    }
    if d.modo != Some(Modo::Quadros) {
        let _ = writeln!(
            md,
            "- Onsets detectados na gravação: {}",
            d.onsets_detectados
        );
        let _ = writeln!(md, "- Pares evento → onset: {}", d.pareamento.pares.len());
        let _ = writeln!(
            md,
            "- Eventos sem onset: {}",
            d.pareamento.eventos_sem_onset
        );
        let _ = writeln!(md, "- Onsets sem evento: {}", d.pareamento.onsets_orfaos);
    }
    md.push('\n');

    md.push_str("## Atraso\n\n");
    if let Some(r) = &d.resumo {
        md.push_str("| n | média | desvio | mín. | p50 | p95 | p99 | máx. | jitter p95 |\n");
        md.push_str("|---|---|---|---|---|---|---|---|---|\n");
        let _ = writeln!(
            md,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r.n,
            ms(r.media),
            ms(r.desvio),
            ms(r.min),
            ms(r.p50),
            ms(r.p95),
            ms(r.p99),
            ms(r.max),
            ms(r.jitter_p95)
        );
        md.push_str(
            "Jitter p95 = percentil 95 de |atraso − média| (variação sem o viés constante). \
             Percentis por posto mais próximo.\n\n",
        );
    } else {
        md.push_str("- Sem pares: nada a resumir. **NÃO VERIFICADO**.\n\n");
    }
    if d.lat_saida_declarada_ms.is_some() || d.lat_entrada_declarada_ms.is_some() {
        md.push_str("## Latência declarada pelo driver (informativa, não medida)\n\n");
        if let Some(x) = d.lat_saida_declarada_ms {
            let _ = writeln!(md, "- Saída (reprodução − callback): {}", ms(x));
        }
        if let Some(x) = d.lat_entrada_declarada_ms {
            let _ = writeln!(md, "- Entrada (callback − captura): {}", ms(x));
        }
        md.push('\n');
    }

    md.push_str("## Limitações\n\n");
    match d.modo {
        Some(Modo::Quadros) => {
            md.push_str(
                "- Contagem manual de quadros por pessoa: erro de ±1 quadro em cada marca \
                 (início e fim), ou seja, até ±2 quadros por medida.\n\
                 - Mede o que a pessoa vê no vídeo: inclui o atraso do monitor e a taxa de \
                 atualização da tela (ex.: 60 Hz = 16,7 ms por quadro de tela).\n",
            );
        }
        _ => {
            md.push_str(
                "- Instantes de software (relógio monotônico do processo): o evento MIDI é \
                 carimbado quando o programa o recebe, **sem** o trajeto USB do teclado \
                 (da ordem de 1 ms) e sem o atraso interno do teclado.\n\
                 - O onset é a primeira amostra acima de um limiar de energia: sons de ataque \
                 lento são detectados depois do início perceptível (viés para mais atraso).\n\
                 - O tempo da gravação assume que o fim de cada bloco acabou de ser capturado; \
                 a latência de captura do conversor e do driver **não** está incluída (viés \
                 para menos atraso).\n\
                 - Com microfone, o som viaja pelo ar (cerca de 3 ms por metro) e a sala soma \
                 eco: manter o microfone perto do alto-falante.\n\
                 - Se o teclado tem som próprio audível, ele também gera onsets (som sem \
                 atraso do PC); usar fone no teclado, volume local zero ou Local Control \
                 desligado.\n\
                 - Não substitui medição com osciloscópio ou captura de duas pontas de hardware.\n",
            );
        }
    }
    md.push_str("- Dispositivos e rótulo são sanitizados; não são coletados hostname, usuário, IP nem seriais.\n");
    for e in &d.erros {
        let _ = writeln!(md, "- Aviso: {}", sanitizar(e));
    }
    md
}

#[cfg(test)]
mod testes {
    use super::*;

    const TAXA: u32 = 48_000;

    /// Ruído branco determinístico em `[-amp, amp]`.
    fn ruido(n: usize, amp: f32, semente: u64) -> Vec<f32> {
        let mut s = semente;
        (0..n)
            .map(|_| ((proximo_aleatorio(&mut s) % 20_001) as f32 / 10_000.0 - 1.0) * amp)
            .collect()
    }

    /// Soma um clique (seno de 1 kHz, ataque de `ataque_ms`, decaimento exponencial) em `pos`.
    fn somar_clique(sinal: &mut [f32], pos: usize, amp: f32, ataque_ms: f64) {
        let ataque = (f64::from(TAXA) * ataque_ms / 1000.0).max(1.0);
        for n in 0..(TAXA as usize / 10) {
            let Some(x) = sinal.get_mut(pos + n) else {
                break;
            };
            let t = n as f64 / f64::from(TAXA);
            let rampa = (n as f64 / ataque).min(1.0);
            let env = (-t / 0.02).exp() * rampa;
            *x += amp * ((2.0 * std::f64::consts::PI * 1000.0 * t).sin() * env) as f32;
        }
    }

    // --- estatística ---

    #[test]
    fn percentis_por_posto_mais_proximo() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        let r = resumir(&v).expect("resumo");
        assert_eq!(r.n, 100);
        assert_eq!(
            (r.min, r.p50, r.p95, r.p99, r.max),
            (1.0, 50.0, 95.0, 99.0, 100.0)
        );
        assert!((r.media - 50.5).abs() < 1e-9);
        assert!((r.desvio - 28.866_070).abs() < 1e-4);
        // |x − 50,5|: 0,5 a 49,5 (cada um duas vezes): p95 = 47,5 a 48,5.
        assert!((47.0..=49.0).contains(&r.jitter_p95), "{}", r.jitter_p95);
    }

    #[test]
    fn resumo_ignora_nao_finitos_e_vazio_e_none() {
        assert_eq!(resumir(&[]), None);
        assert_eq!(resumir(&[f64::NAN, f64::INFINITY]), None);
        let r = resumir(&[f64::NAN, 7.0]).expect("um valor");
        assert_eq!((r.n, r.p95, r.jitter_p95, r.desvio), (1, 7.0, 0.0, 0.0));
    }

    #[test]
    fn veredito_exige_amostras_minimas() {
        let boa = resumir(&vec![20.0; 40]).expect("resumo");
        assert_eq!(veredito(Some(&boa), 50.0, 30), Veredito::Dentro);
        let ruim = resumir(&vec![80.0; 40]).expect("resumo");
        assert_eq!(veredito(Some(&ruim), 50.0, 30), Veredito::Acima);
        let pouca = resumir(&[1.0; 10]).expect("resumo");
        assert_eq!(veredito(Some(&pouca), 50.0, 30), Veredito::Inconclusivo);
        assert_eq!(veredito(None, 50.0, 30), Veredito::Inconclusivo);
        // Na fronteira: p95 igual à meta ainda é "dentro".
        let borda = resumir(&vec![50.0; 30]).expect("resumo");
        assert_eq!(veredito(Some(&borda), 50.0, 30), Veredito::Dentro);
    }

    // --- onset ---

    #[test]
    fn detecta_cliques_sinteticos_com_erro_menor_que_uma_janela() {
        let mut sinal = ruido(TAXA as usize * 3, 0.002, 1);
        let posicoes = [4_800usize, 30_000, 61_234, 100_000, 130_000];
        for p in posicoes {
            somar_clique(&mut sinal, p, 0.5, 0.0);
        }
        let achados = detectar_onsets(&sinal, TAXA, &ConfigOnset::default());
        assert_eq!(achados.len(), posicoes.len(), "{achados:?}");
        for (a, p) in achados.iter().zip(posicoes) {
            // Ataque abrupto: a amostra detectada fica a poucas amostras do início real.
            assert!(a.abs_diff(p) <= 48, "detectado {a}, real {p}");
        }
    }

    #[test]
    fn silencio_e_ruido_sozinho_nao_geram_onset() {
        assert!(detectar_onsets(&vec![0.0; 48_000], TAXA, &ConfigOnset::default()).is_empty());
        let r = ruido(TAXA as usize * 2, 0.004, 9);
        assert!(detectar_onsets(&r, TAXA, &ConfigOnset::default()).is_empty());
        assert!(detectar_onsets(&[], TAXA, &ConfigOnset::default()).is_empty());
        assert!(detectar_onsets(&[0.5; 10], TAXA, &ConfigOnset::default()).is_empty());
    }

    #[test]
    fn periodo_refratario_une_cauda_e_repique() {
        let mut sinal = ruido(TAXA as usize, 0.001, 3);
        somar_clique(&mut sinal, 10_000, 0.5, 0.0);
        // 30 ms depois (dentro dos 120 ms): é a mesma nota.
        somar_clique(&mut sinal, 10_000 + 1_440, 0.5, 0.0);
        somar_clique(&mut sinal, 30_000, 0.5, 0.0);
        let achados = detectar_onsets(&sinal, TAXA, &ConfigOnset::default());
        assert_eq!(achados.len(), 2, "{achados:?}");
        // Sem período refratário, os dois primeiros seriam contados separados? Não: a cauda
        // do primeiro não cai abaixo do limiar em 30 ms, então a janela seguinte seria o mesmo
        // onset repetido várias vezes.
        let cfg = ConfigOnset {
            refratario_ms: 1.0,
            ..ConfigOnset::default()
        };
        assert!(detectar_onsets(&sinal, TAXA, &cfg).len() > 2);
    }

    #[test]
    fn limiar_se_adapta_ao_ruido_de_fundo() {
        // Fundo alto (0,03): um clique de 0,5 ainda é achado, e o fundo sozinho não.
        let mut sinal = ruido(TAXA as usize * 2, 0.03, 5);
        assert!(detectar_onsets(&sinal, TAXA, &ConfigOnset::default()).is_empty());
        somar_clique(&mut sinal, 40_000, 0.5, 0.0);
        let achados = detectar_onsets(&sinal, TAXA, &ConfigOnset::default());
        assert_eq!(achados.len(), 1);
        assert!(achados[0].abs_diff(40_000) <= 96);
    }

    #[test]
    fn ataque_lento_e_detectado_depois_e_nunca_antes() {
        let mut sinal = ruido(TAXA as usize, 0.001, 4);
        somar_clique(&mut sinal, 20_000, 0.5, 8.0);
        let achados = detectar_onsets(&sinal, TAXA, &ConfigOnset::default());
        assert_eq!(achados.len(), 1);
        // Viés para mais atraso: depois do início real e antes do pico (8 ms = 384 amostras).
        assert!(
            achados[0] >= 20_000 && achados[0] <= 20_000 + 400,
            "{}",
            achados[0]
        );
    }

    // --- linha do tempo ---

    #[test]
    fn linha_do_tempo_recupera_o_instante_apesar_do_jitter_do_callback_e_da_deriva() {
        // Gravação de 20 s em blocos de 256 frames; o relógio da placa é 50 ppm mais rápido que
        // o do sistema e cada callback acorda 0 a 3 ms atrasado.
        let bloco = 256u64;
        let taxa_real = f64::from(TAXA) * (1.0 + 50e-6);
        let mut lt = LinhaDoTempo::nova(TAXA, 2.0);
        let mut s = 7;
        let t0_ns = 5_000_000_000u64;
        let verdadeiro = |frame: u64| t0_ns as f64 + frame as f64 / taxa_real * 1e9;
        for k in 1..=(20 * u64::from(TAXA) / bloco) {
            let fim = k * bloco;
            let atraso = if k % 17 == 0 {
                0
            } else {
                proximo_aleatorio(&mut s) % 3_000_000
            };
            lt.registrar_bloco((verdadeiro(fim) + atraso as f64) as u64, fim);
        }
        for frame in [100_000u64, 480_000, 900_000] {
            let t = lt.instante_ns(frame).expect("instante");
            let erro_ms = (t as f64 - verdadeiro(frame)) / 1e6;
            // A deriva em 2 s de janela vale até 0,1 ms; o jitter foi removido pelo mínimo.
            assert!(erro_ms.abs() < 0.2, "frame {frame}: erro {erro_ms} ms");
        }
        assert_eq!(lt.instante_ns(10 * 960_000), None, "fora de qualquer bloco");
    }

    // --- pareamento ---

    const MS: u64 = 1_000_000;

    #[test]
    fn pareia_com_atraso_conhecido_e_conta_perdidos_e_orfaos() {
        let eventos = [1_000 * MS, 2_000 * MS, 3_000 * MS, 4_000 * MS];
        // Atrasos 20, 35, (nenhum para o 3º), 28 ms; mais um onset solto em 2,5 s.
        let onsets = [1_020 * MS, 2_035 * MS, 2_500 * MS, 4_028 * MS];
        let p = parear(&eventos, &onsets, 0, 300 * MS);
        let atrasos = p.atrasos_ms();
        assert_eq!(atrasos.len(), 3);
        for (a, e) in atrasos.iter().zip([20.0, 35.0, 28.0]) {
            assert!((a - e).abs() < 1e-9, "{a} vs {e}");
        }
        assert_eq!((p.eventos_sem_onset, p.onsets_orfaos), (1, 1));
    }

    #[test]
    fn eco_do_mesmo_evento_nao_vira_segundo_par_e_janela_vale() {
        let eventos = [1_000 * MS];
        let onsets = [1_040 * MS, 1_200 * MS, 1_900 * MS];
        let p = parear(&eventos, &onsets, 0, 500 * MS);
        assert_eq!(p.pares.len(), 1);
        assert_eq!(p.onsets_orfaos, 2, "eco e onset fora da janela");
        // Onset antes do evento nunca é par.
        let p = parear(&eventos, &[900 * MS], 0, 500 * MS);
        assert!(p.pares.is_empty());
        assert_eq!((p.eventos_sem_onset, p.onsets_orfaos), (1, 1));
        // Atraso mínimo: um onset 2 ms depois é rejeitado se o mínimo é 5 ms.
        let p = parear(&eventos, &[1_002 * MS], 5 * MS, 500 * MS);
        assert!(p.pares.is_empty());
    }

    #[test]
    fn evento_perdido_nao_rouba_o_onset_do_proximo() {
        // O 1º evento não soou; o onset em 1,1 s é do 2º evento (a 1,05 s).
        let eventos = [1_000 * MS, 1_050 * MS + 400 * MS];
        let onsets = [1_450 * MS + 30 * MS];
        let p = parear(&eventos, &onsets, 0, 500 * MS);
        assert_eq!(p.pares.len(), 1);
        assert_eq!(p.pares[0].evento_ns, eventos[1]);
        assert_eq!(p.eventos_sem_onset, 1);
    }

    #[test]
    fn isolamento_descarta_os_dois_lados_de_um_acorde() {
        let ev = [
            1_000 * MS,
            1_010 * MS,
            2_000 * MS,
            3_000 * MS,
            3_100 * MS,
            5_000 * MS,
        ];
        let (ok, descartados) = filtrar_isolados(&ev, 300 * MS);
        assert_eq!(ok, vec![2_000 * MS, 5_000 * MS]);
        assert_eq!(descartados, 4);
        assert_eq!(filtrar_isolados(&[], 300 * MS), (vec![], 0));
    }

    #[test]
    fn so_nota_ligada_com_velocidade_conta() {
        assert!(eh_nota_ligada(&[0x90, 60, 100]));
        assert!(eh_nota_ligada(&[0x9F, 60, 1]));
        assert!(!eh_nota_ligada(&[0x90, 60, 0]), "NoteOn vel 0 é NoteOff");
        assert!(!eh_nota_ligada(&[0x80, 60, 64]));
        assert!(!eh_nota_ligada(&[0xB0, 64, 127]));
        assert!(!eh_nota_ligada(&[0x90, 60]));
        assert!(!eh_nota_ligada(&[]));
    }

    // --- de ponta a ponta com sinal sintético (sem hardware) ---

    #[test]
    fn de_ponta_a_ponta_mede_o_atraso_que_foi_inserido_no_sinal() {
        // 40 notas a cada ~0,5 s; o "app" soa com 31 ms de atraso fixo e jitter de até 4 ms.
        let mut s = 11;
        let t0 = 2_000_000_000u64;
        let mut eventos = Vec::new();
        let mut instante = t0 + 500 * MS;
        let mut inseridos = Vec::new();
        let total = TAXA as usize * 25;
        let mut sinal = ruido(total, 0.002, 21);
        for _ in 0..40 {
            instante += proximo_intervalo_ms(&mut s) * MS;
            let atraso = 31 * MS + proximo_aleatorio(&mut s) % (4 * MS);
            eventos.push(instante);
            inseridos.push(atraso as f64 / 1e6);
            let pos = ((instante + atraso - t0) as f64 / 1e9 * f64::from(TAXA)) as usize;
            somar_clique(&mut sinal, pos, 0.4, 0.2);
        }
        // Linha do tempo ideal: bloco de 256 com callbacks sem atraso.
        let mut lt = LinhaDoTempo::nova(TAXA, 2.0);
        for k in 1..=(total as u64 / 256) {
            lt.registrar_bloco(t0 + k * 256 * 1_000_000_000 / u64::from(TAXA), k * 256);
        }
        let onsets: Vec<u64> = detectar_onsets(&sinal, TAXA, &ConfigOnset::default())
            .iter()
            .filter_map(|i| lt.instante_ns(*i as u64))
            .collect();
        let (isolados, _) = filtrar_isolados(&eventos, 300 * MS);
        let p = parear(&isolados, &onsets, 0, 400 * MS);
        assert_eq!(p.pares.len(), 40, "{p:?}");
        let medido = resumir(&p.atrasos_ms()).expect("resumo");
        let verdade = resumir(&inseridos).expect("resumo");
        assert!(
            (medido.media - verdade.media).abs() < 0.5,
            "{medido:?} vs {verdade:?}"
        );
        assert!(
            (medido.p95 - verdade.p95).abs() < 0.8,
            "{medido:?} vs {verdade:?}"
        );
    }

    // --- geradores ---

    #[test]
    fn intervalos_dos_cliques_sao_irregulares_e_limitados() {
        let mut s = 3;
        let v: Vec<u64> = (0..200).map(|_| proximo_intervalo_ms(&mut s)).collect();
        assert!(v.iter().all(|x| (350..=650).contains(x)));
        let mut distintos = v.clone();
        distintos.sort_unstable();
        distintos.dedup();
        assert!(distintos.len() > 50, "pouca variação: {}", distintos.len());
    }

    #[test]
    fn gerador_de_cliques_registra_o_instante_do_frame_certo_em_blocos_irregulares() {
        let mut g = GeradorCliques::novo(TAXA, 0.5, 1, 100);
        let mut saida = Vec::new();
        let mut registrados = Vec::new();
        let mut t_cb = 1_000_000_000u64;
        let mut frame_total = 0u64;
        // Blocos de tamanhos variados, como um driver real.
        for tam in [256usize, 512, 100, 1024, 333]
            .into_iter()
            .cycle()
            .take(2000)
        {
            let mut bloco = vec![0.0f32; tam];
            g.preencher(&mut bloco, t_cb, &mut |t| registrados.push(t));
            saida.extend_from_slice(&bloco);
            frame_total += tam as u64;
            t_cb = 1_000_000_000 + frame_total * 1_000_000_000 / u64::from(TAXA);
        }
        assert!(registrados.len() > 10);
        // Cada instante registrado corresponde ao primeiro frame não nulo de um clique.
        let onsets = detectar_onsets(
            &saida,
            TAXA,
            &ConfigOnset {
                limiar_min: 0.02,
                ..ConfigOnset::default()
            },
        );
        assert_eq!(onsets.len(), registrados.len());
        for (o, r) in onsets.iter().zip(&registrados) {
            let esperado = 1_000_000_000 + *o as u64 * 1_000_000_000 / u64::from(TAXA);
            assert!(r.abs_diff(esperado) <= 2_000_000, "{r} vs {esperado}");
        }
        // Primeiro clique a 100 ms de início.
        assert!(registrados[0].abs_diff(1_000_000_000 + 100 * MS) <= 1_000);
    }

    #[test]
    fn gerador_de_tom_comeca_no_primeiro_frame_do_bloco_seguinte_ao_pedido() {
        let mut g = GeradorTom::novo(TAXA, 0.5);
        let mut b = vec![1.0f32; 256];
        g.preencher(&mut b, 0);
        assert!(b.iter().all(|x| *x == 0.0), "sem pedido, silêncio");
        g.preencher(&mut b, 1);
        assert!(
            b[0] == 0.0 && b[1..8].iter().any(|x| *x != 0.0),
            "começa no bloco do pedido"
        );
        // Tom termina (250 ms) e um segundo pedido reinicia.
        for _ in 0..60 {
            g.preencher(&mut b, 1);
        }
        assert!(b.iter().all(|x| *x == 0.0));
        g.preencher(&mut b, 2);
        assert!(b[1..8].iter().any(|x| *x != 0.0));
    }

    // --- F02-4: quadros ---

    #[test]
    fn quadros_viram_ms_pela_taxa_da_camera() {
        assert_eq!(
            ler_numeros("# cabeçalho\n12\n\n13,5\n 9 \n"),
            Ok(vec![12.0, 13.5, 9.0])
        );
        assert!(ler_numeros("12\nabc\n").is_err());
        assert!(ler_numeros("-3\n").is_err());
        let ms = quadros_para_ms(&[12.0, 6.0], 240.0);
        assert_eq!(ms, vec![50.0, 25.0]);
    }

    // --- relatório ---

    fn dados(n: usize, atraso: f64) -> DadosMedicao {
        DadosMedicao {
            modo: Some(Modo::MidiAudio),
            rotulo: Some("cerebro, teste de bancada".into()),
            limite_p95_ms: LIMITE_P95_AUDIO_MS,
            dispositivo_saida: Some("HDA Intel PCH: ALC1220 Analog".into()),
            dispositivo_entrada: Some("USB Audio 0123456789AB".into()),
            porta_midi: Some("KB-330 20:0".into()),
            taxa_saida_hz: Some(48_000),
            taxa_entrada_hz: Some(48_000),
            buffer_frames: Some(256),
            duracao_s: 60.0,
            gerou_som: false,
            eventos: n,
            onsets_detectados: n,
            resumo: resumir(&vec![atraso; n]),
            ..DadosMedicao::default()
        }
    }

    #[test]
    fn relatorio_sanitiza_e_declara_limitacoes() {
        let md = montar_relatorio_latencia(&dados(40, 30.0));
        assert!(md.contains("# Latência MIDI → áudio por captura"));
        assert!(
            md.contains("p95 = 30.00 ms ≤ meta de 50.00 ms (F03-4)"),
            "{md}"
        );
        assert!(md.contains("## Limitações"));
        assert!(md.contains("sem** o trajeto USB"));
        assert!(md.contains("Ambiente: cerebro"));
        // O número de série do dispositivo some.
        assert!(!md.contains("0123456789AB"));
        assert!(md.contains("[removido]"));
        assert!(md.contains("p50") && md.contains("p99") && md.contains("jitter p95"));
    }

    #[test]
    fn relatorio_com_poucas_amostras_ou_sem_pares_diz_nao_verificado() {
        let md = montar_relatorio_latencia(&dados(10, 30.0));
        assert!(md.contains("**NÃO VERIFICADO**: 10 amostras"), "{md}");
        assert!(!md.contains("≤ meta"));
        let mut vazio = dados(0, 0.0);
        vazio.resumo = None;
        let md = montar_relatorio_latencia(&vazio);
        assert!(md.contains("NÃO VERIFICADO"));
        assert!(md.contains("Sem pares"));
    }

    #[test]
    fn relatorio_acima_da_meta_nao_aprova() {
        let md = montar_relatorio_latencia(&dados(50, 75.0));
        assert!(md.contains("**acima** da meta"), "{md}");
        assert!(!md.contains("≤ meta"));
    }

    #[test]
    fn relatorio_de_quadros_usa_meta_de_tela_e_resolucao_da_camera() {
        let ms_quadros = quadros_para_ms(&vec![9.0; 35], 240.0);
        let d = DadosMedicao {
            modo: Some(Modo::Quadros),
            limite_p95_ms: LIMITE_P95_TELA_MS,
            fps: Some(240.0),
            eventos: 35,
            resumo: resumir(&ms_quadros),
            ..DadosMedicao::default()
        };
        let md = montar_relatorio_latencia(&d);
        assert!(md.contains("(F02-4)"));
        assert!(md.contains("p95 = 37.50 ms ≤ meta de 60.00 ms"), "{md}");
        assert!(md.contains("±4.17 ms"));
        assert!(!md.contains("Onsets detectados"));
    }
}
