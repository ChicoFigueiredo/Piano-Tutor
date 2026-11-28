//! Leitor (e escritor mínimo) de `.mxl`, o MusicXML compactado em zip (F07-1, F07-5).
//!
//! O contêiner traz `META-INF/container.xml`, que aponta o arquivo principal (`rootfile`); esse
//! arquivo é entregue ao leitor de [`Partitura::de_musicxml`]. Nada é extraído para o disco.
//!
//! Defesas (todas com teste):
//! - o arquivo inteiro tem [`MXL_TAMANHO_MAXIMO_BYTES`] no máximo;
//! - no máximo [`MXL_MAX_ENTRADAS`] entradas;
//! - nomes de entrada com `..`, caminho absoluto, `\\`, unidade (`C:`) ou NUL recusam o arquivo;
//! - entrada criptografada ou com compressão que não seja armazenada/deflate é recusada;
//! - tamanhos do cabeçalho do zip **não são confiados**: a leitura é limitada por um leitor
//!   com teto (`take`), de [`MXL_MAX_CONTAINER_BYTES`] para o `container.xml` e de
//!   [`TAMANHO_MAXIMO_BYTES`] para a partitura, e a taxa descompactado/compactado é limitada
//!   por [`MXL_TAXA_MAXIMA`];
//! - o `container.xml` é lido sem DTD e com limite de nós;
//! - as mensagens de erro são em português e não reproduzem conteúdo do arquivo além de nomes
//!   truncados em [`MAX_NOME_EM_MENSAGEM`](crate::xml_seguro::MAX_NOME_EM_MENSAGEM) caracteres;
//!   nenhum caminho do disco do usuário entra aqui (este módulo só recebe bytes).
//!
//! Só é descompactado o `container.xml` e o arquivo principal; as demais entradas (imagens,
//! PDFs) nunca são lidas.

use crate::partitura::{ErroPartitura, Partitura, TAMANHO_MAXIMO_BYTES};
use crate::xml_seguro::{mensagem, nome_para_mensagem};
use roxmltree::{Document, ParsingOptions};
use std::io::{Cursor, Read, Write};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Tamanho máximo do `.mxl` inteiro (compactado).
pub const MXL_TAMANHO_MAXIMO_BYTES: usize = 16 * 1024 * 1024;
/// Número máximo de entradas no zip.
pub const MXL_MAX_ENTRADAS: usize = 128;
/// Tamanho máximo do `META-INF/container.xml` descompactado.
pub const MXL_MAX_CONTAINER_BYTES: usize = 64 * 1024;
/// Razão máxima descompactado/compactado de uma entrada (com piso de 512 bytes compactados).
pub const MXL_TAXA_MAXIMA: u64 = 200;
/// Caminho padrão do contêiner dentro do zip.
pub const CAMINHO_CONTAINER: &str = "META-INF/container.xml";

fn recusa(motivo: impl Into<String>) -> ErroPartitura {
    ErroPartitura::ContainerInvalido(motivo.into())
}

/// Confere um nome de entrada ou caminho de `rootfile`: relativo, sem `..`, sem barra invertida.
fn nome_seguro(nome: &str) -> Result<(), ErroPartitura> {
    if nome.is_empty() {
        return Err(recusa("nome de entrada vazio"));
    }
    if nome.contains('\0') || nome.contains('\\') {
        return Err(recusa(format!(
            "nome de entrada inseguro: {:?}",
            nome_para_mensagem(nome)
        )));
    }
    if nome.starts_with('/') || nome.as_bytes().get(1) == Some(&b':') {
        return Err(recusa(format!(
            "caminho absoluto recusado: {:?}",
            nome_para_mensagem(nome)
        )));
    }
    if nome.split('/').any(|c| c == "..") {
        return Err(recusa(format!(
            "caminho com '..' recusado: {:?}",
            nome_para_mensagem(nome)
        )));
    }
    Ok(())
}

