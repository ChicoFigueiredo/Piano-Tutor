//! Testes da correção dos achados N1, N2, N3, N4 e N9 da segunda revisão independente da E01
//! (`docs/evolucao-musical/e01/REVISAO-CODIGO.md`, "Segunda revisão"): cada um nasceu como a
//! sonda de `sondas-revisao/zz_revisao2_app_tela_partitura.rs` que reproduz o achado.

use super::testes::{arquivo_do_rascunho, com_oficina, editar_uma_nota, tick_ate_gravar};
use super::testes_revisao::{criada, editar_outra_nota, reabrir_com_rascunho_pendente};
use super::*;

/// Sessão que caiu com trabalho não salvo depois de uma Versão `v1`: devolve a sessão nova, com o
/// banner pendente, o caminho da Hipótese, o texto do trabalho que caiu e o hash de `v1`.
fn reabrir_com_versao_e_rascunho() -> (tempfile::TempDir, EstadoPartitura, PathBuf, String, String)
{
    let (d, mut e, xml) = com_oficina();
    editar_uma_nota(&mut e);
    let v1 = criada(e.salvar_versao().expect("salva v1"));
    editar_outra_nota(&mut e);
    let editado = e.documento.as_ref().expect("doc").exportado().expect("x");
    e.autosave_tick(0);
    let iv = e.config_autosave.intervalo_ms;
    tick_ate_gravar(&mut e, iv);
    drop(e); // "queda"
    let mut n = EstadoPartitura::novo(None);
    n.abrir_caminho(&xml.to_string_lossy());
    assert!(n.rascunho_pendente().is_some());
    (d, n, xml, editado, v1.hash)
}

fn texto_do_documento(e: &EstadoPartitura) -> String {
    e.documento.as_ref().expect("doc").exportado().expect("x")
}

// --- N1 (P1): Recuperar com uma Versão aberta em leitura ----------------------------------------

#[test]
fn n1_recuperar_com_versao_em_leitura_sai_da_leitura_e_instala_o_rascunho_em_edicao() {
    let (_d, mut n, xml, editado, v1) = reabrir_com_versao_e_rascunho();
    n.abrir_versao(&v1);
    assert!(n.versao_em_leitura().is_some());
    assert!(n.rascunho_pendente().is_some(), "o banner segue em leitura");
    n.recuperar_rascunho();
    assert!(
        n.versao_em_leitura().is_none(),
        "Recuperar sai do modo leitura"
    );
    assert!(!n.nome().contains("(leitura)"), "nome: {}", n.nome());
    assert_eq!(texto_do_documento(&n), editado, "o trabalho recuperado");
    assert!(n.documento.as_ref().expect("doc").modificado());
    assert!(n.rascunho_pendente().is_none());
    // "Voltar à edição" depois de recuperar não existe mais e não perde nada.
    n.voltar_da_leitura();
    assert_eq!(texto_do_documento(&n), editado);
    // O rascunho da sessão que caiu segue em disco até virar Versão ou ser descartado.
    assert!(arquivo_do_rascunho(&xml).is_file());
    // Edita (agora é permitido) e deixa o autosave rodar: o rascunho cobre o trabalho inteiro.
    n.navegar(Comando::Seguinte);
    assert!(
        n.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Acima)
            .is_some(),
        "{:?}",
        n.ultima_edicao()
    );
    let depois = texto_do_documento(&n);
    n.autosave_tick(10_000_000);
    let iv = n.config_autosave.intervalo_ms;
    tick_ate_gravar(&mut n, 10_000_000 + iv);
    let em_disco = std::fs::read_to_string(arquivo_do_rascunho(&xml)).expect("rascunho");
    assert!(em_disco.contains(&depois), "o rascunho tem o trabalho todo");
    // Salvar vira Versão com o trabalho recuperado (mais a edição nova).
    let v2 = criada(n.salvar_versao().expect("salva"));
    assert_eq!(v2.pai.as_deref(), Some(v1.as_str()));
    assert!(!arquivo_do_rascunho(&xml).exists(), "agora virou Versão");
}

