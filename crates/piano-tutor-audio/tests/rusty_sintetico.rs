//! `rustysynth` atrás do motor, com um SF2 sintético gerado em memória
//! (sem amostras de terceiros). Roda sempre, na nuvem e no cerebro.
//!
//! Não avalia timbre: isso é o checklist de escuta assinado (ADR-0006).

use std::sync::Arc;

use piano_tutor_audio::offline::{pico_e_rms, renderizar};
use piano_tutor_audio::rusty::{carregar_soundfont, ler_soundfont};
use piano_tutor_audio::sf2_sintetico::{self, PROGRAMA_QUADRADA, PROGRAMA_SENOIDE};
use piano_tutor_audio::{
    Acao, ComandoAudio, ConfigMotor, ConfigRusty, ControleAudio, ErroAudio, MotorAudio, Quando,
    SintetizadorRusty, TaxaAmostragem,
};
use piano_tutor_core::NotaMidi;
use piano_tutor_core::midi::CanalMidi;

const T0: u64 = 1_000_000_000;

fn taxa() -> TaxaAmostragem {
    TaxaAmostragem::nova(48_000).expect("taxa")
}

fn sintetizador(config: ConfigRusty) -> SintetizadorRusty {
    let bytes = sf2_sintetico::gerar();
    let sf = ler_soundfont(&mut bytes.as_slice()).expect("SF2 sintético válido");
    SintetizadorRusty::novo(&sf, taxa(), config).expect("sintetizador")
}

fn motor() -> (MotorAudio<SintetizadorRusty>, ControleAudio) {
    MotorAudio::novo(
        ConfigMotor::padrao(taxa()),
        sintetizador(ConfigRusty::default()),
    )
    .expect("motor")
}

fn c0() -> CanalMidi {
    CanalMidi::novo(0).expect("canal")
}

fn ligar(nota: u8) -> ComandoAudio {
    ComandoAudio::novo(
        Quando::Imediato,
        Acao::NotaLigada {
            canal: c0(),
            nota: NotaMidi::nova(nota).expect("nota"),
            velocidade: 100,
        },
    )
}

fn render(m: &mut MotorAudio<SintetizadorRusty>, frames: usize) -> Vec<f32> {
    let t = T0 + piano_tutor_audio::relogio::frames_para_ns(m.frame_atual(), taxa());
    renderizar(m, frames, 256, 2, t)
}

#[test]
fn sf2_sintetico_lista_presets() {
    let s = sintetizador(ConfigRusty::default());
    let presets = s.presets();
    assert_eq!(presets.len(), 2);
    assert_eq!(presets[0].1, i32::from(PROGRAMA_QUADRADA));
    assert_eq!(presets[1].1, i32::from(PROGRAMA_SENOIDE));
    assert_eq!(s.volume_interno(), 0.5);
}

#[test]
fn listar_presets_do_soundfont_sem_criar_sintetizador() {
    let bytes = sf2_sintetico::gerar();
    let sf = ler_soundfont(&mut bytes.as_slice()).expect("SF2 sintético válido");
    let presets = piano_tutor_audio::listar_presets(&sf);
    let chaves: Vec<(u16, u8)> = presets.iter().map(|p| (p.banco, p.programa)).collect();
    assert_eq!(
        chaves,
        vec![(0, PROGRAMA_QUADRADA), (0, PROGRAMA_SENOIDE)],
        "{presets:?}"
    );
    assert!(presets.iter().all(|p| !p.nome.is_empty()));
}

#[test]
fn silencio_sem_notas() {
    let (mut m, _c) = motor();
    let saida = render(&mut m, 48_000);
    assert!(saida.iter().all(|x| *x == 0.0));
}

#[test]
fn nota_soa_e_all_notes_off_volta_ao_silencio() {
    let (mut m, mut c) = motor();
    c.enviar(ligar(69)).expect("fila");
    let tocando = render(&mut m, 4_800);
    let (pico, rms) = pico_e_rms(&tocando[960..]);
    assert!(pico > 0.01 && rms > 0.01, "pico {pico} rms {rms}");
    c.todas_notas_desligadas().expect("fila");
    // Release padrão curto + cauda do reverb: depois de 3 s, silêncio.
    render(&mut m, 3 * 48_000);
    let depois = render(&mut m, 4_800);
    let (pico, _) = pico_e_rms(&depois);
    assert!(pico < 1e-4, "resíduo {pico}");
}

#[test]
fn dez_vozes_sustentadas_sem_cortes_e_sem_recorte_no_volume_padrao() {
    let (mut m, mut c) = motor();
    for nota in [48_u8, 52, 55, 60, 64, 67, 72, 76, 79, 84] {
        c.enviar(ligar(nota)).expect("fila");
    }
    render(&mut m, 4_800);
    // 2 s sustentados: nenhuma janela de 10 ms fica em silêncio.
    let sustentado = render(&mut m, 96_000);
    for janela in sustentado.chunks(960) {
        let (_, rms) = pico_e_rms(janela);
        assert!(rms > 0.01, "queda de nível: rms {rms}");
    }
    let (pico, _) = pico_e_rms(&sustentado);
    let t = m.estado().telemetria();
    assert_eq!(
        t.amostras_recortadas, 0,
        "recorte com volume padrão (pico {pico})"
    );
    println!(
        "10 vozes, SF2 sintético: pico {pico:.3}, recortes {}",
        t.amostras_recortadas
    );
}

