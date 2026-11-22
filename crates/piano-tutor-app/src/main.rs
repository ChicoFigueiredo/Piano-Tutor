//! Executável do app "Olá Piano Tutor" (F00).

use bancada_casca::Casca;
use piano_tutor_app::app::{AppPianoTutor, config};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Casca::nova(AppPianoTutor::novo(), config())?.executar()?;
    Ok(())
}
