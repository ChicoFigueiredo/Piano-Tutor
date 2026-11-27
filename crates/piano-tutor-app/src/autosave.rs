//! Autosave e recuperação após queda (ticket 24, A7): o trabalho em andamento da Oficina é
//! gravado sozinho, em intervalos curtos, num **rascunho de recuperação** separado das Versões.
//!
//! Estado puro, sem janela nem egui (seam S1); o relógio é injetado (milissegundos monotônicos
//! passados por quem chama), então os testes não dormem.
//!
//! # Contrato
//!
//! - **Nunca é Versão e nunca toca o original.** O autosave só escreve na própria pasta
//!   `<oficina>/partituras/<id>.recuperacao/` (dentro da pasta de dados do usuário, por
//!   documento: partituras e usuários diferentes não se misturam). A Hipótese, o PDF, o
//!   `estado.toml` e `<id>.versoes/` nunca são abertos para escrita aqui.
//! - **Formato.** Um único arquivo, `rascunho.musicxml.rec`: primeira linha
//!   `PIANO-TUTOR-RASCUNHO 1 <sha256> <bytes> <em>`, depois o MusicXML exportado. O hash e o
//!   tamanho do cabeçalho detectam arquivo truncado ou adulterado (rascunho inválido é ignorado
//!   e apagado, nunca oferecido).
//! - **Atômico.** Grava em `.rascunho.musicxml.rec.<pid>.tmp`, `fsync` e `rename`. Uma queda
//!   antes do `rename` deixa o rascunho anterior intacto (e, no pior caso, um `.tmp` órfão, que a
//!   próxima gravação apaga).
//! - **Só com edição.** [`Autosave::devido`] é falso enquanto o documento não está modificado
//!   (ticket 15, `modificado`); a contagem do intervalo começa na primeira edição pendente. Texto
//!   igual ao último rascunho não regrava. A perda máxima é o intervalo configurado.
//! - **Não bloqueia a interface.** A tela exporta o texto (barato) e pode entregar a gravação a
//!   uma thread por [`Autosave::gravador`] (clonável e `Send`), avisando o resultado por
//!   [`Autosave::gravado`]; [`Autosave::tick`] é a forma síncrona, para testes e para quem não
//!   precisa de thread.
//! - **Ciclo de vida.** Ao reabrir, [`Autosave::pendente`] oferece a recuperação. O rascunho é
//!   apagado por [`Autosave::ao_salvar_versao`] (depois de salvar uma Versão) ou por
//!   [`Autosave::descartar`] (recusar a recuperação ou descartar o trabalho). Recusar não apaga
//!   o original nem as Versões. Enquanto a decisão pende, `tick` não grava nem apaga nada.
//! - **Decisão registrada: recupera só o documento.** O histórico de desfazer não é gravado; ao
//!   recuperar, [`Recuperado::aviso_undo`] traz o aviso explícito de que o desfazer foi perdido.
//!   O documento recuperado difere do arquivo salvo e o chamador deve tratá-lo como modificado.
//! - **Configuração.** [`ConfigAutosave`]: intervalo (padrão 30 s, mínimo 1 s) e pasta base
//!   (padrão: ao lado da partitura; sempre `<id>.recuperacao` dentro dela). Desligar = não
//!   chamar `tick`. A ligação à tela e às Preferências não faz parte deste ticket.

use piano_tutor_core::documento::DocumentoAberto;
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// Intervalo padrão do autosave, em milissegundos.
pub const INTERVALO_PADRAO_MS: u64 = 30_000;
/// Menor intervalo aceito, em milissegundos.
pub const INTERVALO_MINIMO_MS: u64 = 1_000;
/// Nome do arquivo de rascunho dentro da pasta de recuperação.
pub const ARQUIVO_DO_RASCUNHO: &str = "rascunho.musicxml.rec";
/// Tamanho máximo de um rascunho.
pub const MAX_BYTES_DO_RASCUNHO: usize = 64 * 1024 * 1024;
const MAGICA: &str = "PIANO-TUTOR-RASCUNHO 1";

