//! SONDAS TEMPORARIAS da terceira revisao independente da E01 (app: N1-N4, N9).
use super::testes::{arquivo_do_rascunho, com_oficina, editar_uma_nota, tick_ate_gravar};
use super::testes_revisao::{criada, editar_outra_nota};
use super::*;
use crate::atalhos_partitura::PedidoDeAcorde;

fn texto_do_documento(e: &EstadoPartitura) -> String {
    e.documento.as_ref().expect("doc").exportado().expect("x")
}

fn reabrir_com_versao_e_rascunho() -> (tempfile::TempDir, EstadoPartitura, PathBuf, String, String)
{
    let (d, mut e, xml) = com_oficina();
    editar_uma_nota(&mut e);
    let v1 = criada(e.salvar_versao().expect("salva v1"));
    editar_outra_nota(&mut e);
    let editado = texto_do_documento(&e);
    e.autosave_tick(0);
    let iv = e.config_autosave.intervalo_ms;
    tick_ate_gravar(&mut e, iv);
    drop(e);
    let mut n = EstadoPartitura::novo(None);
    n.abrir_caminho(&xml.to_string_lossy());
    assert!(n.rascunho_pendente().is_some());
    (d, n, xml, editado, v1.hash)
}

fn st(rot: &str, e: &EstadoPartitura, xml: &Path, editado: &str) {
    let t = texto_do_documento(e);
    println!(
        "SONDA3 [{rot}] leitura={} pendente={} nome={:?} modificado={} texto=={} rasc_em_disco={} pode_salvar={} aviso={:?} edicao={:?}",
        e.versao_em_leitura().is_some(),
        e.rascunho_pendente().is_some(),
        e.nome(),
        e.documento.as_ref().unwrap().modificado(),
        if t == editado { "TRABALHO_RECUPERADO" } else { "outro" },
        arquivo_do_rascunho(xml).is_file(),
        e.pode_salvar_versao(),
        e.aviso_de_versoes(),
        e.ultima_edicao(),
    );
}

fn tentar_editar(e: &mut EstadoPartitura) -> bool {
    e.navegar(Comando::Seguinte);
    e.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Acima)
        .is_some()
}

