//! Testes da correção do achado R2 da revisão independente da E01
//! (`docs/evolucao-musical/e01/REVISAO-CODIGO.md`): exportar nunca sobrescreve a Hipótese, o
//! arquivo aberto, os objetos de Versão nem o `.ids` lateral deles, e grava atômico.

use super::testes::{com_oficina, editar_uma_nota};
use super::*;

fn editar_outra_nota(e: &mut EstadoPartitura) {
    e.navegar(Comando::Seguinte);
    e.navegar(Comando::Seguinte);
    assert!(
        e.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Abaixo)
            .is_some()
    );
}

#[test]
fn r2_exportar_sobre_a_hipotese_e_recusado_e_o_arquivo_fica_intacto() {
    let (_d, mut e, xml) = com_oficina();
    let antes = std::fs::read(&xml).expect("lê");
    editar_uma_nota(&mut e);
    e.sobrescrever = true;
    let erro = e.exportar(&xml.to_string_lossy()).expect_err("recusa");
    assert!(
        erro.contains("Hipótese") || erro.contains("original"),
        "{erro}"
    );
    assert_eq!(std::fs::read(&xml).expect("lê"), antes, "Hipótese intacta");
    assert!(
        !crate::ids_lateral::caminho_lateral(&xml).exists(),
        "nenhum .ids lateral nasceu ao lado da Hipótese"
    );
    assert!(e.alerta_de_originais().is_none());
}

#[test]
fn r2_o_mesmo_arquivo_por_outro_caminho_tambem_e_recusado() {
    let (_d, mut e, xml) = com_oficina();
    let antes = std::fs::read(&xml).expect("lê");
    editar_uma_nota(&mut e);
    e.sobrescrever = true;
    // `partituras/../partituras/./ode.musicxml` aponta para o mesmo arquivo.
    let pasta = xml.parent().expect("pasta");
    let rodeio = pasta
        .join("..")
        .join("partituras")
        .join(".")
        .join("ode.musicxml");
    assert!(e.exportar(&rodeio.to_string_lossy()).is_err());
    assert_eq!(std::fs::read(&xml).expect("lê"), antes);
    #[cfg(unix)]
    {
        // Um atalho (link simbólico) para a Hipótese também.
        let atalho = pasta.parent().expect("raiz").join("atalho.musicxml");
        std::os::unix::fs::symlink(&xml, &atalho).expect("link");
        assert!(e.exportar(&atalho.to_string_lossy()).is_err());
        // Um link físico (mesmo arquivo, outro nome) também.
        let fisico = pasta.parent().expect("raiz").join("fisico.musicxml");
        std::fs::hard_link(&xml, &fisico).expect("hard link");
        assert!(e.exportar(&fisico.to_string_lossy()).is_err());
        assert_eq!(std::fs::read(&xml).expect("lê"), antes);
    }
}

#[test]
fn r2_exportar_sobre_objeto_de_versao_ou_sobre_a_pasta_dela_e_recusado() {
    let (_d, mut e, xml) = com_oficina();
    editar_uma_nota(&mut e);
    let v1 = match e.salvar_versao().expect("salva") {
        ResultadoDoSalvar::Criada(v) => v,
        outro => panic!("{outro:?}"),
    };
    let objetos = xml.with_file_name("ode.versoes").join("objetos");
    let objeto = objetos.join(format!("{}.musicxml", v1.hash));
    let antes = std::fs::read(&objeto).expect("objeto");
    editar_outra_nota(&mut e);
    e.sobrescrever = true;
    assert!(e.exportar(&objeto.to_string_lossy()).is_err());
    assert_eq!(std::fs::read(&objeto).expect("lê"), antes, "objeto intacto");
    // Qualquer destino dentro da pasta de Versões (o `.ids` lateral dele cairia ali) também.
    let outro = objetos.join("qualquer.xml");
    assert!(e.exportar(&outro.to_string_lossy()).is_err());
    assert!(!outro.exists());
    assert!(
        e.versoes
            .as_ref()
            .expect("oficina")
            .ids_da_versao(&v1.hash)
            .is_some()
    );
}

