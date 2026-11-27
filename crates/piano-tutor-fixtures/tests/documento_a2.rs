//! A2 do spike (ticket 08): alterar a altura de uma nota muda só o compasso dela; o resto do
//! arquivo (créditos, lista de partes, identificação, comentários, filhos desconhecidos e os
//! outros compassos) fica idêntico byte a byte, e o `IdNota` não muda.

mod comum;

use piano_tutor_fixtures::corpus::{Fixture, corpus, fixture};
use piano_tutor_fixtures::documento::{Altura, Documento, IdNota, Nota};
use piano_tutor_fixtures::harness::{comparar_bytes, comparar_semantica, verificar_golden};

/// Próxima altura para o `i`-ésimo teste: alterna passo, sustenido e oitava.
fn outra_altura(a: Altura, i: usize) -> Altura {
    match i % 3 {
        0 => Altura {
            passo: if a.passo == 'G' {
                'A'
            } else {
                (a.passo as u8 + 1) as char
            },
            ..a
        },
        1 => Altura {
            alter: if a.alter == 0 { 1 } else { 0 },
            ..a
        },
        _ => Altura {
            oitava: if a.oitava >= 8 {
                a.oitava - 1
            } else {
                a.oitava + 1
            },
            ..a
        },
    }
}

/// O que identifica uma nota sem depender de faixas (que se deslocam com a edição).
type Essencia = (
    IdNota,
    Option<Altura>,
    Option<(i64, i64)>,
    String,
    String,
    bool,
    bool,
);

fn essencia(n: &Nota) -> Essencia {
    (
        n.id,
        n.altura,
        n.duracao.map(|d| (d.numerador(), d.denominador())),
        n.voz.clone(),
        n.pauta.clone(),
        n.acorde,
        n.pausa,
    )
}

/// Edita a nota `id` e confere A2 por inteiro. Devolve o texto exportado.
fn editar_e_conferir(f_id: &str, xml: &str, id: IdNota, nova: Altura) -> String {
    let mut doc = Documento::abrir(xml).expect("abre");
    let pos = doc.posicao_da_nota(id).expect("posição");
    let antes: Vec<_> = doc.notas().map(essencia).collect();
    doc.definir_altura(id, nova).expect("edita");
    assert_eq!(
        doc.compassos_sujos(),
        vec![pos.clone()],
        "{f_id}: só um sujo"
    );
    let saida = doc.exportar().expect("exporta");

    // 1. Bytes: só o compasso editado difere; nada fora dos compassos mudou.
    let rel = comparar_bytes(xml, &saida).expect("compara");
    assert_eq!(
        rel.compassos_diferentes,
        vec![pos.clone()],
        "{f_id}: compassos que diferem"
    );
    assert!(
        !rel.fora_dos_compassos,
        "{f_id}: algo fora dos compassos mudou"
    );
    verificar_golden(xml, &saida, std::slice::from_ref(&pos), None)
        .unwrap_or_else(|v| panic!("{f_id}: golden: {v:?}"));

    // 2. Tudo antes e tudo depois do compasso editado (créditos, lista de partes, identificação,
    //    comentários, compassos vizinhos) é o mesmo texto, byte a byte.
    let faixa = doc
        .partes()
        .iter()
        .find(|p| p.id == pos.parte)
        .map(|p| p.compassos[pos.indice].faixa.clone())
        .expect("faixa do compasso");
    assert!(saida.starts_with(&xml[..faixa.start]), "{f_id}: prefixo");
    assert!(saida.ends_with(&xml[faixa.end..]), "{f_id}: sufixo");

    // 3. Semântica: pelo harness independente, só esse compasso difere.
    let sem = comparar_semantica(xml, &saida).expect("semântica");
    assert_eq!(
        sem.compassos_diferentes,
        vec![pos],
        "{f_id}: {:?}",
        sem.detalhes
    );
    assert!(!sem.partes_diferentes);

    // 4. Modelo: reabrir o exportado dá os mesmos ids; só a altura da nota editada mudou.
    let reaberto = Documento::abrir(&saida).expect("reabre");
    let depois: Vec<_> = reaberto.notas().map(essencia).collect();
    assert_eq!(antes.len(), depois.len());
    for (a, d) in antes.iter().zip(&depois) {
        if a.0 == id {
            assert_eq!(d.1, Some(nova), "{f_id}: altura nova");
            assert_eq!((a.2, &a.3, &a.4, a.5, a.6), (d.2, &d.3, &d.4, d.5, d.6));
        } else {
            assert_eq!(a, d, "{f_id}: outra nota mudou");
        }
    }
    saida
}

