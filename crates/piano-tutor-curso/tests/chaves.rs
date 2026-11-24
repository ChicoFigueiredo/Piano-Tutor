//! Testes das chaves ed25519 (geração, hex, arquivos).
#![allow(clippy::unwrap_used)] // testes: falhar com panic é o comportamento desejado

use piano_tutor_curso::{ChavePrivada, ChavePublica, ErroPacote, gerar_par};

#[test]
fn par_gerado_assina_e_verifica() {
    let (privada, publica) = gerar_par();
    assert_eq!(privada.publica(), publica);
    let assinatura = privada.assinar(b"ola");
    assert!(publica.verificar(b"ola", &assinatura));
    assert!(!publica.verificar(b"ola!", &assinatura));
}

#[test]
fn pares_gerados_sao_diferentes() {
    assert_ne!(gerar_par().1, gerar_par().1);
}

#[test]
fn hex_vai_e_volta_e_rejeita_lixo() {
    let (privada, publica) = gerar_par();
    assert_eq!(ChavePublica::de_hex(&publica.para_hex()).unwrap(), publica);
    assert_eq!(publica.para_hex().len(), 64);
    assert_eq!(
        ChavePrivada::de_hex(&privada.para_hex()).unwrap().publica(),
        publica
    );
    assert!(matches!(
        ChavePublica::de_hex("zz"),
        Err(ErroPacote::ChaveInvalida(_))
    ));
    assert!(matches!(
        ChavePublica::de_hex(&"g".repeat(64)),
        Err(ErroPacote::ChaveInvalida(_))
    ));
    assert!(matches!(
        ChavePrivada::de_hex("abcd"),
        Err(ErroPacote::ChaveInvalida(_))
    ));
}

#[test]
fn chave_curta_mostra_inicio_e_fim() {
    let (_, publica) = gerar_par();
    let hex = publica.para_hex();
    assert_eq!(publica.curta(), format!("{}…{}", &hex[..4], &hex[60..]));
}

#[test]
fn debug_da_privada_nao_vaza_o_segredo() {
    let (privada, _) = gerar_par();
    assert!(!format!("{privada:?}").contains(&privada.para_hex()));
}

#[test]
fn arquivos_de_chave_ida_e_volta_com_permissao_0600() {
    let pasta = tempfile::tempdir().unwrap();
    let (privada, publica) = gerar_par();
    let caminho_priv = pasta.path().join("curso.chave");
    let caminho_pub = pasta.path().join("curso.pub");
    privada.gravar(&caminho_priv).unwrap();
    publica.gravar(&caminho_pub).unwrap();
    assert_eq!(ChavePrivada::ler(&caminho_priv).unwrap().publica(), publica);
    assert_eq!(ChavePublica::ler(&caminho_pub).unwrap(), publica);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let modo = std::fs::metadata(&caminho_priv)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(modo & 0o777, 0o600);
    }

    // Não sobrescreve chave existente.
    assert!(matches!(
        privada.gravar(&caminho_priv),
        Err(ErroPacote::Io(_))
    ));
}
