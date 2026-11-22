//! Integração com um SF2 real, **somente** quando `PT_SF2` aponta para um
//! arquivo. Sem `PT_SF2` o teste fica ignorado (o SF2 não é baixado nem
//! versionado; licença no ticket #36).
//!
//! ```text
//! PT_SF2=/usr/share/sounds/sf2/FluidR3_GM.sf2 \
//!   cargo test -p piano-tutor-audio --test sf2_real -- --ignored --nocapture
//! ```
//!
//! Com `PT_WAV_DIR=<pasta>` grava também dois WAV curtos de escuta (programa
//! GM 0 e 21, mesma nota). Ouvir e assinar é o checklist de escuta (ADR-0006),
//! não este teste.

use std::path::PathBuf;
use std::sync::Arc;

use piano_tutor_audio::jitter::medir_inicio_rusty;
use piano_tutor_audio::offline::{escrever_wav_pcm16, pico_e_rms, renderizar};
use piano_tutor_audio::relogio::frames_para_ns;
use piano_tutor_audio::rusty::carregar_soundfont;
use piano_tutor_audio::{
    Acao, ComandoAudio, ConfigMotor, ConfigRusty, MotorAudio, Quando, SintetizadorRusty,
    TaxaAmostragem,
};
use piano_tutor_core::NotaMidi;
use piano_tutor_core::midi::CanalMidi;
use rustysynth::SoundFont;

const T0: u64 = 1_000_000_000;

fn c0() -> CanalMidi {
    CanalMidi::novo(0).expect("canal")
}

fn nota(n: u8) -> NotaMidi {
    NotaMidi::nova(n).expect("nota")
}

fn render(m: &mut MotorAudio<SintetizadorRusty>, frames: usize, canais: usize) -> Vec<f32> {
    let taxa = m.config().taxa;
    let t = T0 + frames_para_ns(m.frame_atual(), taxa);
    renderizar(m, frames, 256, canais, t)
}

fn motor(
    sf: &Arc<SoundFont>,
    taxa: TaxaAmostragem,
    reverb: bool,
) -> (
    MotorAudio<SintetizadorRusty>,
    piano_tutor_audio::ControleAudio,
) {
    let sint = SintetizadorRusty::novo(
        sf,
        taxa,
        ConfigRusty {
            reverb_e_chorus: reverb,
            ..ConfigRusty::default()
        },
    )
    .expect("sintetizador");
    MotorAudio::novo(ConfigMotor::padrao(taxa), sint).expect("motor")
}

fn imediato(acao: Acao) -> ComandoAudio {
    ComandoAudio::novo(Quando::Imediato, acao)
}

