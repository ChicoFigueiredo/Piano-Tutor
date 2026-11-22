//! Sanitização de textos antes de entrarem no relatório (lógica pura).
//!
//! Remove sequências que lembram números de série (hexadecimal, ou com 8 ou
//! mais caracteres e ao menos 4 dígitos) e pares `VID:PID` (4 hex + `:` + 4 hex).

/// Marcador que substitui trechos removidos.
pub const REMOVIDO: &str = "[removido]";

/// Divide o texto em segmentos: (é_alfanumérico_ascii, trecho).
fn segmentar(texto: &str) -> Vec<(bool, String)> {
    let mut saida: Vec<(bool, String)> = Vec::new();
    for c in texto.chars() {
        let alfa = c.is_ascii_alphanumeric();
        match saida.last_mut() {
            Some((eh_alfa, trecho)) if *eh_alfa == alfa => trecho.push(c),
            _ => saida.push((alfa, c.to_string())),
        }
    }
    saida
}

fn todo_hex(trecho: &str) -> bool {
    !trecho.is_empty() && trecho.chars().all(|c| c.is_ascii_hexdigit())
}

fn parece_serial(trecho: &str) -> bool {
    let digitos = trecho.chars().filter(char::is_ascii_digit).count();
    trecho.len() >= 8 && (todo_hex(trecho) || digitos >= 4)
}

/// Remove seriais longos e pares VID:PID de um texto.
#[must_use]
pub fn sanitizar(texto: &str) -> String {
    let segmentos = segmentar(texto);
    let mut saida = String::new();
    let mut i = 0;
    while i < segmentos.len() {
        let (alfa, trecho) = &segmentos[i];
        if *alfa {
            let eh_par = trecho.len() == 4
                && todo_hex(trecho)
                && matches!(segmentos.get(i + 1), Some((false, sep)) if sep == ":")
                && matches!(segmentos.get(i + 2), Some((true, pid)) if pid.len() == 4 && todo_hex(pid));
            if eh_par {
                saida.push_str(REMOVIDO);
                i += 3;
                continue;
            }
            if parece_serial(trecho) {
                saida.push_str(REMOVIDO);
                i += 1;
                continue;
            }
        }
        saida.push_str(trecho);
        i += 1;
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn remove_serial_so_com_digitos() {
        assert_eq!(sanitizar("Teclado 1234567890"), "Teclado [removido]");
    }

    #[test]
    fn remove_serial_hexadecimal_longo() {
        assert_eq!(sanitizar("Dev A1B2C3D4E5F6"), "Dev [removido]");
    }

    #[test]
    fn remove_serial_com_prefixo_alfabetico() {
        assert_eq!(sanitizar("KB-330 SN0123456789"), "KB-330 [removido]");
    }

    #[test]
    fn remove_par_vid_pid() {
        assert_eq!(sanitizar("USB 0a12:b3c4 MIDI"), "USB [removido] MIDI");
    }

    #[test]
    fn preserva_nome_de_modelo_e_porta_alsa() {
        assert_eq!(sanitizar("REVAS KB-330 20:0"), "REVAS KB-330 20:0");
    }

    #[test]
    fn preserva_palavras_longas_sem_digitos() {
        assert_eq!(sanitizar("Controller Keyboard"), "Controller Keyboard");
    }

    #[test]
    fn texto_vazio_continua_vazio() {
        assert_eq!(sanitizar(""), "");
    }

    #[test]
    fn preserva_acentos() {
        assert_eq!(sanitizar("Saída padrão"), "Saída padrão");
    }
}
