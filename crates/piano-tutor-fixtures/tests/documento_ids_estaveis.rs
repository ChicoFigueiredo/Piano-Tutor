//! Ticket 17 (E01), notas do supervisor: o arquivo lateral de `IdNota` (ligado ao texto por
//! impressão digital) faz a identidade das notas sobreviver à releitura e às Versões; a
//! conferência da exportação volta a ser completa (identidades e não só a contagem de notas) em
//! edições que criam ou tiram notas; e o diff pareia as notas pelo `IdNota` estável.

use piano_tutor_core::documento::{
    Altura, ComandoDeEdicao, DocumentoAberto, ErroServico, IdNota, MudancaDeNota, TabelaIds,
    diff_de_documentos,
};
use piano_tutor_core::partitura::{Figura, Fracao};
use piano_tutor_fixtures::corpus::{corpus, fixture};

fn abrir(texto: &str) -> DocumentoAberto {
    DocumentoAberto::abrir_texto("t", texto).expect("abre")
}

/// Todas as identidades do modelo de tocar, na ordem do layout.
fn ids_do_layout(d: &DocumentoAberto) -> Vec<Option<IdNota>> {
    let mut v = Vec::new();
    for (pi, parte) in d.partitura().partes.iter().enumerate() {
        for (ci, c) in parte.compassos.iter().enumerate() {
            for oi in 0..c.notas.len() {
                v.push(d.ids().id(pi, ci, oi));
            }
        }
    }
    v
}

/// Um compasso de 4/4 (divisions 1) sem `id` nas notas.
fn peca_sem_ids(medida: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<score-partwise version=\"4.0\">\n  \
         <part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list>\n  \
         <part id=\"P1\">\n    <measure number=\"1\"><attributes><divisions>1</divisions>\
         <key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time>\
         </attributes>\n{medida}    </measure>\n  </part>\n</score-partwise>\n"
    )
}

fn nota_xml(passo: char, duracao: u32, tipo: &str) -> String {
    format!(
        "      <note><pitch><step>{passo}</step><octave>4</octave></pitch><duration>{duracao}\
         </duration><voice>1</voice><type>{tipo}</type></note>\n"
    )
}

fn pausa_xml(duracao: u32, tipo: &str) -> String {
    format!(
        "      <note><rest/><duration>{duracao}</duration><voice>1</voice><type>{tipo}</type>\
         </note>\n"
    )
}

fn re4() -> Altura {
    Altura {
        passo: 'D',
        alter: 0,
        oitava: 4,
    }
}

#[test]
fn edicao_que_cria_notas_reabre_com_as_mesmas_identidades_pelo_arquivo_lateral() {
    // Sem `id` no MusicXML: só a tabela lateral guarda quem é quem depois de criar pausas.
    let texto = peca_sem_ids(&format!(
        "{}{}{}",
        nota_xml('D', 2, "half"),
        nota_xml('E', 1, "quarter"),
        nota_xml('F', 1, "quarter")
    ));
    let mut d = abrir(&texto);
    let primeira = d.documento().notas().next().expect("nota").id;
    // Encurta a mínima: nasce uma pausa de semínima no meio do compasso.
    d.aplicar(ComandoDeEdicao::MudarDuracao {
        nota: primeira,
        figura: Figura::Seminima,
    })
    .expect("encurta");
    let saida = d.exportar().expect("exporta");
    let texto_lateral = d.tabela_ids_para(&saida).para_texto();

    // Sem a tabela, a releitura numera na ordem do arquivo e as identidades se deslocam.
    let sem_tabela = abrir(&saida);
    assert_ne!(ids_do_layout(&sem_tabela), ids_do_layout(&d));

    // Com a tabela (lida do texto do arquivo lateral), as identidades são as mesmas.
    let lida = TabelaIds::de_texto(&texto_lateral).expect("lê o lateral");
    let reaberto = DocumentoAberto::abrir_texto_com_ids("t", &saida, Some(&lida)).expect("reabre");
    assert_eq!(ids_do_layout(&reaberto), ids_do_layout(&d));
    assert_eq!(reaberto.partitura(), d.partitura());
    // E o contador continua de onde parou: a próxima nota nunca reutiliza um id.
    assert_eq!(reaberto.tabela_ids().proximo(), d.tabela_ids().proximo());
}

