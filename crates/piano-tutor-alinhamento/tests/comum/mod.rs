//! Fixtures sintéticas para os testes de integração: piano aditivo, "fala", MusicXML e WAV.
//! Nenhum áudio real é lido ou gravado no repositório.
#![allow(dead_code)]

use hound::{SampleFormat, WavSpec, WavWriter};
use piano_tutor_alinhamento::wav::Audio;
use std::f64::consts::PI;
use std::path::Path;

pub const TAXA: u32 = 22_050;

/// Nota de uma melodia: passo (C, D, ...), alteração, oitava (C4 = Dó central) e duração em
/// semínimas. `tocada = false` marca uma nota que a partitura tem mas o "tutor" não toca.
#[derive(Clone, Copy)]
pub struct NotaFixture {
    pub passo: char,
    pub alter: i8,
    pub oitava: i8,
    pub semin: u32,
    pub tocada: bool,
}

pub const fn n(passo: char, oitava: i8, semin: u32) -> NotaFixture {
    NotaFixture {
        passo,
        alter: 0,
        oitava,
        semin,
        tocada: true,
    }
}

pub fn midi(nota: &NotaFixture) -> u8 {
    let base: i32 = match nota.passo {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        _ => 11,
    };
    (12 * (i32::from(nota.oitava) + 1) + base + i32::from(nota.alter)) as u8
}