/// Lê uma entrada do zip com teto de `limite` bytes, sem confiar nos tamanhos do cabeçalho.
fn ler_entrada(
    zip: &mut ZipArchive<Cursor<&[u8]>>,
    indice: usize,
    limite: usize,
    rotulo: &'static str,
) -> Result<Vec<u8>, ErroPartitura> {
    {
        let bruta = zip
            .by_index_raw(indice)
            .map_err(|_| recusa("entrada ilegível"))?;
        if bruta.encrypted() {
            return Err(recusa("entrada criptografada não é suportada"));
        }
        if !matches!(
            bruta.compression(),
            CompressionMethod::Stored | CompressionMethod::Deflated
        ) {
            return Err(recusa(
                "método de compressão não suportado (só armazenado e deflate)",
            ));
        }
        if bruta.is_dir() {
            return Err(recusa(format!("{rotulo} é um diretório")));
        }
        if bruta.size() > limite as u64 {
            return Err(ErroPartitura::LimiteExcedido(
                "tamanho descompactado declarado no zip",
            ));
        }
    }
    let compactado = {
        let bruta = zip
            .by_index_raw(indice)
            .map_err(|_| recusa("entrada ilegível"))?;
        bruta.compressed_size()
    };
    let entrada = zip
        .by_index(indice)
        .map_err(|_| recusa(format!("não foi possível abrir {rotulo}")))?;
    let mut dados = Vec::new();
    entrada
        .take(limite as u64 + 1)
        .read_to_end(&mut dados)
        .map_err(|_| recusa(format!("dados corrompidos ao descompactar {rotulo}")))?;
    if dados.len() > limite {
        return Err(ErroPartitura::LimiteExcedido(
            "bytes descompactados (zip-bomb?)",
        ));
    }
    if dados.len() as u64 > MXL_TAXA_MAXIMA * compactado.max(512) {
        return Err(ErroPartitura::LimiteExcedido(
            "taxa de compressão suspeita (zip-bomb?)",
        ));
    }
    Ok(dados)
}

/// Texto UTF-8 de um arquivo XML (aceita BOM UTF-8; recusa UTF-16 e outras codificações).
fn texto_utf8(bytes: Vec<u8>, rotulo: &str) -> Result<String, ErroPartitura> {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return Err(recusa(format!("{rotulo} em UTF-16 não é suportado")));
    }
    let sem_bom = bytes
        .strip_prefix(&[0xEF, 0xBB, 0xBF][..])
        .unwrap_or(&bytes);
    std::str::from_utf8(sem_bom)
        .map(str::to_owned)
        .map_err(|_| recusa(format!("{rotulo} não é UTF-8 válido")))
}

/// Caminho do `rootfile` principal indicado por um `container.xml`.
pub fn rootfile_do_container(xml: &str) -> Result<String, ErroPartitura> {
    let opcoes = ParsingOptions {
        allow_dtd: false,
        nodes_limit: 10_000,
        ..Default::default()
    };
    let doc = Document::parse_with_options(xml, opcoes)
        .map_err(|e| recusa(format!("container.xml inválido: {}", mensagem(&e))))?;
    let raiz = doc.root_element();
    if raiz.tag_name().name() != "container" {
        return Err(recusa("container.xml sem elemento 'container'"));
    }
    let candidatos: Vec<_> = raiz
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "rootfile")
        .filter_map(|n| {
            n.attribute("full-path")
                .map(|c| (c.to_owned(), n.attribute("media-type").map(str::to_owned)))
        })
        .collect();
    // Prefere um rootfile cujo media-type seja MusicXML; senão, o primeiro.
    let escolhido = candidatos
        .iter()
        .find(|(_, m)| m.as_deref().is_some_and(|m| m.contains("musicxml")))
        .or_else(|| candidatos.first())
        .ok_or_else(|| recusa("container.xml sem 'rootfile' com 'full-path'"))?;
    nome_seguro(&escolhido.0)?;
    Ok(escolhido.0.clone())
}

