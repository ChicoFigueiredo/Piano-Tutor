//! Log NDJSON anonimizado por sessão: uma linha JSON por evento.
//!
//! Campos comuns: `ts_ns` (monotônico, base do [`crate::Relogio`]) e `tipo`.
//! Canais são gravados como exibidos (1–16). Nomes de porta e mensagens de
//! erro passam por [`piano_tutor_core::sanitizar::sanitizar`]. Bytes de SysEx
//! e de mensagens desconhecidas **não** são gravados (podem conter seriais);
//! só o byte de status.
//!
//! Falhas de escrita nunca interrompem a captura: são contadas e a última é
//! guardada em [`RegistroNdjson::ultimo_erro`].
//!
//! | `tipo` | campos extras |
//! | --- | --- |
//! | `sessao` | `formato`, `versao` |
//! | `nota_ligada`, `nota_desligada` | `canal`, `nota`, `velocidade` |
//! | `controle` | `canal`, `controle`, `valor` |
//! | `programa` | `canal`, `programa` |
//! | `pitch_bend` | `canal`, `valor` (14 bits, 0–16383, centro 8192) |
//! | `desconhecido` | `status` (ou ausente) |
//! | `erro_midi` | `erro`, `tamanho` |
//! | `conectado` | `porta`, `reconexao` |
//! | `desconectado` | `porta`, `motivo` |
//! | `erro_conexao` | `porta`, `mensagem` |

use piano_tutor_core::midi::{ErroMidi, EventoMidi};
use piano_tutor_core::sanitizar::sanitizar;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Versão do formato das linhas.
pub const FORMATO: u32 = 1;

/// Motivo de uma desconexão registrada.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MotivoDesconexao {
    /// A porta sumiu da listagem (dispositivo desplugado).
    PortaAusente,
    /// O usuário pediu.
    Manual,
    /// O usuário escolheu outra porta.
    TrocaDePorta,
    /// Reconexão forçada pelo usuário.
    ReconexaoForcada,
}

impl MotivoDesconexao {
    #[must_use]
    pub const fn como_texto(self) -> &'static str {
        match self {
            Self::PortaAusente => "porta_ausente",
            Self::Manual => "manual",
            Self::TrocaDePorta => "troca_de_porta",
            Self::ReconexaoForcada => "reconexao_forcada",
        }
    }
}

pub struct RegistroNdjson<W: Write> {
    saida: W,
    linhas: u64,
    falhas: u64,
    ultimo_erro: Option<String>,
}

impl<W: Write> RegistroNdjson<W> {
    pub fn novo(saida: W) -> Self {
        Self {
            saida,
            linhas: 0,
            falhas: 0,
            ultimo_erro: None,
        }
    }

    /// Linhas escritas com sucesso.
    pub fn linhas(&self) -> u64 {
        self.linhas
    }

    /// Linhas perdidas por falha de escrita.
    pub fn falhas(&self) -> u64 {
        self.falhas
    }

    pub fn ultimo_erro(&self) -> Option<&str> {
        self.ultimo_erro.as_deref()
    }

    pub fn saida(&self) -> &W {
        &self.saida
    }

    pub fn em_saida(self) -> W {
        self.saida
    }

    pub fn registrar_inicio_sessao(&mut self, ts_ns: u64) {
        let mut l = Linha::nova(ts_ns, "sessao");
        l.numero("formato", u64::from(FORMATO));
        l.texto("versao", env!("CARGO_PKG_VERSION"));
        self.escrever(l);
    }

    pub fn registrar_evento(&mut self, evento: &EventoMidi, status: Option<u8>) {
        let ts = evento.timestamp_monotonico_ns();
        let l = match *evento {
            EventoMidi::NotaLigada {
                canal,
                nota,
                velocidade,
                ..
            }
            | EventoMidi::NotaDesligada {
                canal,
                nota,
                velocidade,
                ..
            } => {
                let tipo = if matches!(evento, EventoMidi::NotaLigada { .. }) {
                    "nota_ligada"
                } else {
                    "nota_desligada"
                };
                let mut l = Linha::nova(ts, tipo);
                l.numero("canal", u64::from(canal.exibicao()));
                l.numero("nota", u64::from(nota.numero()));
                l.numero("velocidade", u64::from(velocidade));
                l
            }
            EventoMidi::ControleMudou {
                canal,
                controle,
                valor,
                ..
            } => {
                let mut l = Linha::nova(ts, "controle");
                l.numero("canal", u64::from(canal.exibicao()));
                l.numero("controle", u64::from(controle));
                l.numero("valor", u64::from(valor));
                l
            }
            EventoMidi::ProgramaMudou {
                canal, programa, ..
            } => {
                let mut l = Linha::nova(ts, "programa");
                l.numero("canal", u64::from(canal.exibicao()));
                l.numero("programa", u64::from(programa));
                l
            }
            EventoMidi::PitchBend { canal, valor, .. } => {
                let mut l = Linha::nova(ts, "pitch_bend");
                l.numero("canal", u64::from(canal.exibicao()));
                // Registra o valor de 14 bits (0..=16383), sem sinal.
                l.numero("valor", u64::try_from(i32::from(valor) + 8192).unwrap_or(0));
                l
            }
            EventoMidi::Desconhecido { .. } => {
                let mut l = Linha::nova(ts, "desconhecido");
                if let Some(s) = status.filter(|s| s & 0x80 != 0) {
                    l.numero("status", u64::from(s));
                }
                l
            }
        };
        self.escrever(l);
    }