#[test]
fn zz3_app_a_ordens() {
    // S1: abrir Versao -> Recuperar -> Recuperar de novo (API; o botao some)
    {
        let (_d, mut n, xml, editado, v1) = reabrir_com_versao_e_rascunho();
        n.abrir_versao(&v1);
        n.recuperar_rascunho();
        st("S1 apos recuperar em leitura", &n, &xml, &editado);
        tentar_editar(&mut n);
        let apos_edicao = texto_do_documento(&n);
        n.recuperar_rascunho(); // API: o botao nao existe, mas e `pub`
        println!(
            "SONDA3 [S1 recuperar duas vezes] edicao nova perdida={} (texto igual ao trabalho recuperado: {})",
            texto_do_documento(&n) != apos_edicao,
            texto_do_documento(&n) == editado
        );
    }
    // S2: abrir Versao -> Descartar -> Recuperar(API) -> Voltar -> editar
    {
        let (_d, mut n, xml, editado, v1) = reabrir_com_versao_e_rascunho();
        n.abrir_versao(&v1);
        n.descartar_rascunho();
        st("S2 apos descartar em leitura", &n, &xml, &editado);
        n.recuperar_rascunho();
        st("S2 recuperar sem rascunho", &n, &xml, &editado);
        n.voltar_da_leitura();
        st("S2 apos voltar", &n, &xml, &editado);
        println!("SONDA3 [S2] edita={}", tentar_editar(&mut n));
        n.autosave_tick(0);
        let iv = n.config_autosave.intervalo_ms;
        tick_ate_gravar(&mut n, iv);
        println!("SONDA3 [S2] rascunho gravado depois={}", arquivo_do_rascunho(&xml).is_file());
    }
    // S3: abrir Hipotese em leitura (outra versao) depois v1, Recuperar
    {
        let (_d, mut n, xml, editado, v1) = reabrir_com_versao_e_rascunho();
        let hip = hash_do_conteudo(EMBUTIDAS[3].1.as_bytes());
        n.abrir_versao(&hip);
        n.abrir_versao(&v1);
        n.recuperar_rascunho();
        st("S3 duas leituras e recuperar", &n, &xml, &editado);
        n.voltar_da_leitura();
        st("S3 voltar apos recuperar", &n, &xml, &editado);
        let v2 = n.salvar_versao();
        println!("SONDA3 [S3] salvar: {:?}", v2.as_ref().map(|r| match r { ResultadoDoSalvar::Criada(v) => format!("criada pai={:?} v1={}", v.pai, &v1[..8]), o => format!("{o:?}") }));
        println!("SONDA3 [S3] rascunho em disco apos salvar={}", arquivo_do_rascunho(&xml).exists());
    }
    // S4: recuperar -> abrir Versao -> autosave em leitura -> voltar -> editar -> salvar
    {
        let (_d, mut n, xml, editado, v1) = reabrir_com_versao_e_rascunho();
        n.recuperar_rascunho();
        n.abrir_versao(&v1);
        n.autosave_tick(5_000_000);
        let iv = n.config_autosave.intervalo_ms;
        n.autosave_tick(5_000_000 + iv);
        println!("SONDA3 [S4] gravando durante leitura={}", n.gravando_rascunho());
        n.voltar_da_leitura();
        st("S4 voltar", &n, &xml, &editado);
        println!("SONDA3 [S4] edita={}", tentar_editar(&mut n));
        let r = n.salvar_versao();
        println!("SONDA3 [S4] salvar={:?}", r.as_ref().map(|r| matches!(r, ResultadoDoSalvar::Criada(_))));
    }
    // S5: rascunho que nao abre, em leitura: Recuperar falha; Descartar; Voltar; editar
    {
        let (d, e0, xml) = com_oficina();
        drop(e0);
        let pasta = xml.parent().unwrap().to_path_buf();
        let cfg = crate::autosave::ConfigAutosave::default();
        let a = crate::autosave::Autosave::em(&pasta, "ode", &cfg).unwrap();
        a.gravador().gravar_com_ids("isto nao e musicxml", None, "2026-01-01T00:00:00Z").unwrap();
        drop(a);
        let mut n = EstadoPartitura::novo(None);
        n.abrir_caminho(&xml.to_string_lossy());
        let hip = hash_do_conteudo(EMBUTIDAS[3].1.as_bytes());
        n.abrir_versao(&hip);
        n.recuperar_rascunho();
        st("S5 recuperar falho em leitura", &n, &xml, "");
        n.descartar_rascunho();
        st("S5 descartar em leitura", &n, &xml, "");
        n.voltar_da_leitura();
        st("S5 voltar", &n, &xml, "");
        println!("SONDA3 [S5] edita={}", tentar_editar(&mut n));
        drop(d);
    }
    // S6: Continuar com pendente bloqueado; Descartar; Continuar; Salvar
    {
        let (_d, mut n, xml, editado, v1) = reabrir_com_versao_e_rascunho();
        n.continuar_a_partir_da_versao(&v1);
        st("S6 continuar com pendente", &n, &xml, &editado);
        n.abrir_versao(&v1);
        n.continuar_a_partir_da_versao(&v1);
        st("S6 continuar em leitura com pendente", &n, &xml, &editado);
        n.descartar_rascunho();
        n.continuar_a_partir_da_versao(&v1);
        st("S6 continuar apos descartar", &n, &xml, &editado);
        println!("SONDA3 [S6] edita={}", tentar_editar(&mut n));
    }
    // S7: Recuperar -> Continuar (modificado => recusa) -> Salvar -> Continuar
    {
        let (_d, mut n, xml, editado, v1) = reabrir_com_versao_e_rascunho();
        n.recuperar_rascunho();
        n.continuar_a_partir_da_versao(&v1);
        st("S7 continuar apos recuperar", &n, &xml, &editado);
        let r = criada(n.salvar_versao().expect("salva"));
        println!("SONDA3 [S7] versao salva pai={:?} rasc_em_disco={}", r.pai.map(|p| p[..8].to_owned()), arquivo_do_rascunho(&xml).exists());
        n.continuar_a_partir_da_versao(&v1);
        st("S7 continuar apos salvar", &n, &xml, &editado);
    }
    // S8: Descartar duas vezes / Voltar sem leitura / Recuperar sem autosave
    {
        let (_d, mut n, xml, editado, _v1) = reabrir_com_versao_e_rascunho();
        n.descartar_rascunho();
        n.descartar_rascunho();
        n.voltar_da_leitura();
        st("S8 descartar x2 + voltar", &n, &xml, &editado);
        let mut solto = EstadoPartitura::novo(None);
        solto.recuperar_rascunho();
        solto.descartar_rascunho();
        println!("SONDA3 [S8] solto sem panico");
    }
}

