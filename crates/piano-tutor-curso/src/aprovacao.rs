//! Aprovação por hash, lida do índice de Versões da Oficina (tickets 23 e 25).
//!
//! A aprovação é atributo da **Versão** (SHA-256 do conteúdo), nunca do nome do arquivo nem do
//! `estado.toml`. Este módulo é a leitura mínima e única da regra para quem só consome: o
//! importador (CLI e app) decide o estado de cada Exercício por aqui. O app escreve o índice
//! (`piano-tutor-app`, módulo `versoes`) e usa estas mesmas constantes e esta mesma regra.
//!
//! Layout (ver `versoes.rs` do app): `<oficina>/partituras/<id>.versoes/indice.toml`, com uma
//! `[[versao]]` por Versão (`hash`, `estado`) e um registro append-only `[[decisao]]`
//! (`hash`, `para`, ...).
//!
//! Regra: **Aprovada** só se a `[[versao]]` do hash está `aprovada` **e** a última `[[decisao]]`
//! sobre esse hash tem `para = "aprovada"`. Estado `aprovada` escrito à mão, sem registro,
//! conta como Hipótese. Hash sem Versão: sem decisão (`None`).

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::erro::ErroPacote;
use crate::importacao::DecideEstado;
use crate::modelo::EstadoConteudo;

/// Nome do arquivo de índice dentro da pasta de Versões.
pub const ARQUIVO_DO_INDICE: &str = "indice.toml";

/// Pasta das Versões da partitura `id` que mora em `pasta_partituras`.
pub fn pasta_das_versoes(pasta_partituras: &Path, id: &str) -> PathBuf {
    pasta_partituras.join(format!("{id}.versoes"))
}

#[derive(Debug, Default, Deserialize)]
struct Indice {
    #[serde(default)]
    versao: Vec<Versao>,
    #[serde(default)]
    decisao: Vec<Decisao>,
}

#[derive(Debug, Deserialize)]
struct Versao {
    hash: String,
    estado: String,
}

#[derive(Debug, Deserialize)]
struct Decisao {
    hash: String,
    para: String,
}

/// SHA-256 do conteúdo em hexadecimal minúsculo (a identidade da Versão).
pub fn hash_do_conteudo(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// O estado do conteúdo `bytes` segundo o índice em `indice` (por hash). `None` = índice ausente
/// ou nenhuma Versão com esse hash. Índice ilegível ou malformado é erro, nunca aprovação.
pub fn estado_do_conteudo_no_indice(
    indice: &Path,
    bytes: &[u8],
) -> Result<Option<EstadoConteudo>, ErroPacote> {
    let texto = match fs::read_to_string(indice) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(ErroPacote::Importacao(format!(
                "índice de Versões {}: {e}",
                indice.display()
            )));
        }
    };
    let lido: Indice = toml::from_str(&texto).map_err(|e| {
        ErroPacote::Importacao(format!("índice de Versões {}: {e}", indice.display()))
    })?;
    Ok(estado_por_hash(&lido, &hash_do_conteudo(bytes)))
}

fn estado_por_hash(indice: &Indice, hash: &str) -> Option<EstadoConteudo> {
    let versao = indice.versao.iter().find(|v| v.hash == hash)?;
    Some(match versao.estado.as_str() {
        "aprovada" => {
            let registrada = indice
                .decisao
                .iter()
                .rev()
                .find(|d| d.hash == hash)
                .is_some_and(|d| d.para == "aprovada");
            if registrada {
                EstadoConteudo::Aprovada
            } else {
                EstadoConteudo::Hipotese
            }
        }
        "em_revisao" => EstadoConteudo::EmRevisao,
        "rejeitada" => EstadoConteudo::Rejeitada,
        _ => EstadoConteudo::Hipotese,
    })
}

/// Decisor do importador por hash: o estado do Exercício é o da Versão cujo hash é o do conteúdo
/// de `partituras/<id>.musicxml`, nunca o do `estado.toml`. Conteúdo sem Versão registrada fica
/// sem decisão (entra como Hipótese, com aviso). Mudar o arquivo muda o hash: a aprovação não
/// acompanha.
#[derive(Debug, Clone, Copy, Default)]
pub struct DecisorPorHash;

impl DecideEstado for DecisorPorHash {
    fn estado_do_exercicio(
        &self,
        oficina: &Path,
        id: &str,
        conteudo: Option<&[u8]>,
    ) -> Result<Option<EstadoConteudo>, ErroPacote> {
        let Some(bytes) = conteudo else {
            return Ok(None);
        };
        let pasta = pasta_das_versoes(&oficina.join("partituras"), id);
        estado_do_conteudo_no_indice(&pasta.join(ARQUIVO_DO_INDICE), bytes)
    }

    fn aviso_sem_decisao(&self) -> &'static str {
        "conteúdo sem Versão registrada (entram como hipótese)"
    }
}
