//! Texto `chave=valor` genérico das opções de tela que vão para o perfil (ADR-0010).
//!
//! Cada tela tira um retrato das suas opções ([`Kv`]) e o app compara com o último gravado a cada
//! quadro: mudou, grava na hora no perfil ativo. Enums são gravados pelo nome da variante
//! (`Debug`) e lidos de volta procurando entre os valores possíveis, então um valor antigo ou
//! inválido simplesmente fica no padrão.

use std::fmt::{Debug, Display};
use std::str::FromStr;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Kv(Vec<(String, String)>);

impl Kv {
    pub fn novo() -> Self {
        Self::default()
    }

    /// Valor com `Display` (números, bool, texto).
    pub fn por(&mut self, chave: &str, valor: impl Display) -> &mut Self {
        let v = valor.to_string().replace(['\n', '\r'], " ");
        self.0.push((chave.to_owned(), v));
        self
    }

    /// Enum (ou `Option` de enum) pelo `Debug`.
    pub fn por_enum(&mut self, chave: &str, valor: impl Debug) -> &mut Self {
        self.por(chave, format!("{valor:?}"))
    }

    pub fn texto(&self) -> String {
        let mut s = String::new();
        for (k, v) in &self.0 {
            s.push_str(k);
            s.push('=');
            s.push_str(v);
            s.push('\n');
        }
        s
    }

    pub fn de_texto(t: &str) -> Self {
        Self(
            t.lines()
                .filter_map(|l| {
                    let l = l.trim();
                    if l.starts_with('#') {
                        return None;
                    }
                    let (k, v) = l.split_once('=')?;
                    Some((k.trim().to_owned(), v.trim().to_owned()))
                })
                .collect(),
        )
    }

    fn bruto(&self, chave: &str) -> Option<&str> {
        self.0
            .iter()
            .rev()
            .find(|(k, _)| k == chave)
            .map(|(_, v)| v.as_str())
    }

    /// Valor que se lê com `FromStr`; ausente ou inválido = `None`.
    pub fn ler<T: FromStr>(&self, chave: &str) -> Option<T> {
        self.bruto(chave)?.parse().ok()
    }

    /// Enum pelo nome da variante, entre `opcoes`.
    pub fn ler_enum<T: Debug + Clone>(&self, chave: &str, opcoes: &[T]) -> Option<T> {
        let v = self.bruto(chave)?;
        opcoes.iter().find(|o| format!("{o:?}") == v).cloned()
    }

    /// Atribui se a chave existir e for válida (atalho para aplicar um retrato).
    pub fn em<T: FromStr>(&self, chave: &str, alvo: &mut T) {
        if let Some(v) = self.ler(chave) {
            *alvo = v;
        }
    }

    pub fn em_enum<T: Debug + Clone>(&self, chave: &str, opcoes: &[T], alvo: &mut T) {
        if let Some(v) = self.ler_enum(chave, opcoes) {
            *alvo = v;
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Cor {
        Azul,
        Verde,
    }

    #[test]
    fn ida_e_volta_e_lixo_fica_no_padrao() {
        let mut kv = Kv::novo();
        kv.por("zoom", 1.25_f32)
            .por("ajustar", true)
            .por_enum("cor", Cor::Verde)
            .por_enum("opcional", Some(Cor::Azul));
        let lido = Kv::de_texto(&kv.texto());
        assert_eq!(lido.ler::<f32>("zoom"), Some(1.25));
        assert_eq!(lido.ler::<bool>("ajustar"), Some(true));
        assert_eq!(
            lido.ler_enum("cor", &[Cor::Azul, Cor::Verde]),
            Some(Cor::Verde)
        );
        assert_eq!(
            lido.ler_enum("opcional", &[None, Some(Cor::Azul), Some(Cor::Verde)]),
            Some(Some(Cor::Azul))
        );
        let lixo = Kv::de_texto("cor=Roxo\nzoom=abc");
        let mut c = Cor::Azul;
        lixo.em_enum("cor", &[Cor::Azul, Cor::Verde], &mut c);
        assert_eq!(c, Cor::Azul);
        assert_eq!(lixo.ler::<f32>("zoom"), None);
    }
}
