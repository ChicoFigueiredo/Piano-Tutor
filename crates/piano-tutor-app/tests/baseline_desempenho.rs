//! Baseline de desempenho dos comandos numa peça de 100 compassos (E01, ticket 26, A16).
//!
//! **Sem asserção de meta**: nada aqui falha por tempo. A meta de 100 ms é "a calibrar" (A16) e
//! só é comparada no relatório, como observação. Dois testes:
//!
//! - `fumaca_do_baseline` roda no `cargo test` padrão, com 1 execução por comando, só para
//!   garantir que o harness compila e executa (qualquer perfil; o tempo medido não vale nada);
//! - `baseline_de_desempenho` é `#[ignore]`: o benchmark de verdade, para o perfil release.
//!
//! Reprodução (a partir da raiz do repositório):
//!
//! ```text
//! PT_BASELINE_N=200 PT_BASELINE_SAIDA=docs/evolucao-musical/e01/desempenho \
//!   cargo test --release -p piano-tutor-app --test baseline_desempenho \
//!   -- --ignored --nocapture
//! ```
//!
//! `PT_BASELINE_N` = execuções por comando leve (padrão 200); os comandos que tocam disco
//! (salvar Versão, diff, conferência) usam `N/4` (mínimo 10). A peça vem do gerador de fixtures
//! (`gerar_cem_compassos`), sintética e CC0. Em cada comando de edição só o `aplicar` é medido;
//! o `desfazer` que devolve o documento ao estado de partida fica fora da medição.

use piano_tutor_app::documento_partitura::DocumentoPartitura;
use piano_tutor_app::geometria::Ponto;
use piano_tutor_app::partitura_visual::{
    calcular_layout, calcular_layout_com_exibicao, calcular_layout_com_ids, calcular_layout_fita,
};
use piano_tutor_app::pratica_versao::PraticaDaVersao;
use piano_tutor_app::tela_partitura::EstadoPartitura;
use piano_tutor_app::versoes::{ResultadoDoSalvar, Versoes};
use piano_tutor_app::versoes_diff::diff_entre_versoes;
use piano_tutor_core::documento::{
    ComandoDeEdicao, DocumentoAberto, Formato, IdNota, ResultadoDaEdicao, SentidoDaAltura,
    UnidadeDeAltura, diff_de_documentos,
};
use piano_tutor_core::mxl::empacotar_mxl;
use piano_tutor_core::partitura::Figura;
use piano_tutor_fixtures::gerador::{COMPASSOS_LONGA, gerar_cem_compassos};
use std::fmt::Write as _;
use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

const EM: &str = "2026-11-28T10:00:00Z";
const LARGURA_SP: f32 = 1200.0;
const META_MS: f64 = 100.0;
const DESCARTE: usize = 3;

struct Medida {
    id: &'static str,
    descricao: &'static str,
    ticket: &'static str,
    amostras_ms: Vec<f64>,
}

struct Estatistica {
    n: usize,
    min: f64,
    mediana: f64,
    media: f64,
    p95: f64,
    max: f64,
}

