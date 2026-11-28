//! Durações em `Fracao` exata no spike (ticket 08): `divisions` é o MMC dos denominadores e o que
//! não couber vira erro explícito, nunca arredondamento.

use piano_tutor_core::partitura::Fracao;
use piano_tutor_fixtures::documento::{Documento, ErroDocumento, mmc_dos_denominadores};
use piano_tutor_fixtures::harness::comparar_semantica;

const CABECA: &str = "<?xml version=\"1.0\"?>\n<score-partwise version=\"4.0\"><part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list>\n<part id=\"P1\">\n";

fn nota(passo: &str, dur: u32) -> String {
    format!(
        "<note><pitch><step>{passo}</step><octave>4</octave></pitch><duration>{dur}</duration><voice>1</voice></note>"
    )
}

/// Dois compassos com `backup`, `divisions=1`.
fn peca(extra_no_compasso_2: &str) -> String {
    format!(
        "{CABECA}<measure number=\"1\"><attributes><divisions>1</divisions></attributes>{}{}</measure>\n<measure number=\"2\">{}<backup><duration>4</duration></backup>{}{}{extra_no_compasso_2}</measure>\n</part></score-partwise>\n",
        nota("C", 2),
        nota("D", 2),
        nota("E", 4),
        nota("G", 2),
        nota("A", 2),
    )
}

fn f(n: i64, d: i64) -> Fracao {
    Fracao::nova(n, d)
}

#[test]
fn divisions_e_o_mmc_dos_denominadores() {
    assert_eq!(mmc_dos_denominadores(&[]).expect("vazio"), 1);
    assert_eq!(
        mmc_dos_denominadores(&[f(1, 2), f(1, 3), f(3, 4)]).expect("mmc"),
        12
    );
    assert_eq!(
        mmc_dos_denominadores(&[f(5, 1), f(7, 1)]).expect("inteiros"),
        1
    );
    assert_eq!(mmc_dos_denominadores(&[f(1, 6), f(1, 4)]).expect("mmc"), 12);
}

#[test]
fn mmc_que_estoura_e_erro_explicito() {
    let r = mmc_dos_denominadores(&[f(1, i64::MAX), f(1, i64::MAX - 1)]);
    assert!(matches!(r, Err(ErroDocumento::Overflow(_))), "{r:?}");
}

#[test]
fn valor_nao_representavel_vira_erro_e_nada_e_arredondado() {
    let xml = peca("");
    let mut doc = Documento::abrir(&xml).expect("abre");
    let id = doc.notas().next().expect("nota").id;
    let r = doc.definir_duracao(id, f(1, 3));
    assert_eq!(
        r,
        Err(ErroDocumento::DuracaoNaoRepresentavel {
            valor: f(1, 3),
            divisions: 1,
            necessarias: 3
        })
    );
    assert!(doc.compassos_sujos().is_empty(), "recusar não suja nada");
    assert_eq!(doc.exportar().expect("exporta"), xml);
    // Duração nula ou negativa também é recusada.
    assert!(doc.definir_duracao(id, f(0, 1)).is_err());
    assert!(doc.definir_duracao(id, f(-1, 2)).is_err());
}

#[test]
fn valor_representavel_muda_so_o_compasso_da_nota() {
    let xml = peca("");
    let mut doc = Documento::abrir(&xml).expect("abre");
    let id = doc.notas().next().expect("nota").id;
    doc.definir_duracao(id, f(3, 1)).expect("3 semínimas cabem");
    let saida = doc.exportar().expect("exporta");
    assert_eq!(
        saida,
        xml.replacen("<duration>2</duration>", "<duration>3</duration>", 1)
    );
    assert_eq!(doc.nota(id).expect("nota").duracao, Some(f(3, 1)));
}

