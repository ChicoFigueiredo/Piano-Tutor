//! Leitura e verificação de um `.ptcurso`.

use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::Path;

use serde::Deserialize;
use zip::ZipArchive;

use crate::chaves::ChavePublica;
use crate::erro::ErroPacote;
use crate::modelo::{Alinhamento, Contagem, ExercicioNoCurso, Manifesto, Roteiro};
use crate::validacao::{
    conferir_formato_versao, exigir_caminho_seguro, validar_alinhamento, validar_manifesto,
    validar_roteiro,
};
use crate::{
    ARQUIVO_ASSINATURA, ARQUIVO_MANIFESTO, ARQUIVO_ROTEIRO, MAX_ARQUIVO_BYTES, MAX_ARQUIVOS,
    MAX_DOCUMENTO_BYTES, MAX_PACOTE_BYTES,
};

/// Quanto o app confia na chave que assinou o pacote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confianca {
    /// A chave está na lista de chaves confiáveis.
    Confiavel,
    /// Assinatura válida, mas de uma chave que o app não conhece.
    Desconhecida,
}

/// Pacote com assinatura, hashes e estrutura conferidos. Só se constrói por [`ler_pacote`].
#[derive(Debug, Clone)]
pub struct PacoteVerificado {
    pub manifesto: Manifesto,
    pub chave_publica: ChavePublica,
    pub roteiro: Option<Roteiro>,
    pub(crate) manifesto_bytes: Vec<u8>,
    pub(crate) assinatura: [u8; 64],
    pub(crate) arquivos: BTreeMap<String, Vec<u8>>,
    pub(crate) alinhamentos: BTreeMap<String, Alinhamento>,
}

impl PacoteVerificado {
    /// Bytes de um arquivo listado em `[[arquivos]]`.
    pub fn arquivo(&self, caminho: &str) -> Option<&[u8]> {
        self.arquivos.get(caminho).map(Vec::as_slice)
    }

    /// Alinhamento do Exercício `id_exercicio`, se houver.
    pub fn alinhamento(&self, id_exercicio: &str) -> Option<&Alinhamento> {
        self.alinhamentos.get(id_exercicio)
    }

    /// Confiável só se a chave que assinou estiver em `confiaveis`.
    pub fn confianca(&self, confiaveis: &[ChavePublica]) -> Confianca {
        if confiaveis.contains(&self.chave_publica) {
            Confianca::Confiavel
        } else {
            Confianca::Desconhecida
        }
    }

    /// Exercícios que podem ser mostrados ao aluno (só `aprovada`).
    pub fn exercicios_para_aluno(&self) -> impl Iterator<Item = ExercicioNoCurso<'_>> {
        self.manifesto.exercicios_para_aluno()
    }

    pub fn contagem(&self) -> Contagem {
        self.manifesto.contagem()
    }
}

/// Lê um `.ptcurso` do disco. Recusa arquivos acima do limite antes de ler.
pub fn ler_pacote_de_arquivo(caminho: &Path) -> Result<PacoteVerificado, ErroPacote> {
    let io = |e: std::io::Error| ErroPacote::Io(format!("{}: {e}", caminho.display()));
    let tamanho = std::fs::metadata(caminho).map_err(io)?.len();
    if tamanho > MAX_PACOTE_BYTES {
        return Err(ErroPacote::LimiteExcedido(format!(
            "o pacote tem {tamanho} bytes (máximo {MAX_PACOTE_BYTES})"
        )));
    }
    ler_pacote(&std::fs::read(caminho).map_err(io)?)
}

#[derive(Deserialize)]
struct Previa {
    formato_versao: u32,
    #[serde(default)]
    assinatura: PreviaAssinatura,
}

#[derive(Deserialize, Default)]
struct PreviaAssinatura {
    #[serde(default)]
    chave_publica: String,
}

