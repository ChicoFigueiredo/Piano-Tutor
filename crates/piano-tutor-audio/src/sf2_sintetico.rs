//! Gerador de um SF2 mínimo, **sintético e próprio** (formas de onda
//! calculadas aqui, sem amostras de terceiros), para testar o `rustysynth`
//! na nuvem/CI sem baixar nem versionar SoundFonts.
//!
//! Presets gerados (banco 0): programa 0 = onda quadrada, programa 19 =
//! senoide. Não serve para avaliar timbre: é só infraestrutura de teste.

/// Programa GM do preset de onda quadrada.
pub const PROGRAMA_QUADRADA: u8 = 0;
/// Programa GM do preset de senoide.
pub const PROGRAMA_SENOIDE: u8 = 19;

const TAXA_AMOSTRA: u32 = 44_100;
const PERIODO: usize = 100; // 441 Hz com nota raiz 69
const PERIODOS: usize = 10;
const AMPLITUDE: f64 = 8_000.0;
const GERADORES_POR_ZONA: u16 = 5;
/// -32768 timecents (≈ 0 s) codificado como u16.
const TIMECENTS_MINIMO: u16 = 0x8000;

fn bloco(id: &[u8; 4], dados: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(8 + dados.len() + 1);
    v.extend_from_slice(id);
    v.extend_from_slice(&(dados.len() as u32).to_le_bytes());
    v.extend_from_slice(dados);
    if dados.len() % 2 == 1 {
        v.push(0);
    }
    v
}

fn lista(tipo: &[u8; 4], filhos: &[Vec<u8>]) -> Vec<u8> {
    let mut corpo = tipo.to_vec();
    for f in filhos {
        corpo.extend_from_slice(f);
    }
    bloco(b"LIST", &corpo)
}

fn nome20(nome: &str) -> [u8; 20] {
    let mut n = [0_u8; 20];
    for (d, s) in n.iter_mut().zip(nome.bytes().take(19)) {
        *d = s;
    }
    n
}

fn u16le(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn u32le(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn onda(senoide: bool) -> Vec<i16> {
    (0..PERIODO * PERIODOS)
        .map(|i| {
            let fase = (i % PERIODO) as f64 / PERIODO as f64;
            let x = if senoide {
                (2.0 * std::f64::consts::PI * fase).sin()
            } else if fase < 0.5 {
                1.0
            } else {
                -1.0
            };
            (x * AMPLITUDE).round() as i16
        })
        .collect()
}

/// Bytes de um arquivo SF2 válido com dois presets sintéticos.
pub fn gerar() -> Vec<u8> {
    let ondas = [onda(false), onda(true)];
    let nomes = ["Quadrada", "Senoide"];
    let programas = [u16::from(PROGRAMA_QUADRADA), u16::from(PROGRAMA_SENOIDE)];

    // sdta/smpl: cada amostra seguida de 46 zeros (exigência do formato).
    let mut smpl = Vec::new();
    let mut inicios = Vec::new();
    let mut pos = 0_u32;
    for o in &ondas {
        inicios.push(pos);
        for s in o {
            smpl.extend_from_slice(&s.to_le_bytes());
        }
        smpl.extend(std::iter::repeat_n(0_u8, 46 * 2));
        pos += (o.len() + 46) as u32;
    }

    let mut phdr = Vec::new();
    let mut pbag = Vec::new();
    let mut pgen = Vec::new();
    let mut inst = Vec::new();
    let mut ibag = Vec::new();
    let mut igen = Vec::new();
    let mut shdr = Vec::new();
    for i in 0..ondas.len() {
        let i16_ = i as u16;
        // Preset i → zona i → gerador "instrument" = i.
        phdr.extend_from_slice(&nome20(nomes[i]));
        u16le(&mut phdr, programas[i]);
        u16le(&mut phdr, 0); // banco
        u16le(&mut phdr, i16_); // primeira zona
        u32le(&mut phdr, 0);
        u32le(&mut phdr, 0);
        u32le(&mut phdr, 0);
        u16le(&mut pbag, i16_); // índice do gerador
        u16le(&mut pbag, 0);
        u16le(&mut pgen, 41); // instrument
        u16le(&mut pgen, i16_);

        // Instrumento i → zona i → geradores: atraso e ataque mínimos do
        // envelope de volume (isola a quantização do render), loop contínuo,
        // nota raiz e, por último, a amostra.
        inst.extend_from_slice(&nome20(nomes[i]));
        u16le(&mut inst, i16_);
        u16le(&mut ibag, i16_ * GERADORES_POR_ZONA);
        u16le(&mut ibag, 0);
        for (ger, val) in [
            (33_u16, TIMECENTS_MINIMO),
            (34, TIMECENTS_MINIMO),
            (54, 1_u16),
            (58, 69),
            (53, i16_),
        ] {
            u16le(&mut igen, ger);
            u16le(&mut igen, val);
        }

        let ini = inicios[i];
        let fim = ini + ondas[i].len() as u32;
        shdr.extend_from_slice(&nome20(nomes[i]));
        u32le(&mut shdr, ini);
        u32le(&mut shdr, fim);
        u32le(&mut shdr, ini + PERIODO as u32);
        u32le(&mut shdr, fim - PERIODO as u32);
        u32le(&mut shdr, TAXA_AMOSTRA);
        shdr.push(69); // nota original
        shdr.push(0); // correção de afinação
        u16le(&mut shdr, 0); // link
        u16le(&mut shdr, 1); // mono
    }
    let n = ondas.len() as u16;
    // Terminadores.
    phdr.extend_from_slice(&nome20("EOP"));
    phdr.extend_from_slice(&[0; 4]);
    u16le(&mut phdr, n);
    phdr.extend_from_slice(&[0; 12]);
    u16le(&mut pbag, n);
    u16le(&mut pbag, 0);
    pgen.extend_from_slice(&[0; 4]);
    inst.extend_from_slice(&nome20("EOI"));
    u16le(&mut inst, n);
    u16le(&mut ibag, n * GERADORES_POR_ZONA);
    u16le(&mut ibag, 0);
    igen.extend_from_slice(&[0; 4]);
    shdr.extend_from_slice(&nome20("EOS"));
    shdr.extend_from_slice(&[0; 26]);

    let mut ifil = Vec::new();
    u16le(&mut ifil, 2);
    u16le(&mut ifil, 1);
    let info = lista(
        b"INFO",
        &[
            bloco(b"ifil", &ifil),
            bloco(b"isng", b"EMU8000\0"),
            bloco(b"INAM", b"Piano Tutor sintetico\0"),
        ],
    );
    let sdta = lista(b"sdta", &[bloco(b"smpl", &smpl)]);
    let pdta = lista(
        b"pdta",
        &[
            bloco(b"phdr", &phdr),
            bloco(b"pbag", &pbag),
            bloco(b"pmod", &[0; 10]),
            bloco(b"pgen", &pgen),
            bloco(b"inst", &inst),
            bloco(b"ibag", &ibag),
            bloco(b"imod", &[0; 10]),
            bloco(b"igen", &igen),
            bloco(b"shdr", &shdr),
        ],
    );
    let mut corpo = b"sfbk".to_vec();
    corpo.extend_from_slice(&info);
    corpo.extend_from_slice(&sdta);
    corpo.extend_from_slice(&pdta);
    bloco(b"RIFF", &corpo)
}
