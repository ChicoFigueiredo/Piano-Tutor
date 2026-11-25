//! Importador: plano de importação + oficina → pasta-origem pronta para `empacotar`.
//!
//! O **plano** (TOML escrito à mão) descreve a estrutura do Curso: metadados, Seções, Lições e
//! Exercícios. A **oficina** (fora do git) traz as partituras, os estados de revisão, o áudio,
//! os Alinhamentos e o Roteiro. O importador nunca promove conteúdo: o estado de cada Exercício
//! é o de `partituras/<id>.estado.toml` (ausente = Hipótese).
//!
//! Formato do plano: `docs/specs/FORMATO-PACOTE-DE-CURSO.md`, seção "Plano de importação".

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::FORMATO_VERSAO;
use crate::erro::ErroPacote;
use crate::modelo::{
    AssinaturaInfo, Aula, Autor, Contagem, EstadoConteudo, Exercicio, Licao, Licenciado, Manifesto,
    Mao, Modo, Roteiro, Secao, TipoLicao, TrechoAula,
};
use crate::validacao::{conferir_formato_versao, id_valido, validar_roteiro};

#[derive(Debug, Deserialize)]
struct Plano {
    formato_versao: u32,
    id: String,
    titulo: String,
    versao: String,
    idioma: String,
    convencao_oitava: String,
    autor: Autor,
    licenciado: Licenciado,
    #[serde(default)]
    secoes: Vec<PlanoSecao>,
}

#[derive(Debug, Deserialize)]
struct PlanoSecao {
    id: String,
    numero: u32,
    titulo: String,
    #[serde(default)]
    licoes: Vec<PlanoLicao>,
}

#[derive(Debug, Deserialize)]
struct PlanoLicao {
    id: String,
    numero: u32,
    titulo: String,
    tipo: TipoLicao,
    #[serde(default)]
    aula: Option<Aula>,
    #[serde(default)]
    exercicios: Vec<PlanoExercicio>,
}

#[derive(Debug, Deserialize)]
struct PlanoExercicio {
    id: String,
    titulo: String,
    modo: Modo,
    mao: Mao,
    #[serde(default)]
    bpm: Option<u32>,
}

/// `partituras/<id>.estado.toml`: o resto dos campos (proveniência, confiança...) é ignorado.
#[derive(Debug, Deserialize)]
struct EstadoOficina {
    #[serde(default)]
    estado: Option<EstadoConteudo>,
    #[serde(default)]
    trecho_aula: Option<TrechoAula>,
    #[serde(default)]
    bpm_sugerido: Option<u32>,
}

/// Arquivo da oficina a copiar para a pasta-origem.
struct Copia {
    relativo: String,
    origem: PathBuf,
}

/// Resultado da importação: o que foi gravado e o que ficou de fora.
#[derive(Debug, Clone, PartialEq)]
pub struct Resumo {
    pub id: String,
    pub titulo: String,
    pub destino: PathBuf,
    /// Lições e Exercícios por estado (só os que entraram na pasta-origem).
    pub contagem: Contagem,
    pub partituras: usize,
    pub audios: usize,
    pub alinhamentos: usize,
    /// Há `roteiro.toml` na pasta-origem.
    pub roteiro: bool,
    pub falas: usize,
    pub falas_aprovadas: usize,
    /// Exercícios deixados de fora por `--somente-aprovadas`.
    pub omitidos_exercicios: usize,
    /// Falas deixadas de fora por `--somente-aprovadas`.
    pub omitidos_falas: usize,
    pub avisos: Vec<String>,
}

