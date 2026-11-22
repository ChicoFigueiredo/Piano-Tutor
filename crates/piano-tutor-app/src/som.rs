//! Serviço de som do app (F03, #38): SoundFont, saída de áudio, timbre, volume e pânico.
//!
//! Sem janela: tudo aqui é testável com um abridor de saída falso (motor com
//! `SintetizadorFalso` renderizado offline) e SF2 sintético.
//!
//! **Produtor único da fila (SPSC).** A fila `rtrb` do motor tem um único produtor, o
//! [`ControleAudio`] guardado aqui. O [`ServicoSom`] vive na thread da UI (que também é a thread
//! de controle): eventos MIDI do gerenciador, eventos da simulação, troca de timbre, volume e
//! pânico entram todos por esta thread. Nenhuma outra thread envia comandos ao motor; o
//! carregamento do SF2 roda em outra thread mas só devolve o `SoundFont` para esta.
//!
//! **Erros recuperáveis.** SF2 ausente ou inválido e saída de áudio indisponível viram texto
//! para a UI; o app continua (Espelho, Monitor MIDI) sem som. Nada aqui entra em pânico.

use piano_tutor_audio::rusty::carregar_soundfont;
use piano_tutor_audio::{
    Acao, ComandoAudio, ConfigRusty, ConfigSaida, ControleAudio, ErroAudio, PresetSf2, Quando,
    Relogio, SaidaAudio, SintetizadorRusty, SoundFont, Telemetria, listar_presets,
};
use piano_tutor_core::midi::{CanalMidi, EventoMidi};
use piano_tutor_core::teclado::{AcaoTecla, EventoTecla};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

/// SoundFont padrão: FluidR3_GM, do pacote Debian `fluid-soundfont-gm` (dependência do .deb,
/// MIT; ver `docs/aceite/F03/auditoria-sf2.md`). Não é embutido no app.
pub const SF2_PADRAO: &str = "/usr/share/sounds/sf2/FluidR3_GM.sf2";
/// Programa GM do piano acústico de cauda.
pub const PROGRAMA_PIANO: u8 = 0;
/// Programa GM do acordeon (segundo timbre preferido, ADR-0005).
pub const PROGRAMA_ACORDEON: u8 = 21;
/// Programa GM do órgão de igreja (segundo timbre alternativo).
pub const PROGRAMA_ORGAO: u8 = 19;
/// Canal de percussão da convenção GM (canal 10 exibido, 9 interno): não recebe timbre.
pub const CANAL_PERCUSSAO: u8 = 9;
/// Velocidade das notas da simulação (mouse e teclado do PC). Constante declarada: a
/// simulação não tem velocity e não finge ter.
pub const VELOCIDADE_SIMULACAO: u8 = 100;
/// Volume mestre inicial (ganho 1,0, o mesmo dos WAV de escuta da F03).
pub const VOLUME_PADRAO: f32 = 1.0;

/// Uma saída de áudio aberta, vista pelo serviço. A real é a [`SaidaAudio`] (cpal).
pub trait SaidaSom {
    /// O produtor único da fila do motor.
    fn controle(&mut self) -> &mut ControleAudio;
    fn telemetria(&self) -> Telemetria;
    /// Texto curto para diagnóstico (dispositivo, taxa, buffer, latência alvo).
    fn descricao(&self) -> String;
}

impl SaidaSom for SaidaAudio {
    fn controle(&mut self) -> &mut ControleAudio {
        SaidaAudio::controle(self)
    }

    fn telemetria(&self) -> Telemetria {
        SaidaAudio::telemetria(self)
    }

    fn descricao(&self) -> String {
        let i = self.info();
        let buffer = i
            .buffer_frames
            .map_or_else(|| "padrão do host".to_owned(), |b| format!("{b} frames"));
        let latencia_ms = i.latencia_alvo_frames as f64 * 1000.0 / f64::from(i.taxa_hz.max(1));
        format!(
            "{} ({}) · {} Hz · {} canais · {} · buffer {} · latência alvo {:.1} ms",
            i.dispositivo, i.host, i.taxa_hz, i.canais, i.formato, buffer, latencia_ms
        )
        .replace('.', ",")
    }
}

/// Abre uma saída de áudio para um SoundFont, usando o relógio compartilhado.
pub type AbridorSaida =
    Box<dyn FnMut(&Arc<SoundFont>, Arc<dyn Relogio>) -> Result<Box<dyn SaidaSom>, ErroAudio>>;

/// Lê um SF2 do disco (roda numa thread de carregamento).
pub type CarregadorSf2 =
    Arc<dyn Fn(&Path) -> Result<Arc<SoundFont>, ErroAudio> + Send + Sync + 'static>;

/// Aviso chamado quando um carregamento termina (pedir um quadro à UI).
pub type AvisoSom = Arc<dyn Fn() + Send + Sync>;

/// Abridor real: dispositivo de saída padrão via `cpal`, sintetizador `rustysynth`.
pub fn abridor_cpal() -> AbridorSaida {
    Box::new(|sf: &Arc<SoundFont>, relogio: Arc<dyn Relogio>| {
        let sf = Arc::clone(sf);
        let saida = SaidaAudio::abrir_padrao(ConfigSaida::default(), relogio, move |taxa| {
            SintetizadorRusty::novo(&sf, taxa, ConfigRusty::default())
        })?;
        Ok(Box::new(saida) as Box<dyn SaidaSom>)
    })
}

