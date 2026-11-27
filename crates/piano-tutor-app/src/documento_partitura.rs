//! Documento da partitura fora da interface: o texto MusicXML carregado, a [`Partitura`]
//! derivada, os dados musicais ([`Musica`]) e a exportação. Não depende de egui, áudio nem
//! janela (seam S1): a tela de Partitura só o usa.

use piano_tutor_core::mxl::{MXL_TAMANHO_MAXIMO_BYTES, extrair_musicxml};
use piano_tutor_core::partitura::{
    ErroPartitura, EventoReproducao, MapaAndamento, Partitura, TAMANHO_MAXIMO_BYTES,
};
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

/// Lê um `.musicxml`, `.xml` ou `.mxl` (com os limites de tamanho) e devolve o texto XML.
pub fn ler_arquivo(caminho: &Path) -> Result<String, String> {
    let ext = caminho
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    if ext != "musicxml" && ext != "xml" && ext != "mxl" {
        return Err("use um arquivo .musicxml, .xml ou .mxl".into());
    }
    let meta = std::fs::metadata(caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
    if !meta.is_file() {
        return Err(format!("{} não é um arquivo", caminho.display()));
    }
    if ext == "mxl" {
        if meta.len() > MXL_TAMANHO_MAXIMO_BYTES as u64 {
            return Err(format!(
                "arquivo grande demais ({} bytes; máximo {MXL_TAMANHO_MAXIMO_BYTES})",
                meta.len()
            ));
        }
        let bytes = std::fs::read(caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
        return extrair_musicxml(&bytes).map_err(|e| e.to_string());
    }
    if meta.len() > TAMANHO_MAXIMO_BYTES as u64 {
        return Err(format!(
            "arquivo grande demais ({} bytes; máximo {TAMANHO_MAXIMO_BYTES})",
            meta.len()
        ));
    }
    std::fs::read_to_string(caminho).map_err(|e| format!("{}: {e}", caminho.display()))
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

/// Partitura carregada: nome, texto XML original (nada é descartado), modelo e dados musicais.
#[derive(Clone, Debug)]
pub struct DocumentoPartitura {
    pub nome: String,
    /// Texto XML carregado, gravado de volta na exportação.
    pub xml: String,
    pub partitura: Partitura,
    pub musica: Musica,
}

impl DocumentoPartitura {
    /// Interpreta o texto MusicXML.
    pub fn de_texto(nome: String, texto: String) -> Result<Self, ErroPartitura> {
        let partitura = Partitura::de_musicxml(&texto)?;
        let musica = Musica::de(&partitura);
        Ok(Self {
            nome,
            xml: texto,
            partitura,
            musica,
        })
    }

    /// Abre um arquivo `.musicxml`, `.xml` ou `.mxl`; o nome é o do arquivo.
    pub fn de_arquivo(caminho: &Path, nome_se_faltar: &str) -> Result<Self, String> {
        let texto = ler_arquivo(caminho)?;
        let nome = caminho
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| nome_se_faltar.to_owned());
        Self::de_texto(nome, texto).map_err(|e| e.to_string())
    }

    /// Grava o texto original em `destino` e confere relendo o arquivo.
    pub fn gravar(&self, destino: &Path, sobrescrever: bool) -> Result<(), String> {
        if self.xml.is_empty() {
            return Err("nenhuma partitura carregada".into());
        }
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
        f.write_all(self.xml.as_bytes())
            .map_err(|e| format!("{}: {e}", destino.display()))?;
        drop(f);
        let lido =
            std::fs::read_to_string(destino).map_err(|e| format!("{}: {e}", destino.display()))?;
        let relido = Partitura::de_musicxml(&lido).map_err(|e| e.to_string())?;
        if relido != self.partitura {
            return Err("a releitura do arquivo exportado difere da partitura carregada".into());
        }
        Ok(())
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
        assert_eq!(d.xml, EMBUTIDAS[3].1, "o texto original é guardado");
        assert_eq!(d.partitura.compassos.len(), 16);
        assert_eq!(
            d.musica.eventos.len(),
            d.partitura.eventos_sem_unissono().len()
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
        // A releitura de conferência detecta um modelo que difere do arquivo gravado.
        let outro =
            DocumentoPartitura::de_texto("B".into(), EMBUTIDAS[0].1.to_owned()).expect("ok");
        let mut trocado = d.clone();
        trocado.partitura = outro.partitura;
        let e = trocado.gravar(&destino, true).expect_err("difere");
        assert!(e.contains("releitura"), "{e}");
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
        assert_eq!(d.xml, EMBUTIDAS[3].1);

        let mxl = dir.join("ode.mxl");
        std::fs::write(
            &mxl,
            empacotar_mxl("ode.musicxml", EMBUTIDAS[3].1).expect("zip"),
        )
        .expect("grava");
        let m = DocumentoPartitura::de_arquivo(&mxl, "?").expect("abre mxl");
        assert_eq!(m.xml, EMBUTIDAS[3].1, "o .mxl entrega o mesmo XML");
        assert_eq!(m.partitura, d.partitura);

        let ruim = dir.join("a.txt");
        std::fs::write(&ruim, "x").expect("grava");
        assert!(DocumentoPartitura::de_arquivo(&ruim, "?").is_err());
        assert!(DocumentoPartitura::de_arquivo(&dir.join("nao-existe.xml"), "?").is_err());
        let falso = dir.join("falso.mxl");
        std::fs::write(&falso, "não é zip").expect("grava");
        assert!(DocumentoPartitura::de_arquivo(&falso, "?").is_err());
    }
}