#[test]
fn conferir_exportacao_volta_a_conferir_as_identidades_em_edicao_que_cria_notas() {
    let texto = peca_sem_ids(&format!(
        "{}{}{}",
        nota_xml('D', 2, "half"),
        nota_xml('E', 1, "quarter"),
        nota_xml('F', 1, "quarter")
    ));
    let mut d = abrir(&texto);
    let primeira = d.documento().notas().next().expect("nota").id;
    d.aplicar(ComandoDeEdicao::MudarDuracao {
        nota: primeira,
        figura: Figura::Seminima,
    })
    .expect("encurta");
    let saida = d.exportar().expect("exporta");
    // A conferência completa aceita o texto gravado (com a tabela que o serviço sabe montar).
    d.conferir_exportacao(&saida).expect("confere");
    // E continua recusando um texto de outro modelo.
    let outro = saida.replacen("<step>E</step>", "<step>G</step>", 1);
    let e = d.conferir_exportacao(&outro).expect_err("difere");
    assert!(matches!(e, ErroServico::ReleituraDifere(_)), "{e:?}");
}

#[test]
fn tabela_de_outro_texto_e_recusada_na_abertura_pelo_servico() {
    let a = peca_sem_ids(&nota_xml('C', 4, "whole"));
    let b = peca_sem_ids(&nota_xml('D', 4, "whole"));
    let da = abrir(&a);
    let e = DocumentoAberto::abrir_texto_com_ids("b", &b, Some(&da.tabela_ids()))
        .expect_err("tabela de outro texto");
    assert!(e.to_string().to_lowercase().contains("tabela"), "{e}");
}

#[test]
fn tabela_antiga_sem_serie_continua_legivel() {
    let t =
        TabelaIds::de_texto("piano-tutor-ids v1\nproximo=3\npos\tP1\t0\t0\t1\npos\tP1\t0\t1\t2\n")
            .expect("lê");
    assert_eq!(t.len(), 2);
    assert_eq!(t.proximo(), 3);
}

#[test]
fn tabela_ida_e_volta_no_texto_guarda_a_serie() {
    let d = abrir(&peca_sem_ids(&nota_xml('C', 4, "whole")));
    let t = d.tabela_ids();
    let de_volta = TabelaIds::de_texto(&t.para_texto()).expect("lê");
    assert_eq!(de_volta, t);
}

#[test]
fn corpus_toda_tabela_gravada_reabre_o_mesmo_documento_com_as_mesmas_identidades() {
    for f in corpus() {
        let d = abrir(&f.conteudo);
        let lateral = d.tabela_ids_para(&f.conteudo);
        let lida =
            TabelaIds::de_texto(&lateral.para_texto()).unwrap_or_else(|e| panic!("{}: {e}", f.id));
        let r = DocumentoAberto::abrir_texto_com_ids(f.id, &f.conteudo, Some(&lida))
            .unwrap_or_else(|e| panic!("{}: {e}", f.id));
        assert_eq!(ids_do_layout(&r), ids_do_layout(&d), "{}", f.id);
        assert_eq!(r.exportar().expect("exporta"), f.conteudo, "{}", f.id);
    }
}