#[test]
fn volume_responde() {
    let medir = |volume: f32| {
        let (mut m, mut c) = motor();
        c.volume(volume).expect("fila");
        c.enviar(ligar(69)).expect("fila");
        let s = render(&mut m, 9_600);
        pico_e_rms(&s[4_800..]).1
    };
    let cheio = medir(1.0);
    let metade = medir(0.5);
    let mudo = medir(0.0);
    assert!(cheio > 0.01);
    assert!((metade / cheio - 0.5).abs() < 0.01, "{metade} / {cheio}");
    assert_eq!(mudo, 0.0);
}

fn motor_seco() -> (MotorAudio<SintetizadorRusty>, ControleAudio) {
    MotorAudio::novo(
        ConfigMotor::padrao(taxa()),
        sintetizador(ConfigRusty {
            reverb_e_chorus: false,
            ..ConfigRusty::default()
        }),
    )
    .expect("motor")
}

#[test]
fn panico_corta_imediatamente_mesmo_com_sustain() {
    let (mut m, mut c) = motor_seco();
    c.enviar(ligar(69)).expect("fila");
    // Sustain ligado: all-notes-off sozinho não liberaria a voz.
    c.enviar(ComandoAudio::novo(
        Quando::Imediato,
        Acao::Controle {
            canal: c0(),
            controle: 64,
            valor: 127,
        },
    ))
    .expect("fila");
    render(&mut m, 4_800);
    c.todas_notas_desligadas().expect("fila");
    let com_sustain = render(&mut m, 4_800);
    assert!(
        pico_e_rms(&com_sustain[2_400..]).1 > 0.01,
        "sustain deveria segurar a nota após all-notes-off"
    );
    c.panico();
    let s = render(&mut m, 4_800);
    // Sem reverb: depois do primeiro bloco de 256 frames, silêncio exato.
    let (pico_fim, _) = pico_e_rms(&s[2 * 256..]);
    assert_eq!(pico_fim, 0.0, "resíduo após pânico");
}

#[test]
fn troca_de_programa_muda_a_forma_de_onda() {
    let aspereza = |programa: u8| {
        let (mut m, mut c) = motor_seco();
        c.enviar(ComandoAudio::novo(
            Quando::Imediato,
            Acao::Programa {
                canal: c0(),
                programa,
            },
        ))
        .expect("fila");
        c.enviar(ligar(69)).expect("fila");
        let s = render(&mut m, 9_600);
        let esq: Vec<f32> = s[4_800..].iter().step_by(2).copied().collect();
        let dif: Vec<f32> = esq.windows(2).map(|w| w[1] - w[0]).collect();
        // RMS da derivada / RMS: senoide de 441 Hz a 48 kHz ≈ 0,058;
        // onda quadrada (bordas abruptas) bem maior.
        pico_e_rms(&dif).1 / pico_e_rms(&esq).1
    };
    let quadrada = aspereza(PROGRAMA_QUADRADA);
    let senoide = aspereza(PROGRAMA_SENOIDE);
    assert!(senoide < 0.08, "senoide {senoide}");
    assert!(
        quadrada > 2.0 * senoide,
        "quadrada {quadrada} × senoide {senoide}"
    );
}

#[test]
fn sf2_invalido_vira_erro_recuperavel() {
    let casos: [&[u8]; 4] = [
        b"",
        b"isto nao e um sf2",
        b"RIFF\x04\x00\x00\x00sfbk",
        b"RIFF\x10\x00\x00\x00WAVEfmt \x00\x00\x00\x00",
    ];
    for bytes in casos {
        let r = ler_soundfont(&mut &bytes[..]);
        assert!(
            matches!(r, Err(ErroAudio::SoundFontInvalido(_))),
            "{bytes:?}"
        );
    }
    // SF2 válido truncado em vários pontos: erro, nunca pânico propagado.
    let bom = sf2_sintetico::gerar();
    for corte in (0..bom.len()).step_by(97) {
        let r = ler_soundfont(&mut &bom[..corte]);
        assert!(r.is_err(), "corte {corte} aceito");
    }
    // Arquivo inexistente.
    let r = carregar_soundfont(std::path::Path::new("/caminho/que/nao/existe.sf2"));
    assert!(matches!(r, Err(ErroAudio::SoundFontInvalido(_))));
}

#[test]
fn sf2_com_bytes_corrompidos_nao_derruba() {
    let bom = sf2_sintetico::gerar();
    let mut estado = 0x1234_5678_u32;
    for _ in 0..200 {
        let mut ruim = bom.clone();
        for _ in 0..8 {
            estado = estado.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let i = (estado as usize) % ruim.len();
            ruim[i] = (estado >> 24) as u8;
        }
        // Ok ou Err: o importante é não propagar pânico.
        let r = ler_soundfont(&mut ruim.as_slice());
        if let Ok(sf) = r {
            let _ = SintetizadorRusty::novo(&Arc::clone(&sf), taxa(), ConfigRusty::default());
        }
    }
}

#[test]
fn parametros_invalidos_do_sintetizador_sao_erro() {
    let bytes = sf2_sintetico::gerar();
    let sf = ler_soundfont(&mut bytes.as_slice()).expect("SF2");
    let r = SintetizadorRusty::novo(
        &sf,
        taxa(),
        ConfigRusty {
            bloco_interno: 4,
            ..ConfigRusty::default()
        },
    );
    assert!(matches!(r, Err(ErroAudio::Sintetizador(_))));
    let r = SintetizadorRusty::novo(
        &sf,
        TaxaAmostragem::nova(8_000).expect("taxa"),
        ConfigRusty::default(),
    );
    assert!(matches!(r, Err(ErroAudio::Sintetizador(_))));
}