/// Aviso quando o histórico de desfazer não volta com a recuperação.
pub const AVISO_UNDO_PERDIDO: &str =
    "Trabalho recuperado. O histórico de desfazer não foi guardado e começa vazio.";

/// Configuração do autosave.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigAutosave {
    pub intervalo_ms: u64,
    /// Pasta base da recuperação; `None` = ao lado da partitura.
    pub pasta: Option<PathBuf>,
}

impl Default for ConfigAutosave {
    fn default() -> Self {
        Self {
            intervalo_ms: INTERVALO_PADRAO_MS,
            pasta: None,
        }
    }
}

/// Falha do autosave.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ErroAutosave {
    Disco(String),
    /// Identificador de partitura que sairia da pasta (separador, `..`, vazio).
    IdInvalido(String),
    ConteudoInvalido(String),
    /// A recuperação foi pedida sem rascunho íntegro.
    SemRascunho,
    /// O rascunho não abre como documento.
    NaoAbre(String),
}

impl fmt::Display for ErroAutosave {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disco(e) => write!(f, "problema no disco: {e}"),
            Self::IdInvalido(i) => write!(f, "identificador de partitura inválido: {i:?}"),
            Self::ConteudoInvalido(e) => write!(f, "conteúdo inválido para o rascunho: {e}"),
            Self::SemRascunho => write!(f, "não há rascunho de recuperação"),
            Self::NaoAbre(e) => write!(f, "o rascunho não abre: {e}"),
        }
    }
}

impl std::error::Error for ErroAutosave {}

fn disco(e: &std::io::Error) -> ErroAutosave {
    ErroAutosave::Disco(e.to_string())
}

/// Ponto de queda simulado na gravação (testes).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Queda {
    Nenhuma,
    /// O temporário fica pela metade e o `rename` não acontece.
    NoMeioDoTemporario,
    /// O temporário fica completo, mas o `rename` não acontece.
    AntesDoRename,
}

/// Rascunho lido e verificado.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rascunho {
    pub texto: String,
    pub hash: String,
    /// Instante da gravação, como o chamador informou (ISO-8601 UTC).
    pub em: String,
}

/// Documento recuperado e o aviso de que o desfazer foi perdido.
pub struct Recuperado {
    pub documento: DocumentoAberto,
    pub aviso_undo: &'static str,
}

/// O que um `tick` fez.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultadoDoTick {
    /// Nada a fazer (sem edição, antes do intervalo, ou decisão de recuperação pendente).
    Nada,
    Gravado,
    /// Texto igual ao último rascunho: não regravou.
    SemMudanca,
}

/// Gravador isolado do estado: pode ir a uma thread.
#[derive(Clone, Debug)]
pub struct GravadorDeRascunho {
    pasta: PathBuf,
}

impl GravadorDeRascunho {
    fn arquivo(&self) -> PathBuf {
        self.pasta.join(ARQUIVO_DO_RASCUNHO)
    }

    /// Grava `texto` atomicamente; devolve o hash gravado.
    pub fn gravar(&self, texto: &str, em: &str) -> Result<String, ErroAutosave> {
        self.gravar_com(texto, em, Queda::Nenhuma)
    }