#[test]
fn ajustando_divisions_a_parte_vai_ao_mmc_e_so_a_nota_muda_de_valor() {
    let xml = peca("");
    let mut doc = Documento::abrir(&xml).expect("abre");
    let id = doc.notas().next().expect("nota").id;
    doc.definir_duracao_ajustando_divisions(id, f(1, 3))
        .expect("ajusta");
    assert_eq!(doc.divisions_da_parte("P1"), Some(3));
    let saida = doc.exportar().expect("exporta");
    let esperado = xml
        .replace("<divisions>1</divisions>", "<divisions>3</divisions>")
        .replacen("<duration>2</duration>", "<duration>1</duration>", 1) // 1/3 × 3
        .replacen("<duration>2</duration>", "<duration>6</duration>", 1)
        .replace("<duration>4</duration>", "<duration>12</duration>")
        .replace("<duration>2</duration>", "<duration>6</duration>");
    assert_eq!(saida, esperado);
    // Semântica: só a duração da primeira nota mudou (a música das outras é a mesma).
    let rel = comparar_semantica(&xml, &saida).expect("semântica");
    assert_eq!(rel.compassos_diferentes.len(), 1, "{:?}", rel.detalhes);
    assert_eq!(rel.compassos_diferentes[0].indice, 0);
    // Todos os compassos da parte foram reescritos: é o custo de mudar `divisions`.
    assert_eq!(doc.compassos_sujos().len(), 2);
}

#[test]
fn ajustar_divisions_com_offset_e_recusado_e_o_documento_fica_intacto() {
    let xml = peca(
        "<direction><direction-type><words>x</words></direction-type><offset>1</offset></direction>",
    );
    let mut doc = Documento::abrir(&xml).expect("abre");
    let id = doc.notas().next().expect("nota").id;
    let r = doc.definir_duracao_ajustando_divisions(id, f(1, 3));
    assert!(matches!(r, Err(ErroDocumento::NaoSuportado(_))), "{r:?}");
    assert!(doc.compassos_sujos().is_empty());
    assert_eq!(doc.divisions_da_parte("P1"), Some(1));
    assert_eq!(doc.exportar().expect("exporta"), xml);
}

#[test]
fn ajustar_divisions_que_estoura_e_erro_e_o_documento_fica_intacto() {
    let xml = peca("");
    let mut doc = Documento::abrir(&xml).expect("abre");
    let id = doc.notas().next().expect("nota").id;
    let r = doc.definir_duracao_ajustando_divisions(id, f(1, i64::MAX));
    assert!(matches!(r, Err(ErroDocumento::Overflow(_))), "{r:?}");
    assert!(doc.compassos_sujos().is_empty());
    assert_eq!(doc.exportar().expect("exporta"), xml);
}

#[test]
fn parte_com_divisions_declarado_duas_vezes_nao_e_reescalada() {
    let xml = format!(
        "{CABECA}<measure number=\"1\"><attributes><divisions>1</divisions></attributes>{}</measure>\n<measure number=\"2\"><attributes><divisions>2</divisions></attributes>{}</measure>\n</part></score-partwise>\n",
        nota("C", 1),
        nota("D", 2)
    );
    let mut doc = Documento::abrir(&xml).expect("abre");
    let id = doc.notas().next().expect("nota").id;
    let r = doc.definir_duracao_ajustando_divisions(id, f(1, 3));
    assert!(matches!(r, Err(ErroDocumento::NaoSuportado(_))), "{r:?}");
    assert_eq!(doc.exportar().expect("exporta"), xml);
}

/// Caracterização, não aprovação: a aritmética do `Fracao` do núcleo satura em silêncio no
/// estouro (clamp para `i64::MAX`). O spike evita isso com `i128` e erros explícitos; os tickets
/// de edição de duração (16) não podem confiar em `Fracao + Fracao` perto do limite.
#[test]
fn fracao_do_nucleo_satura_em_silencio_no_estouro() {
    let soma = f(1, i64::MAX) + f(1, i64::MAX - 1);
    assert_eq!(
        soma.denominador(),
        i64::MAX,
        "se isto falhar, o núcleo passou a sinalizar: ótimo"
    );
}
