//! Executável do Piano Tutor: MIDI (`midir`/ALSA), som (`cpal` + `rustysynth`) e SF2 do sistema.

use bancada_casca::Casca;
use piano_tutor_app::app::{AppPianoTutor, config, preferencias_da_janela_salvas};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Tema, Ribbon, densidade, escala e painéis do perfil ativo, antes de abrir a janela.
    let janela = preferencias_da_janela_salvas();
    let mut casca = Casca::nova(AppPianoTutor::do_sistema(), config())?;
    if let Some(p) = janela {
        casca.aplicar_preferencias(&p);
    }
    casca.executar()?;
    Ok(())
}