fn estatistica(a: &[f64]) -> Estatistica {
    let mut v = a.to_vec();
    v.sort_by(|x, y| x.total_cmp(y));
    let n = v.len();
    // p95 por posto mais próximo (nearest-rank): o menor valor com >= 95% das amostras abaixo.
    let rank = ((0.95 * n as f64).ceil() as usize).clamp(1, n);
    let mediana = if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    };
    Estatistica {
        n,
        min: v[0],
        mediana,
        media: v.iter().sum::<f64>() / n as f64,
        p95: v[rank - 1],
        max: v[n - 1],
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// Mede `f` `n` vezes (depois de `DESCARTE` execuções de aquecimento descartadas).
fn medir<T>(n: usize, mut f: impl FnMut() -> T) -> Vec<f64> {
    for _ in 0..DESCARTE {
        black_box(f());
    }
    (0..n)
        .map(|_| {
            let t = Instant::now();
            black_box(f());
            ms(t)
        })
        .collect()
}

/// Mede só o trecho cronometrado de `f`: `f(i, &mut cronometro)` devolve o tempo em ms.
fn medir_trecho(n: usize, mut f: impl FnMut(usize) -> f64) -> Vec<f64> {
    (0..(n + DESCARTE)).map(&mut f).skip(DESCARTE).collect()
}

fn nota(a: &DocumentoAberto, compasso: usize, ordinal: usize) -> IdNota {
    a.ids()
        .id(0, compasso, ordinal)
        .expect("a peça tem 4 notas por compasso")
}

fn altura_acima(a: &DocumentoAberto, compasso: usize, ordinal: usize) -> ComandoDeEdicao {
    ComandoDeEdicao::MudarAltura {
        nota: nota(a, compasso, ordinal),
        unidade: UnidadeDeAltura::Semitom,
        sentido: SentidoDaAltura::Acima,
    }
}

/// Aplica `c` fora da medição.
fn preparar(a: &mut DocumentoAberto, c: ComandoDeEdicao) -> ResultadoDaEdicao {
    a.aplicar(c).expect("o comando do benchmark é válido")
}

/// Aplica um comando medindo só `aplicar`; depois desfaz (fora da medição).
fn medir_comando(
    n: usize,
    a: &mut DocumentoAberto,
    mut comando: impl FnMut(&DocumentoAberto, usize) -> ComandoDeEdicao,
) -> Vec<f64> {
    medir_trecho(n, |i| {
        let c = comando(a, i);
        let t = Instant::now();
        let r = a.aplicar(c);
        let dt = ms(t);
        r.expect("o comando do benchmark é válido");
        a.desfazer();
        dt
    })
}

fn rodar(n: usize) -> (Vec<Medida>, String) {
    let n_pesado = (n / 4).max(10);
    // A peça do gerador tem `divisions` 1 (semínima = 1), o que recusa colcheia e ponto de
    // aumento (exigem mudar `divisions`, outro comando). O benchmark usa a mesma peça com
    // `divisions` 4 (durations multiplicadas por 4): mesmas notas, mesma estrutura.
    let xml = gerar_cem_compassos()
        .replace("<divisions>1</divisions>", "<divisions>4</divisions>")
        .replace("<duration>1</duration>", "<duration>4</duration>");
    assert_eq!(COMPASSOS_LONGA, 100);
    let mxl = empacotar_mxl("score.xml", &xml).expect("empacota a peça");
    let mut m: Vec<Medida> = Vec::new();
    let mut add = |id, descricao, ticket, amostras_ms| {
        m.push(Medida {
            id,
            descricao,
            ticket,
            amostras_ms,
        });
    };

    // --- Abrir ---------------------------------------------------------------------------
    add(
        "abrir_musicxml",
        "DocumentoAberto::abrir_texto (MusicXML de 100 compassos)",
        "10",
        medir(n, || {
            DocumentoAberto::abrir_texto("cem.musicxml", &xml).expect("abre")
        }),
    );
    add(
        "abrir_mxl",
        "DocumentoAberto::abrir_bytes (.mxl, inclui descompactar)",
        "10",
        medir(n, || {
            DocumentoAberto::abrir_bytes("cem.mxl", &mxl, Formato::Mxl).expect("abre")
        }),
    );
    add(
        "abrir_documento_da_tela",
        "DocumentoPartitura::de_texto (abrir + Musica + índice de leitura + exibição)",
        "19",
        medir(n, || {
            DocumentoPartitura::de_texto("cem.musicxml".into(), xml.clone()).expect("abre")
        }),
    );

    // --- Comandos de edição ------------------------------------------------------------------
    let mut a = DocumentoAberto::abrir_texto("cem.musicxml", &xml).expect("abre");
    let alvo = |i: usize| (i % COMPASSOS_LONGA, i % 4);

    add(
        "altura_semitom",
        "MudarAltura, 1 semitom acima",
        "14",
        medir_comando(n, &mut a, |a, i| {
            let (c, k) = alvo(i);
            altura_acima(a, c, k)
        }),
    );
    add(
        "altura_oitava",
        "MudarAltura, 1 oitava acima",
        "14",
        medir_comando(n, &mut a, |a, i| {
            let (c, k) = alvo(i);
            ComandoDeEdicao::MudarAltura {
                nota: nota(a, c, k),
                unidade: UnidadeDeAltura::Oitava,
                sentido: SentidoDaAltura::Acima,
            }
        }),
    );
    add(
        "duracao_figura",
        "MudarDuracao, semínima para colcheia",
        "16",
        medir_comando(n, &mut a, |a, i| {
            let (c, k) = alvo(i);
            ComandoDeEdicao::MudarDuracao {
                nota: nota(a, c, k),
                figura: Figura::Colcheia,
            }
        }),
    );
    // Ponto: a nota vira colcheia antes (fora da medição, sobra uma pausa) e ganha o ponto
    // (medido), que consome parte da pausa; semínima pontuada não caberia no compasso.
    add(
        "duracao_ponto",
        "AlternarPonto numa colcheia (o ponto consome parte da pausa que sobrou)",
        "16",
        medir_trecho(n, |i| {
            let (c, k) = alvo(i);
            let id = nota(&a, c, k);
            preparar(
                &mut a,
                ComandoDeEdicao::MudarDuracao {
                    nota: id,
                    figura: Figura::Colcheia,
                },
            );
            let t = Instant::now();
            let r = a.aplicar(ComandoDeEdicao::AlternarPonto { nota: id });
            let dt = ms(t);
            r.expect("ponto do benchmark é válido");
            a.desfazer();
            a.desfazer();
            dt
        }),
    );
    add(
        "remover_nota",
        "RemoverNota (a nota vira pausa)",
        "17",
        medir_comando(n, &mut a, |a, i| {
            let (c, k) = alvo(i);
            ComandoDeEdicao::RemoverNota {
                nota: nota(a, c, k),
            }
        }),
    );
    // Inserir: a nota é removida (fora da medição) e reinserida (medido) pelo inverso exato.
    add(
        "inserir_nota",
        "InserirNota numa pausa de semínima (reinserção após remover)",
        "17",
        medir_trecho(n, |i| {
            let (c, k) = alvo(i);
            let id = nota(&a, c, k);
            let r = preparar(&mut a, ComandoDeEdicao::RemoverNota { nota: id });
            let inserir = r.inverso;
            assert!(
                matches!(inserir, ComandoDeEdicao::InserirNota { .. }),
                "o inverso de remover uma nota solta é inserir"
            );
            let t = Instant::now();
            let ri = a.aplicar(inserir);
            let dt = ms(t);
            ri.expect("inserir do benchmark é válido");
            a.desfazer();
            a.desfazer();
            dt
        }),
    );
    add(
        "acorde_acrescentar_letra",
        "AcrescentarLetra (Shift+letra): acrescenta a terça ao acorde",
        "18",
        medir_comando(n, &mut a, |a, i| {
            let (c, k) = alvo(i);
            ComandoDeEdicao::AcrescentarLetra {
                nota: nota(a, c, k),
                letra: 'E',
            }
        }),
    );
    add(
        "acorde_acrescentar_intervalo",
        "AcrescentarIntervalo (Alt+número): acrescenta a quinta (5 graus)",
        "18",
        medir_comando(n, &mut a, |a, i| {
            let (c, k) = alvo(i);
            ComandoDeEdicao::AcrescentarIntervalo {
                nota: nota(a, c, k),
                graus: 5,
            }
        }),
    );
    // Retirar altura: monta o acorde (fora da medição) e retira a nota original (medido).
    add(
        "acorde_retirar_altura",
        "RetirarAltura de um acorde de duas alturas",
        "18",
        medir_trecho(n, |i| {
            let (c, k) = alvo(i);
            let id = nota(&a, c, k);
            preparar(
                &mut a,
                ComandoDeEdicao::AcrescentarLetra {
                    nota: id,
                    letra: 'E',
                },
            );
            let t = Instant::now();
            let r = a.aplicar(ComandoDeEdicao::RetirarAltura { nota: id });
            let dt = ms(t);
            r.expect("retirar altura do benchmark é válido");
            a.desfazer();
            a.desfazer();
            dt
        }),
    );

    // --- Desfazer / refazer (de uma mudança de altura) ---------------------------------------
    {
        let (mut des, mut refs) = (Vec::new(), Vec::new());
        for i in 0..(n + DESCARTE) {
            let (c, k) = alvo(i);
            let cmd = altura_acima(&a, c, k);
            preparar(&mut a, cmd);
            let t = Instant::now();
            black_box(a.desfazer());
            let d = ms(t);
            let t = Instant::now();
            black_box(a.refazer());
            let r = ms(t);
            a.desfazer();
            if i >= DESCARTE {
                des.push(d);
                refs.push(r);
            }
        }
        add("desfazer", "desfazer uma mudança de altura", "15", des);
        add("refazer", "refazer uma mudança de altura", "15", refs);
    }

    // --- Exportar -------------------------------------------------------------------------
    add(
        "exportar_sem_edicao",
        "exportar sem compasso sujo (cópia byte a byte)",
        "11",
        medir(n, || a.exportar().expect("exporta")),
    );
    let cmd = altura_acima(&a, 50, 0);
    preparar(&mut a, cmd);
    add(
        "exportar_1_compasso_sujo",
        "exportar com 1 compasso sujo (regenera só ele)",
        "11",
        medir(n, || a.exportar().expect("exporta")),
    );
    add(
        "exportar_e_conferir",
        "exportar + conferir_exportacao (o caminho do salvar na tela, sem disco)",
        "11",
        medir(n, || {
            let t = a.exportar().expect("exporta");
            a.conferir_exportacao(&t).expect("confere");
        }),
    );
    let editado = a.exportar().expect("exporta");
    drop(a);

    // --- Layout + exibição, hit test ---------------------------------------------------------
    let doc = DocumentoPartitura::de_texto("cem.musicxml".into(), xml.clone()).expect("abre");
    let (p, ids, ex) = (doc.partitura(), doc.ids(), doc.exibicao());
    add(
        "layout_sem_ids",
        "calcular_layout (sistemas quebrados, sem identidade de nota)",
        "06",
        medir(n, || calcular_layout(p, LARGURA_SP)),
    );
    add(
        "layout_com_ids",
        "calcular_layout_com_ids",
        "06",
        medir(n, || calcular_layout_com_ids(p, ids, LARGURA_SP)),
    );
    add(
        "layout_com_exibicao",
        "calcular_layout_com_exibicao (ids + letra, cifra e dedilhado)",
        "19",
        medir(n, || calcular_layout_com_exibicao(p, ids, ex, LARGURA_SP)),
    );
    add(
        "layout_fita",
        "calcular_layout_fita (sistema único)",
        "06",
        medir(n, || calcular_layout_fita(p)),
    );
    let lay = calcular_layout_com_exibicao(p, ids, ex, LARGURA_SP);
    let pontos: Vec<(IdNota, Ponto)> = (0..COMPASSOS_LONGA)
        .map(|c| {
            let id = ids.id(0, c, c % 4).expect("id");
            let (_, pt) = lay.cabeca_da_nota(id).expect("cabeça no layout");
            (id, pt)
        })
        .collect();
    let mut j = 0usize;
    add(
        "hit_test_nota",
        "Layout::nota_em sobre a cabeça de uma nota (100 posições em rodízio)",
        "06",
        medir(n * 10, || {
            j = (j + 1) % pontos.len();
            lay.nota_em(pontos[j].1, 2.0)
        }),
    );
    let mut j = 0usize;
    add(
        "hit_test_acertar",
        "Layout::acertar (nota, senão compasso) sobre uma cabeça",
        "06",
        medir(n * 10, || {
            j = (j + 1) % pontos.len();
            lay.acertar(pontos[j].1, 2.0)
        }),
    );

    // --- Versões: salvar, diff, conferência de prática ---------------------------------------
    let pasta = tempfile::tempdir().expect("pasta temporária");
    let partituras = pasta.path().join("oficina").join("partituras");
    std::fs::create_dir_all(&partituras).expect("cria");
    std::fs::write(partituras.join("cem.musicxml"), &xml).expect("hipótese");
    let v = Versoes::em(&partituras, "cem", None);
    v.registrar_origem("Baseline", EM).expect("origem");
    // Conteúdo distinto a cada salvar: a nota 0 de um compasso diferente sobe um semitom.
    let mut base = DocumentoAberto::abrir_texto("cem.musicxml", &xml).expect("abre");
    let mut ultimo = String::new();
    add(
        "salvar_versao",
        "Versoes::salvar (objeto + índice; disco do diretório temporário desta máquina)",
        "21",
        medir_trecho(n_pesado, |i| {
            let cmd = altura_acima(&base, i % COMPASSOS_LONGA, 0);
            preparar(&mut base, cmd);
            let texto = base.exportar().expect("exporta");
            let t = Instant::now();
            let r = v.salvar(None, &texto, Some("baseline"), "Baseline", EM);
            let dt = ms(t);
            match r {
                Ok(ResultadoDoSalvar::Criada(ver)) => ultimo = ver.hash,
                outro => panic!("salvar do benchmark: {outro:?}"),
            }
            dt
        }),
    );

    // Diff: duas Versões da cadeia, afastadas (origem e ponta).
    let lista = v.listar().expect("lista");
    let h0 = lista.first().expect("origem").hash.clone();
    let h1 = lista.last().expect("ponta").hash.clone();
    assert_ne!(h0, h1);
    add(
        "diff_entre_versoes",
        "diff_entre_versoes: abre as 2 Versões (leitura + hash) e compara (origem x ponta)",
        "22",
        medir(n_pesado, || diff_entre_versoes(&v, &h0, &h1).expect("diff")),
    );
    let a0 = DocumentoAberto::abrir_texto("a", &xml).expect("abre");
    let a1 = DocumentoAberto::abrir_texto("b", &editado).expect("abre");
    add(
        "diff_de_documentos",
        "diff_de_documentos puro (núcleo, sem disco; 1 compasso diferente)",
        "22",
        medir(n, || diff_de_documentos(a0.documento(), a1.documento())),
    );

    // Conferência da prática de Versão (ticket 25): a tela a chama no máximo 1 vez por segundo.
    v.aprovar(&ultimo, Some("baseline"), "Baseline", EM)
        .expect("aprova");
    let mut estado = EstadoPartitura::novo(None);
    let pratica = PraticaDaVersao::abrir(&v, &ultimo, false, &mut estado).expect("abre a Versão");
    add(
        "conferencia_pratica_versao",
        "PraticaDaVersao::conferir (lê o arquivo, SHA-256, estado de aprovação)",
        "25",
        medir(n_pesado, || pratica.conferir(&v, &estado).expect("confere")),
    );

    (
        m,
        format!("n={n}, n_pesado={n_pesado}, descarte={DESCARTE}"),
    )
}

// --- Máquina e saída ------------------------------------------------------------------------

fn comando_texto(cmd: &str, args: &[&str]) -> String {
    std::process::Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_else(|| "desconhecido".into())
}

fn cpu() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|t| {
            t.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_owned())
        })
        .unwrap_or_else(|| "desconhecido".into())
}