/// Lê `plano_path` e a `oficina`, e grava a pasta-origem em `destino` (que não pode ter conteúdo).
///
/// Tudo é lido e conferido antes de gravar: uma falha não deixa pasta-origem pela metade.
/// Com `somente_aprovadas`, entram só os Exercícios `aprovada` e as falas `aprovada` cujo
/// Exercício (se houver) também entrou.
pub fn importar(
    plano_path: &Path,
    oficina: &Path,
    destino: &Path,
    somente_aprovadas: bool,
) -> Result<Resumo, ErroPacote> {
    let texto = ler_texto(plano_path)?;
    let plano: Plano = toml::from_str(&texto)
        .map_err(|e| importacao(format!("plano {}: {e}", plano_path.display())))?;
    conferir_formato_versao(plano.formato_versao, "plano de importação")?;
    exigir_destino_vazio(destino)?;

    let montado = montar(&plano, oficina, somente_aprovadas)?;
    gravar(destino, &montado)?;

    let contagem = montado.manifesto.contagem();
    let mut audios = 0;
    let mut alinhamentos = 0;
    for e in montado.manifesto.exercicios() {
        audios += usize::from(e.exercicio.audio.is_some());
        alinhamentos += usize::from(e.exercicio.alinhamento.is_some());
    }
    Ok(Resumo {
        id: plano.id,
        titulo: plano.titulo,
        destino: destino.to_path_buf(),
        contagem,
        partituras: montado
            .copias
            .iter()
            .filter(|c| c.relativo.starts_with("partituras/"))
            .count(),
        audios,
        alinhamentos,
        roteiro: montado.roteiro.is_some(),
        falas: montado.falas,
        falas_aprovadas: montado.falas_aprovadas,
        omitidos_exercicios: montado.omitidos_exercicios,
        omitidos_falas: montado.omitidos_falas,
        avisos: montado.avisos,
    })
}

/// Tudo o que a gravação precisa, já conferido.
struct Montado {
    manifesto: Manifesto,
    copias: Vec<Copia>,
    /// Texto de `roteiro.toml` a gravar, se a oficina tiver Roteiro.
    roteiro: Option<String>,
    falas: usize,
    falas_aprovadas: usize,
    omitidos_exercicios: usize,
    omitidos_falas: usize,
    avisos: Vec<String>,
}

