//! Documento da partitura fora da interface: o [`DocumentoAberto`] do serviço de documento do
//! núcleo (texto original, [`Partitura`] derivada e identidade de cada nota), os dados musicais
//! ([`Musica`]) e a exportação. Não depende de egui, áudio nem janela (seam S1): a tela de
//! Partitura só o usa. Abrir e exportar passam **somente** pelo serviço (`piano_tutor_core::
//! documento`); aqui ficam apenas a leitura e a gravação de arquivos, com os limites de tamanho.

use piano_tutor_core::documento::{DocumentoAberto, ErroServico, Formato, IdsPorNota};
use piano_tutor_core::mxl::MXL_TAMANHO_MAXIMO_BYTES;
use piano_tutor_core::partitura::{
    EventoReproducao, MapaAndamento, Partitura, TAMANHO_MAXIMO_BYTES,
};
use piano_tutor_core::xml_seguro::nome_para_mensagem;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Dados musicais derivados da partitura, calculados uma vez por carga.
#[derive(Clone, Debug)]
pub struct Musica {
    pub eventos: Vec<EventoReproducao>,
    pub mapa: MapaAndamento,
    /// Início de cada compasso tocado (semínimas na execução) e seu índice.
    pub execucao: Vec<(f64, usize)>,
    pub duracao_q: f64,
    pub bpm_arquivo: Option<f64>,
}

impl Musica {
    pub fn de(p: &Partitura) -> Self {
        Self {
            // Sem uníssonos: a mesma lista (e a mesma numeração) de `esperados_da_partitura`.
            eventos: p.eventos_sem_unissono(),
            mapa: p.mapa_de_andamento(),
            execucao: p
                .compassos_na_execucao()
                .into_iter()
                .map(|(q, c)| (q.como_f64(), c))
                .collect(),
            duracao_q: p.duracao_da_execucao().como_f64(),
            bpm_arquivo: p.andamento_inicial(),
        }
    }

    /// Segundos do início da execução até `q` semínimas, no andamento `bpm_alvo`.
    pub fn segundos(&self, q: f64, bpm_alvo: f64) -> f64 {
        let base = self.bpm_arquivo.unwrap_or(bpm_alvo).max(1.0);
        self.mapa.segundos(q, base, bpm_alvo / base)
    }

    /// Instante (semínimas na execução) do primeiro compasso `c` tocado.
    pub fn inicio_do_compasso(&self, c: usize) -> Option<f64> {
        self.execucao.iter().find(|(_, i)| *i == c).map(|(q, _)| *q)
    }

    /// Compasso e tempo dentro dele para a posição `q` (semínimas na execução).
    pub fn compasso_em(&self, q: f64) -> Option<(usize, f64)> {
        self.execucao
            .iter()
            .rev()
            .find(|(ini, _)| *ini <= q + 1e-9)
            .map(|(ini, c)| (*c, (q - ini).max(0.0)))
    }
}

/// Lê os bytes de um `.musicxml`, `.xml` ou `.mxl` (com os limites de tamanho) e diz o formato.
/// Quem interpreta os bytes é o serviço de documento.
pub fn ler_arquivo(caminho: &Path) -> Result<(Vec<u8>, Formato), String> {
    let formato = caminho
        .extension()
        .and_then(|e| e.to_str())
        .and_then(Formato::de_extensao)
        .ok_or_else(|| "use um arquivo .musicxml, .xml ou .mxl".to_owned())?;
    let nome = nome_curto(caminho);
    let meta =
        std::fs::metadata(caminho).map_err(|e| format!("{nome}: {}", erro_de_leitura(&e)))?;
    if !meta.is_file() {
        return Err(format!("{nome} não é um arquivo"));
    }
    let maximo = match formato {
        Formato::Mxl => MXL_TAMANHO_MAXIMO_BYTES,
        Formato::MusicXml => TAMANHO_MAXIMO_BYTES,
    };
    if meta.len() > maximo as u64 {
        return Err(format!(
            "arquivo grande demais ({} bytes; máximo {maximo})",
            meta.len()
        ));
    }
    // O arquivo pode crescer entre a checagem e a leitura (ou ser um dispositivo que não acaba):
    // a leitura em si também tem teto.
    let mut bytes = Vec::new();
    std::fs::File::open(caminho)
        .and_then(|f| f.take(maximo as u64 + 1).read_to_end(&mut bytes))
        .map_err(|e| format!("{nome}: {}", erro_de_leitura(&e)))?;
    if bytes.len() > maximo {
        return Err(format!(
            "arquivo grande demais (mais de {maximo} bytes; máximo {maximo})"
        ));
    }
    Ok((bytes, formato))
}