fn so() -> String {
    let os = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|t| {
            t.lines()
                .find_map(|l| l.strip_prefix("PRETTY_NAME="))
                .map(|s| s.trim_matches('"').to_owned())
        })
        .unwrap_or_else(|| std::env::consts::OS.into());
    format!("{os}; kernel {}", comando_texto("uname", &["-r"]))
}

fn json_texto(s: &str) -> String {
    let mut o = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn escrever(pasta: &Path, medidas: &[Medida], parametros: &str) {
    std::fs::create_dir_all(pasta).expect("pasta de saída");
    let perfil = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let carga = std::fs::read_to_string("/proc/loadavg")
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|_| "desconhecido".into());
    let mut j = String::from("{\n");
    let _ = writeln!(j, "  \"ticket\": \"26\",");
    let _ = writeln!(
        j,
        "  \"peca\": \"gerar_cem_compassos() (piano-tutor-fixtures), 100 compassos 4/4, 400 notas\","
    );
    let _ = writeln!(j, "  \"perfil\": {},", json_texto(perfil));
    let _ = writeln!(
        j,
        "  \"codigo\": {},",
        json_texto(&comando_texto("git", &["rev-parse", "--short", "HEAD"]))
    );
    let _ = writeln!(
        j,
        "  \"data\": {},",
        json_texto(&comando_texto("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"]))
    );
    let _ = writeln!(j, "  \"cpu\": {},", json_texto(&cpu()));
    let _ = writeln!(
        j,
        "  \"nucleos_logicos\": {},",
        std::thread::available_parallelism().map_or(0, usize::from)
    );
    let _ = writeln!(j, "  \"so\": {},", json_texto(&so()));
    let _ = writeln!(
        j,
        "  \"carga_media_ao_gravar (1, 5, 15 min)\": {},",
        json_texto(&carga)
    );
    let _ = writeln!(j, "  \"parametros\": {},", json_texto(parametros));
    let _ = writeln!(j, "  \"meta_ms_a_calibrar\": {META_MS},");
    let _ = writeln!(j, "  \"comandos\": [");
    let mut csv =
        String::from("id,ticket,n,min_ms,mediana_ms,media_ms,p95_ms,max_ms,max_acima_de_100ms\n");
    for (i, md) in medidas.iter().enumerate() {
        let e = estatistica(&md.amostras_ms);
        let acima = e.max > META_MS;
        let amostras = md
            .amostras_ms
            .iter()
            .map(|x| format!("{x:.6}"))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = write!(
            j,
            "    {{\"id\": {}, \"ticket\": {}, \"descricao\": {}, \"n\": {}, \"min_ms\": {:.6}, \"mediana_ms\": {:.6}, \"media_ms\": {:.6}, \"p95_ms\": {:.6}, \"max_ms\": {:.6}, \"max_acima_de_100ms\": {}, \"amostras_ms\": [{}]}}",
            json_texto(md.id),
            json_texto(md.ticket),
            json_texto(md.descricao),
            e.n,
            e.min,
            e.mediana,
            e.media,
            e.p95,
            e.max,
            acima,
            amostras
        );
        j.push_str(if i + 1 < medidas.len() { ",\n" } else { "\n" });
        let _ = writeln!(
            csv,
            "{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{}",
            md.id, md.ticket, e.n, e.min, e.mediana, e.media, e.p95, e.max, acima
        );
    }
    j.push_str("  ]\n}\n");
    std::fs::write(pasta.join(format!("dados-{perfil}.json")), j).expect("grava json");
    std::fs::write(pasta.join(format!("dados-{perfil}.csv")), csv).expect("grava csv");
}