/// Extrai o texto MusicXML de dentro de um `.mxl`, com todas as defesas descritas no módulo.
pub fn extrair_musicxml(bytes: &[u8]) -> Result<String, ErroPartitura> {
    if bytes.len() > MXL_TAMANHO_MAXIMO_BYTES {
        return Err(ErroPartitura::GrandeDemais(bytes.len()));
    }
    let mut zip = ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| recusa("não é um zip válido (diretório central ausente ou corrompido)"))?;
    if zip.len() > MXL_MAX_ENTRADAS {
        return Err(ErroPartitura::LimiteExcedido("número de entradas do zip"));
    }
    let mut nomes = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let nome = zip
            .name_for_index(i)
            .ok_or_else(|| recusa("entrada sem nome"))?
            .map_err(|_| recusa("nome de entrada ilegível"))?
            .into_owned();
        nome_seguro(&nome)?;
        nomes.push(nome);
    }
    let i_container = nomes
        .iter()
        .position(|n| n == CAMINHO_CONTAINER)
        .ok_or_else(|| recusa("falta META-INF/container.xml"))?;
    let bruto = ler_entrada(
        &mut zip,
        i_container,
        MXL_MAX_CONTAINER_BYTES,
        "container.xml",
    )?;
    let container = texto_utf8(bruto, "container.xml")?;
    let principal = rootfile_do_container(&container)?;
    let i_principal = nomes.iter().position(|n| *n == principal).ok_or_else(|| {
        recusa(format!(
            "o rootfile '{}' não existe no zip",
            nome_para_mensagem(&principal)
        ))
    })?;
    let bruto = ler_entrada(&mut zip, i_principal, TAMANHO_MAXIMO_BYTES, "partitura")?;
    texto_utf8(bruto, "partitura")
}

impl Partitura {
    /// Lê uma partitura de um `.mxl` (zip com `META-INF/container.xml`).
    pub fn de_mxl(bytes: &[u8]) -> Result<Self, ErroPartitura> {
        Self::de_musicxml(&extrair_musicxml(bytes)?)
    }
}

/// Monta um `.mxl` mínimo (container.xml + um arquivo MusicXML, ambos com deflate) em memória.
/// O carimbo de data do zip é fixo, então a saída é determinística.
pub fn empacotar_mxl(nome_interno: &str, musicxml: &str) -> Result<Vec<u8>, ErroPartitura> {
    nome_seguro(nome_interno)?;
    let container = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<container>\n  <rootfiles>\n    \
         <rootfile full-path=\"{nome_interno}\" media-type=\"application/vnd.recordare.musicxml+xml\"/>\n  \
         </rootfiles>\n</container>\n"
    );
    let opcoes = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let mut grava = |nome: &str, dados: &[u8]| -> Result<(), ErroPartitura> {
        zip.start_file(nome, opcoes)
            .map_err(|e| recusa(format!("falha ao escrever zip: {e}")))?;
        zip.write_all(dados)
            .map_err(|e| recusa(format!("falha ao escrever zip: {e}")))
    };
    grava(CAMINHO_CONTAINER, container.as_bytes())?;
    grava(nome_interno, musicxml.as_bytes())?;
    zip.finish()
        .map(Cursor::into_inner)
        .map_err(|e| recusa(format!("falha ao fechar zip: {e}")))
}

#[cfg(test)]
mod testes {
    use super::*;

    const WIEGENLIED: &str =
        include_str!("../../../assets/partituras/brahms-wiegenlied-op49-4.musicxml");

    const CONTAINER_OK: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container><rootfiles><rootfile full-path="score.xml" media-type="application/vnd.recordare.musicxml+xml"/></rootfiles></container>"#;

    const MINIMA: &str = r#"<?xml version="1.0"?>
<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list>
<part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes>
<note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note></measure></part></score-partwise>"#;

    /// Zip em memória: (nome, dados, deflate?).
    fn zip_de(entradas: &[(&str, &[u8], bool)]) -> Vec<u8> {
        let mut z = ZipWriter::new(Cursor::new(Vec::new()));
        for (nome, dados, deflate) in entradas {
            let met = if *deflate {
                CompressionMethod::Deflated
            } else {
                CompressionMethod::Stored
            };
            z.start_file(*nome, SimpleFileOptions::default().compression_method(met))
                .expect("inicia");
            z.write_all(dados).expect("grava");
        }
        z.finish().expect("fecha").into_inner()
    }

    fn mxl_ok() -> Vec<u8> {
        zip_de(&[
            (CAMINHO_CONTAINER, CONTAINER_OK.as_bytes(), true),
            ("score.xml", MINIMA.as_bytes(), true),
        ])
    }

