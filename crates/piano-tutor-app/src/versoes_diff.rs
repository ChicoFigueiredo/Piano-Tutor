//! Diff entre duas Versões (ticket 22, A10): abre as duas pelo hash em [`Versoes`] e compara a
//! **música** (por compasso), com o motor semântico do núcleo
//! ([`piano_tutor_core::documento::diff`]). Sem janela nem egui (seam S1): a Oficina e o MCP (E06)
//! chamam [`diff_entre_versoes`] e só apresentam o resultado.
//!
//! As Versões podem ser quaisquer duas da cadeia (não consecutivas, de ramos diferentes): o
//! ancestral comum não é necessário, pois se comparam os dois conteúdos entre si. Só lê: nunca
//! grava nem altera a cadeia.

use crate::versoes::{ErroVersoes, Hash, Versoes, hash_curto};
use piano_tutor_core::documento::{
    CompassoAlterado, DiffDeDocumentos, DocumentoAberto, MudancaDeNota, ResumoDeNota,
    diff_de_documentos,
};
use std::fmt;

/// Falha ao calcular o diff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ErroDiff {
    Versoes(ErroVersoes),
    /// O MusicXML de uma das Versões não abriu como documento.
    Documento {
        hash: Hash,
        motivo: String,
    },
}

impl fmt::Display for ErroDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Versoes(e) => write!(f, "{e}"),
            Self::Documento { hash, motivo } => write!(
                f,
                "a Versão {} não abre como partitura: {motivo}",
                hash_curto(hash)
            ),
        }
    }
}

impl std::error::Error for ErroDiff {}

impl From<ErroVersoes> for ErroDiff {
    fn from(e: ErroVersoes) -> Self {
        Self::Versoes(e)
    }
}

/// Diff de duas Versões: quem é o "antes" e o "depois" e os compassos alterados.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffDeVersoes {
    pub antes: Hash,
    pub depois: Hash,
    pub diff: DiffDeDocumentos,
}

impl DiffDeVersoes {
    /// Mensagem para quando não há diferença musical.
    pub const MENSAGEM_SEM_DIFERENCAS: &'static str =
        "Sem diferenças musicais entre as duas Versões.";

    /// Linhas da lista navegável (uma por compasso alterado), na ordem da partitura.
    /// Vazio quando não há diferença: quem apresenta mostra [`Self::MENSAGEM_SEM_DIFERENCAS`].
    pub fn linhas(&self) -> Vec<String> {
        self.diff.compassos.iter().map(linha_do_compasso).collect()
    }
}

/// Calcula o diff semântico entre as Versões `antes` e `depois` (hashes completos).
pub fn diff_entre_versoes(
    versoes: &Versoes,
    antes: &str,
    depois: &str,
) -> Result<DiffDeVersoes, ErroDiff> {
    let a = abrir_documento(versoes, antes)?;
    let d = abrir_documento(versoes, depois)?;
    Ok(DiffDeVersoes {
        antes: antes.to_owned(),
        depois: depois.to_owned(),
        diff: diff_de_documentos(a.documento(), d.documento()),
    })
}

fn abrir_documento(versoes: &Versoes, hash: &str) -> Result<DocumentoAberto, ErroDiff> {
    let aberta = versoes.abrir(hash)?;
    DocumentoAberto::abrir_texto(&format!("{}.musicxml", hash_curto(hash)), &aberta.conteudo)
        .map_err(|e| ErroDiff::Documento {
            hash: hash.to_owned(),
            motivo: e.to_string(),
        })
}

fn nota_em_texto(n: &ResumoDeNota) -> String {
    let base = if n.pausa {
        "pausa".to_owned()
    } else {
        n.altura.clone().unwrap_or_else(|| "?".to_owned())
    };
    match n.duracao {
        Some(d) if d.denominador() == 1 => format!("{base} ({})", d.numerador()),
        Some(d) => format!("{base} ({}/{})", d.numerador(), d.denominador()),
        None => base,
    }
}

