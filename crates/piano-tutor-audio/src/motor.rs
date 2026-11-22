//! Motor de áudio: roda **dentro do callback**. Drena a fila sem lock,
//! converte timestamps em frames pelo relógio mestre, aplica cada evento no
//! frame exato (dividindo o render do bloco), aplica ganho mestre com rampa,
//! limita a [-1, 1] e intercala para o dispositivo.
//!
//! Depois de [`MotorAudio::novo`], `processar_intercalado` não aloca, não faz
//! E/S e não toma lock (a fila de agendados é pré-alocada e limitada).

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};

use crate::comando::{Acao, ComandoAudio, ControleAudio, Quando};
use crate::erro::ErroAudio;
use crate::relogio::{
    ConfigAncora, EstimadorAncora, LeitorRelogio, TaxaAmostragem, ms_para_frames,
};
use crate::sintetizador::Sintetizador;

/// Estado publicado pelo callback em atômicos (lido por UI/diagnóstico).
#[derive(Debug)]
pub struct EstadoMotor {
    pub(crate) frames_renderizados: AtomicU64,
    pub(crate) deslocamento_ns: AtomicI64,
    pub(crate) geracao_panico: AtomicU32,
    callbacks: AtomicU64,
    comandos_atrasados: AtomicU64,
    comandos_descartados_panico: AtomicU64,
    agendados_excedidos: AtomicU64,
    amostras_recortadas: AtomicU64,
    reancoragens: AtomicU64,
    pub(crate) erros_dispositivo: AtomicU64,
    pub(crate) xruns: AtomicU64,
    pub(crate) dispositivo_falhou: AtomicBool,
}

impl EstadoMotor {
    pub(crate) const SEM_ANCORA: i64 = i64::MIN;

    pub(crate) fn novo() -> Self {
        Self {
            frames_renderizados: AtomicU64::new(0),
            deslocamento_ns: AtomicI64::new(Self::SEM_ANCORA),
            geracao_panico: AtomicU32::new(0),
            callbacks: AtomicU64::new(0),
            comandos_atrasados: AtomicU64::new(0),
            comandos_descartados_panico: AtomicU64::new(0),
            agendados_excedidos: AtomicU64::new(0),
            amostras_recortadas: AtomicU64::new(0),
            reancoragens: AtomicU64::new(0),
            erros_dispositivo: AtomicU64::new(0),
            xruns: AtomicU64::new(0),
            dispositivo_falhou: AtomicBool::new(false),
        }
    }

    /// Fotografia dos contadores.
    pub fn telemetria(&self) -> Telemetria {
        Telemetria {
            frames: self.frames_renderizados.load(Ordering::Acquire),
            callbacks: self.callbacks.load(Ordering::Relaxed),
            comandos_atrasados: self.comandos_atrasados.load(Ordering::Relaxed),
            comandos_descartados_panico: self.comandos_descartados_panico.load(Ordering::Relaxed),
            agendados_excedidos: self.agendados_excedidos.load(Ordering::Relaxed),
            amostras_recortadas: self.amostras_recortadas.load(Ordering::Relaxed),
            reancoragens: self.reancoragens.load(Ordering::Relaxed),
            erros_dispositivo: self.erros_dispositivo.load(Ordering::Relaxed),
            xruns: self.xruns.load(Ordering::Relaxed),
            dispositivo_falhou: self.dispositivo_falhou.load(Ordering::Acquire),
        }
    }
}

/// Contadores do motor e da saída.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Telemetria {
    pub frames: u64,
    pub callbacks: u64,
    /// Comandos cujo frame de destino já tinha passado (tocados no início do
    /// bloco seguinte): latência alvo curta demais para o bloco/jitter.
    pub comandos_atrasados: u64,
    pub comandos_descartados_panico: u64,
    /// Agendados aplicados antes da hora porque a fila interna encheu.
    pub agendados_excedidos: u64,
    /// Amostras limitadas a [-1, 1] (ou não finitas zeradas).
    pub amostras_recortadas: u64,
    pub reancoragens: u64,
    pub erros_dispositivo: u64,
    pub xruns: u64,
    pub dispositivo_falhou: bool,
}