    pub fn registrar_erro_midi(&mut self, ts_ns: u64, erro: &ErroMidi, tamanho: usize) {
        let mut l = Linha::nova(ts_ns, "erro_midi");
        l.texto("erro", &erro.to_string());
        l.numero("tamanho", u64::try_from(tamanho).unwrap_or(u64::MAX));
        self.escrever(l);
    }

    pub fn registrar_conexao(&mut self, ts_ns: u64, porta: &str, reconexao: bool) {
        let mut l = Linha::nova(ts_ns, "conectado");
        l.texto("porta", &sanitizar(porta));
        l.booleano("reconexao", reconexao);
        self.escrever(l);
    }

    pub fn registrar_desconexao(&mut self, ts_ns: u64, porta: &str, motivo: MotivoDesconexao) {
        let mut l = Linha::nova(ts_ns, "desconectado");
        l.texto("porta", &sanitizar(porta));
        l.texto("motivo", motivo.como_texto());
        self.escrever(l);
    }

    pub fn registrar_erro_conexao(&mut self, ts_ns: u64, porta: &str, mensagem: &str) {
        let mut l = Linha::nova(ts_ns, "erro_conexao");
        l.texto("porta", &sanitizar(porta));
        l.texto("mensagem", &sanitizar(mensagem));
        self.escrever(l);
    }

    /// Descarrega o buffer do escritor.
    pub fn descarregar(&mut self) {
        if let Err(e) = self.saida.flush() {
            self.falhas = self.falhas.saturating_add(1);
            self.ultimo_erro = Some(e.to_string());
        }
    }

    fn escrever(&mut self, linha: Linha) {
        let mut texto = linha.fechar();
        texto.push('\n');
        match self.saida.write_all(texto.as_bytes()) {
            Ok(()) => self.linhas = self.linhas.saturating_add(1),
            Err(e) => {
                self.falhas = self.falhas.saturating_add(1);
                self.ultimo_erro = Some(e.to_string());
            }
        }
    }
}

/// Montador de um objeto JSON plano (somente números, textos e booleanos).
struct Linha(String);

impl Linha {
    fn nova(ts_ns: u64, tipo: &str) -> Self {
        let mut l = Self(String::with_capacity(96));
        l.0.push('{');
        l.0.push_str("\"ts_ns\":");
        l.0.push_str(&ts_ns.to_string());
        l.texto("tipo", tipo);
        l
    }

    fn chave(&mut self, chave: &str) {
        self.0.push(',');
        escapar_em(&mut self.0, chave);
        self.0.push(':');
    }

    fn numero(&mut self, chave: &str, valor: u64) {
        self.chave(chave);
        self.0.push_str(&valor.to_string());
    }

    fn texto(&mut self, chave: &str, valor: &str) {
        self.chave(chave);
        escapar_em(&mut self.0, valor);
    }

    fn booleano(&mut self, chave: &str, valor: bool) {
        self.chave(chave);
        self.0.push_str(if valor { "true" } else { "false" });
    }

    fn fechar(mut self) -> String {
        self.0.push('}');
        self.0
    }
}

/// Escreve `texto` como string JSON (RFC 8259) entre aspas.
fn escapar_em(saida: &mut String, texto: &str) {
    saida.push('"');
    for c in texto.chars() {
        match c {
            '"' => saida.push_str("\\\""),
            '\\' => saida.push_str("\\\\"),
            '\n' => saida.push_str("\\n"),
            '\r' => saida.push_str("\\r"),
            '\t' => saida.push_str("\\t"),
            c if u32::from(c) < 0x20 => saida.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => saida.push(c),
        }
    }
    saida.push('"');
}

/// Nome do arquivo de sessão: `sessao-midi-<segundos-unix>.ndjson`.
#[must_use]
pub fn nome_arquivo_sessao(inicio: SystemTime) -> String {
    let s = inicio
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("sessao-midi-{s}.ndjson")
}

/// Cria `pasta` se preciso e um arquivo de sessão novo (nunca sobrescreve:
/// se o nome existir, acrescenta `-1`, `-2`, …).
pub fn criar_arquivo_sessao(
    pasta: &Path,
    inicio: SystemTime,
) -> io::Result<(PathBuf, RegistroNdjson<BufWriter<File>>)> {
    std::fs::create_dir_all(pasta)?;
    let base = nome_arquivo_sessao(inicio);
    let raiz = base.trim_end_matches(".ndjson");
    for n in 0..1000u32 {
        let nome = if n == 0 {
            base.clone()
        } else {
            format!("{raiz}-{n}.ndjson")
        };
        let caminho = pasta.join(nome);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&caminho)
        {
            Ok(arquivo) => return Ok((caminho, RegistroNdjson::novo(BufWriter::new(arquivo)))),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "muitos arquivos de sessão com o mesmo nome",
    ))
}

