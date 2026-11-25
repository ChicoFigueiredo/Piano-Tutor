//! Alinhamento ponta a ponta com áudio sintético (piano aditivo + "fala" + silêncio).
//! Use `cargo test -p piano-tutor-alinhamento -- --nocapture` para ver os erros medidos.

mod comum;

use comum::{Gravacao, NotaFixture, gravar, melodia, musicxml};
use piano_tutor_alinhamento::{Alinhamento, ConfigAlinhamento, alinhar};
use piano_tutor_core::partitura::Partitura;

const LIMITE_ERRO_S: f64 = 0.060;

fn alinhar_gravacao(
    notas: &[NotaFixture],
    bpm_arquivo: u32,
    g: &Gravacao,
    cfg: &ConfigAlinhamento,
) -> Alinhamento {
    let p = Partitura::de_musicxml(&musicxml(notas, bpm_arquivo)).expect("MusicXML válido");
    alinhar(&p, &g.audio, cfg).expect("alinha")
}

/// Erro absoluto (s) por nota tocada, e o máximo.
fn erros(r: &Alinhamento, g: &Gravacao) -> (Vec<Option<f64>>, f64) {
    let por_nota: Vec<Option<f64>> = r
        .notas
        .iter()
        .zip(&g.verdade)
        .map(|(n, v)| v.map(|t| (n.t_audio_s - t).abs()))
        .collect();
    let max = por_nota.iter().flatten().copied().fold(0.0, f64::max);
    (por_nota, max)
}

#[test]
fn mesmo_andamento_cada_nota_com_erro_menor_que_60_ms() {
    let notas = melodia();
    let g = gravar(&notas, 100.0, 1.0, 6.0);
    let r = alinhar_gravacao(&notas, 100, &g, &ConfigAlinhamento::default());
    let (por_nota, max) = erros(&r, &g);
    eprintln!("[mesmo andamento] erros (ms): {:?}", ms(&por_nota));
    eprintln!("[mesmo andamento] confianças: {:?}", conf(&r));
    assert_eq!(r.notas.len(), 7);
    assert!(max < LIMITE_ERRO_S, "erro máximo {max:.3} s");
    assert!(r.notas.iter().all(|n| n.confianca > 0.6), "{:?}", conf(&r));
    assert!(r.notas.iter().all(|n| n.com_onset));
    // Recorte: primeira nota − 0,5 s até o fim da melodia + 0,5 s.
    assert!(
        (r.recorte.inicio_s - (g.inicio_s - 0.5)).abs() < 0.1,
        "{:?}",
        r.recorte
    );
    assert!(
        (r.recorte.fim_s - (g.fim_s + 0.5)).abs() < 0.25,
        "{:?}",
        r.recorte
    );
}

#[test]
fn andamento_15_por_cento_mais_lento_que_a_partitura() {
    let notas = melodia();
    let g = gravar(&notas, 100.0, 1.15, 6.0);
    let r = alinhar_gravacao(&notas, 100, &g, &ConfigAlinhamento::default());
    let (por_nota, max) = erros(&r, &g);
    eprintln!("[15% mais lento] erros (ms): {:?}", ms(&por_nota));
    eprintln!("[15% mais lento] confianças: {:?}", conf(&r));
    assert!(max < LIMITE_ERRO_S, "erro máximo {max:.3} s");
    assert!(r.confianca_media > 0.6, "{}", r.confianca_media);
    // O recorte acompanha o andamento mais lento (a melodia dura 8 * 0,69 s).
    assert!(
        (r.recorte.fim_s - (g.fim_s + 0.5)).abs() < 0.3,
        "{:?}",
        r.recorte
    );
}

#[test]
fn andamento_15_por_cento_mais_rapido_que_a_partitura() {
    let notas = melodia();
    let g = gravar(&notas, 100.0, 1.0 / 1.15, 6.0);
    let r = alinhar_gravacao(&notas, 100, &g, &ConfigAlinhamento::default());
    let (por_nota, max) = erros(&r, &g);
    eprintln!("[15% mais rápido] erros (ms): {:?}", ms(&por_nota));
    assert!(max < LIMITE_ERRO_S, "erro máximo {max:.3} s");
}

#[test]
fn bpm_informado_sobrepoe_o_do_arquivo() {
    let notas = melodia();
    // Arquivo diz 100, o tutor toca a 80.
    let g = gravar(&notas, 80.0, 1.0, 6.0);
    let cfg = ConfigAlinhamento {
        bpm: Some(80.0),
        ..ConfigAlinhamento::default()
    };
    let r = alinhar_gravacao(&notas, 100, &g, &cfg);
    let (por_nota, max) = erros(&r, &g);
    eprintln!("[bpm informado] erros (ms): {:?}", ms(&por_nota));
    assert!(max < LIMITE_ERRO_S, "erro máximo {max:.3} s");
    // t_partitura_s já reflete 80 bpm: 0,75 s por semínima.
    assert!((r.notas[1].t_partitura_s - r.notas[0].t_partitura_s - 0.75).abs() < 1e-6);
}

