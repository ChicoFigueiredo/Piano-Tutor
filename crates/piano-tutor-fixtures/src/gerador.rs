//! Geradores versionados: peça de 100 compassos e `.mxl` (válidos e maliciosos).
//!
//! O repositório ignora `*.mxl` (`.gitignore`), então estes arquivos nunca são versionados:
//! são gerados por este código, de forma determinística, em memória ou numa pasta de teste.
//! Nenhum contém payload perigoso real: o "zip bomb" é só espaço em branco repetido e os
//! caminhos fora do escopo são nomes de entrada que o leitor deve recusar, nunca gravados.

use piano_tutor_core::mxl::empacotar_mxl;
use std::io::{Cursor, Write};
use std::path::Path;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

/// Número de compassos da peça longa.
pub const COMPASSOS_LONGA: usize = 100;

/// Peça de 100 compassos em 4/4, Dó maior, uma pauta, 4 semínimas por compasso: escala
/// diatônica subindo e descendo entre C4 e C6. Totalmente sintética (CC0, sem obra de terceiros).
pub fn gerar_cem_compassos() -> String {
    const ALTURAS: [(&str, u8); 8] = [
        ("C", 0),
        ("D", 0),
        ("E", 0),
        ("F", 0),
        ("G", 0),
        ("A", 0),
        ("B", 0),
        ("C", 1),
    ];
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<score-partwise version=\"4.0\">\n  \
         <work><work-title>Cem compassos (sintética)</work-title></work>\n  \
         <identification><rights>CC0 1.0: peça sintética gerada por gerar_cem_compassos() \
         (piano-tutor-fixtures); sem material de terceiros</rights></identification>\n  \
         <part-list><score-part id=\"P1\"><part-name>Piano</part-name></score-part></part-list>\n  \
         <part id=\"P1\">\n",
    );
    for m in 0..COMPASSOS_LONGA {
        s.push_str(&format!("    <measure number=\"{}\">", m + 1));
        if m == 0 {
            s.push_str(
                "<attributes><divisions>1</divisions><key><fifths>0</fifths></key>\
                 <time><beats>4</beats><beat-type>4</beat-type></time>\
                 <clef><sign>G</sign><line>2</line></clef></attributes>",
            );
        }
        s.push('\n');
        for t in 0..4 {
            let k = (m * 4 + t) % 14;
            // 0..7 sobe, 7..14 desce.
            let i = if k < 8 { k } else { 14 - k };
            let (passo, extra) = ALTURAS[i];
            let oitava = 5 + usize::from(extra) - usize::from(m % 2 == 1);
            s.push_str(&format!(
                "      <note><pitch><step>{passo}</step><octave>{oitava}</octave></pitch>\
                 <duration>1</duration><voice>1</voice><type>quarter</type></note>\n"
            ));
        }
        s.push_str("    </measure>\n");
    }
    s.push_str("  </part>\n</score-partwise>\n");
    s
}

/// Um `.mxl` de teste e o que o leitor deve fazer com ele.
#[derive(Debug, Clone)]
pub struct MxlDeTeste {
    pub nome: &'static str,
    pub bytes: Vec<u8>,
    /// `true` = o leitor deve abrir; `false` = deve recusar com erro (sem pânico).
    pub valido: bool,
    pub descricao: &'static str,
}

const MINIMO: &str = "<?xml version=\"1.0\"?>\n<score-partwise version=\"4.0\"><part-list>\
<score-part id=\"P1\"><part-name>P</part-name></score-part></part-list><part id=\"P1\">\
<measure number=\"1\"><attributes><divisions>1</divisions><time><beats>4</beats>\
<beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes>\
<note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type>\
</note></measure></part></score-partwise>";

fn container(rootfile: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<container><rootfiles>\
         <rootfile full-path=\"{rootfile}\" media-type=\"application/vnd.recordare.musicxml+xml\"/>\
         </rootfiles></container>"
    )
}

