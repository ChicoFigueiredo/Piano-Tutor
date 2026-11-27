//! Serviço de documento (ticket 10, E01, seam S1): a **porta única** para abrir e exportar uma
//! partitura MusicXML ou MXL, sem janela, sem áudio, sem rede e sem tocar o disco.
//!
//! # Contrato em linguagem de domínio
//!
//! - **Abrir** recebe o texto (ou os bytes e o [`Formato`]) e devolve um [`DocumentoAberto`]: a
//!   partitura para tocar e desenhar ([`Partitura`]), o documento com origem por faixa de bytes
//!   ([`Documento`]) e a identidade estável de cada nota ([`IdsPorNota`]). Ou recusa com um
//!   [`ErroServico`] tipado; nunca aceita errado em silêncio.
//! - **Exportar** devolve o texto MusicXML. Sem edição, é **idêntico byte a byte** ao original
//!   (critério A1); para `.mxl`, o contrato vale sobre o XML de dentro do zip (recompactar não
//!   devolveria os mesmos bytes).
//! - **Conferir** reabre o texto que foi gravado e confirma que é o mesmo documento (a "releitura
//!   de conferência" da exportação).
//! - **Identidade das notas:** atribuída na abertura, monotônica a partir de 1, na ordem do
//!   arquivo. O serviço **não lê nem grava arquivo lateral de ids** (decisão do supervisor: só no
//!   ticket 17, quando houver edição estrutural); sem edição, reabrir o mesmo texto devolve as
//!   mesmas identidades.
//! - **Ciclo de vida:** um documento aberto sem edição ([`DocumentoAberto::editado`] falso) não
//!   gera nada: o serviço não grava arquivo algum (nem autosave nem Versão); quem escolhe e grava o
//!   destino é o chamador, sempre com confirmação antes de sobrescrever.
//!
//! # O que é recusado
//!
//! Tudo o que o leitor de partitura já recusava (XML inválido, raiz diferente de `score-partwise`,
//! limites de tamanho e de nós, `.mxl` malformado ou perigoso) e, a mais, o que o documento não
//! sabe preservar byte a byte: bytes que não são UTF-8 (UTF-16, Latin-1 com acento), entidade de
//! DTD que expande elementos e estrutura de compasso inconsistente. Normalizar a codificação é
//! decisão de importação posterior (a exportação passaria a valer sobre o derivado).

use super::historico::Historico;
use super::{Documento, ErroDocumento, IdNota, LimitesDeAltura};
use crate::mxl::extrair_musicxml;
use crate::partitura::{ErroPartitura, Partitura};
use std::fmt;

/// Formato do arquivo de origem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Formato {
    /// `.musicxml` ou `.xml`: texto UTF-8.
    MusicXml,
    /// `.mxl`: MusicXML compactado em zip.
    Mxl,
}

impl Formato {
    /// Formato pela extensão do arquivo (sem ponto, qualquer caixa); `None` se não for suportada.
    pub fn de_extensao(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "musicxml" | "xml" => Some(Self::MusicXml),
            "mxl" => Some(Self::Mxl),
            _ => None,
        }
    }
}

/// Por que um documento não abriu, não exportou ou não conferiu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroServico {
    /// Recusa do leitor de partitura (XML, limites, `.mxl`); a mensagem é a de sempre.
    Partitura(ErroPartitura),
    /// Recusa do documento (codificação, entidades, faixas, durações...).
    Documento(ErroDocumento),
    /// Os dois modelos discordam sobre as notas do arquivo (não deve acontecer).
    Inconsistente(String),
    /// O texto gravado reabre como outro documento.
    ReleituraDifere(String),
}

impl fmt::Display for ErroServico {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Partitura(e) => write!(f, "{e}"),
            Self::Documento(e) => write!(f, "{e}"),
            Self::Inconsistente(m) => write!(f, "documento inconsistente: {m}"),
            Self::ReleituraDifere(m) => write!(f, "a releitura do arquivo exportado difere: {m}"),
        }
    }
}

impl std::error::Error for ErroServico {}

impl From<ErroPartitura> for ErroServico {
    fn from(e: ErroPartitura) -> Self {
        Self::Partitura(e)
    }
}