#[test]
fn nota_faltando_tem_confianca_baixa() {
    let mut notas = melodia();
    notas[3].tocada = false; // o Sol não é tocado
    let g = gravar(&notas, 100.0, 1.0, 6.0);
    let r = alinhar_gravacao(&notas, 100, &g, &ConfigAlinhamento::default());
    let (por_nota, max) = erros(&r, &g);
    eprintln!("[nota faltando] erros (ms): {:?}", ms(&por_nota));
    eprintln!("[nota faltando] confianças: {:?}", conf(&r));
    let faltando = r.notas[3].confianca;
    let melhor_das_outras = r
        .notas
        .iter()
        .filter(|n| n.indice != 3)
        .map(|n| n.confianca)
        .fold(f32::INFINITY, f32::min);
    assert!(faltando < 0.35, "confiança da nota faltando: {faltando}");
    assert!(!r.notas[3].com_onset);
    assert!(
        melhor_das_outras > faltando,
        "a pior das outras ({melhor_das_outras}) deve superar a faltante ({faltando})"
    );
    assert!(max < LIMITE_ERRO_S, "erro máximo nas tocadas {max:.3} s");
}

#[test]
fn nota_tocada_a_mais_pelo_tutor_fica_com_confianca_baixa_na_vizinha() {
    // A partitura não tem o Ré (índice 1), mas o tutor o toca. O DTW pode "pular" a nota
    // vizinha anterior (Dó), mas ela sai com confiança baixa e as demais continuam certas.
    let completa = melodia();
    let g = gravar(&completa, 100.0, 1.0, 6.0);
    let mut sem_re = completa.clone();
    sem_re.remove(1);
    let r = alinhar_gravacao(&sem_re, 100, &g, &ConfigAlinhamento::default());
    // Verdade das notas que existem na partitura: sem o índice 1.
    let verdade: Vec<f64> = g
        .verdade
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 1)
        .filter_map(|(_, v)| *v)
        .collect();
    let erros_ms: Vec<f64> = r
        .notas
        .iter()
        .zip(&verdade)
        .map(|(n, t)| (n.t_audio_s - t).abs() * 1000.0)
        .collect();
    eprintln!(
        "[nota a mais] erros (ms): {erros_ms:.0?}; confianças {:?}",
        conf(&r)
    );
    for (k, e) in erros_ms.iter().enumerate().skip(1) {
        assert!(*e < 60.0, "nota {k}: erro {e:.0} ms");
    }
    let a_mais = erros_ms[0] > 60.0;
    assert!(
        !a_mais || r.notas[0].confianca < 0.2,
        "erro grande na primeira nota ({:.0} ms) deve vir com confiança baixa ({})",
        erros_ms[0],
        r.notas[0].confianca
    );
}

#[test]
fn ruido_de_fundo_leve_nao_atrapalha() {
    let notas = melodia();
    let mut g = gravar(&notas, 100.0, 1.15, 6.0);
    let mut rng = comum::Aleatorio::novo(99);
    for a in &mut g.audio.amostras {
        *a += (0.01 * rng.proximo()) as f32;
    }
    let r = alinhar_gravacao(&notas, 100, &g, &ConfigAlinhamento::default());
    let (por_nota, max) = erros(&r, &g);
    eprintln!("[ruído de fundo] erros (ms): {:?}", ms(&por_nota));
    eprintln!("[ruído de fundo] confianças: {:?}", conf(&r));
    assert!(max < LIMITE_ERRO_S, "erro máximo {max:.3} s");
}

#[test]
fn busca_restrita_mantem_tempos_do_audio_inteiro() {
    let notas = melodia();
    let g = gravar(&notas, 100.0, 1.0, 6.0);
    let cfg = ConfigAlinhamento {
        busca: Some((4.0, 12.0)),
        ..ConfigAlinhamento::default()
    };
    let r = alinhar_gravacao(&notas, 100, &g, &cfg);
    let (_, max) = erros(&r, &g);
    assert!(max < LIMITE_ERRO_S, "erro máximo {max:.3} s");
    assert!(
        r.notas[0].t_audio_s > 5.5,
        "tempo no áudio inteiro, não no recorte"
    );
}

#[test]
fn dtw_global_funciona_quando_o_audio_e_so_o_exercicio() {
    let notas = melodia();
    // Sem fala nem silêncio inicial grande: a melodia começa em 0,3 s.
    let g = gravar(&notas, 100.0, 1.0, 0.3);
    let mut cfg = ConfigAlinhamento::default();
    cfg.dtw.subsequencia = false;
    let r = alinhar_gravacao(&notas, 100, &g, &cfg);
    let (por_nota, max) = erros(&r, &g);
    eprintln!("[global] erros (ms): {:?}", ms(&por_nota));
    assert!(max < LIMITE_ERRO_S, "erro máximo {max:.3} s");
}

