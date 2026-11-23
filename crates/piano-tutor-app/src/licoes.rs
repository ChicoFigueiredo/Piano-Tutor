//! Lições em arquivos `.toml` (ADR-0009): leitura, validação e notação dos passos.
//!
//! Uma Lição por arquivo, agrupada em Trilhas. As Lições de fábrica (`assets/licoes/`) são
//! embutidas no binário pelo `build.rs`; as do usuário ficam em `<pasta de dados>/licoes/` e
//! substituem a de fábrica com o mesmo `id`. Formato: `docs/referencias/FORMATO-LICOES.md`.

use crate::exercicios::{Exercicio, Passo, Tipo};
use piano_tutor_core::NotaMidi;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Subpasta da pasta de dados com as Lições do usuário.
pub const PASTA_LICOES: &str = "licoes";
/// Faixa do teclado de 61 teclas (C2..C7).
pub const NOTA_MIN: u8 = 36;
pub const NOTA_MAX: u8 = 96;
/// Arquivos maiores que isto são recusados (proteção contra arquivo errado).
const TAMANHO_MAX: u64 = 512 * 1024;

include!(concat!(env!("OUT_DIR"), "/licoes_embutidas.rs"));

/// Estado de revisão (CONTEXT.md: Gabarito só a partir de conteúdo aprovado).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Situacao {
    Proposta,
    Aprovada,
}

/// De onde a Lição veio.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Origem {
    Fabrica(&'static str),
    Usuario(PathBuf),
}