fn montar(plano: &Plano, oficina: &Path, somente: bool) -> Result<Montado, ErroPacote> {
    let mut copias = Vec::new();
    let mut avisos = Vec::new();
    let mut sem_estado = Vec::new();
    let mut sem_audio = Vec::new();
    let mut sem_alinhamento = Vec::new();
    let mut omitidos_exercicios = 0;

    let mut secoes = Vec::new();
    for secao in &plano.secoes {
        let mut licoes = Vec::new();
        for licao in &secao.licoes {
            let mut exercicios = Vec::new();
            for ex in &licao.exercicios {
                if !id_valido(&ex.id) {
                    return Err(ErroPacote::IdInvalido {
                        tipo: "exercício",
                        id: ex.id.clone(),
                    });
                }
                let estado = ler_estado(oficina, &ex.id)?;
                if estado.is_none() {
                    sem_estado.push(ex.id.clone());
                }
                let estado_conteudo = estado
                    .as_ref()
                    .and_then(|e| e.estado)
                    .unwrap_or(EstadoConteudo::Hipotese);
                if somente && estado_conteudo != EstadoConteudo::Aprovada {
                    omitidos_exercicios += 1;
                    continue;
                }

                let partitura = format!("partituras/{}.musicxml", ex.id);
                let origem = oficina.join(&partitura);
                if !origem.is_file() {
                    return Err(importacao(format!(
                        "o exercício {:?} não tem partitura na oficina: falta {}",
                        ex.id,
                        origem.display()
                    )));
                }
                copias.push(Copia {
                    relativo: partitura.clone(),
                    origem,
                });

                let audio =
                    copiar_se_existir(oficina, &format!("audio/{}.opus", ex.id), &mut copias);
                if audio.is_none() {
                    sem_audio.push(ex.id.clone());
                }
                let alinhamento = copiar_se_existir(
                    oficina,
                    &format!("alinhamentos/{}.json", ex.id),
                    &mut copias,
                );
                if alinhamento.is_none() {
                    sem_alinhamento.push(ex.id.clone());
                }

                exercicios.push(Exercicio {
                    id: ex.id.clone(),
                    titulo: ex.titulo.clone(),
                    modo: ex.modo,
                    mao: ex.mao,
                    estado: estado_conteudo,
                    partitura,
                    audio,
                    alinhamento,
                    bpm: ex.bpm.or(estado.as_ref().and_then(|e| e.bpm_sugerido)),
                    trecho_aula: estado.as_ref().and_then(|e| e.trecho_aula),
                });
            }
            if licao.tipo == TipoLicao::Exercicio && exercicios.is_empty() {
                avisos.push(format!(
                    "lição {} ficou sem exercícios para entrar no pacote",
                    licao.id
                ));
            }
            licoes.push(Licao {
                id: licao.id.clone(),
                numero: licao.numero,
                titulo: licao.titulo.clone(),
                tipo: licao.tipo,
                aula: licao.aula.clone(),
                exercicios,
            });
        }
        secoes.push(Secao {
            id: secao.id.clone(),
            numero: secao.numero,
            titulo: secao.titulo.clone(),
            licoes,
        });
    }

    let manifesto = Manifesto {
        formato_versao: FORMATO_VERSAO,
        id: plano.id.clone(),
        titulo: plano.titulo.clone(),
        versao: plano.versao.clone(),
        gerado_em: agora_utc(),
        idioma: plano.idioma.clone(),
        convencao_oitava: plano.convencao_oitava.clone(),
        autor: plano.autor.clone(),
        licenciado: plano.licenciado.clone(),
        assinatura: AssinaturaInfo::default(),
        secoes,
        arquivos: Vec::new(),
    };

    let (roteiro, falas, falas_aprovadas, omitidos_falas) =
        montar_roteiro(oficina, &manifesto, somente, &mut avisos)?;

    if !sem_estado.is_empty() {
        avisos.push(format!(
            "sem partituras/<id>.estado.toml (entram como hipótese): {}",
            sem_estado.join(", ")
        ));
    }
    if !sem_audio.is_empty() {
        avisos.push(format!(
            "sem audio/<id>.opus (o tutor não toca o trecho): {}",
            sem_audio.join(", ")
        ));
    }
    if !sem_alinhamento.is_empty() {
        avisos.push(format!(
            "sem alinhamentos/<id>.json: {}",
            sem_alinhamento.join(", ")
        ));
    }

    Ok(Montado {
        manifesto,
        copias,
        roteiro,
        falas,
        falas_aprovadas,
        omitidos_exercicios,
        omitidos_falas,
        avisos,
    })
}

/// Lê, filtra (se `somente`) e confere o `roteiro.toml` da oficina contra o manifesto montado.
/// Devolve o texto a gravar e as contagens de falas.
fn montar_roteiro(
    oficina: &Path,
    manifesto: &Manifesto,
    somente: bool,
    avisos: &mut Vec<String>,
) -> Result<(Option<String>, usize, usize, usize), ErroPacote> {
    let caminho = oficina.join("roteiro.toml");
    if !caminho.is_file() {
        avisos.push("sem roteiro.toml na oficina: o curso não terá falas".into());
        return Ok((None, 0, 0, 0));
    }
    let bruto = ler_texto(&caminho)?;
    let mut roteiro = Roteiro::de_toml(&bruto)?;
    let total = roteiro.falas.len();
    let mut omitidos = 0;
    if somente {
        let incluidos: Vec<&str> = manifesto
            .exercicios()
            .map(|e| e.exercicio.id.as_str())
            .collect();
        roteiro.falas.retain(|f| {
            f.estado == EstadoConteudo::Aprovada
                && f.exercicio
                    .as_deref()
                    .is_none_or(|e| incluidos.contains(&e))
        });
        omitidos = total - roteiro.falas.len();
    }
    validar_roteiro(&roteiro, manifesto)?;
    let texto = if somente { roteiro.para_toml()? } else { bruto };
    let aprovadas = roteiro.falas_aprovadas().count();
    Ok((Some(texto), roteiro.falas.len(), aprovadas, omitidos))
}