#[test]
fn acorde_compartilha_instante_e_indices_seguem_os_esperados() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
<part id="P1"><measure number="1">
<attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
<direction><direction-type><metronome><beat-unit>quarter</beat-unit><per-minute>100</per-minute></metronome></direction-type><sound tempo="100"/></direction>
<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>
<note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note>
<note><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note>
<note><chord/><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note>
</measure></part></score-partwise>"#;
    let p = Partitura::de_musicxml(xml).expect("válido");
    let g = {
        // Verdade manual: C em 6,0; D em 6,6; E+G em 7,2.
        let mut buf = vec![0.0_f32; (11.0 * f64::from(comum::TAXA)) as usize];
        comum::fala(&mut buf, 1.0, 4.5, 3);
        comum::piano(&mut buf, 60, 6.0, 0.55, 0.25);
        comum::piano(&mut buf, 62, 6.6, 0.55, 0.25);
        comum::piano(&mut buf, 64, 7.2, 1.1, 0.25);
        comum::piano(&mut buf, 67, 7.2, 1.1, 0.25);
        piano_tutor_alinhamento::wav::Audio {
            taxa_hz: comum::TAXA,
            amostras: buf,
        }
    };
    let r = alinhar(&p, &g, &ConfigAlinhamento::default()).expect("alinha");
    assert_eq!(r.notas.len(), 4);
    let midis: Vec<u8> = r.notas.iter().map(|n| n.midi).collect();
    assert_eq!(midis.iter().filter(|m| **m == 64 || **m == 67).count(), 2);
    let (e, gg) = (&r.notas[2], &r.notas[3]);
    assert_eq!(e.t_audio_s, gg.t_audio_s, "acorde divide o instante");
    for (n, t) in [(&r.notas[0], 6.0), (&r.notas[1], 6.6), (e, 7.2)] {
        assert!((n.t_audio_s - t).abs() < LIMITE_ERRO_S, "{n:?} vs {t}");
    }
}

#[test]
fn erros_de_entrada() {
    let notas = melodia();
    let g = gravar(&notas, 100.0, 1.0, 6.0);
    let p = Partitura::de_musicxml(&musicxml(&notas, 100)).expect("válido");
    let ruim = ConfigAlinhamento {
        bpm: Some(-3.0),
        ..ConfigAlinhamento::default()
    };
    assert!(alinhar(&p, &g.audio, &ruim).is_err());
    let busca_ruim = ConfigAlinhamento {
        busca: Some((10.0, 5.0)),
        ..ConfigAlinhamento::default()
    };
    assert!(alinhar(&p, &g.audio, &busca_ruim).is_err());
    let curto = piano_tutor_alinhamento::wav::Audio {
        taxa_hz: comum::TAXA,
        amostras: vec![0.0; 2000],
    };
    assert!(alinhar(&p, &curto, &ConfigAlinhamento::default()).is_err());
}

#[test]
fn notas_repetidas_recebem_cada_uma_o_seu_ataque() {
    // Cada altura quatro vezes seguidas (como o Exercício 2 do curso): o croma é idêntico dentro
    // de cada sequência e só os onsets distinguem uma nota da outra.
    let mut notas = Vec::new();
    for passo in ['C', 'D', 'E', 'F'] {
        for _ in 0..4 {
            notas.push(comum::n(passo, 4, 1));
        }
    }
    notas.push(comum::n('C', 4, 4));
    let g = gravar(&notas, 100.0, 1.0, 6.0);
    let r = alinhar_gravacao(&notas, 100, &g, &ConfigAlinhamento::default());
    let (por_nota, max) = erros(&r, &g);
    eprintln!("[repetidas] erros (ms): {:?}", ms(&por_nota));
    eprintln!("[repetidas] confianças: {:?}", conf(&r));
    assert!(max < LIMITE_ERRO_S, "erro máximo {max:.3} s");
    assert!(
        r.notas.windows(2).all(|w| w[1].t_audio_s > w[0].t_audio_s),
        "instantes estritamente crescentes"
    );
    assert!(r.notas.iter().all(|n| n.com_onset));
}

fn ms(v: &[Option<f64>]) -> Vec<String> {
    v.iter()
        .map(|e| e.map_or("-".to_owned(), |x| format!("{:.0}", x * 1000.0)))
        .collect()
}

fn conf(r: &Alinhamento) -> Vec<String> {
    r.notas
        .iter()
        .map(|n| format!("{:.2}", n.confianca))
        .collect()
}