    pub(crate) fn gravar_com(
        &self,
        texto: &str,
        em: &str,
        queda: Queda,
    ) -> Result<String, ErroAutosave> {
        if texto.trim().is_empty() || texto.len() > MAX_BYTES_DO_RASCUNHO {
            return Err(ErroAutosave::ConteudoInvalido(
                "vazio ou grande demais".to_owned(),
            ));
        }
        if em.contains(['\n', '\r']) {
            return Err(ErroAutosave::ConteudoInvalido("data inválida".to_owned()));
        }
        fs::create_dir_all(&self.pasta).map_err(|e| disco(&e))?;
        self.limpar_temporarios();
        let hash = hash_do_texto(texto);
        let dados = format!("{MAGICA} {hash} {} {em}\n{texto}", texto.len());
        let destino = self.arquivo();
        let tmp = self
            .pasta
            .join(format!(".{ARQUIVO_DO_RASCUNHO}.{}.tmp", std::process::id()));
        let r = (|| -> std::io::Result<()> {
            let mut f = fs::File::create(&tmp)?;
            if queda == Queda::NoMeioDoTemporario {
                f.write_all(&dados.as_bytes()[..dados.len() / 2])?;
                return Err(std::io::Error::other("queda simulada"));
            }
            f.write_all(dados.as_bytes())?;
            f.sync_all()?;
            if queda == Queda::AntesDoRename {
                return Err(std::io::Error::other("queda simulada"));
            }
            fs::rename(&tmp, &destino)
        })();
        match r {
            Ok(()) => Ok(hash),
            Err(e) => {
                // Uma queda real não limparia nada: só o erro comum limpa o temporário.
                if queda == Queda::Nenhuma {
                    let _ = fs::remove_file(&tmp);
                }
                Err(disco(&e))
            }
        }
    }

    fn limpar_temporarios(&self) {
        let Ok(it) = fs::read_dir(&self.pasta) else {
            return;
        };
        let prefixo = format!(".{ARQUIVO_DO_RASCUNHO}.");
        for e in it.flatten() {
            let nome = e.file_name().to_string_lossy().into_owned();
            if nome.starts_with(&prefixo) && nome.ends_with(".tmp") {
                let _ = fs::remove_file(e.path());
            }
        }
    }

    fn ler(&self) -> Option<Rascunho> {
        let bytes = fs::read(self.arquivo()).ok()?;
        let tudo = String::from_utf8(bytes).ok()?;
        let (cab, texto) = tudo.split_once('\n')?;
        let resto = cab.strip_prefix(MAGICA)?.strip_prefix(' ')?;
        let mut p = resto.splitn(3, ' ');
        let hash = p.next()?;
        let tam: usize = p.next()?.parse().ok()?;
        let em = p.next()?;
        if texto.len() != tam || hash_do_texto(texto) != hash {
            return None;
        }
        Some(Rascunho {
            texto: texto.to_owned(),
            hash: hash.to_owned(),
            em: em.to_owned(),
        })
    }

    fn apagar(&self) -> Result<(), ErroAutosave> {
        match fs::remove_file(self.arquivo()) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(disco(&e)),
        }
        self.limpar_temporarios();
        Ok(())
    }
}

