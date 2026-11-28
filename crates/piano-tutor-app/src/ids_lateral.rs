//! Arquivo lateral de identidades de nota (`IdNota`) no app (ticket 17b, seam S1): onde ele mora,
//! como é lido e gravado, sem janela e sem egui. O formato da tabela é do núcleo
//! ([`TabelaIds`]); aqui ficam só o nome do arquivo, o teto de tamanho e a gravação atômica.
//!
//! # Contrato em linguagem de domínio
//!
//! - **Nome.** O arquivo vai ao lado do texto, com `.ids` acrescentado ao nome inteiro:
//!   `ode.musicxml` tem `ode.musicxml.ids` (assim `ode.xml` e `ode.musicxml` nunca disputam o
//!   mesmo arquivo). Em Versões e no rascunho do autosave o nome é o do objeto
//!   (`objetos/<hash>.ids`, `rascunho.ids`).
//! - **A tabela vale para um texto só.** O núcleo a liga ao texto por impressão digital; tabela de
//!   outro texto é recusada na abertura e **o app abre sem ela**, sem erro fatal: as identidades
//!   são renumeradas e o diff cai no pareamento por conteúdo. Arquivo ausente, ilegível, grande
//!   demais ou com formato desconhecido faz o mesmo: nunca impede de abrir a partitura.
//! - **Linhagem.** O arquivo de uma Versão (ou do rascunho) pode começar com uma linha
//!   `derivada-de=<hash>`: a Versão cujas identidades este documento herdou. O diff só usa
//!   `IdNota` entre uma Versão e quem descende dela por essa linha (ver `versoes_diff`). Sem a
//!   linha o arquivo é exatamente o texto do núcleo e continua legível por ele.
//! - **Gravação atômica** (temporário vizinho e `rename`), como o resto do app.

use crate::revisao::gravar_atomico;
use crate::versoes::hash_valido;
use piano_tutor_core::documento::{DocumentoAberto, ErroServico, TabelaIds};
use std::io::Read as _;
use std::path::{Path, PathBuf};

/// Extensão acrescentada ao nome do arquivo para formar o arquivo lateral.
pub const EXTENSAO: &str = "ids";
/// Maior arquivo lateral lido (a tabela tem uma linha por nota: bem abaixo disto).
pub const MAX_BYTES: u64 = 16 * 1024 * 1024;
const PREFIXO_DA_LINHAGEM: &str = "derivada-de=";

/// A tabela de identidades e, se houver, a Versão de que ela descende.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdsGravados {
    pub derivada_de: Option<String>,
    pub tabela: TabelaIds,
}

impl IdsGravados {
    pub fn nova(tabela: TabelaIds, derivada_de: Option<&str>) -> Self {
        Self {
            derivada_de: derivada_de.filter(|h| hash_valido(h)).map(str::to_owned),
            tabela,
        }
    }

    /// Texto a gravar: a linha de linhagem (se houver) e a tabela do núcleo.
    pub fn para_texto(&self) -> String {
        let mut s = String::new();
        if let Some(h) = &self.derivada_de {
            s.push_str(PREFIXO_DA_LINHAGEM);
            s.push_str(h);
            s.push('\n');
        }
        s.push_str(&self.tabela.para_texto());
        s
    }

    /// Lê o texto gravado. `Err` com o motivo se não é uma tabela do núcleo.
    pub fn de_texto(texto: &str) -> Result<Self, String> {
        let (derivada_de, resto) = match texto.split_once('\n') {
            Some((primeira, resto)) if primeira.starts_with(PREFIXO_DA_LINHAGEM) => {
                let h = primeira[PREFIXO_DA_LINHAGEM.len()..].trim();
                // Linhagem malformada não invalida a tabela: só deixa de valer como linhagem.
                (hash_valido(h).then(|| h.to_owned()), resto)
            }
            _ => (None, texto),
        };
        let tabela = TabelaIds::de_texto(resto).map_err(|e| e.to_string())?;
        Ok(Self {
            derivada_de,
            tabela,
        })
    }
}

/// O que aconteceu com o arquivo lateral ao abrir um texto.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SituacaoDosIds {
    /// Não havia arquivo lateral: identidades numeradas pela leitura.
    #[default]
    Nenhum,
    /// O arquivo lateral foi usado: as notas reencontraram as identidades de antes.
    Usado,
    /// Havia arquivo, mas não serve (de outro texto, ilegível, formato desconhecido): o texto
    /// abriu sem ele, com o motivo aqui. Nunca é erro fatal.
    Recusado(String),
}