/// Lição: metadados e seus Exercícios (ids dos Exercícios já vêm como `licao/exercicio`).
#[derive(Clone, Debug, PartialEq)]
pub struct Licao {
    pub id: String,
    pub trilha: String,
    pub ordem_trilha: i32,
    pub ordem: i32,
    pub titulo: String,
    pub descricao: String,
    pub nivel: String,
    pub situacao: Situacao,
    pub fontes: Vec<String>,
    /// Lições que precisam estar concluídas antes.
    pub requer: Vec<String>,
    pub exercicios: Vec<Exercicio>,
    pub origem: Origem,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArquivoToml {
    licao: LicaoToml,
    #[serde(default)]
    exercicio: Vec<ExercicioToml>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LicaoToml {
    id: String,
    trilha: String,
    #[serde(default)]
    ordem_trilha: i32,
    #[serde(default)]
    ordem: i32,
    titulo: String,
    #[serde(default)]
    descricao: String,
    #[serde(default)]
    nivel: String,
    #[serde(default)]
    situacao: Option<String>,
    #[serde(default)]
    fontes: Vec<String>,
    #[serde(default)]
    requer: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExercicioToml {
    id: String,
    titulo: String,
    tipo: String,
    #[serde(default)]
    mao: String,
    objetivo: String,
    instrucoes: String,
    #[serde(default)]
    dedilhado: String,
    passos: String,
    /// Passo a passo de como fazer (opcional).
    #[serde(default)]
    roteiro: Vec<String>,
    #[serde(default)]
    acerto: Option<String>,
    #[serde(default)]
    erro: Option<String>,
    #[serde(default)]
    conclusao: Option<String>,
    /// `None`: exige o anterior; `Some("")`: livre; `Some(id)`: exige aquele exercício.
    #[serde(default)]
    requer: Option<String>,
    /// Reservado (ADR-0009): ignorado com aviso.
    #[serde(default)]
    script: Option<String>,
}

fn id_valido(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn tipo_de(s: &str) -> Result<Tipo, String> {
    Ok(match s {
        "localizar" => Tipo::Localizar,
        "sequencia" => Tipo::Sequencia,
        "intervalos" => Tipo::Intervalos,
        "escala" => Tipo::Escala,
        "acorde" => Tipo::Acorde,
        "arpejo" => Tipo::Arpejo,
        "ouvir" => Tipo::OuvirERepetir,
        outro => return Err(format!("tipo desconhecido \"{outro}\"")),
    })
}

/// Lê uma nota (`C4`, `F#3`, `Bb`, `E`). Sem oitava: `(nota na oitava 4, true)`.
pub fn ler_nota(tok: &str) -> Result<(NotaMidi, bool), String> {
    let mut c = tok.chars();
    let base: i32 = match c.next().map(|x| x.to_ascii_uppercase()) {
        Some('C') => 0,
        Some('D') => 2,
        Some('E') => 4,
        Some('F') => 5,
        Some('G') => 7,
        Some('A') => 9,
        Some('B') => 11,
        _ => return Err(format!("nota inválida \"{tok}\"")),
    };
    let resto: String = c.collect();
    let (acidente, oitava) = match resto.chars().next() {
        Some('#') => (1, &resto[1..]),
        Some('b') => (-1, &resto[1..]),
        _ => (0, resto.as_str()),
    };
    let (oitava, qualquer) = if oitava.is_empty() {
        (4, true)
    } else {
        match oitava.parse::<i32>() {
            Ok(o) if (2..=7).contains(&o) => (o, false),
            _ => return Err(format!("oitava inválida em \"{tok}\" (use 2 a 7)")),
        }
    };
    let midi = (oitava + 1) * 12 + base + acidente;
    let ok = if qualquer {
        (0..=127).contains(&midi)
    } else {
        (i32::from(NOTA_MIN)..=i32::from(NOTA_MAX)).contains(&midi)
    };
    if !ok {
        return Err(format!(
            "\"{tok}\" fica fora do teclado de 61 teclas (C2 a C7)"
        ));
    }
    let nota = u8::try_from(midi)
        .ok()
        .and_then(NotaMidi::nova)
        .ok_or_else(|| format!("nota inválida \"{tok}\""))?;
    Ok((nota, qualquer))
}

/// Separa a legenda opcional (`...:texto_com_sublinhado`).
fn com_legenda(tok: &str) -> (&str, String) {
    match tok.split_once(':') {
        Some((a, b)) => (a, b.replace('_', " ")),
        None => (tok, String::new()),
    }
}

/// Lê a notação de `passos` (ver FORMATO-LICOES.md).
pub fn ler_passos(texto: &str) -> Result<Vec<Passo>, String> {
    let mut passos = Vec::new();
    let mut resto = texto.trim();
    while !resto.is_empty() {
        let abre = resto.chars().next().unwrap_or(' ');
        if abre == '[' || abre == '{' {
            let fecha = if abre == '[' { ']' } else { '}' };
            let Some(fim) = resto.find(fecha) else {
                return Err(format!("faltou fechar \"{abre}\" em \"{resto}\""));
            };
            let dentro = &resto[1..fim];
            let depois = &resto[fim + 1..];
            let (legenda_tok, prox) = match depois.find(char::is_whitespace) {
                Some(i) => (&depois[..i], &depois[i..]),
                None => (depois, ""),
            };
            let legenda = match legenda_tok.strip_prefix(':') {
                Some(l) => l.replace('_', " "),
                None if legenda_tok.is_empty() => String::new(),
                None => {
                    return Err(format!(
                        "texto solto depois de \"{fecha}\": \"{legenda_tok}\""
                    ));
                }
            };
            let mut notas = Vec::new();
            for t in dentro.split_whitespace() {
                let (n, qualquer) = ler_nota(t)?;
                if qualquer {
                    return Err(format!(
                        "dentro de \"{abre}...{fecha}\" use nota com oitava: \"{t}\""
                    ));
                }
                if !notas.contains(&n) {
                    notas.push(n);
                }
            }
            if notas.len() < 2 {
                return Err(format!(
                    "\"{abre}{dentro}{fecha}\" precisa de pelo menos duas notas"
                ));
            }
            passos.push(Passo {
                notas,
                por_classe: abre == '{',
                legenda,
            });
            resto = prox.trim_start();
        } else {
            let (tok, prox) = match resto.find(char::is_whitespace) {
                Some(i) => (&resto[..i], &resto[i..]),
                None => (resto, ""),
            };
            let (nota_tok, legenda) = com_legenda(tok);
            let (n, qualquer) = ler_nota(nota_tok)?;
            passos.push(Passo {
                notas: vec![n],
                por_classe: qualquer,
                legenda,
            });
            resto = prox.trim_start();
        }
    }
    if passos.is_empty() {
        return Err("gabarito vazio".to_owned());
    }
    Ok(passos)
}

fn padrao_acerto(t: Tipo) -> &'static str {
    match t {
        Tipo::Acorde => "Acorde certo, as notas soaram juntas.",
        Tipo::Intervalos => "Intervalo certo.",
        Tipo::OuvirERepetir => "Você ouviu e acertou essa nota.",
        _ => "Nota certa, siga para a próxima.",
    }
}

fn padrao_erro(t: Tipo) -> &'static str {
    match t {
        Tipo::Acorde => "Toque só as notas do acorde, juntas, sem nota a mais.",
        Tipo::Intervalos => "Toque só as duas notas marcadas, juntas.",
        Tipo::OuvirERepetir => "Essa nota não é a da frase. Ouça de novo e tente outra.",
        _ => "Toque a tecla marcada; a nota anterior continua valendo.",
    }
}

/// Lê uma Lição de um texto TOML. `avisos` recebe o que foi ignorado sem impedir a leitura.
pub fn ler_licao(texto: &str, origem: Origem, avisos: &mut Vec<String>) -> Result<Licao, String> {
    let a: ArquivoToml = toml::from_str(texto).map_err(|e| e.message().to_owned())?;
    let l = a.licao;
    if !id_valido(&l.id) {
        return Err(format!(
            "id de Lição inválido \"{}\" (use a-z, 0-9 e -)",
            l.id
        ));
    }
    if l.trilha.trim().is_empty() || l.titulo.trim().is_empty() {
        return Err(format!("Lição {}: trilha e título são obrigatórios", l.id));
    }
    let situacao = match l.situacao.as_deref() {
        None | Some("proposta") => Situacao::Proposta,
        Some("aprovada") => Situacao::Aprovada,
        Some(s) => return Err(format!("Lição {}: situação desconhecida \"{s}\"", l.id)),
    };
    if a.exercicio.is_empty() {
        return Err(format!("Lição {}: nenhum exercício", l.id));
    }
    let mut exercicios: Vec<Exercicio> = Vec::with_capacity(a.exercicio.len());
    for (i, x) in a.exercicio.into_iter().enumerate() {
        let ctx = format!("Lição {}, exercício {}", l.id, x.id);
        if !id_valido(&x.id) {
            return Err(format!("{ctx}: id inválido (use a-z, 0-9 e -)"));
        }
        let id = format!("{}/{}", l.id, x.id);
        if exercicios.iter().any(|e| e.id == id) {
            return Err(format!("{ctx}: id repetido"));
        }
        let tipo = tipo_de(&x.tipo).map_err(|e| format!("{ctx}: {e}"))?;
        let passos = ler_passos(&x.passos).map_err(|e| format!("{ctx}: {e}"))?;
        match tipo {
            Tipo::Intervalos if passos.iter().any(|p| p.notas.len() < 2 || p.por_classe) => {
                return Err(format!("{ctx}: intervalos usam passos [..]"));
            }
            Tipo::Acorde if passos.iter().any(|p| !p.por_classe || p.notas.len() < 2) => {
                return Err(format!("{ctx}: acordes usam passos {{..}}"));
            }
            _ => {}
        }
        if x.roteiro.iter().any(|r| r.trim().is_empty()) {
            return Err(format!("{ctx}: roteiro com item vazio"));
        }
        if x.script.is_some() {
            avisos.push(format!(
                "{ctx}: \"script\" ainda não é suportado e foi ignorado (ADR-0009)"
            ));
        }
        let prerequisito = match x.requer.as_deref() {
            Some("") => None,
            Some(r) if r.contains('/') => Some(r.to_owned()),
            Some(r) => Some(format!("{}/{r}", l.id)),
            None => (i > 0).then(|| exercicios[i - 1].id.clone()),
        };
        exercicios.push(Exercicio {
            id,
            numero: u16::try_from(i + 1).unwrap_or(u16::MAX),
            licao: 0,
            titulo: x.titulo,
            tipo,
            mao: x.mao,
            objetivo: x.objetivo,
            instrucoes: x.instrucoes,
            dedilhado: x.dedilhado,
            passos,
            roteiro: x.roteiro,
            prerequisito,
            feedback_acerto: x.acerto.unwrap_or_else(|| padrao_acerto(tipo).to_owned()),
            feedback_erro: x.erro.unwrap_or_else(|| padrao_erro(tipo).to_owned()),
            conclusao: x
                .conclusao
                .unwrap_or_else(|| "Exercício concluído.".to_owned()),
        });
    }
    for r in &l.requer {
        if !id_valido(r) {
            return Err(format!("Lição {}: requer inválido \"{r}\"", l.id));
        }
    }
    Ok(Licao {
        id: l.id,
        trilha: l.trilha.trim().to_owned(),
        ordem_trilha: l.ordem_trilha,
        ordem: l.ordem,
        titulo: l.titulo,
        descricao: l.descricao,
        nivel: l.nivel,
        situacao,
        fontes: l.fontes,
        requer: l.requer,
        exercicios,
        origem,
    })
}

/// Lições embutidas no binário. Erros (não deveriam existir: há teste) vão para `erros`.
pub fn licoes_de_fabrica(erros: &mut Vec<String>) -> Vec<Licao> {
    let mut v = Vec::new();
    for (nome, texto) in LICOES_EMBUTIDAS {
        match ler_licao(texto, Origem::Fabrica(nome), erros) {
            Ok(l) => v.push(l),
            Err(e) => erros.push(format!("{nome}: {e}")),
        }
    }
    v
}

/// Lições da pasta do usuário (`<pasta>/licoes/*.toml`); ausência da pasta não é erro.
pub fn licoes_do_usuario(pasta_dados: &Path, erros: &mut Vec<String>) -> Vec<Licao> {
    let dir = pasta_dados.join(PASTA_LICOES);
    let Ok(entradas) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut caminhos: Vec<PathBuf> = entradas
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    caminhos.sort();
    let mut v = Vec::new();
    for c in caminhos {
        let nome = c
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let texto = match std::fs::metadata(&c) {
            Ok(m) if m.len() > TAMANHO_MAX => {
                erros.push(format!("{nome}: arquivo grande demais"));
                continue;
            }
            Ok(_) => match std::fs::read(&c) {
                Ok(b) => match String::from_utf8(b) {
                    Ok(t) => t,
                    Err(_) => {
                        erros.push(format!("{nome}: não é texto UTF-8"));
                        continue;
                    }
                },
                Err(e) => {
                    erros.push(format!("{nome}: {e}"));
                    continue;
                }
            },
            Err(e) => {
                erros.push(format!("{nome}: {e}"));
                continue;
            }
        };
        match ler_licao(&texto, Origem::Usuario(c.clone()), erros) {
            Ok(l) => v.push(l),
            Err(e) => erros.push(format!("{nome}: {e}")),
        }
    }
    v
}

/// Junta fábrica e usuário (o usuário substitui pelo `id`) e ordena por Trilha e Lição.
pub fn juntar(fabrica: Vec<Licao>, usuario: Vec<Licao>, erros: &mut Vec<String>) -> Vec<Licao> {
    let mut v: Vec<Licao> = Vec::with_capacity(fabrica.len() + usuario.len());
    for l in fabrica.into_iter().chain(usuario) {
        if let Some(i) = v.iter().position(|x| x.id == l.id) {
            if matches!(v[i].origem, Origem::Fabrica(_)) && matches!(l.origem, Origem::Usuario(_)) {
                v[i] = l;
            } else {
                erros.push(format!("Lição {} repetida; a segunda foi ignorada", l.id));
            }
        } else {
            v.push(l);
        }
    }
    // A ordem da Trilha é a menor declarada por suas Lições.
    let ordem_da = |t: &str, v: &[Licao]| {
        v.iter()
            .filter(|l| l.trilha == t)
            .map(|l| l.ordem_trilha)
            .min()
            .unwrap_or(0)
    };
    let chaves: Vec<(i32, String, i32, String)> = v
        .iter()
        .map(|l| {
            (
                ordem_da(&l.trilha, &v),
                l.trilha.clone(),
                l.ordem,
                l.id.clone(),
            )
        })
        .collect();
    let mut idx: Vec<usize> = (0..v.len()).collect();
    idx.sort_by(|a, b| chaves[*a].cmp(&chaves[*b]));
    let mut ordenadas: Vec<Option<Licao>> = v.into_iter().map(Some).collect();
    idx.into_iter()
        .filter_map(|i| ordenadas[i].take())
        .collect()
}

/// Todas as Lições: fábrica e, se houver pasta, as do usuário.
pub fn carregar_todas(pasta_dados: Option<&Path>, erros: &mut Vec<String>) -> Vec<Licao> {
    let fabrica = licoes_de_fabrica(erros);
    let usuario = pasta_dados.map_or_else(Vec::new, |p| licoes_do_usuario(p, erros));
    juntar(fabrica, usuario, erros)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn n(x: u8) -> NotaMidi {
        NotaMidi::nova(x).expect("nota")
    }

    #[test]
    fn notas_com_e_sem_oitava() {
        assert_eq!(ler_nota("C4"), Ok((n(60), false)));
        assert_eq!(ler_nota("F#3"), Ok((n(54), false)));
        assert_eq!(ler_nota("Bb4"), Ok((n(70), false)));
        assert_eq!(ler_nota("C2"), Ok((n(36), false)));
        assert_eq!(ler_nota("C7"), Ok((n(96), false)));
        assert_eq!(ler_nota("E"), Ok((n(64), true)));
        assert!(ler_nota("B1").is_err());
        assert!(ler_nota("C#7").is_err(), "97 fica fora das 61 teclas");
        assert!(ler_nota("H4").is_err());
        assert!(ler_nota("C44").is_err());
        assert!(ler_nota("").is_err());
    }

    #[test]
    fn passos_simples_conjuntos_acordes_e_legendas() {
        let p = ler_passos("C4 [C4 E4]:terça_maior {A3 C4 E4}:Lá_menor D").expect("passos");
        assert_eq!(p.len(), 4);
        assert_eq!(p[0].notas, vec![n(60)]);
        assert!(!p[0].por_classe);
        assert_eq!(p[1].notas, vec![n(60), n(64)]);
        assert!(!p[1].por_classe);
        assert_eq!(p[1].legenda, "terça maior");
        assert!(p[2].por_classe);
        assert_eq!(p[2].legenda, "Lá menor");
        assert!(p[3].por_classe, "nota sem oitava vale em qualquer oitava");
    }

    #[test]
    fn passos_invalidos_sao_recusados_com_mensagem() {
        for ruim in ["", "C4 [C4 E4", "{C4}", "[C E]", "C4 X9", "[C4 E4]x", "C8"] {
            assert!(ler_passos(ruim).is_err(), "deveria recusar \"{ruim}\"");
        }
    }

    const MINIMA: &str = r#"
[licao]
id = "teste"
trilha = "Teste"
titulo = "Lição de teste"

[[exercicio]]
id = "a"
titulo = "A"
tipo = "sequencia"
objetivo = "o"
instrucoes = "i"
passos = "C4 D4"

[[exercicio]]
id = "b"
titulo = "B"
tipo = "acorde"
objetivo = "o"
instrucoes = "i"
passos = "{C4 E4 G4}"
script = "return true"
"#;

    #[test]
    fn licao_minima_le_ids_prerequisitos_e_padroes() {
        let mut avisos = Vec::new();
        let l = ler_licao(MINIMA, Origem::Fabrica("t"), &mut avisos).expect("lição");
        assert_eq!(l.situacao, Situacao::Proposta, "sem situação = proposta");
        assert_eq!(l.exercicios[0].id, "teste/a");
        assert_eq!(l.exercicios[0].prerequisito, None);
        assert_eq!(l.exercicios[1].prerequisito.as_deref(), Some("teste/a"));
        assert!(!l.exercicios[1].feedback_acerto.is_empty());
        assert_eq!(avisos.len(), 1, "script é ignorado com aviso");
    }

    #[test]
    fn licao_com_erro_e_recusada_sem_panico() {
        let mut av = Vec::new();
        for ruim in [
            MINIMA.replace("tipo = \"acorde\"", "tipo = \"danca\""),
            MINIMA.replace("passos = \"{C4 E4 G4}\"", "passos = \"C4\""),
            MINIMA.replace("id = \"b\"", "id = \"a\""),
            MINIMA.replace("id = \"teste\"", "id = \"Teste X\""),
            MINIMA.replace("titulo = \"A\"", "titulo = \"A\"\ncampo_estranho = 1"),
            "isto não é toml = = =".to_owned(),
        ] {
            assert!(ler_licao(&ruim, Origem::Fabrica("t"), &mut av).is_err());
        }
    }

    #[test]
    fn usuario_substitui_fabrica_e_ordem_segue_trilha_e_licao() {
        let mut av = Vec::new();
        let mk = |id: &str, trilha: &str, ot: i32, o: i32, origem: Origem| {
            let t = MINIMA
                .replace("id = \"teste\"", &format!("id = \"{id}\""))
                .replace(
                    "trilha = \"Teste\"",
                    &format!("trilha = \"{trilha}\"\nordem_trilha = {ot}\nordem = {o}"),
                );
            ler_licao(&t, origem, &mut Vec::new()).expect("lição")
        };
        let fab = vec![
            mk("b", "Escalas", 20, 1, Origem::Fabrica("b")),
            mk("a", "Fundamentos", 10, 2, Origem::Fabrica("a")),
            mk("c", "Fundamentos", 10, 1, Origem::Fabrica("c")),
        ];
        let mut minha = mk(
            "b",
            "Escalas",
            20,
            1,
            Origem::Usuario(PathBuf::from("/x/b.toml")),
        );
        minha.titulo = "Minha versão".to_owned();
        let v = juntar(fab, vec![minha], &mut av);
        let ids: Vec<&str> = v.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(ids, ["c", "a", "b"]);
        assert_eq!(v[2].titulo, "Minha versão");
        assert!(av.is_empty());
    }

    #[test]
    fn licoes_de_fabrica_sao_todas_validas_e_unicas() {
        let mut erros = Vec::new();
        let v = carregar_todas(None, &mut erros);
        assert!(erros.is_empty(), "erros nas Lições de fábrica: {erros:#?}");
        assert!(!v.is_empty());
        assert_eq!(v[0].id, "primeiros-passos", "a primeira Lição é a da F05");
        let mut ids: Vec<&str> = v.iter().map(|l| l.id.as_str()).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "ids de Lição únicos");
        for l in &v {
            assert!(!l.fontes.is_empty(), "Lição {} sem fontes", l.id);
            for x in &l.exercicios {
                if let Some(p) = &x.prerequisito {
                    assert!(
                        v.iter().flat_map(|l| &l.exercicios).any(|y| &y.id == p),
                        "{}: pré-requisito {p} não existe",
                        x.id
                    );
                }
            }
            for r in &l.requer {
                assert!(
                    v.iter().any(|y| &y.id == r),
                    "Lição {}: requer {r} não existe",
                    l.id
                );
            }
        }
    }

    #[test]
    fn pasta_do_usuario_soma_licoes_e_relata_arquivo_ruim() {
        let pasta = std::env::temp_dir().join(format!("pt-licoes-{}", std::process::id()));
        let dir = pasta.join(PASTA_LICOES);
        std::fs::create_dir_all(&dir).expect("pasta");
        std::fs::write(
            dir.join("minha.toml"),
            MINIMA.replace("\"teste\"", "\"minha\""),
        )
        .expect("w");
        std::fs::write(dir.join("ruim.toml"), "[licao]\nid=1").expect("w");
        std::fs::write(dir.join("ignorado.txt"), "x").expect("w");
        let mut erros = Vec::new();
        let v = carregar_todas(Some(&pasta), &mut erros);
        assert!(v.iter().any(|l| l.id == "minha"));
        let ruins: Vec<&String> = erros.iter().filter(|e| !e.contains("script")).collect();
        assert_eq!(ruins.len(), 1, "{erros:?}");
        assert!(ruins[0].starts_with("ruim.toml"));
        let _ = std::fs::remove_dir_all(&pasta);
    }
}