/// MusicXML de uma voz só, 4/4, divisions = 1, com andamento declarado.
pub fn musicxml(notas: &[NotaFixture], bpm: u32) -> String {
    let mut x = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
<part id="P1">"#,
    );
    let mut compasso = 1;
    let mut batidas = 0;
    x.push_str(&format!(r#"<measure number="{compasso}">"#));
    x.push_str(&format!(
        "<attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>\
         <direction><direction-type><metronome><beat-unit>quarter</beat-unit><per-minute>{bpm}</per-minute></metronome></direction-type><sound tempo=\"{bpm}\"/></direction>"
    ));
    for nota in notas {
        if batidas + nota.semin > 4 {
            compasso += 1;
            batidas = 0;
            x.push_str(&format!(r#"</measure><measure number="{compasso}">"#));
        }
        let alter = if nota.alter == 0 {
            String::new()
        } else {
            format!("<alter>{}</alter>", nota.alter)
        };
        let tipo = match nota.semin {
            1 => "quarter",
            2 => "half",
            3 => "half",
            _ => "whole",
        };
        x.push_str(&format!(
            "<note><pitch><step>{}</step>{alter}<octave>{}</octave></pitch><duration>{}</duration><voice>1</voice><type>{tipo}</type></note>",
            nota.passo, nota.oitava, nota.semin
        ));
        batidas += nota.semin;
    }
    x.push_str("</measure></part></score-partwise>");
    x
}

/// Gerador pseudoaleatório determinístico (xorshift64*).
pub struct Aleatorio(u64);

impl Aleatorio {
    pub fn novo(semente: u64) -> Self {
        Self(semente.max(1))
    }
    /// Uniforme em [-1, 1).
    pub fn proximo(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let v = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (v >> 11) as f64 / (1_u64 << 52) as f64 - 1.0
    }
}

/// Soma uma nota de "piano" (6 harmônicos, ataque de 5 ms, decaimento de 0,7 s) em `buf`.
/// `dur_s` é quanto a tecla fica pressionada (depois há 30 ms de soltura).
pub fn piano(buf: &mut [f32], midi: u8, ini_s: f64, dur_s: f64, amp: f64) {
    let taxa = f64::from(TAXA);
    let f0 = 440.0 * 2.0_f64.powf((f64::from(midi) - 69.0) / 12.0);
    let a = (ini_s * taxa) as usize;
    let b = (((ini_s + dur_s + 0.03) * taxa) as usize).min(buf.len());
    if a >= b {
        return;
    }
    for (k, amostra) in buf[a..b].iter_mut().enumerate() {
        let t = k as f64 / taxa;
        let env =
            (-t / 0.7).exp() * (t / 0.005).min(1.0) * ((dur_s + 0.03 - t) / 0.03).clamp(0.0, 1.0);
        let mut v = 0.0;
        for h in 1..=6 {
            let hz = f0 * f64::from(h);
            if hz < taxa / 2.0 {
                v += (2.0 * PI * hz * t).sin() / f64::from(h);
            }
        }
        *amostra += (amp * env * v) as f32;
    }
}

/// "Fala": voz com harmônicos e formantes, f0 variável, sílabas a ~3,5 Hz e ruído de sopro.
pub fn fala(buf: &mut [f32], ini_s: f64, fim_s: f64, semente: u64) {
    let taxa = f64::from(TAXA);
    let mut rng = Aleatorio::novo(semente);
    let a = (ini_s * taxa) as usize;
    let b = ((fim_s * taxa) as usize).min(buf.len());
    let mut fase = 0.0_f64;
    for (k, amostra) in buf[a..b].iter_mut().enumerate() {
        let t = k as f64 / taxa;
        let f0 = 118.0 + 28.0 * (2.0 * PI * 0.7 * t).sin() + 10.0 * (2.0 * PI * 2.3 * t).sin();
        fase += 2.0 * PI * f0 / taxa;
        let silaba = (2.0 * PI * 3.5 * t).sin().powi(2);
        let mut v = 0.0;
        for h in 1..=24 {
            let hz = f0 * f64::from(h);
            let peso = (-((hz - 650.0) / 350.0).powi(2)).exp()
                + 0.6 * (-((hz - 1800.0) / 500.0).powi(2)).exp();
            v += peso * (fase * f64::from(h)).sin();
        }
        let sopro = 0.04 * rng.proximo() * (1.0 - silaba);
        *amostra += (0.12 * silaba * v + sopro) as f32;
    }
}

/// Resultado de tocar uma melodia: o áudio e a verdade (instante real de cada nota).
pub struct Gravacao {
    pub audio: Audio,
    /// Por nota da partitura (mesma ordem): instante em que foi tocada, ou `None` se faltou.
    pub verdade: Vec<Option<f64>>,
    pub inicio_s: f64,
    pub fim_s: f64,
}

/// Monta a gravação: silêncio, fala, silêncio, a melodia (andamento `bpm * 1 / escala`, isto é,
/// `escala` > 1 é mais lento) e silêncio final.
pub fn gravar(notas: &[NotaFixture], bpm: f64, escala: f64, inicio_melodia_s: f64) -> Gravacao {
    let semin_s = 60.0 / bpm * escala;
    let total: u32 = notas.iter().map(|x| x.semin).sum();
    let fim_melodia = inicio_melodia_s + f64::from(total) * semin_s;
    let duracao = fim_melodia + 2.5;
    let mut buf = vec![0.0_f32; (duracao * f64::from(TAXA)) as usize];
    fala(&mut buf, 1.5, (inicio_melodia_s - 1.0).max(1.5), 7);
    let mut verdade = Vec::new();
    let mut t = inicio_melodia_s;
    for nota in notas {
        let dur = f64::from(nota.semin) * semin_s;
        if nota.tocada {
            piano(&mut buf, midi(nota), t, dur * 0.92, 0.25);
            verdade.push(Some(t));
        } else {
            verdade.push(None);
        }
        t += dur;
    }
    Gravacao {
        audio: Audio {
            taxa_hz: TAXA,
            amostras: buf,
        },
        verdade,
        inicio_s: inicio_melodia_s,
        fim_s: fim_melodia,
    }
}

/// Melodia de 7 notas (Dó Ré Mi Sol Mi Ré Dó), a última com 2 tempos.
pub fn melodia() -> Vec<NotaFixture> {
    vec![
        n('C', 4, 1),
        n('D', 4, 1),
        n('E', 4, 1),
        n('G', 4, 1),
        n('E', 4, 1),
        n('D', 4, 1),
        n('C', 4, 2),
    ]
}

pub fn escrever_wav(caminho: &Path, audio: &Audio) {
    let spec = WavSpec {
        channels: 1,
        sample_rate: audio.taxa_hz,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut w = WavWriter::create(caminho, spec).expect("criar WAV");
    for a in &audio.amostras {
        w.write_sample((a.clamp(-1.0, 1.0) * 32767.0) as i16)
            .expect("amostra");
    }
    w.finalize().expect("fechar WAV");
}
