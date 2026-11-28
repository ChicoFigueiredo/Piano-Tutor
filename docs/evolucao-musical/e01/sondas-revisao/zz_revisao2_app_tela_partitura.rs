//! SONDAS TEMPORARIAS da segunda revisao independente da E01. Nao fazem parte do produto.
use super::testes::{arquivo_do_rascunho, com_oficina, editar_uma_nota, tick_ate_gravar};
use super::testes_revisao::{criada, editar_outra_nota, midis_do_compasso_1};
use super::*;
use crate::atalhos_partitura::PedidoDeAcorde;
use std::os::unix::fs::symlink;

fn ler(p: &Path) -> Vec<u8> {
    std::fs::read(p).expect("le")
}

// ---------- R2 ----------
#[test]
fn zz_r2_atalho_de_pasta_para_a_oficina() {
    let (d, mut e, xml) = com_oficina();
    let antes = ler(&xml);
    editar_uma_nota(&mut e);
    e.sobrescrever = true;
    let raiz = d.path().join("oficina");
    symlink(raiz.join("partituras"), raiz.join("atalho")).unwrap();
    let r = e.exportar(&raiz.join("atalho/ode.musicxml").to_string_lossy());
    println!(
        "SONDA r2 atalho de pasta: {r:?} hipotese_mudou={}",
        ler(&xml) != antes
    );
}

#[test]
fn zz_r2_caminho_relativo_e_maiusculas() {
    let (d, mut e, xml) = com_oficina();
    let antes = ler(&xml);
    editar_uma_nota(&mut e);
    e.sobrescrever = true;
    let raiz = d.path().join("oficina");
    let velho = std::env::current_dir().unwrap();
    std::env::set_current_dir(raiz.join("partituras")).unwrap();
    let r1 = e.exportar("ode.musicxml");
    let r2 = e.exportar("./ode.musicxml");
    std::env::set_current_dir(&raiz).unwrap();
    let r3 = e.exportar("partituras/ode.musicxml");
    let r4 = e.exportar("partituras/../partituras/ode.musicxml");
    let r5 = e.exportar("partituras/ode");
    std::env::set_current_dir(velho).unwrap();
    println!(
        "SONDA r2 relativos (recusado?): {:?} {:?} {:?} {:?} {:?} hipotese_mudou={}",
        r1.is_err(),
        r2.is_err(),
        r3.is_err(),
        r4.is_err(),
        r5.is_err(),
        ler(&xml) != antes
    );
    let r6 = e.exportar(&raiz.join("partituras/ODE.musicxml").to_string_lossy());
    let r7 = e.exportar(&raiz.join("partituras/ode.MUSICXML").to_string_lossy());
    println!(
        "SONDA r2 maiusculas (Linux, FS sensivel): {r6:?} {r7:?} hipotese_mudou={}",
        ler(&xml) != antes
    );
}

#[test]
fn zz_r2_link_para_objeto_de_versao_e_hardlink_do_objeto() {
    let (d, mut e, xml) = com_oficina();
    editar_uma_nota(&mut e);
    let v1 = criada(e.salvar_versao().unwrap());
    let objeto = xml
        .with_file_name("ode.versoes")
        .join("objetos")
        .join(format!("{}.musicxml", v1.hash));
    let antes = ler(&objeto);
    editar_outra_nota(&mut e);
    e.sobrescrever = true;
    let sim = d.path().join("sim.musicxml");
    symlink(&objeto, &sim).unwrap();
    let r1 = e.exportar(&sim.to_string_lossy());
    let hl = d.path().join("hl.musicxml");
    std::fs::hard_link(&objeto, &hl).unwrap();
    let r2 = e.exportar(&hl.to_string_lossy());
    println!(
        "SONDA r2 symlink->objeto: err={} | hardlink->objeto: {:?} objeto_mudou={}",
        r1.is_err(),
        r2,
        ler(&objeto) != antes
    );
}

