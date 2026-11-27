//! Medição do spike (ticket 08): tabela por fixture com A1, tamanho do que é regenerado numa
//! edição e tempos. Rode com `cargo test --release -p piano-tutor-fixtures --test
//! documento_medicao -- --nocapture` para reproduzir os números do ADR-0017. Os tempos só são
//! impressos (dependem da máquina); as asserções cobrem o que é determinístico.

mod comum;

use piano_tutor_fixtures::corpus::corpus;
use piano_tutor_fixtures::documento::{Altura, Documento};
use std::time::Instant;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

#[test]
fn tabela_de_medicao_por_fixture() {
    eprintln!(
        "{:<24} {:>8} {:>4} {:>6} {:>6} {:>4} {:>9} {:>9} {:>9} {:>9} {:>10}",
        "fixture",
        "bytes",
        "part",
        "comp",
        "notas",
        "A1",
        "abrir_ms",
        "expo_ms",
        "edit_ms",
        "regen_B",
        "regen_%"
    );
    let (mut notas_total, mut sem_altura_com_pitch, mut sem_duracao) = (0, 0, 0);
    for f in corpus() {
        let t = Instant::now();
        let doc = Documento::abrir(&f.conteudo).expect("abre");
        let abrir = ms(t);
        let t = Instant::now();
        let a1 = doc.exportar().expect("exporta") == f.conteudo;
        let expo = ms(t);
        assert!(a1, "{}: A1", f.id);

        let compassos: usize = doc.partes().iter().map(|p| p.compassos.len()).sum();
        let mut ed = doc.clone();
        let alvo = ed
            .notas()
            .find(|n| n.altura.is_some())
            .map(|n| (n.id, n.altura.expect("a")));
        let (edit, regen) = match alvo {
            Some((id, a)) => {
                let t = Instant::now();
                ed.definir_altura(
                    id,
                    Altura {
                        passo: if a.passo == 'C' { 'D' } else { 'C' },
                        ..a
                    },
                )
                .expect("edita");
                let saida = ed.exportar().expect("exporta");
                let edit = ms(t);
                let regen: usize = ed.patches().itens().iter().map(|p| p.novo.len()).sum();
                assert_ne!(saida, f.conteudo);
                (edit, regen)
            }
            None => (0.0, 0),
        };
        for n in doc.notas() {
            notas_total += 1;
            if n.altura.is_none() && !n.pausa {
                sem_altura_com_pitch += 1;
            }
            if n.duracao.is_none() && !n.graca {
                sem_duracao += 1;
            }
        }
        eprintln!(
            "{:<24} {:>8} {:>4} {:>6} {:>6} {:>4} {:>9.2} {:>9.2} {:>9.2} {:>9} {:>9.2}%",
            f.id,
            f.conteudo.len(),
            doc.partes().len(),
            compassos,
            doc.total_de_notas(),
            if a1 { "ok" } else { "FALHA" },
            abrir,
            expo,
            edit,
            regen,
            100.0 * regen as f64 / f.conteudo.len() as f64
        );
    }
    eprintln!(
        "notas={notas_total} sem_altura_nao_pausa={sem_altura_com_pitch} sem_duracao_nao_graca={sem_duracao}"
    );
    assert_eq!(
        sem_altura_com_pitch, 0,
        "toda nota afinada do corpus tem altura inteira"
    );
}

#[test]
fn contagem_de_variantes_adversas() {
    let (mut total, mut ok) = (0, 0);
    for f in corpus() {
        for (_, texto) in comum::variantes(&f.conteudo) {
            total += 1;
            if Documento::abrir(&texto)
                .and_then(|d| d.exportar_regenerando_tudo())
                .is_ok_and(|s| s == texto)
            {
                ok += 1;
            }
        }
    }
    eprintln!("variantes adversas: {ok}/{total} idênticas byte a byte");
    assert_eq!(ok, total);
}