/// Só o nome do arquivo (nunca a pasta do usuário), curto e sem caracteres de controle.
fn nome_curto(caminho: &Path) -> String {
    caminho.file_name().map_or_else(
        || "arquivo".to_owned(),
        |n| nome_para_mensagem(&n.to_string_lossy()),
    )
}

/// Motivo de uma falha de E/S em português, sem o texto do sistema (que vem em inglês e pode
/// trazer caminho).
fn erro_de_leitura(e: &std::io::Error) -> &'static str {
    match e.kind() {
        std::io::ErrorKind::NotFound => "arquivo não encontrado",
        std::io::ErrorKind::PermissionDenied => "sem permissão para ler",
        _ => "não foi possível ler o arquivo",
    }
}

/// Valida o caminho de destino da exportação (acrescenta `.musicxml` se faltar extensão).
pub fn destino_de_exportacao(caminho: &str) -> Result<PathBuf, String> {
    let t = caminho.trim();
    if t.is_empty() {
        return Err("digite o caminho do arquivo de destino".into());
    }
    let mut c = PathBuf::from(t);
    if c.extension().is_none() {
        c.set_extension("musicxml");
    }
    let ext = c
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    if ext != "musicxml" && ext != "xml" {
        return Err("o destino deve terminar em .musicxml ou .xml".into());
    }
    Ok(c)
}

/// Partitura carregada: nome, documento aberto pelo serviço (texto original, modelo e
/// identidades de nota) e dados musicais.
#[derive(Clone, Debug)]
pub struct DocumentoPartitura {
    pub nome: String,
    aberto: DocumentoAberto,
    pub musica: Musica,
}

impl DocumentoPartitura {
    fn de_aberto(nome: String, aberto: DocumentoAberto) -> Self {
        let musica = Musica::de(aberto.partitura());
        Self {
            nome,
            aberto,
            musica,
        }
    }

    /// Interpreta o texto MusicXML.
    pub fn de_texto(nome: String, texto: String) -> Result<Self, ErroServico> {
        let aberto = DocumentoAberto::abrir_texto(&nome, &texto)?;
        Ok(Self::de_aberto(nome, aberto))
    }

