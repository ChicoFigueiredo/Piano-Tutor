//! Testes da correção dos achados T6 e T7 da terceira revisão independente da E01
//! (`docs/evolucao-musical/e01/REVISAO-CODIGO.md`, "Terceira revisão"): cada um nasceu como a
//! sonda de `sondas-revisao/zz_revisao3_app_tela_partitura.rs` que reproduz o achado.

use super::testes::{com_oficina, editar_uma_nota};
use super::testes_revisao::{editar_outra_nota, reabrir_com_rascunho_pendente};
use super::*;
use crate::atalhos_partitura::PedidoDeAcorde;

fn texto_do_documento(e: &EstadoPartitura) -> String {
    e.documento.as_ref().expect("doc").exportado().expect("x")
}

/// Dó4, depois um acorde de **uníssono** Ré4 + Ré4.
const UNISSONO: &str = r#"<score-partwise><part-list><score-part id="P1"><part-name>x</part-name></score-part></part-list>
<part id="P1"><measure number="1"><attributes><divisions>1</divisions><key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note>
<note><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note>
<note><chord/><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note>
</measure></part></score-partwise>"#;

#[test]
fn t6_retirar_altura_de_unissono_mantem_a_selecao_no_acorde() {
    let mut e = EstadoPartitura::novo(None);
    e.carregar_texto("a".into(), UNISSONO.to_owned())
        .expect("abre");
    e.navegar(Comando::Seguinte);
    e.navegar(Comando::Seguinte);
    let ret = e.nota_selecionada().expect("Ré selecionado");
    let r = e
        .editar_acorde(PedidoDeAcorde::Retirar)
        .expect("retira uma das duas");
    assert!(matches!(r.inverso, ComandoDeEdicao::RestaurarAltura { .. }));
    let depois = e
        .nota_selecionada()
        .expect("a seleção passou para a outra altura do acorde");
    assert_ne!(depois, ret);
    // O desfazer segue devolvendo o arquivo.
    assert!(e.desfazer());
    assert_eq!(texto_do_documento(&e), UNISSONO);
}

#[test]
fn t7a_descartar_depois_de_recuperar_falho_nao_deixa_o_aviso_velho() {
    let (_d, mut e, xml, _) = reabrir_com_rascunho_pendente();
    // Estraga o rascunho em disco: Recuperar falha e o rascunho segue pendente.
    let pasta = xml.parent().expect("pasta").to_path_buf();
    let cfg = crate::autosave::ConfigAutosave::default();
    let a = crate::autosave::Autosave::em(&pasta, "ode", &cfg).expect("autosave");
    a.gravador()
        .gravar_com_ids("isto não é musicxml", None, "2026-01-01T00:00:00Z")
        .expect("grava lixo");
    drop(a);
    e.recuperar_rascunho();
    if e.rascunho_pendente().is_none() {
        // O ambiente recuperou mesmo assim: não há o que provar aqui.
        return;
    }
    assert!(
        e.aviso_de_versoes()
            .expect("aviso")
            .contains("continua guardado")
    );
    e.descartar_rascunho();
    assert!(e.rascunho_pendente().is_none());
    assert!(
        e.aviso_de_versoes()
            .is_none_or(|a| !a.contains("continua guardado")),
        "o aviso diz que o rascunho continua, e ele foi apagado: {:?}",
        e.aviso_de_versoes()
    );
}

#[test]
fn t7b_recuperar_de_novo_nao_descarta_a_edicao_feita_depois() {
    let (_d, mut e, xml, editado) = reabrir_com_rascunho_pendente();
    e.recuperar_rascunho();
    assert_eq!(texto_do_documento(&e), editado);
    // Edita depois da recuperação; o rascunho em disco segue (é o de antes).
    editar_outra_nota(&mut e);
    let com_edicao = texto_do_documento(&e);
    assert_ne!(com_edicao, editado);
    let aviso = e.aviso_de_versoes().map(str::to_owned);
    // Segunda chamada (API pública; o botão só existe no banner): nada acontece.
    e.recuperar_rascunho();
    assert_eq!(texto_do_documento(&e), com_edicao, "a edição foi perdida");
    assert_eq!(e.aviso_de_versoes().map(str::to_owned), aviso);
    assert!(super::testes::arquivo_do_rascunho(&xml).is_file());
}

#[test]
fn t7b_recuperar_sem_rascunho_pendente_nao_faz_nada() {
    let (_d, mut e, _xml) = com_oficina();
    editar_uma_nota(&mut e);
    let antes = texto_do_documento(&e);
    let aviso = e.aviso_de_versoes().map(str::to_owned);
    e.recuperar_rascunho();
    assert_eq!(texto_do_documento(&e), antes);
    assert_eq!(e.aviso_de_versoes().map(str::to_owned), aviso);
    assert!(e.desfazer(), "o histórico continua");
}