#[cfg(test)]
mod testes {
    use super::*;
    use piano_tutor_core::midi::interpretar;
    use serde_json::Value;

    fn linhas(r: &RegistroNdjson<Vec<u8>>) -> Vec<Value> {
        String::from_utf8(r.saida().clone())
            .expect("UTF-8")
            .lines()
            .map(|l| serde_json::from_str(l).expect("linha JSON válida"))
            .collect()
    }

    #[test]
    fn grava_nota_com_canal_de_exibicao() {
        let mut r = RegistroNdjson::novo(Vec::new());
        let ev = interpretar(&[0x95, 60, 100], 1234).expect("válida");
        r.registrar_evento(&ev, Some(0x95));
        let l = linhas(&r);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0]["ts_ns"], 1234);
        assert_eq!(l[0]["tipo"], "nota_ligada");
        assert_eq!(l[0]["canal"], 6);
        assert_eq!(l[0]["nota"], 60);
        assert_eq!(l[0]["velocidade"], 100);
    }

    #[test]
    fn grava_controle_programa_e_desconhecido_sem_dados() {
        let mut r = RegistroNdjson::novo(Vec::new());
        r.registrar_evento(&interpretar(&[0xB0, 64, 127], 1).expect("cc"), Some(0xB0));
        r.registrar_evento(&interpretar(&[0xC0, 9], 2).expect("pc"), Some(0xC0));
        let sysex = [0xF0, 0x41, 0x12, 0x34, 0x56, 0xF7];
        r.registrar_evento(&interpretar(&sysex, 3).expect("sysex"), Some(0xF0));
        let l = linhas(&r);
        assert_eq!(l[0]["tipo"], "controle");
        assert_eq!(l[0]["controle"], 64);
        assert_eq!(l[0]["valor"], 127);
        assert_eq!(l[1]["tipo"], "programa");
        assert_eq!(l[1]["programa"], 9);
        assert_eq!(l[2]["tipo"], "desconhecido");
        assert_eq!(l[2]["status"], 0xF0);
        let obj = l[2].as_object().expect("objeto");
        assert_eq!(obj.len(), 3, "SysEx não pode vazar bytes de dados: {obj:?}");
    }

    #[test]
    fn porta_e_mensagem_sao_sanitizadas_e_escapadas() {
        let mut r = RegistroNdjson::novo(Vec::new());
        r.registrar_conexao(5, "KB \"330\" SN0123456789 0a12:b3c4 20:0", true);
        r.registrar_erro_conexao(6, "X", "linha1\nlinha2\t\\ A1B2C3D4E5F6");
        let l = linhas(&r);
        assert_eq!(l[0]["porta"], "KB \"330\" [removido] [removido] 20:0");
        assert_eq!(l[0]["reconexao"], true);
        assert_eq!(l[1]["mensagem"], "linha1\nlinha2\t\\ [removido]");
    }

    #[test]
    fn caracteres_de_controle_sao_escapados() {
        let mut s = String::new();
        escapar_em(&mut s, "a\u{1}b");
        assert_eq!(s, "\"a\\u0001b\"");
        let v: Value = serde_json::from_str(&s).expect("JSON");
        assert_eq!(v, "a\u{1}b");
    }

    struct Quebrado;
    impl Write for Quebrado {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("disco cheio"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("disco cheio"))
        }
    }

    #[test]
    fn falha_de_escrita_e_contada_sem_panico() {
        let mut r = RegistroNdjson::novo(Quebrado);
        r.registrar_inicio_sessao(0);
        r.descarregar();
        assert_eq!(r.linhas(), 0);
        assert_eq!(r.falhas(), 2);
        assert_eq!(r.ultimo_erro(), Some("disco cheio"));
    }

    #[test]
    fn nome_de_arquivo_nao_tem_dados_pessoais() {
        let t = UNIX_EPOCH + std::time::Duration::from_secs(1_800_000_000);
        assert_eq!(nome_arquivo_sessao(t), "sessao-midi-1800000000.ndjson");
    }

    #[test]
    fn arquivo_de_sessao_nao_sobrescreve() {
        let pasta = std::env::temp_dir().join(format!(
            "piano-tutor-midi-teste-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&pasta);
        let t = UNIX_EPOCH + std::time::Duration::from_secs(42);
        let (c1, mut r1) = criar_arquivo_sessao(&pasta, t).expect("cria 1");
        r1.registrar_inicio_sessao(0);
        r1.descarregar();
        let (c2, _r2) = criar_arquivo_sessao(&pasta, t).expect("cria 2");
        assert_ne!(c1, c2);
        assert!(c2.ends_with("sessao-midi-42-1.ndjson"));
        let conteudo = std::fs::read_to_string(&c1).expect("lê");
        let v: Value = serde_json::from_str(conteudo.trim()).expect("JSON");
        assert_eq!(v["tipo"], "sessao");
        let _ = std::fs::remove_dir_all(&pasta);
    }
}
