//! Relatório do que o modelo atual (`Partitura::de_musicxml`) descarta, por fixture.
//!
//! Para cada categoria conta-se o que há no XML e, quando o modelo tem campo para isso, o que
//! chegou ao modelo. Categorias sem campo no modelo aparecem com `no_modelo = None`: tudo o que
//! o XML tem nelas é descartado. Insumo para a lista de não suportados e para o spike de modelo.

use crate::corpus::Fixture;
use piano_tutor_core::partitura::Partitura;
use roxmltree::{Document, Node, ParsingOptions};

/// Uma categoria medida numa fixture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDescarte {
    pub categoria: &'static str,
    pub no_arquivo: usize,
    /// `None` = o modelo atual não tem campo para esta categoria.
    pub no_modelo: Option<usize>,
}

impl ItemDescarte {
    /// Quantos itens do arquivo não chegaram ao modelo.
    pub fn descartados(&self) -> usize {
        self.no_arquivo.saturating_sub(self.no_modelo.unwrap_or(0))
    }
}

/// Resultado da leitura de uma fixture.
#[derive(Debug, Clone)]
pub struct RelatorioLeitura {
    pub id: &'static str,
    /// `Err` = mensagem de erro do leitor atual.
    pub leitura: Result<(), String>,
    pub itens: Vec<ItemDescarte>,
}

fn conta(doc: &Document<'_>, f: impl Fn(Node<'_, '_>) -> bool) -> usize {
    doc.descendants()
        .filter(|n| n.is_element() && f(*n))
        .count()
}

fn eh(n: Node<'_, '_>, nome: &str) -> bool {
    n.tag_name().name() == nome
}

/// Elementos que o modelo não conhece e que não têm campo previsto: extensões `x-`, `grouping`,
/// `figured-bass`, `harmonic`, `fingernails`.
fn desconhecido(n: Node<'_, '_>) -> bool {
    let nome = n.tag_name().name();
    nome.starts_with("x-")
        || matches!(
            nome,
            "grouping" | "figured-bass" | "harmonic" | "fingernails"
        )
}