/// Verifica um pacote: zip seguro, versão do formato, assinatura ed25519 sobre os bytes exatos
/// do manifesto, estrutura, arquivos listados com SHA-256 e tamanho, sem arquivo extra.
pub fn ler_pacote(bytes: &[u8]) -> Result<PacoteVerificado, ErroPacote> {
    if bytes.len() as u64 > MAX_PACOTE_BYTES {
        return Err(ErroPacote::LimiteExcedido(format!(
            "o pacote tem {} bytes (máximo {MAX_PACOTE_BYTES})",
            bytes.len()
        )));
    }
    let mut entradas = extrair_entradas(bytes)?;

    let manifesto_bytes = entradas
        .remove(ARQUIVO_MANIFESTO)
        .ok_or(ErroPacote::ManifestoAusente)?;
    let assinatura_bytes = entradas
        .remove(ARQUIVO_ASSINATURA)
        .ok_or(ErroPacote::AssinaturaAusente)?;
    if manifesto_bytes.len() as u64 > MAX_DOCUMENTO_BYTES {
        return Err(ErroPacote::LimiteExcedido(
            "manifesto.toml grande demais".into(),
        ));
    }
    let texto = std::str::from_utf8(&manifesto_bytes)
        .map_err(|_| ErroPacote::ManifestoInvalido("manifesto.toml não é UTF-8".into()))?;

    // 1. versão do formato (um pacote futuro pode ter campos que não conhecemos);
    let previa: Previa = toml::from_str(texto)
        .map_err(|e| ErroPacote::ManifestoInvalido(format!("manifesto.toml: {e}")))?;
    conferir_formato_versao(previa.formato_versao, ARQUIVO_MANIFESTO)?;

    // 2. autenticidade, antes de interpretar o resto;
    let chave_publica = ChavePublica::de_hex(&previa.assinatura.chave_publica)?;
    let assinatura: [u8; 64] = assinatura_bytes.as_slice().try_into().map_err(|_| {
        ErroPacote::AssinaturaMalformada(format!(
            "esperava 64 bytes, veio {}",
            assinatura_bytes.len()
        ))
    })?;
    if !chave_publica.verificar(&manifesto_bytes, &assinatura) {
        return Err(ErroPacote::AssinaturaInvalida);
    }

    // 3. estrutura do manifesto;
    let manifesto = Manifesto::de_toml(texto)?;
    validar_manifesto(&manifesto)?;

    // 4. arquivos: nem a mais, nem a menos, hash e tamanho conferem.
    for caminho in entradas.keys() {
        if !manifesto.arquivos.iter().any(|a| &a.caminho == caminho) {
            return Err(ErroPacote::ArquivoNaoListado {
                caminho: caminho.clone(),
            });
        }
    }
    for a in &manifesto.arquivos {
        let conteudo = entradas
            .get(&a.caminho)
            .ok_or_else(|| ErroPacote::ArquivoAusente {
                caminho: a.caminho.clone(),
            })?;
        if conteudo.len() as u64 != a.bytes {
            return Err(ErroPacote::TamanhoDivergente {
                caminho: a.caminho.clone(),
            });
        }
        if crate::sha256_hex(conteudo) != a.sha256 {
            return Err(ErroPacote::HashDivergente {
                caminho: a.caminho.clone(),
            });
        }
    }

    // 5. Roteiro e Alinhamentos.
    let (roteiro, alinhamentos) = ler_documentos(&manifesto, &entradas)?;

    Ok(PacoteVerificado {
        manifesto,
        chave_publica,
        roteiro,
        manifesto_bytes,
        assinatura,
        arquivos: entradas,
        alinhamentos,
    })
}