/// Carregador real: lê o arquivo com o `rustysynth` (erro recuperável em qualquer falha).
pub fn carregador_arquivo() -> CarregadorSf2 {
    Arc::new(|caminho: &Path| carregar_soundfont(caminho))
}

/// Timbre escolhido: banco e programa do SF2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Timbre {
    pub banco: u16,
    pub programa: u8,
}

impl Timbre {
    pub const PIANO: Timbre = Timbre {
        banco: 0,
        programa: PROGRAMA_PIANO,
    };
}

/// Resumo do estado do som, para a barra de status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EstadoSom {
    /// Saída aberta e SoundFont carregado.
    Pronto,
    /// Lendo o SF2 (sem saída aberta ainda).
    Carregando,
    /// Há SoundFont, mas a saída de áudio não abriu.
    Indisponivel,
    /// Nenhum SoundFont utilizável.
    SemSoundFont,
}

struct Sf2Carregado {
    caminho: PathBuf,
    soundfont: Arc<SoundFont>,
    presets: Vec<PresetSf2>,
}

struct Carregamento {
    caminho: PathBuf,
    rx: Receiver<Result<Arc<SoundFont>, ErroAudio>>,
}

/// Serviço de som. Vive na thread da UI (produtor único da fila do motor).
pub struct ServicoSom {
    relogio: Arc<dyn Relogio>,
    abridor: AbridorSaida,
    carregador: CarregadorSf2,
    aviso: Option<AvisoSom>,
    sf2: Option<Sf2Carregado>,
    carregando: Option<Carregamento>,
    erro_sf2: Option<String>,
    saida: Option<Box<dyn SaidaSom>>,
    erro_saida: Option<String>,
    descricao_saida: Option<String>,
    timbre: Timbre,
    volume: f32,
    comandos_perdidos: u64,
    panicos: u64,
    reaberturas: u64,
    reabertura_automatica_usada: bool,
}

impl std::fmt::Debug for ServicoSom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServicoSom")
            .field("estado", &self.estado())
            .field("sf2", &self.caminho_sf2())
            .field("timbre", &self.timbre)
            .field("volume", &self.volume)
            .finish_non_exhaustive()
    }
}

impl ServicoSom {
    /// `relogio` é o **mesmo** entregue ao gerenciador MIDI (contrato de timestamp).
    pub fn novo(
        relogio: Arc<dyn Relogio>,
        abridor: AbridorSaida,
        carregador: CarregadorSf2,
    ) -> Self {
        Self {
            relogio,
            abridor,
            carregador,
            aviso: None,
            sf2: None,
            carregando: None,
            erro_sf2: None,
            saida: None,
            erro_saida: None,
            descricao_saida: None,
            timbre: Timbre::PIANO,
            volume: VOLUME_PADRAO,
            comandos_perdidos: 0,
            panicos: 0,
            reaberturas: 0,
            reabertura_automatica_usada: false,
        }
    }

    /// Serviço sem dispositivo nem SoundFont (testes de tela e app sem hardware).
    pub fn inativo(relogio: Arc<dyn Relogio>) -> Self {
        let mut s = Self::novo(
            relogio,
            Box::new(|_, _| Err(ErroAudio::SemDispositivo)),
            Arc::new(|c: &Path| {
                Err(ErroAudio::SoundFontInvalido(format!(
                    "{}: carregamento desativado neste modo",
                    c.display()
                )))
            }),
        );
        s.erro_saida = Some("som desativado (modo sem hardware)".to_owned());
        s
    }

    pub fn definir_aviso(&mut self, aviso: AvisoSom) {
        self.aviso = Some(aviso);
    }

    // --- SoundFont -------------------------------------------------------------------------

    /// Começa a carregar um SF2 numa thread própria. Arquivo ausente vira erro na hora.
    /// Enquanto carrega, o som atual (se houver) continua; em caso de falha, também.
    pub fn carregar_sf2(&mut self, caminho: impl AsRef<Path>) {
        let caminho = caminho.as_ref().to_path_buf();
        if !caminho.is_file() {
            self.carregando = None;
            self.erro_sf2 = Some(mensagem_sf2_ausente(&caminho, self.sf2.is_some()));
            return;
        }
        let (tx, rx) = mpsc::channel();
        let carregador = Arc::clone(&self.carregador);
        let aviso = self.aviso.clone();
        let alvo = caminho.clone();
        let criado = std::thread::Builder::new()
            .name("piano-tutor-sf2".to_owned())
            .spawn(move || {
                let _ = tx.send(carregador(&alvo));
                if let Some(a) = aviso {
                    a();
                }
            });
        match criado {
            Ok(_) => {
                self.erro_sf2 = None;
                self.carregando = Some(Carregamento { caminho, rx });
            }
            Err(e) => {
                self.erro_sf2 = Some(format!(
                    "Não foi possível iniciar a leitura de {}: {e}",
                    caminho.display()
                ));
            }
        }
    }

    /// Espera o carregamento em curso terminar (até `limite`) e o aplica. Para testes e para a
    /// captura de tela; a UI usa [`ServicoSom::atualizar`], que não bloqueia.
    pub fn aguardar_carregamento(&mut self, limite: Duration) -> bool {
        let Some(c) = self.carregando.take() else {
            return false;
        };
        match c.rx.recv_timeout(limite) {
            Ok(r) => {
                self.concluir(c.caminho, r);
                true
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.carregando = Some(c);
                false
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.concluir(
                    c.caminho,
                    Err(ErroAudio::SoundFontInvalido(
                        "a leitura terminou sem resultado".to_owned(),
                    )),
                );
                true
            }
        }
    }