#[test]
fn zz_r2_escolher_arquivo_solto_da_pasta_de_assets_sobrescreve_o_original() {
    let d = tempfile::tempdir().unwrap();
    let pasta = d.path().join("partituras");
    std::fs::create_dir_all(&pasta).unwrap();
    let solto = pasta.join("solta.musicxml");
    let texto = format!("{}<!-- a -->\n", EMBUTIDAS[3].1);
    std::fs::write(&solto, &texto).unwrap();
    let mut e = EstadoPartitura::novo(Some(d.path().to_path_buf()));
    let i = e
        .fontes
        .iter()
        .position(|f| f.caminho.is_some())
        .expect("fonte");
    e.escolher(i);
    println!("SONDA r2 escolher: erro={:?} nome={}", e.erro, e.nome());
    e.navegar(Comando::Seguinte);
    assert!(
        e.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Acima)
            .is_some()
    );
    e.sobrescrever = true;
    let r = e.exportar(&solto.to_string_lossy());
    println!(
        "SONDA r2 escolher: exportar sobre o arquivo escolhido na lista = {r:?}; original_mudou={}",
        std::fs::read_to_string(&solto).unwrap() != texto
    );
    let mut e2 = EstadoPartitura::novo(None);
    std::fs::write(&solto, &texto).unwrap();
    e2.abrir_caminho(&solto.to_string_lossy());
    e2.navegar(Comando::Seguinte);
    assert!(
        e2.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Acima)
            .is_some()
    );
    e2.sobrescrever = true;
    println!(
        "SONDA r2 abrir_caminho protege: {:?}",
        e2.exportar(&solto.to_string_lossy()).is_err()
    );
}

#[test]
fn zz_r2_depois_de_continuar_a_partir_de_versao_a_hipotese_segue_protegida() {
    let (_d, mut e, xml) = com_oficina();
    let antes = ler(&xml);
    editar_uma_nota(&mut e);
    let v1 = criada(e.salvar_versao().unwrap());
    e.continuar_a_partir_da_versao(&v1.hash);
    e.sobrescrever = true;
    println!(
        "SONDA r2 pos-continuar: err={}",
        e.exportar(&xml.to_string_lossy()).is_err()
    );
    assert_eq!(ler(&xml), antes);
}

#[test]
fn zz_r2_exportar_com_pasta_inexistente_e_destino_diretorio() {
    let (d, mut e, _xml) = com_oficina();
    editar_uma_nota(&mut e);
    let r1 = e.exportar(&d.path().join("nao/existe/x.musicxml").to_string_lossy());
    let dir = d.path().join("dir.musicxml");
    std::fs::create_dir(&dir).unwrap();
    e.sobrescrever = true;
    let r2 = e.exportar(&dir.to_string_lossy());
    println!("SONDA r2 pasta inexistente: {r1:?}; destino diretorio: {r2:?}");
    let restos: Vec<_> = std::fs::read_dir(d.path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|f| f.file_name())
        .collect();
    println!("SONDA r2 restos na pasta: {restos:?}");
}

// ---------- R3 ----------
fn tres_proximas(e: &mut EstadoPartitura) {
    e.navegar(Comando::Seguinte);
    e.navegar(Comando::Seguinte);
    e.navegar(Comando::Seguinte);
}