#[test]
fn n1_descartar_em_leitura_apaga_so_o_rascunho_e_voltar_devolve_a_edicao() {
    let (_d, mut n, xml, editado, v1) = reabrir_com_versao_e_rascunho();
    let original = texto_do_documento(&n);
    assert_ne!(original, editado);
    n.abrir_versao(&v1);
    n.descartar_rascunho();
    assert!(n.rascunho_pendente().is_none());
    assert!(!arquivo_do_rascunho(&xml).exists(), "descarte explícito");
    assert!(n.versao_em_leitura().is_some(), "segue lendo a Versão");
    n.voltar_da_leitura();
    assert_eq!(texto_do_documento(&n), original);
    // Nada de rascunho fantasma: editar e deixar o autosave gravar funciona como sessão nova.
    editar_uma_nota(&mut n);
    n.autosave_tick(0);
    let iv = n.config_autosave.intervalo_ms;
    tick_ate_gravar(&mut n, iv);
    assert!(arquivo_do_rascunho(&xml).is_file());
}

#[test]
fn n1_abrir_versao_e_voltar_sem_recuperar_mantem_o_banner_e_o_rascunho() {
    let (_d, mut n, xml, _editado, v1) = reabrir_com_versao_e_rascunho();
    n.abrir_versao(&v1);
    n.voltar_da_leitura();
    assert!(n.rascunho_pendente().is_some());
    assert!(arquivo_do_rascunho(&xml).is_file());
    assert!(n.motivo_sem_salvar_versao().is_some());
}

// --- N2 (P2): Recuperar ou Descartar que falha ---------------------------------------------------

/// Deixa um rascunho íntegro (hash e tamanho) que não abre como MusicXML e reabre a partitura.
fn reabrir_com_rascunho_que_nao_abre() -> (tempfile::TempDir, EstadoPartitura, PathBuf) {
    let (d, e0, xml) = com_oficina();
    drop(e0);
    let pasta = xml.parent().expect("pasta").to_path_buf();
    let cfg = crate::autosave::ConfigAutosave::default();
    let a = crate::autosave::Autosave::em(&pasta, "ode", &cfg).expect("autosave");
    a.gravador()
        .gravar_com_ids("isto nao e musicxml", None, "2026-01-01T00:00:00Z")
        .expect("grava");
    drop(a);
    let mut n = EstadoPartitura::novo(None);
    n.abrir_caminho(&xml.to_string_lossy());
    assert!(n.rascunho_pendente().is_some());
    (d, n, xml)
}

#[test]
fn n2_recuperar_que_falha_mantem_o_rascunho_pendente_a_edicao_bloqueada_e_o_aviso() {
    let (_d, mut n, xml) = reabrir_com_rascunho_que_nao_abre();
    let antes = texto_do_documento(&n);
    n.recuperar_rascunho();
    assert!(n.rascunho_pendente().is_some(), "o banner continua");
    let aviso = n.aviso_de_versoes().expect("aviso visível");
    assert!(aviso.contains("Não foi possível recuperar"), "{aviso}");
    assert!(aviso.contains("Descarte"), "diz o que fazer: {aviso}");
    n.navegar(Comando::Seguinte);
    assert!(
        n.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Acima)
            .is_none(),
        "a edição segue bloqueada"
    );
    assert_eq!(texto_do_documento(&n), antes);
    assert!(n.salvar_versao().is_err());
    assert!(
        std::fs::read_to_string(arquivo_do_rascunho(&xml))
            .expect("rascunho")
            .contains("isto nao e musicxml"),
        "o rascunho ruim fica até o usuário descartar"
    );
    // Descartar explicitamente libera a edição e o autosave volta a gravar.
    n.descartar_rascunho();
    assert!(n.rascunho_pendente().is_none());
    editar_uma_nota(&mut n);
    n.autosave_tick(0);
    let iv = n.config_autosave.intervalo_ms;
    tick_ate_gravar(&mut n, iv);
    let em_disco = std::fs::read_to_string(arquivo_do_rascunho(&xml)).expect("rascunho");
    assert!(!em_disco.contains("isto nao e musicxml"));
    assert!(em_disco.contains("<score-partwise"));
}

