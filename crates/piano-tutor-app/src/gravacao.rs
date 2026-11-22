//! Gravar, importar e reproduzir MIDI (F06).
//!
//! - **Gravação**: cada evento do teclado físico (e as notas simuladas pelo mouse/teclado do PC)
//!   é guardado com o instante relativo ao início da gravação, no mesmo relógio monotônico do
//!   restante do app. Altura, velocidade, ordem, canal e o pedal (CC64) são preservados.
//! - **Arquivo**: Standard MIDI File escrito e lido à mão (sem biblioteca nova). Escreve tipo 0
//!   a 480 PPQ com meta de andamento (120 bpm se a gravação não tem outro). Lê tipo 0 e 1
//!   (junta as trilhas e respeita mudanças de andamento). Arquivos malformados devolvem
//!   [`ErroSmf`]: nada de pânico, e a memória é limitada por tamanho de arquivo e número de
//!   eventos.
//! - **Reprodução**: agenda à frente (~150 ms) no canal de demonstração do [`ServicoSom`], com o
//!   timbre escolhido em `timbre_demonstracao`. Pausar e parar soltam as notas que estão soando.
//!
//! Resolução temporal: a 480 PPQ e 120 bpm, 1 tick vale cerca de 1,04 ms. Exportar e importar
//! preserva cada instante com erro de no máximo meio tick na escrita (e nenhum depois da
//! primeira ida e volta); a tolerância documentada do aceite é de 1 tick.
//!
//! Limitações registradas: durante a contagem os toques não são gravados; mensagens fora de
//! notas, CC e programa (pitch bend, aftertouch, SysEx) são ignoradas na importação; na
//! reprodução só o pedal (CC64) é repassado e o programa/volume do arquivo não mexem no timbre
//! do app; notas do canal 10 tocam na percussão e as demais no canal de demonstração.

use crate::ritmo::nome_seguro;
use crate::som::{
    CANAL_DEMONSTRACAO, CANAL_PERCUSSAO, CANAL_PRINCIPAL, ServicoSom, VELOCIDADE_SIMULACAO,
};
use piano_tutor_audio::Acao;
use piano_tutor_core::NotaMidi;
use piano_tutor_core::midi::{CanalMidi, EventoMidi};
use piano_tutor_core::ritmo::{CLIQUE_FORTE, CLIQUE_FRACO};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Resolução do arquivo escrito: ticks por semínima.
pub const PPQ: u16 = 480;
/// Andamento padrão: 120 bpm = 500 000 µs por semínima.
pub const US_POR_SEMINIMA_PADRAO: u32 = 500_000;
/// Maior arquivo aceito na importação.
pub const TAMANHO_MAX_ARQUIVO: u64 = 4 * 1024 * 1024;
/// Maior número de eventos (ou mudanças de andamento) guardados.
pub const EVENTOS_MAX: usize = 500_000;
/// Maior número de trilhas lidas de um arquivo.
pub const TRILHAS_MAX: usize = 256;
/// Quanto à frente a reprodução agenda.
pub const ANTECEDENCIA: Duration = Duration::from_millis(150);
/// Atraso entre apertar "Tocar" e o primeiro som.
pub const ATRASO_INICIO: Duration = Duration::from_millis(100);
/// Duração dos cliques da contagem.
const DURACAO_CLIQUE_NS: u64 = 90_000_000;
const NS_POR_MS: u64 = 1_000_000;
/// Limite de ticks (evita contas absurdas com delta enorme em arquivo hostil).
const TICK_MAX: u64 = 1 << 40;
const US_MAX: u32 = 0x00FF_FFFF;
const CC_SUSTAIN: u8 = 64;

fn ns(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}

// --- Modelo -------------------------------------------------------------------------------------

/// Mensagem MIDI guardada (canal 0–15, notas e valores 0–127).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mensagem {
    NotaLigada { canal: u8, nota: u8, velocidade: u8 },
    NotaDesligada { canal: u8, nota: u8, velocidade: u8 },
    Controle { canal: u8, controle: u8, valor: u8 },
    Programa { canal: u8, programa: u8 },
}

impl Mensagem {
    /// Converte um evento normalizado; o que não é nota, CC ou programa vira `None`.
    pub fn de_evento(ev: &EventoMidi) -> Option<Self> {
        Some(match *ev {
            EventoMidi::NotaLigada {
                canal,
                nota,
                velocidade,
                ..
            } => Self::NotaLigada {
                canal: canal.interno(),
                nota: nota.numero(),
                velocidade,
            },
            EventoMidi::NotaDesligada {
                canal,
                nota,
                velocidade,
                ..
            } => Self::NotaDesligada {
                canal: canal.interno(),
                nota: nota.numero(),
                velocidade,
            },
            EventoMidi::ControleMudou {
                canal,
                controle,
                valor,
                ..
            } => Self::Controle {
                canal: canal.interno(),
                controle,
                valor,
            },
            EventoMidi::ProgramaMudou {
                canal, programa, ..
            } => Self::Programa {
                canal: canal.interno(),
                programa,
            },
            EventoMidi::Desconhecido { .. } => return None,
        })
    }
}

/// Mensagem com o instante relativo ao início da gravação.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventoGravado {
    pub t_ns: u64,
    pub msg: Mensagem,
}

/// Uma nota pareada (ligar e soltar), para a linha do tempo.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NotaLinha {
    pub nota: u8,
    pub canal: u8,
    pub velocidade: u8,
    pub inicio_ns: u64,
    pub fim_ns: u64,
}

/// Uma gravação: eventos em ordem de tempo, mudanças de andamento e duração total.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Gravacao {
    pub nome: String,
    pub eventos: Vec<EventoGravado>,
    /// Mudanças de andamento `(instante em ns, µs por semínima)`. Vazio = 120 bpm.
    pub tempos: Vec<(u64, u32)>,
    pub duracao_ns: u64,
}

impl Gravacao {
    pub fn novo(nome: impl Into<String>) -> Self {
        Self {
            nome: nome.into(),
            ..Self::default()
        }
    }

    /// Quantas notas foram tocadas.
    pub fn total_notas(&self) -> usize {
        self.eventos
            .iter()
            .filter(|e| matches!(e.msg, Mensagem::NotaLigada { .. }))
            .count()
    }

    /// Notas pareadas (cada "ligar" com o "soltar" seguinte da mesma nota e canal); as que
    /// ficam abertas terminam na duração da gravação.
    pub fn notas(&self) -> Vec<NotaLinha> {
        let mut abertas: HashMap<(u8, u8), VecDeque<usize>> = HashMap::new();
        let mut saida: Vec<NotaLinha> = Vec::new();
        for e in &self.eventos {
            match e.msg {
                Mensagem::NotaLigada {
                    canal,
                    nota,
                    velocidade,
                } => {
                    abertas
                        .entry((canal, nota))
                        .or_default()
                        .push_back(saida.len());
                    saida.push(NotaLinha {
                        nota,
                        canal,
                        velocidade,
                        inicio_ns: e.t_ns,
                        fim_ns: self.duracao_ns.max(e.t_ns),
                    });
                }
                Mensagem::NotaDesligada { canal, nota, .. } => {
                    if let Some(i) = abertas
                        .get_mut(&(canal, nota))
                        .and_then(VecDeque::pop_front)
                    {
                        saida[i].fim_ns = e.t_ns;
                    }
                }
                _ => {}
            }
        }
        saida
    }

    /// Escreve um Standard MIDI File tipo 0 a [`PPQ`].
    pub fn para_smf(&self) -> Result<Vec<u8>, ErroSmf> {
        if self.eventos.len() > EVENTOS_MAX || self.tempos.len() > EVENTOS_MAX {
            return Err(ErroSmf::GrandeDemais("eventos"));
        }
        let mapa = MapaTempo::de_ns(&self.tempos, PPQ);
        struct Item {
            tick: u64,
            ordem: u8,
            n: usize,
            b: [u8; 6],
        }
        let mut itens: Vec<Item> = Vec::with_capacity(self.eventos.len() + mapa.segs.len());
        for s in &mapa.segs {
            let us = s.us.to_be_bytes();
            itens.push(Item {
                tick: s.tick,
                ordem: 0,
                n: 6,
                b: [0xFF, 0x51, 0x03, us[1], us[2], us[3]],
            });
        }
        let mut anterior = 0u64;
        for e in &self.eventos {
            let tick = mapa.ns_para_tick(e.t_ns, PPQ).max(anterior);
            anterior = tick;
            let (n, b) = match e.msg {
                Mensagem::NotaLigada {
                    canal,
                    nota,
                    velocidade,
                } => (3, [0x90 | (canal & 0xF), nota & 0x7F, velocidade & 0x7F]),
                Mensagem::NotaDesligada {
                    canal,
                    nota,
                    velocidade,
                } => (3, [0x80 | (canal & 0xF), nota & 0x7F, velocidade & 0x7F]),
                Mensagem::Controle {
                    canal,
                    controle,
                    valor,
                } => (3, [0xB0 | (canal & 0xF), controle & 0x7F, valor & 0x7F]),
                Mensagem::Programa { canal, programa } => {
                    (2, [0xC0 | (canal & 0xF), programa & 0x7F, 0])
                }
            };
            itens.push(Item {
                tick,
                ordem: 1,
                n,
                b: [b[0], b[1], b[2], 0, 0, 0],
            });
        }
        let fim = mapa.ns_para_tick(self.duracao_ns, PPQ).max(anterior);
        itens.sort_by_key(|i| (i.tick, i.ordem));

        let mut trilha: Vec<u8> = Vec::with_capacity(itens.len() * 4 + 16);
        let mut atual = 0u64;
        for i in &itens {
            escrever_vlq(&mut trilha, i.tick - atual)?;
            atual = i.tick;
            trilha.extend_from_slice(&i.b[..i.n]);
        }
        escrever_vlq(&mut trilha, fim.saturating_sub(atual))?;
        trilha.extend_from_slice(&[0xFF, 0x2F, 0x00]);

        let tamanho = u32::try_from(trilha.len()).map_err(|_| ErroSmf::GrandeDemais("trilha"))?;
        let mut saida = Vec::with_capacity(trilha.len() + 22);
        saida.extend_from_slice(b"MThd");
        saida.extend_from_slice(&6u32.to_be_bytes());
        saida.extend_from_slice(&0u16.to_be_bytes());
        saida.extend_from_slice(&1u16.to_be_bytes());
        saida.extend_from_slice(&PPQ.to_be_bytes());
        saida.extend_from_slice(b"MTrk");
        saida.extend_from_slice(&tamanho.to_be_bytes());
        saida.extend_from_slice(&trilha);
        Ok(saida)
    }

