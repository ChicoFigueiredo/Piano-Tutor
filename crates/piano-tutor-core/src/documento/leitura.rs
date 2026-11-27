//! Abertura: analisa o texto com roxmltree, decompõe cada compasso em peças (faixas do texto
//! original) e atribui `IdNota` a cada nota.

use super::erro::ErroDocumento;
use super::exportar::impressao_de;
use super::ids::{ChaveNota, TabelaIds};
use super::texto::{Abertura, No, Peca, Texto, fim_da_abertura};
use super::{Altura, Compasso, Documento, IdNota, Nota, Parte};
use crate::partitura::Fracao;
use roxmltree::{Document, Node, ParsingOptions};
use std::collections::{BTreeSet, HashMap};

/// Quais elementos são decompostos (e em que profundidade); o resto fica como faixa bruta.
fn decompor_aqui(nome: &str, prof: usize) -> bool {
    matches!(
        (nome, prof),
        ("measure", 0)
            | ("note", 1)
            | ("backup", 1)
            | ("forward", 1)
            | ("attributes", 1)
            | ("pitch", 2)
    )
}

fn nome<'a>(n: Node<'a, '_>) -> &'a str {
    n.tag_name().name()
}

fn filho_el<'a, 'b>(n: Node<'a, 'b>, nome_filho: &str) -> Option<Node<'a, 'b>> {
    n.children()
        .find(|c| c.is_element() && nome(*c) == nome_filho)
}

fn texto_de(n: Node<'_, '_>, nome_filho: &str) -> Option<String> {
    filho_el(n, nome_filho).map(|c| c.text().unwrap_or("").trim().to_owned())
}

fn decompor(node: Node<'_, '_>, base: &str, prof: usize) -> Result<No, ErroDocumento> {
    let r = node.range();
    let n = nome(node);
    if !decompor_aqui(n, prof) {
        return Ok(No::bruto(n, r));
    }
    let abertura = match fim_da_abertura(base, r.start) {
        Some(Abertura::Aberta(i)) if i <= r.end => i,
        Some(Abertura::AutoFechada) => return Ok(No::bruto(n, r)),
        _ => return Err(ErroDocumento::FaixaInconsistente(format!("<{n}>"))),
    };
    let fecha = base[r.clone()]
        .rfind("</")
        .map(|p| p + r.start)
        .filter(|&f| f >= abertura)
        .ok_or_else(|| ErroDocumento::FaixaInconsistente(format!("<{n}> sem fechamento")))?;
    let mut pecas = vec![Peca::Txt(Texto::Original(r.start..abertura))];
    let mut cursor = abertura;
    for c in node.children().filter(Node::is_element) {
        let cr = c.range();
        if cr.start < cursor || cr.end > fecha {
            return Err(ErroDocumento::FaixaInconsistente(format!(
                "<{}> dentro de <{n}> fora da faixa do pai (entidade que expande elementos?)",
                nome(c)
            )));
        }
        pecas.push(Peca::Txt(Texto::Original(cursor..cr.start)));
        pecas.push(Peca::Elem(decompor(c, base, prof + 1)?));
        cursor = cr.end;
    }
    pecas.push(Peca::Txt(Texto::Original(cursor..r.end)));
    Ok(No {
        nome: n.to_owned(),
        pecas,
    })
}

fn ler_altura(pitch: Node<'_, '_>) -> Option<Altura> {
    let passo = texto_de(pitch, "step")?.chars().next()?;
    if !('A'..='G').contains(&passo) {
        return None;
    }
    let alter = match texto_de(pitch, "alter") {
        None => 0,
        Some(t) => match t.parse::<i8>() {
            Ok(v) => v,
            Err(_) => {
                let f = t.parse::<f64>().ok()?;
                if f.fract() != 0.0 || f.abs() > 2.0 {
                    return None;
                }
                f as i8
            }
        },
    };
    let oitava = texto_de(pitch, "octave")?.parse::<i8>().ok()?;
    Some(Altura {
        passo,
        alter,
        oitava,
    })
}

fn ler_nota(n: Node<'_, '_>, id: IdNota, idx: usize, divisions: i64) -> Nota {
    let graca = filho_el(n, "grace").is_some();
    let duracao = if graca {
        None
    } else {
        texto_de(n, "duration")
            .and_then(|t| t.parse::<i64>().ok())
            .map(|d| Fracao::nova(d, divisions))
    };
    Nota {
        id,
        id_xml: n.attribute("id").map(str::to_owned),
        altura: filho_el(n, "pitch").and_then(ler_altura),
        duracao,
        voz: texto_de(n, "voice").unwrap_or_else(|| "1".into()),
        pauta: texto_de(n, "staff").unwrap_or_else(|| "1".into()),
        acorde: filho_el(n, "chord").is_some(),
        pausa: filho_el(n, "rest").is_some(),
        graca,
        divisions,
        idx,
        faixa: n.range(),
    }
}