#[test]
#[ignore = "requer PT_SF2 apontando para um arquivo SF2 aprovado (não versionado)"]
fn sf2_real_definido_em_pt_sf2() {
    let Some(caminho) = std::env::var_os("PT_SF2").map(PathBuf::from) else {
        panic!("PT_SF2 não definido: este teste só roda com um SF2 real");
    };
    assert!(
        caminho.is_file(),
        "PT_SF2 não é um arquivo: {}",
        caminho.display()
    );
    let taxa = TaxaAmostragem::nova(48_000).expect("taxa");
    let inicio = std::time::Instant::now();
    let sf = carregar_soundfont(&caminho).expect("SF2 carrega");
    println!(
        "SF2: {} ({} presets) carregado em {:.2} s",
        sf.get_info().get_bank_name(),
        sf.get_presets().len(),
        inicio.elapsed().as_secs_f64()
    );

    // 1. Presets GM de piano (0) e acordeon (21) no banco 0.
    let tem = |programa: i32| {
        sf.get_presets()
            .iter()
            .find(|p| p.get_bank_number() == 0 && p.get_patch_number() == programa)
            .map(|p| p.get_name().to_string())
    };
    let piano = tem(0).expect("preset GM 0 (piano) no banco 0");
    let acordeon = tem(21).expect("preset GM 21 (acordeon) no banco 0");
    println!("preset 0: {piano}; preset 21: {acordeon}");

    // 2. Silêncio sem notas.
    let (mut m, _c) = motor(&sf, taxa, true);
    assert!(render(&mut m, 24_000, 2).iter().all(|x| *x == 0.0));

    // 3. Acorde de 10 vozes sustentado no volume padrão.
    let (mut m, mut c) = motor(&sf, taxa, true);
    let acorde = [48_u8, 52, 55, 60, 64, 67, 72, 76, 79, 84];
    for n in acorde {
        c.enviar(imediato(Acao::NotaLigada {
            canal: c0(),
            nota: nota(n),
            velocidade: 100,
        }))
        .expect("fila");
    }
    let sustentado = render(&mut m, 2 * 48_000, 2);
    let mut janelas_mudas = 0;
    for janela in sustentado[4_800..].chunks(960) {
        if pico_e_rms(janela).1 < 1e-3 {
            janelas_mudas += 1;
        }
    }
    let (pico, rms) = pico_e_rms(&sustentado);
    let recortes = m.estado().telemetria().amostras_recortadas;
    let fracao = recortes as f64 / sustentado.len() as f64;
    println!(
        "10 vozes: pico {pico:.3}, rms {rms:.3}, recortes {recortes} ({:.4}%), janelas mudas {janelas_mudas}",
        fracao * 100.0
    );
    assert_eq!(janelas_mudas, 0, "nota cortada durante o sustento");
    assert!(fracao <= 0.001, "recorte persistente no volume padrão");

    // 4. All-notes-off: silêncio depois da cauda.
    c.todas_notas_desligadas().expect("fila");
    render(&mut m, 4 * 48_000, 2);
    let (residuo, _) = pico_e_rms(&render(&mut m, 4_800, 2));
    println!("resíduo 4 s após all-notes-off: {residuo:.2e}");
    assert!(residuo < 1e-3, "resíduo após all-notes-off {residuo}");

    // 5. Pânico: corte imediato (sem reverb).
    let (mut m, mut c) = motor(&sf, taxa, false);
    c.enviar(imediato(Acao::NotaLigada {
        canal: c0(),
        nota: nota(60),
        velocidade: 100,
    }))
    .expect("fila");
    render(&mut m, 4_800, 2);
    c.panico();
    let depois = render(&mut m, 4_800, 2);
    assert_eq!(pico_e_rms(&depois[512..]).0, 0.0, "som após pânico");

    // 6. Atraso de início de nota com o SF2 real (sem reverb).
    let sint = SintetizadorRusty::novo(
        &sf,
        taxa,
        ConfigRusty {
            reverb_e_chorus: false,
            ..ConfigRusty::default()
        },
    )
    .expect("sintetizador");
    let r = medir_inicio_rusty(sint, taxa, 64, 50, 0x5EED_0200);
    let e = r.estatistica();
    println!(
        "início de nota (piano, bloco interno 64): {}/{} detectados; atraso médio {:.1} fr, p95 {:.0} fr, máx {:.0} fr",
        r.detectados, r.eventos, e.media, e.p95, e.max
    );
    assert_eq!(r.detectados, r.eventos);

    // 7. WAV de escuta (opcional).
    if let Some(dir) = std::env::var_os("PT_WAV_DIR").map(PathBuf::from) {
        let taxa_wav = TaxaAmostragem::nova(44_100).expect("taxa");
        for (programa, nome) in [
            (0_u8, "f03-gm000-piano-c4.wav"),
            (21, "f03-gm021-acordeon-c4.wav"),
        ] {
            let (mut m, mut c) = motor(&sf, taxa_wav, true);
            c.enviar(imediato(Acao::Programa {
                canal: c0(),
                programa,
            }))
            .expect("fila");
            c.enviar(imediato(Acao::NotaLigada {
                canal: c0(),
                nota: nota(60),
                velocidade: 100,
            }))
            .expect("fila");
            c.enviar(ComandoAudio::novo(
                Quando::Frame(52_920), // 1,2 s
                Acao::NotaDesligada {
                    canal: c0(),
                    nota: nota(60),
                },
            ))
            .expect("fila");
            let s = render(&mut m, 88_200, 1); // 2,0 s mono
            let (p, rms) = pico_e_rms(&s);
            let destino = dir.join(nome);
            escrever_wav_pcm16(&destino, &s, 1, taxa_wav).expect("gravar WAV");
            println!(
                "WAV {}: programa {programa}, pico {p:.3}, rms {rms:.3}, recortes {}",
                destino.display(),
                m.estado().telemetria().amostras_recortadas
            );
        }
    }
}