    /// Posições de todas as ocorrências de uma assinatura de 4 bytes.
    fn acha(dados: &[u8], assinatura: [u8; 4]) -> Vec<usize> {
        dados
            .windows(4)
            .enumerate()
            .filter(|(_, w)| *w == assinatura)
            .map(|(i, _)| i)
            .collect()
    }

    fn eh_container(r: Result<Partitura, ErroPartitura>) -> String {
        match r {
            Err(ErroPartitura::ContainerInvalido(m)) => m,
            outro => panic!("esperava ContainerInvalido, veio {outro:?}"),
        }
    }

    #[test]
    fn le_mxl_minimo_igual_ao_xml() {
        let via_mxl = Partitura::de_mxl(&mxl_ok()).expect("lê mxl");
        let direto = Partitura::de_musicxml(MINIMA).expect("lê xml");
        assert_eq!(via_mxl.total_de_notas(), direto.total_de_notas());
        assert_eq!(
            via_mxl.compassos_na_execucao(),
            direto.compassos_na_execucao()
        );
        assert_eq!(via_mxl.total_de_notas(), 1);
    }

    #[test]
    fn empacotar_e_ler_devolve_a_mesma_partitura() {
        let mxl = empacotar_mxl("brahms.musicxml", WIEGENLIED).expect("empacota");
        assert!(mxl.len() < WIEGENLIED.len(), "o zip comprime");
        assert_eq!(extrair_musicxml(&mxl).expect("extrai"), WIEGENLIED);
        let a = Partitura::de_mxl(&mxl).expect("lê");
        let b = Partitura::de_musicxml(WIEGENLIED).expect("lê xml");
        assert_eq!(a.total_de_notas(), b.total_de_notas());
        assert_eq!(a.eventos_de_reproducao(), b.eventos_de_reproducao());
        assert_eq!(
            empacotar_mxl("brahms.musicxml", WIEGENLIED).expect("de novo"),
            mxl
        );
    }

    #[test]
    fn rootfile_vem_do_container_e_pode_estar_em_subpasta() {
        let container = r#"<container><rootfiles>
<rootfile full-path="docs/capa.pdf" media-type="application/pdf"/>
<rootfile full-path="musica/dentro.xml" media-type="application/vnd.recordare.musicxml+xml"/>
</rootfiles></container>"#;
        let z = zip_de(&[
            ("musica/lixo.xml", b"<outra-coisa/>", true),
            (CAMINHO_CONTAINER, container.as_bytes(), true),
            ("musica/dentro.xml", MINIMA.as_bytes(), true),
        ]);
        assert_eq!(Partitura::de_mxl(&z).expect("lê").total_de_notas(), 1);
        // Sem media-type usa o primeiro rootfile.
        let c2 = r#"<container><rootfiles><rootfile full-path="a.xml"/></rootfiles></container>"#;
        let z = zip_de(&[
            (CAMINHO_CONTAINER, c2.as_bytes(), false),
            ("a.xml", MINIMA.as_bytes(), false),
        ]);
        assert!(Partitura::de_mxl(&z).is_ok());
    }

    #[test]
    fn aceita_bom_utf8_na_partitura() {
        let mut com_bom = vec![0xEF, 0xBB, 0xBF];
        com_bom.extend_from_slice(MINIMA.as_bytes());
        let z = zip_de(&[
            (CAMINHO_CONTAINER, CONTAINER_OK.as_bytes(), true),
            ("score.xml", &com_bom, true),
        ]);
        assert!(Partitura::de_mxl(&z).is_ok());
    }

    #[test]
    fn recusa_utf16_e_utf8_invalido() {
        let mut u16b = vec![0xFF, 0xFE];
        u16b.extend_from_slice(b"<\0s\0");
        let z = zip_de(&[
            (CAMINHO_CONTAINER, CONTAINER_OK.as_bytes(), true),
            ("score.xml", &u16b, true),
        ]);
        assert!(eh_container(Partitura::de_mxl(&z)).contains("UTF-16"));
        let z = zip_de(&[
            (CAMINHO_CONTAINER, CONTAINER_OK.as_bytes(), true),
            ("score.xml", &[0xC3, 0x28, 0xA0], true),
        ]);
        assert!(eh_container(Partitura::de_mxl(&z)).contains("UTF-8"));
    }

