//! Executável do Piano Tutor: MIDI (`midir`/ALSA), som (`cpal` + `rustysynth`) e SF2 do sistema.

use bancada_casca::Casca;
use piano_tutor_app::app::{AppPianoTutor, config};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Casca::nova(AppPianoTutor::do_sistema(), config())?.executar()?;
    Ok(())
}
