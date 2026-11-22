//! Render offline (sem dispositivo) e exportação WAV de teste.
//!
//! O render offline chama o mesmo [`MotorAudio::processar_intercalado`] que o
//! callback real, simulando os despertares do callback. Serve para testes,
//! para o relatório de jitter e para gerar WAV de escuta.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::erro::ErroAudio;
use crate::motor::MotorAudio;
use crate::relogio::{TaxaAmostragem, frames_para_ns};
use crate::sintetizador::Sintetizador;

/// Renderiza `frames_total` frames em blocos de `bloco` frames, com despertar
/// ideal (instante = `t0_ns` + tempo do frame inicial). Devolve as amostras
/// intercaladas.
pub fn renderizar<S: Sintetizador>(
    motor: &mut MotorAudio<S>,
    frames_total: usize,
    bloco: usize,
    canais: usize,
    t0_ns: u64,
) -> Vec<f32> {
    let taxa = motor.config().taxa;
    let mut saida = vec![0.0_f32; frames_total * canais];
    let bloco = bloco.max(1);
    let mut feito = 0;
    while feito < frames_total {
        let n = bloco.min(frames_total - feito);
        let agora = t0_ns + frames_para_ns(motor.frame_atual(), taxa);
        motor.processar_intercalado(
            &mut saida[feito * canais..(feito + n) * canais],
            canais,
            agora,
        );
        feito += n;
    }
    saida
}

/// Grava WAV PCM 16 bits. Amostras fora de [-1, 1] são limitadas.
pub fn escrever_wav_pcm16(
    caminho: &Path,
    amostras: &[f32],
    canais: u16,
    taxa: TaxaAmostragem,
) -> Result<(), ErroAudio> {
    let mut w = BufWriter::new(File::create(caminho)?);
    w.write_all(&cabecalho_wav_pcm16(amostras.len(), canais, taxa))?;
    for s in amostras {
        let v = (s.clamp(-1.0, 1.0) * 32_767.0).round() as i16;
        w.write_all(&v.to_le_bytes())?;
    }
    w.flush()?;
    Ok(())
}

/// Cabeçalho RIFF/WAVE de 44 bytes para PCM 16 bits.
pub fn cabecalho_wav_pcm16(n_amostras: usize, canais: u16, taxa: TaxaAmostragem) -> [u8; 44] {
    let dados = (n_amostras * 2) as u32;
    let bloco = canais * 2;
    let bytes_por_s = taxa.hz() * u32::from(bloco);
    let mut h = [0_u8; 44];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&(36 + dados).to_le_bytes());
    h[8..12].copy_from_slice(b"WAVE");
    h[12..16].copy_from_slice(b"fmt ");
    h[16..20].copy_from_slice(&16_u32.to_le_bytes());
    h[20..22].copy_from_slice(&1_u16.to_le_bytes());
    h[22..24].copy_from_slice(&canais.to_le_bytes());
    h[24..28].copy_from_slice(&taxa.hz().to_le_bytes());
    h[28..32].copy_from_slice(&bytes_por_s.to_le_bytes());
    h[32..34].copy_from_slice(&bloco.to_le_bytes());
    h[34..36].copy_from_slice(&16_u16.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&dados.to_le_bytes());
    h
}

/// Pico absoluto e RMS de um sinal.
pub fn pico_e_rms(amostras: &[f32]) -> (f32, f32) {
    if amostras.is_empty() {
        return (0.0, 0.0);
    }
    let pico = amostras.iter().fold(0.0_f32, |m, x| m.max(x.abs()));
    let soma: f64 = amostras.iter().map(|x| f64::from(*x) * f64::from(*x)).sum();
    (pico, (soma / amostras.len() as f64).sqrt() as f32)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cabecalho_wav_tem_campos_corretos() {
        let taxa = TaxaAmostragem::nova(44_100).expect("taxa");
        let h = cabecalho_wav_pcm16(1_000, 2, taxa);
        assert_eq!(&h[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes([h[4], h[5], h[6], h[7]]), 36 + 2_000);
        assert_eq!(&h[8..16], b"WAVEfmt ");
        assert_eq!(u16::from_le_bytes([h[22], h[23]]), 2);
        assert_eq!(u32::from_le_bytes([h[24], h[25], h[26], h[27]]), 44_100);
        assert_eq!(u32::from_le_bytes([h[28], h[29], h[30], h[31]]), 176_400);
        assert_eq!(u16::from_le_bytes([h[32], h[33]]), 4);
        assert_eq!(u32::from_le_bytes([h[40], h[41], h[42], h[43]]), 2_000);
    }

    #[test]
    fn pico_e_rms_de_sinal_conhecido() {
        assert_eq!(pico_e_rms(&[]), (0.0, 0.0));
        let (p, r) = pico_e_rms(&[0.5, -0.5, 0.5, -0.5]);
        assert_eq!(p, 0.5);
        assert!((r - 0.5).abs() < 1e-6);
    }
}