#[test]
fn zz_r3_tres_sessoes_e_continuar_de_versao_antiga() {
    let (_d, mut e, xml) = com_oficina();
    let h = hash_do_conteudo(EMBUTIDAS[3].1.as_bytes());
    editar_uma_nota(&mut e);
    let v1 = criada(e.salvar_versao().unwrap());
    let m1 = midis_do_compasso_1(&e);
    let mut e2 = EstadoPartitura::novo(None);
    e2.abrir_caminho(&xml.to_string_lossy());
    editar_outra_nota(&mut e2);
    let v2 = criada(e2.salvar_versao().unwrap());
    let m2 = midis_do_compasso_1(&e2);
    let mut e3 = EstadoPartitura::novo(None);
    e3.abrir_caminho(&xml.to_string_lossy());
    e3.continuar_a_partir_da_versao(&v1.hash);
    assert_eq!(midis_do_compasso_1(&e3), m1);
    tres_proximas(&mut e3);
    assert!(
        e3.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Acima)
            .is_some()
    );
    let v3 = criada(e3.salvar_versao().unwrap());
    e3.navegar(Comando::Seguinte);
    assert!(
        e3.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Acima)
            .is_some()
    );
    let v4 = criada(e3.salvar_versao().unwrap());
    let mut e4 = EstadoPartitura::novo(None);
    e4.abrir_caminho(&xml.to_string_lossy());
    e4.continuar_a_partir_da_versao(&v2.hash);
    assert_eq!(midis_do_compasso_1(&e4), m2);
    tres_proximas(&mut e4);
    assert!(
        e4.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Abaixo)
            .is_some()
    );
    let v5 = criada(e4.salvar_versao().unwrap());
    println!(
        "SONDA r3: v1.pai==H {} v2.pai==H {} v3.pai==v1 {} v4.pai==v3 {} v5.pai==v2 {}",
        v1.pai.as_deref() == Some(h.as_str()),
        v2.pai.as_deref() == Some(h.as_str()),
        v3.pai.as_deref() == Some(v1.hash.as_str()),
        v4.pai.as_deref() == Some(v3.hash.as_str()),
        v5.pai.as_deref() == Some(v2.hash.as_str()),
    );
    let m3 = midis_do_compasso_1(&e3);
    println!("SONDA r3 m1={m1:?}\n m2={m2:?}\n m3={m3:?}");
    let mut e5 = EstadoPartitura::novo(None);
    e5.abrir_caminho(&xml.to_string_lossy());
    e5.continuar_a_partir_da_versao(&h);
    println!(
        "SONDA r3 aviso continuar da hipotese: {:?}",
        e5.aviso_de_versoes()
    );
    tres_proximas(&mut e5);
    e5.navegar(Comando::Seguinte);
    let _ = e5.mudar_altura(UnidadeDeAltura::Oitava, SentidoDaAltura::Acima);
    let v6 = e5.salvar_versao();
    println!(
        "SONDA r3 v6 (a partir da Hipotese): {:?}",
        v6.map(|r| match r {
            ResultadoDoSalvar::Criada(v) => format!("Criada pai==H {}", v.pai.as_deref() == Some(h.as_str())),
            o => format!("{o:?}"),
        })
    );
}

#[test]
fn zz_r3_recuperado_de_sessao_que_caiu_herda_a_linhagem() {
    let (_d, mut e, xml) = com_oficina();
    editar_uma_nota(&mut e);
    let v1 = criada(e.salvar_versao().unwrap());
    editar_outra_nota(&mut e);
    e.autosave_tick(0);
    let iv = e.config_autosave.intervalo_ms;
    tick_ate_gravar(&mut e, iv);
    let lateral = arquivo_do_rascunho(&xml).with_file_name("rascunho.ids");
    println!(
        "SONDA r3 rascunho.ids existe: {} primeira-linha={:?}",
        lateral.exists(),
        std::fs::read_to_string(&lateral)
            .ok()
            .and_then(|t| t.lines().next().map(str::to_owned))
    );
    drop(e);
    let mut n = EstadoPartitura::novo(None);
    n.abrir_caminho(&xml.to_string_lossy());
    assert!(n.rascunho_pendente().is_some());
    n.recuperar_rascunho();
    let base = n
        .documento
        .as_ref()
        .unwrap()
        .base_de_ids()
        .map(str::to_owned);
    println!(
        "SONDA r3 base apos recuperar == v1: {}",
        base.as_deref() == Some(v1.hash.as_str())
    );
    tres_proximas(&mut n);
    let ed = n.mudar_altura(UnidadeDeAltura::Oitava, SentidoDaAltura::Acima);
    println!("SONDA r3 editou={}", ed.is_some());
    let v2 = n.salvar_versao();
    println!(
        "SONDA r3 recuperado: salvar -> {:?}",
        v2.map(|r| match r {
            ResultadoDoSalvar::Criada(v) => format!("Criada pai==v1 {}", v.pai.as_deref() == Some(v1.hash.as_str())),
            o => format!("{o:?}"),
        })
    );
}