    #[test]
    fn recusa_lixo_e_zip_truncado() {
        assert!(Partitura::de_mxl(b"").is_err());
        assert!(Partitura::de_mxl(b"isto nao e um zip").is_err());
        let z = mxl_ok();
        assert!(Partitura::de_mxl(&z[..z.len() / 2]).is_err());
        assert!(Partitura::de_mxl(&z[..z.len() - 3]).is_err());
    }

    #[test]
    fn recusa_container_ausente_ou_invalido() {
        // Sem META-INF/container.xml.
        let z = zip_de(&[("score.xml", MINIMA.as_bytes(), true)]);
        assert!(eh_container(Partitura::de_mxl(&z)).contains("container.xml"));
        // container.xml que não é XML.
        let z = zip_de(&[
            (CAMINHO_CONTAINER, b"nao e xml", true),
            ("score.xml", MINIMA.as_bytes(), true),
        ]);
        assert!(eh_container(Partitura::de_mxl(&z)).contains("container.xml"));
        // Raiz errada.
        let z = zip_de(&[
            (CAMINHO_CONTAINER, b"<outro/>", true),
            ("score.xml", MINIMA.as_bytes(), true),
        ]);
        assert!(eh_container(Partitura::de_mxl(&z)).contains("container"));
        // Sem rootfile.
        let z = zip_de(&[
            (
                CAMINHO_CONTAINER,
                b"<container><rootfiles/></container>",
                true,
            ),
            ("score.xml", MINIMA.as_bytes(), true),
        ]);
        assert!(eh_container(Partitura::de_mxl(&z)).contains("rootfile"));
        // Rootfile aponta para entrada que não existe.
        let c =
            r#"<container><rootfiles><rootfile full-path="fantasma.xml"/></rootfiles></container>"#;
        let z = zip_de(&[(CAMINHO_CONTAINER, c.as_bytes(), true)]);
        assert!(eh_container(Partitura::de_mxl(&z)).contains("fantasma.xml"));
    }

    #[test]
    fn container_com_dtd_ou_entidades_e_recusado() {
        let c = r#"<?xml version="1.0"?><!DOCTYPE container [<!ENTITY x "file:///etc/passwd">]>
<container><rootfiles><rootfile full-path="score.xml"/></rootfiles></container>"#;
        let z = zip_de(&[
            (CAMINHO_CONTAINER, c.as_bytes(), true),
            ("score.xml", MINIMA.as_bytes(), true),
        ]);
        assert!(eh_container(Partitura::de_mxl(&z)).contains("container.xml"));
    }

    #[test]
    fn container_grande_demais_e_recusado() {
        let mut c =
            String::from("<container><rootfiles><rootfile full-path=\"score.xml\"/></rootfiles>");
        c.push_str(&"<!-- enchimento -->".repeat(MXL_MAX_CONTAINER_BYTES / 10));
        c.push_str("</container>");
        let z = zip_de(&[
            (CAMINHO_CONTAINER, c.as_bytes(), false),
            ("score.xml", MINIMA.as_bytes(), true),
        ]);
        assert!(matches!(
            Partitura::de_mxl(&z),
            Err(ErroPartitura::LimiteExcedido(_))
        ));
    }

    #[test]
    fn recusa_nomes_com_travessia_de_caminho() {
        let mut mantidos = 0;
        for ruim in [
            "../fora.xml",
            "a/../../fora.xml",
            "/etc/passwd",
            "C:/windows/x.xml",
            "a\\b.xml",
        ] {
            let z = zip_de(&[
                (CAMINHO_CONTAINER, CONTAINER_OK.as_bytes(), true),
                ("score.xml", MINIMA.as_bytes(), true),
                (ruim, b"x", false),
            ]);
            // O gravador pode normalizar o nome; só vale a conferência se ele foi mantido.
            let nomes: Vec<String> = ZipArchive::new(Cursor::new(&z[..]))
                .map(|a| {
                    a.file_names()
                        .filter_map(Result::ok)
                        .map(|n| n.into_owned())
                        .collect()
                })
                .unwrap_or_default();
            if nomes.iter().any(|n| n == ruim) {
                mantidos += 1;
                assert!(
                    eh_container(Partitura::de_mxl(&z)).contains("inseguro")
                        || eh_container(Partitura::de_mxl(&z)).contains("recusado")
                );
                assert!(
                    Partitura::de_mxl(&z).is_err(),
                    "nome {ruim:?} deveria ser recusado"
                );
            }
        }
        assert!(
            mantidos >= 3,
            "o gravador manteve só {mantidos} nomes maliciosos"
        );
        // Verificação direta da função de nomes.
        for ruim in ["../x", "a/../../x", "/x", "C:/x", "a\\b", "", "a\0b"] {
            assert!(nome_seguro(ruim).is_err(), "{ruim:?}");
        }
        for bom in [
            "score.xml",
            "META-INF/container.xml",
            "a/b/c.musicxml",
            "a..b/c",
        ] {
            assert!(nome_seguro(bom).is_ok(), "{bom:?}");
        }
    }