fn gravar(destino: &Path, montado: &Montado) -> Result<(), ErroPacote> {
    let escrever = |caminho: PathBuf, dados: &[u8]| -> Result<(), ErroPacote> {
        if let Some(pai) = caminho.parent() {
            fs::create_dir_all(pai).map_err(|e| io(pai, e))?;
        }
        fs::write(&caminho, dados).map_err(|e| io(&caminho, e))
    };
    fs::create_dir_all(destino).map_err(|e| io(destino, e))?;
    escrever(
        destino.join("manifesto.toml"),
        montado.manifesto.para_toml()?.as_bytes(),
    )?;
    for c in &montado.copias {
        let caminho = destino.join(&c.relativo);
        if let Some(pai) = caminho.parent() {
            fs::create_dir_all(pai).map_err(|e| io(pai, e))?;
        }
        fs::copy(&c.origem, &caminho).map_err(|e| io(&c.origem, e))?;
    }
    if let Some(texto) = &montado.roteiro {
        escrever(destino.join("roteiro.toml"), texto.as_bytes())?;
    }
    Ok(())
}

fn exigir_destino_vazio(destino: &Path) -> Result<(), ErroPacote> {
    if !destino.exists() {
        return Ok(());
    }
    let vazio = fs::read_dir(destino)
        .map_err(|e| io(destino, e))?
        .next()
        .is_none();
    if vazio {
        Ok(())
    } else {
        Err(importacao(format!(
            "a pasta-origem {} já existe e não está vazia; escolha outra ou esvazie-a",
            destino.display()
        )))
    }
}

/// Estado do Exercício em `partituras/<id>.estado.toml`; `None` se o arquivo não existe.
fn ler_estado(oficina: &Path, id: &str) -> Result<Option<EstadoOficina>, ErroPacote> {
    let caminho = oficina.join(format!("partituras/{id}.estado.toml"));
    if !caminho.is_file() {
        return Ok(None);
    }
    let texto = ler_texto(&caminho)?;
    toml::from_str(&texto)
        .map(Some)
        .map_err(|e| importacao(format!("{}: {e}", caminho.display())))
}

/// Arquivo opcional: se existir na oficina, entra em `copias` e devolve o caminho relativo.
fn copiar_se_existir(oficina: &Path, relativo: &str, copias: &mut Vec<Copia>) -> Option<String> {
    let origem = oficina.join(relativo);
    if !origem.is_file() {
        return None;
    }
    copias.push(Copia {
        relativo: relativo.to_string(),
        origem,
    });
    Some(relativo.to_string())
}

fn ler_texto(caminho: &Path) -> Result<String, ErroPacote> {
    fs::read_to_string(caminho).map_err(|e| io(caminho, e))
}

fn io(caminho: &Path, e: std::io::Error) -> ErroPacote {
    ErroPacote::Io(format!("{}: {e}", caminho.display()))
}

fn importacao(mensagem: String) -> ErroPacote {
    ErroPacote::Importacao(mensagem)
}

/// Agora em UTC, como `gerado_em` do manifesto (`AAAA-MM-DDTHH:MM:SSZ`).
fn agora_utc() -> String {
    let segundos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    iso_utc(i64::try_from(segundos).unwrap_or(i64::MAX))
}

/// Segundos desde 1970-01-01 em `AAAA-MM-DDTHH:MM:SSZ` (calendário gregoriano, proléptico).
fn iso_utc(segundos: i64) -> String {
    let dias = segundos.div_euclid(86_400) + 719_468;
    let resto = segundos.rem_euclid(86_400);
    let era = dias.div_euclid(146_097);
    let doe = dias - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let ano = yoe + era * 400 + i64::from(mes <= 2);
    format!(
        "{ano:04}-{mes:02}-{dia:02}T{:02}:{:02}:{:02}Z",
        resto / 3600,
        resto % 3600 / 60,
        resto % 60
    )
}

#[cfg(test)]
mod testes {
    use super::iso_utc;

    #[test]
    fn data_utc_a_partir_dos_segundos() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00Z", "ano bissexto");
    }
}