#[test]
fn zz_r3_recuperado_sem_ids_lateral_perde_a_linhagem() {
    let (_d, mut e, xml) = com_oficina();
    editar_uma_nota(&mut e);
    let v1 = criada(e.salvar_versao().unwrap());
    editar_outra_nota(&mut e);
    e.autosave_tick(0);
    let iv = e.config_autosave.intervalo_ms;
    tick_ate_gravar(&mut e, iv);
    drop(e);
    let lateral = arquivo_do_rascunho(&xml).with_file_name("rascunho.ids");
    let _ = std::fs::remove_file(&lateral);
    let mut n = EstadoPartitura::novo(None);
    n.abrir_caminho(&xml.to_string_lossy());
    n.recuperar_rascunho();
    tres_proximas(&mut n);
    let ed = n.mudar_altura(UnidadeDeAltura::Oitava, SentidoDaAltura::Acima);
    println!("SONDA r3 sem ids editou={}", ed.is_some());
    let h = hash_do_conteudo(EMBUTIDAS[3].1.as_bytes());
    let v2 = n.salvar_versao();
    println!(
        "SONDA r3 sem .ids: {:?}",
        v2.map(|r| match r {
            ResultadoDoSalvar::Criada(v) => format!(
                "Criada pai==v1 {} pai==H {}",
                v.pai.as_deref() == Some(v1.hash.as_str()),
                v.pai.as_deref() == Some(h.as_str())
            ),
            o => format!("{o:?}"),
        })
    );
}

#[test]
fn zz_r3_continuar_com_objeto_adulterado_e_com_leitura_aberta() {
    let (_d, mut e, xml) = com_oficina();
    editar_uma_nota(&mut e);
    let v1 = criada(e.salvar_versao().unwrap());
    editar_outra_nota(&mut e);
    let v2 = criada(e.salvar_versao().unwrap());
    let obj = xml
        .with_file_name("ode.versoes")
        .join("objetos")
        .join(format!("{}.musicxml", v1.hash));
    let original = std::fs::read_to_string(&obj).unwrap();
    std::fs::write(&obj, original.replace("<step>", "<step >")).unwrap();
    let antes = midis_do_compasso_1(&e);
    e.continuar_a_partir_da_versao(&v1.hash);
    println!(
        "SONDA r3 adulterado: aviso={:?} doc_igual={}",
        e.aviso_de_versoes(),
        midis_do_compasso_1(&e) == antes
    );
    std::fs::write(&obj, original).unwrap();
    e.abrir_versao(&v1.hash);
    println!(
        "SONDA r3 leitura: em_leitura={}",
        e.versao_em_leitura().is_some()
    );
    e.continuar_a_partir_da_versao(&v2.hash);
    println!(
        "SONDA r3 continuar a partir da leitura: em_leitura={} aviso={:?} nome={}",
        e.versao_em_leitura().is_some(),
        e.aviso_de_versoes(),
        e.nome()
    );
    editar_uma_nota(&mut e);
    let v3 = e.salvar_versao();
    println!(
        "SONDA r3 salvar depois: {:?}",
        v3.map(|r| match r {
            ResultadoDoSalvar::Criada(v) => format!("pai==v2 {}", v.pai.as_deref() == Some(v2.hash.as_str())),
            o => format!("{o:?}"),
        })
    );
}