#[cfg(unix)]
#[test]
fn n2_descartar_que_falha_mantem_o_rascunho_pendente_e_o_aviso() {
    let (_d, mut n, xml, _editado) = reabrir_com_rascunho_pendente();
    // O arquivo de identidades vira uma pasta não vazia: `remove_file` falha (disco que se
    // recusa a apagar).
    let ids = arquivo_do_rascunho(&xml).with_file_name("rascunho.ids");
    let _ = std::fs::remove_file(&ids);
    std::fs::create_dir_all(ids.join("dentro")).expect("pasta no lugar do arquivo");
    n.descartar_rascunho();
    assert!(
        n.rascunho_pendente().is_some(),
        "o descarte falhou: a decisão continua pendente"
    );
    let aviso = n.aviso_de_versoes().expect("aviso visível");
    assert!(aviso.contains("Não foi possível descartar"), "{aviso}");
    n.navegar(Comando::Seguinte);
    assert!(
        n.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Acima)
            .is_none(),
        "a edição segue bloqueada"
    );
    n.autosave_tick(10_000_000);
    assert!(n.rascunho_pendente().is_some());
}

// --- N3 (P2): exportar sobre o arquivo escolhido na lista de fontes -----------------------------

#[test]
fn n3_escolher_arquivo_solto_da_lista_protege_o_original_na_exportacao() {
    let d = tempfile::tempdir().expect("pasta");
    let pasta = d.path().join("partituras");
    std::fs::create_dir_all(&pasta).expect("cria");
    let solto = pasta.join("solta.musicxml");
    let texto = format!("{}<!-- a -->\n", EMBUTIDAS[3].1);
    std::fs::write(&solto, &texto).expect("escreve");
    let mut e = EstadoPartitura::novo(Some(d.path().to_path_buf()));
    let i = e
        .fontes
        .iter()
        .position(|f| f.caminho.is_some())
        .expect("fonte do disco");
    e.escolher(i);
    assert!(e.erro.is_none(), "{:?}", e.erro);
    e.navegar(Comando::Seguinte);
    assert!(
        e.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Acima)
            .is_some()
    );
    e.sobrescrever = true;
    let r = e.exportar(&solto.to_string_lossy());
    assert!(r.is_err(), "o arquivo escolhido não é sobrescrito: {r:?}");
    assert_eq!(std::fs::read_to_string(&solto).expect("lê"), texto);
    // Outro destino segue livre.
    assert!(
        e.exportar(&d.path().join("outra.musicxml").to_string_lossy())
            .is_ok()
    );
}

#[test]
fn n3_fonte_embutida_nao_registra_arquivo_de_origem_e_trocar_de_fonte_limpa_a_origem() {
    let mut e = EstadoPartitura::novo(None);
    e.escolher(0);
    assert!(e.arquivo_aberto.is_none());
    let d = tempfile::tempdir().expect("pasta");
    let solto = d.path().join("s.musicxml");
    std::fs::write(&solto, EMBUTIDAS[3].1).expect("escreve");
    e.abrir_caminho(&solto.to_string_lossy());
    assert!(e.arquivo_aberto.is_some());
    // Trocar de documento pela lista troca (ou limpa) a origem: nada de proteção velha.
    e.escolher(0);
    assert!(e.arquivo_aberto.is_none());
}

// --- N4 (P3): "Sobrescrever" preserva permissões; somente leitura e atalho são recusados --------

