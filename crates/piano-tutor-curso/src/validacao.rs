//! Validação estrutural do manifesto, do Roteiro e dos Alinhamentos.

use std::collections::{BTreeSet, HashSet};

use crate::chaves::{ChavePublica, e_sha256_hex};
use crate::erro::ErroPacote;
use crate::modelo::{Alinhamento, Exercicio, Manifesto, Momento, Roteiro, TipoLicao};
use crate::{
    ARQUIVO_ASSINATURA, ARQUIVO_MANIFESTO, FORMATO_VERSAO, MAX_ARQUIVO_BYTES, MAX_ARQUIVOS,
    PASTAS_RESERVADAS,
};

const MAX_ID: usize = 100;
const MAX_CAMINHO: usize = 240;

/// Confere a versão do formato de um documento (`1..=FORMATO_VERSAO`).
pub(crate) fn conferir_formato_versao(versao: u32, documento: &str) -> Result<(), ErroPacote> {
    if versao == 0 {
        return Err(ErroPacote::ManifestoInvalido(format!(
            "{documento}: formato_versao deve ser 1 ou maior"
        )));
    }
    if versao > FORMATO_VERSAO {
        return Err(ErroPacote::FormatoMaisNovo {
            encontrado: versao,
            suportado: FORMATO_VERSAO,
        });
    }
    Ok(())
}

/// Caminho relativo seguro: com `/`, sem `..`, `.`, componentes vazios, barra invertida,
/// dois-pontos ou caracteres de controle; nunca absoluto.
pub fn caminho_seguro(caminho: &str) -> bool {
    if caminho.is_empty() || caminho.len() > MAX_CAMINHO {
        return false;
    }
    if caminho
        .chars()
        .any(|c| c == '\\' || c == ':' || c.is_control())
    {
        return false;
    }
    caminho
        .split('/')
        .all(|parte| !parte.is_empty() && parte != "." && parte != "..")
}

pub(crate) fn exigir_caminho_seguro(caminho: &str) -> Result<(), ErroPacote> {
    if caminho_seguro(caminho) {
        Ok(())
    } else {
        Err(ErroPacote::CaminhoInseguro(caminho.to_string()))
    }
}

/// Identificador: `[a-z0-9.-]`, começando por letra ou número.
pub fn id_valido(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID
        && id
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
}

fn exigir_id(tipo: &'static str, id: &str) -> Result<(), ErroPacote> {
    if id_valido(id) {
        Ok(())
    } else {
        Err(ErroPacote::IdInvalido {
            tipo,
            id: id.to_string(),
        })
    }
}

fn exigir_unico(
    tipo: &'static str,
    vistos: &mut HashSet<String>,
    id: &str,
) -> Result<(), ErroPacote> {
    if vistos.insert(id.to_string()) {
        Ok(())
    } else {
        Err(ErroPacote::IdDuplicado {
            tipo,
            id: id.to_string(),
        })
    }
}

fn exigir_texto(valor: &str, campo: &str) -> Result<(), ErroPacote> {
    if valor.trim().is_empty() {
        Err(ErroPacote::Estrutura(format!(
            "{campo} não pode ficar vazio"
        )))
    } else {
        Ok(())
    }
}

fn exigir_finito(valor: f64, campo: &str) -> Result<(), ErroPacote> {
    if valor.is_finite() {
        Ok(())
    } else {
        Err(ErroPacote::Estrutura(format!(
            "{campo} precisa ser um número finito"
        )))
    }
}