    fn concluir(&mut self, caminho: PathBuf, r: Result<Arc<SoundFont>, ErroAudio>) {
        match r {
            Ok(soundfont) => {
                let presets = listar_presets(&soundfont);
                // Troca de SoundFont: corta o som antigo antes de soltar a saída.
                self.fechar_saida();
                self.timbre = escolher_timbre(&presets, self.timbre);
                self.sf2 = Some(Sf2Carregado {
                    caminho,
                    soundfont,
                    presets,
                });
                self.erro_sf2 = None;
                self.reabertura_automatica_usada = false;
                self.abrir_saida();
            }
            Err(e) => {
                let continua = if self.sf2.is_some() {
                    "O som continua com o SoundFont anterior."
                } else {
                    "O app continua sem som."
                };
                self.erro_sf2 = Some(format!(
                    "Não foi possível usar {}: {e}. {continua}",
                    caminho.display()
                ));
            }
        }
    }

    // --- Saída -------------------------------------------------------------------------------

    fn abrir_saida(&mut self) {
        let Some(sf2) = self.sf2.as_ref() else {
            return;
        };
        match (self.abridor)(&sf2.soundfont, Arc::clone(&self.relogio)) {
            Ok(saida) => {
                self.descricao_saida = Some(saida.descricao());
                self.saida = Some(saida);
                self.erro_saida = None;
                self.aplicar_timbre();
                self.aplicar_volume();
            }
            Err(e) => {
                self.saida = None;
                self.descricao_saida = None;
                self.erro_saida = Some(format!(
                    "Saída de áudio indisponível: {e}. O resto do app funciona sem som."
                ));
            }
        }
    }

    fn fechar_saida(&mut self) {
        if let Some(mut s) = self.saida.take() {
            s.controle().panico();
        }
        self.descricao_saida = None;
    }

    /// Fecha e reabre a saída (botão da tela Timbres ou recuperação de falha).
    pub fn reabrir_saida(&mut self) {
        self.fechar_saida();
        self.reaberturas += 1;
        self.reabertura_automatica_usada = false;
        self.abrir_saida();
    }

    /// Chamar a cada quadro: aplica um carregamento concluído e recupera falha de dispositivo
    /// (pânico, fecha e tenta reabrir uma vez; se falhar, fica "indisponível" até o botão).
    pub fn atualizar(&mut self) {
        if let Some(c) = self.carregando.take() {
            match c.rx.try_recv() {
                Ok(r) => self.concluir(c.caminho, r),
                Err(mpsc::TryRecvError::Empty) => self.carregando = Some(c),
                Err(mpsc::TryRecvError::Disconnected) => self.concluir(
                    c.caminho,
                    Err(ErroAudio::SoundFontInvalido(
                        "a leitura terminou sem resultado".to_owned(),
                    )),
                ),
            }
        }
        if self
            .saida
            .as_ref()
            .is_some_and(|s| s.telemetria().dispositivo_falhou)
        {
            // Uma reabertura automática por episódio: abrir pode bloquear (tempo limite do
            // host), então nada de tentar a cada quadro.
            if self.reabertura_automatica_usada {
                self.fechar_saida();
            } else {
                self.reabertura_automatica_usada = true;
                self.fechar_saida();
                self.reaberturas += 1;
                self.abrir_saida();
            }
            if self.saida.is_none() {
                self.erro_saida = Some(
                    "O dispositivo de áudio falhou ou foi removido. Use \"Reabrir saída de \
                     áudio\" quando ele voltar; o resto do app funciona sem som."
                        .to_owned(),
                );
            }
        }
    }

    // --- Comandos ------------------------------------------------------------------------------

    fn enviar(&mut self, comando: ComandoAudio) -> bool {
        let Some(s) = self.saida.as_mut() else {
            return false;
        };
        match s.controle().enviar(comando) {
            Ok(()) => true,
            Err(_) => {
                self.comandos_perdidos += 1;
                // Uma liberação perdida deixaria nota presa (bug crítico): corta tudo.
                if matches!(comando.acao, Acao::NotaDesligada { .. }) {
                    s.controle().panico();
                    self.panicos += 1;
                }
                false
            }
        }
    }

    /// Evento do teclado físico. Notas e controles são agendados pelo timestamp monotônico do
    /// evento (mesmo relógio da saída). Mudança de programa e seleção de banco vindas do
    /// dispositivo **não** trocam o timbre (quem escolhe é a tela Timbres); ficam só no Monitor.
    pub fn evento_midi(&mut self, evento: &EventoMidi) {
        match *evento {
            EventoMidi::ProgramaMudou { .. } | EventoMidi::Desconhecido { .. } => {}
            EventoMidi::ControleMudou { controle, .. } if controle == 0 || controle == 32 => {}
            _ => {
                if let Some(c) = ComandoAudio::de_evento_midi(evento) {
                    self.enviar(c);
                }
            }
        }
    }

