//! Diff entre duas Versões (ticket 22, A10): abre as duas pelo hash em [`Versoes`] e compara a
//! **música** (por compasso), com o motor semântico do núcleo
//! ([`piano_tutor_core::documento::diff`]). Sem janela nem egui (seam S1): a Oficina e o MCP (E06)
//! chamam [`diff_entre_versoes`] e só apresentam o resultado.
//!
//! As Versões podem ser quaisquer duas da cadeia (não consecutivas, de ramos diferentes): o
//! ancestral comum não é necessário, pois se comparam os dois conteúdos entre si. Só lê: nunca
//! grava nem altera a cadeia.
//!
//! # Pareamento das notas (ticket 17b)
//!
//! Quando uma das Versões descende da outra pela linhagem das identidades
//! ([`Versoes::ids_comparaveis`]), cada uma é aberta **com o arquivo `.ids` dela** e o diff do
//! núcleo pareia as notas pelo `IdNota` estável (a inserção no meio do compasso aparece como
//! inserção, sem adivinhação). Em qualquer outro caso (Versões irmãs, primas, antigas sem `.ids`,
//! `.ids` de outro texto ou ilegível) as duas abrem **sem** tabela e o diff pareia por conteúdo,
//! como antes: nunca é erro. Irmãs saídas da mesma base herdam o mesmo contador e dariam o mesmo
//! `IdNota` a notas inseridas diferentes; por isso entre elas não se usa o `IdNota`.

use crate::ids_lateral::abrir_tolerante;
use crate::versoes::{ErroVersoes, Hash, Versoes, hash_curto};
use piano_tutor_core::documento::{
    CompassoAlterado, DiffDeDocumentos, DocumentoAberto, Formato, MudancaDeNota, ResumoDeNota,
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
    let por_ids = versoes.ids_comparaveis(antes, depois);
    let a = abrir_documento(versoes, antes, por_ids)?;
    let d = abrir_documento(versoes, depois, por_ids)?;
    Ok(DiffDeVersoes {
        antes: antes.to_owned(),
        depois: depois.to_owned(),
        diff: diff_de_documentos(a.documento(), d.documento()),
    })
}