fn hash_do_texto(texto: &str) -> String {
    Sha256::digest(texto.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// O autosave de uma partitura.
#[derive(Clone, Debug)]
pub struct Autosave {
    gravador: GravadorDeRascunho,
    intervalo_ms: u64,
    /// Início da contagem do intervalo (primeira edição pendente ou última gravação).
    marco_ms: Option<u64>,
    ultimo_hash: Option<String>,
    /// Há rascunho oferecido e ainda sem decisão: `tick` não mexe em nada.
    decisao_pendente: bool,
    /// O trabalho foi recuperado e ainda não virou Versão: o rascunho fica.
    recuperado: bool,
}

impl Autosave {
    /// Autosave da partitura `id` que mora em `pasta_partituras`.
    pub fn em(
        pasta_partituras: &Path,
        id: &str,
        config: &ConfigAutosave,
    ) -> Result<Self, ErroAutosave> {
        if id.is_empty()
            || id == "."
            || id == ".."
            || id.contains(['/', '\\', '\0'])
            || Path::new(id).is_absolute()
        {
            return Err(ErroAutosave::IdInvalido(id.to_owned()));
        }
        let base = config.pasta.as_deref().unwrap_or(pasta_partituras);
        Ok(Self {
            gravador: GravadorDeRascunho {
                pasta: base.join(format!("{id}.recuperacao")),
            },
            intervalo_ms: config.intervalo_ms.max(INTERVALO_MINIMO_MS),
            marco_ms: None,
            ultimo_hash: None,
            decisao_pendente: false,
            recuperado: false,
        })
    }

    /// Pasta de recuperação deste documento.
    pub fn pasta(&self) -> &Path {
        &self.gravador.pasta
    }

    pub fn intervalo_ms(&self) -> u64 {
        self.intervalo_ms
    }

    /// Gravador clonável para rodar fora da thread da interface.
    pub fn gravador(&self) -> GravadorDeRascunho {
        self.gravador.clone()
    }

    /// Ao reabrir: há rascunho íntegro a recuperar? Rascunho inválido (truncado) é apagado.
    /// Enquanto a decisão não vem, [`Self::tick`] fica quieto.
    pub fn pendente(&mut self) -> Result<Option<Rascunho>, ErroAutosave> {
        if let Some(r) = self.gravador.ler() {
            self.decisao_pendente = true;
            Ok(Some(r))
        } else {
            if self.gravador.arquivo().exists() {
                self.gravador.apagar()?;
            }
            Ok(None)
        }
    }

    /// Aceita a recuperação: abre o rascunho como documento. O rascunho é mantido até salvar
    /// uma Versão ou descartar.
    pub fn recuperar(&mut self, nome: &str) -> Result<Recuperado, ErroAutosave> {
        let r = self.gravador.ler().ok_or(ErroAutosave::SemRascunho)?;
        let documento = DocumentoAberto::abrir_texto(nome, &r.texto)
            .map_err(|e| ErroAutosave::NaoAbre(e.to_string()))?;
        self.decisao_pendente = false;
        self.recuperado = true;
        self.ultimo_hash = Some(r.hash);
        self.marco_ms = None;
        Ok(Recuperado {
            documento,
            aviso_undo: AVISO_UNDO_PERDIDO,
        })
    }

    /// Descarta o rascunho (recusar a recuperação ou descartar o trabalho). Só apaga a pasta de
    /// recuperação: o original e as Versões ficam como estão.
    pub fn descartar(&mut self) -> Result<(), ErroAutosave> {
        self.gravador.apagar()?;
        self.decisao_pendente = false;
        self.recuperado = false;
        self.ultimo_hash = None;
        self.marco_ms = None;
        Ok(())
    }

    /// Chamar depois de salvar uma Versão com sucesso: o rascunho some.
    pub fn ao_salvar_versao(&mut self) -> Result<(), ErroAutosave> {
        self.descartar()
    }

    /// Já é hora de gravar? Conta o intervalo a partir da primeira edição pendente.
    pub fn devido(&mut self, agora_ms: u64, modificado: bool) -> bool {
        if self.decisao_pendente {
            return false;
        }
        if !modificado {
            self.marco_ms = None;
            return false;
        }
        let marco = *self.marco_ms.get_or_insert(agora_ms);
        agora_ms.saturating_sub(marco) >= self.intervalo_ms
    }

    /// Registra o resultado de uma gravação feita por [`Self::gravador`] fora da thread da UI.
    pub fn gravado(&mut self, hash: String, agora_ms: u64) {
        self.ultimo_hash = Some(hash);
        self.marco_ms = Some(agora_ms);
    }

    /// Forma síncrona: se for hora, exporta o documento e grava o rascunho. Se o documento
    /// voltou ao estado salvo, o rascunho obsoleto some (salvo recuperação ainda não salva).
    pub fn tick(
        &mut self,
        agora_ms: u64,
        documento: &DocumentoAberto,
        em: &str,
    ) -> Result<ResultadoDoTick, ErroAutosave> {
        self.tick_com(agora_ms, documento, em, Queda::Nenhuma)
    }

    pub(crate) fn tick_com(
        &mut self,
        agora_ms: u64,
        documento: &DocumentoAberto,
        em: &str,
        queda: Queda,
    ) -> Result<ResultadoDoTick, ErroAutosave> {
        let modificado = documento.modificado();
        if !modificado && !self.decisao_pendente && !self.recuperado && self.ultimo_hash.is_some() {
            self.gravador.apagar()?;
            self.ultimo_hash = None;
        }
        if !self.devido(agora_ms, modificado) {
            return Ok(ResultadoDoTick::Nada);
        }
        let texto = documento
            .exportar()
            .map_err(|e| ErroAutosave::ConteudoInvalido(e.to_string()))?;
        self.marco_ms = Some(agora_ms);
        if self.ultimo_hash.as_deref() == Some(hash_do_texto(&texto).as_str()) {
            return Ok(ResultadoDoTick::SemMudanca);
        }
        let hash = self.gravador.gravar_com(&texto, em, queda)?;
        self.ultimo_hash = Some(hash);
        Ok(ResultadoDoTick::Gravado)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::versoes::Versoes;
    use piano_tutor_core::documento::{ComandoDeEdicao, SentidoDaAltura, UnidadeDeAltura};

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="3.1"><part-list><score-part id="P1"><part-name>P</part-name></score-part></part-list>
<part id="P1"><measure number="1"><attributes><divisions>1</divisions><key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes>
<note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note></measure></part></score-partwise>"#;
    const EM: &str = "2026-11-27T10:00:00Z";

    struct Cenario {
        _d: tempfile::TempDir,
        partituras: PathBuf,
        a: Autosave,
    }

    fn cenario() -> Cenario {
        let d = tempfile::tempdir().expect("pasta");
        let partituras = d.path().join("oficina/partituras");
        fs::create_dir_all(&partituras).expect("cria");
        fs::write(partituras.join("ode.musicxml"), XML).expect("hipotese");
        let a = Autosave::em(&partituras, "ode", &ConfigAutosave::default()).expect("autosave");
        Cenario {
            _d: d,
            partituras,
            a,
        }
    }

    fn editar(d: &mut DocumentoAberto, unidade: UnidadeDeAltura) {
        let nota = d.ids().id(0, 0, 0).expect("nota");
        d.aplicar(ComandoDeEdicao::MudarAltura {
            nota,
            unidade,
            sentido: SentidoDaAltura::Acima,
        })
        .expect("edita");
    }

    fn editado() -> DocumentoAberto {
        let mut d = DocumentoAberto::abrir_texto("ode", XML).expect("abre");
        editar(&mut d, UnidadeDeAltura::Semitom);
        assert!(d.modificado());
        d
    }

    #[test]
    fn so_grava_com_edicao_e_apos_o_intervalo() {
        let mut c = cenario();
        let limpo = DocumentoAberto::abrir_texto("ode", XML).expect("abre");
        assert_eq!(c.a.tick(0, &limpo, EM), Ok(ResultadoDoTick::Nada));
        assert_eq!(c.a.tick(999_999, &limpo, EM), Ok(ResultadoDoTick::Nada));
        assert!(!c.a.pasta().exists(), "sem edição não cria nada");

        let d = editado();
        assert_eq!(c.a.tick(1_000_000, &d, EM), Ok(ResultadoDoTick::Nada));
        assert_eq!(
            c.a.tick(1_000_000 + INTERVALO_PADRAO_MS - 1, &d, EM),
            Ok(ResultadoDoTick::Nada)
        );
        assert_eq!(
            c.a.tick(1_000_000 + INTERVALO_PADRAO_MS, &d, EM),
            Ok(ResultadoDoTick::Gravado)
        );
        // Mesmo texto: não regrava.
        assert_eq!(
            c.a.tick(1_000_000 + 2 * INTERVALO_PADRAO_MS, &d, EM),
            Ok(ResultadoDoTick::SemMudanca)
        );
    }

    #[test]
    fn queda_apos_autosave_oferece_recuperacao_equivalente_ao_ultimo() {
        let mut c = cenario();
        let d = editado();
        let esperado = d.exportar().expect("exporta");
        c.a.tick(0, &d, EM).expect("t0");
        c.a.tick(INTERVALO_PADRAO_MS, &d, EM).expect("grava");
        drop(d); // processo morto: sem fechar, sem salvar Versão

        let mut reaberto =
            Autosave::em(&c.partituras, "ode", &ConfigAutosave::default()).expect("reabre");
        let r = reaberto.pendente().expect("le").expect("oferece");
        assert_eq!(r.texto, esperado);
        assert_eq!(r.em, EM);
        let rec = reaberto.recuperar("ode").expect("recupera");
        assert_eq!(rec.documento.exportar().expect("exporta"), esperado);
        assert_eq!(rec.aviso_undo, AVISO_UNDO_PERDIDO);
        assert!(c.a.pasta().join(ARQUIVO_DO_RASCUNHO).exists());
    }

    #[test]
    fn perda_maxima_e_o_intervalo() {
        let mut c = cenario();
        let mut d = editado();
        c.a.tick(0, &d, EM).expect("t0");
        c.a.tick(INTERVALO_PADRAO_MS, &d, EM).expect("grava");
        let gravado = d.exportar().expect("exporta");
        // Nova edição antes do próximo intervalo: ainda não está no rascunho.
        editar(&mut d, UnidadeDeAltura::Oitava);
        assert_eq!(
            c.a.tick(INTERVALO_PADRAO_MS + 1, &d, EM),
            Ok(ResultadoDoTick::Nada)
        );
        let mut reaberto =
            Autosave::em(&c.partituras, "ode", &ConfigAutosave::default()).expect("reabre");
        assert_eq!(
            reaberto.pendente().expect("le").expect("oferece").texto,
            gravado
        );
        assert_ne!(gravado, d.exportar().expect("exporta"));
        // Passado mais um intervalo, a edição entra no rascunho.
        c.a.tick(2 * INTERVALO_PADRAO_MS + 1, &d, EM)
            .expect("grava");
        let mut r2 =
            Autosave::em(&c.partituras, "ode", &ConfigAutosave::default()).expect("reabre");
        assert_eq!(
            r2.pendente().expect("le").expect("oferece").texto,
            d.exportar().expect("exporta")
        );
    }

    #[test]
    fn intervalo_configuravel_e_com_minimo() {
        let d = tempfile::tempdir().expect("pasta");
        let cfg = ConfigAutosave {
            intervalo_ms: 5,
            pasta: Some(d.path().join("rec")),
        };
        let a = Autosave::em(d.path(), "ode", &cfg).expect("autosave");
        assert_eq!(a.intervalo_ms(), INTERVALO_MINIMO_MS);
        assert_eq!(a.pasta(), d.path().join("rec/ode.recuperacao"));
        let b = Autosave::em(
            d.path(),
            "ode",
            &ConfigAutosave {
                intervalo_ms: 2_000,
                pasta: None,
            },
        )
        .expect("autosave");
        assert_eq!(b.intervalo_ms(), 2_000);
    }

    #[test]
    fn queda_durante_a_escrita_preserva_o_rascunho_anterior() {
        for queda in [Queda::NoMeioDoTemporario, Queda::AntesDoRename] {
            let c = cenario();
            let g = c.a.gravador();
            g.gravar("<score-partwise><!-- antigo --></score-partwise>", EM)
                .expect("primeiro");
            let r = g.gravar_com("<score-partwise><!-- novo --></score-partwise>", EM, queda);
            assert!(r.is_err(), "{queda:?}");
            let lido = g.ler().expect("anterior íntegro");
            assert!(lido.texto.contains("antigo"), "{queda:?}");
            // A próxima gravação limpa o temporário órfão.
            g.gravar("<score-partwise><!-- novo --></score-partwise>", EM)
                .expect("depois");
            let tmps = fs::read_dir(c.a.pasta())
                .expect("lista")
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
                .count();
            assert_eq!(tmps, 0);
            assert!(g.ler().expect("novo").texto.contains("novo"));
        }
    }

    #[test]
    fn queda_na_primeira_gravacao_nao_oferece_nada() {
        let mut c = cenario();
        let _ =
            c.a.gravador()
                .gravar_com("<a>x</a>", EM, Queda::NoMeioDoTemporario);
        assert_eq!(c.a.pendente(), Ok(None));
    }

    #[test]
    fn rascunho_truncado_e_ignorado_e_apagado() {
        let mut c = cenario();
        c.a.gravador()
            .gravar("<score-partwise>texto</score-partwise>", EM)
            .expect("grava");
        let arq = c.a.pasta().join(ARQUIVO_DO_RASCUNHO);
        let bytes = fs::read(&arq).expect("le");
        fs::write(&arq, &bytes[..bytes.len() - 5]).expect("trunca");
        assert_eq!(c.a.pendente(), Ok(None));
        assert!(!arq.exists());
    }

    #[test]
    fn salvar_versao_apaga_o_rascunho_e_descartar_nao_toca_originais_nem_versoes() {
        let mut c = cenario();
        let d = editado();
        c.a.tick(0, &d, EM).expect("t0");
        c.a.tick(INTERVALO_PADRAO_MS, &d, EM).expect("grava");
        let arq = c.a.pasta().join(ARQUIVO_DO_RASCUNHO);
        assert!(arq.exists());

        let v = Versoes::em(&c.partituras, "ode", None);
        v.registrar_origem("t", EM).expect("origem");
        let antes = v.listar().expect("lista");
        let texto = d.exportar().expect("exporta");
        v.salvar(None, &texto, None, "t", EM).expect("salva");
        let versoes = v.listar().expect("lista");
        // O autosave nunca criou Versão: só a que o usuário salvou.
        assert_eq!(versoes.len(), antes.len() + 1);
        c.a.ao_salvar_versao().expect("apaga");
        assert!(!arq.exists());
        assert_eq!(v.listar().expect("lista"), versoes);

        // Recusar a recuperação: apaga só o rascunho.
        c.a.gravador().gravar(&texto, EM).expect("de novo");
        let hipotese = fs::read(c.partituras.join("ode.musicxml")).expect("le");
        assert!(c.a.pendente().expect("le").is_some());
        c.a.descartar().expect("recusa");
        assert!(!arq.exists());
        assert_eq!(
            fs::read(c.partituras.join("ode.musicxml")).expect("le"),
            hipotese
        );
        assert_eq!(v.listar().expect("lista"), versoes);
        assert_eq!(c.a.pendente(), Ok(None));
    }

    #[test]
    fn decisao_pendente_congela_o_tick() {
        let mut c = cenario();
        c.a.gravador().gravar(XML, EM).expect("rascunho");
        assert!(c.a.pendente().expect("le").is_some());
        let limpo = DocumentoAberto::abrir_texto("ode", XML).expect("abre");
        assert_eq!(c.a.tick(10, &limpo, EM), Ok(ResultadoDoTick::Nada));
        assert!(c.a.pasta().join(ARQUIVO_DO_RASCUNHO).exists());
    }

    #[test]
    fn voltar_ao_estado_salvo_apaga_rascunho_obsoleto() {
        let mut c = cenario();
        let mut d = editado();
        c.a.tick(0, &d, EM).expect("t0");
        c.a.tick(INTERVALO_PADRAO_MS, &d, EM).expect("grava");
        d.desfazer().expect("desfaz");
        assert!(!d.modificado());
        c.a.tick(INTERVALO_PADRAO_MS + 1, &d, EM).expect("tick");
        assert!(!c.a.pasta().join(ARQUIVO_DO_RASCUNHO).exists());
    }

    #[test]
    fn pasta_por_documento_e_id_seguro() {
        let c = cenario();
        let outro = Autosave::em(&c.partituras, "outra", &ConfigAutosave::default()).expect("ok");
        assert_ne!(outro.pasta(), c.a.pasta());
        for ruim in ["", "..", "a/b", "a\\b", "/abs"] {
            assert!(matches!(
                Autosave::em(&c.partituras, ruim, &ConfigAutosave::default()),
                Err(ErroAutosave::IdInvalido(_))
            ));
        }
    }
}