/// Abre um documento com o arquivo lateral (`lateral`, como [`ler`] devolve), **sem nunca falhar
/// por causa dele**: tabela ausente, ilegível ou de outro texto vira abertura sem tabela. Só um
/// erro do próprio texto (que falharia igual sem tabela) é devolvido. `abrir` recebe a tabela a
/// usar ou `None`. Devolve também a linhagem (`derivada-de`) quando a tabela foi usada.
pub fn abrir_tolerante(
    lateral: Option<Result<IdsGravados, String>>,
    abrir: impl Fn(Option<&TabelaIds>) -> Result<DocumentoAberto, ErroServico>,
) -> Result<(DocumentoAberto, SituacaoDosIds, Option<String>), ErroServico> {
    match lateral {
        None => Ok((abrir(None)?, SituacaoDosIds::Nenhum, None)),
        Some(Err(motivo)) => Ok((abrir(None)?, SituacaoDosIds::Recusado(motivo), None)),
        Some(Ok(g)) => match abrir(Some(&g.tabela)) {
            Ok(d) => Ok((d, SituacaoDosIds::Usado, g.derivada_de)),
            Err(e) => {
                let d = abrir(None)?;
                Ok((d, SituacaoDosIds::Recusado(e.to_string()), None))
            }
        },
    }
}

/// Caminho do arquivo lateral de `arquivo`: o mesmo nome com `.ids` acrescentado.
pub fn caminho_lateral(arquivo: &Path) -> PathBuf {
    let mut nome = arquivo
        .file_name()
        .map_or_else(|| "partitura".into(), std::ffi::OsStr::to_os_string);
    nome.push(".");
    nome.push(EXTENSAO);
    arquivo.with_file_name(nome)
}

/// Lê um arquivo lateral. `None` = não existe (caso comum, sem aviso); `Some(Err)` = existe e
/// não serve (grande demais, ilegível, formato desconhecido); o chamador abre sem a tabela.
pub fn ler(caminho: &Path) -> Option<Result<IdsGravados, String>> {
    let f = match std::fs::File::open(caminho) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(_) => return Some(Err("não foi possível ler o arquivo de identidades".into())),
    };
    let mut bytes = Vec::new();
    if f.take(MAX_BYTES + 1).read_to_end(&mut bytes).is_err() {
        return Some(Err("não foi possível ler o arquivo de identidades".into()));
    }
    if bytes.len() as u64 > MAX_BYTES {
        return Some(Err("arquivo de identidades grande demais".into()));
    }
    Some(match String::from_utf8(bytes) {
        Ok(t) => IdsGravados::de_texto(&t),
        Err(_) => Err("arquivo de identidades não é UTF-8".into()),
    })
}

/// Grava o arquivo lateral de forma atômica.
pub fn gravar(caminho: &Path, ids: &IdsGravados) -> Result<(), String> {
    gravar_atomico(caminho, &ids.para_texto()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod testes {
    use super::*;
    use piano_tutor_core::documento::DocumentoAberto;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="3.1"><part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list>
<part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
<note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note></measure></part></score-partwise>"#;

    fn tabela() -> TabelaIds {
        DocumentoAberto::abrir_texto("t", XML)
            .expect("abre")
            .tabela_ids()
    }

    #[test]
    fn o_nome_acrescenta_ids_ao_nome_inteiro() {
        assert_eq!(
            caminho_lateral(Path::new("/x/ode.musicxml")),
            PathBuf::from("/x/ode.musicxml.ids")
        );
        assert_ne!(
            caminho_lateral(Path::new("/x/ode.xml")),
            caminho_lateral(Path::new("/x/ode.musicxml"))
        );
    }

    #[test]
    fn ida_e_volta_com_e_sem_linhagem_e_sem_linhagem_e_o_texto_do_nucleo() {
        let t = tabela();
        let sem = IdsGravados::nova(t.clone(), None);
        assert_eq!(sem.para_texto(), t.para_texto());
        assert_eq!(IdsGravados::de_texto(&sem.para_texto()), Ok(sem));
        let h = "a".repeat(64);
        let com = IdsGravados::nova(t.clone(), Some(&h));
        assert!(com.para_texto().starts_with("derivada-de=aaaa"));
        assert_eq!(IdsGravados::de_texto(&com.para_texto()), Ok(com));
        // Linhagem que não é um hash vira "sem linhagem" (a tabela continua valendo).
        let torta = format!("derivada-de=xyz\n{}", t.para_texto());
        assert_eq!(IdsGravados::de_texto(&torta).expect("lê").derivada_de, None);
        assert!(IdsGravados::de_texto("lixo\n").is_err());
        assert!(IdsGravados::de_texto("").is_err());
    }

    #[test]
    fn grava_e_le_do_disco_e_ausente_nao_e_erro() {
        let d = tempfile::tempdir().expect("pasta");
        let c = caminho_lateral(&d.path().join("p.musicxml"));
        assert!(ler(&c).is_none());
        let ids = IdsGravados::nova(tabela(), None);
        gravar(&c, &ids).expect("grava");
        assert_eq!(ler(&c), Some(Ok(ids)));
        std::fs::write(&c, "não é tabela").expect("estraga");
        assert!(matches!(ler(&c), Some(Err(_))));
        std::fs::write(&c, vec![b'x'; (MAX_BYTES + 1) as usize]).expect("grande");
        assert!(matches!(ler(&c), Some(Err(m)) if m.contains("grande demais")));
        std::fs::write(&c, [0xff, 0xfe, 0x00]).expect("binário");
        assert!(matches!(ler(&c), Some(Err(_))));
    }
}