impl From<ErroDocumento> for ErroServico {
    fn from(e: ErroDocumento) -> Self {
        Self::Documento(e)
    }
}

/// Identidade de cada nota do modelo de tocar ([`Partitura`]), indexada do mesmo jeito que o
/// layout a percorre: parte, compasso e posição da nota em `Compasso::notas`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IdsPorNota {
    partes: Vec<Vec<Vec<IdNota>>>,
}

impl IdsPorNota {
    /// Identidade da nota `ordinal` do compasso `compasso` da parte `parte` (índices de
    /// `Partitura::partes[..].compassos[..].notas[..]`); `None` fora dos limites.
    pub fn id(&self, parte: usize, compasso: usize, ordinal: usize) -> Option<IdNota> {
        self.partes.get(parte)?.get(compasso)?.get(ordinal).copied()
    }

    /// Identidades do compasso (vazio fora dos limites). Serve ao instantâneo do histórico.
    pub(super) fn compasso(&self, parte: usize, compasso: usize) -> Vec<IdNota> {
        self.partes
            .get(parte)
            .and_then(|p| p.get(compasso))
            .cloned()
            .unwrap_or_default()
    }

    /// Troca as identidades do compasso (histórico: desfazer e refazer).
    pub(super) fn definir_compasso(&mut self, parte: usize, compasso: usize, ids: Vec<IdNota>) {
        if let Some(c) = self.partes.get_mut(parte).and_then(|p| p.get_mut(compasso)) {
            *c = ids;
        }
    }

    /// Quantas notas têm identidade.
    pub fn total(&self) -> usize {
        self.partes
            .iter()
            .flat_map(|p| p.iter())
            .map(Vec::len)
            .sum()
    }
}

/// Partitura aberta: o modelo de tocar, o documento com origem por faixa e as identidades.
#[derive(Clone, Debug)]
pub struct DocumentoAberto {
    nome: String,
    formato: Formato,
    pub(super) documento: Documento,
    pub(super) partitura: Partitura,
    pub(super) ids: IdsPorNota,
    /// Limites de altura que os comandos de edição respeitam (ticket 14).
    pub(super) limites: LimitesDeAltura,
    /// Pilhas de desfazer e refazer (ticket 15).
    pub(super) historico: Historico,
}

impl DocumentoAberto {
    /// Abre um texto MusicXML (`score-partwise`). `nome` é só rótulo para a interface.
    pub fn abrir_texto(nome: &str, texto: &str) -> Result<Self, ErroServico> {
        Self::abrir_com_formato(nome, texto, Formato::MusicXml)
    }

    /// Abre os bytes de um arquivo no `formato` dado. Em `.mxl` extrai o XML (com as defesas do
    /// módulo `mxl`); em MusicXML exige UTF-8 (com ou sem BOM).
    pub fn abrir_bytes(nome: &str, bytes: &[u8], formato: Formato) -> Result<Self, ErroServico> {
        match formato {
            Formato::Mxl => {
                let texto = extrair_musicxml(bytes)?;
                Self::abrir_com_formato(nome, &texto, Formato::Mxl)
            }
            Formato::MusicXml => {
                let texto =
                    std::str::from_utf8(bytes).map_err(|e| ErroDocumento::nao_utf8(bytes, &e))?;
                Self::abrir_com_formato(nome, texto, Formato::MusicXml)
            }
        }
    }

    fn abrir_com_formato(nome: &str, texto: &str, formato: Formato) -> Result<Self, ErroServico> {
        // O leitor de partitura vai primeiro: as recusas e mensagens de sempre não mudam.
        let partitura = Partitura::de_musicxml(texto)?;
        let documento = Documento::abrir(texto)?;
        let ids = montar_ids(&partitura, &documento)?;
        Ok(Self {
            nome: nome.to_owned(),
            formato,
            documento,
            partitura,
            ids,
            limites: LimitesDeAltura::default(),
            historico: Historico::default(),
        })
    }

    pub fn nome(&self) -> &str {
        &self.nome
    }

    pub fn formato(&self) -> Formato {
        self.formato
    }

    /// O documento com origem por faixa de bytes (base das edições dos próximos tickets).
    pub fn documento(&self) -> &Documento {
        &self.documento
    }