/// Validação completa do manifesto (sem olhar o conteúdo dos arquivos).
pub fn validar_manifesto(m: &Manifesto) -> Result<(), ErroPacote> {
    conferir_formato_versao(m.formato_versao, ARQUIVO_MANIFESTO)?;

    exigir_id("curso", &m.id)?;
    if PASTAS_RESERVADAS.contains(&m.id.as_str()) {
        return Err(ErroPacote::IdInvalido {
            tipo: "curso",
            id: m.id.clone(),
        });
    }
    exigir_texto(&m.titulo, "titulo")?;
    exigir_texto(&m.versao, "versao")?;
    exigir_texto(&m.autor.nome, "autor.nome")?;
    exigir_texto(&m.licenciado.nome, "licenciado.nome")?;
    ChavePublica::de_hex(&m.assinatura.chave_publica)?;

    // Arquivos listados.
    if m.arquivos.len() > MAX_ARQUIVOS {
        return Err(ErroPacote::LimiteExcedido(format!(
            "o manifesto lista {} arquivos (máximo {MAX_ARQUIVOS})",
            m.arquivos.len()
        )));
    }
    let mut listados = BTreeSet::new();
    for a in &m.arquivos {
        exigir_caminho_seguro(&a.caminho)?;
        if a.caminho == ARQUIVO_MANIFESTO || a.caminho == ARQUIVO_ASSINATURA {
            return Err(ErroPacote::Estrutura(format!(
                "{:?} não pode constar em [[arquivos]]",
                a.caminho
            )));
        }
        if !e_sha256_hex(&a.sha256) {
            return Err(ErroPacote::Estrutura(format!(
                "sha256 de {:?} deve ter 64 dígitos hexadecimais minúsculos",
                a.caminho
            )));
        }
        if a.bytes > MAX_ARQUIVO_BYTES {
            return Err(ErroPacote::LimiteExcedido(format!(
                "{:?} tem {} bytes (máximo {MAX_ARQUIVO_BYTES})",
                a.caminho, a.bytes
            )));
        }
        if !listados.insert(a.caminho.as_str()) {
            return Err(ErroPacote::Estrutura(format!(
                "arquivo repetido em [[arquivos]]: {:?}",
                a.caminho
            )));
        }
    }

    // Seções, Lições e Exercícios.
    let mut secoes = HashSet::new();
    let mut licoes = HashSet::new();
    let mut exercicios = HashSet::new();
    for secao in &m.secoes {
        exigir_id("seção", &secao.id)?;
        exigir_unico("seção", &mut secoes, &secao.id)?;
        exigir_texto(&secao.titulo, "titulo da seção")?;
        for licao in &secao.licoes {
            exigir_id("lição", &licao.id)?;
            exigir_unico("lição", &mut licoes, &licao.id)?;
            exigir_texto(&licao.titulo, "titulo da lição")?;
            if licao.tipo == TipoLicao::Assistir && !licao.exercicios.is_empty() {
                return Err(ErroPacote::Estrutura(format!(
                    "a lição {:?} é do tipo assistir e não pode ter exercícios",
                    licao.id
                )));
            }
            if let Some(aula) = &licao.aula {
                if aula.nome_arquivo.is_empty()
                    || aula.nome_arquivo.contains(['/', '\\'])
                    || aula.nome_arquivo.chars().any(char::is_control)
                {
                    return Err(ErroPacote::Estrutura(format!(
                        "aula.nome_arquivo da lição {:?} deve ser só o nome do arquivo, sem pastas",
                        licao.id
                    )));
                }
                if let Some(d) = aula.duracao_s {
                    exigir_finito(d, "aula.duracao_s")?;
                }
                if aula.sha256.as_deref().is_some_and(|s| !e_sha256_hex(s)) {
                    return Err(ErroPacote::Estrutura(format!(
                        "aula.sha256 da lição {:?} deve ter 64 dígitos hexadecimais minúsculos",
                        licao.id
                    )));
                }
            }
            for ex in &licao.exercicios {
                exigir_id("exercício", &ex.id)?;
                exigir_unico("exercício", &mut exercicios, &ex.id)?;
                exigir_texto(&ex.titulo, "titulo do exercício")?;
                validar_exercicio(ex, &listados)?;
            }
        }
    }
    Ok(())
}

fn validar_exercicio(ex: &Exercicio, listados: &BTreeSet<&str>) -> Result<(), ErroPacote> {
    let referencias = [
        Some(("partitura", ex.partitura.as_str())),
        ex.audio.as_deref().map(|c| ("audio", c)),
        ex.alinhamento.as_deref().map(|c| ("alinhamento", c)),
    ];
    for (campo, caminho) in referencias.into_iter().flatten() {
        exigir_caminho_seguro(caminho)?;
        if !listados.contains(caminho) {
            return Err(ErroPacote::ArquivoReferenciadoNaoListado {
                caminho: caminho.to_string(),
                referenciado_por: format!("{campo} do exercício {:?}", ex.id),
            });
        }
    }
    if ex.bpm.is_some_and(|b| b == 0 || b > 1000) {
        return Err(ErroPacote::Estrutura(format!(
            "bpm do exercício {:?} deve ficar entre 1 e 1000",
            ex.id
        )));
    }
    if let Some(t) = ex.trecho_aula {
        exigir_finito(t.inicio_s, "trecho_aula.inicio_s")?;
        exigir_finito(t.fim_s, "trecho_aula.fim_s")?;
        if t.inicio_s < 0.0 || t.fim_s <= t.inicio_s {
            return Err(ErroPacote::Estrutura(format!(
                "trecho_aula do exercício {:?} precisa ter 0 <= inicio_s < fim_s",
                ex.id
            )));
        }
    }
    Ok(())
}