    /// Lê um Standard MIDI File tipo 0 ou 1 (trilhas juntas, andamento respeitado).
    pub fn de_smf(dados: &[u8], nome: &str) -> Result<Self, ErroSmf> {
        if dados.len() as u64 > TAMANHO_MAX_ARQUIVO {
            return Err(ErroSmf::GrandeDemais("arquivo"));
        }
        let mut l = Leitor::novo(dados);
        let id = l.bytes(4).map_err(|_| ErroSmf::CabecalhoInvalido)?;
        if id != b"MThd" {
            return Err(ErroSmf::CabecalhoInvalido);
        }
        let tam = l.u32().map_err(|_| ErroSmf::CabecalhoInvalido)?;
        if !(6..=1024).contains(&tam) {
            return Err(ErroSmf::CabecalhoInvalido);
        }
        let formato = l.u16()?;
        let _trilhas = l.u16()?;
        let divisao = l.u16()?;
        l.bytes(tam as usize - 6)?;
        match formato {
            0 | 1 => {}
            2 => return Err(ErroSmf::FormatoNaoSuportado),
            _ => return Err(ErroSmf::CabecalhoInvalido),
        }
        if divisao & 0x8000 != 0 || divisao == 0 {
            return Err(ErroSmf::DivisaoNaoSuportada);
        }

        let mut brutos: Vec<(u64, Mensagem)> = Vec::new();
        let mut tempos: Vec<(u64, u32)> = Vec::new();
        let mut fim_tick = 0u64;
        let mut n_trilhas = 0usize;
        while l.restante() >= 8 {
            let id = l.bytes(4)?;
            let tam = l.u32()? as usize;
            let corpo = l.bytes(tam)?;
            if id == b"MTrk" {
                n_trilhas += 1;
                if n_trilhas > TRILHAS_MAX {
                    return Err(ErroSmf::GrandeDemais("trilhas"));
                }
                ler_trilha(corpo, &mut brutos, &mut tempos, &mut fim_tick)?;
            }
        }
        if n_trilhas == 0 {
            return Err(ErroSmf::SemTrilhas);
        }

        // `sort_by_key` é estável: no mesmo tick vale a ordem do arquivo (trilha por trilha).
        brutos.sort_by_key(|(t, _)| *t);
        tempos.sort_by_key(|(t, _)| *t);
        let mapa = MapaTempo::de_ticks(&tempos, divisao)?;
        let mut eventos = Vec::with_capacity(brutos.len());
        for (tick, msg) in brutos {
            eventos.push(EventoGravado {
                t_ns: mapa.tick_para_ns(tick, divisao)?,
                msg,
            });
        }
        let mut duracao_ns = mapa.tick_para_ns(fim_tick, divisao)?;
        if let Some(u) = eventos.last() {
            duracao_ns = duracao_ns.max(u.t_ns);
        }
        let tempos_ns = mapa
            .segs
            .iter()
            .filter(|s| s.tick > 0 || s.us != US_POR_SEMINIMA_PADRAO)
            .map(|s| (s.ns, s.us))
            .collect();
        Ok(Self {
            nome: nome.to_owned(),
            eventos,
            tempos: tempos_ns,
            duracao_ns,
        })
    }
}

// --- Erros --------------------------------------------------------------------------------------

/// Erro ao ler ou escrever um arquivo MIDI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ErroSmf {
    /// O arquivo acaba antes do que ele mesmo declara.
    Truncado,
    /// Não começa com um cabeçalho `MThd` válido.
    CabecalhoInvalido,
    /// Formato 2 (trilhas independentes) não é suportado.
    FormatoNaoSuportado,
    /// Divisão SMPTE ou zero.
    DivisaoNaoSuportada,
    /// Nenhuma trilha `MTrk`.
    SemTrilhas,
    /// Conteúdo de trilha inválido.
    TrilhaInvalida(&'static str),
    /// Passa dos limites de tamanho do app.
    GrandeDemais(&'static str),
    /// Duração absurda.
    DuracaoInvalida,
}

impl fmt::Display for ErroSmf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncado => write!(f, "arquivo MIDI truncado"),
            Self::CabecalhoInvalido => write!(f, "não é um arquivo MIDI válido (cabeçalho)"),
            Self::FormatoNaoSuportado => write!(f, "arquivo MIDI tipo 2 não é suportado"),
            Self::DivisaoNaoSuportada => {
                write!(f, "divisão de tempo SMPTE ou inválida não é suportada")
            }
            Self::SemTrilhas => write!(f, "o arquivo MIDI não tem trilhas"),
            Self::TrilhaInvalida(m) => write!(f, "trilha MIDI inválida: {m}"),
            Self::GrandeDemais(o) => write!(f, "{o} grande demais para o app"),
            Self::DuracaoInvalida => write!(f, "duração do arquivo MIDI inválida"),
        }
    }
}

impl std::error::Error for ErroSmf {}

// --- Mapa de andamento --------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct Seg {
    tick: u64,
    ns: u64,
    us: u32,
}

/// Segmentos de andamento, do primeiro (sempre no tick 0) ao último.
#[derive(Clone, Debug)]
struct MapaTempo {
    segs: Vec<Seg>,
}

fn div_arred(n: u128, d: u128) -> u128 {
    (n + d / 2) / d.max(1)
}

impl MapaTempo {
    fn inicial() -> Seg {
        Seg {
            tick: 0,
            ns: 0,
            us: US_POR_SEMINIMA_PADRAO,
        }
    }

    /// A partir de mudanças em ticks (já ordenadas). Mesmo tick: vale a última.
    fn de_ticks(tempos: &[(u64, u32)], ppq: u16) -> Result<Self, ErroSmf> {
        let mut segs = vec![Self::inicial()];
        for &(tick, us) in tempos {
            let ultimo = segs[segs.len() - 1];
            if tick == ultimo.tick {
                let n = segs.len() - 1;
                segs[n].us = us;
            } else {
                let delta = u128::from(tick - ultimo.tick) * u128::from(ultimo.us) * 1000;
                let dns = u64::try_from(div_arred(delta, u128::from(ppq)))
                    .map_err(|_| ErroSmf::DuracaoInvalida)?;
                segs.push(Seg {
                    tick,
                    ns: ultimo.ns.checked_add(dns).ok_or(ErroSmf::DuracaoInvalida)?,
                    us,
                });
            }
        }
        Ok(Self { segs })
    }

    /// A partir de mudanças em ns (qualquer ordem).
    fn de_ns(tempos: &[(u64, u32)], ppq: u16) -> Self {
        let mut ord: Vec<(u64, u32)> = tempos.to_vec();
        ord.sort_by_key(|(t, _)| *t);
        let mut segs = vec![Self::inicial()];
        for (t_ns, us) in ord {
            let us = us.clamp(1, US_MAX);
            let ultimo = segs[segs.len() - 1];
            if t_ns == ultimo.ns {
                let n = segs.len() - 1;
                segs[n].us = us;
            } else {
                let tick = ultimo.tick + ns_para_ticks(t_ns - ultimo.ns, ultimo.us, ppq);
                segs.push(Seg { tick, ns: t_ns, us });
            }
        }
        Self { segs }
    }

    fn tick_para_ns(&self, tick: u64, ppq: u16) -> Result<u64, ErroSmf> {
        let i = self
            .segs
            .partition_point(|s| s.tick <= tick)
            .saturating_sub(1);
        let s = self.segs[i];
        let delta = u128::from(tick - s.tick) * u128::from(s.us) * 1000;
        let dns = u64::try_from(div_arred(delta, u128::from(ppq)))
            .map_err(|_| ErroSmf::DuracaoInvalida)?;
        s.ns.checked_add(dns).ok_or(ErroSmf::DuracaoInvalida)
    }

    fn ns_para_tick(&self, t_ns: u64, ppq: u16) -> u64 {
        let i = self
            .segs
            .partition_point(|s| s.ns <= t_ns)
            .saturating_sub(1);
        let s = self.segs[i];
        s.tick + ns_para_ticks(t_ns - s.ns, s.us, ppq)
    }
}

fn ns_para_ticks(dns: u64, us: u32, ppq: u16) -> u64 {
    let t = div_arred(
        u128::from(dns) * u128::from(ppq),
        u128::from(us.max(1)) * 1000,
    );
    u64::try_from(t).unwrap_or(u64::MAX).min(TICK_MAX)
}

// --- Leitura e escrita de baixo nível ----------------------------------------------------------

fn escrever_vlq(saida: &mut Vec<u8>, valor: u64) -> Result<(), ErroSmf> {
    if valor > 0x0FFF_FFFF {
        return Err(ErroSmf::DuracaoInvalida);
    }
    let v = valor as u32;
    let mut partes = [0u8; 4];
    let mut n = 0;
    let mut resto = v;
    loop {
        partes[n] = (resto & 0x7F) as u8;
        n += 1;
        resto >>= 7;
        if resto == 0 {
            break;
        }
    }
    for i in (0..n).rev() {
        saida.push(partes[i] | if i > 0 { 0x80 } else { 0 });
    }
    Ok(())
}