#[cfg(unix)]
#[test]
fn n4_sobrescrever_preserva_as_permissoes_do_destino() {
    use std::os::unix::fs::PermissionsExt;
    let (d, mut e, _xml) = com_oficina();
    editar_uma_nota(&mut e);
    let alvo = d.path().join("saida.musicxml");
    std::fs::write(&alvo, "ANTIGO").expect("escreve");
    std::fs::set_permissions(&alvo, std::fs::Permissions::from_mode(0o640)).expect("modo");
    e.sobrescrever = true;
    e.exportar(&alvo.to_string_lossy()).expect("sobrescreve");
    let modo = std::fs::metadata(&alvo).expect("meta").permissions().mode() & 0o777;
    assert_eq!(modo, 0o640, "o modo do destino foi preservado");
    assert_ne!(std::fs::read_to_string(&alvo).expect("lê"), "ANTIGO");
}

#[cfg(unix)]
#[test]
fn n4_sobrescrever_recusa_destino_somente_leitura_com_mensagem() {
    use std::os::unix::fs::PermissionsExt;
    let (d, mut e, _xml) = com_oficina();
    editar_uma_nota(&mut e);
    let alvo = d.path().join("protegido.musicxml");
    std::fs::write(&alvo, "ORIGINAL").expect("escreve");
    std::fs::set_permissions(&alvo, std::fs::Permissions::from_mode(0o444)).expect("modo");
    e.sobrescrever = true;
    let r = e.exportar(&alvo.to_string_lossy());
    let msg = r.expect_err("somente leitura é recusado");
    assert!(msg.contains("somente leitura"), "{msg}");
    assert_eq!(std::fs::read_to_string(&alvo).expect("lê"), "ORIGINAL");
    let modo = std::fs::metadata(&alvo).expect("meta").permissions().mode() & 0o777;
    assert_eq!(modo, 0o444);
}

#[cfg(unix)]
#[test]
fn n4_sobrescrever_recusa_link_simbolico_como_destino() {
    let (d, mut e, _xml) = com_oficina();
    editar_uma_nota(&mut e);
    let real = d.path().join("real.musicxml");
    std::fs::write(&real, "REAL").expect("escreve");
    let atalho = d.path().join("atalho.musicxml");
    std::os::unix::fs::symlink(&real, &atalho).expect("atalho");
    e.sobrescrever = true;
    let msg = e
        .exportar(&atalho.to_string_lossy())
        .expect_err("atalho é recusado");
    assert!(msg.contains("atalho"), "{msg}");
    assert!(
        std::fs::symlink_metadata(&atalho)
            .expect("meta")
            .file_type()
            .is_symlink(),
        "o atalho continua atalho"
    );
    assert_eq!(std::fs::read_to_string(&real).expect("lê"), "REAL");
    // Sem "Sobrescrever", um atalho (mesmo para arquivo existente) também não é tocado.
    e.sobrescrever = false;
    assert!(e.exportar(&atalho.to_string_lossy()).is_err());
    assert_eq!(std::fs::read_to_string(&real).expect("lê"), "REAL");
}

// --- N9 (a): pasta inexistente ------------------------------------------------------------------

#[test]
fn n9_exportar_para_pasta_inexistente_diz_que_a_pasta_nao_existe() {
    let (d, mut e, _xml) = com_oficina();
    editar_uma_nota(&mut e);
    let alvo = d.path().join("nao/existe/x.musicxml");
    for sobrescrever in [false, true] {
        e.sobrescrever = sobrescrever;
        let msg = e
            .exportar(&alvo.to_string_lossy())
            .expect_err("pasta inexistente");
        assert!(msg.contains("pasta"), "{msg}");
        assert!(msg.contains("não existe"), "{msg}");
        assert!(
            !msg.contains("Sobrescrever"),
            "a dica de sobrescrever é enganosa aqui: {msg}"
        );
    }
}

#[test]
fn n9_dica_de_sobrescrever_so_aparece_quando_o_arquivo_ja_existe() {
    let (d, mut e, _xml) = com_oficina();
    editar_uma_nota(&mut e);
    let alvo = d.path().join("saida.musicxml");
    e.exportar(&alvo.to_string_lossy()).expect("exporta");
    let msg = e.exportar(&alvo.to_string_lossy()).expect_err("já existe");
    assert!(msg.contains("Sobrescrever"), "{msg}");
}