/// Abre a Versão `hash`; com `com_ids`, usa o arquivo de identidades dela (se servir).
fn abrir_documento(
    versoes: &Versoes,
    hash: &str,
    com_ids: bool,
) -> Result<DocumentoAberto, ErroDiff> {
    let aberta = versoes.abrir(hash)?;
    let nome = format!("{}.musicxml", hash_curto(hash));
    let lateral = aberta.ids.clone().filter(|_| com_ids).map(Ok);
    abrir_tolerante(lateral, |tabela| {
        DocumentoAberto::abrir_bytes_com_ids(
            &nome,
            aberta.conteudo.as_bytes(),
            Formato::MusicXml,
            tabela,
        )
    })
    .map(|(d, _, _)| d)
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

    // --- Identidades das notas (ticket 17b) ------------------------------------------------

    use crate::ids_lateral::IdsGravados;
    use crate::versoes::IdsDaVersao;
    use piano_tutor_core::documento::{Altura, ComandoDeEdicao, PosCompasso};
    use piano_tutor_core::partitura::{Figura, Fracao};

    /// Um compasso 4/4 (divisions 1) sem `id` nas notas: Ré (1), pausa (2), Ré (1).
    fn re_pausa_re() -> String {
        let nota = |p: char, d: u32, t: &str| {
            format!(
                "<note><pitch><step>{p}</step><octave>4</octave></pitch><duration>{d}</duration>\
                 <voice>1</voice><type>{t}</type></note>"
            )
        };
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<score-partwise version=\"4.0\">\n\
             <part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list>\n\
             <part id=\"P1\"><measure number=\"1\"><attributes><divisions>1</divisions>\
             <key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time>\
             </attributes>\n{}\n<note><rest/><duration>2</duration><voice>1</voice>\
             <type>half</type></note>\n{}\n</measure></part></score-partwise>\n",
            nota('D', 1, "quarter"),
            nota('D', 1, "quarter")
        )
    }

    fn cenario_com_texto(texto: &str) -> Cenario {
        let d = tempfile::tempdir().expect("pasta");
        let partituras = d.path().join("oficina/partituras");
        fs::create_dir_all(&partituras).expect("cria");
        fs::write(partituras.join("p.musicxml"), texto).expect("hipotese");
        let v = Versoes::em(&partituras, "p", None);
        v.registrar_origem("Chico", EM).expect("origem");
        let hipotese = v.listar().expect("lista")[0].hash.clone();
        Cenario { _d: d, v, hipotese }
    }

    /// O que a Oficina faz: abre a Hipótese (sem arquivo lateral), insere um Mi de semínima em
    /// `posicao` e salva a Versão com as identidades e a linhagem `derivada_de`.
    fn inserir_mi_e_salvar(c: &Cenario, posicao: i64, pai: &str, derivada_de: &str) -> Hash {
        let aberta = c.v.abrir(&c.hipotese).expect("abre");
        let mut doc = DocumentoAberto::abrir_texto("p", &aberta.conteudo).expect("abre");
        doc.aplicar(ComandoDeEdicao::InserirNota {
            compasso: PosCompasso::nova("P1", 0),
            voz: "1".into(),
            pauta: "1".into(),
            posicao: Fracao::inteiro(posicao),
            altura: Altura {
                passo: 'E',
                alter: 0,
                oitava: 4,
            },
            acidente: None,
            figura: Figura::Seminima,
            pontos: 0,
        })
        .expect("insere");
        let texto = doc.exportar().expect("exporta");
        let tabela = doc.tabela_ids_para(&texto);
        let ids = IdsDaVersao {
            tabela: &tabela,
            derivada_de: Some(derivada_de),
        };
        match c
            .v
            .salvar_com_ids(Some(pai), &texto, Some(ids), Some("m"), "Chico", EM)
        {
            Ok(ResultadoDoSalvar::Criada(v)) => v.hash,
            outro => panic!("esperava Versão criada, veio {outro:?}"),
        }
    }

    fn contar(d: &DiffDeVersoes) -> (usize, usize, usize) {
        let (mut ins, mut rem, mut alt) = (0, 0, 0);
        for c in &d.diff.compassos {
            for m in &c.mudancas {
                match m {
                    MudancaDeNota::Inserida(_) => ins += 1,
                    MudancaDeNota::Removida(_) => rem += 1,
                    MudancaDeNota::Alterada { .. } => alt += 1,
                }
            }
        }
        (ins, rem, alt)
    }

    #[test]
    fn insercao_no_meio_do_compasso_pareia_por_idnota_entre_a_hipotese_e_a_descendente() {
        let c = cenario_com_texto(&re_pausa_re());
        let v1 = inserir_mi_e_salvar(&c, 1, &c.hipotese, &c.hipotese);
        // A Hipótese e a Versão têm identidades comparáveis (a Versão descende da Hipótese).
        assert!(c.v.ids_comparaveis(&c.hipotese, &v1));
        assert!(
            c.v.ids_da_versao(&c.hipotese).is_some(),
            "origem guarda os ids"
        );
        assert!(c.v.ids_da_versao(&v1).is_some());
        let d = diff_entre_versoes(&c.v, &c.hipotese, &v1).expect("diff");
        // A pausa de mínima saiu; entraram o Mi e a pausa de semínima; os dois Ré, idênticos em
        // conteúdo, não são tomados um pelo outro nem "alterados" por ordem.
        assert_eq!(contar(&d), (2, 1, 0), "{:?}", d.diff.compassos[0].mudancas);
        // E nos dois sentidos.
        let volta = diff_entre_versoes(&c.v, &v1, &c.hipotese).expect("diff");
        assert_eq!(contar(&volta), (1, 2, 0));
    }

    #[test]
    fn versoes_irmas_nao_pareiam_por_idnota_e_o_diff_entre_elas_usa_o_conteudo() {
        let c = cenario_com_texto(&re_pausa_re());
        // Duas filhas da mesma Hipótese: as duas herdam o mesmo contador e dão o mesmo IdNota a
        // notas inseridas diferentes (o Mi no tempo 1 e o Mi no tempo 2).
        let a = inserir_mi_e_salvar(&c, 1, &c.hipotese, &c.hipotese);
        let b = inserir_mi_e_salvar(&c, 2, &c.hipotese, &c.hipotese);
        assert!(!c.v.ids_comparaveis(&a, &b), "irmãs não são comparáveis");
        assert!(c.v.ids_comparaveis(&c.hipotese, &a));
        assert!(c.v.ids_comparaveis(&c.hipotese, &b));
        // O hazard é real: pareando pelas tabelas, o mesmo id juntaria notas diferentes.
        let abrir_com = |h: &str| {
            let aberta = c.v.abrir(h).expect("abre");
            let tabela = aberta.ids.expect("ids").tabela;
            DocumentoAberto::abrir_texto_com_ids("x", &aberta.conteudo, Some(&tabela))
                .expect("abre")
        };
        let (da, db) = (abrir_com(&a), abrir_com(&b));
        assert!(da.documento().ids_comparaveis(db.documento()));
        let por_id =
            piano_tutor_core::documento::diff_de_documentos(da.documento(), db.documento());
        let alteradas = por_id.compassos[0]
            .mudancas
            .iter()
            .filter(|m| matches!(m, MudancaDeNota::Alterada { .. }))
            .count();
        assert_eq!(
            alteradas, 2,
            "por IdNota o Mi viraria pausa e a pausa viraria Mi"
        );
        // O diff do app, entre irmãs, compara por conteúdo: sem "alterada" inventada.
        let d = diff_entre_versoes(&c.v, &a, &b).expect("diff");
        assert_eq!(contar(&d), (1, 1, 0), "{:?}", d.diff.compassos[0].mudancas);
    }

    #[test]
    fn nova_sessao_que_reabre_a_hipotese_nao_pareia_por_idnota_com_a_versao_anterior() {
        let c = cenario_com_texto(&re_pausa_re());
        let v1 = inserir_mi_e_salvar(&c, 1, &c.hipotese, &c.hipotese);
        // Outra sessão: a Oficina reabre a Hipótese (a numeração recomeça dali) e o `pai`
        // registrado é a ponta (v1), mas as identidades descendem da Hipótese, não de v1.
        let v2 = inserir_mi_e_salvar(&c, 2, &v1, &c.hipotese);
        assert_eq!(c.v.ponta().expect("ponta").expect("v2").hash, v2);
        assert!(!c.v.ids_comparaveis(&v1, &v2));
        assert!(c.v.ids_comparaveis(&c.hipotese, &v2));
        let d = diff_entre_versoes(&c.v, &v1, &v2).expect("diff");
        assert_eq!(contar(&d), (1, 1, 0));
    }

    #[test]
    fn a_cadeia_de_uma_sessao_so_pareia_por_idnota_ao_longo_da_linhagem() {
        let c = cenario_com_texto(&re_pausa_re());
        let v1 = inserir_mi_e_salvar(&c, 1, &c.hipotese, &c.hipotese);
        // v2 descende de v1 (mesma sessão): a edição vem por cima das identidades de v1.
        let aberta = c.v.abrir(&v1).expect("abre");
        let tabela = aberta.ids.expect("ids").tabela;
        let mut doc = DocumentoAberto::abrir_texto_com_ids("p", &aberta.conteudo, Some(&tabela))
            .expect("abre");
        let id_mi = doc.documento().partes()[0].compassos[0].notas[1].id;
        doc.aplicar(ComandoDeEdicao::RemoverNota { nota: id_mi })
            .expect("remove");
        let texto = doc.exportar().expect("exporta");
        let t2 = doc.tabela_ids_para(&texto);
        let r =
            c.v.salvar_com_ids(
                None,
                &texto,
                Some(IdsDaVersao {
                    tabela: &t2,
                    derivada_de: Some(&v1),
                }),
                None,
                "Chico",
                EM,
            )
            .expect("salva");
        let ResultadoDoSalvar::Criada(v2) = r else {
            panic!("esperava Versão criada");
        };
        // v2 -> v1 -> Hipótese: comparáveis em toda a linhagem, nos dois sentidos.
        assert!(c.v.ids_comparaveis(&c.hipotese, &v2.hash));
        assert!(c.v.ids_comparaveis(&v2.hash, &c.hipotese));
        assert!(c.v.ids_comparaveis(&v1, &v2.hash));
        // Remover a nota só a torna pausa: uma alteração, sem inserção nem remoção.
        let d = diff_entre_versoes(&c.v, &v1, &v2.hash).expect("diff");
        assert_eq!(contar(&d), (0, 0, 1));
    }

    #[test]
    fn linhagem_para_versao_que_nao_existe_ou_de_outra_serie_e_descartada() {
        let c = cenario_com_texto(&re_pausa_re());
        let falsa = "f".repeat(64);
        let v1 = inserir_mi_e_salvar(&c, 1, &c.hipotese, &falsa);
        assert!(c.v.ids_da_versao(&v1).expect("ids").derivada_de.is_none());
        assert!(!c.v.ids_comparaveis(&c.hipotese, &v1));
        // Com o diff, nada quebra: cai no conteúdo.
        let d = diff_entre_versoes(&c.v, &c.hipotese, &v1).expect("diff");
        assert!(!d.diff.vazio());
        // Versão-base de outra série (documento aberto de outro texto) também é descartada.
        let outra = fixture("duas-vozes").conteudo;
        let doc = DocumentoAberto::abrir_texto("o", &outra).expect("abre");
        let tabela = doc.tabela_ids_para(&outra);
        let r =
            c.v.salvar_com_ids(
                Some(&c.hipotese),
                &outra,
                Some(IdsDaVersao {
                    tabela: &tabela,
                    derivada_de: Some(&c.hipotese),
                }),
                None,
                "Chico",
                EM,
            )
            .expect("salva");
        let ResultadoDoSalvar::Criada(v) = r else {
            panic!("esperava Versão criada");
        };
        assert!(
            c.v.ids_da_versao(&v.hash)
                .expect("ids")
                .derivada_de
                .is_none()
        );
    }

    #[test]
    fn versao_sem_arquivo_de_ids_ou_com_ids_de_outro_texto_cai_no_conteudo_sem_erro() {
        let c = cenario_com_texto(&re_pausa_re());
        // Salva por `salvar` (o jeito de antes do 17b): sem arquivo `.ids`.
        let texto = re_pausa_re().replacen("<step>D</step>", "<step>F</step>", 1);
        let antiga = salvar(&c, &c.hipotese, &texto);
        assert!(c.v.ids_da_versao(&antiga).is_none());
        assert!(c.v.abrir(&antiga).expect("abre").ids.is_none());
        let d = diff_entre_versoes(&c.v, &c.hipotese, &antiga).expect("diff");
        assert_eq!(contar(&d), (0, 0, 1));
        // Arquivo `.ids` de outro texto copiado por cima do de uma Versão: recusado ao abrir.
        let v1 = inserir_mi_e_salvar(&c, 1, &c.hipotese, &c.hipotese);
        let alheia = IdsGravados::nova(
            c.v.ids_da_versao(&c.hipotese).expect("ids").tabela,
            Some(&c.hipotese),
        );
        let arquivo = c.v.pasta().join("objetos").join(format!("{v1}.ids"));
        fs::write(&arquivo, alheia.para_texto()).expect("troca");
        // Ainda "descendente" pela linhagem, mas a tabela é de outro texto: recusada ao abrir.
        assert!(c.v.ids_comparaveis(&c.hipotese, &v1));
        let d = diff_entre_versoes(&c.v, &c.hipotese, &v1).expect("sem erro");
        assert!(!d.diff.vazio());
        // Arquivo ilegível: o mesmo.
        fs::write(&arquivo, "lixo").expect("estraga");
        assert!(c.v.ids_da_versao(&v1).is_none());
        assert!(!c.v.ids_comparaveis(&c.hipotese, &v1));
        assert!(diff_entre_versoes(&c.v, &c.hipotese, &v1).is_ok());
        assert!(IdsGravados::de_texto("x").is_err());
    }

    #[test]
    fn o_arquivo_de_ids_da_versao_nunca_e_sobrescrito() {
        let c = cenario_com_texto(&re_pausa_re());
        let v1 = inserir_mi_e_salvar(&c, 1, &c.hipotese, &c.hipotese);
        let antes = c.v.ids_da_versao(&v1).expect("ids");
        // Salvar o mesmo conteúdo de novo (JaExiste) não toca no arquivo.
        let aberta = c.v.abrir(&v1).expect("abre");
        let tabela = antes.tabela.clone();
        let r =
            c.v.salvar_com_ids(
                Some(&c.hipotese),
                &aberta.conteudo,
                Some(IdsDaVersao {
                    tabela: &tabela,
                    derivada_de: None,
                }),
                None,
                "Chico",
                EM,
            )
            .expect("salva");
        assert!(matches!(r, ResultadoDoSalvar::JaExiste { .. }));
        assert_eq!(c.v.ids_da_versao(&v1), Some(antes));
    }

    #[test]
    fn versao_inexistente_e_erro_claro() {
        let c = cenario("dedilhado");
        let falsa = "0".repeat(64);
        let e = diff_entre_versoes(&c.v, &c.hipotese, &falsa).expect_err("erro");
        assert!(matches!(e, ErroDiff::Versoes(ErroVersoes::Inexistente(_))));
    }
}