/// Zip em memória: (nome, dados). Carimbo fixo, saída determinística.
fn zip_de(entradas: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    let opcoes = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (nome, dados) in entradas {
        // Os geradores só escrevem em memória; falha aqui é erro de programação do teste.
        z.start_file(nome.as_str(), opcoes)
            .expect("zip em memória: início de entrada");
        z.write_all(dados).expect("zip em memória: escrita");
    }
    z.finish().expect("zip em memória: fechamento").into_inner()
}

/// Todos os `.mxl` de teste: um válido (a peça de 100 compassos) e os maliciosos.
pub fn mxl_de_teste() -> Vec<MxlDeTeste> {
    let longa = gerar_cem_compassos();
    let valido = empacotar_mxl("score.xml", &longa).expect("empacota a peça longa");

    // Whitespace repetido: comprime a milhares de vezes, estoura o teto de 8 MiB descompactado.
    let bomba = {
        let corpo = vec![b' '; 24 * 1024 * 1024];
        zip_de(&[
            (
                "META-INF/container.xml".into(),
                container("score.xml").into_bytes(),
            ),
            ("score.xml".into(), corpo),
        ])
    };
    let fora_entrada = zip_de(&[
        (
            "META-INF/container.xml".into(),
            container("score.xml").into_bytes(),
        ),
        ("score.xml".into(), MINIMO.as_bytes().to_vec()),
        ("../../fora-do-escopo.txt".into(), b"nunca gravado".to_vec()),
    ]);
    let fora_rootfile = zip_de(&[
        (
            "META-INF/container.xml".into(),
            container("../fora.xml").into_bytes(),
        ),
        ("score.xml".into(), MINIMO.as_bytes().to_vec()),
    ]);
    let absoluto = zip_de(&[
        (
            "META-INF/container.xml".into(),
            container("/etc/score.xml").into_bytes(),
        ),
        ("/etc/score.xml".into(), MINIMO.as_bytes().to_vec()),
    ]);
    let entradas_demais = {
        let mut e = vec![
            (
                "META-INF/container.xml".to_string(),
                container("score.xml").into_bytes(),
            ),
            ("score.xml".to_string(), MINIMO.as_bytes().to_vec()),
        ];
        for i in 0..200 {
            e.push((format!("extra/{i}.txt"), b"x".to_vec()));
        }
        zip_de(&e)
    };
    let sem_container = zip_de(&[("score.xml".into(), MINIMO.as_bytes().to_vec())]);
    vec![
        MxlDeTeste {
            nome: "valido-cem-compassos.mxl",
            bytes: valido,
            valido: true,
            descricao: "MXL válido com a peça sintética de 100 compassos",
        },
        MxlDeTeste {
            nome: "malicioso-zip-bomb.mxl",
            bytes: bomba,
            valido: false,
            descricao: "24 MiB de espaços comprimidos (taxa e teto de descompactação)",
        },
        MxlDeTeste {
            nome: "malicioso-entrada-fora-do-escopo.mxl",
            bytes: fora_entrada,
            valido: false,
            descricao: "entrada com nome ../../ (path traversal)",
        },
        MxlDeTeste {
            nome: "malicioso-rootfile-fora-do-escopo.mxl",
            bytes: fora_rootfile,
            valido: false,
            descricao: "container.xml aponta um rootfile com ..",
        },
        MxlDeTeste {
            nome: "malicioso-caminho-absoluto.mxl",
            bytes: absoluto,
            valido: false,
            descricao: "rootfile e entrada com caminho absoluto",
        },
        MxlDeTeste {
            nome: "malicioso-entradas-demais.mxl",
            bytes: entradas_demais,
            valido: false,
            descricao: "202 entradas (acima do limite de 128)",
        },
        MxlDeTeste {
            nome: "malicioso-sem-container.mxl",
            bytes: sem_container,
            valido: false,
            descricao: "zip sem META-INF/container.xml",
        },
    ]
}

/// Grava todos os `.mxl` de teste em `pasta` (que deve existir). Útil para abrir à mão.
pub fn gravar_mxl_de_teste(pasta: &Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut caminhos = Vec::new();
    for m in mxl_de_teste() {
        let c = pasta.join(m.nome);
        std::fs::write(&c, &m.bytes)?;
        caminhos.push(c);
    }
    Ok(caminhos)
}