#[test]
fn r2_exportar_sobre_a_hipotese_de_outro_item_da_oficina_e_recusado() {
    let (_d, mut e, xml) = com_oficina();
    let pasta = xml.parent().expect("pasta");
    let outra = pasta.join("outra.musicxml");
    std::fs::write(&outra, EMBUTIDAS[3].1).expect("outra hipótese");
    std::fs::write(pasta.join("outra.estado.toml"), "estado = \"hipotese\"\n").expect("toml");
    editar_uma_nota(&mut e);
    e.sobrescrever = true;
    assert!(e.exportar(&outra.to_string_lossy()).is_err());
    assert_eq!(
        std::fs::read_to_string(&outra).expect("lê"),
        EMBUTIDAS[3].1,
        "a Hipótese do outro item fica como estava"
    );
}

#[test]
fn r2_exportar_de_arquivo_solto_nao_sobrescreve_o_proprio_arquivo_aberto() {
    let d = tempfile::tempdir().expect("pasta");
    let solto = d.path().join("solta.musicxml");
    std::fs::write(&solto, EMBUTIDAS[3].1).expect("escreve");
    let mut e = EstadoPartitura::novo(None);
    e.abrir_caminho(&solto.to_string_lossy());
    assert!(e.erro.is_none());
    e.navegar(Comando::Seguinte);
    assert!(
        e.mudar_altura(UnidadeDeAltura::Semitom, SentidoDaAltura::Acima)
            .is_some()
    );
    e.sobrescrever = true;
    assert!(e.exportar(&solto.to_string_lossy()).is_err());
    assert_eq!(std::fs::read_to_string(&solto).expect("lê"), EMBUTIDAS[3].1);
}

/// Gravar por tmp+rename troca o arquivo inteiro: quem tinha o arquivo antigo aberto (aqui, um
/// link físico) continua vendo o conteúdo antigo completo; truncar e escrever no lugar o
/// reescreveria (e uma queda no meio deixaria o arquivo pela metade).
#[cfg(unix)]
#[test]
fn r2_sobrescrever_troca_o_arquivo_por_rename_e_nao_reescreve_no_lugar() {
    let (d, mut e, _xml) = com_oficina();
    editar_uma_nota(&mut e);
    let destino = d.path().join("saida.musicxml");
    e.exportar(&destino.to_string_lossy()).expect("exporta");
    let primeiro = std::fs::read(&destino).expect("lê");
    let vizinho = d.path().join("vizinho.musicxml");
    std::fs::hard_link(&destino, &vizinho).expect("link");
    e.sobrescrever = true;
    editar_outra_nota(&mut e);
    e.exportar(&destino.to_string_lossy()).expect("sobrescreve");
    assert_ne!(std::fs::read(&destino).expect("lê"), primeiro);
    assert_eq!(
        std::fs::read(&vizinho).expect("lê"),
        primeiro,
        "o inode antigo ficou intacto: houve rename, não reescrita no lugar"
    );
}

#[test]
fn r2_exportar_para_outro_lugar_grava_atomico_sem_sobrar_temporario() {
    let (d, mut e, _xml) = com_oficina();
    editar_uma_nota(&mut e);
    let editado = e.documento.as_ref().expect("doc").exportado().expect("x");
    let destino = d.path().join("saida.musicxml");
    let c = e.exportar(&destino.to_string_lossy()).expect("exporta");
    assert_eq!(c, destino);
    assert_eq!(std::fs::read_to_string(&destino).expect("lê"), editado);
    assert!(crate::ids_lateral::caminho_lateral(&destino).is_file());
    // Sem "Sobrescrever", um arquivo que já existe fica como está.
    e.sobrescrever = false;
    assert!(e.exportar(&destino.to_string_lossy()).is_err());
    // Com "Sobrescrever", troca por tmp+rename: nenhum temporário `.tmp` fica na pasta.
    e.sobrescrever = true;
    editar_outra_nota(&mut e);
    e.exportar(&destino.to_string_lossy()).expect("sobrescreve");
    let restos: Vec<_> = std::fs::read_dir(d.path())
        .expect("lê a pasta")
        .filter_map(Result::ok)
        .filter(|f| f.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(restos.is_empty(), "{restos:?}");
}