    /// Nota da simulação (mouse e teclado do PC): canal 1, velocidade constante, no próximo
    /// bloco de áudio (sem timestamp de captura).
    pub fn nota_simulada(&mut self, evento: EventoTecla) {
        let canal = canal(0);
        let acao = match evento.acao {
            AcaoTecla::Ligar => Acao::NotaLigada {
                canal,
                nota: evento.nota,
                velocidade: VELOCIDADE_SIMULACAO,
            },
            AcaoTecla::Desligar => Acao::NotaDesligada {
                canal,
                nota: evento.nota,
            },
        };
        self.enviar(ComandoAudio::novo(Quando::Imediato, acao));
    }

    /// Dispositivo desconectado ou porta trocada: silencia tudo já (pânico do motor, que não
    /// depende de vaga na fila e também solta o sustain).
    pub fn limpar_notas(&mut self) {
        self.panico();
    }

    /// Pânico: corte imediato de todas as vozes. Os comandos anteriores na fila são
    /// descartados pelo motor, então o timbre e o volume são reenviados logo depois.
    pub fn panico(&mut self) {
        if self.silenciar_e_reaplicar() {
            self.panicos += 1;
        }
    }

    fn silenciar_e_reaplicar(&mut self) -> bool {
        let Some(s) = self.saida.as_mut() else {
            return false;
        };
        s.controle().panico();
        self.aplicar_timbre();
        self.aplicar_volume();
        true
    }

    /// Escolhe o timbre. Precisa existir no SF2 carregado; bancos de percussão (≥ 128) não
    /// são aceitos (na convenção GM tocam só no canal 10). Notas soando são cortadas antes da
    /// troca (spec §3.4: pânico na troca de timbre), para nada ficar preso no timbre antigo.
    pub fn selecionar_timbre(&mut self, banco: u16, programa: u8) -> Result<(), String> {
        if banco > 127 || programa > 127 {
            return Err(format!(
                "O preset {banco}:{programa} é de percussão ou está fora da faixa MIDI; \
                 escolha um timbre melódico (banco 0–127)."
            ));
        }
        let Some(sf2) = self.sf2.as_ref() else {
            return Err("Nenhum SoundFont carregado.".to_owned());
        };
        if !sf2
            .presets
            .iter()
            .any(|p| p.banco == banco && p.programa == programa)
        {
            return Err(format!(
                "O preset {banco}:{programa} não existe neste SoundFont."
            ));
        }
        self.timbre = Timbre { banco, programa };
        self.silenciar_e_reaplicar();
        Ok(())
    }

    fn aplicar_timbre(&mut self) {
        let Timbre { banco, programa } = self.timbre;
        let Ok(banco) = u8::try_from(banco) else {
            return;
        };
        for c in (0..16).filter(|c| *c != CANAL_PERCUSSAO) {
            let canal = canal(c);
            for acao in [
                Acao::Controle {
                    canal,
                    controle: 0,
                    valor: banco,
                },
                Acao::Controle {
                    canal,
                    controle: 32,
                    valor: 0,
                },
                Acao::Programa { canal, programa },
            ] {
                self.enviar(ComandoAudio::novo(Quando::Imediato, acao));
            }
        }
    }

    /// Volume mestre 0,0–1,0 (valores fora da faixa são limitados).
    pub fn definir_volume(&mut self, volume: f32) {
        self.volume = if volume.is_finite() {
            volume.clamp(0.0, 1.0)
        } else {
            VOLUME_PADRAO
        };
        self.aplicar_volume();
    }

    fn aplicar_volume(&mut self) {
        let v = self.volume;
        self.enviar(ComandoAudio::novo(Quando::Imediato, Acao::Volume(v)));
    }

    // --- Leitura -------------------------------------------------------------------------------

    pub fn estado(&self) -> EstadoSom {
        if self.saida.is_some() {
            EstadoSom::Pronto
        } else if self.carregando.is_some() {
            EstadoSom::Carregando
        } else if self.sf2.is_some() || (self.erro_saida.is_some() && self.erro_sf2.is_none()) {
            EstadoSom::Indisponivel
        } else {
            EstadoSom::SemSoundFont
        }
    }

    /// Texto da barra de status.
    pub fn texto_status(&self) -> &'static str {
        match self.estado() {
            EstadoSom::Pronto => "Áudio: pronto",
            EstadoSom::Carregando => "Áudio: carregando SoundFont…",
            EstadoSom::Indisponivel => "Áudio: indisponível",
            EstadoSom::SemSoundFont => "Áudio: sem SoundFont",
        }
    }

    pub fn carregando(&self) -> Option<&Path> {
        self.carregando.as_ref().map(|c| c.caminho.as_path())
    }

    pub fn caminho_sf2(&self) -> Option<&Path> {
        self.sf2.as_ref().map(|s| s.caminho.as_path())
    }

    pub fn presets(&self) -> &[PresetSf2] {
        self.sf2.as_ref().map_or(&[], |s| s.presets.as_slice())
    }

    pub fn tem_preset(&self, banco: u16, programa: u8) -> bool {
        self.presets()
            .iter()
            .any(|p| p.banco == banco && p.programa == programa)
    }

    pub fn timbre(&self) -> Timbre {
        self.timbre
    }

    /// Nome do preset atual no SF2, se carregado.
    pub fn nome_do_timbre(&self) -> Option<&str> {
        let t = self.timbre;
        self.presets()
            .iter()
            .find(|p| p.banco == t.banco && p.programa == t.programa)
            .map(|p| p.nome.as_str())
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn erro_sf2(&self) -> Option<&str> {
        self.erro_sf2.as_deref()
    }

    pub fn erro_saida(&self) -> Option<&str> {
        self.erro_saida.as_deref()
    }

    pub fn descricao_saida(&self) -> Option<&str> {
        self.descricao_saida.as_deref()
    }

    pub fn telemetria(&self) -> Option<Telemetria> {
        self.saida.as_ref().map(|s| s.telemetria())
    }

    /// Comandos que não couberam na fila (deveria ficar em zero).
    pub fn comandos_perdidos(&self) -> u64 {
        self.comandos_perdidos
    }

    pub fn panicos(&self) -> u64 {
        self.panicos
    }

    pub fn reaberturas(&self) -> u64 {
        self.reaberturas
    }
}