impl Documento {
    /// Abre um texto MusicXML (partwise), atribuindo ids por ordem de documento.
    pub fn abrir(texto: &str) -> Result<Self, ErroDocumento> {
        Self::abrir_com_ids(texto, None)
    }

    /// Abre bytes: só UTF-8 (com ou sem BOM). Outras codificações são erro explícito, porque
    /// converter mudaria os bytes e a promessa é devolvê-los idênticos.
    pub fn abrir_bytes(bytes: &[u8]) -> Result<Self, ErroDocumento> {
        let texto = std::str::from_utf8(bytes).map_err(|e| {
            let dica = if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
                " (BOM de UTF-16)"
            } else {
                ""
            };
            ErroDocumento::NaoUtf8(format!("{e}{dica}"))
        })?;
        Self::abrir(texto)
    }

    /// Abre reaproveitando uma tabela de ids anterior (arquivo lateral). A tabela precisa ter
    /// sido gravada para este mesmo texto.
    pub fn abrir_com_ids(texto: &str, previa: Option<&TabelaIds>) -> Result<Self, ErroDocumento> {
        if let Some(t) = previa
            && t.base.as_ref().is_some_and(|b| *b != impressao_de(texto))
        {
            return Err(ErroDocumento::TabelaIdsDeOutroTexto);
        }
        let doc = Document::parse_with_options(
            texto,
            ParsingOptions {
                allow_dtd: true,
                nodes_limit: 4_000_000,
                ..ParsingOptions::default()
            },
        )
        .map_err(|e| ErroDocumento::Xml(e.to_string()))?;
        let raiz = doc.root_element();
        if nome(raiz) != "score-partwise" {
            return Err(ErroDocumento::NaoSuportado(format!(
                "raiz <{}>: só score-partwise",
                nome(raiz)
            )));
        }
        let mut tabela = TabelaIds {
            proximo: previa.map_or(1, |t| t.proximo),
            ..TabelaIds::default()
        };
        let mut usadas: BTreeSet<ChaveNota> = BTreeSet::new();
        let mut indice = HashMap::new();
        let mut partes = Vec::new();
        for (pi, parte) in raiz
            .children()
            .filter(|c| c.is_element() && nome(*c) == "part")
            .enumerate()
        {
            let pid = parte.attribute("id").unwrap_or("").to_owned();
            let mut divisions = 1_i64;
            let mut declaracoes = 0;
            let mut compassos = Vec::new();
            for (ci, m) in parte
                .children()
                .filter(|c| c.is_element() && nome(*c) == "measure")
                .enumerate()
            {
                let no = decompor(m, texto, 0)?;
                let mut notas = Vec::new();
                for (j, c) in m.children().filter(Node::is_element).enumerate() {
                    match nome(c) {
                        "attributes" => {
                            if let Some(d) = texto_de(c, "divisions") {
                                declaracoes += 1;
                                divisions =
                                    d.parse::<i64>().ok().filter(|v| *v > 0).ok_or_else(|| {
                                        ErroDocumento::NaoSuportado(format!("divisions '{d}'"))
                                    })?;
                            }
                        }
                        "note" => {
                            let idx = 2 * j + 2;
                            if !matches!(no.pecas.get(idx), Some(Peca::Elem(e)) if e.nome == "note")
                            {
                                return Err(ErroDocumento::FaixaInconsistente(
                                    "índice de peça da nota".into(),
                                ));
                            }
                            let ordinal = notas.len();
                            let mut chave = match c.attribute("id") {
                                Some(x) => ChaveNota::Xml(x.to_owned()),
                                None => ChaveNota::Posicao {
                                    parte: pid.clone(),
                                    compasso: ci,
                                    ordinal,
                                },
                            };
                            if !usadas.insert(chave.clone()) {
                                // `id` repetido no arquivo (inválido, mas existe): cai para posição.
                                chave = ChaveNota::Posicao {
                                    parte: pid.clone(),
                                    compasso: ci,
                                    ordinal,
                                };
                                usadas.insert(chave.clone());
                            }
                            let id = match previa.and_then(|t| t.mapa.get(&chave)) {
                                Some(id) => *id,
                                None => tabela.alocar(),
                            };
                            tabela.proximo = tabela.proximo.max(id.0 + 1);
                            tabela.mapa.insert(chave, id);
                            indice.insert(id, (pi, ci, ordinal));
                            notas.push(ler_nota(c, id, idx, divisions));
                        }
                        _ => {}
                    }
                }
                compassos.push(Compasso {
                    numero: m.attribute("number").unwrap_or("").to_owned(),
                    faixa: m.range(),
                    sujo: false,
                    no,
                    notas,
                });
            }
            partes.push(Parte {
                id: pid,
                faixa: parte.range(),
                compassos,
                declaracoes_divisions: declaracoes,
            });
        }
        Ok(Self {
            original: texto.to_owned(),
            partes,
            indice,
            ids: tabela,
        })
    }
}