/// Notas afinadas de `doc`, `passo` em `passo` (todas se forem poucas).
fn amostra(doc: &Documento, max: usize) -> Vec<IdNota> {
    let todas: Vec<IdNota> = doc
        .notas()
        .filter(|n| n.altura.is_some())
        .map(|n| n.id)
        .collect();
    let passo = todas.len().div_ceil(max).max(1);
    todas.into_iter().step_by(passo).collect()
}

fn varre(f: &Fixture, max: usize) -> usize {
    let doc = Documento::abrir(&f.conteudo).expect("abre");
    let ids = amostra(&doc, max);
    for (i, id) in ids.iter().enumerate() {
        let atual = doc.nota(*id).and_then(|n| n.altura).expect("afinada");
        editar_e_conferir(f.id, &f.conteudo, *id, outra_altura(atual, i));
    }
    ids.len()
}

#[test]
fn a2_duas_vozes_altura_da_voz_1_muda_so_o_compasso_1() {
    let f = fixture("duas-vozes");
    let doc = Documento::abrir(&f.conteudo).expect("abre");
    let alvo = doc
        .notas()
        .find(|n| n.voz == "1" && n.altura.is_some())
        .expect("nota da voz 1")
        .id;
    let saida = editar_e_conferir(
        f.id,
        &f.conteudo,
        alvo,
        Altura {
            passo: 'F',
            alter: 0,
            oitava: 5,
        },
    );
    // A voz 2 do mesmo compasso, o `backup` e o compasso 2 continuam como estavam.
    assert!(saida.contains("<backup><duration>4</duration></backup>"));
    assert!(saida.contains(
        "<step>C</step><octave>4</octave></pitch><duration>4</duration><voice>2</voice>"
    ));
    assert!(saida.contains("<measure number=\"2\">"));
    assert!(saida.contains("<pitch><step>F</step><octave>5</octave></pitch>"));
}

#[test]
fn a2_corpus_inteiro_alterar_altura_muda_so_um_compasso() {
    let mut total = 0;
    let mut fixtures_com_duas_vozes = 0;
    for f in corpus() {
        let doc = Documento::abrir(&f.conteudo).expect("abre");
        let duas_vozes = doc.partes().iter().any(|p| {
            p.compassos
                .iter()
                .any(|c| c.notas.iter().any(|n| n.voz != c.notas[0].voz))
        });
        fixtures_com_duas_vozes += usize::from(duas_vozes);
        total += varre(&f, 20);
    }
    assert!(total >= 150, "amostra pequena demais: {total}");
    assert!(
        fixtures_com_duas_vozes >= 3,
        "o critério pede pelo menos 3 fixtures com duas vozes"
    );
}

/// Exaustivo: TODAS as notas afinadas de TODAS as fixtures (1804 notas no corpus atual). Lento em
/// debug; rode com `cargo test --release -p piano-tutor-fixtures --test documento_a2 --
/// --ignored`.
#[test]
#[ignore = "exaustivo; rodar em release"]
fn a2_exaustivo_toda_nota_afinada_do_corpus() {
    let mut total = 0;
    for f in corpus() {
        total += varre(&f, usize::MAX);
    }
    eprintln!("A2 exaustivo: {total} edições conferidas");
    assert!(total >= 1700, "{total}");
}

#[test]
fn a2_vale_tambem_nas_variantes_adversas() {
    // BOM, CRLF, marcas com espaço, comentários dentro do compasso, atributo com `>` etc.
    for id_fix in ["duas-vozes", "brilha-brilha", "brahms-wiegenlied", "letra"] {
        let f = fixture(id_fix);
        for (rotulo, texto) in comum::variantes(&f.conteudo) {
            let doc = Documento::abrir(&texto).expect("abre");
            for (i, id) in amostra(&doc, 6).into_iter().enumerate() {
                let atual = doc.nota(id).and_then(|n| n.altura).expect("afinada");
                editar_e_conferir(
                    &format!("{id_fix}/{rotulo}"),
                    &texto,
                    id,
                    outra_altura(atual, i),
                );
            }
        }
    }
}