fn canal(interno: u8) -> CanalMidi {
    CanalMidi::novo(interno & 0x0F).expect("0–15 é sempre um canal válido")
}

/// Mantém o timbre se ele existe no SF2 novo; senão piano (0:0); senão o primeiro melódico.
fn escolher_timbre(presets: &[PresetSf2], atual: Timbre) -> Timbre {
    let existe = |t: Timbre| {
        presets
            .iter()
            .any(|p| p.banco == t.banco && p.programa == t.programa)
    };
    if existe(atual) {
        return atual;
    }
    if existe(Timbre::PIANO) {
        return Timbre::PIANO;
    }
    presets
        .iter()
        .find(|p| p.banco <= 127)
        .map_or(Timbre::PIANO, |p| Timbre {
            banco: p.banco,
            programa: p.programa,
        })
}

fn mensagem_sf2_ausente(caminho: &Path, ha_anterior: bool) -> String {
    let continua = if ha_anterior {
        "O som continua com o SoundFont anterior."
    } else {
        "O app continua sem som."
    };
    if caminho == Path::new(SF2_PADRAO) {
        format!(
            "SoundFont padrão não encontrado em {SF2_PADRAO}. Instale o pacote \
             fluid-soundfont-gm (dependência do .deb) ou informe outro arquivo .sf2. {continua}"
        )
    } else {
        format!(
            "Arquivo não encontrado: {}. Informe o caminho de um arquivo .sf2. {continua}",
            caminho.display()
        )
    }
}

/// Apoio aos testes sem janela e sem placa de som: um abridor que monta o motor real com
/// `SintetizadorFalso` e deixa o motor com o teste, para render offline.
#[cfg(test)]
pub(crate) mod apoio_testes {
    use super::*;
    use piano_tutor_audio::offline::renderizar;
    use piano_tutor_audio::relogio::frames_para_ns;
    use piano_tutor_audio::sf2_sintetico;
    use piano_tutor_audio::sintetizador::RegistroFalso;
    use piano_tutor_audio::{
        ConfigMotor, EstadoMotor, MotorAudio, SintetizadorFalso, TaxaAmostragem,
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, PoisonError};

    pub const TAXA_HZ: u32 = 48_000;
    pub const BLOCO: usize = 256;

    pub fn taxa() -> TaxaAmostragem {
        TaxaAmostragem::nova(TAXA_HZ).expect("taxa não nula")
    }

    struct SaidaDeTeste {
        controle: ControleAudio,
        estado: Arc<EstadoMotor>,
        falha_em_execucao: Arc<AtomicBool>,
    }

    impl SaidaSom for SaidaDeTeste {
        fn controle(&mut self) -> &mut ControleAudio {
            &mut self.controle
        }
        fn telemetria(&self) -> Telemetria {
            let mut t = self.estado.telemetria();
            t.dispositivo_falhou |= self.falha_em_execucao.load(Ordering::SeqCst);
            t
        }
        fn descricao(&self) -> String {
            "saída de teste (offline)".to_owned()
        }
    }

    /// Motor do teste: o "callback" é o teste chamando [`MotorDeTeste::renderizar`].
    #[derive(Clone, Default)]
    pub struct MotorDeTeste {
        motor: Arc<Mutex<Option<MotorAudio<SintetizadorFalso>>>>,
        /// O relógio que o serviço entregou ao abrir a saída (o "callback" o lê).
        relogio: Arc<Mutex<Option<Arc<dyn Relogio>>>>,
        pub aberturas: Arc<Mutex<u32>>,
        /// Abrir a saída falha (sem placa de som).
        pub falhar: Arc<Mutex<bool>>,
        /// A saída aberta informa falha do dispositivo (cabo removido, fluxo invalidado).
        pub falha_em_execucao: Arc<AtomicBool>,
    }

    impl MotorDeTeste {
        pub fn abridor(&self) -> AbridorSaida {
            let m = self.clone();
            Box::new(move |_sf, relogio| {
                if *m.falhar.lock().unwrap_or_else(PoisonError::into_inner) {
                    return Err(ErroAudio::SemDispositivo);
                }
                *m.relogio.lock().unwrap_or_else(PoisonError::into_inner) = Some(relogio);
                let (motor, controle) =
                    MotorAudio::novo(ConfigMotor::padrao(taxa()), SintetizadorFalso::novo())?;
                let estado = motor.estado();
                *m.motor.lock().unwrap_or_else(PoisonError::into_inner) = Some(motor);
                *m.aberturas.lock().unwrap_or_else(PoisonError::into_inner) += 1;
                Ok(Box::new(SaidaDeTeste {
                    controle,
                    estado,
                    falha_em_execucao: Arc::clone(&m.falha_em_execucao),
                }) as Box<dyn SaidaSom>)
            })
        }

