//! Testes da correção dos achados R1, R2 e R3 da revisão independente da E01
//! (`docs/evolucao-musical/e01/REVISAO-CODIGO.md`): cada um nasceu como a sonda que reproduz.

use super::testes::{arquivo_do_rascunho, com_oficina, editar_uma_nota, tick_ate_gravar};
use super::*;

/// Edita uma nota diferente da de `editar_uma_nota` (a segunda, para baixo).
pub(super) fn editar_outra_nota(e: &mut EstadoPartitura) {
    e.navegar(Comando::Seguinte);
    e.navegar(Comando::Seguinte);
    assert!(
        e.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Abaixo)
            .is_some()
    );
}

pub(super) fn midis_do_compasso_1(e: &EstadoPartitura) -> Vec<Option<u8>> {
    let d = e.documento.as_ref().expect("doc");
    d.partitura().partes[0].compassos[0]
        .notas
        .iter()
        .map(|n| n.midi)
        .collect()
}

/// A Versão criada, ou falha o teste.
pub(super) fn criada(r: ResultadoDoSalvar) -> Versao {
    match r {
        ResultadoDoSalvar::Criada(v) => v,
        outro => panic!("esperava Criada, veio {outro:?}"),
    }
}

// --- R1 (P0): salvar Versão não apaga o rascunho de recuperação pendente ------------------------

/// Deixa um rascunho de "sessão que caiu" no disco e reabre a partitura (rascunho pendente).
pub(super) fn reabrir_com_rascunho_pendente()
-> (tempfile::TempDir, EstadoPartitura, PathBuf, String) {
    let (d, mut e, xml) = com_oficina();
    editar_uma_nota(&mut e);
    let editado = e.documento.as_ref().expect("doc").exportado().expect("x");
    e.autosave_tick(0);
    let iv = e.config_autosave.intervalo_ms;
    tick_ate_gravar(&mut e, iv);
    drop(e); // "queda": o estado em memória some
    let mut novo = EstadoPartitura::novo(None);
    novo.abrir_caminho(&xml.to_string_lossy());
    assert!(novo.rascunho_pendente().is_some());
    (d, novo, xml, editado)
}

#[test]
fn r1_salvar_versao_fica_bloqueado_com_dica_enquanto_ha_rascunho_pendente() {
    let (_d, mut e, xml, editado) = reabrir_com_rascunho_pendente();
    e.navegar(Comando::Seguinte);
    e.navegar(Comando::Seguinte);
    assert!(
        e.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Abaixo)
            .is_some()
    );
    assert!(!e.pode_salvar_versao(), "salvar fica bloqueado");
    let dica = e.motivo_sem_salvar_versao().expect("dica");
    assert!(
        dica.contains("recupere") && dica.contains("descarte"),
        "a dica manda recuperar ou descartar primeiro: {dica}"
    );
    assert!(e.salvar_versao().is_err());
    // Nada foi apagado nem criado: o rascunho segue, o banner segue, nenhuma Versão.
    assert!(arquivo_do_rascunho(&xml).is_file(), "o rascunho sobrevive");
    assert!(e.rascunho_pendente().is_some());
    assert!(
        e.lista_de_versoes().is_empty(),
        "nem a origem foi registrada"
    );
    // E "Recuperar" ainda recupera o trabalho da sessão que caiu.
    e.recuperar_rascunho();
    assert!(e.rascunho_pendente().is_none());
    assert_eq!(
        e.documento.as_ref().expect("doc").exportado().expect("x"),
        editado
    );
    assert!(e.aviso_de_versoes().expect("aviso").contains("desfazer"));
    // Decidido o rascunho, salvar volta a valer e apaga o rascunho (agora é o trabalho dele).
    assert!(e.pode_salvar_versao());
    e.salvar_versao().expect("salva");
    assert!(!arquivo_do_rascunho(&xml).exists());
}

#[test]
fn r1_descartar_o_rascunho_pendente_libera_o_salvar() {
    let (_d, mut e, xml, _) = reabrir_com_rascunho_pendente();
    assert!(!e.pode_salvar_versao());
    e.descartar_rascunho();
    assert!(!arquivo_do_rascunho(&xml).exists());
    assert!(e.pode_salvar_versao());
    assert!(e.motivo_sem_salvar_versao().is_none());
    editar_uma_nota(&mut e);
    e.salvar_versao().expect("salva");
    assert_eq!(e.lista_de_versoes().len(), 2);
}

// --- R3 (P1): o pai da Versão é a base do documento, não a ponta ---------------------------------

#[test]
fn r3_duas_sessoes_a_segunda_parte_da_hipotese_e_o_pai_e_a_hipotese() {
    let (_d, mut e, xml) = com_oficina();
    let hipotese = hash_do_conteudo(EMBUTIDAS[3].1.as_bytes());
    editar_uma_nota(&mut e);
    let v1 = criada(e.salvar_versao().expect("salva v1"));
    assert_eq!(v1.pai.as_deref(), Some(hipotese.as_str()));
    let m1 = midis_do_compasso_1(&e);
    // Sessão 2: reabre o arquivo da Hipótese (a Oficina sempre abre o `<id>.musicxml`).
    let mut e2 = EstadoPartitura::novo(None);
    e2.abrir_caminho(&xml.to_string_lossy());
    editar_outra_nota(&mut e2);
    let m2 = midis_do_compasso_1(&e2);
    let v2 = criada(e2.salvar_versao().expect("salva v2"));
    assert_ne!(m1, m2, "v2 não contém a edição de v1");
    assert_eq!(
        v2.pai.as_deref(),
        Some(hipotese.as_str()),
        "v2 partiu da Hipótese: o pai é a Hipótese, não a ponta v1"
    );
    assert_ne!(v2.pai.as_deref(), Some(v1.hash.as_str()));
}

#[test]
fn r3_na_mesma_sessao_a_cadeia_continua_pai_e_a_versao_anterior() {
    let (_d, mut e, _) = com_oficina();
    editar_uma_nota(&mut e);
    let v1 = criada(e.salvar_versao().expect("v1"));
    editar_outra_nota(&mut e);
    let v2 = criada(e.salvar_versao().expect("v2"));
    assert_eq!(v2.pai.as_deref(), Some(v1.hash.as_str()));
}

#[test]
fn r3_salvar_sem_mudar_a_partir_da_hipotese_nao_cria_versao_mesmo_com_ponta_diferente() {
    let (_d, mut e, xml) = com_oficina();
    editar_uma_nota(&mut e);
    criada(e.salvar_versao().expect("v1"));
    let mut e2 = EstadoPartitura::novo(None);
    e2.abrir_caminho(&xml.to_string_lossy());
    assert!(matches!(
        e2.salvar_versao().expect("ok"),
        ResultadoDoSalvar::SemMudancas { .. }
    ));
    assert_eq!(e2.lista_de_versoes().len(), 2, "só Hipótese e v1");
}
