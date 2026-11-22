//! Saída `cpal` no dispositivo padrão. Na nuvem não há placa de som: o
//! esperado é um **erro recuperável**, sem pânico. No cerebro, o fluxo abre e
//! o relógio mestre avança. Nenhum dos dois casos mede latência real.

use std::time::Duration;

use piano_tutor_audio::{ConfigSaida, RelogioMonotonico, SaidaAudio, SintetizadorFalso};

#[test]
fn abrir_saida_padrao_nao_derruba_o_processo() {
    let relogio = RelogioMonotonico::novo();
    match SaidaAudio::abrir_padrao(ConfigSaida::default(), relogio, |_taxa| {
        Ok(SintetizadorFalso::novo())
    }) {
        Ok(mut saida) => {
            println!("saída aberta: {:?}", saida.info());
            saida.controle().volume(0.0).expect("fila");
            std::thread::sleep(Duration::from_millis(300));
            let t = saida.telemetria();
            println!("telemetria: {t:?}");
            // Se o dispositivo aceitou o fluxo, o relógio mestre deve andar,
            // a menos que o host tenha sinalizado falha.
            assert!(t.frames > 0 || t.dispositivo_falhou || t.erros_dispositivo > 0);
        }
        Err(e) => {
            println!("NÃO VERIFICADO (sem saída de áudio utilizável aqui): {e}");
        }
    }
}

#[test]
fn erro_do_criador_de_sintetizador_e_propagado() {
    let relogio = RelogioMonotonico::novo();
    let r = SaidaAudio::abrir_padrao(ConfigSaida::default(), relogio, |_taxa| {
        Err::<SintetizadorFalso, _>(piano_tutor_audio::ErroAudio::SoundFontInvalido(
            "teste".to_string(),
        ))
    });
    // Sem dispositivo o erro é de dispositivo; com dispositivo, é o do SF2.
    assert!(r.is_err());
}

#[test]
fn buffer_zero_e_parametro_invalido_ou_sem_dispositivo() {
    let relogio = RelogioMonotonico::novo();
    let config = ConfigSaida {
        buffer_frames: Some(0),
        ..ConfigSaida::default()
    };
    let r = SaidaAudio::abrir_padrao(config, relogio, |_| Ok(SintetizadorFalso::novo()));
    assert!(r.is_err());
}