        /// Renderiza `frames` com despertares ideais: o frame 0 do relógio mestre corresponde a
        /// `t0_ns` do relógio comum (o bloco que começa no frame `f` desperta em
        /// `t0_ns + f/taxa`).
        pub fn renderizar(&self, frames: usize, t0_ns: u64) -> Vec<f32> {
            let mut g = self.motor.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(motor) = g.as_mut() else {
                return Vec::new();
            };
            renderizar(motor, frames, BLOCO, 2, t0_ns)
        }

        /// Como o callback real: o bloco que começa no frame atual desperta **agora**, lido
        /// do relógio que o serviço entregou à saída; os seguintes, em tempos ideais.
        pub fn renderizar_no_relogio(&self, frames: usize) -> Vec<f32> {
            let agora = self
                .relogio
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref()
                .map_or(0, |r| r.agora_ns());
            let mut g = self.motor.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(motor) = g.as_mut() else {
                return Vec::new();
            };
            let t0 = agora.saturating_sub(frames_para_ns(motor.frame_atual(), taxa()));
            renderizar(motor, frames, BLOCO, 2, t0)
        }

        pub fn registro(&self) -> Vec<RegistroFalso> {
            self.motor
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref()
                .map(|m| m.sintetizador().registro().to_vec())
                .unwrap_or_default()
        }

        pub fn notas_ativas(&self) -> usize {
            self.motor
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref()
                .map_or(0, |m| m.sintetizador().notas_ativas())
        }

        pub fn latencia_alvo_frames(&self) -> u64 {
            self.motor
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref()
                .map_or(0, |m| m.config().latencia_alvo_frames)
        }
    }

    /// Grava o SF2 sintético (programas 0 e 19) num arquivo temporário.
    /// Cada chamada usa uma subpasta própria: testes em paralelo que pedem o
    /// mesmo nome não podem ler um arquivo ainda sendo escrito por outro.
    pub fn sf2_sintetico_em_arquivo(nome: &str) -> PathBuf {
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let pasta = std::env::temp_dir()
            .join(format!("piano-tutor-testes-{}", std::process::id()))
            .join(format!("sf2-{n}"));
        let _ = std::fs::create_dir_all(&pasta);
        let caminho = pasta.join(nome);
        std::fs::write(&caminho, sf2_sintetico::gerar()).unwrap_or_else(|e| panic!("{e}"));
        caminho
    }