    #[test]
    fn recusa_rootfile_que_sai_do_zip() {
        for caminho in ["../score.xml", "/etc/passwd", "a/../../b.xml"] {
            let c = format!(
                r#"<container><rootfiles><rootfile full-path="{caminho}"/></rootfiles></container>"#
            );
            let z = zip_de(&[
                (CAMINHO_CONTAINER, c.as_bytes(), true),
                ("score.xml", MINIMA.as_bytes(), true),
            ]);
            assert!(
                eh_container(Partitura::de_mxl(&z)).contains("recusado"),
                "{caminho}"
            );
        }
    }

    #[test]
    fn recusa_zip_com_entradas_demais() {
        let nomes: Vec<String> = (0..=MXL_MAX_ENTRADAS)
            .map(|i| format!("f{i}.txt"))
            .collect();
        let mut itens: Vec<(&str, &[u8], bool)> = vec![
            (CAMINHO_CONTAINER, CONTAINER_OK.as_bytes(), true),
            ("score.xml", MINIMA.as_bytes(), true),
        ];
        itens.extend(nomes.iter().map(|n| (n.as_str(), &b""[..], false)));
        let z = zip_de(&itens);
        assert_eq!(
            Partitura::de_mxl(&z),
            Err(ErroPartitura::LimiteExcedido("número de entradas do zip"))
        );
        // No limite exato ainda passa.
        let itens_ok: Vec<(&str, &[u8], bool)> = itens.into_iter().take(MXL_MAX_ENTRADAS).collect();
        assert!(Partitura::de_mxl(&zip_de(&itens_ok)).is_ok());
    }

    #[test]
    fn zip_bomb_por_tamanho_e_recusada_sem_alocar_tudo() {
        // 12 MiB de zeros viram poucos KiB com deflate; o teto é 8 MiB descompactado.
        let zeros = vec![0u8; 12 * 1024 * 1024];
        let z = zip_de(&[
            (CAMINHO_CONTAINER, CONTAINER_OK.as_bytes(), true),
            ("score.xml", &zeros, true),
        ]);
        assert!(
            z.len() < 100 * 1024,
            "a bomba é pequena compactada: {}",
            z.len()
        );
        assert!(matches!(
            Partitura::de_mxl(&z),
            Err(ErroPartitura::LimiteExcedido(_))
        ));
    }

    #[test]
    fn zip_bomb_por_taxa_abaixo_do_teto_de_tamanho_e_recusada() {
        // 4 MiB de zeros cabem no teto de 8 MiB, mas a taxa passa de 200:1.
        let mut xml = String::from("<score-partwise>");
        xml.push_str(&" ".repeat(4 * 1024 * 1024));
        xml.push_str("</score-partwise>");
        let z = zip_de(&[
            (CAMINHO_CONTAINER, CONTAINER_OK.as_bytes(), true),
            ("score.xml", xml.as_bytes(), true),
        ]);
        assert_eq!(
            Partitura::de_mxl(&z),
            Err(ErroPartitura::LimiteExcedido(
                "taxa de compressão suspeita (zip-bomb?)"
            ))
        );
    }

    #[test]
    fn partitura_real_passa_na_taxa_de_compressao() {
        let z = empacotar_mxl("score.xml", WIEGENLIED).expect("empacota");
        assert!(Partitura::de_mxl(&z).is_ok());
    }