/// Lê e valida `roteiro.toml` e os `alinhamentos/*.json` citados pelos Exercícios.
pub(crate) fn ler_documentos(
    manifesto: &Manifesto,
    arquivos: &BTreeMap<String, Vec<u8>>,
) -> Result<(Option<Roteiro>, BTreeMap<String, Alinhamento>), ErroPacote> {
    let roteiro = match arquivos.get(ARQUIVO_ROTEIRO) {
        Some(bytes) => {
            exigir_tamanho_documento(ARQUIVO_ROTEIRO, bytes)?;
            let texto = std::str::from_utf8(bytes)
                .map_err(|_| ErroPacote::ManifestoInvalido("roteiro.toml não é UTF-8".into()))?;
            let r = Roteiro::de_toml(texto)?;
            validar_roteiro(&r, manifesto)?;
            Some(r)
        }
        None => None,
    };
    let mut alinhamentos = BTreeMap::new();
    for e in manifesto.exercicios() {
        let Some(caminho) = &e.exercicio.alinhamento else {
            continue;
        };
        let bytes = arquivos
            .get(caminho)
            .ok_or_else(|| ErroPacote::ArquivoAusente {
                caminho: caminho.clone(),
            })?;
        exigir_tamanho_documento(caminho, bytes)?;
        let a = Alinhamento::de_json(bytes)?;
        validar_alinhamento(&a, e.exercicio)?;
        alinhamentos.insert(e.exercicio.id.clone(), a);
    }
    Ok((roteiro, alinhamentos))
}

fn exigir_tamanho_documento(nome: &str, bytes: &[u8]) -> Result<(), ErroPacote> {
    if bytes.len() as u64 > MAX_DOCUMENTO_BYTES {
        Err(ErroPacote::LimiteExcedido(format!(
            "{nome} grande demais (máximo {MAX_DOCUMENTO_BYTES} bytes)"
        )))
    } else {
        Ok(())
    }
}

/// Lê todas as entradas do zip para a memória, com todos os limites e checagens de caminho.
fn extrair_entradas(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, ErroPacote> {
    let mut zip =
        ZipArchive::new(Cursor::new(bytes)).map_err(|e| ErroPacote::ZipInvalido(e.to_string()))?;
    // manifesto + assinatura + arquivos listados
    if zip.len() > MAX_ARQUIVOS + 2 {
        return Err(ErroPacote::LimiteExcedido(format!(
            "o pacote tem {} entradas (máximo {MAX_ARQUIVOS} arquivos)",
            zip.len()
        )));
    }
    let mut saida = BTreeMap::new();
    let mut total: u64 = 0;
    for i in 0..zip.len() {
        let mut entrada = zip
            .by_index(i)
            .map_err(|e| ErroPacote::ZipInvalido(e.to_string()))?;
        let nome = entrada
            .name()
            .map_err(|_| ErroPacote::ZipInvalido("entrada com nome que não é UTF-8".into()))?
            .into_owned();
        let e_pasta = entrada.is_dir();
        exigir_caminho_seguro(nome.strip_suffix('/').filter(|_| e_pasta).unwrap_or(&nome))?;
        if e_pasta {
            continue;
        }
        if entrada
            .unix_mode()
            .is_some_and(|m| m & 0o170_000 == 0o120_000)
        {
            return Err(ErroPacote::ZipInvalido(format!(
                "{nome:?} é um link simbólico"
            )));
        }
        if entrada.size() > MAX_ARQUIVO_BYTES {
            return Err(ErroPacote::LimiteExcedido(format!(
                "{nome:?} declara {} bytes (máximo {MAX_ARQUIVO_BYTES} por arquivo)",
                entrada.size()
            )));
        }
        let mut dados = Vec::new();
        (&mut entrada)
            .take(MAX_ARQUIVO_BYTES + 1)
            .read_to_end(&mut dados)
            .map_err(|e| ErroPacote::ZipInvalido(format!("{nome:?}: {e}")))?;
        if dados.len() as u64 > MAX_ARQUIVO_BYTES {
            return Err(ErroPacote::LimiteExcedido(format!(
                "{nome:?} passa de {MAX_ARQUIVO_BYTES} bytes"
            )));
        }
        total += dados.len() as u64;
        if total > MAX_PACOTE_BYTES {
            return Err(ErroPacote::LimiteExcedido(format!(
                "o conteúdo do pacote passa de {MAX_PACOTE_BYTES} bytes"
            )));
        }
        if saida.insert(nome.clone(), dados).is_some() {
            return Err(ErroPacote::ZipInvalido(format!(
                "entrada repetida no zip: {nome:?}"
            )));
        }
    }
    Ok(saida)
}