fn imprimir(medidas: &[Medida]) {
    println!(
        "{:<30} {:>5} {:>10} {:>10} {:>10}  (ms)",
        "comando", "n", "mediana", "p95", "max"
    );
    for md in medidas {
        let e = estatistica(&md.amostras_ms);
        println!(
            "{:<30} {:>5} {:>10.4} {:>10.4} {:>10.4}",
            md.id, e.n, e.mediana, e.p95, e.max
        );
    }
}

fn n_do_ambiente() -> usize {
    std::env::var("PT_BASELINE_N")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|n| *n >= 1)
        .unwrap_or(200)
}

/// Fumaça: 1 execução por comando, só para o harness não apodrecer. Sem asserção de tempo.
#[test]
fn fumaca_do_baseline() {
    let (medidas, _) = rodar(1);
    assert!(medidas.len() >= 20, "todos os comandos medidos");
    assert!(medidas.iter().all(|m| !m.amostras_ms.is_empty()));
}

/// O benchmark de verdade (use `--release`). Sem asserção de meta.
#[test]
#[ignore = "benchmark: rode com --release e --ignored (ticket 26)"]
fn baseline_de_desempenho() {
    let (medidas, parametros) = rodar(n_do_ambiente());
    imprimir(&medidas);
    if let Ok(pasta) = std::env::var("PT_BASELINE_SAIDA") {
        escrever(Path::new(&pasta), &medidas, &parametros);
    }
}
