//! Medição de jitter offline do relógio mestre (F03, ticket #37).
//!
//! Exige erro ≤ 1 bloco nos cenários dentro da especificação e confere que
//! `docs/aceite/F03/jitter-offline.md` corresponde ao código. Para regravar:
//! `PT_PUBLICAR_JITTER=1 cargo test -p piano-tutor-audio --test jitter_offline`.

use std::path::PathBuf;

use piano_tutor_audio::jitter::{
    ModoAgendamento, cenarios_padrao, medir_cenario, medir_rusty_sintetico, relatorio_markdown,
};

fn caminho_relatorio() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/aceite/F03/jitter-offline.md")
}

#[test]
fn jitter_offline_dentro_de_um_bloco_e_relatorio_publicado() {
    let resultados: Vec<_> = cenarios_padrao().iter().map(medir_cenario).collect();
    for r in &resultados {
        let c = &r.cenario;
        assert!(
            r.todos_detectados(),
            "{}: {} de {} impulsos",
            c.nome,
            r.impulsos_detectados,
            r.eventos_enviados
        );
        if c.dentro_da_especificacao() {
            let e = r.erro_absoluto();
            assert!(
                e.max <= c.bloco as f64,
                "{}: erro máximo {} > bloco {}",
                c.nome,
                e.max,
                c.bloco
            );
            assert_eq!(r.comandos_atrasados, 0, "{}: eventos atrasados", c.nome);
        }
    }

    // A âncora suavizada reduz o jitter com os mesmos atrasos (C × D).
    let crua = resultados
        .iter()
        .find(|r| r.cenario.nome.starts_with("C:"))
        .expect("cenário C");
    let suave = resultados
        .iter()
        .find(|r| r.cenario.nome.starts_with("D:"))
        .expect("cenário D");
    assert!(suave.jitter().p95 < crua.jitter().p95);
    assert!(suave.jitter().max < crua.jitter().max);

    // Sem agendamento (G) o jitter é maior que com agendamento (D).
    let imediato = resultados
        .iter()
        .find(|r| r.cenario.modo == ModoAgendamento::Imediato)
        .expect("cenário G");
    assert!(imediato.jitter().p95 > suave.jitter().p95);

    // Latência alvo curta demais deve gerar atrasos (mostra o limite).
    let curta = resultados
        .iter()
        .find(|r| r.cenario.nome.starts_with("H:"))
        .expect("cenário H");
    assert!(curta.comandos_atrasados > 0);

    let rusty: Vec<_> = [64_usize, 16]
        .iter()
        .map(|b| medir_rusty_sintetico(*b, 200).expect("SF2 sintético carrega"))
        .collect();
    for r in &rusty {
        assert_eq!(r.detectados, r.eventos, "bloco interno {}", r.bloco_interno);
        let e = r.estatistica();
        assert!(
            r.atrasos_frames.iter().all(|a| *a >= 0),
            "nota soou antes do agendado: {:?}",
            r.atrasos_frames
        );
        assert!(
            e.max <= r.bloco_interno as f64 + 1.0,
            "bloco interno {}: atraso máx {}",
            r.bloco_interno,
            e.max
        );
    }

    let md = relatorio_markdown(&resultados, &rusty);
    println!("{md}");
    let caminho = caminho_relatorio();
    if std::env::var_os("PT_PUBLICAR_JITTER").is_some() {
        if let Some(pai) = caminho.parent() {
            std::fs::create_dir_all(pai).expect("criar docs/aceite/F03");
        }
        std::fs::write(&caminho, &md).expect("gravar relatório");
    } else {
        let publicado = std::fs::read_to_string(&caminho).unwrap_or_default();
        assert!(
            publicado == md,
            "docs/aceite/F03/jitter-offline.md desatualizado; rode com PT_PUBLICAR_JITTER=1"
        );
    }
}