#[test]
fn zz3_app_b_acorde_unissono_selecao() {
    const ACORDE: &str = r#"<score-partwise><part-list><score-part id="P1"><part-name>x</part-name></score-part></part-list>
<part id="P1"><measure number="1"><attributes><divisions>1</divisions><key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note>
<note><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note>
<note><chord/><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note>
</measure></part></score-partwise>"#;
    for (rot, texto) in [("uníssono", ACORDE), ("distinto", &ACORDE.replace("<chord/><pitch><step>D", "<chord/><pitch><step>F"))] {
        let mut e = EstadoPartitura::novo(None);
        e.carregar_texto("a".into(), texto.to_owned()).unwrap();
        e.navegar(Comando::Seguinte);
        e.navegar(Comando::Seguinte);
        let antes = e.nota_selecionada();
        let r = e.editar_acorde(PedidoDeAcorde::Retirar);
        println!(
            "SONDA3 [acorde {rot}] sel antes={antes:?} depois={:?} inverso={:?} msg={:?}",
            e.nota_selecionada(),
            r.as_ref().map(|r| format!("{:?}", r.inverso).chars().take(40).collect::<String>()),
            e.ultima_edicao()
        );
        let ok = e.desfazer();
        println!("SONDA3 [acorde {rot}] desfazer ok={ok} texto igual={}", texto_do_documento(&e) == texto);
    }
}