#[test]
fn diff_pareia_pelo_idnota_estavel_na_insercao_no_meio_do_compasso_sem_id_no_musicxml() {
    // Antes: [Ré (1), pausa (2), Ré (1)]. Insere um Ré (semínima) no tempo 1, em cima da pausa:
    // depois: [Ré, Ré, pausa (1), Ré]. Os dois Ré que não mudaram são idênticos em conteúdo, o
    // que enganaria um pareamento por posição ou por conteúdo.
    let texto = peca_sem_ids(&format!(
        "{}{}{}",
        nota_xml('D', 1, "quarter"),
        pausa_xml(2, "half"),
        nota_xml('D', 1, "quarter")
    ));
    let mut a = abrir(&texto);
    let pausa = a.documento().partes()[0].compassos[0].notas[1].id;
    assert!(a.documento().nota(pausa).expect("pausa").pausa);
    let r = a
        .aplicar(ComandoDeEdicao::InserirNota {
            compasso: a.documento().posicao_da_nota(pausa).expect("pos"),
            voz: "1".into(),
            pauta: "1".into(),
            posicao: Fracao::inteiro(1),
            altura: re4(),
            acidente: None,
            figura: Figura::Seminima,
            pontos: 0,
        })
        .expect("insere");
    assert!(r.alterou);
    let saida = a.exportar().expect("exporta");
    let lateral = a.tabela_ids_para(&saida);

    // O "antes" é o texto original aberto sem tabela; o "depois" é o exportado com a tabela.
    let antes = abrir(&texto);
    let depois = DocumentoAberto::abrir_texto_com_ids("d", &saida, Some(&lateral)).expect("abre");
    assert!(antes.documento().ids_comparaveis(depois.documento()));
    let diff = diff_de_documentos(antes.documento(), depois.documento());
    assert_eq!(diff.compassos.len(), 1);
    let ms = &diff.compassos[0].mudancas;
    // A pausa de mínima foi substituída (removida) e nasceram uma nota e uma pausa; os dois Ré
    // que não mudaram não aparecem e nada é "alterado" por adivinhação de ordem.
    let (mut inseridas, mut removidas, mut alteradas) = (0, 0, 0);
    for m in ms {
        match m {
            MudancaDeNota::Inserida(_) => inseridas += 1,
            MudancaDeNota::Removida(_) => removidas += 1,
            MudancaDeNota::Alterada { .. } => alteradas += 1,
        }
    }
    assert_eq!((inseridas, removidas, alteradas), (2, 1, 0), "{ms:?}");

    // Sem a tabela as identidades não são comparáveis e o diff volta ao pareamento de sempre.
    let sem = abrir(&saida);
    assert!(!antes.documento().ids_comparaveis(sem.documento()));
}

#[test]
fn diff_com_ids_estaveis_mostra_a_nota_removida_como_alterada_para_pausa() {
    let texto = peca_sem_ids(&format!(
        "{}{}{}",
        nota_xml('C', 1, "quarter"),
        nota_xml('D', 1, "quarter"),
        nota_xml('E', 2, "half")
    ));
    let mut a = abrir(&texto);
    let meio = a.documento().partes()[0].compassos[0].notas[1].id;
    a.aplicar(ComandoDeEdicao::RemoverNota { nota: meio })
        .expect("remove");
    let saida = a.exportar().expect("exporta");
    let lateral = a.tabela_ids_para(&saida);
    let antes = abrir(&texto);
    let depois = DocumentoAberto::abrir_texto_com_ids("d", &saida, Some(&lateral)).expect("abre");
    let diff = diff_de_documentos(antes.documento(), depois.documento());
    assert_eq!(diff.compassos.len(), 1);
    assert_eq!(diff.compassos[0].mudancas.len(), 1);
    assert!(matches!(
        diff.compassos[0].mudancas[0],
        MudancaDeNota::Alterada { .. }
    ));
}

#[test]
fn fixture_real_a_tabela_do_texto_original_pareia_o_documento_consigo_mesmo_sem_diferencas() {
    let f = fixture("duas-vozes");
    let d = abrir(&f.conteudo);
    let r = DocumentoAberto::abrir_texto_com_ids("r", &f.conteudo, Some(&d.tabela_ids()))
        .expect("reabre");
    assert!(d.documento().ids_comparaveis(r.documento()));
    assert!(diff_de_documentos(d.documento(), r.documento()).vazio());
}