    pub fn arquivo_com(nome: &str, bytes: &[u8]) -> PathBuf {
        let pasta = std::env::temp_dir().join(format!("piano-tutor-testes-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&pasta);
        let caminho = pasta.join(nome);
        std::fs::write(&caminho, bytes).unwrap_or_else(|e| panic!("{e}"));
        caminho
    }

    /// Serviço com o motor de teste e o carregador real de arquivo.
    pub fn servico(relogio: Arc<dyn Relogio>) -> (ServicoSom, MotorDeTeste) {
        let motor = MotorDeTeste::default();
        let s = ServicoSom::novo(relogio, motor.abridor(), carregador_arquivo());
        (s, motor)
    }

    /// Serviço já com o SF2 sintético carregado e a saída de teste aberta.
    pub fn servico_pronto(relogio: Arc<dyn Relogio>) -> (ServicoSom, MotorDeTeste) {
        let (mut s, motor) = servico(relogio);
        s.carregar_sf2(sf2_sintetico_em_arquivo("sintetico.sf2"));
        assert!(s.aguardar_carregamento(Duration::from_secs(30)));
        assert_eq!(s.estado(), EstadoSom::Pronto, "{:?}", s.erro_sf2());
        (s, motor)
    }
}

#[cfg(test)]
mod testes {
    use super::apoio_testes::*;
    use super::*;
    use piano_tutor_audio::RelogioManual;
    use piano_tutor_audio::sf2_sintetico::{PROGRAMA_QUADRADA, PROGRAMA_SENOIDE};
    use piano_tutor_audio::sintetizador::RegistroFalso;
    use piano_tutor_core::NotaMidi;
    use piano_tutor_core::teclado::OrigemEventos;

    fn relogio() -> Arc<dyn Relogio> {
        Arc::new(RelogioManual::novo())
    }

    fn nota(n: u8) -> NotaMidi {
        NotaMidi::nova(n).expect("nota válida")
    }

    fn programas(r: &[RegistroFalso]) -> Vec<(u8, u8)> {
        r.iter()
            .filter_map(|e| match *e {
                RegistroFalso::Programa {
                    canal, programa, ..
                } => Some((canal, programa)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn sf2_sintetico_carrega_lista_presets_abre_saida_e_poe_piano_em_15_canais() {
        let (s, motor) = servico_pronto(relogio());
        assert_eq!(s.texto_status(), "Áudio: pronto");
        let chaves: Vec<(u16, u8)> = s.presets().iter().map(|p| (p.banco, p.programa)).collect();
        assert_eq!(chaves, vec![(0, PROGRAMA_QUADRADA), (0, PROGRAMA_SENOIDE)]);
        assert_eq!(s.timbre(), Timbre::PIANO);
        motor.renderizar(BLOCO, 0);
        let p = programas(&motor.registro());
        assert_eq!(p.len(), 15, "todos os canais menos o 10 (percussão)");
        assert!(
            p.iter()
                .all(|(c, prog)| *c != CANAL_PERCUSSAO && *prog == 0)
        );
    }

    #[test]
    fn troca_de_preset_chega_ao_sintetizador_e_preset_inexistente_e_recusado() {
        let (mut s, motor) = servico_pronto(relogio());
        motor.renderizar(BLOCO, 0);
        assert_eq!(s.selecionar_timbre(0, PROGRAMA_SENOIDE), Ok(()));
        assert_eq!(
            s.timbre(),
            Timbre {
                banco: 0,
                programa: PROGRAMA_SENOIDE
            }
        );
        motor.renderizar(BLOCO, 0);
        let p = programas(&motor.registro());
        assert_eq!(
            p.iter()
                .filter(|(_, prog)| *prog == PROGRAMA_SENOIDE)
                .count(),
            15
        );

        // Acordeon não existe no SF2 sintético: erro recuperável, timbre mantido.
        assert!(!s.tem_preset(0, PROGRAMA_ACORDEON));
        let erro = s
            .selecionar_timbre(0, PROGRAMA_ACORDEON)
            .expect_err("acordeon ausente");
        assert!(erro.contains("não existe"), "{erro}");
        assert!(s.selecionar_timbre(128, 0).is_err(), "percussão recusada");
        assert_eq!(s.timbre().programa, PROGRAMA_SENOIDE);
    }

    #[test]
    fn panico_silencia_ja_e_reenvia_timbre_e_volume() {
        let (mut s, motor) = servico_pronto(relogio());
        s.selecionar_timbre(0, PROGRAMA_SENOIDE)
            .unwrap_or_else(|e| panic!("{e}"));
        s.definir_volume(0.5);
        for n in 60..64 {
            s.nota_simulada(EventoTecla {
                origem: OrigemEventos::Simulacao,
                acao: AcaoTecla::Ligar,
                nota: nota(n),
            });
        }
        motor.renderizar(BLOCO, 0);
        assert_eq!(motor.notas_ativas(), 4);
        s.panico();
        motor.renderizar(BLOCO, 0);
        assert_eq!(motor.notas_ativas(), 0);
        let r = motor.registro();
        let pos_silencio = r
            .iter()
            .rposition(|e| matches!(e, RegistroFalso::Silenciar { .. }))
            .expect("pânico chegou ao sintetizador");
        let depois = programas(&r[pos_silencio..]);
        assert_eq!(depois.len(), 15, "timbre reenviado depois do pânico");
        assert!(depois.iter().all(|(_, p)| *p == PROGRAMA_SENOIDE));
        assert_eq!(s.panicos(), 1);
        assert_eq!(s.volume(), 0.5);
    }

    #[test]
    fn sf2_invalido_e_ausente_viram_erro_recuperavel_sem_derrubar() {
        let (mut s, _motor) = servico(relogio());
        s.carregar_sf2(arquivo_com("lixo.sf2", b"isto nao e um sf2 de verdade"));
        assert!(s.aguardar_carregamento(Duration::from_secs(10)));
        let erro = s.erro_sf2().expect("erro na UI").to_owned();
        assert!(erro.contains("O app continua sem som"), "{erro}");
        assert_eq!(s.estado(), EstadoSom::SemSoundFont);
        assert_eq!(s.texto_status(), "Áudio: sem SoundFont");

        s.carregar_sf2("/caminho/que/nao/existe.sf2");
        assert!(s.erro_sf2().is_some_and(|e| e.contains("não encontrado")));

        // Depois de um SF2 bom, um ruim não derruba o som atual.
        s.carregar_sf2(sf2_sintetico_em_arquivo("bom.sf2"));
        assert!(s.aguardar_carregamento(Duration::from_secs(30)));
        assert_eq!(s.estado(), EstadoSom::Pronto);
        s.carregar_sf2(arquivo_com("lixo2.sf2", &[0u8; 64]));
        assert!(s.aguardar_carregamento(Duration::from_secs(10)));
        assert_eq!(s.estado(), EstadoSom::Pronto);
        assert!(
            s.erro_sf2()
                .is_some_and(|e| e.contains("SoundFont anterior")),
            "{:?}",
            s.erro_sf2()
        );
        assert!(s.caminho_sf2().is_some_and(|c| c.ends_with("bom.sf2")));
    }

    #[test]
    fn sf2_padrao_ausente_tem_mensagem_que_cita_o_pacote() {
        let m = mensagem_sf2_ausente(Path::new(SF2_PADRAO), false);
        assert!(m.contains("fluid-soundfont-gm"), "{m}");
        assert!(m.contains("sem som"), "{m}");
    }

    #[test]
    fn sem_dispositivo_de_audio_fica_indisponivel_e_comandos_sao_ignorados() {
        let (mut s, motor) = servico(relogio());
        *motor
            .falhar
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
        s.carregar_sf2(sf2_sintetico_em_arquivo("sem-placa.sf2"));
        assert!(s.aguardar_carregamento(Duration::from_secs(30)));
        assert_eq!(s.estado(), EstadoSom::Indisponivel);
        assert_eq!(s.texto_status(), "Áudio: indisponível");
        assert!(s.erro_saida().is_some_and(|e| e.contains("sem som")));
        // Nada quebra sem saída.
        s.nota_simulada(EventoTecla {
            origem: OrigemEventos::Simulacao,
            acao: AcaoTecla::Ligar,
            nota: nota(60),
        });
        s.panico();
        s.definir_volume(0.3);
        assert_eq!(s.selecionar_timbre(0, PROGRAMA_SENOIDE), Ok(()));
        assert_eq!(s.comandos_perdidos(), 0);
        // A placa "volta": reabrir funciona e reaplica o timbre escolhido.
        *motor
            .falhar
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = false;
        s.reabrir_saida();
        assert_eq!(s.estado(), EstadoSom::Pronto);
        motor.renderizar(BLOCO, 0);
        assert!(
            programas(&motor.registro())
                .iter()
                .all(|(_, p)| *p == PROGRAMA_SENOIDE)
        );
    }

    #[test]
    fn falha_do_dispositivo_em_execucao_reabre_uma_vez_e_depois_fica_indisponivel() {
        use std::sync::PoisonError;
        use std::sync::atomic::Ordering;
        let (mut s, motor) = servico_pronto(relogio());
        let aberturas = || {
            *motor
                .aberturas
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
        };
        assert_eq!(aberturas(), 1);
        motor.falha_em_execucao.store(true, Ordering::SeqCst);
        s.atualizar();
        assert_eq!(aberturas(), 2, "uma reabertura automática");
        // A saída nova também falha: fecha e fica indisponível, sem tentar a cada quadro.
        for _ in 0..5 {
            s.atualizar();
        }
        assert_eq!(aberturas(), 2);
        assert_eq!(s.estado(), EstadoSom::Indisponivel);
        assert!(s.erro_saida().is_some_and(|e| e.contains("falhou")));
        // O dispositivo volta: o botão reabre.
        motor.falha_em_execucao.store(false, Ordering::SeqCst);
        s.reabrir_saida();
        assert_eq!(s.estado(), EstadoSom::Pronto);
        assert_eq!(aberturas(), 3);
    }

    #[test]
    fn programa_e_banco_vindos_do_dispositivo_nao_trocam_o_timbre() {
        let (mut s, motor) = servico_pronto(relogio());
        motor.renderizar(BLOCO, 0);
        let antes = motor.registro().len();
        let c = canal(0);
        s.evento_midi(&EventoMidi::ProgramaMudou {
            canal: c,
            programa: 40,
            timestamp_monotonico_ns: 0,
        });
        s.evento_midi(&EventoMidi::ControleMudou {
            canal: c,
            controle: 0,
            valor: 5,
            timestamp_monotonico_ns: 0,
        });
        s.evento_midi(&EventoMidi::ControleMudou {
            canal: c,
            controle: 64,
            valor: 127,
            timestamp_monotonico_ns: 0,
        });
        motor.renderizar(BLOCO * 8, 0);
        let novos = &motor.registro()[antes..];
        assert_eq!(novos.len(), 1, "{novos:?}");
        assert!(matches!(
            novos[0],
            RegistroFalso::Controle {
                controle: 64,
                valor: 127,
                ..
            }
        ));
    }

    #[test]
    fn volume_e_limitado() {
        let (mut s, _m) = servico(relogio());
        s.definir_volume(3.0);
        assert_eq!(s.volume(), 1.0);
        s.definir_volume(-1.0);
        assert_eq!(s.volume(), 0.0);
        s.definir_volume(f32::NAN);
        assert_eq!(s.volume(), VOLUME_PADRAO);
    }

    #[test]
    fn escolher_timbre_mantem_o_atual_ou_cai_para_piano_ou_primeiro_melodico() {
        let p = |banco, programa| PresetSf2 {
            banco,
            programa,
            nome: "x".into(),
        };
        let t = |banco, programa| Timbre { banco, programa };
        assert_eq!(escolher_timbre(&[p(0, 0), p(0, 21)], t(0, 21)), t(0, 21));
        assert_eq!(escolher_timbre(&[p(0, 0), p(0, 19)], t(0, 21)), t(0, 0));
        assert_eq!(escolher_timbre(&[p(0, 5), p(128, 0)], t(0, 21)), t(0, 5));
    }

    /// Com o SF2 padrão instalado (pacote `fluid-soundfont-gm`): carrega pelo mesmo caminho do
    /// app e confere que piano (0), órgão (19) e acordeon (21) existem no banco 0. Ignorado por
    /// padrão (lê 148 MB); rodar com `--ignored`. Não avalia timbre (checklist de escuta).
    #[test]
    #[ignore = "precisa de /usr/share/sounds/sf2/FluidR3_GM.sf2"]
    fn sf2_padrao_do_sistema_tem_piano_orgao_e_acordeon() {
        let (mut s, motor) = servico(relogio());
        s.carregar_sf2(SF2_PADRAO);
        assert!(
            s.aguardar_carregamento(Duration::from_secs(300)),
            "{:?}",
            s.erro_sf2()
        );
        assert_eq!(s.estado(), EstadoSom::Pronto, "{:?}", s.erro_sf2());
        for prog in [PROGRAMA_PIANO, PROGRAMA_ORGAO, PROGRAMA_ACORDEON] {
            assert!(s.tem_preset(0, prog), "falta o programa {prog}");
        }
        println!(
            "presets: {} · piano = {:?}",
            s.presets().len(),
            s.nome_do_timbre()
        );
        assert_eq!(s.selecionar_timbre(0, PROGRAMA_ACORDEON), Ok(()));
        println!("segundo timbre = {:?}", s.nome_do_timbre());
        motor.renderizar(BLOCO, 0);
        assert_eq!(motor.notas_ativas(), 0);
    }
}