// ---------- R1 ----------
#[test]
fn zz_r1_todas_as_edicoes_recusadas_com_rascunho_pendente() {
    let (_d, mut e, xml, _editado) =
        super::testes_revisao::reabrir_com_rascunho_pendente();
    let antes = e.documento.as_ref().unwrap().exportado().unwrap();
    e.navegar(Comando::Seguinte);
    let mut rs = vec![];
    rs.push((
        "altura",
        e.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Acima)
            .is_none(),
    ));
    rs.push(("duracao", e.mudar_duracao(5).is_none()));
    rs.push(("ponto", e.alternar_ponto().is_none()));
    rs.push(("inserir nota", e.inserir_nota_na_entrada('c').is_none()));
    rs.push(("inserir pausa", e.inserir_pausa_na_entrada().is_none()));
    rs.push(("remover", e.remover_nota_selecionada().is_none()));
    rs.push((
        "acorde letra",
        e.editar_acorde(PedidoDeAcorde::Letra('e')).is_none(),
    ));
    rs.push((
        "acorde intervalo",
        e.editar_acorde(PedidoDeAcorde::Intervalo(3)).is_none(),
    ));
    rs.push((
        "acorde retirar",
        e.editar_acorde(PedidoDeAcorde::Retirar).is_none(),
    ));
    rs.push(("desfazer", !e.desfazer()));
    rs.push(("refazer", !e.refazer()));
    println!("SONDA r1 bloqueios: {rs:?}");
    assert_eq!(e.documento.as_ref().unwrap().exportado().unwrap(), antes);
    assert!(e.rascunho_pendente().is_some());
    assert!(arquivo_do_rascunho(&xml).is_file());
    println!("SONDA r1 salvar recusado: {:?}", e.salvar_versao().is_err());
}

#[test]
fn zz_r1_abrir_versao_em_leitura_e_recuperar() {
    let (_d, mut e, xml) = com_oficina();
    editar_uma_nota(&mut e);
    let v1 = criada(e.salvar_versao().unwrap());
    editar_outra_nota(&mut e);
    let editado = e.documento.as_ref().unwrap().exportado().unwrap();
    e.autosave_tick(0);
    let iv = e.config_autosave.intervalo_ms;
    tick_ate_gravar(&mut e, iv);
    drop(e);
    let mut n = EstadoPartitura::novo(None);
    n.abrir_caminho(&xml.to_string_lossy());
    assert!(n.rascunho_pendente().is_some());
    n.abrir_versao(&v1.hash);
    println!(
        "SONDA r1 leitura: pendente={} em_leitura={}",
        n.rascunho_pendente().is_some(),
        n.versao_em_leitura().is_some()
    );
    n.recuperar_rascunho();
    println!(
        "SONDA r1 leitura+recuperar: em_leitura={} nome={} doc_e_o_rascunho={} pendente={} aviso={:?}",
        n.versao_em_leitura().is_some(),
        n.nome(),
        n.documento.as_ref().unwrap().exportado().unwrap() == editado,
        n.rascunho_pendente().is_some(),
        n.aviso_de_versoes()
    );
    n.navegar(Comando::Seguinte);
    let r = n.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Acima);
    println!(
        "SONDA r1 editar apos recuperar-em-leitura: editou={:?} ultima_edicao={:?}",
        r.is_some(),
        n.ultima_edicao()
    );
    n.voltar_da_leitura();
    println!(
        "SONDA r1 voltar: doc_e_o_rascunho={} modificado={} banner={} rascunho_em_disco={}",
        n.documento.as_ref().unwrap().exportado().unwrap() == editado,
        n.documento.as_ref().unwrap().modificado(),
        n.rascunho_pendente().is_some(),
        arquivo_do_rascunho(&xml).exists()
    );
    n.navegar(Comando::Seguinte);
    let _ = n.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Acima);
    // O autosave sobrescreve o rascunho da sessao que caiu com a edicao nova?
    let antes_rascunho = std::fs::read_to_string(arquivo_do_rascunho(&xml)).unwrap_or_default();
    let iv = n.config_autosave.intervalo_ms;
    n.autosave_tick(10_000_000);
    tick_ate_gravar(&mut n, 10_000_000 + iv);
    let depois_rascunho = std::fs::read_to_string(arquivo_do_rascunho(&xml)).unwrap_or_default();
    println!(
        "SONDA r1 autosave sobrescreveu o rascunho da sessao que caiu: {} (o rascunho antigo era o trabalho recuperado: {})",
        antes_rascunho != depois_rascunho,
        antes_rascunho.contains("<step>")
    );
    let s = n.salvar_versao();
    println!(
        "SONDA r1 salvar depois de voltar: ok={:?} rascunho_em_disco={}",
        s.is_ok(),
        arquivo_do_rascunho(&xml).exists()
    );
}