    /// O modelo para tocar e desenhar.
    pub fn partitura(&self) -> &Partitura {
        &self.partitura
    }

    /// Identidade de cada nota do modelo de tocar, para o layout.
    pub fn ids(&self) -> &IdsPorNota {
        &self.ids
    }

    /// O texto MusicXML original (imutável); em `.mxl`, o XML de dentro do zip.
    pub fn texto_original(&self) -> &str {
        self.documento.original()
    }

    /// Há algum compasso editado (sujo)? Sem edição, exportar devolve o original.
    pub fn editado(&self) -> bool {
        !self.documento.compassos_sujos().is_empty()
    }

    /// O texto MusicXML a gravar: copia byte a byte tudo que não foi editado.
    pub fn exportar(&self) -> Result<String, ErroServico> {
        Ok(self.documento.exportar()?)
    }

    /// Releitura de conferência: reabre `texto_gravado` pelo próprio serviço e confirma que dá o
    /// mesmo modelo de partitura e as mesmas identidades de nota (e, sem edição, os mesmos bytes
    /// do original).
    pub fn conferir_exportacao(&self, texto_gravado: &str) -> Result<(), ErroServico> {
        let relido = Self::abrir_texto(&self.nome, texto_gravado)?;
        if relido.partitura != self.partitura {
            return Err(ErroServico::ReleituraDifere(
                "o modelo da partitura mudou".into(),
            ));
        }
        if relido.ids != self.ids {
            return Err(ErroServico::ReleituraDifere(
                "as identidades das notas mudaram".into(),
            ));
        }
        if !self.editado() && texto_gravado != self.texto_original() {
            return Err(ErroServico::ReleituraDifere(
                "os bytes diferem do original".into(),
            ));
        }
        Ok(())
    }
}

