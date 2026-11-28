//! "Continuar a partir desta Versão" (achado R3 da revisão independente da E01 e critério do
//! ticket 21, "abrir Versão antiga ... como novo ponto de partida").

use super::testes::{banco, com_oficina, editar_uma_nota, quadro};
use super::testes_revisao::{
    criada, editar_outra_nota, midis_do_compasso_1, reabrir_com_rascunho_pendente,
};
use super::*;

#[test]
fn continuar_a_partir_da_versao_edita_a_versao_e_a_proxima_e_filha_dela() {
    let (_d, mut e, xml) = com_oficina();
    editar_uma_nota(&mut e);
    let v1 = criada(e.salvar_versao().expect("salva v1"));
    let m1 = midis_do_compasso_1(&e);
    // Sessão 2: abre a Hipótese, mas escolhe continuar a partir de v1 na lista.
    let mut e2 = EstadoPartitura::novo(None);
    e2.abrir_caminho(&xml.to_string_lossy());
    assert_ne!(
        midis_do_compasso_1(&e2),
        m1,
        "a Hipótese ainda não tem a edição de v1"
    );
    assert!(e2.motivo_sem_continuar_da_versao().is_none());
    e2.continuar_a_partir_da_versao(&v1.hash);
    assert_eq!(midis_do_compasso_1(&e2), m1, "o documento é o de v1");
    assert!(e2.versao_em_leitura().is_none(), "é edição, não leitura");
    assert!(!e2.documento.as_ref().expect("doc").modificado());
    assert!(e2.pode_salvar_versao());
    assert!(e2.aviso_de_versoes().expect("aviso").contains("filha dela"));
    // Sem editar, salvar não cria Versão (igual ao pai).
    assert!(matches!(
        e2.salvar_versao().expect("ok"),
        ResultadoDoSalvar::SemMudancas { .. }
    ));
    editar_outra_nota(&mut e2);
    let v2 = criada(e2.salvar_versao().expect("salva v2"));
    assert_eq!(
        v2.pai.as_deref(),
        Some(v1.hash.as_str()),
        "v2 é filha de v1"
    );
    // v2 contém a edição de v1 (a primeira nota) além da sua.
    let m2 = midis_do_compasso_1(&e2);
    assert_eq!(m2[0], m1[0]);
    assert_ne!(m2[1], m1[1]);
    // A identidade das notas segue a de v1: o diff v1→v2 pareia por IdNota.
    let versoes = e2.versoes.clone().expect("oficina");
    assert!(versoes.ids_comparaveis(&v1.hash, &v2.hash));
    let d = crate::versoes_diff::diff_entre_versoes(&versoes, &v1.hash, &v2.hash).expect("diff");
    assert!(!d.diff.vazio());
}

#[test]
fn continuar_a_partir_da_hipotese_depois_de_uma_versao_faz_a_filha_da_hipotese() {
    let (_d, mut e, _) = com_oficina();
    let hipotese = hash_do_conteudo(EMBUTIDAS[3].1.as_bytes());
    editar_uma_nota(&mut e);
    criada(e.salvar_versao().expect("v1"));
    e.continuar_a_partir_da_versao(&hipotese);
    assert_eq!(e.documento.as_ref().expect("doc").xml(), EMBUTIDAS[3].1);
    editar_outra_nota(&mut e);
    let v2 = criada(e.salvar_versao().expect("v2"));
    assert_eq!(v2.pai.as_deref(), Some(hipotese.as_str()));
}

#[test]
fn continuar_recusa_com_edicoes_nao_salvas_e_nada_muda() {
    let (_d, mut e, _) = com_oficina();
    editar_uma_nota(&mut e);
    let v1 = criada(e.salvar_versao().expect("v1"));
    editar_outra_nota(&mut e);
    let editado = e.documento.as_ref().expect("doc").exportado().expect("x");
    assert!(e.motivo_sem_continuar_da_versao().is_some());
    e.continuar_a_partir_da_versao(&v1.hash);
    let aviso = e.aviso_de_versoes().expect("aviso");
    assert!(aviso.contains("edições não salvas"), "{aviso}");
    assert_eq!(
        e.documento.as_ref().expect("doc").exportado().expect("x"),
        editado,
        "o trabalho em curso fica como estava"
    );
    // Também se as edições estão guardadas enquanto se lê outra Versão.
    e.abrir_versao(&v1.hash);
    assert!(e.versao_em_leitura().is_some());
    assert!(e.motivo_sem_continuar_da_versao().is_some());
    e.continuar_a_partir_da_versao(&v1.hash);
    assert!(e.versao_em_leitura().is_some(), "segue em leitura");
    e.voltar_da_leitura();
    assert_eq!(
        e.documento.as_ref().expect("doc").exportado().expect("x"),
        editado
    );
}

#[test]
fn continuar_a_partir_de_uma_versao_aberta_em_leitura_vira_edicao() {
    let (_d, mut e, _) = com_oficina();
    editar_uma_nota(&mut e);
    let v1 = criada(e.salvar_versao().expect("v1"));
    e.abrir_versao(&v1.hash);
    assert!(e.versao_em_leitura().is_some());
    assert!(e.motivo_sem_continuar_da_versao().is_none());
    e.continuar_a_partir_da_versao(&v1.hash);
    assert!(e.versao_em_leitura().is_none());
    assert!(e.pode_salvar_versao());
    editar_outra_nota(&mut e);
    let v2 = criada(e.salvar_versao().expect("v2"));
    assert_eq!(v2.pai.as_deref(), Some(v1.hash.as_str()));
}

#[test]
fn continuar_recusa_com_rascunho_pendente() {
    let (_d, mut e, _xml, _) = reabrir_com_rascunho_pendente();
    let hipotese = hash_do_conteudo(EMBUTIDAS[3].1.as_bytes());
    assert!(
        e.motivo_sem_continuar_da_versao()
            .expect("motivo")
            .contains("recupere")
    );
    e.continuar_a_partir_da_versao(&hipotese);
    assert!(e.rascunho_pendente().is_some(), "o banner segue");
}

#[test]
fn continuar_de_versao_inexistente_ou_em_arquivo_solto_so_avisa() {
    let (_d, mut e, _) = com_oficina();
    editar_uma_nota(&mut e);
    criada(e.salvar_versao().expect("v1"));
    let antes = e.documento.as_ref().expect("doc").exportado().expect("x");
    e.continuar_a_partir_da_versao(&"0".repeat(64));
    assert!(e.aviso_de_versoes().expect("aviso").contains("Versão"));
    assert_eq!(
        e.documento.as_ref().expect("doc").exportado().expect("x"),
        antes
    );
    let solto = EstadoPartitura::novo(None);
    assert!(solto.motivo_sem_continuar_da_versao().is_some());
}

#[test]
fn a_lista_desenha_o_botao_continuar_sem_quebrar_o_quadro() {
    let (mut som, ctx) = banco();
    let (_d, mut e, _) = com_oficina();
    editar_uma_nota(&mut e);
    criada(e.salvar_versao().expect("v1"));
    quadro(&ctx, &mut e, &mut som);
    editar_outra_nota(&mut e);
    quadro(&ctx, &mut e, &mut som);
    assert!(e.motivo_sem_continuar_da_versao().is_some());
}