#[test]
fn zz_r1_recuperar_que_falha_deixa_autosave_mudo() {
    let (_d, e0, xml) = com_oficina();
    drop(e0);
    let pasta = xml.parent().unwrap().to_path_buf();
    let cfg = crate::autosave::ConfigAutosave::default();
    let a = crate::autosave::Autosave::em(&pasta, "ode", &cfg).expect("autosave");
    a.gravador()
        .gravar_com_ids("isto nao e musicxml", None, "2026-01-01T00:00:00Z")
        .expect("grava");
    drop(a);
    let mut n = EstadoPartitura::novo(None);
    n.abrir_caminho(&xml.to_string_lossy());
    println!("SONDA r1 pendente={}", n.rascunho_pendente().is_some());
    n.recuperar_rascunho();
    println!(
        "SONDA r1 aviso apos falha: {:?} pendente={}",
        n.aviso_de_versoes(),
        n.rascunho_pendente().is_some()
    );
    editar_uma_nota(&mut n);
    let iv = n.config_autosave.intervalo_ms;
    n.autosave_tick(0);
    n.autosave_tick(iv * 3);
    std::thread::sleep(std::time::Duration::from_millis(300));
    n.autosave_tick(iv * 4);
    let conteudo = std::fs::read_to_string(arquivo_do_rascunho(&xml)).unwrap_or_default();
    println!(
        "SONDA r1 rascunho em disco continua o lixo={} gravando={}",
        conteudo.contains("isto nao e musicxml"),
        n.gravando_rascunho()
    );
}

#[test]
fn zz_r1_descartar_libera_a_edicao_e_o_autosave_volta() {
    let (_d, mut e, xml, _) = super::testes_revisao::reabrir_com_rascunho_pendente();
    e.descartar_rascunho();
    editar_uma_nota(&mut e);
    e.autosave_tick(0);
    let iv = e.config_autosave.intervalo_ms;
    tick_ate_gravar(&mut e, iv);
    println!(
        "SONDA r1 descartar -> autosave grava: {}",
        arquivo_do_rascunho(&xml).is_file()
    );
}

#[test]
fn zz_r2_sobrescrever_arquivo_somente_leitura_e_atalho() {
    use std::os::unix::fs::PermissionsExt;
    let (d, mut e, _xml) = com_oficina();
    editar_uma_nota(&mut e);
    let alvo = d.path().join("protegido.musicxml");
    std::fs::write(&alvo, "ORIGINAL").unwrap();
    std::fs::set_permissions(&alvo, std::fs::Permissions::from_mode(0o444)).unwrap();
    e.sobrescrever = true;
    let r = e.exportar(&alvo.to_string_lossy());
    let modo = std::fs::metadata(&alvo).unwrap().permissions().mode() & 0o777;
    println!(
        "SONDA r2 somente-leitura (0444): {:?} conteudo_mudou={} modo_depois={:o}",
        r.as_ref().map(|_| ()).map_err(String::clone),
        std::fs::read_to_string(&alvo).unwrap() != "ORIGINAL",
        modo
    );
    let real = d.path().join("real.musicxml");
    std::fs::write(&real, "REAL").unwrap();
    let atalho = d.path().join("atalho2.musicxml");
    symlink(&real, &atalho).unwrap();
    let r = e.exportar(&atalho.to_string_lossy());
    println!(
        "SONDA r2 atalho para arquivo comum: {:?} atalho_ainda_e_link={} real_mudou={}",
        r.is_ok(),
        std::fs::symlink_metadata(&atalho)
            .unwrap()
            .file_type()
            .is_symlink(),
        std::fs::read_to_string(&real).unwrap() != "REAL"
    );
}