    /// Abre um arquivo `.musicxml`, `.xml` ou `.mxl`; o nome é o do arquivo.
    pub fn de_arquivo(caminho: &Path, nome_se_faltar: &str) -> Result<Self, String> {
        let (bytes, formato) = ler_arquivo(caminho)?;
        let nome = caminho
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| nome_se_faltar.to_owned());
        let aberto =
            DocumentoAberto::abrir_bytes(&nome, &bytes, formato).map_err(|e| e.to_string())?;
        Ok(Self::de_aberto(nome, aberto))
    }

    /// O modelo para tocar e desenhar.
    pub fn partitura(&self) -> &Partitura {
        self.aberto.partitura()
    }

    /// Identidade de cada nota do modelo, para o layout.
    pub fn ids(&self) -> &IdsPorNota {
        self.aberto.ids()
    }

    /// O texto MusicXML original, que a exportação sem edição devolve idêntico.
    pub fn xml(&self) -> &str {
        self.aberto.texto_original()
    }

    /// Grava o MusicXML exportado pelo serviço em `destino` e confere relendo o arquivo.
    pub fn gravar(&self, destino: &Path, sobrescrever: bool) -> Result<(), String> {
        let texto = self.aberto.exportar().map_err(|e| e.to_string())?;
        let mut aberto = std::fs::OpenOptions::new();
        aberto.write(true);
        if sobrescrever {
            aberto.create(true).truncate(true);
        } else {
            aberto.create_new(true);
        }
        use std::io::Write;
        let mut f = aberto.open(destino).map_err(|e| {
            format!(
                "{}: {e} (marque \"Sobrescrever\" para trocar um arquivo)",
                destino.display()
            )
        })?;
        f.write_all(texto.as_bytes())
            .map_err(|e| format!("{}: {e}", destino.display()))?;
        drop(f);
        let lido =
            std::fs::read_to_string(destino).map_err(|e| format!("{}: {e}", destino.display()))?;
        self.aberto
            .conferir_exportacao(&lido)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::tela_partitura::EMBUTIDAS;
    use piano_tutor_core::mxl::empacotar_mxl;

    fn pasta(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("pt-doc-{nome}-{}", std::process::id()));
        std::fs::create_dir_all(&p).expect("pasta");
        p
    }

    #[test]
    fn abre_texto_e_deriva_modelo_e_musica_sem_interface() {
        let d = DocumentoPartitura::de_texto("Ode".into(), EMBUTIDAS[3].1.to_owned())
            .expect("fixture válida");
        assert_eq!(d.nome, "Ode");
        assert_eq!(d.xml(), EMBUTIDAS[3].1, "o texto original é guardado");
        assert_eq!(d.partitura().compassos.len(), 16);
        assert_eq!(
            d.musica.eventos.len(),
            d.partitura().eventos_sem_unissono().len()
        );
        assert!(d.musica.duracao_q > 0.0);
        assert_eq!(d.musica.compasso_em(0.0).map(|(c, _)| c), Some(0));
    }

    #[test]
    fn texto_invalido_e_recusado_com_erro() {
        assert!(DocumentoPartitura::de_texto("x".into(), "<nao-e-musicxml/>".into()).is_err());
        assert!(DocumentoPartitura::de_texto("x".into(), String::new()).is_err());
    }

    #[test]
    fn exporta_o_texto_original_e_nao_sobrescreve_sem_pedir() {
        let d = DocumentoPartitura::de_texto("Ode".into(), EMBUTIDAS[3].1.to_owned()).expect("ok");
        let dir = pasta("exporta");
        let destino = dir.join("saida.musicxml");
        let _ = std::fs::remove_file(&destino);
        d.gravar(&destino, false).expect("grava");
        assert_eq!(
            std::fs::read_to_string(&destino).expect("lê"),
            EMBUTIDAS[3].1
        );
        assert!(
            d.gravar(&destino, false).is_err(),
            "não sobrescreve sem pedir"
        );
        assert!(d.gravar(&destino, true).is_ok());
    }

    #[test]
    fn exportar_confere_a_releitura_pelo_servico() {
        // Se o arquivo gravado deixasse de ser o documento aberto, a conferência do serviço
        // recusaria (cobertura do caminho de erro no teste do núcleo e aqui pelo mesmo contrato).
        let d = DocumentoPartitura::de_texto("Ode".into(), EMBUTIDAS[3].1.to_owned()).expect("ok");
        let outro =
            DocumentoPartitura::de_texto("B".into(), EMBUTIDAS[0].1.to_owned()).expect("ok");
        let e = d
            .aberto
            .conferir_exportacao(outro.xml())
            .expect_err("difere");
        assert!(e.to_string().contains("releitura"), "{e}");
    }

    #[test]
    fn abrir_sem_editar_nao_grava_nada_em_disco() {
        let dir = pasta("sem-gravar");
        let xml = dir.join("ode.musicxml");
        std::fs::write(&xml, EMBUTIDAS[3].1).expect("grava");
        let mxl = dir.join("ode.mxl");
        std::fs::write(
            &mxl,
            empacotar_mxl("ode.musicxml", EMBUTIDAS[3].1).expect("zip"),
        )
        .expect("grava");
        let instantaneo = || {
            let mut v: Vec<_> = std::fs::read_dir(&dir)
                .expect("lê")
                .map(|e| {
                    let e = e.expect("entrada");
                    let m = e.metadata().expect("meta");
                    (e.file_name(), m.len(), m.modified().expect("mtime"))
                })
                .collect();
            v.sort();
            v
        };
        let antes = instantaneo();
        for c in [&xml, &mxl] {
            let d = DocumentoPartitura::de_arquivo(c, "?").expect("abre");
            assert!(!d.aberto.editado());
            // Exportar devolve o texto, sem tocar o disco (quem grava é `gravar`).
            let _ = d.aberto.exportar().expect("exporta");
        }
        assert_eq!(instantaneo(), antes, "nenhum arquivo criado nem alterado");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rota_de_abrir_e_exportar_so_usa_o_servico_sem_parse_direto() {
        // Ticket 10: nenhum parse direto de MusicXML/MXL na rota da tela (só o serviço do núcleo).
        for (nome, fonte) in [
            (
                "documento_partitura.rs",
                include_str!("documento_partitura.rs"),
            ),
            ("tela_partitura.rs", include_str!("tela_partitura.rs")),
        ] {
            let producao = fonte
                .split("#[cfg(test)]\nmod testes")
                .next()
                .unwrap_or_default();
            for proibido in ["de_musicxml", "extrair_musicxml", "de_mxl", "roxmltree"] {
                assert!(!producao.contains(proibido), "{nome} usa {proibido}");
            }
        }
    }

    #[test]
    fn erros_de_leitura_nao_vazam_o_caminho_completo_e_estao_em_portugues() {
        let dir = pasta("sem-vazar");
        let segredo = dir.join("pasta-do-usuario-secreta");
        std::fs::create_dir_all(&segredo).expect("pasta");
        let ausente = segredo.join("some.mxl");
        let falso = segredo.join("lixo.musicxml");
        std::fs::write(&falso, "isto não é xml").expect("grava");
        let diretorio = segredo.join("pasta.musicxml");
        std::fs::create_dir_all(&diretorio).expect("pasta");
        for c in [&ausente, &falso, &diretorio] {
            let e = DocumentoPartitura::de_arquivo(c, "?").expect_err("recusa");
            assert!(!e.contains("pasta-do-usuario-secreta"), "vaza caminho: {e}");
            assert!(!e.contains(&*dir.to_string_lossy()), "vaza caminho: {e}");
            let baixa = e.to_lowercase();
            for ingles in ["no such file", "os error", "not a directory", "permission"] {
                assert!(!baixa.contains(ingles), "mensagem em inglês: {e}");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn identidade_de_cada_nota_chega_do_servico() {
        let d = DocumentoPartitura::de_texto("Ode".into(), EMBUTIDAS[3].1.to_owned()).expect("ok");
        assert_eq!(d.ids().total(), d.partitura().total_de_notas());
        assert!(d.ids().id(0, 0, 0).is_some());
    }

    #[test]
    fn destino_valida_extensao_e_completa_a_que_falta() {
        assert!(destino_de_exportacao("").is_err());
        assert!(destino_de_exportacao("  ").is_err());
        assert!(destino_de_exportacao("a.txt").is_err());
        assert_eq!(
            destino_de_exportacao("a/b").expect("ok"),
            PathBuf::from("a/b.musicxml")
        );
        assert!(destino_de_exportacao("a.XML").is_ok());
    }

    #[test]
    fn abre_arquivo_xml_e_mxl_e_recusa_o_que_nao_serve() {
        let dir = pasta("abre");
        let xml = dir.join("ode.musicxml");
        std::fs::write(&xml, EMBUTIDAS[3].1).expect("grava");
        let d = DocumentoPartitura::de_arquivo(&xml, "?").expect("abre xml");
        assert_eq!(d.nome, "ode.musicxml");
        assert_eq!(d.xml(), EMBUTIDAS[3].1);

        let mxl = dir.join("ode.mxl");
        std::fs::write(
            &mxl,
            empacotar_mxl("ode.musicxml", EMBUTIDAS[3].1).expect("zip"),
        )
        .expect("grava");
        let m = DocumentoPartitura::de_arquivo(&mxl, "?").expect("abre mxl");
        assert_eq!(m.xml(), EMBUTIDAS[3].1, "o .mxl entrega o mesmo XML");
        assert_eq!(m.partitura(), d.partitura());

        let ruim = dir.join("a.txt");
        std::fs::write(&ruim, "x").expect("grava");
        assert!(DocumentoPartitura::de_arquivo(&ruim, "?").is_err());
        assert!(DocumentoPartitura::de_arquivo(&dir.join("nao-existe.xml"), "?").is_err());
        let falso = dir.join("falso.mxl");
        std::fs::write(&falso, "não é zip").expect("grava");
        assert!(DocumentoPartitura::de_arquivo(&falso, "?").is_err());
    }
}