/// Liga cada nota do modelo de tocar à do documento pela posição: as duas leituras produzem
/// exatamente uma nota por elemento `<note>`, na ordem do arquivo. Partes sem compasso não entram
/// no modelo de tocar e são puladas aqui também.
fn montar_ids(p: &Partitura, doc: &Documento) -> Result<IdsPorNota, ErroServico> {
    let incons = |m: String| ErroServico::Inconsistente(m);
    let do_doc: Vec<_> = doc
        .partes()
        .iter()
        .filter(|parte| !parte.compassos.is_empty())
        .collect();
    if do_doc.len() != p.partes.len() {
        return Err(incons(format!(
            "{} partes no documento, {} na partitura",
            do_doc.len(),
            p.partes.len()
        )));
    }
    let mut partes = Vec::with_capacity(p.partes.len());
    for (pi, (pt, pd)) in p.partes.iter().zip(&do_doc).enumerate() {
        let mut compassos = Vec::with_capacity(pt.compassos.len());
        for (ci, ct) in pt.compassos.iter().enumerate() {
            // A partitura completa as partes curtas com compassos vazios.
            let notas_doc = pd.compassos.get(ci).map_or(&[][..], |c| &c.notas[..]);
            if notas_doc.len() != ct.notas.len() {
                return Err(incons(format!(
                    "parte {pi}, compasso {ci}: {} notas no documento, {} na partitura",
                    notas_doc.len(),
                    ct.notas.len()
                )));
            }
            compassos.push(notas_doc.iter().map(|n| n.id).collect());
        }
        partes.push(compassos);
    }
    Ok(IdsPorNota { partes })
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::mxl::empacotar_mxl;

    const DUAS_PARTES: &str = "<?xml version=\"1.0\"?>\n<score-partwise version=\"4.0\">\n\
<part-list><score-part id=\"P1\"><part-name>A</part-name></score-part>\
<score-part id=\"P2\"><part-name>B</part-name></score-part></part-list>\n\
<part id=\"P1\"><measure number=\"1\"><attributes><divisions>1</divisions></attributes>\
<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><type>half</type></note>\
<note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><type>half</type></note>\
<note><rest/><duration>2</duration><type>half</type></note></measure></part>\n\
<part id=\"P2\"><measure number=\"1\"><attributes><divisions>1</divisions></attributes>\
<note><pitch><step>G</step><octave>3</octave></pitch><duration>4</duration><type>whole</type></note>\
</measure></part>\n</score-partwise>\n";

    #[test]
    fn formato_pela_extensao() {
        assert_eq!(Formato::de_extensao("MusicXML"), Some(Formato::MusicXml));
        assert_eq!(Formato::de_extensao("xml"), Some(Formato::MusicXml));
        assert_eq!(Formato::de_extensao("MXL"), Some(Formato::Mxl));
        assert_eq!(Formato::de_extensao("txt"), None);
        assert_eq!(Formato::de_extensao(""), None);
    }

    #[test]
    fn abre_e_exporta_identico_com_ids_em_ordem_de_documento() {
        let d = DocumentoAberto::abrir_texto("duas", DUAS_PARTES).expect("abre");
        assert_eq!(d.nome(), "duas");
        assert_eq!(d.exportar().expect("exporta"), DUAS_PARTES);
        assert!(!d.editado());
        assert_eq!(d.ids().total(), 4);
        // Monotônico a partir de 1, na ordem do arquivo: parte 0 (3 notas), depois parte 1.
        assert_eq!(d.ids().id(0, 0, 0), Some(IdNota(1)));
        assert_eq!(d.ids().id(0, 0, 2), Some(IdNota(3)));
        assert_eq!(d.ids().id(1, 0, 0), Some(IdNota(4)));
        assert_eq!(d.ids().id(1, 0, 1), None);
        assert_eq!(d.ids().id(2, 0, 0), None);
        d.conferir_exportacao(DUAS_PARTES).expect("confere");
    }

    #[test]
    fn mxl_entrega_o_mesmo_xml_e_exporta_o_xml() {
        let zip = empacotar_mxl("a.musicxml", DUAS_PARTES).expect("zip");
        let d = DocumentoAberto::abrir_bytes("a.mxl", &zip, Formato::Mxl).expect("abre");
        assert_eq!(d.formato(), Formato::Mxl);
        assert_eq!(d.texto_original(), DUAS_PARTES);
        assert_eq!(d.exportar().expect("exporta"), DUAS_PARTES);
    }

    #[test]
    fn recusas_continuam_tipadas_e_com_a_mensagem_de_sempre() {
        let e = DocumentoAberto::abrir_texto("x", "").expect_err("vazio");
        assert!(matches!(e, ErroServico::Partitura(_)), "{e:?}");
        let e = DocumentoAberto::abrir_bytes("x", b"isto nao e zip", Formato::Mxl)
            .expect_err("não é zip");
        assert!(e.to_string().contains("arquivo .mxl recusado"), "{e}");
        let e = DocumentoAberto::abrir_bytes("x", &[0xFF, 0xFE, 0x3C, 0x00], Formato::MusicXml)
            .expect_err("utf-16");
        assert!(
            matches!(e, ErroServico::Documento(ErroDocumento::NaoUtf8(_))),
            "{e:?}"
        );
        assert!(e.to_string().contains("UTF-8"), "{e}");
    }

    #[test]
    fn bom_utf8_abre_e_volta_identico() {
        let com_bom = format!("\u{feff}{DUAS_PARTES}");
        let d =
            DocumentoAberto::abrir_bytes("b", com_bom.as_bytes(), Formato::MusicXml).expect("abre");
        assert_eq!(d.exportar().expect("exporta"), com_bom);
    }

    #[test]
    fn conferir_detecta_documento_diferente_e_texto_invalido() {
        let d = DocumentoAberto::abrir_texto("duas", DUAS_PARTES).expect("abre");
        let outro = DUAS_PARTES.replace("<step>G</step>", "<step>A</step>");
        let e = d.conferir_exportacao(&outro).expect_err("difere");
        assert!(matches!(e, ErroServico::ReleituraDifere(_)), "{e:?}");
        assert!(d.conferir_exportacao("<x/>").is_err());
        // Mesmo modelo, mas bytes diferentes (espaço a mais): sem edição, o original manda.
        let espacado = DUAS_PARTES.replace("</score-partwise>", "</score-partwise>\n\n");
        let e = d.conferir_exportacao(&espacado).expect_err("bytes");
        assert!(e.to_string().contains("bytes"), "{e}");
    }
}
