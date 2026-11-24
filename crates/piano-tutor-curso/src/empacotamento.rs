//! Empacotador: pasta de origem + chave privada → bytes de um `.ptcurso`.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::chaves::ChavePrivada;
use crate::erro::ErroPacote;
use crate::leitura::ler_documentos;
use crate::modelo::{Arquivo, Manifesto};
use crate::validacao::{conferir_formato_versao, validar_manifesto};
use crate::{
    ARQUIVO_ASSINATURA, ARQUIVO_MANIFESTO, MAX_ARQUIVO_BYTES, MAX_ARQUIVOS, MAX_DOCUMENTO_BYTES,
    MAX_PACOTE_BYTES, sha256_hex,
};

/// Empacota `pasta_origem` assinando com `chave`.
///
/// Lê `manifesto.toml` da pasta, recalcula `[[arquivos]]` (SHA-256 e tamanho) com tudo que
/// houver na pasta, exceto o próprio manifesto, `assinatura.ed25519` e nomes que começam por
/// ponto, grava a chave pública da `chave` em `[assinatura]`, valida a estrutura, assina os bytes
/// exatos do manifesto regravado e monta o zip. A saída é determinística para a mesma origem e chave.
pub fn empacotar(pasta_origem: &Path, chave: &ChavePrivada) -> Result<Vec<u8>, ErroPacote> {
    let caminho_manifesto = pasta_origem.join(ARQUIVO_MANIFESTO);
    let io = |e: std::io::Error| ErroPacote::Io(format!("{}: {e}", caminho_manifesto.display()));
    let tamanho = fs::metadata(&caminho_manifesto).map_err(io)?.len();
    if tamanho > MAX_DOCUMENTO_BYTES {
        return Err(ErroPacote::LimiteExcedido(
            "manifesto.toml grande demais".into(),
        ));
    }
    let texto = fs::read_to_string(&caminho_manifesto).map_err(io)?;
    let mut manifesto = Manifesto::de_toml(&texto)?;
    conferir_formato_versao(manifesto.formato_versao, ARQUIVO_MANIFESTO)?;

    let mut arquivos = BTreeMap::new();
    coletar(pasta_origem, "", &mut arquivos)?;

    manifesto.assinatura.chave_publica = chave.publica().para_hex();
    manifesto.arquivos = arquivos
        .iter()
        .map(|(caminho, dados)| Arquivo {
            caminho: caminho.clone(),
            sha256: sha256_hex(dados),
            bytes: dados.len() as u64,
        })
        .collect();
    validar_manifesto(&manifesto)?;
    ler_documentos(&manifesto, &arquivos)?;

    let manifesto_bytes = manifesto.para_toml()?.into_bytes();
    let assinatura = chave.assinar(&manifesto_bytes);

    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let mut gravar = |nome: &str, dados: &[u8]| -> Result<(), ErroPacote> {
        let metodo = if nome.ends_with(".opus") {
            CompressionMethod::Stored
        } else {
            CompressionMethod::Deflated
        };
        zip.start_file(
            nome,
            SimpleFileOptions::default().compression_method(metodo),
        )
        .map_err(|e| ErroPacote::ZipInvalido(e.to_string()))?;
        zip.write_all(dados)?;
        Ok(())
    };
    gravar(ARQUIVO_MANIFESTO, &manifesto_bytes)?;
    gravar(ARQUIVO_ASSINATURA, &assinatura)?;
    for (caminho, dados) in &arquivos {
        gravar(caminho, dados)?;
    }
    let saida = zip
        .finish()
        .map_err(|e| ErroPacote::ZipInvalido(e.to_string()))?
        .into_inner();
    if saida.len() as u64 > MAX_PACOTE_BYTES {
        return Err(ErroPacote::LimiteExcedido(format!(
            "o pacote ficou com {} bytes (máximo {MAX_PACOTE_BYTES})",
            saida.len()
        )));
    }
    Ok(saida)
}

/// Lê recursivamente os arquivos da origem (sem seguir links; links simbólicos são erro).
fn coletar(
    pasta: &Path,
    prefixo: &str,
    saida: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), ErroPacote> {
    let io = |e: std::io::Error| ErroPacote::Io(format!("{}: {e}", pasta.display()));
    for entrada in fs::read_dir(pasta).map_err(io)? {
        let entrada = entrada.map_err(io)?;
        let nome = entrada
            .file_name()
            .into_string()
            .map_err(|n| ErroPacote::CaminhoInseguro(n.to_string_lossy().into_owned()))?;
        if nome.starts_with('.') {
            continue;
        }
        let relativo = if prefixo.is_empty() {
            nome.clone()
        } else {
            format!("{prefixo}/{nome}")
        };
        let tipo = entrada.file_type().map_err(io)?;
        if tipo.is_symlink() {
            return Err(ErroPacote::Estrutura(format!(
                "link simbólico na origem: {relativo:?}"
            )));
        } else if tipo.is_dir() {
            coletar(&entrada.path(), &relativo, saida)?;
        } else if tipo.is_file() {
            if relativo == ARQUIVO_MANIFESTO || relativo == ARQUIVO_ASSINATURA {
                continue;
            }
            let caminho = entrada.path();
            let tamanho = fs::metadata(&caminho).map_err(io)?.len();
            if tamanho > MAX_ARQUIVO_BYTES {
                return Err(ErroPacote::LimiteExcedido(format!(
                    "{relativo:?} tem {tamanho} bytes (máximo {MAX_ARQUIVO_BYTES} por arquivo)"
                )));
            }
            if saida.len() >= MAX_ARQUIVOS {
                return Err(ErroPacote::LimiteExcedido(format!(
                    "mais de {MAX_ARQUIVOS} arquivos na origem"
                )));
            }
            saida.insert(relativo, fs::read(&caminho).map_err(io)?);
        }
    }
    Ok(())
}