    #[test]
    fn arquivo_compactado_grande_demais_e_recusado() {
        let enorme = vec![0u8; MXL_TAMANHO_MAXIMO_BYTES + 1];
        assert_eq!(
            Partitura::de_mxl(&enorme),
            Err(ErroPartitura::GrandeDemais(MXL_TAMANHO_MAXIMO_BYTES + 1))
        );
    }

    #[test]
    fn arquivo_principal_enorme_aninhado_e_recusado_pelo_tamanho_declarado() {
        // Armazenado (sem compressão), 9 MiB: o tamanho declarado já passa do teto.
        let grande = vec![b' '; TAMANHO_MAXIMO_BYTES + 1024];
        let z = zip_de(&[
            (CAMINHO_CONTAINER, CONTAINER_OK.as_bytes(), true),
            ("score.xml", &grande, false),
        ]);
        assert!(matches!(
            Partitura::de_mxl(&z),
            Err(ErroPartitura::LimiteExcedido(_))
        ));
    }

    #[test]
    fn mxl_dentro_de_mxl_nao_e_aberto() {
        let interno = mxl_ok();
        let z = zip_de(&[
            (CAMINHO_CONTAINER, CONTAINER_OK.as_bytes(), true),
            ("score.xml", &interno, false),
        ]);
        assert!(Partitura::de_mxl(&z).is_err());
    }

    #[test]
    fn entradas_nao_referenciadas_nunca_sao_lidas() {
        // Uma entrada enorme e corrompida que ninguém aponta não atrapalha nem é descompactada.
        let z = zip_de(&[
            (CAMINHO_CONTAINER, CONTAINER_OK.as_bytes(), true),
            ("score.xml", MINIMA.as_bytes(), true),
            ("capa.png", &vec![7u8; 1024 * 1024], false),
        ]);
        assert!(Partitura::de_mxl(&z).is_ok());
    }

    #[test]
    fn cabecalho_que_mente_sobre_o_tamanho_nao_vence_o_limite() {
        // Altera o tamanho descompactado declarado (diretório central e cabeçalho local) para 100.
        let zeros = vec![0u8; 12 * 1024 * 1024];
        let mut z = zip_de(&[
            (CAMINHO_CONTAINER, CONTAINER_OK.as_bytes(), true),
            ("score.xml", &zeros, true),
        ]);
        for pos in acha(&z, [0x50, 0x4B, 0x01, 0x02]) {
            let nome_len = u16::from_le_bytes([z[pos + 28], z[pos + 29]]) as usize;
            let nome = z[pos + 46..pos + 46 + nome_len].to_vec();
            if nome == b"score.xml" {
                z[pos + 24..pos + 28].copy_from_slice(&100u32.to_le_bytes());
            }
        }
        for pos in acha(&z, [0x50, 0x4B, 0x03, 0x04]) {
            let nome_len = u16::from_le_bytes([z[pos + 26], z[pos + 27]]) as usize;
            if &z[pos + 30..pos + 30 + nome_len] == b"score.xml" {
                z[pos + 22..pos + 26].copy_from_slice(&100u32.to_le_bytes());
            }
        }
        assert!(Partitura::de_mxl(&z).is_err());
    }

    fn altera_entrada(
        z: &mut [u8],
        nome: &[u8],
        central: impl Fn(&mut [u8]),
        local: impl Fn(&mut [u8]),
    ) {
        for pos in acha(z, [0x50, 0x4B, 0x01, 0x02]) {
            let n = u16::from_le_bytes([z[pos + 28], z[pos + 29]]) as usize;
            if &z[pos + 46..pos + 46 + n] == nome {
                central(&mut z[pos..]);
            }
        }
        for pos in acha(z, [0x50, 0x4B, 0x03, 0x04]) {
            let n = u16::from_le_bytes([z[pos + 26], z[pos + 27]]) as usize;
            if &z[pos + 30..pos + 30 + n] == nome {
                local(&mut z[pos..]);
            }
        }
    }

    #[test]
    fn entrada_criptografada_e_recusada() {
        let mut z = mxl_ok();
        // Bit 0 das flags: criptografada (central +8, local +6).
        altera_entrada(&mut z, b"score.xml", |c| c[8] |= 1, |l| l[6] |= 1);
        assert!(eh_container(Partitura::de_mxl(&z)).contains("criptografada"));
        let mut z = mxl_ok();
        altera_entrada(
            &mut z,
            CAMINHO_CONTAINER.as_bytes(),
            |c| c[8] |= 1,
            |l| l[6] |= 1,
        );
        assert!(eh_container(Partitura::de_mxl(&z)).contains("criptografada"));
    }