/// Lê a fixture com o modelo atual e mede o que foi descartado.
pub fn relatorio_de_leitura(f: &Fixture) -> RelatorioLeitura {
    let leitura = Partitura::de_musicxml(&f.conteudo);
    let doc = Document::parse_with_options(
        &f.conteudo,
        ParsingOptions {
            allow_dtd: true,
            nodes_limit: 4_000_000,
            ..ParsingOptions::default()
        },
    )
    .ok();
    let (modelo, doc) = match (leitura, doc) {
        (Ok(m), Some(d)) => (m, d),
        (Err(e), _) => {
            return RelatorioLeitura {
                id: f.id,
                leitura: Err(e.to_string()),
                itens: Vec::new(),
            };
        }
        (Ok(_), None) => {
            return RelatorioLeitura {
                id: f.id,
                leitura: Err("o modelo leu, mas o harness não analisou o XML".into()),
                itens: Vec::new(),
            };
        }
    };
    let notas_modelo = modelo.total_de_notas();
    let por_nota = |pred: fn(&piano_tutor_core::partitura::Nota) -> bool| {
        modelo
            .partes
            .iter()
            .flat_map(|p| &p.compassos)
            .flat_map(|c| &c.notas)
            .filter(|n| pred(n))
            .count()
    };
    let compassos_primeira_parte = doc
        .descendants()
        .find(|n| n.is_element() && eh(*n, "part"))
        .map_or(0, |p| {
            p.children()
                .filter(|c| c.is_element() && eh(*c, "measure"))
                .count()
        });
    let compassos_com_casa = doc
        .descendants()
        .filter(|n| n.is_element() && eh(*n, "measure"))
        .filter(|m| m.descendants().any(|d| d.is_element() && eh(d, "ending")))
        .count();
    let itens = vec![
        ItemDescarte {
            categoria: "compassos (1ª parte)",
            no_arquivo: compassos_primeira_parte,
            no_modelo: Some(modelo.compassos.len()),
        },
        ItemDescarte {
            categoria: "notas e pausas",
            no_arquivo: conta(&doc, |n| eh(n, "note")),
            no_modelo: Some(notas_modelo),
        },
        ItemDescarte {
            categoria: "notas de adorno (grace)",
            no_arquivo: conta(&doc, |n| eh(n, "grace")),
            no_modelo: Some(por_nota(|n| n.adorno)),
        },
        ItemDescarte {
            categoria: "tercinas e grupos irregulares (time-modification)",
            no_arquivo: conta(&doc, |n| eh(n, "time-modification")),
            no_modelo: Some(por_nota(|n| n.tuplo.is_some())),
        },
        ItemDescarte {
            categoria: "ligaduras de valor (tie start)",
            no_arquivo: conta(&doc, |n| {
                eh(n, "tie") && n.attribute("type") == Some("start")
            }),
            no_modelo: Some(por_nota(|n| n.ligada_ao_proximo)),
        },
        ItemDescarte {
            categoria: "marcas de repetição (repeat; o modelo une as partes, então 2 partes com a mesma barra contam 2 no arquivo e 1 no modelo)",
            no_arquivo: conta(&doc, |n| eh(n, "repeat")),
            no_modelo: Some(
                modelo
                    .compassos
                    .iter()
                    .map(|c| usize::from(c.repeticao_inicio) + usize::from(c.repeticao_fim))
                    .sum(),
            ),
        },
        ItemDescarte {
            categoria: "compassos com casa (ending)",
            no_arquivo: compassos_com_casa,
            no_modelo: Some(
                modelo
                    .compassos
                    .iter()
                    .filter(|c| !c.casas.is_empty())
                    .count(),
            ),
        },
        ItemDescarte {
            categoria: "andamento (sound tempo)",
            no_arquivo: conta(&doc, |n| eh(n, "sound") && n.attribute("tempo").is_some()),
            no_modelo: Some(modelo.andamentos.len()),
        },
        ItemDescarte {
            categoria: "letra (lyric)",
            no_arquivo: conta(&doc, |n| eh(n, "lyric")),
            no_modelo: None,
        },
        ItemDescarte {
            categoria: "cifra (harmony)",
            no_arquivo: conta(&doc, |n| eh(n, "harmony")),
            no_modelo: None,
        },
        ItemDescarte {
            categoria: "dedilhado (fingering)",
            no_arquivo: conta(&doc, |n| eh(n, "fingering")),
            no_modelo: None,
        },
        ItemDescarte {
            categoria: "dinâmica (dynamics, wedge)",
            no_arquivo: conta(&doc, |n| eh(n, "dynamics") || eh(n, "wedge")),
            no_modelo: None,
        },
        ItemDescarte {
            categoria: "ornamentos e fermata",
            no_arquivo: conta(&doc, |n| eh(n, "ornaments") || eh(n, "fermata")),
            no_modelo: None,
        },
        ItemDescarte {
            categoria: "elementos desconhecidos (x-*, grouping, figured-bass, ...)",
            no_arquivo: conta(&doc, desconhecido),
            no_modelo: None,
        },
    ];
    RelatorioLeitura {
        id: f.id,
        leitura: Ok(()),
        itens,
    }
}

/// Relatório em Markdown de todas as fixtures (só categorias com algo no arquivo).
pub fn renderizar_markdown(relatorios: &[RelatorioLeitura]) -> String {
    let mut s = String::from(
        "# Relatório de descartes do modelo atual\n\n\
         Gerado pelo teste `leitura` de `piano-tutor-fixtures` (modelo `Partitura::de_musicxml`).\n\
         \"sem campo\" = o modelo atual não tem onde guardar a categoria: tudo é descartado.\n\n",
    );
    for r in relatorios {
        s.push_str(&format!("## {}\n\n", r.id));
        match &r.leitura {
            Err(e) => s.push_str(&format!("Leitura recusada: {e}\n\n")),
            Ok(()) => {
                s.push_str(
                    "| categoria | no arquivo | no modelo | descartado |\n|---|---|---|---|\n",
                );
                for i in r.itens.iter().filter(|i| i.no_arquivo > 0) {
                    let (modelo, desc) = match i.no_modelo {
                        Some(m) => (m.to_string(), i.descartados().to_string()),
                        None => ("sem campo".to_owned(), i.no_arquivo.to_string()),
                    };
                    s.push_str(&format!(
                        "| {} | {} | {} | {} |\n",
                        i.categoria, i.no_arquivo, modelo, desc
                    ));
                }
                s.push('\n');
            }
        }
    }
    s
}