/// O critério do ticket: compasso com duas vozes, em pelo menos três fixtures diferentes. Edita a
/// altura de uma nota de CADA voz de cada compasso que tenha `<backup>` e duas vozes.
#[test]
fn a2_compasso_com_duas_vozes_em_pelo_menos_tres_fixtures() {
    let mut fixtures_cobertas = Vec::new();
    for f in corpus() {
        let doc = Documento::abrir(&f.conteudo).expect("abre");
        let mut alvos = Vec::new();
        for p in doc.partes() {
            for c in &p.compassos {
                let tem_backup = f.conteudo[c.faixa.clone()].contains("<backup");
                let vozes: std::collections::BTreeSet<&str> =
                    c.notas.iter().map(|n| n.voz.as_str()).collect();
                if tem_backup && vozes.len() >= 2 {
                    for v in &vozes {
                        if let Some(n) = c.notas.iter().find(|n| n.voz == *v && n.altura.is_some())
                        {
                            alvos.push(n.id);
                        }
                    }
                }
            }
        }
        // Até 10 compassos por fixture mantêm o teste rápido em debug.
        let passo = alvos.len().div_ceil(10).max(1);
        for (i, id) in alvos.iter().copied().step_by(passo).enumerate() {
            let atual = doc.nota(id).and_then(|n| n.altura).expect("afinada");
            editar_e_conferir(f.id, &f.conteudo, id, outra_altura(atual, i));
        }
        if !alvos.is_empty() {
            fixtures_cobertas.push(f.id);
        }
    }
    eprintln!("fixtures com compasso de duas vozes editado: {fixtures_cobertas:?}");
    assert!(fixtures_cobertas.len() >= 3, "{fixtures_cobertas:?}");
}

#[test]
fn elementos_desconhecidos_do_compasso_editado_sobrevivem() {
    let f = fixture("desconhecido");
    let doc = Documento::abrir(&f.conteudo).expect("abre");
    let ids: Vec<IdNota> = doc.notas().map(|n| n.id).collect();
    let desconhecidos = [
        "<grouping type=\"start\" number=\"1\"/>",
        "<figured-bass><figure><figure-number>6</figure-number></figure></figured-bass>",
        "<harmonic/><fingernails/></technical><x-extensao-futura valor=\"1\"/>",
        "<x-elemento-inexistente><filho>texto</filho></x-elemento-inexistente>",
    ];
    // Edita a 1ª nota (a que carrega a extensão dentro de <notations>) e a 2ª.
    for (i, id) in ids.iter().take(2).enumerate() {
        let atual = doc.nota(*id).and_then(|n| n.altura).expect("afinada");
        let saida = editar_e_conferir(f.id, &f.conteudo, *id, outra_altura(atual, i));
        for d in desconhecidos {
            assert!(saida.contains(d), "desconhecido perdido: {d}");
        }
    }
}

#[test]
fn desconhecido_dentro_da_propria_nota_e_do_pitch_fica_no_lugar() {
    let xml = "<?xml version=\"1.0\"?>\n<score-partwise version=\"4.0\"><part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list>\n<part id=\"P1\"><measure number=\"1\"><attributes><divisions>1</divisions></attributes>\n<note id=\"n1\"><x-antes/><pitch><step>C</step><x-no-pitch a=\"1\"/><octave>4</octave></pitch><duration>4</duration><x-meio at=\"&amp;\"/><voice>1</voice><type>whole</type><x-fim/></note>\n</measure></part></score-partwise>\n";
    let mut doc = Documento::abrir(xml).expect("abre");
    let id = doc.notas().next().expect("nota").id;
    doc.definir_altura(
        id,
        Altura {
            passo: 'C',
            alter: 1,
            oitava: 4,
        },
    )
    .expect("edita");
    let saida = doc.exportar().expect("exporta");
    // Regra: o elemento novo entra imediatamente antes do primeiro filho de posto maior
    // (`octave`), depois de `step`; o desconhecido entre os dois não se move.
    let esperado = xml.replace(
        "<step>C</step><x-no-pitch a=\"1\"/><octave>4</octave>",
        "<step>C</step><x-no-pitch a=\"1\"/><alter>1</alter><octave>4</octave>",
    );
    assert_eq!(saida, esperado);
}