    #[test]
    fn compressao_nao_suportada_e_recusada() {
        let mut z = mxl_ok();
        // Método 12 (bzip2) no diretório central (+10) e no cabeçalho local (+8).
        altera_entrada(
            &mut z,
            b"score.xml",
            |c| c[10..12].copy_from_slice(&12u16.to_le_bytes()),
            |l| l[8..10].copy_from_slice(&12u16.to_le_bytes()),
        );
        assert!(eh_container(Partitura::de_mxl(&z)).contains("compress"));
    }

    #[test]
    fn dados_deflate_corrompidos_falham_sem_panico() {
        let mut z = mxl_ok();
        let pos = acha(&z, [0x50, 0x4B, 0x03, 0x04])[1];
        let n = u16::from_le_bytes([z[pos + 26], z[pos + 27]]) as usize;
        let m = u16::from_le_bytes([z[pos + 28], z[pos + 29]]) as usize;
        let ini = pos + 30 + n + m;
        for b in &mut z[ini..ini + 20] {
            *b = 0xFF;
        }
        assert!(Partitura::de_mxl(&z).is_err());
    }

    #[test]
    fn xml_malicioso_dentro_do_mxl_cai_no_leitor_de_musicxml() {
        let bomba = r#"<?xml version="1.0"?><!DOCTYPE x [<!ENTITY a "aaaaaaaaaa"><!ENTITY b "&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;"><!ENTITY c "&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;"><!ENTITY d "&c;&c;&c;&c;&c;&c;&c;&c;&c;&c;"><!ENTITY e "&d;&d;&d;&d;&d;&d;&d;&d;&d;&d;"><!ENTITY f "&e;&e;&e;&e;&e;&e;&e;&e;&e;&e;"><!ENTITY g "&f;&f;&f;&f;&f;&f;&f;&f;&f;&f;"><!ENTITY h "&g;&g;&g;&g;&g;&g;&g;&g;&g;&g;">]><score-partwise>&h;&h;&h;&h;&h;&h;&h;&h;&h;&h;</score-partwise>"#;
        let z = zip_de(&[
            (CAMINHO_CONTAINER, CONTAINER_OK.as_bytes(), true),
            ("score.xml", bomba.as_bytes(), true),
        ]);
        assert!(Partitura::de_mxl(&z).is_err());
    }

    const ODE: &str =
        include_str!("../../../assets/partituras/beethoven-ode-a-alegria-tema.musicxml");
    /// Gera o `.mxl` do Ode em um caminho dado (o repositório ignora `*.mxl` por política):
    /// `PT_MXL_SAIDA=/tmp/ode.mxl cargo test -p piano-tutor-core gera_mxl -- --ignored`.
    #[test]
    #[ignore = "só grava um arquivo quando PT_MXL_SAIDA está definida"]
    fn gera_mxl_do_ode_em_caminho_escolhido() {
        let Some(destino) = std::env::var_os("PT_MXL_SAIDA") else {
            return;
        };
        let z = empacotar_mxl("beethoven-ode-a-alegria-tema.musicxml", ODE).expect("empacota");
        std::fs::write(destino, z).expect("grava");
    }

    #[test]
    fn mxl_do_ode_gerado_em_memoria_equivale_ao_musicxml() {
        let z = empacotar_mxl("beethoven-ode-a-alegria-tema.musicxml", ODE).expect("empacota");
        let via_mxl = Partitura::de_mxl(&z).expect("lê o .mxl");
        let direto = Partitura::de_musicxml(ODE).expect("lê o .musicxml");
        assert_eq!(via_mxl, direto);
        assert_eq!(via_mxl.compassos.len(), 16);
        assert_eq!(via_mxl.total_de_notas(), 94);
        assert_eq!(extrair_musicxml(&z).expect("extrai"), ODE);
        assert!(z.len() < 8 * 1024, "o .mxl é pequeno: {}", z.len());
    }
}