#[cfg(unix)]
#[test]
fn zz3_app_c_exportar() {
    use std::os::unix::fs::PermissionsExt;
    let (d, mut e, xml) = com_oficina();
    editar_uma_nota(&mut e);
    e.sobrescrever = false;
    let pasta = d.path().to_path_buf();
    let alvo_alheio = pasta.join("importante.txt");
    std::fs::write(&alvo_alheio, "IMPORTANTE").unwrap();

    // E1: lateral .ids e link simbolico para arquivo alheio
    let dest = pasta.join("saida1.musicxml");
    let lateral = crate::ids_lateral::caminho_lateral(&dest);
    std::os::unix::fs::symlink(&alvo_alheio, &lateral).unwrap();
    let r = e.exportar(&dest.to_string_lossy());
    println!(
        "SONDA3 [E1 lateral symlink] r={:?} alheio={:?} lateral_ainda_link={}",
        r.as_ref().map(|_| ()).map_err(|e| e.clone()),
        std::fs::read_to_string(&alvo_alheio).unwrap(),
        std::fs::symlink_metadata(&lateral).map(|m| m.file_type().is_symlink()).unwrap_or(false)
    );

    // E2: lateral somente leitura
    let dest = pasta.join("saida2.musicxml");
    let lateral = crate::ids_lateral::caminho_lateral(&dest);
    std::fs::write(&lateral, "LATERAL ANTIGO").unwrap();
    std::fs::set_permissions(&lateral, std::fs::Permissions::from_mode(0o444)).unwrap();
    let r = e.exportar(&dest.to_string_lossy());
    println!(
        "SONDA3 [E2 lateral 0444] r={:?} lateral_mudou={} modo={:o}",
        r.as_ref().map(|_| ()).map_err(|e| e.clone()),
        std::fs::read_to_string(&lateral).unwrap() != "LATERAL ANTIGO",
        std::fs::metadata(&lateral).unwrap().permissions().mode() & 0o777
    );

    // E3: destino e diretorio, com sobrescrever
    e.sobrescrever = true;
    let dir = pasta.join("eumdir.musicxml");
    std::fs::create_dir(&dir).unwrap();
    let r = e.exportar(&dir.to_string_lossy());
    let sobras: Vec<_> = std::fs::read_dir(&pasta).unwrap().filter_map(|x| x.ok()).map(|x| x.file_name().to_string_lossy().into_owned()).filter(|n| n.contains(".tmp")).collect();
    println!("SONDA3 [E3 destino diretorio] r={:?} tmps_sobrando={sobras:?}", r.as_ref().map(|_| ()).map_err(|e| e.clone()));
    e.sobrescrever = false;
    let r = e.exportar(&dir.to_string_lossy());
    println!("SONDA3 [E3b destino diretorio sem sobrescrever] r={:?}", r.as_ref().map(|_| ()).map_err(|e| e.clone()));

    // E4: link simbolico para pasta; link pendente (dangling)
    let alvo_dir = pasta.join("pasta_real");
    std::fs::create_dir(&alvo_dir).unwrap();
    let link_dir = pasta.join("link_dir.musicxml");
    std::os::unix::fs::symlink(&alvo_dir, &link_dir).unwrap();
    let r = e.exportar(&link_dir.to_string_lossy());
    println!("SONDA3 [E4 link para pasta] r={:?}", r.as_ref().map(|_| ()).map_err(|e| e.clone()));
    let pend = pasta.join("pendente.musicxml");
    std::os::unix::fs::symlink(pasta.join("nao_existe_ainda.musicxml"), &pend).unwrap();
    let r = e.exportar(&pend.to_string_lossy());
    println!(
        "SONDA3 [E4b link pendente] r={:?} criou_alvo={}",
        r.as_ref().map(|_| ()).map_err(|e| e.clone()),
        pasta.join("nao_existe_ainda.musicxml").exists()
    );

    // E5: pasta pai sem permissao de escrita (root ignora; informativo)
    let ro = pasta.join("ro");
    std::fs::create_dir(&ro).unwrap();
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).unwrap();
    let r = e.exportar(&ro.join("x.musicxml").to_string_lossy());
    println!("SONDA3 [E5 pasta so leitura (root ignora)] r={:?}", r.as_ref().map(|_| ()).map_err(|e| e.clone()));
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();

    // E6: hard link do objeto de Versao para fora do .versoes, com sobrescrever
    let v = e.salvar_versao();
    println!("SONDA3 [E6] salvar versao: {:?}", v.as_ref().map(|_| ()).map_err(|e| e.clone()));
    let versoes_dir = xml.with_file_name("ode.versoes");
    let mut objeto = None;
    if let Ok(rd) = std::fs::read_dir(&versoes_dir) {
        for ent in rd.flatten() {
            if ent.path().is_file() {
                objeto = Some(ent.path());
                break;
            }
        }
    }
    if let Some(obj) = objeto {
        let antes = std::fs::read(&obj).unwrap();
        let fisico = pasta.join("fisico_da_versao.musicxml");
        std::fs::hard_link(&obj, &fisico).unwrap();
        e.sobrescrever = true;
        let r = e.exportar(&fisico.to_string_lossy());
        println!(
            "SONDA3 [E6 hard link de objeto de versao] r={:?} objeto_intacto={}",
            r.as_ref().map(|_| ()).map_err(|e| e.clone()),
            std::fs::read(&obj).unwrap() == antes
        );
    } else {
        println!("SONDA3 [E6] sem objeto de versao em {versoes_dir:?}: {:?}", std::fs::read_dir(xml.parent().unwrap()).map(|r| r.flatten().map(|e| e.file_name()).collect::<Vec<_>>()));
    }

    // E7: exportar em leitura (documento "(leitura)")
    let hip = hash_do_conteudo(EMBUTIDAS[3].1.as_bytes());
    e.abrir_versao(&hip);
    e.sobrescrever = false;
    let r = e.exportar(&pasta.join("da_leitura.musicxml").to_string_lossy());
    println!("SONDA3 [E7 exportar em leitura] r={:?}", r.as_ref().map(|_| ()).map_err(|e| e.clone()));
    // E8: caminho vazio / "." / com NUL
    for c in ["", ".", "\0x.musicxml", "   "] {
        let r = e.exportar(c);
        println!("SONDA3 [E8 destino {c:?}] r={:?}", r.as_ref().map(|_| ()).map_err(|e| e.clone()));
    }
}

#[test]
fn zz3_app_d_leitura_com_rascunho_que_nao_abre() {
    let (_d, mut e, xml) = com_oficina();
    editar_uma_nota(&mut e);
    let v1 = criada(e.salvar_versao().expect("v1"));
    drop(e);
    let pasta = xml.parent().unwrap().to_path_buf();
    let cfg = crate::autosave::ConfigAutosave::default();
    let a = crate::autosave::Autosave::em(&pasta, "ode", &cfg).unwrap();
    a.gravador().gravar_com_ids("isto nao e musicxml", None, "2026-01-01T00:00:00Z").unwrap();
    drop(a);
    let mut n = EstadoPartitura::novo(None);
    n.abrir_caminho(&xml.to_string_lossy());
    n.abrir_versao(&v1.hash);
    st("D1 leitura aberta", &n, &xml, "");
    n.recuperar_rascunho();
    st("D2 recuperar falho em leitura", &n, &xml, "");
    n.descartar_rascunho();
    st("D3 descartar em leitura (aviso velho?)", &n, &xml, "");
    n.voltar_da_leitura();
    st("D4 voltar", &n, &xml, "");
    println!("SONDA3 [D] edita={}", tentar_editar(&mut n));
}