#[test]
fn acidente_novo_entra_na_ordem_do_esquema_entre_type_e_stem() {
    let xml = "<score-partwise version=\"4.0\"><part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list>\n<part id=\"P1\">\n<measure number=\"1\">\n  <attributes><divisions>1</divisions></attributes>\n  <note>\n    <pitch><step>F</step><octave>4</octave></pitch>\n    <duration>4</duration>\n    <voice>1</voice>\n    <type>whole</type>\n    <stem>up</stem>\n    <staff>1</staff>\n  </note>\n</measure>\n</part></score-partwise>";
    let mut doc = Documento::abrir(xml).expect("abre");
    let id = doc.notas().next().expect("nota").id;
    doc.definir_acidente(id, Some("sharp")).expect("insere");
    let saida = doc.exportar().expect("exporta");
    let esperado = xml.replace(
        "    <type>whole</type>\n",
        "    <type>whole</type>\n    <accidental>sharp</accidental>\n",
    );
    assert_eq!(saida, esperado);
    // Trocar e remover mantêm a ordem e o recuo.
    let mut doc2 = Documento::abrir(&saida).expect("reabre");
    let id2 = doc2.notas().next().expect("nota").id;
    doc2.definir_acidente(id2, Some("natural")).expect("troca");
    assert!(
        doc2.exportar()
            .expect("exporta")
            .contains("<accidental>natural</accidental>")
    );
    doc2.definir_acidente(id2, None).expect("remove");
    assert_eq!(doc2.exportar().expect("exporta"), xml);
}

#[test]
fn id_nota_e_monotonico_comeca_em_1_e_e_estavel_ao_reabrir() {
    for f in corpus() {
        let doc = Documento::abrir(&f.conteudo).expect("abre");
        let ids: Vec<u64> = doc.notas().map(|n| n.id.0).collect();
        let esperados: Vec<u64> = (1..=ids.len() as u64).collect();
        assert_eq!(ids, esperados, "{}: ids em ordem de documento", f.id);
        let de_novo = Documento::abrir(&f.conteudo).expect("abre");
        assert!(doc.notas().map(|n| n.id).eq(de_novo.notas().map(|n| n.id)));
    }
}

#[test]
fn duas_edicoes_em_compassos_diferentes_geram_dois_patches_e_so_dois_compassos_mudam() {
    let f = fixture("cem-compassos");
    let mut doc = Documento::abrir(&f.conteudo).expect("abre");
    let (a, b) = {
        let mut it = doc.notas().filter(|n| n.altura.is_some());
        let a = it.nth(3).expect("a").id;
        let b = it.nth(200).expect("b").id;
        (a, b)
    };
    for id in [a, b] {
        let atual = doc.nota(id).and_then(|n| n.altura).expect("afinada");
        doc.definir_altura(id, outra_altura(atual, 0))
            .expect("edita");
    }
    let sujos = doc.compassos_sujos();
    assert_eq!(sujos.len(), 2);
    assert_eq!(doc.patches().itens().len(), 2);
    let saida = doc.exportar().expect("exporta");
    verificar_golden(&f.conteudo, &saida, &sujos, None).expect("só os dois");
}

#[test]
fn editar_para_a_mesma_altura_nao_suja_nada() {
    let f = fixture("duas-vozes");
    let mut doc = Documento::abrir(&f.conteudo).expect("abre");
    let n = doc.notas().find(|n| n.altura.is_some()).expect("nota");
    let (id, a) = (n.id, n.altura.expect("altura"));
    doc.definir_altura(id, a).expect("no-op");
    assert!(doc.compassos_sujos().is_empty());
    assert_eq!(doc.exportar().expect("exporta"), f.conteudo);
}

#[test]
fn edicao_invalida_e_recusada_sem_alterar_nada() {
    let f = fixture("duas-vozes");
    let mut doc = Documento::abrir(&f.conteudo).expect("abre");
    let id = doc.notas().next().expect("nota").id;
    assert!(
        doc.definir_altura(
            id,
            Altura {
                passo: 'H',
                alter: 0,
                oitava: 4
            }
        )
        .is_err()
    );
    assert!(
        doc.definir_altura(
            id,
            Altura {
                passo: 'C',
                alter: 0,
                oitava: 12
            }
        )
        .is_err()
    );
    assert!(
        doc.definir_altura(
            IdNota(9999),
            Altura {
                passo: 'C',
                alter: 0,
                oitava: 4
            }
        )
        .is_err()
    );
    assert!(doc.compassos_sujos().is_empty());
}
