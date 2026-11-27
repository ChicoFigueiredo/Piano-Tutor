//! IdNota no spike (ticket 08): monotônico, nunca reutilizado, atribuído na leitura; persistência
//! pelo `id` do MusicXML quando existir, senão por arquivo lateral. Mede se o lateral é
//! necessário no corpus.

use piano_tutor_fixtures::corpus::{corpus, fixture};
use piano_tutor_fixtures::documento::{Altura, Documento, ErroDocumento, IdNota, TabelaIds};

/// A mesma tabela sem a amarra ao texto (o que um arquivo lateral editado à mão seria).
fn sem_base(t: &TabelaIds) -> TabelaIds {
    let completo = t.para_texto();
    let texto: Vec<&str> = completo
        .lines()
        .filter(|l| !l.starts_with("base="))
        .collect();
    TabelaIds::de_texto(&texto.join("\n")).expect("lê")
}

fn com_ids_xml(ordem: &[&str]) -> String {
    let mut s = String::from(
        "<score-partwise version=\"4.0\"><part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list><part id=\"P1\"><measure number=\"1\"><attributes><divisions>1</divisions></attributes>",
    );
    for id in ordem {
        s.push_str(&format!(
            "<note id=\"{id}\"><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration></note>"
        ));
    }
    s.push_str("</measure></part></score-partwise>");
    s
}

#[test]
fn corpus_nao_usa_id_de_nota_do_musicxml_portanto_a_chave_e_a_posicao() {
    let (mut notas, mut com_id) = (0, 0);
    for f in corpus() {
        let doc = Documento::abrir(&f.conteudo).expect("abre");
        for n in doc.notas() {
            notas += 1;
            com_id += usize::from(n.id_xml.is_some());
        }
    }
    eprintln!("corpus: {notas} notas, {com_id} com atributo id no MusicXML");
    assert!(notas > 1000);
    assert_eq!(
        com_id, 0,
        "se o corpus passar a ter id de nota, reavaliar a conclusão do ADR-0017"
    );
}

#[test]
fn sem_edicao_estrutural_reabrir_da_os_mesmos_ids_sem_arquivo_lateral() {
    // Conclusão medida: para edição de altura/duração (nada entra, nada sai, nada muda de ordem),
    // a ordem do documento basta; o arquivo lateral é redundante no corpus.
    for f in corpus() {
        let mut doc = Documento::abrir(&f.conteudo).expect("abre");
        let antes: Vec<IdNota> = doc.notas().map(|n| n.id).collect();
        let alvo = doc
            .notas()
            .find(|n| n.altura.is_some())
            .map(|n| (n.id, n.altura.expect("altura")));
        if let Some((id, a)) = alvo {
            doc.definir_altura(
                id,
                Altura {
                    oitava: if a.oitava < 9 { a.oitava + 1 } else { 8 },
                    ..a
                },
            )
            .expect("edita");
        }
        let saida = doc.exportar().expect("exporta");
        let depois: Vec<IdNota> = Documento::abrir(&saida)
            .expect("reabre")
            .notas()
            .map(|n| n.id)
            .collect();
        assert_eq!(antes, depois, "{}", f.id);
    }
}

#[test]
fn id_do_musicxml_segue_a_nota_mesmo_se_a_ordem_mudar_e_nunca_e_reutilizado() {
    let doc = Documento::abrir(&com_ids_xml(&["a", "b", "c"])).expect("abre");
    let ids: Vec<u64> = doc.notas().map(|n| n.id.0).collect();
    assert_eq!(ids, vec![1, 2, 3]);
    let tabela = doc.tabela_ids();

    // Outro arquivo: ordem trocada, "b" removida, "d" nova.
    let outro = com_ids_xml(&["c", "a", "d"]);
    let reaberto = Documento::abrir_com_ids(&outro, Some(&sem_base(&tabela))).expect("abre");
    let por_xml: Vec<(String, u64)> = reaberto
        .notas()
        .map(|n| (n.id_xml.clone().expect("id xml"), n.id.0))
        .collect();
    assert_eq!(
        por_xml,
        vec![("c".into(), 3), ("a".into(), 1), ("d".into(), 4)],
        "c e a mantêm o id; d recebe 4, não o 2 que ficou vago"
    );
    assert!(reaberto.tabela_ids().proximo() >= 5);
}

#[test]
fn tabela_lateral_grava_e_le_sem_perda() {
    let f = fixture("brilha-brilha");
    let doc = Documento::abrir(&f.conteudo).expect("abre");
    let t = doc.tabela_ids();
    let de_volta = TabelaIds::de_texto(&t.para_texto()).expect("lê");
    assert_eq!(t, de_volta);
    assert_eq!(t.len(), doc.total_de_notas());
    assert!(matches!(
        TabelaIds::de_texto("lixo"),
        Err(ErroDocumento::TabelaIdsInvalida(_))
    ));
    assert!(matches!(
        TabelaIds::de_texto("piano-tutor-ids v1\nproximo=2\nxml\ta\t7\n"),
        Err(ErroDocumento::TabelaIdsInvalida(_))
    ));
}

#[test]
fn tabela_amarrada_ao_texto_e_recusada_se_o_arquivo_mudou() {
    let f = fixture("duas-vozes");
    let mut doc = Documento::abrir(&f.conteudo).expect("abre");
    let id = doc.notas().next().expect("nota").id;
    doc.definir_altura(
        id,
        Altura {
            passo: 'G',
            alter: 0,
            oitava: 5,
        },
    )
    .expect("edita");
    let saida = doc.exportar().expect("exporta");

    // Gravada para o texto exportado: reabre com os mesmos ids.
    let lateral = doc.tabela_ids_para(&saida);
    let reaberto = Documento::abrir_com_ids(&saida, Some(&lateral)).expect("reabre");
    assert!(doc.notas().map(|n| n.id).eq(reaberto.notas().map(|n| n.id)));

    // A tabela do texto original não vale para o exportado (nem vice-versa): recusa explícita.
    assert!(matches!(
        Documento::abrir_com_ids(&saida, Some(&doc.tabela_ids())),
        Err(ErroDocumento::TabelaIdsDeOutroTexto)
    ));
    // Arquivo editado por fora (nota removida na mão): a tabela antiga é recusada.
    let removida = saida.replacen("<note><pitch><step>G</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>half</type><stem>up</stem></note>", "", 1);
    assert_ne!(removida, saida);
    assert!(matches!(
        Documento::abrir_com_ids(&removida, Some(&lateral)),
        Err(ErroDocumento::TabelaIdsDeOutroTexto)
    ));
}

#[test]
fn chave_por_posicao_sem_amarra_ao_texto_deslocaria_os_ids_por_isso_a_amarra_e_obrigatoria() {
    let f = fixture("duas-vozes");
    let doc = Documento::abrir(&f.conteudo).expect("abre");
    let t = sem_base(&doc.tabela_ids());
    let removida = f.conteudo.replacen("<note><pitch><step>E</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>half</type><stem>up</stem></note>", "", 1);
    assert_ne!(removida, f.conteudo);
    let reaberto = Documento::abrir_com_ids(&removida, Some(&t)).expect("abre sem amarra");
    // A 1ª nota que sobrou é a antiga 2ª, mas herda o id da antiga 1ª: erro silencioso.
    let primeira = reaberto.notas().next().expect("nota");
    assert_eq!(primeira.altura.map(|a| a.passo), Some('D'));
    assert_eq!(primeira.id, IdNota(1));
}