/// Configuração do motor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConfigMotor {
    pub taxa: TaxaAmostragem,
    /// Latência alvo somada aos eventos com [`Quando::Instante`]. Deve ser
    /// maior que um bloco do dispositivo mais o jitter de despertar, senão
    /// eventos chegam atrasados (contados em [`Telemetria::comandos_atrasados`]).
    pub latencia_alvo_frames: u64,
    pub capacidade_fila: usize,
    /// Máximo de eventos futuros aguardando o frame de destino.
    pub capacidade_agendados: usize,
    /// Tamanho dos buffers internos; blocos maiores são processados em partes.
    pub max_frames_bloco: usize,
    pub ancora: ConfigAncora,
    pub volume_inicial: f32,
    /// Duração da rampa de mudança de volume, em frames (evita estalo).
    pub rampa_volume_frames: u32,
}

impl ConfigMotor {
    /// Padrão: latência alvo de 10 ms, fila de 1024 comandos.
    pub fn padrao(taxa: TaxaAmostragem) -> Self {
        Self {
            taxa,
            latencia_alvo_frames: ms_para_frames(10.0, taxa),
            capacidade_fila: 1024,
            capacidade_agendados: 1024,
            max_frames_bloco: 4096,
            ancora: ConfigAncora::default(),
            volume_inicial: 1.0,
            rampa_volume_frames: ms_para_frames(5.0, taxa).max(1) as u32,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Agendado {
    frame: u64,
    acao: Acao,
}

/// Motor que o callback de áudio aciona.
pub struct MotorAudio<S: Sintetizador> {
    sintetizador: S,
    receptor: rtrb::Consumer<ComandoAudio>,
    estado: Arc<EstadoMotor>,
    config: ConfigMotor,
    estimador: EstimadorAncora,
    frame: u64,
    agendados: VecDeque<Agendado>,
    esq: Vec<f32>,
    dir: Vec<f32>,
    ganho: f32,
    ganho_alvo: f32,
    passo_rampa: f32,
    geracao_aplicada: u32,
}

impl<S: Sintetizador> MotorAudio<S> {
    /// Cria o motor e o lado de controle da fila. Toda alocação acontece aqui.
    pub fn novo(config: ConfigMotor, sintetizador: S) -> Result<(Self, ControleAudio), ErroAudio> {
        if config.capacidade_fila == 0 {
            return Err(ErroAudio::ParametroInvalido("capacidade_fila deve ser > 0"));
        }
        if config.max_frames_bloco == 0 {
            return Err(ErroAudio::ParametroInvalido(
                "max_frames_bloco deve ser > 0",
            ));
        }
        let estado = Arc::new(EstadoMotor::novo());
        let (produtor, receptor) = rtrb::RingBuffer::new(config.capacidade_fila);
        let ganho = limitar_volume(config.volume_inicial);
        let motor = Self {
            sintetizador,
            receptor,
            estado: Arc::clone(&estado),
            config,
            estimador: EstimadorAncora::novo(config.taxa, config.ancora),
            frame: 0,
            agendados: VecDeque::with_capacity(config.capacidade_agendados),
            esq: vec![0.0; config.max_frames_bloco],
            dir: vec![0.0; config.max_frames_bloco],
            ganho,
            ganho_alvo: ganho,
            passo_rampa: 1.0 / config.rampa_volume_frames.max(1) as f32,
            geracao_aplicada: 0,
        };
        Ok((motor, ControleAudio::novo(produtor, estado)))
    }

    pub fn config(&self) -> &ConfigMotor {
        &self.config
    }

    /// Estado compartilhado (telemetria).
    pub fn estado(&self) -> Arc<EstadoMotor> {
        Arc::clone(&self.estado)
    }

    /// Leitor do relógio mestre para outras threads.
    pub fn leitor_relogio(&self) -> LeitorRelogio {
        LeitorRelogio::novo(Arc::clone(&self.estado), self.config.taxa)
    }

    /// Frame do relógio mestre no início do próximo bloco.
    pub fn frame_atual(&self) -> u64 {
        self.frame
    }

    pub fn sintetizador(&self) -> &S {
        &self.sintetizador
    }

    /// Processa um bloco intercalado de `canais` canais. `agora_ns` é o
    /// instante monotônico do despertar do callback.
    ///
    /// Mono recebe (E+D)/2; com 3 ou mais canais, os extras ficam em silêncio.
    pub fn processar_intercalado(&mut self, saida: &mut [f32], canais: usize, agora_ns: u64) {
        if canais == 0 {
            saida.fill(0.0);
            return;
        }
        let frames = saida.len() / canais;
        let inicio = self.frame;

        self.estimador.atualizar(inicio, agora_ns);
        if let Some(d) = self.estimador.deslocamento_ns() {
            self.estado.deslocamento_ns.store(d, Ordering::Release);
        }
        let geracao = self.estado.geracao_panico.load(Ordering::Acquire);
        if geracao != self.geracao_aplicada {
            self.aplicar_panico(geracao);
        }
        self.drenar_fila(inicio);

        let mut feito = 0;
        let mut recortadas = 0_u64;
        while feito < frames {
            let n = (frames - feito).min(self.config.max_frames_bloco);
            self.renderizar_trecho(n);
            recortadas +=
                self.misturar(&mut saida[feito * canais..(feito + n) * canais], canais, n);
            feito += n;
        }
        saida[frames * canais..].fill(0.0);

        if recortadas > 0 {
            self.estado
                .amostras_recortadas
                .fetch_add(recortadas, Ordering::Relaxed);
        }
        self.estado
            .reancoragens
            .store(self.estimador.reancoragens(), Ordering::Relaxed);
        self.estado.callbacks.fetch_add(1, Ordering::Relaxed);
        self.estado
            .frames_renderizados
            .store(self.frame, Ordering::Release);
    }

    fn aplicar_panico(&mut self, geracao: u32) {
        self.sintetizador.silenciar_tudo();
        let descartados = self.agendados.len() as u64;
        self.agendados.clear();
        if descartados > 0 {
            self.estado
                .comandos_descartados_panico
                .fetch_add(descartados, Ordering::Relaxed);
        }
        self.geracao_aplicada = geracao;
    }

    fn drenar_fila(&mut self, inicio: u64) {
        while let Ok(cmd) = self.receptor.pop() {
            // Geração mais antiga que o último pânico: comando obsoleto.
            let delta = cmd.geracao.wrapping_sub(self.geracao_aplicada) as i32;
            if delta < 0 {
                self.estado
                    .comandos_descartados_panico
                    .fetch_add(1, Ordering::Relaxed);
                continue;
            }
            if delta > 0 {
                // Pânico pedido depois do início deste callback.
                self.aplicar_panico(cmd.geracao);
            }
            let destino = match cmd.quando {
                Quando::Imediato => {
                    self.aplicar(cmd.acao);
                    continue;
                }
                Quando::Instante(ns) => self
                    .estimador
                    .frame_destino(ns, self.config.latencia_alvo_frames)
                    .unwrap_or(inicio),
                Quando::Frame(f) => f,
            };
            if destino < inicio {
                self.estado
                    .comandos_atrasados
                    .fetch_add(1, Ordering::Relaxed);
                self.aplicar(cmd.acao);
            } else if self.agendados.len() >= self.config.capacidade_agendados {
                self.estado
                    .agendados_excedidos
                    .fetch_add(1, Ordering::Relaxed);
                self.aplicar(cmd.acao);
            } else {
                // Inserção ordenada e estável (capacidade reservada: sem alocação).
                let mut pos = self.agendados.len();
                while pos > 0 && self.agendados[pos - 1].frame > destino {
                    pos -= 1;
                }
                self.agendados.insert(
                    pos,
                    Agendado {
                        frame: destino,
                        acao: cmd.acao,
                    },
                );
            }
        }
    }

    fn renderizar_trecho(&mut self, n: usize) {
        let fim = self.frame + n as u64;
        let mut cursor = 0_usize;
        while let Some(prox) = self.agendados.front().copied() {
            if prox.frame >= fim {
                break;
            }
            let rel = (prox.frame.saturating_sub(self.frame) as usize).min(n);
            if rel > cursor {
                self.sintetizador
                    .renderizar(&mut self.esq[cursor..rel], &mut self.dir[cursor..rel]);
                cursor = rel;
            }
            self.agendados.pop_front();
            self.aplicar(prox.acao);
        }
        if cursor < n {
            self.sintetizador
                .renderizar(&mut self.esq[cursor..n], &mut self.dir[cursor..n]);
        }
        self.frame = fim;
    }

    fn misturar(&mut self, saida: &mut [f32], canais: usize, n: usize) -> u64 {
        let mut recortadas = 0_u64;
        for (i, quadro) in saida.chunks_exact_mut(canais).take(n).enumerate() {
            if self.ganho != self.ganho_alvo {
                let diff = self.ganho_alvo - self.ganho;
                if diff.abs() <= self.passo_rampa {
                    self.ganho = self.ganho_alvo;
                } else {
                    self.ganho += self.passo_rampa * diff.signum();
                }
            }
            let e = self.esq[i] * self.ganho;
            let d = self.dir[i] * self.ganho;
            if canais == 1 {
                quadro[0] = limitar(0.5 * (e + d), &mut recortadas);
            } else {
                quadro[0] = limitar(e, &mut recortadas);
                quadro[1] = limitar(d, &mut recortadas);
                quadro[2..].fill(0.0);
            }
        }
        recortadas
    }

    fn aplicar(&mut self, acao: Acao) {
        match acao {
            Acao::NotaLigada {
                canal,
                nota,
                velocidade,
            } => self.sintetizador.nota_ligada(canal, nota, velocidade),
            Acao::NotaDesligada { canal, nota } => self.sintetizador.nota_desligada(canal, nota),
            Acao::Controle {
                canal,
                controle,
                valor,
            } => self.sintetizador.controle(canal, controle, valor),
            Acao::Programa { canal, programa } => self.sintetizador.programa(canal, programa),
            Acao::TodasNotasDesligadas => self.sintetizador.todas_notas_desligadas(),
            Acao::Volume(v) => self.ganho_alvo = limitar_volume(v),
        }
    }
}

fn limitar_volume(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn limitar(x: f32, recortadas: &mut u64) -> f32 {
    if !x.is_finite() {
        *recortadas += 1;
        0.0
    } else if x > 1.0 {
        *recortadas += 1;
        1.0
    } else if x < -1.0 {
        *recortadas += 1;
        -1.0
    } else {
        x
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::relogio::frames_para_ns;
    use crate::sintetizador::{RegistroFalso, SintetizadorFalso};
    use piano_tutor_core::NotaMidi;
    use piano_tutor_core::midi::{CanalMidi, EventoMidi};

    const T0: u64 = 1_000_000_000;

    fn taxa() -> TaxaAmostragem {
        TaxaAmostragem::nova(48_000).expect("taxa")
    }

    fn config() -> ConfigMotor {
        ConfigMotor {
            latencia_alvo_frames: 480,
            ancora: ConfigAncora {
                alfa: 1.0,
                ..ConfigAncora::default()
            },
            ..ConfigMotor::padrao(taxa())
        }
    }

    fn motor() -> (MotorAudio<SintetizadorFalso>, ControleAudio) {
        MotorAudio::novo(config(), SintetizadorFalso::novo()).expect("motor")
    }

    fn c0() -> CanalMidi {
        CanalMidi::novo(0).expect("canal")
    }

    fn n(x: u8) -> NotaMidi {
        NotaMidi::nova(x).expect("nota")
    }

    fn ligar(nota: u8) -> Acao {
        Acao::NotaLigada {
            canal: c0(),
            nota: n(nota),
            velocidade: 127,
        }
    }

    /// Callback ideal: bloco `k` acorda exatamente no instante do seu frame.
    fn bloco(m: &mut MotorAudio<SintetizadorFalso>, frames: usize) -> Vec<f32> {
        let mut saida = vec![0.0; frames * 2];
        let agora = T0 + frames_para_ns(m.frame_atual(), taxa());
        m.processar_intercalado(&mut saida, 2, agora);
        saida
    }

    fn impulsos(saida: &[f32], canais: usize, base: u64) -> Vec<u64> {
        saida
            .chunks_exact(canais)
            .enumerate()
            .filter(|(_, q)| q[0] != 0.0)
            .map(|(i, _)| base + i as u64)
            .collect()
    }

    #[test]
    fn config_invalida_e_rejeitada() {
        let mut c = config();
        c.capacidade_fila = 0;
        assert!(MotorAudio::novo(c, SintetizadorFalso::novo()).is_err());
        let mut c = config();
        c.max_frames_bloco = 0;
        assert!(MotorAudio::novo(c, SintetizadorFalso::novo()).is_err());
    }

    #[test]
    fn silencio_sem_comandos() {
        let (mut m, _c) = motor();
        for _ in 0..10 {
            assert!(bloco(&mut m, 256).iter().all(|x| *x == 0.0));
        }
        assert_eq!(m.frame_atual(), 2_560);
        assert_eq!(m.leitor_relogio().frames(), 2_560);
        assert_eq!(m.estado().telemetria().callbacks, 10);
    }

    #[test]
    fn evento_em_frame_absoluto_soa_no_frame_exato() {
        let (mut m, mut c) = motor();
        for alvo in [0_u64, 1, 100, 255, 256, 700] {
            c.enviar(ComandoAudio::novo(Quando::Frame(alvo + 1_024), ligar(60)))
                .expect("fila");
            let mut achados = Vec::new();
            for _ in 0..8 {
                let base = m.frame_atual();
                achados.extend(impulsos(&bloco(&mut m, 256), 2, base));
            }
            assert_eq!(achados, vec![alvo + 1_024], "alvo {alvo}");
            // Reinicia o frame de referência para o próximo caso.
            let (novo_m, novo_c) = motor();
            m = novo_m;
            c = novo_c;
        }
    }

    #[test]
    fn evento_midi_agendado_com_latencia_alvo() {
        let (mut m, mut c) = motor();
        bloco(&mut m, 256); // ancora: frame 0 ↔ T0
        // Nota capturada 3 ms após T0 → frame 144 + latência 480 = 624.
        let ev = EventoMidi::NotaLigada {
            canal: c0(),
            nota: n(60),
            velocidade: 127,
            timestamp_monotonico_ns: T0 + 3_000_000,
        };
        assert!(c.enviar_evento_midi(&ev).expect("fila"));
        let mut achados = Vec::new();
        for _ in 0..4 {
            let base = m.frame_atual();
            achados.extend(impulsos(&bloco(&mut m, 256), 2, base));
        }
        assert_eq!(achados, vec![624]);
        assert_eq!(m.estado().telemetria().comandos_atrasados, 0);
    }

    #[test]
    fn evento_atrasado_toca_no_inicio_do_bloco_e_e_contado() {
        let (mut m, mut c) = motor();
        bloco(&mut m, 256);
        bloco(&mut m, 256);
        c.enviar(ComandoAudio::novo(Quando::Frame(10), ligar(60)))
            .expect("fila");
        let base = m.frame_atual();
        assert_eq!(impulsos(&bloco(&mut m, 256), 2, base), vec![512]);
        assert_eq!(m.estado().telemetria().comandos_atrasados, 1);
    }

    #[test]
    fn eventos_fora_de_ordem_sao_ordenados() {
        let (mut m, mut c) = motor();
        for f in [300_u64, 100, 200, 100] {
            c.enviar(ComandoAudio::novo(Quando::Frame(f), ligar(60)))
                .expect("fila");
        }
        let saida = bloco(&mut m, 512);
        assert_eq!(impulsos(&saida, 2, 0), vec![100, 200, 300]);
        // Dois eventos no frame 100 somam no mesmo impulso (2 × 1,0 → recorte).
        assert_eq!(saida[200], 1.0);
        let frames: Vec<u64> = m
            .sintetizador()
            .registro()
            .iter()
            .filter_map(|r| match r {
                RegistroFalso::Ligada { frame, .. } => Some(*frame),
                _ => None,
            })
            .collect();
        assert_eq!(frames, vec![100, 100, 200, 300]);
    }

    #[test]
    fn bloco_maior_que_buffer_interno_e_dividido() {
        let cfg = ConfigMotor {
            max_frames_bloco: 64,
            ..config()
        };
        let (mut m, mut c) = MotorAudio::novo(cfg, SintetizadorFalso::novo()).expect("motor");
        c.enviar(ComandoAudio::novo(Quando::Frame(200), ligar(60)))
            .expect("fila");
        let saida = bloco(&mut m, 1_000);
        assert_eq!(impulsos(&saida, 2, 0), vec![200]);
        assert_eq!(m.frame_atual(), 1_000);
    }

    #[test]
    fn mono_e_multicanal() {
        let (mut m, mut c) = motor();
        c.enviar(ComandoAudio::novo(Quando::Frame(3), ligar(60)))
            .expect("fila");
        let mut mono = vec![9.0; 8];
        m.processar_intercalado(&mut mono, 1, T0);
        assert_eq!(mono, vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]);

        let (mut m, mut c) = motor();
        c.enviar(ComandoAudio::novo(Quando::Frame(1), ligar(60)))
            .expect("fila");
        let mut quatro = vec![9.0; 4 * 3 + 2]; // 3 frames + 2 amostras soltas
        m.processar_intercalado(&mut quatro, 4, T0);
        assert_eq!(
            quatro,
            vec![
                0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0
            ]
        );
        let mut nada = vec![9.0; 4];
        m.processar_intercalado(&mut nada, 0, T0);
        assert_eq!(nada, vec![0.0; 4]);
    }

    #[test]
    fn volume_escala_a_saida_com_rampa() {
        let cfg = ConfigMotor {
            rampa_volume_frames: 1,
            ..config()
        };
        let (mut m, mut c) = MotorAudio::novo(cfg, SintetizadorFalso::novo()).expect("motor");
        c.volume(0.25).expect("fila");
        c.enviar(ComandoAudio::novo(Quando::Frame(10), ligar(60)))
            .expect("fila");
        let saida = bloco(&mut m, 64);
        assert_eq!(saida[20], 0.25);
        // Volume fora da faixa é limitado.
        c.volume(7.0).expect("fila");
        c.enviar(ComandoAudio::novo(Quando::Frame(70), ligar(60)))
            .expect("fila");
        let saida = bloco(&mut m, 64);
        assert_eq!(saida[12], 1.0);
        c.volume(f32::NAN).expect("fila");
        c.enviar(ComandoAudio::novo(Quando::Frame(130), ligar(60)))
            .expect("fila");
        assert!(bloco(&mut m, 64).iter().all(|x| *x == 0.0));
    }

    #[test]
    fn rampa_de_volume_e_gradual() {
        let (mut m, mut c) = motor(); // rampa padrão de 5 ms = 240 frames
        c.volume(0.0).expect("fila");
        c.enviar(ComandoAudio::novo(Quando::Frame(120), ligar(60)))
            .expect("fila");
        let saida = bloco(&mut m, 256);
        let v = saida[240];
        assert!(v > 0.4 && v < 0.6, "meio da rampa: {v}");
    }

    #[test]
    fn recorte_e_contado() {
        let (mut m, mut c) = motor();
        for _ in 0..3 {
            c.enviar(ComandoAudio::novo(Quando::Frame(5), ligar(60)))
                .expect("fila");
        }
        let saida = bloco(&mut m, 16);
        assert_eq!(saida[10], 1.0);
        assert_eq!(m.estado().telemetria().amostras_recortadas, 2);
    }

    #[test]
    fn todas_notas_desligadas_libera_vozes() {
        let (mut m, mut c) = motor();
        for nota in 60..70 {
            c.enviar(ComandoAudio::novo(Quando::Imediato, ligar(nota)))
                .expect("fila");
        }
        bloco(&mut m, 64);
        assert_eq!(m.sintetizador().notas_ativas(), 10);
        c.todas_notas_desligadas().expect("fila");
        bloco(&mut m, 64);
        assert_eq!(m.sintetizador().notas_ativas(), 0);
    }

    #[test]
    fn panico_silencia_e_descarta_agendados_anteriores() {
        let (mut m, mut c) = motor();
        c.enviar(ComandoAudio::novo(Quando::Imediato, ligar(60)))
            .expect("fila");
        c.enviar(ComandoAudio::novo(Quando::Frame(1_000), ligar(61)))
            .expect("fila");
        bloco(&mut m, 256);
        // Comando enviado antes do pânico, ainda na fila: obsoleto.
        c.enviar(ComandoAudio::novo(Quando::Frame(600), ligar(62)))
            .expect("fila");
        c.panico();
        // Comando enviado depois do pânico: deve tocar.
        c.enviar(ComandoAudio::novo(Quando::Frame(700), ligar(63)))
            .expect("fila");
        let mut achados = Vec::new();
        for _ in 0..4 {
            let base = m.frame_atual();
            achados.extend(impulsos(&bloco(&mut m, 256), 2, base));
        }
        assert_eq!(achados, vec![700]);
        assert_eq!(m.sintetizador().notas_ativas(), 1);
        let t = m.estado().telemetria();
        assert_eq!(t.comandos_descartados_panico, 2);
        assert!(
            m.sintetizador()
                .registro()
                .iter()
                .any(|r| matches!(r, RegistroFalso::Silenciar { frame: 256 }))
        );
    }

    #[test]
    fn panico_sem_comandos_e_aplicado_no_proximo_bloco() {
        let (mut m, mut c) = motor();
        c.enviar(ComandoAudio::novo(Quando::Imediato, ligar(60)))
            .expect("fila");
        bloco(&mut m, 64);
        c.panico();
        bloco(&mut m, 64);
        assert_eq!(m.sintetizador().notas_ativas(), 0);
    }

    #[test]
    fn agendados_excedidos_sao_aplicados_e_contados() {
        let cfg = ConfigMotor {
            capacidade_agendados: 2,
            ..config()
        };
        let (mut m, mut c) = MotorAudio::novo(cfg, SintetizadorFalso::novo()).expect("motor");
        for f in [500_u64, 600, 700] {
            c.enviar(ComandoAudio::novo(Quando::Frame(f), ligar(60)))
                .expect("fila");
        }
        let saida = bloco(&mut m, 64);
        assert_eq!(impulsos(&saida, 2, 0), vec![0]);
        assert_eq!(m.estado().telemetria().agendados_excedidos, 1);
    }

    #[test]
    fn leitor_relogio_publica_ancora() {
        let (mut m, _c) = motor();
        let leitor = m.leitor_relogio();
        assert_eq!(leitor.instante_de_frame(0), None);
        bloco(&mut m, 480);
        assert_eq!(leitor.frames(), 480);
        assert_eq!(leitor.tempo_audio_ns(), 10_000_000);
        assert_eq!(leitor.instante_de_frame(480), Some(T0 + 10_000_000));
    }
}