struct Leitor<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Leitor<'a> {
    fn novo(d: &'a [u8]) -> Self {
        Self { d, p: 0 }
    }

    fn restante(&self) -> usize {
        self.d.len() - self.p
    }

    fn fim(&self) -> bool {
        self.p >= self.d.len()
    }

    fn u8(&mut self) -> Result<u8, ErroSmf> {
        let b = *self.d.get(self.p).ok_or(ErroSmf::Truncado)?;
        self.p += 1;
        Ok(b)
    }

    fn bytes(&mut self, n: usize) -> Result<&'a [u8], ErroSmf> {
        let fim = self.p.checked_add(n).ok_or(ErroSmf::Truncado)?;
        let s = self.d.get(self.p..fim).ok_or(ErroSmf::Truncado)?;
        self.p = fim;
        Ok(s)
    }

    fn u16(&mut self) -> Result<u16, ErroSmf> {
        let b = self.bytes(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, ErroSmf> {
        let b = self.bytes(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Quantidade de tamanho variável: no máximo 4 bytes.
    fn vlq(&mut self) -> Result<u32, ErroSmf> {
        let mut v = 0u32;
        for _ in 0..4 {
            let b = self.u8()?;
            v = (v << 7) | u32::from(b & 0x7F);
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(ErroSmf::TrilhaInvalida("quantidade variável longa demais"))
    }

    fn dado(&mut self, primeiro: Option<u8>) -> Result<u8, ErroSmf> {
        let b = match primeiro {
            Some(b) => b,
            None => self.u8()?,
        };
        if b > 0x7F {
            return Err(ErroSmf::TrilhaInvalida("byte de dados maior que 127"));
        }
        Ok(b)
    }
}

fn ler_trilha(
    corpo: &[u8],
    brutos: &mut Vec<(u64, Mensagem)>,
    tempos: &mut Vec<(u64, u32)>,
    fim_tick: &mut u64,
) -> Result<(), ErroSmf> {
    let mut l = Leitor::novo(corpo);
    let mut tick = 0u64;
    let mut running: Option<u8> = None;
    while !l.fim() {
        let delta = l.vlq()?;
        tick = tick
            .checked_add(u64::from(delta))
            .filter(|t| *t <= TICK_MAX)
            .ok_or(ErroSmf::DuracaoInvalida)?;
        let b = l.u8()?;
        let (status, primeiro) = if b >= 0x80 {
            (b, None)
        } else {
            match running {
                Some(s) => (s, Some(b)),
                None => return Err(ErroSmf::TrilhaInvalida("dado sem status anterior")),
            }
        };
        match status {
            0xFF => {
                running = None;
                let tipo = l.u8()?;
                let tam = l.vlq()? as usize;
                let dados = l.bytes(tam)?;
                if tipo == 0x2F {
                    break;
                }
                if tipo == 0x51 {
                    if dados.len() != 3 {
                        return Err(ErroSmf::TrilhaInvalida("andamento com tamanho errado"));
                    }
                    let us = u32::from_be_bytes([0, dados[0], dados[1], dados[2]]);
                    if us == 0 {
                        return Err(ErroSmf::TrilhaInvalida("andamento zero"));
                    }
                    if tempos.len() >= EVENTOS_MAX {
                        return Err(ErroSmf::GrandeDemais("mudanças de andamento"));
                    }
                    tempos.push((tick, us));
                }
            }
            0xF0 | 0xF7 => {
                running = None;
                let tam = l.vlq()? as usize;
                l.bytes(tam)?;
            }
            0xF1..=0xFE => return Err(ErroSmf::TrilhaInvalida("status de sistema inesperado")),
            _ => {
                running = Some(status);
                let canal = status & 0x0F;
                let d1 = l.dado(primeiro)?;
                let msg = match status >> 4 {
                    0x8 => {
                        let d2 = l.dado(None)?;
                        Some(Mensagem::NotaDesligada {
                            canal,
                            nota: d1,
                            velocidade: d2,
                        })
                    }
                    0x9 => {
                        let d2 = l.dado(None)?;
                        Some(if d2 == 0 {
                            Mensagem::NotaDesligada {
                                canal,
                                nota: d1,
                                velocidade: 0,
                            }
                        } else {
                            Mensagem::NotaLigada {
                                canal,
                                nota: d1,
                                velocidade: d2,
                            }
                        })
                    }
                    0xB => {
                        let d2 = l.dado(None)?;
                        Some(Mensagem::Controle {
                            canal,
                            controle: d1,
                            valor: d2,
                        })
                    }
                    0xC => Some(Mensagem::Programa {
                        canal,
                        programa: d1,
                    }),
                    0xD => None,
                    // 0xA (aftertouch por nota) e 0xE (pitch bend): dois dados, ignorados.
                    _ => {
                        l.dado(None)?;
                        None
                    }
                };
                if let Some(m) = msg {
                    if brutos.len() >= EVENTOS_MAX {
                        return Err(ErroSmf::GrandeDemais("eventos"));
                    }
                    brutos.push((tick, m));
                }
            }
        }
    }
    *fim_tick = (*fim_tick).max(tick);
    Ok(())
}

// --- Estado do serviço --------------------------------------------------------------------------

/// Uma gravação na memória da sessão.
#[derive(Clone, Debug)]
pub struct Take {
    pub gravacao: Gravacao,
    /// Notas pareadas para a linha do tempo (calculadas uma vez).
    pub notas: Vec<NotaLinha>,
    /// Já salva (ou veio de um arquivo) e sem mudanças desde então.
    pub salvo: bool,
    pub caminho: Option<PathBuf>,
}

impl Take {
    fn nova(gravacao: Gravacao, salvo: bool, caminho: Option<PathBuf>) -> Self {
        let notas = gravacao.notas();
        Self {
            gravacao,
            notas,
            salvo,
            caminho,
        }
    }
}

#[derive(Debug)]
struct Captura {
    /// Instante (relógio comum) em que a gravação começa; eventos antes dele são ignorados
    /// (a contagem vem antes).
    t0_ns: u64,
    eventos: Vec<EventoGravado>,
    ligadas: Vec<(u8, u8)>,
    pedal: [bool; 16],
}

#[derive(Debug)]
struct Tocando {
    take: usize,
    inicio_ns: u64,
    pos0_ns: u64,
    proximo: usize,
    /// Notas ligadas (canal de saída, nota, instante agendado) ainda sem o "soltar" agendado.
    ligadas: Vec<(u8, u8, u64)>,
    pedal: bool,
    ultimo_ns: u64,
    fim_ns: Option<u64>,
}

#[derive(Debug)]
enum Repro {
    Parada,
    Tocando(Tocando),
    Pausada { take: usize, pos_ns: u64 },
}

/// Estado do serviço de gravação (F06).
#[derive(Debug)]
pub struct EstadoGravacao {
    pub pasta: Option<PathBuf>,
    pub takes: Vec<Take>,
    pub selecionada: Option<usize>,
    /// Contagem de um compasso (4 tempos) antes de gravar.
    pub contagem: bool,
    pub bpm_contagem: u16,
    pub nome_arquivo: String,
    pub caminho_importar: String,
    pub mensagem: Option<String>,
    captura: Option<Captura>,
    repro: Repro,
    contador: usize,
}

impl EstadoGravacao {
    pub fn novo(pasta: Option<PathBuf>) -> Self {
        Self {
            pasta,
            takes: Vec::new(),
            selecionada: None,
            contagem: false,
            bpm_contagem: 100,
            nome_arquivo: String::new(),
            caminho_importar: String::new(),
            mensagem: None,
            captura: None,
            repro: Repro::Parada,
            contador: 0,
        }
    }

    pub fn gravando(&self) -> bool {
        self.captura.is_some()
    }

    pub fn tocando(&self) -> bool {
        matches!(self.repro, Repro::Tocando(_))
    }

    pub fn pausada(&self) -> bool {
        matches!(self.repro, Repro::Pausada { .. })
    }

    /// Gravação em andamento com algum evento, ou gravação sem arquivo salvo.
    pub fn tem_dados_nao_salvos(&self) -> bool {
        self.captura.as_ref().is_some_and(|c| !c.eventos.is_empty())
            || self.takes.iter().any(|t| !t.salvo)
    }

    /// Nanossegundos que faltam para a gravação começar de fato (contagem), se for o caso.
    pub fn contagem_restante_ns(&self, agora_ns: u64) -> Option<u64> {
        let c = self.captura.as_ref()?;
        (agora_ns < c.t0_ns).then(|| c.t0_ns - agora_ns)
    }

    /// Duração gravada até agora.
    pub fn duracao_gravando_ns(&self, agora_ns: u64) -> u64 {
        self.captura
            .as_ref()
            .map_or(0, |c| agora_ns.saturating_sub(c.t0_ns))
    }

    /// Eventos já gravados na captura em andamento.
    pub fn eventos_gravando(&self) -> usize {
        self.captura.as_ref().map_or(0, |c| c.eventos.len())
    }

    /// Posição de reprodução (tocando ou pausada), em ns da gravação.
    pub fn posicao_ns(&self, agora_ns: u64) -> Option<u64> {
        match &self.repro {
            Repro::Tocando(t) => {
                let d = self.takes.get(t.take)?.gravacao.duracao_ns;
                Some((t.pos0_ns + agora_ns.saturating_sub(t.inicio_ns)).min(d))
            }
            Repro::Pausada { pos_ns, .. } => Some(*pos_ns),
            Repro::Parada => None,
        }
    }

    // --- Captura ---------------------------------------------------------------------------

    /// Evento do teclado físico. Só grava durante a gravação.
    pub fn evento(&mut self, ev: &EventoMidi) {
        if let Some(m) = Mensagem::de_evento(ev) {
            self.registrar(ev.timestamp_monotonico_ns(), m);
        }
    }

    /// Nota do mouse ou do teclado do PC (canal 1, velocidade fixa de simulação).
    pub fn nota_simulada(&mut self, ligar: bool, nota: NotaMidi, agora_ns: u64) {
        let (canal, nota) = (CANAL_PRINCIPAL, nota.numero());
        let msg = if ligar {
            Mensagem::NotaLigada {
                canal,
                nota,
                velocidade: VELOCIDADE_SIMULACAO,
            }
        } else {
            Mensagem::NotaDesligada {
                canal,
                nota,
                velocidade: 64,
            }
        };
        self.registrar(agora_ns, msg);
    }

    fn registrar(&mut self, ts_ns: u64, msg: Mensagem) {
        let Some(c) = self.captura.as_mut() else {
            return;
        };
        if ts_ns < c.t0_ns || c.eventos.len() >= EVENTOS_MAX {
            return;
        }
        match msg {
            Mensagem::NotaLigada { canal, nota, .. } => {
                if !c.ligadas.contains(&(canal, nota)) {
                    c.ligadas.push((canal, nota));
                }
            }
            Mensagem::NotaDesligada { canal, nota, .. } => {
                c.ligadas.retain(|x| *x != (canal, nota));
            }
            Mensagem::Controle {
                canal,
                controle: CC_SUSTAIN,
                valor,
            } => c.pedal[usize::from(canal & 0xF)] = valor >= 64,
            _ => {}
        }
        c.eventos.push(EventoGravado {
            t_ns: ts_ns - c.t0_ns,
            msg,
        });
    }

    /// Começa a gravar (com a contagem, se ligada).
    pub fn gravar(&mut self, som: &mut ServicoSom) {
        if self.captura.is_some() {
            return;
        }
        self.parar_reproducao(som);
        let agora = som.agora_ns();
        let t0_ns = if self.contagem {
            let bpm = u64::from(self.bpm_contagem.clamp(30, 240));
            let tempo = 60_000_000_000 / bpm;
            let primeiro = agora.saturating_add(ns(ATRASO_INICIO));
            for i in 0..4u64 {
                let t = primeiro.saturating_add(i * tempo);
                let nota = if i == 0 { CLIQUE_FORTE } else { CLIQUE_FRACO };
                agendar_clique(som, t, nota);
            }
            primeiro.saturating_add(4 * tempo)
        } else {
            agora
        };
        self.captura = Some(Captura {
            t0_ns,
            eventos: Vec::new(),
            ligadas: Vec::new(),
            pedal: [false; 16],
        });
        self.mensagem = None;
    }

    fn finalizar_captura(&mut self, agora_ns: u64) {
        let Some(c) = self.captura.take() else {
            return;
        };
        if agora_ns < c.t0_ns {
            self.mensagem = Some("Contagem cancelada.".to_owned());
            return;
        }
        let mut eventos = c.eventos;
        eventos.sort_by_key(|e| e.t_ns);
        let mut duracao = agora_ns - c.t0_ns;
        if let Some(u) = eventos.last() {
            duracao = duracao.max(u.t_ns);
        }
        for (canal, nota) in c.ligadas {
            eventos.push(EventoGravado {
                t_ns: duracao,
                msg: Mensagem::NotaDesligada {
                    canal,
                    nota,
                    velocidade: 0,
                },
            });
        }
        for (canal, baixo) in c.pedal.iter().enumerate() {
            if *baixo {
                eventos.push(EventoGravado {
                    t_ns: duracao,
                    msg: Mensagem::Controle {
                        canal: canal as u8,
                        controle: CC_SUSTAIN,
                        valor: 0,
                    },
                });
            }
        }
        let g = Gravacao {
            nome: String::new(),
            eventos,
            tempos: Vec::new(),
            duracao_ns: duracao,
        };
        if g.total_notas() == 0 {
            self.mensagem = Some("Nada foi gravado: nenhuma nota tocada.".to_owned());
            return;
        }
        self.contador += 1;
        let mut g = g;
        g.nome = format!("Gravação {}", self.contador);
        self.nome_arquivo = g.nome.clone();
        self.takes.push(Take::nova(g, false, None));
        self.selecionada = Some(self.takes.len() - 1);
        self.mensagem = Some("Gravação concluída. Salve como .mid para não perdê-la.".to_owned());
    }

    // --- Reprodução ------------------------------------------------------------------------

    /// Para a gravação (se gravando) e a reprodução (se tocando ou pausada).
    pub fn parar(&mut self, som: &mut ServicoSom) {
        let agora = som.agora_ns();
        self.finalizar_captura(agora);
        self.parar_reproducao(som);
    }

    /// Toca a gravação selecionada (ou retoma, se estava pausada).
    pub fn tocar(&mut self, som: &mut ServicoSom) {
        if self.captura.is_some() {
            self.mensagem = Some("Pare a gravação antes de tocar.".to_owned());
            return;
        }
        if self.tocando() {
            return;
        }
        let Some(i) = self.selecionada.filter(|i| *i < self.takes.len()) else {
            self.mensagem = Some("Grave ou abra uma gravação para tocar.".to_owned());
            return;
        };
        let g = &self.takes[i].gravacao;
        let pos0 = match self.repro {
            Repro::Pausada { take, pos_ns } if take == i => pos_ns,
            _ => 0,
        };
        let pos0 = if pos0 >= g.duracao_ns { 0 } else { pos0 };
        let proximo = g.eventos.partition_point(|e| e.t_ns < pos0);
        let inicio_ns = som.agora_ns().saturating_add(ns(ATRASO_INICIO));
        // Pedal já apertado nesse ponto do arquivo.
        let pedal = g.eventos[..proximo]
            .iter()
            .rev()
            .find_map(|e| match e.msg {
                Mensagem::Controle {
                    controle: CC_SUSTAIN,
                    valor,
                    ..
                } => Some(valor >= 64),
                _ => None,
            })
            .unwrap_or(false);
        let mut t = Tocando {
            take: i,
            inicio_ns,
            pos0_ns: pos0,
            proximo,
            ligadas: Vec::new(),
            pedal: false,
            ultimo_ns: inicio_ns,
            fim_ns: None,
        };
        if pedal {
            enviar_mensagem(
                som,
                inicio_ns,
                Mensagem::Controle {
                    canal: CANAL_DEMONSTRACAO,
                    controle: CC_SUSTAIN,
                    valor: 127,
                },
                &mut t,
            );
        }
        self.repro = Repro::Tocando(t);
        self.mensagem = None;
        self.atualizar(som);
    }

    /// Pausa (se tocando) ou retoma (se pausada). Pausar solta as notas que estão soando.
    pub fn pausar(&mut self, som: &mut ServicoSom) {
        match std::mem::replace(&mut self.repro, Repro::Parada) {
            Repro::Tocando(mut t) => {
                let agora = som.agora_ns();
                let dur = self.takes.get(t.take).map_or(0, |x| x.gravacao.duracao_ns);
                let pos = (t.pos0_ns + agora.saturating_sub(t.inicio_ns)).min(dur);
                cortar_em(som, &mut t, agora);
                self.repro = Repro::Pausada {
                    take: t.take,
                    pos_ns: pos,
                };
            }
            outro @ Repro::Pausada { .. } => {
                self.repro = outro;
                self.tocar(som);
            }
            Repro::Parada => {}
        }
    }

    /// Para só a reprodução, soltando as notas que estão soando.
    pub fn parar_reproducao(&mut self, som: &mut ServicoSom) {
        if let Repro::Tocando(mut t) = std::mem::replace(&mut self.repro, Repro::Parada) {
            let agora = som.agora_ns();
            cortar_em(som, &mut t, agora);
        }
    }

    /// Posiciona a reprodução (só com a reprodução parada ou pausada).
    pub fn buscar(&mut self, ns: u64) {
        if self.tocando() || self.captura.is_some() {
            return;
        }
        let Some(i) = self.selecionada.filter(|i| *i < self.takes.len()) else {
            return;
        };
        let d = self.takes[i].gravacao.duracao_ns;
        self.repro = Repro::Pausada {
            take: i,
            pos_ns: ns.min(d),
        };
    }

    /// Chamar a cada quadro: agenda a reprodução até `agora + ANTECEDENCIA` e a encerra no fim.
    pub fn atualizar(&mut self, som: &mut ServicoSom) {
        let Self { repro, takes, .. } = self;
        let Repro::Tocando(t) = repro else {
            return;
        };
        let Some(take) = takes.get(t.take) else {
            *repro = Repro::Parada;
            return;
        };
        if avancar(t, &take.gravacao, som) {
            *repro = Repro::Parada;
        }
    }

    // --- Takes e arquivos ------------------------------------------------------------------

    /// Seleciona uma gravação (e sugere o nome do arquivo).
    pub fn selecionar(&mut self, i: usize, som: &mut ServicoSom) {
        if i >= self.takes.len() || self.selecionada == Some(i) {
            return;
        }
        self.parar_reproducao(som);
        self.repro = Repro::Parada;
        self.selecionada = Some(i);
        self.nome_arquivo = self.takes[i].gravacao.nome.clone();
    }

    /// Descarta uma gravação da memória (para a reprodução antes).
    pub fn remover(&mut self, i: usize, som: &mut ServicoSom) {
        if i >= self.takes.len() {
            return;
        }
        let toca_esta = match &self.repro {
            Repro::Tocando(t) => t.take == i,
            Repro::Pausada { take, .. } => *take == i,
            Repro::Parada => false,
        };
        if toca_esta {
            self.parar_reproducao(som);
            self.repro = Repro::Parada;
        }
        self.takes.remove(i);
        // Índices de reprodução acima de `i` deslocam uma posição.
        match &mut self.repro {
            Repro::Tocando(t) if t.take > i => t.take -= 1,
            Repro::Pausada { take, .. } if *take > i => *take -= 1,
            _ => {}
        }
        self.selecionada = match self.selecionada {
            Some(s) if s == i => None,
            Some(s) if s > i => Some(s - 1),
            outro => outro,
        };
    }

    fn pasta_gravacoes(&self) -> Option<PathBuf> {
        self.pasta.as_ref().map(|p| p.join("gravacoes"))
    }

    /// Arquivos `.mid` salvos (nome sem extensão).
    pub fn salvos(&self) -> Vec<String> {
        let Some(p) = self.pasta_gravacoes() else {
            return Vec::new();
        };
        let mut v: Vec<String> = std::fs::read_dir(p)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.strip_suffix(".mid").map(str::to_owned)
            })
            .collect();
        v.sort();
        v
    }

    /// Salva a gravação `i` como `.mid` com o nome de `nome_arquivo`.
    pub fn salvar(&mut self, i: usize) -> Option<PathBuf> {
        let Some(take) = self.takes.get_mut(i) else {
            self.mensagem = Some("Nenhuma gravação selecionada.".to_owned());
            return None;
        };
        let nome = nome_seguro(&self.nome_arquivo);
        if nome.is_empty() {
            self.mensagem = Some("Dê um nome à gravação antes de salvar.".to_owned());
            return None;
        }
        let Some(pasta) = self.pasta.as_ref().map(|p| p.join("gravacoes")) else {
            self.mensagem = Some("Sem pasta de dados (HOME não definido).".to_owned());
            return None;
        };
        let bytes = match take.gravacao.para_smf() {
            Ok(b) => b,
            Err(e) => {
                self.mensagem = Some(format!("Não foi possível gerar o .mid: {e}"));
                return None;
            }
        };
        let caminho = pasta.join(format!("{nome}.mid"));
        let r = std::fs::create_dir_all(&pasta).and_then(|()| std::fs::write(&caminho, bytes));
        match r {
            Ok(()) => {
                take.gravacao.nome = self.nome_arquivo.trim().to_owned();
                take.salvo = true;
                take.caminho = Some(caminho.clone());
                self.mensagem = Some(format!("Salvo em {}", caminho.display()));
                Some(caminho)
            }
            Err(e) => {
                self.mensagem = Some(format!("Não foi possível salvar: {e}"));
                None
            }
        }
    }

    /// Salvamento temporário de recuperação: grava as gravações ainda não salvas em
    /// `gravacoes/recuperacao/`, sem marcá-las como salvas. Devolve quantas foram escritas.
    pub fn salvar_recuperacao(&mut self) -> usize {
        let Some(pasta) = self.pasta_gravacoes().map(|p| p.join("recuperacao")) else {
            return 0;
        };
        let mut n = 0;
        for (i, t) in self.takes.iter().enumerate().filter(|(_, t)| !t.salvo) {
            let Ok(bytes) = t.gravacao.para_smf() else {
                continue;
            };
            let nome = format!("{}-{}.mid", i + 1, nome_seguro(&t.gravacao.nome));
            let ok = std::fs::create_dir_all(&pasta)
                .and_then(|()| std::fs::write(pasta.join(nome), bytes))
                .is_ok();
            n += usize::from(ok);
        }
        self.mensagem = Some(if n > 0 {
            format!("Rascunho de recuperação salvo ({n}).")
        } else {
            "Nada para recuperar ou sem pasta de dados.".to_owned()
        });
        n
    }

    /// Abre um `.mid` salvo na pasta de gravações.
    pub fn abrir_salvo(&mut self, nome: &str) {
        if nome.is_empty() || nome.contains(['/', '\\']) || nome.contains("..") {
            self.mensagem = Some("Nome de arquivo inválido.".to_owned());
            return;
        }
        let Some(p) = self.pasta_gravacoes() else {
            return;
        };
        self.importar_arquivo(&p.join(format!("{nome}.mid")));
    }

    /// Importa o arquivo cujo caminho foi digitado.
    pub fn importar_digitado(&mut self) {
        let t = self
            .caminho_importar
            .trim()
            .trim_matches(['"', '\''])
            .trim();
        if t.is_empty() {
            self.mensagem = Some("Digite o caminho de um arquivo .mid.".to_owned());
            return;
        }
        let caminho = PathBuf::from(t);
        self.importar_arquivo(&caminho);
    }

    /// Lê um `.mid` (tipo 0 ou 1) e o põe na lista. Erros viram mensagem; nunca derruba o app.
    pub fn importar_arquivo(&mut self, caminho: &Path) {
        let r = ler_arquivo(caminho);
        match r {
            Ok(g) => {
                let nome = g.nome.clone();
                self.takes
                    .push(Take::nova(g, true, Some(caminho.to_path_buf())));
                self.selecionada = Some(self.takes.len() - 1);
                self.nome_arquivo = nome.clone();
                self.mensagem = Some(format!("Importado: {nome}"));
            }
            Err(e) => self.mensagem = Some(format!("Não foi possível importar: {e}")),
        }
    }
}

fn ler_arquivo(caminho: &Path) -> Result<Gravacao, String> {
    let meta = std::fs::metadata(caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
    if !meta.is_file() {
        return Err(format!("{} não é um arquivo.", caminho.display()));
    }
    if meta.len() > TAMANHO_MAX_ARQUIVO {
        return Err("arquivo grande demais (máx. 4 MiB)".to_owned());
    }
    let bytes = std::fs::read(caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
    let nome = caminho.file_stem().map_or_else(
        || "Importado".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    Gravacao::de_smf(&bytes, &nome).map_err(|e| e.to_string())
}

// --- Envio ao motor ----------------------------------------------------------------------------

fn canal_saida(canal: u8) -> u8 {
    if canal == CANAL_PERCUSSAO {
        CANAL_PERCUSSAO
    } else {
        CANAL_DEMONSTRACAO
    }
}

fn agendar_clique(som: &mut ServicoSom, t: u64, nota: u8) {
    let (Some(canal), Some(nota)) = (CanalMidi::novo(CANAL_PERCUSSAO), NotaMidi::nova(nota)) else {
        return;
    };
    som.agendar(
        t,
        Acao::NotaLigada {
            canal,
            nota,
            velocidade: 100,
        },
    );
    som.agendar(
        t.saturating_add(DURACAO_CLIQUE_NS),
        Acao::NotaDesligada { canal, nota },
    );
}

fn agendar_nota(som: &mut ServicoSom, t: u64, ligar: bool, canal: u8, nota: u8, vel: u8) {
    let (Some(canal), Some(nota)) = (CanalMidi::novo(canal), NotaMidi::nova(nota)) else {
        return;
    };
    let acao = if ligar {
        Acao::NotaLigada {
            canal,
            nota,
            velocidade: vel.clamp(1, 127),
        }
    } else {
        Acao::NotaDesligada { canal, nota }
    };
    som.agendar(t, acao);
}

fn agendar_pedal(som: &mut ServicoSom, t: u64, valor: u8) {
    if let Some(canal) = CanalMidi::novo(CANAL_DEMONSTRACAO) {
        som.agendar(
            t,
            Acao::Controle {
                canal,
                controle: CC_SUSTAIN,
                valor,
            },
        );
    }
}

/// Agenda uma mensagem da gravação e atualiza o que está soando.
fn enviar_mensagem(som: &mut ServicoSom, inst: u64, msg: Mensagem, t: &mut Tocando) {
    t.ultimo_ns = t.ultimo_ns.max(inst);
    match msg {
        Mensagem::NotaLigada {
            canal,
            nota,
            velocidade,
        } => {
            let c = canal_saida(canal);
            agendar_nota(som, inst, true, c, nota, velocidade);
            t.ligadas.push((c, nota, inst));
        }
        Mensagem::NotaDesligada { canal, nota, .. } => {
            let c = canal_saida(canal);
            agendar_nota(som, inst, false, c, nota, 0);
            if let Some(p) = t.ligadas.iter().position(|x| x.0 == c && x.1 == nota) {
                t.ligadas.remove(p);
            }
        }
        Mensagem::Controle {
            controle: CC_SUSTAIN,
            valor,
            ..
        } => {
            agendar_pedal(som, inst, valor);
            t.pedal = valor >= 64;
        }
        Mensagem::Controle { .. } | Mensagem::Programa { .. } => {}
    }
}

/// Solta o que está soando (e o pedal) a partir de `base_ns`: cada nota solta em `base`, ou
/// logo depois do seu "ligar" se este ainda está na fila; o pedal depois de tudo.
fn cortar_em(som: &mut ServicoSom, t: &mut Tocando, base_ns: u64) {
    let mut limite = base_ns;
    for (c, nota, on) in std::mem::take(&mut t.ligadas) {
        let inst = base_ns.max(on.saturating_add(NS_POR_MS));
        limite = limite.max(inst);
        agendar_nota(som, inst, false, c, nota, 0);
    }
    if t.pedal {
        let inst = limite.max(t.ultimo_ns.saturating_add(NS_POR_MS));
        agendar_pedal(som, inst, 0);
        t.pedal = false;
    }
}

/// Agenda o que cabe na janela; `true` quando a reprodução terminou.
fn avancar(t: &mut Tocando, g: &Gravacao, som: &mut ServicoSom) -> bool {
    let agora = som.agora_ns();
    let horizonte = agora.saturating_add(ns(ANTECEDENCIA));
    while let Some(e) = g.eventos.get(t.proximo) {
        let inst = t.inicio_ns.saturating_add(e.t_ns.saturating_sub(t.pos0_ns));
        if inst > horizonte {
            break;
        }
        enviar_mensagem(som, inst, e.msg, t);
        t.proximo += 1;
    }
    if t.proximo >= g.eventos.len() && t.fim_ns.is_none() {
        let fim = t
            .inicio_ns
            .saturating_add(g.duracao_ns.saturating_sub(t.pos0_ns));
        // Notas que o arquivo deixou abertas soltam no fim.
        cortar_em(som, t, fim);
        t.fim_ns = Some(fim);
    }
    t.fim_ns.is_some_and(|f| agora >= f)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::som::apoio_testes::{BLOCO, servico_pronto};
    use piano_tutor_audio::RelogioManual;
    use piano_tutor_audio::sintetizador::RegistroFalso;
    use piano_tutor_core::tempo::Relogio;
    use std::sync::Arc;

    const MS: u64 = 1_000_000;
    /// Um tick a 480 PPQ e 120 bpm, em ns (arredondado para cima).
    const TICK_NS: u64 = 1_041_667;

    fn ev(t_ms: u64, msg: Mensagem) -> EventoGravado {
        EventoGravado {
            t_ns: t_ms * MS,
            msg,
        }
    }

    fn lig(canal: u8, nota: u8, velocidade: u8) -> Mensagem {
        Mensagem::NotaLigada {
            canal,
            nota,
            velocidade,
        }
    }

    fn des(canal: u8, nota: u8) -> Mensagem {
        Mensagem::NotaDesligada {
            canal,
            nota,
            velocidade: 40,
        }
    }

    fn des_v(canal: u8, nota: u8, velocidade: u8) -> Mensagem {
        Mensagem::NotaDesligada {
            canal,
            nota,
            velocidade,
        }
    }

    fn pedal(canal: u8, valor: u8) -> Mensagem {
        Mensagem::Controle {
            canal,
            controle: 64,
            valor,
        }
    }

    /// Três notas, um acorde, uma pausa e pedal, em dois canais.
    fn sequencia() -> Gravacao {
        Gravacao {
            nome: "teste".to_owned(),
            eventos: vec![
                ev(
                    0,
                    Mensagem::Programa {
                        canal: 0,
                        programa: 0,
                    },
                ),
                ev(0, lig(0, 60, 80)),
                ev(400, des(0, 60)),
                ev(500, lig(0, 62, 90)),
                ev(900, des(0, 62)),
                ev(1000, lig(0, 64, 100)),
                ev(1400, des(0, 64)),
                // Pausa de 600 ms, depois o acorde com pedal.
                ev(2000, pedal(0, 127)),
                ev(2000, lig(0, 60, 70)),
                ev(2000, lig(0, 64, 71)),
                ev(2000, lig(1, 67, 72)),
                ev(2500, des(0, 60)),
                ev(2500, des(0, 64)),
                ev(2500, des(1, 67)),
                ev(3000, pedal(0, 0)),
            ],
            tempos: Vec::new(),
            duracao_ns: 3200 * MS,
        }
    }

    fn mesmas_mensagens(a: &Gravacao, b: &Gravacao) {
        let ma: Vec<Mensagem> = a.eventos.iter().map(|e| e.msg).collect();
        let mb: Vec<Mensagem> = b.eventos.iter().map(|e| e.msg).collect();
        assert_eq!(ma, mb, "mensagens e ordem preservadas");
        for (x, y) in a.eventos.iter().zip(&b.eventos) {
            let d = x.t_ns.abs_diff(y.t_ns);
            assert!(d <= TICK_NS, "{:?} vs {:?} (dif {d} ns)", x, y);
        }
    }

    // --- Arquivo -------------------------------------------------------------------------

    #[test]
    fn ida_e_volta_preserva_notas_velocidade_canal_ordem_e_pedal() {
        let g = sequencia();
        let bytes = g.para_smf().expect("escreve");
        assert_eq!(&bytes[..4], b"MThd");
        assert_eq!(&bytes[8..10], &[0, 0], "tipo 0");
        assert_eq!(&bytes[12..14], &PPQ.to_be_bytes());
        let lida = Gravacao::de_smf(&bytes, "teste").expect("lê");
        mesmas_mensagens(&g, &lida);
        assert!(lida.duracao_ns.abs_diff(g.duracao_ns) <= TICK_NS);
        // Segunda volta: estável.
        let de_novo = Gravacao::de_smf(&lida.para_smf().expect("escreve"), "teste").expect("lê");
        assert_eq!(de_novo.eventos, lida.eventos);
        // O pedal ficou como CC64 no canal 1.
        assert!(lida.eventos.iter().any(|e| e.msg == pedal(0, 127)));
        assert!(lida.eventos.iter().any(|e| e.msg == pedal(0, 0)));
    }

    #[test]
    fn andamento_padrao_e_120_bpm_e_mudanca_de_andamento_sobrevive() {
        let mut g = sequencia();
        let bytes = g.para_smf().expect("escreve");
        // Meta de andamento 500000 µs logo no início da trilha.
        assert_eq!(&bytes[23..29], &[0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20]);
        assert!(Gravacao::de_smf(&bytes, "x").expect("lê").tempos.is_empty());
        // Muda para 240 bpm (250000 µs) em 1 s.
        g.tempos = vec![(1000 * MS, 250_000)];
        let lida = Gravacao::de_smf(&g.para_smf().expect("escreve"), "x").expect("lê");
        assert_eq!(lida.tempos.len(), 1);
        assert_eq!(lida.tempos[0].1, 250_000);
        assert!(lida.tempos[0].0.abs_diff(1000 * MS) <= TICK_NS);
        mesmas_mensagens(&g, &lida);
    }

    // --- Arquivos de referência montados à mão --------------------------------------------

    fn cabecalho(tipo: u16, trilhas: u16, ppq: u16) -> Vec<u8> {
        let mut v = b"MThd".to_vec();
        v.extend_from_slice(&6u32.to_be_bytes());
        v.extend_from_slice(&tipo.to_be_bytes());
        v.extend_from_slice(&trilhas.to_be_bytes());
        v.extend_from_slice(&ppq.to_be_bytes());
        v
    }

    fn trilha(corpo: &[u8]) -> Vec<u8> {
        let mut v = b"MTrk".to_vec();
        v.extend_from_slice(&(corpo.len() as u32).to_be_bytes());
        v.extend_from_slice(corpo);
        v
    }

    #[test]
    fn tipo_1_junta_trilhas_e_respeita_mudanca_de_andamento() {
        let mut f = cabecalho(1, 3, 480);
        // Trilha de andamento: 120 bpm no 0; 240 bpm (250000) em 480 ticks.
        f.extend(trilha(&[
            0x00, 0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20, // 500000
            0x83, 0x60, 0xFF, 0x51, 0x03, 0x03, 0xD0, 0x90, // delta 480; 250000
            0x00, 0xFF, 0x2F, 0x00,
        ]));
        // Canal 1: C4 de 0 a 480; E4 de 480 a 960.
        f.extend(trilha(&[
            0x00, 0x90, 60, 100, 0x83, 0x60, 0x80, 60, 0x40, 0x00, 0x90, 64, 90, 0x83, 0x60, 0x80,
            64, 0x40, 0x00, 0xFF, 0x2F, 0x00,
        ]));
        // Canal 3: G4 com running status e note-on velocidade 0 como soltar, em 480.
        f.extend(trilha(&[
            0x83, 0x60, 0x92, 67, 80, 0x83, 0x60, 67, 0, 0x00, 0xFF, 0x2F, 0x00,
        ]));
        let g = Gravacao::de_smf(&f, "t1").expect("lê");
        let m: Vec<(u64, Mensagem)> = g.eventos.iter().map(|e| (e.t_ns, e.msg)).collect();
        // Ticks 0 e 480 a 120 bpm = 0 e 500 ms; depois 240 bpm: 480 ticks = 250 ms.
        assert_eq!(
            m,
            vec![
                (0, lig(0, 60, 100)),
                (
                    500 * MS,
                    Mensagem::NotaDesligada {
                        canal: 0,
                        nota: 60,
                        velocidade: 64
                    }
                ),
                (500 * MS, lig(0, 64, 90)),
                (500 * MS, lig(2, 67, 80)),
                (
                    750 * MS,
                    Mensagem::NotaDesligada {
                        canal: 0,
                        nota: 64,
                        velocidade: 64
                    }
                ),
                (
                    750 * MS,
                    Mensagem::NotaDesligada {
                        canal: 2,
                        nota: 67,
                        velocidade: 0
                    }
                ),
            ]
        );
        assert_eq!(g.tempos, vec![(500 * MS, 250_000)]);
        assert_eq!(g.duracao_ns, 750 * MS);
        // Reescrever e ler de novo mantém tudo dentro de 1 tick.
        let de_novo = Gravacao::de_smf(&g.para_smf().expect("escreve"), "t1").expect("lê");
        mesmas_mensagens(&g, &de_novo);
        assert_eq!(de_novo.tempos.len(), 1);
    }

    #[test]
    fn leitor_ignora_meta_sysex_e_mensagens_fora_do_escopo_e_corta_no_fim_da_trilha() {
        let mut f = cabecalho(0, 1, 96);
        f.extend(trilha(&[
            0x00, 0xFF, 0x03, 0x02, b'o', b'k', // nome da trilha
            0x00, 0xF0, 0x03, 0x7E, 0x01, 0xF7, // SysEx
            0x00, 0xE0, 0x00, 0x40, // pitch bend
            0x00, 0xD0, 0x20, // pressão de canal
            0x00, 0xA0, 60, 30, // aftertouch por nota
            0x00, 0xC5, 19, // programa
            0x60, 0x95, 72, 64, // 96 ticks = 1 semínima (500 ms)
            0x00, 0xFF, 0x2F, 0x00, // fim
            0x00, 0x90, 1, 1, // depois do fim: ignorado
        ]));
        let g = Gravacao::de_smf(&f, "x").expect("lê");
        assert_eq!(g.eventos.len(), 2);
        assert_eq!(
            g.eventos[0].msg,
            Mensagem::Programa {
                canal: 5,
                programa: 19
            }
        );
        assert_eq!(g.eventos[1], ev(500, lig(5, 72, 64)));
    }

    // --- Malformados -----------------------------------------------------------------------

    #[test]
    fn arquivos_malformados_devolvem_erro_sem_panico() {
        let valido = sequencia().para_smf().expect("escreve");
        // Vazio, curto e cabeçalho errado.
        assert!(Gravacao::de_smf(&[], "x").is_err());
        assert!(Gravacao::de_smf(b"MThd", "x").is_err());
        let mut errado = valido.clone();
        errado[0] = b'X';
        assert_eq!(
            Gravacao::de_smf(&errado, "x"),
            Err(ErroSmf::CabecalhoInvalido)
        );
        // Tamanho do cabeçalho gigante.
        let mut gigante = valido.clone();
        gigante[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(Gravacao::de_smf(&gigante, "x").is_err());
        // Tipo 2 e SMPTE.
        assert_eq!(
            Gravacao::de_smf(&cabecalho(2, 1, 480), "x"),
            Err(ErroSmf::FormatoNaoSuportado)
        );
        assert_eq!(
            Gravacao::de_smf(&cabecalho(0, 1, 0xE728), "x"),
            Err(ErroSmf::DivisaoNaoSuportada)
        );
        assert_eq!(
            Gravacao::de_smf(&cabecalho(0, 1, 0), "x"),
            Err(ErroSmf::DivisaoNaoSuportada)
        );
        // Só cabeçalho: sem trilhas.
        assert_eq!(
            Gravacao::de_smf(&cabecalho(0, 1, 480), "x"),
            Err(ErroSmf::SemTrilhas)
        );
        // Trilha com tamanho declarado gigante (4 GiB) e o arquivo curto.
        let mut f = cabecalho(0, 1, 480);
        f.extend_from_slice(b"MTrk");
        f.extend_from_slice(&u32::MAX.to_be_bytes());
        f.extend_from_slice(&[0, 0xFF, 0x2F, 0]);
        assert_eq!(Gravacao::de_smf(&f, "x"), Err(ErroSmf::Truncado));
        // Evento truncado, VLQ de 5 bytes, dado >127, status de sistema, sem status inicial.
        for corpo in [
            &[0x00, 0x90, 60][..],
            &[0x80, 0x80, 0x80, 0x80, 0x00, 0x90, 60, 64],
            &[0x00, 0x90, 200, 64],
            &[0x00, 0xF2, 0x00, 0x00],
            &[0x00, 60, 64],
            &[0x00, 0xFF, 0x51, 0x03, 0, 0, 0],
            &[0x00, 0xFF, 0x51, 0x02, 1, 1],
            &[0x00, 0xFF, 0x01, 0x7F],
            &[0x00, 0xF0, 0xFF, 0xFF, 0xFF, 0x7F, 1],
        ] {
            let mut f = cabecalho(0, 1, 480);
            f.extend(trilha(corpo));
            assert!(Gravacao::de_smf(&f, "x").is_err(), "{corpo:02X?}");
        }
        // Delta enorme acumulado: erro de duração, não conta estourada.
        let mut f = cabecalho(0, 1, 1);
        let mut corpo = Vec::new();
        for _ in 0..5000 {
            corpo.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0x7F, 0x90, 60, 64]);
        }
        f.extend(trilha(&corpo));
        assert!(Gravacao::de_smf(&f, "x").is_err());
    }

    #[test]
    fn arquivo_acima_do_limite_e_recusado_e_cortes_e_mutacoes_nao_entram_em_panico() {
        let enorme = vec![0u8; TAMANHO_MAX_ARQUIVO as usize + 1];
        assert_eq!(
            Gravacao::de_smf(&enorme, "x"),
            Err(ErroSmf::GrandeDemais("arquivo"))
        );
        let valido = sequencia().para_smf().expect("escreve");
        for n in 0..valido.len() {
            let _ = Gravacao::de_smf(&valido[..n], "x");
        }
        let mut semente = 0x1234_5678_u32;
        for _ in 0..3000 {
            let mut m = valido.clone();
            for _ in 0..3 {
                semente = semente.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let i = (semente >> 8) as usize % m.len();
                semente = semente.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                m[i] = (semente >> 16) as u8;
            }
            let _ = Gravacao::de_smf(&m, "x");
        }
    }

    #[test]
    fn muitos_eventos_sao_limitados() {
        let mut corpo = Vec::new();
        for _ in 0..(EVENTOS_MAX + 10) {
            corpo.extend_from_slice(&[0x00, 0xB0, 1, 1]);
        }
        corpo.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);
        let mut f = cabecalho(0, 1, 480);
        f.extend(trilha(&corpo));
        // Dentro de 4 MiB (2 MB), mas passa do limite de eventos.
        assert_eq!(
            Gravacao::de_smf(&f, "x"),
            Err(ErroSmf::GrandeDemais("eventos"))
        );
    }

    // --- Gravação --------------------------------------------------------------------------

    fn canal(c: u8) -> CanalMidi {
        CanalMidi::novo(c).expect("canal")
    }

    fn nota(n: u8) -> NotaMidi {
        NotaMidi::nova(n).expect("nota")
    }

    fn lig_ev(c: u8, n: u8, v: u8, t_ms: u64) -> EventoMidi {
        EventoMidi::NotaLigada {
            canal: canal(c),
            nota: nota(n),
            velocidade: v,
            timestamp_monotonico_ns: t_ms * MS,
        }
    }

    fn des_ev(c: u8, n: u8, t_ms: u64) -> EventoMidi {
        EventoMidi::NotaDesligada {
            canal: canal(c),
            nota: nota(n),
            velocidade: 33,
            timestamp_monotonico_ns: t_ms * MS,
        }
    }

    fn cc_ev(c: u8, cc: u8, valor: u8, t_ms: u64) -> EventoMidi {
        EventoMidi::ControleMudou {
            canal: canal(c),
            controle: cc,
            valor,
            timestamp_monotonico_ns: t_ms * MS,
        }
    }

    fn par() -> (EstadoGravacao, crate::som::ServicoSom, RelogioManual) {
        let relogio = RelogioManual::novo();
        let (som, _motor) = servico_pronto(Arc::new(relogio.clone()) as Arc<dyn Relogio>);
        (EstadoGravacao::novo(None), som, relogio)
    }

    #[test]
    fn grava_so_quando_gravando_e_preserva_tudo_com_tempo_relativo() {
        let (mut g, mut som, relogio) = par();
        g.evento(&lig_ev(0, 50, 90, 0));
        assert!(!g.tem_dados_nao_salvos(), "fora da gravação não grava");
        relogio.avancar(Duration::from_millis(1000));
        g.gravar(&mut som);
        assert!(g.gravando() && !g.tocando());
        g.evento(&lig_ev(0, 60, 80, 1100));
        g.nota_simulada(true, nota(64), 1200 * MS);
        g.evento(&cc_ev(0, 64, 127, 1300));
        g.nota_simulada(false, nota(64), 1500 * MS);
        g.evento(&des_ev(0, 60, 1600));
        g.evento(&EventoMidi::Desconhecido {
            timestamp_monotonico_ns: 1650 * MS,
        });
        // Fica uma nota e o pedal presos: o fim solta ambos.
        g.evento(&lig_ev(2, 72, 55, 1700));
        assert!(g.tem_dados_nao_salvos());
        relogio.avancar(Duration::from_millis(1000));
        g.parar(&mut som);
        assert!(!g.gravando());
        assert_eq!(g.takes.len(), 1);
        let t = &g.takes[0];
        assert!(!t.salvo);
        let e = &t.gravacao.eventos;
        assert_eq!(e[0], ev(100, lig(0, 60, 80)));
        assert_eq!(e[1], ev(200, lig(0, 64, VELOCIDADE_SIMULACAO)));
        assert_eq!(e[2], ev(300, pedal(0, 127)));
        assert_eq!(e[3].t_ns, 500 * MS);
        assert_eq!(e[4], ev(600, des_v(0, 60, 33)));
        assert_eq!(e[5], ev(700, lig(2, 72, 55)));
        // Soltas no fim: duração = 2000 - 1000 = 1000 ms.
        assert_eq!(t.gravacao.duracao_ns, 1000 * MS);
        assert!(e.contains(&ev(1000, des_v(2, 72, 0))));
        assert!(e.contains(&ev(1000, pedal(0, 0))));
        assert_eq!(t.notas.len(), 3);
        let total = e.len();
        // Sem gravar, `evento` não muda nada.
        g.evento(&lig_ev(0, 60, 80, 3000));
        assert_eq!(g.takes[0].gravacao.eventos.len(), total);
    }

    #[test]
    fn gravacao_vazia_nao_vira_take() {
        let (mut g, mut som, relogio) = par();
        g.gravar(&mut som);
        relogio.avancar(Duration::from_millis(500));
        g.parar(&mut som);
        assert!(g.takes.is_empty());
        assert!(!g.tem_dados_nao_salvos());
        assert!(g.mensagem.as_deref().unwrap_or("").contains("Nada"));
    }

    #[test]
    fn contagem_agenda_quatro_cliques_e_ignora_toques_antes_do_inicio() {
        let relogio = RelogioManual::novo();
        let (mut som, motor) = servico_pronto(Arc::new(relogio.clone()) as Arc<dyn Relogio>);
        motor.renderizar(BLOCO, 0);
        let mut g = EstadoGravacao::novo(None);
        g.contagem = true;
        g.bpm_contagem = 120;
        g.gravar(&mut som);
        // Primeiro clique em 100 ms; gravação começa em 100 ms + 4 x 500 ms = 2100 ms.
        assert_eq!(g.contagem_restante_ns(0), Some(2100 * MS));
        g.evento(&lig_ev(0, 60, 80, 1000));
        g.evento(&lig_ev(0, 62, 80, 2200));
        relogio.avancar(Duration::from_millis(3000));
        g.parar(&mut som);
        assert_eq!(g.takes.len(), 1);
        assert_eq!(g.takes[0].gravacao.eventos.len(), 2, "nota + solta no fim");
        assert_eq!(g.takes[0].gravacao.eventos[0].t_ns, 100 * MS);
        motor.renderizar(48_000 * 3, 0);
        let cliques: Vec<(u8, u8)> = motor
            .registro()
            .iter()
            .filter_map(|e| match *e {
                RegistroFalso::Ligada { canal, nota, .. } => Some((canal, nota)),
                _ => None,
            })
            .collect();
        assert_eq!(
            cliques,
            vec![
                (CANAL_PERCUSSAO, CLIQUE_FORTE),
                (CANAL_PERCUSSAO, CLIQUE_FRACO),
                (CANAL_PERCUSSAO, CLIQUE_FRACO),
                (CANAL_PERCUSSAO, CLIQUE_FRACO),
            ]
        );
    }

    #[test]
    fn cancelar_durante_a_contagem_nao_cria_gravacao() {
        let (mut g, mut som, _r) = par();
        g.contagem = true;
        g.gravar(&mut som);
        g.parar(&mut som);
        assert!(g.takes.is_empty() && !g.gravando());
    }

    // --- Reprodução ------------------------------------------------------------------------

    fn simples() -> Gravacao {
        Gravacao {
            nome: "simples".to_owned(),
            eventos: vec![
                ev(0, lig(0, 60, 80)),
                ev(100, lig(0, 64, 90)),
                ev(300, des(0, 60)),
                ev(500, des(0, 64)),
            ],
            tempos: Vec::new(),
            duracao_ns: 600 * MS,
        }
    }

    fn com_take(g: Gravacao) -> (EstadoGravacao, ServicoSomPar) {
        let relogio = RelogioManual::novo();
        let (som, motor) = servico_pronto(Arc::new(relogio.clone()) as Arc<dyn Relogio>);
        let mut e = EstadoGravacao::novo(None);
        e.takes.push(Take::nova(g, false, None));
        e.selecionada = Some(0);
        (
            e,
            ServicoSomPar {
                som,
                motor,
                relogio,
            },
        )
    }

    struct ServicoSomPar {
        som: ServicoSom,
        motor: crate::som::apoio_testes::MotorDeTeste,
        relogio: RelogioManual,
    }

    fn rodar(e: &mut EstadoGravacao, p: &mut ServicoSomPar, ms: u64) {
        for _ in 0..(ms / 16) {
            p.relogio.avancar(Duration::from_millis(16));
            e.atualizar(&mut p.som);
        }
    }

    fn eventos_de_nota(motor: &crate::som::apoio_testes::MotorDeTeste) -> Vec<(bool, u8, u8, u64)> {
        motor
            .registro()
            .iter()
            .filter_map(|e| match *e {
                RegistroFalso::Ligada {
                    frame, canal, nota, ..
                } => Some((true, canal, nota, frame)),
                RegistroFalso::Desligada {
                    frame, canal, nota, ..
                } => Some((false, canal, nota, frame)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn reproducao_agenda_no_canal_de_demonstracao_com_tempos_relativos() {
        let (mut e, mut p) = com_take(simples());
        p.motor.renderizar(BLOCO, 0);
        e.tocar(&mut p.som);
        assert!(e.tocando());
        rodar(&mut e, &mut p, 1200);
        assert!(!e.tocando(), "termina sozinha");
        p.motor.renderizar(48_000 * 2, 0);
        let v = eventos_de_nota(&p.motor);
        assert_eq!(v.len(), 4, "{v:?}");
        assert!(v.iter().all(|(_, c, _, _)| *c == CANAL_DEMONSTRACAO));
        let ordem: Vec<(bool, u8)> = v.iter().map(|(l, _, n, _)| (*l, *n)).collect();
        assert_eq!(
            ordem,
            vec![(true, 60), (true, 64), (false, 60), (false, 64)]
        );
        // Diferenças em frames (48 kHz) iguais às em ms, com folga de dois blocos.
        let frames_por_ms = 48.0;
        for (i, esperado_ms) in [(1usize, 100.0), (2, 300.0), (3, 500.0)] {
            let d = (v[i].3 - v[0].3) as f64;
            assert!(
                (d - esperado_ms * frames_por_ms).abs() <= 2.0 * BLOCO as f64,
                "evento {i}: {d} frames"
            );
        }
        assert_eq!(p.motor.notas_ativas(), 0);
    }

    #[test]
    fn mesmas_alturas_e_tempos_em_dois_timbres_so_muda_o_programa_do_canal_3() {
        use crate::som::Timbre;
        use piano_tutor_audio::sf2_sintetico::{PROGRAMA_QUADRADA, PROGRAMA_SENOIDE};
        // O SF2 sintético faz o papel de "piano" e "acordeom" (dois presets de som distinto).
        let mut resultados = Vec::new();
        for programa in [PROGRAMA_QUADRADA, PROGRAMA_SENOIDE] {
            let (mut e, mut p) = com_take(simples());
            let mut c = p.som.config_teclado();
            c.timbre_esquerda = Timbre {
                banco: 0,
                programa: PROGRAMA_SENOIDE,
            };
            c.timbre_demonstracao = Some(Timbre { banco: 0, programa });
            assert_eq!(p.som.definir_config_teclado(c), Ok(()));
            p.motor.renderizar(BLOCO, 0);
            e.tocar(&mut p.som);
            rodar(&mut e, &mut p, 1200);
            p.motor.renderizar(48_000 * 2, 0);
            let notas = eventos_de_nota(&p.motor);
            let ordem: Vec<(bool, u8, u8)> =
                notas.iter().map(|(l, c, n, _)| (*l, *c, *n)).collect();
            let quadros: Vec<u64> = notas.iter().map(|x| x.3 - notas[0].3).collect();
            let programas: Vec<u8> = p
                .motor
                .registro()
                .iter()
                .filter_map(|r| match *r {
                    RegistroFalso::Programa {
                        canal, programa, ..
                    } if canal == CANAL_DEMONSTRACAO => Some(programa),
                    _ => None,
                })
                .collect();
            assert_eq!(programas.last(), Some(&programa));
            resultados.push((ordem, quadros));
        }
        assert_eq!(resultados[0].0, resultados[1].0, "mesmas notas e ordem");
        for (a, b) in resultados[0].1.iter().zip(&resultados[1].1) {
            assert!(a.abs_diff(*b) <= BLOCO as u64, "mesmos tempos");
        }
    }

    #[test]
    fn pausar_solta_as_notas_e_retomar_continua_do_ponto() {
        let (mut e, mut p) = com_take(simples());
        p.motor.renderizar(BLOCO, 0);
        e.tocar(&mut p.som);
        // 100 ms de atraso + 200 ms: notas 60 e 64 ligadas, nenhuma solta ainda.
        rodar(&mut e, &mut p, 320);
        e.pausar(&mut p.som);
        assert!(e.pausada() && !e.tocando());
        let pos = e.posicao_ns(p.som.agora_ns()).expect("posição");
        assert!(pos > 100 * MS && pos < 300 * MS, "{pos}");
        p.motor.renderizar(48_000, 0);
        assert_eq!(p.motor.notas_ativas(), 0, "pausa soltou tudo");
        let v = eventos_de_nota(&p.motor);
        assert_eq!(v.iter().filter(|x| x.0).count(), 2);
        assert_eq!(v.iter().filter(|x| !x.0).count(), 2);
        // Retoma: só o que vem depois do ponto (as soltas, em 300 e 500 ms), sem religar.
        e.pausar(&mut p.som);
        assert!(e.tocando());
        rodar(&mut e, &mut p, 800);
        assert!(!e.tocando());
        p.motor.renderizar(48_000 * 2, 0);
        assert_eq!(p.motor.notas_ativas(), 0);
    }

    #[test]
    fn parar_solta_notas_ate_as_que_ainda_estao_na_fila_e_o_pedal() {
        let g = Gravacao {
            nome: "p".to_owned(),
            eventos: vec![
                ev(0, pedal(0, 127)),
                ev(0, lig(0, 60, 80)),
                ev(120, lig(0, 62, 80)),
                ev(5000, des(0, 60)),
                ev(5000, des(0, 62)),
            ],
            tempos: Vec::new(),
            duracao_ns: 5100 * MS,
        };
        let (mut e, mut p) = com_take(g);
        p.motor.renderizar(BLOCO, 0);
        e.tocar(&mut p.som);
        rodar(&mut e, &mut p, 60);
        e.parar(&mut p.som);
        assert!(!e.tocando() && !e.pausada());
        p.motor.renderizar(48_000 * 2, 0);
        assert_eq!(p.motor.notas_ativas(), 0, "nada preso");
        let pedais: Vec<u8> = p
            .motor
            .registro()
            .iter()
            .filter_map(|r| match *r {
                RegistroFalso::Controle {
                    canal,
                    controle: 64,
                    valor,
                    ..
                } if canal == CANAL_DEMONSTRACAO => Some(valor),
                _ => None,
            })
            .collect();
        assert_eq!(pedais, vec![127, 0]);
    }

    #[test]
    fn notas_abertas_no_arquivo_soltam_no_fim_e_percussao_vai_para_o_canal_10() {
        let g = Gravacao {
            nome: "aberta".to_owned(),
            eventos: vec![ev(0, lig(0, 60, 80)), ev(10, lig(9, 38, 100))],
            tempos: Vec::new(),
            duracao_ns: 400 * MS,
        };
        let (mut e, mut p) = com_take(g);
        p.motor.renderizar(BLOCO, 0);
        e.tocar(&mut p.som);
        rodar(&mut e, &mut p, 800);
        assert!(!e.tocando());
        p.motor.renderizar(48_000 * 2, 0);
        assert_eq!(p.motor.notas_ativas(), 0);
        let v = eventos_de_nota(&p.motor);
        assert!(v.contains(&(true, CANAL_PERCUSSAO, 38, v[1].3)));
        assert_eq!(v.iter().filter(|x| !x.0).count(), 2);
    }

    #[test]
    fn nao_toca_durante_gravacao_nem_sem_selecao() {
        let (mut g, mut som, _r) = par();
        g.tocar(&mut som);
        assert!(!g.tocando());
        g.gravar(&mut som);
        g.takes.push(Take::nova(simples(), true, None));
        g.selecionada = Some(0);
        g.tocar(&mut som);
        assert!(!g.tocando());
    }

    #[test]
    fn remover_take_em_reproducao_para_e_ajusta_indices() {
        let (mut e, mut p) = com_take(simples());
        e.takes.push(Take::nova(simples(), true, None));
        e.tocar(&mut p.som);
        assert!(e.tocando());
        e.remover(0, &mut p.som);
        assert!(!e.tocando());
        assert_eq!(e.takes.len(), 1);
        assert_eq!(e.selecionada, None);
    }

    // --- Arquivos no disco -----------------------------------------------------------------

    fn pasta_temp(nome: &str) -> PathBuf {
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!("pt-grav-{}-{nome}-{n}", std::process::id()))
    }

    #[test]
    fn salvar_listar_abrir_e_importar_do_disco() {
        let pasta = pasta_temp("disco");
        let mut e = EstadoGravacao::novo(Some(pasta.clone()));
        e.takes.push(Take::nova(sequencia(), false, None));
        e.selecionada = Some(0);
        assert!(e.tem_dados_nao_salvos());
        e.nome_arquivo = "Minha ../peça".to_owned();
        let caminho = e.salvar(0).expect("salvou");
        assert!(caminho.starts_with(pasta.join("gravacoes")));
        assert!(!e.tem_dados_nao_salvos());
        assert_eq!(e.salvos().len(), 1);
        let nome = e.salvos()[0].clone();
        e.abrir_salvo(&nome);
        assert_eq!(e.takes.len(), 2);
        mesmas_mensagens(&e.takes[0].gravacao, &e.takes[1].gravacao);
        assert!(e.takes[1].salvo);
        // Importar por caminho digitado, com aspas.
        e.caminho_importar = format!("\"{}\"", caminho.display());
        e.importar_digitado();
        assert_eq!(e.takes.len(), 3);
        // Erros viram mensagem.
        e.caminho_importar = pasta.join("nao-existe.mid").display().to_string();
        e.importar_digitado();
        assert_eq!(e.takes.len(), 3);
        assert!(e.mensagem.as_deref().unwrap_or("").contains("importar"));
        e.abrir_salvo("../fora");
        assert_eq!(e.takes.len(), 3);
        // Arquivo corrompido no disco.
        std::fs::write(
            pasta.join("gravacoes").join("ruim.mid"),
            b"MThd\0\0\0\x06\0\0",
        )
        .expect("escreve");
        e.abrir_salvo("ruim");
        assert_eq!(e.takes.len(), 3);
        let _ = std::fs::remove_dir_all(pasta);
    }

    #[test]
    fn salvar_sem_pasta_ou_sem_nome_avisa_e_recuperacao_escreve_rascunho() {
        let mut e = EstadoGravacao::novo(None);
        e.takes.push(Take::nova(simples(), false, None));
        e.nome_arquivo = "x".to_owned();
        assert!(e.salvar(0).is_none());
        assert!(e.tem_dados_nao_salvos());
        let pasta = pasta_temp("rec");
        let mut e = EstadoGravacao::novo(Some(pasta.clone()));
        e.takes.push(Take::nova(simples(), false, None));
        e.nome_arquivo = "   ".to_owned();
        assert!(e.salvar(0).is_none());
        assert_eq!(e.salvar_recuperacao(), 1);
        assert!(e.tem_dados_nao_salvos(), "recuperação não conta como salvo");
        assert!(pasta.join("gravacoes").join("recuperacao").is_dir());
        let _ = std::fs::remove_dir_all(pasta);
    }

    #[test]
    fn notas_pareiam_ligar_e_soltar() {
        let n = sequencia().notas();
        assert_eq!(n.len(), 6);
        assert_eq!((n[0].nota, n[0].inicio_ns, n[0].fim_ns), (60, 0, 400 * MS));
        let aberta = Gravacao {
            nome: String::new(),
            eventos: vec![ev(100, lig(0, 60, 80))],
            tempos: Vec::new(),
            duracao_ns: 900 * MS,
        };
        assert_eq!(aberta.notas()[0].fim_ns, 900 * MS);
    }
}