/// Uma linha curta por compasso, ex.: `Compasso 3 [P1]: Ré 4 (1) -> Si 4 (1)`.
pub fn linha_do_compasso(c: &CompassoAlterado) -> String {
    let rotulo = if c.pos.numero.is_empty() {
        (c.pos.indice + 1).to_string()
    } else {
        c.pos.numero.clone()
    };
    let corpo = if !c.existe_antes {
        "compasso novo".to_owned()
    } else if !c.existe_depois {
        "compasso removido".to_owned()
    } else {
        let mut partes: Vec<String> = c
            .mudancas
            .iter()
            .map(|m| match m {
                MudancaDeNota::Alterada {
                    antes,
                    depois,
                    campos,
                } => {
                    let marca = if campos.iter().all(|k| k.e_marcacao()) {
                        " [marcação]"
                    } else {
                        ""
                    };
                    format!(
                        "{} -> {}{marca}",
                        nota_em_texto(antes),
                        nota_em_texto(depois)
                    )
                }
                MudancaDeNota::Inserida(n) => format!("+ {}", nota_em_texto(n)),
                MudancaDeNota::Removida(n) => format!("- {}", nota_em_texto(n)),
            })
            .collect();
        if c.cifra.is_some() {
            partes.push("cifra alterada [marcação]".to_owned());
        }
        partes.join("; ")
    };
    format!("Compasso {rotulo} [{}]: {corpo}", c.pos.parte)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::versoes::ResultadoDoSalvar;
    use piano_tutor_core::documento::CampoAlterado;
    use piano_tutor_fixtures::corpus::fixture;
    use std::fs;

    const EM: &str = "2026-11-27T10:00:00Z";

    struct Cenario {
        _d: tempfile::TempDir,
        v: Versoes,
        hipotese: Hash,
    }

    /// Oficina temporária cuja Hipótese é a fixture `id` (própria do projeto, nunca comprada).
    fn cenario(id: &str) -> Cenario {
        let d = tempfile::tempdir().expect("pasta");
        let partituras = d.path().join("oficina/partituras");
        fs::create_dir_all(&partituras).expect("cria");
        let texto = fixture(id).conteudo;
        fs::write(partituras.join("p.musicxml"), &texto).expect("hipotese");
        let v = Versoes::em(&partituras, "p", None);
        v.registrar_origem("Chico", EM).expect("origem");
        let hipotese = v.listar().expect("lista")[0].hash.clone();
        Cenario { _d: d, v, hipotese }
    }

    fn salvar(c: &Cenario, pai: &str, texto: &str) -> Hash {
        match c.v.salvar(Some(pai), texto, Some("motivo"), "Chico", EM) {
            Ok(ResultadoDoSalvar::Criada(v)) => v.hash,
            outro => panic!("esperava Versão criada, veio {outro:?}"),
        }
    }

    #[test]
    fn versoes_identicas_e_so_formatacao_dao_diff_vazio() {
        let c = cenario("dedilhado");
        let h = &c.hipotese;
        assert!(diff_entre_versoes(&c.v, h, h).expect("diff").diff.vazio());
        let formatada = fixture("dedilhado")
            .conteudo
            .replace("<note>", "\n\n        <note>");
        let v1 = salvar(&c, h, &formatada);
        let d = diff_entre_versoes(&c.v, h, &v1).expect("diff");
        assert!(d.diff.vazio());
        assert!(d.linhas().is_empty());
        assert!(!DiffDeVersoes::MENSAGEM_SEM_DIFERENCAS.is_empty());
    }

    #[test]
    fn nao_consecutivas_e_simetrico_em_compassos() {
        let c = cenario("dedilhado");
        let base = fixture("dedilhado").conteudo;
        let t1 = base.replacen("<step>D</step>", "<step>F</step>", 1);
        let t2 = t1.replacen("<step>E</step>", "<step>G</step>", 1);
        let v1 = salvar(&c, &c.hipotese, &t1);
        let v2 = salvar(&c, &v1, &t2);
        let ida = diff_entre_versoes(&c.v, &c.hipotese, &v2).expect("diff");
        let volta = diff_entre_versoes(&c.v, &v2, &c.hipotese).expect("diff");
        assert!(!ida.diff.vazio());
        assert_eq!(ida.diff.posicoes(), volta.diff.posicoes());
        // v1 -> v2 é consecutiva; hipótese -> v2 pula uma Versão e inclui as duas edições.
        let curta = diff_entre_versoes(&c.v, &v1, &v2).expect("diff");
        let total = |d: &DiffDeVersoes| {
            d.diff
                .compassos
                .iter()
                .map(|c| c.mudancas.len())
                .sum::<usize>()
        };
        assert!(total(&curta) < total(&ida));
        assert!(ida.linhas()[0].starts_with("Compasso "));
    }

    #[test]
    fn dedilhado_alterado_aparece_como_marcacao() {
        let c = cenario("dedilhado");
        let base = fixture("dedilhado").conteudo;
        let v1 = salvar(
            &c,
            &c.hipotese,
            &base.replacen("<fingering>1</fingering>", "<fingering>5</fingering>", 1),
        );
        let d = diff_entre_versoes(&c.v, &c.hipotese, &v1).expect("diff");
        assert_eq!(d.diff.compassos.len(), 1);
        let comp = &d.diff.compassos[0];
        assert!(comp.so_marcacao());
        match &comp.mudancas[..] {
            [MudancaDeNota::Alterada { campos, .. }] => {
                assert_eq!(campos, &[CampoAlterado::Dedilhado]);
            }
            outro => panic!("{outro:?}"),
        }
        assert!(d.linhas()[0].contains("[marcação]"));
    }

    #[test]
    fn letra_e_cifra_alteradas_aparecem_com_marcacao() {
        let c = cenario("letra");
        let base = fixture("letra").conteudo;
        let t = base.replacen("<text>Pi</text>", "<text>Pa</text>", 1);
        let v1 = salvar(&c, &c.hipotese, &t);
        let d = diff_entre_versoes(&c.v, &c.hipotese, &v1).expect("diff");
        assert_eq!(d.diff.compassos.len(), 1);
        assert!(d.diff.compassos[0].so_marcacao());

        let c = cenario("cifra");
        let base = fixture("cifra").conteudo;
        let v1 = salvar(&c, &c.hipotese, &base.replacen(">major<", ">minor<", 1));
        let d = diff_entre_versoes(&c.v, &c.hipotese, &v1).expect("diff");
        assert_eq!(d.diff.compassos.len(), 1);
        assert!(d.diff.compassos[0].cifra.is_some());
        assert!(d.linhas()[0].contains("cifra alterada"));
    }

    #[test]
    fn nota_alterada_traz_antes_e_depois() {
        let c = cenario("duas-vozes");
        let base = fixture("duas-vozes").conteudo;
        let t = base.replacen("<step>C</step>", "<step>A</step>", 1);
        let v1 = salvar(&c, &c.hipotese, &t);
        let d = diff_entre_versoes(&c.v, &c.hipotese, &v1).expect("diff");
        assert_eq!(d.diff.compassos.len(), 1);
        let comp = &d.diff.compassos[0];
        assert_eq!(comp.antes.len(), comp.depois.len());
        assert!(matches!(comp.mudancas[0], MudancaDeNota::Alterada { .. }));
    }

    #[test]
    fn versao_inexistente_e_erro_claro() {
        let c = cenario("dedilhado");
        let falsa = "0".repeat(64);
        let e = diff_entre_versoes(&c.v, &c.hipotese, &falsa).expect_err("erro");
        assert!(matches!(e, ErroDiff::Versoes(ErroVersoes::Inexistente(_))));
    }
}