/// Valida o Roteiro contra o manifesto (Lições e Exercícios citados precisam existir).
pub fn validar_roteiro(r: &Roteiro, m: &Manifesto) -> Result<(), ErroPacote> {
    conferir_formato_versao(r.formato_versao, "roteiro.toml")?;
    let mut ids = HashSet::new();
    for f in &r.falas {
        exigir_id("fala", &f.id)?;
        exigir_unico("fala", &mut ids, &f.id)?;
        exigir_texto(&f.texto, &format!("texto da fala {:?}", f.id))?;
        let licao = m
            .secoes
            .iter()
            .flat_map(|s| &s.licoes)
            .find(|l| l.id == f.licao)
            .ok_or_else(|| {
                ErroPacote::Estrutura(format!(
                    "a fala {:?} cita a lição inexistente {:?}",
                    f.id, f.licao
                ))
            })?;
        if let Some(ex) = &f.exercicio
            && !licao.exercicios.iter().any(|e| &e.id == ex)
        {
            return Err(ErroPacote::Estrutura(format!(
                "a fala {:?} cita o exercício {:?}, que não pertence à lição {:?}",
                f.id, ex, f.licao
            )));
        }
        match (f.momento, f.passo) {
            (Momento::Passo, None) => {
                return Err(ErroPacote::Estrutura(format!(
                    "a fala {:?} tem momento = \"passo\" e precisa de passo",
                    f.id
                )));
            }
            (Momento::Passo, Some(_)) | (_, None) => {}
            (_, Some(_)) => {
                return Err(ErroPacote::Estrutura(format!(
                    "a fala {:?} só pode ter passo com momento = \"passo\"",
                    f.id
                )));
            }
        }
        if let Some(o) = f.origem {
            exigir_finito(o.inicio_s, "origem.inicio_s")?;
            exigir_finito(o.fim_s, "origem.fim_s")?;
            if o.inicio_s < 0.0 || o.fim_s < o.inicio_s {
                return Err(ErroPacote::Estrutura(format!(
                    "origem da fala {:?} tem tempos incoerentes",
                    f.id
                )));
            }
        }
    }
    Ok(())
}

/// Valida um Alinhamento contra o Exercício a que pertence.
pub fn validar_alinhamento(a: &Alinhamento, ex: &Exercicio) -> Result<(), ErroPacote> {
    conferir_formato_versao(a.formato_versao, "alinhamento")?;
    if a.exercicio != ex.id {
        return Err(ErroPacote::Estrutura(format!(
            "o alinhamento de {:?} declara o exercício {:?}",
            ex.id, a.exercicio
        )));
    }
    if let Some(audio) = &a.audio
        && ex.audio.as_ref() != Some(audio)
    {
        return Err(ErroPacote::Estrutura(format!(
            "o alinhamento de {:?} cita o áudio {:?}, diferente do exercício",
            ex.id, audio
        )));
    }
    exigir_finito(a.offset_aula_s, "offset_aula_s")?;
    let mut anterior: Option<u32> = None;
    for n in &a.notas {
        if anterior.is_some_and(|p| n.indice <= p) {
            return Err(ErroPacote::Estrutura(format!(
                "o alinhamento de {:?} tem índices de nota fora de ordem",
                ex.id
            )));
        }
        anterior = Some(n.indice);
        if n.midi > 127 {
            return Err(ErroPacote::Estrutura(format!(
                "nota MIDI {} fora de 0..=127",
                n.midi
            )));
        }
        exigir_finito(n.t_partitura_s, "t_partitura_s")?;
        exigir_finito(n.t_audio_s, "t_audio_s")?;
        exigir_finito(n.confianca, "confianca")?;
        if n.t_partitura_s < 0.0 || n.t_audio_s < 0.0 || !(0.0..=1.0).contains(&n.confianca) {
            return Err(ErroPacote::Estrutura(format!(
                "a nota {} do alinhamento de {:?} tem tempo negativo ou confiança fora de 0..=1",
                n.indice, ex.id
            )));
        }
    }
    Ok(())
}
