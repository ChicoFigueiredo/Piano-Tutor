//! Testes do próprio harness (byte a byte, por compasso e semântico).

use piano_tutor_fixtures::corpus::fixture;
use piano_tutor_fixtures::harness::{
    PosCompasso, Violacao, comparar_bytes, comparar_semantica, iguais_byte_a_byte, verificar_golden,
};

/// Troca a altura da primeira nota do compasso de rótulo `numero` (parte única de `cem-compassos`).
fn alterar_altura(xml: &str, numero: usize) -> String {
    let marca = format!("<measure number=\"{numero}\">");
    let ini = xml.find(&marca).expect("compasso existe");
    let step = xml[ini..].find("<step>").expect("tem nota") + ini + "<step>".len();
    let atual = &xml[step..step + 1];
    let novo = if atual == "C" { "D" } else { "C" };
    format!("{}{}{}", &xml[..step], novo, &xml[step + 1..])
}

fn longa() -> String {
    fixture("cem-compassos").conteudo
}

#[test]
fn duas_copias_identicas_passam() {
    let a = longa();
    let b = a.clone();
    assert!(iguais_byte_a_byte(&a, &b));
    let r = comparar_bytes(&a, &b).expect("compara");
    assert!(r.identicos && r.compassos_diferentes.is_empty() && !r.fora_dos_compassos);
    assert!(comparar_semantica(&a, &b).expect("compara").equivalentes());
    assert_eq!(verificar_golden(&a, &b, &[], Some(&a)), Ok(()));
}

#[test]
fn alteracao_de_altura_e_reportada_exatamente_no_compasso() {
    let a = longa();
    let b = alterar_altura(&a, 37);
    let bytes = comparar_bytes(&a, &b).expect("compara");
    assert!(!bytes.identicos && !bytes.fora_dos_compassos);
    assert_eq!(
        bytes.compassos_diferentes,
        vec![PosCompasso::nova("P1", 36)]
    );
    assert_eq!(bytes.compassos_diferentes[0].numero, "37");
    let sem = comparar_semantica(&a, &b).expect("compara");
    assert!(!sem.equivalentes());
    assert_eq!(sem.compassos_diferentes, vec![PosCompasso::nova("P1", 36)]);
}

#[test]
fn edicao_dentro_do_compasso_permitido_passa_e_fora_dele_reprova() {
    let a = longa();
    let editavel = [PosCompasso::nova("P1", 36)];
    let dentro = alterar_altura(&a, 37);
    assert_eq!(
        verificar_golden(&a, &dentro, &editavel, Some(&dentro)),
        Ok(())
    );

    let fora = alterar_altura(&a, 50);
    let erro = verificar_golden(&a, &fora, &editavel, None).expect_err("deve reprovar");
    assert_eq!(
        erro,
        vec![Violacao::CompassoAlterado(PosCompasso::nova("P1", 49))]
    );

    // Mudança do cabeçalho (fora de qualquer compasso) também reprova.
    let titulo = a.replace("Cem compassos", "Outro título");
    let erro = verificar_golden(&a, &titulo, &editavel, None).expect_err("deve reprovar");
    assert_eq!(erro, vec![Violacao::ForaDosCompassosAlterado]);
}

#[test]
fn resultado_diferente_do_esperado_aponta_o_compasso() {
    let a = longa();
    let esperado = alterar_altura(&a, 10);
    // O obtido alterou o compasso 11 em vez do 10.
    let obtido = alterar_altura(&a, 11);
    let editaveis = [PosCompasso::nova("P1", 9), PosCompasso::nova("P1", 10)];
    let erro = verificar_golden(&a, &obtido, &editaveis, Some(&esperado)).expect_err("reprova");
    assert!(erro.contains(&Violacao::SemanticaDivergente(PosCompasso::nova("P1", 9))));
    assert!(erro.contains(&Violacao::SemanticaDivergente(PosCompasso::nova("P1", 10))));
    assert_eq!(erro.len(), 2);
}

#[test]
fn semantica_ignora_formatacao_mas_bytes_nao() {
    let a = fixture("letra").conteudo;
    // Só formatação: indentação, atributos de posição, haste e vigas, ordem dos versos.
    let b = a
        .replace("<note>", "<note default-x=\"12\">\n          ")
        .replace(
            "<type>quarter</type>",
            "<type>quarter</type><stem>up</stem><beam number=\"1\">begin</beam>",
        )
        .replace("</measure>", "\n\n    </measure>");
    assert!(!iguais_byte_a_byte(&a, &b));
    let bytes = comparar_bytes(&a, &b).expect("compara");
    assert_eq!(bytes.compassos_diferentes.len(), 2);
    assert!(comparar_semantica(&a, &b).expect("compara").equivalentes());
}

#[test]
fn semantica_enxerga_letra_cifra_dedilhado_voz_e_duracao() {
    let casos = [
        ("letra", "<text>Pi</text>", "<text>Pa</text>"),
        (
            "cifra",
            "<root-step>G</root-step>",
            "<root-step>D</root-step>",
        ),
        (
            "dedilhado",
            "<fingering>2</fingering>",
            "<fingering>3</fingering>",
        ),
        (
            "duas-vozes",
            "<voice>2</voice><type>whole</type>",
            "<voice>3</voice><type>whole</type>",
        ),
        (
            "anacruse",
            "<duration>2</duration>",
            "<duration>1</duration>",
        ),
        (
            "tercina",
            "<actual-notes>3</actual-notes>",
            "<actual-notes>5</actual-notes>",
        ),
    ];
    for (id, de, para) in casos {
        let a = fixture(id).conteudo;
        assert!(a.contains(de), "{id}: trecho '{de}' não encontrado");
        let b = a.replacen(de, para, 1);
        let rel = comparar_semantica(&a, &b).expect("compara");
        assert_eq!(
            rel.compassos_diferentes.len(),
            1,
            "{id}: {:?}",
            rel.detalhes
        );
    }
}

#[test]
fn compasso_extra_ou_faltando_conta_como_diferenca() {
    let a = fixture("anacruse").conteudo;
    let ini = a.find("<measure number=\"2\">").expect("compasso 2");
    let fim = a
        .find("</measure>")
        .map(|_| a.rfind("</measure>").expect("fim"))
        .expect("fim")
        + "</measure>".len();
    let b = format!("{}{}", &a[..ini], &a[fim..]);
    let bytes = comparar_bytes(&a, &b).expect("compara");
    assert_eq!(bytes.compassos_diferentes, vec![PosCompasso::nova("P1", 2)]);
    let sem = comparar_semantica(&a, &b).expect("compara");
    assert_eq!(sem.compassos_diferentes, vec![PosCompasso::nova("P1", 2)]);
}

#[test]
fn xml_invalido_vira_erro_e_nao_panico() {
    assert!(comparar_bytes("<a>", "<a/>").is_err());
    assert!(comparar_semantica("<a/>", "<a>").is_err());
}
