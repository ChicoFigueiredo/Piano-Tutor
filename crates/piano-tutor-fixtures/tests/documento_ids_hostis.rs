//! Arquivo lateral de ids hostil (achados R4 e R6 da revisão independente da E01,
//! `docs/evolucao-musical/e01/REVISAO-CODIGO.md`): contador no teto de `u64`, ids repetidos e ids
//! fora do contador são recusados como tabela inválida, nunca derrubam o programa, e o texto abre
//! do mesmo jeito sem a tabela (as identidades vêm da leitura, o diff cai no conteúdo).

use piano_tutor_core::documento::{
    ComandoDeEdicao, DocumentoAberto, ErroDocumento, ErroServico, IdNota, SentidoDaAltura,
    TETO_DO_CONTADOR_DE_IDS, TabelaIds, UnidadeDeAltura,
};

fn peca() -> String {
    let nota = |p: char| {
        format!(
            "<note><pitch><step>{p}</step><octave>4</octave></pitch><duration>1</duration>\
             <voice>1</voice><type>quarter</type></note>"
        )
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<score-partwise version=\"4.0\">\
         <part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list>\
         <part id=\"P1\"><measure number=\"1\"><attributes><divisions>1</divisions>\
         <time><beats>4</beats><beat-type>4</beat-type></time></attributes>{}{}{}{}</measure>\
         </part></score-partwise>\n",
        nota('C'),
        nota('D'),
        nota('E'),
        nota('F')
    )
}

fn invalida(r: Result<TabelaIds, ErroDocumento>) -> bool {
    matches!(r, Err(ErroDocumento::TabelaIdsInvalida(_)))
}

#[test]
fn r4_contador_no_maximo_de_u64_e_recusado_e_nao_derruba_a_abertura() {
    let hostil = "piano-tutor-ids v1\nproximo=18446744073709551615\n";
    assert!(invalida(TabelaIds::de_texto(hostil)));
    // Sem a tabela, o texto abre e edita normalmente.
    let mut d = DocumentoAberto::abrir_texto("t", &peca()).expect("abre sem a tabela");
    assert_eq!(d.documento().total_de_notas(), 4);
    let id = d.ids().id(0, 0, 0).expect("id");
    d.aplicar(ComandoDeEdicao::MudarAltura {
        nota: id,
        unidade: UnidadeDeAltura::Semitom,
        sentido: SentidoDaAltura::Acima,
    })
    .expect("edita");
}

#[test]
fn r4_o_teto_do_contador_e_a_fronteira_exata() {
    let com = |p: u64| format!("piano-tutor-ids v1\nproximo={p}\n");
    assert!(TabelaIds::de_texto(&com(TETO_DO_CONTADOR_DE_IDS)).is_ok());
    assert!(invalida(TabelaIds::de_texto(&com(
        TETO_DO_CONTADOR_DE_IDS + 1
    ))));
    assert!(invalida(TabelaIds::de_texto(&com(u64::MAX - 1))));
}

#[test]
fn r4_contador_no_teto_nao_deixa_alocar_nem_inserir_e_nao_entra_em_panico() {
    // Um contador legítimo no limite: a abertura vale, mas nenhuma conta estoura.
    let t = TabelaIds::de_texto(&format!(
        "piano-tutor-ids v1\nproximo={TETO_DO_CONTADOR_DE_IDS}\n"
    ))
    .expect("no limite vale");
    let r = std::panic::catch_unwind(|| {
        // As notas do texto precisam de ids novos além do teto: recusa com erro, sem pânico.
        DocumentoAberto::abrir_texto_com_ids("t", &peca(), Some(&t)).map(|d| d.ids().total())
    });
    let r = r.expect("sem pânico");
    assert!(
        matches!(
            r,
            Err(ErroServico::Documento(
                ErroDocumento::Overflow(_) | ErroDocumento::TabelaIdsInvalida(_)
            ))
        ),
        "{r:?}"
    );
}

#[test]
fn r6_ids_repetidos_sao_recusados_como_tabela_invalida() {
    let lateral = "piano-tutor-ids v1\nproximo=10\n\
        pos\tP1\t0\t0\t1\npos\tP1\t0\t1\t1\npos\tP1\t0\t2\t1\npos\tP1\t0\t3\t1\n";
    assert!(invalida(TabelaIds::de_texto(lateral)));
    // Também entre chaves de espécies diferentes (id do MusicXML e posição).
    let misto = "piano-tutor-ids v1\nproximo=10\nxml\ta\t3\npos\tP1\t0\t1\t3\n";
    assert!(invalida(TabelaIds::de_texto(misto)));
}

#[test]
fn r6_id_igual_ou_maior_que_o_contador_e_recusado_em_qualquer_ordem_das_linhas() {
    // O contador depois das notas (o formato do núcleo grava antes, mas o leitor não deve
    // depender da ordem): o id 7 passa do contador 2 e continua sendo recusado.
    assert!(invalida(TabelaIds::de_texto(
        "piano-tutor-ids v1\nxml\ta\t7\nproximo=2\n"
    )));
    assert!(invalida(TabelaIds::de_texto(
        "piano-tutor-ids v1\nproximo=2\nxml\ta\t7\n"
    )));
    assert!(invalida(TabelaIds::de_texto(
        "piano-tutor-ids v1\nproximo=7\nxml\ta\t7\n"
    )));
}

#[test]
fn tabela_gravada_pelo_nucleo_continua_valendo_depois_de_editar_e_reabrir() {
    let mut d = DocumentoAberto::abrir_texto("t", &peca()).expect("abre");
    let id = d.ids().id(0, 0, 1).expect("id");
    d.aplicar(ComandoDeEdicao::RemoverNota { nota: id })
        .expect("remove");
    let texto = d.exportar().expect("exporta");
    let tabela = d.tabela_ids_para(&texto);
    let de_volta = TabelaIds::de_texto(&tabela.para_texto()).expect("a tabela do núcleo vale");
    let reaberto =
        DocumentoAberto::abrir_texto_com_ids("t", &texto, Some(&de_volta)).expect("reabre");
    assert_eq!(reaberto.documento().total_de_notas(), 4);
    assert_ne!(reaberto.ids().id(0, 0, 0), Some(IdNota(0)));
}
