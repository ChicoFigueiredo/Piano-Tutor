//! Professor falante (M4, etapa 6): voz sintética **local** (Piper) para o Roteiro.
//!
//! - [`Voz`]: contrato mínimo (`falar`, `parar`, `falando`, `estado`) que a interface usa sem
//!   saber qual voz está por trás;
//! - [`VozPiper`]: roda o binário `piper` (PATH e `~/.local/bin`), com o modelo `.onnx` de
//!   `<dados>/vozes/` (prefere `pt_BR`, e dentre elas `faber-medium`), grava o WAV em
//!   `<dados>/cache-voz/<sha256(voz + texto)>.wav` (não sintetiza duas vezes o mesmo texto) e
//!   o toca com o primeiro player disponível (`pw-play`, `paplay`, `aplay`, `mpv --no-video`).
//!   Tudo em thread própria: `falar` volta na hora e a interface não trava;
//! - [`VozMuda`]: sem voz instalada, só o texto na tela, com a mensagem de como instalar;
//! - [`VozFalsa`]: registra o que foi pedido (testes e telas em desenvolvimento);
//! - [`Roteiro`] (o de `piano-tutor-curso`) e [`falas_para_dizer`]: o `roteiro.toml` do Pacote de Curso;
//!   só falas com estado `aprovada` viram voz.
//!
//! Nada aqui foi ouvido numa saída de áudio real nos testes automáticos: a síntese e a
//! reprodução só são exercidas por teste manual (**NÃO VERIFICADO** em CI).

use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

// --- Pastas ---------------------------------------------------------------------------------

/// Pasta dos modelos de voz (`*.onnx` e `*.onnx.json`) dentro da pasta de dados do app.
pub fn pasta_de_vozes(dados: &Path) -> PathBuf {
    dados.join("vozes")
}

/// Pasta do cache de falas sintetizadas.
pub fn pasta_de_cache_da_voz(dados: &Path) -> PathBuf {
    dados.join("cache-voz")
}

/// Pasta de dados do app nesta plataforma (`~/.local/share/piano-tutor` no Linux).
pub fn pasta_de_dados_do_usuario() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("piano-tutor"))
}

fn hex(bytes: &[u8]) -> String {
    use fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Caminho do WAV no cache: `<cache>/<sha256(voz + NUL + texto)>.wav`. Trocar de voz ou de
/// texto muda o nome, então o cache nunca devolve a fala errada.
pub fn caminho_no_cache(cache: &Path, voz: &str, texto: &str) -> PathBuf {
    let mut h = Sha256::new();
    h.update(voz.as_bytes());
    h.update([0u8]);
    h.update(texto.as_bytes());
    cache.join(format!("{}.wav", hex(&h.finalize())))
}

/// Devolve o WAV do cache, chamando `sintetizar` (que grava no caminho recebido) só se ele
/// ainda não existir. A síntese vai para um arquivo `.part` que só vira `.wav` quando termina
/// bem, para um WAV pela metade nunca ser tocado.
pub fn garantir_wav(
    cache: &Path,
    voz: &str,
    texto: &str,
    sintetizar: impl FnOnce(&Path) -> Result<(), ErroVoz>,
) -> Result<PathBuf, ErroVoz> {
    let wav = caminho_no_cache(cache, voz, texto);
    if wav.is_file() {
        return Ok(wav);
    }
    fs::create_dir_all(cache).map_err(|e| ErroVoz::Disco(e.to_string()))?;
    let parcial = wav.with_extension("wav.part");
    let r = sintetizar(&parcial);
    match r {
        Ok(()) => fs::rename(&parcial, &wav).map_err(|e| ErroVoz::Disco(e.to_string()))?,
        Err(e) => {
            let _ = fs::remove_file(&parcial);
            return Err(e);
        }
    }
    Ok(wav)
}

// --- Descoberta de programas e modelos ------------------------------------------------------

fn e_executavel(p: &Path) -> bool {
    let Ok(meta) = fs::metadata(p) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Procura `nome` nas pastas dadas, na ordem.
pub fn procurar_executavel(nome: &str, pastas: &[PathBuf]) -> Option<PathBuf> {
    let candidatos = if cfg!(windows) {
        vec![format!("{nome}.exe"), nome.to_owned()]
    } else {
        vec![nome.to_owned()]
    };
    pastas
        .iter()
        .flat_map(|p| candidatos.iter().map(move |c| p.join(c)))
        .find(|p| e_executavel(p))
}

/// Pastas onde procurar programas: o `PATH` e `~/.local/bin`.
pub fn pastas_de_programas() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = dirs::home_dir() {
        v.push(home.join(".local").join("bin"));
    }
    v
}

/// Modelos `.onnx` encontrados em `pasta`, em ordem alfabética.
pub fn listar_modelos(pasta: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(pasta)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "onnx"))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Escolhe o modelo: `pt_BR-faber-medium`, depois qualquer `pt_BR` `medium`, qualquer `pt_BR`
/// e, por último, outra variante do português (`pt_PT`...). Modelos de outros idiomas nunca
/// são escolhidos (falariam errado).
pub fn escolher_modelo(modelos: &[PathBuf]) -> Option<&PathBuf> {
    let nota = |p: &PathBuf| -> Option<u8> {
        let nome = p.file_name()?.to_str()?;
        if nome.starts_with("pt_BR-faber-medium") {
            Some(0)
        } else if nome.starts_with("pt_BR") && nome.contains("medium") {
            Some(1)
        } else if nome.starts_with("pt_BR") {
            Some(2)
        } else if nome.starts_with("pt_") || nome.starts_with("pt-") {
            Some(3)
        } else {
            None
        }
    };
    modelos
        .iter()
        .filter_map(|p| nota(p).map(|n| (n, p)))
        .min_by_key(|(n, _)| *n)
        .map(|(_, p)| p)
}

/// Players de WAV tentados, na ordem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Player {
    PwPlay,
    Paplay,
    Aplay,
    Mpv,
}

impl Player {
    pub const ORDEM: [Player; 4] = [Player::PwPlay, Player::Paplay, Player::Aplay, Player::Mpv];

    pub const fn programa(self) -> &'static str {
        match self {
            Player::PwPlay => "pw-play",
            Player::Paplay => "paplay",
            Player::Aplay => "aplay",
            Player::Mpv => "mpv",
        }
    }

    /// Argumentos antes do arquivo.
    pub const fn argumentos(self) -> &'static [&'static str] {
        match self {
            Player::PwPlay | Player::Paplay => &[],
            Player::Aplay => &["-q"],
            Player::Mpv => &["--no-video", "--really-quiet"],
        }
    }
}

/// Primeiro player da [`Player::ORDEM`] achado nas pastas.
pub fn escolher_player(pastas: &[PathBuf]) -> Option<(Player, PathBuf)> {
    Player::ORDEM
        .into_iter()
        .find_map(|p| procurar_executavel(p.programa(), pastas).map(|c| (p, c)))
}

/// Comando que sintetiza: texto na entrada padrão, WAV em `saida`, sem log (`-q`: o Piper
/// escreve muito no stderr).
pub fn comando_piper(piper: &Path, modelo: &Path, saida: &Path) -> Command {
    let mut c = Command::new(piper);
    c.arg("-q")
        .arg("-m")
        .arg(modelo)
        .arg("-f")
        .arg(saida)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    c
}

/// Comando que toca o WAV.
pub fn comando_player(player: Player, caminho: &Path, wav: &Path) -> Command {
    let mut c = Command::new(caminho);
    c.args(player.argumentos())
        .arg(wav)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    c
}

// --- Erros e estado -------------------------------------------------------------------------

/// Como instalar a voz, em português, para mostrar ao aluno.
pub const COMO_INSTALAR_A_VOZ: &str = "O professor ainda não tem voz neste computador. Para ouvi-lo, instale o Piper (binário `piper`, em ~/.local/bin ou no PATH) e uma voz em português do Brasil (por exemplo pt_BR-faber-medium.onnx, com o arquivo .onnx.json) na pasta `vozes` dos dados do Piano Tutor (~/.local/share/piano-tutor/vozes). Enquanto isso, as falas aparecem só em texto.";

/// Por que a voz não funcionou.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ErroVoz {
    SemPiper,
    /// Pasta de vozes sem modelo em português.
    SemModelo(PathBuf),
    SemPlayer,
    Disco(String),
    Sintese(String),
    Reproducao(String),
}

impl fmt::Display for ErroVoz {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemPiper => write!(
                f,
                "Voz indisponível: o programa `piper` não foi encontrado (PATH ou ~/.local/bin). {COMO_INSTALAR_A_VOZ}"
            ),
            Self::SemModelo(p) => write!(
                f,
                "Voz indisponível: nenhuma voz em português em {}. {COMO_INSTALAR_A_VOZ}",
                p.display()
            ),
            Self::SemPlayer => write!(
                f,
                "Voz indisponível: nenhum player de áudio encontrado (pw-play, paplay, aplay ou mpv)."
            ),
            Self::Disco(e) => write!(f, "Voz com problema no disco: {e}"),
            Self::Sintese(e) => write!(f, "O Piper não conseguiu gerar a fala: {e}"),
            Self::Reproducao(e) => write!(f, "Não consegui tocar a fala: {e}"),
        }
    }
}

impl std::error::Error for ErroVoz {}

/// Situação da voz para a interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EstadoVoz {
    Pronta,
    /// Sem voz: só texto. A mensagem diz por quê e como instalar.
    Indisponivel(String),
}

/// Contrato da voz do professor.
pub trait Voz: Send {
    /// Pede para dizer `texto`; volta na hora (a fala corre em segundo plano) e interrompe a
    /// fala anterior. Texto vazio não faz nada.
    fn falar(&mut self, texto: &str) -> Result<(), ErroVoz>;
    /// Corta a fala em curso (e uma síntese em andamento).
    fn parar(&mut self);
    /// Ainda sintetizando ou tocando.
    fn falando(&self) -> bool;
    fn estado(&self) -> EstadoVoz;
    /// Último erro ocorrido em segundo plano (síntese ou reprodução), uma vez só.
    fn tomar_erro(&mut self) -> Option<ErroVoz> {
        None
    }
}

// --- VozMuda e VozFalsa ---------------------------------------------------------------------

/// Sem voz: o texto só aparece na tela. `falar` guarda a fala para a tela mostrar.
#[derive(Clone, Debug, Default)]
pub struct VozMuda {
    motivo: Option<ErroVoz>,
    ultima: Option<String>,
}

impl VozMuda {
    pub fn nova() -> Self {
        Self::default()
    }

    /// Muda por causa de `motivo` (a mensagem aparece em [`Voz::estado`]).
    pub fn por_causa_de(motivo: ErroVoz) -> Self {
        Self {
            motivo: Some(motivo),
            ultima: None,
        }
    }

    /// Última fala pedida, para mostrar como legenda.
    pub fn ultima_fala(&self) -> Option<&str> {
        self.ultima.as_deref()
    }
}

impl Voz for VozMuda {
    fn falar(&mut self, texto: &str) -> Result<(), ErroVoz> {
        let texto = texto.trim();
        if !texto.is_empty() {
            self.ultima = Some(texto.to_owned());
        }
        Ok(())
    }

    fn parar(&mut self) {}

    fn falando(&self) -> bool {
        false
    }

    fn estado(&self) -> EstadoVoz {
        EstadoVoz::Indisponivel(
            self.motivo
                .as_ref()
                .map_or_else(|| COMO_INSTALAR_A_VOZ.to_owned(), ToString::to_string),
        )
    }
}

/// Voz de teste: registra os pedidos, não faz som. Os clones compartilham o registro, então o
/// teste guarda um clone e entrega o outro ao código sob teste.
#[derive(Clone, Debug, Default)]
pub struct VozFalsa {
    registro: Arc<Mutex<RegistroFalso>>,
}

#[derive(Debug, Default)]
struct RegistroFalso {
    pedidos: Vec<String>,
    paradas: usize,
    falando: bool,
}

fn travar<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl VozFalsa {
    pub fn nova() -> Self {
        Self::default()
    }

    pub fn pedidos(&self) -> Vec<String> {
        travar(&self.registro).pedidos.clone()
    }

    pub fn vezes_que_parou(&self) -> usize {
        travar(&self.registro).paradas
    }

    /// Simula o fim da fala (para quem espera `falando()` ficar falso).
    pub fn terminar_a_fala(&self) {
        travar(&self.registro).falando = false;
    }
}

impl Voz for VozFalsa {
    fn falar(&mut self, texto: &str) -> Result<(), ErroVoz> {
        let texto = texto.trim();
        if !texto.is_empty() {
            let mut r = travar(&self.registro);
            r.pedidos.push(texto.to_owned());
            r.falando = true;
        }
        Ok(())
    }

    fn parar(&mut self) {
        let mut r = travar(&self.registro);
        r.paradas += 1;
        r.falando = false;
    }

    fn falando(&self) -> bool {
        travar(&self.registro).falando
    }

    fn estado(&self) -> EstadoVoz {
        EstadoVoz::Pronta
    }
}

// --- VozPiper -------------------------------------------------------------------------------

#[derive(Debug, Default)]
struct Compartilhado {
    /// Cada `falar`/`parar` troca a geração; a thread de uma geração velha desiste.
    geracao: u64,
    filho: Option<Child>,
    falando: bool,
    erro: Option<ErroVoz>,
}

/// Voz local com o Piper. Veja o módulo.
#[derive(Debug)]
pub struct VozPiper {
    piper: PathBuf,
    modelo: PathBuf,
    player: Player,
    caminho_player: PathBuf,
    cache: PathBuf,
    compartilhado: Arc<Mutex<Compartilhado>>,
}

impl VozPiper {
    /// Detecta Piper, modelo e player. `pastas` = onde procurar programas
    /// ([`pastas_de_programas`]); `dados` = pasta de dados do app.
    pub fn detectar(dados: &Path, pastas: &[PathBuf]) -> Result<Self, ErroVoz> {
        let piper = procurar_executavel("piper", pastas).ok_or(ErroVoz::SemPiper)?;
        let vozes = pasta_de_vozes(dados);
        let modelos = listar_modelos(&vozes);
        let modelo = escolher_modelo(&modelos)
            .cloned()
            .ok_or(ErroVoz::SemModelo(vozes))?;
        let (player, caminho_player) = escolher_player(pastas).ok_or(ErroVoz::SemPlayer)?;
        Ok(Self {
            piper,
            modelo,
            player,
            caminho_player,
            cache: pasta_de_cache_da_voz(dados),
            compartilhado: Arc::new(Mutex::new(Compartilhado::default())),
        })
    }

    /// Nome da voz em uso (nome do arquivo do modelo sem extensão), parte da chave do cache.
    pub fn nome_da_voz(&self) -> String {
        nome_do_modelo(&self.modelo)
    }

    pub fn player(&self) -> Player {
        self.player
    }

    /// Gera (ou acha no cache) o WAV de `texto`, **sem tocar**. Bloqueia durante a síntese:
    /// serve para pré-gerar falas e para teste manual.
    pub fn sintetizar_para_o_cache(&self, texto: &str) -> Result<PathBuf, ErroVoz> {
        let contexto = self.contexto();
        let g = travar(&self.compartilhado).geracao;
        contexto.wav_da_fala(g, texto)
    }

    fn contexto(&self) -> Contexto {
        Contexto {
            piper: self.piper.clone(),
            modelo: self.modelo.clone(),
            player: self.player,
            caminho_player: self.caminho_player.clone(),
            cache: self.cache.clone(),
            comp: Arc::clone(&self.compartilhado),
        }
    }
}

fn nome_do_modelo(modelo: &Path) -> String {
    modelo
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned())
}

impl Voz for VozPiper {
    fn falar(&mut self, texto: &str) -> Result<(), ErroVoz> {
        let texto = texto.trim().to_owned();
        if texto.is_empty() {
            return Ok(());
        }
        let geracao = {
            let mut c = travar(&self.compartilhado);
            matar(&mut c);
            c.geracao += 1;
            c.falando = true;
            c.erro = None;
            c.geracao
        };
        let contexto = self.contexto();
        thread::Builder::new()
            .name("professor-falante".to_owned())
            .spawn(move || contexto.executar(geracao, &texto))
            .map_err(|e| {
                travar(&self.compartilhado).falando = false;
                ErroVoz::Reproducao(e.to_string())
            })?;
        Ok(())
    }

    fn parar(&mut self) {
        let mut c = travar(&self.compartilhado);
        matar(&mut c);
        c.geracao += 1;
        c.falando = false;
    }

    fn falando(&self) -> bool {
        travar(&self.compartilhado).falando
    }

    fn estado(&self) -> EstadoVoz {
        EstadoVoz::Pronta
    }

    fn tomar_erro(&mut self) -> Option<ErroVoz> {
        travar(&self.compartilhado).erro.take()
    }
}

impl Drop for VozPiper {
    fn drop(&mut self) {
        self.parar();
    }
}

fn matar(c: &mut Compartilhado) {
    if let Some(mut filho) = c.filho.take() {
        let _ = filho.kill();
        let _ = filho.wait();
    }
}

/// O que a thread da fala precisa (cópias baratas).
struct Contexto {
    piper: PathBuf,
    modelo: PathBuf,
    player: Player,
    caminho_player: PathBuf,
    cache: PathBuf,
    comp: Arc<Mutex<Compartilhado>>,
}

/// Resultado de esperar um processo.
enum Fim {
    Terminou(bool),
    Cancelado,
}

impl Contexto {
    fn executar(&self, geracao: u64, texto: &str) {
        let r = self.wav_da_fala(geracao, texto).and_then(|wav| {
            if self.cancelada(geracao) {
                return Ok(());
            }
            self.tocar(geracao, &wav)
        });
        let mut c = travar(&self.comp);
        if c.geracao == geracao {
            c.falando = false;
            if let Err(e) = r {
                c.erro = Some(e);
            }
        }
    }

    fn cancelada(&self, geracao: u64) -> bool {
        travar(&self.comp).geracao != geracao
    }

    fn wav_da_fala(&self, geracao: u64, texto: &str) -> Result<PathBuf, ErroVoz> {
        garantir_wav(&self.cache, &nome_do_modelo(&self.modelo), texto, |saida| {
            let mut cmd = comando_piper(&self.piper, &self.modelo, saida);
            let mut filho = cmd
                .spawn()
                .map_err(|e| ErroVoz::Sintese(format!("não consegui iniciar o piper: {e}")))?;
            if let Some(mut entrada) = filho.stdin.take() {
                entrada
                    .write_all(texto.as_bytes())
                    .and_then(|()| entrada.write_all(b"\n"))
                    .map_err(|e| ErroVoz::Sintese(e.to_string()))?;
            }
            match self.esperar(geracao, filho)? {
                Fim::Terminou(true) if saida.is_file() => Ok(()),
                Fim::Terminou(_) => Err(ErroVoz::Sintese("o piper terminou com erro".to_owned())),
                Fim::Cancelado => Err(ErroVoz::Sintese("síntese cancelada".to_owned())),
            }
        })
    }

    fn tocar(&self, geracao: u64, wav: &Path) -> Result<(), ErroVoz> {
        let filho = comando_player(self.player, &self.caminho_player, wav)
            .spawn()
            .map_err(|e| ErroVoz::Reproducao(format!("{}: {e}", self.player.programa())))?;
        match self.esperar(geracao, filho)? {
            Fim::Terminou(false) => Err(ErroVoz::Reproducao(format!(
                "{} terminou com erro",
                self.player.programa()
            ))),
            _ => Ok(()),
        }
    }

    /// Guarda o processo (para `parar` poder matá-lo) e espera por ele sem segurar a trava.
    fn esperar(&self, geracao: u64, filho: Child) -> Result<Fim, ErroVoz> {
        {
            let mut c = travar(&self.comp);
            if c.geracao != geracao {
                let mut filho = filho;
                let _ = filho.kill();
                let _ = filho.wait();
                return Ok(Fim::Cancelado);
            }
            c.filho = Some(filho);
        }
        loop {
            {
                let mut c = travar(&self.comp);
                if c.geracao != geracao {
                    // `parar`/`falar` já mataram e recolheram o processo.
                    return Ok(Fim::Cancelado);
                }
                match c.filho.as_mut().map(Child::try_wait) {
                    Some(Ok(Some(status))) => {
                        c.filho = None;
                        return Ok(Fim::Terminou(status.success()));
                    }
                    Some(Ok(None)) => {}
                    Some(Err(e)) => {
                        c.filho = None;
                        return Err(ErroVoz::Reproducao(e.to_string()));
                    }
                    None => return Ok(Fim::Cancelado),
                }
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
}

/// Voz do sistema: [`VozPiper`] se tudo estiver instalado; senão [`VozMuda`] com o motivo
/// (mostre `estado()` ao aluno).
pub fn voz_do_sistema(dados: &Path) -> Box<dyn Voz> {
    match VozPiper::detectar(dados, &pastas_de_programas()) {
        Ok(v) => Box::new(v),
        Err(e) => Box::new(VozMuda::por_causa_de(e)),
    }
}

// --- Roteiro ---------------------------------------------------------------------------------
//
// Os tipos do Roteiro (`roteiro.toml`) são os de `piano-tutor-curso`: o app e o Pacote de Curso
// falam o mesmo formato (docs/specs/FORMATO-PACOTE-DE-CURSO.md).

pub use piano_tutor_curso::{EstadoConteudo, Fala, Momento, OrigemFala, Roteiro};

/// Versão do `roteiro.toml` que este código lê.
pub const VERSAO_DO_ROTEIRO: u32 = piano_tutor_curso::FORMATO_VERSAO;

/// Roteiro ilegível.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ErroRoteiro {
    Toml(String),
    VersaoNovaDemais(u32),
}

impl fmt::Display for ErroRoteiro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toml(e) => write!(f, "roteiro inválido: {e}"),
            Self::VersaoNovaDemais(v) => {
                write!(
                    f,
                    "roteiro feito para uma versão mais nova do app (formato {v})"
                )
            }
        }
    }
}

impl std::error::Error for ErroRoteiro {}

/// Lê um `roteiro.toml`: recusa TOML inválido, estado desconhecido e formato mais novo.
pub fn ler_roteiro(texto: &str) -> Result<Roteiro, ErroRoteiro> {
    let r = Roteiro::de_toml(texto).map_err(|e| ErroRoteiro::Toml(e.to_string()))?;
    if r.formato_versao > VERSAO_DO_ROTEIRO {
        return Err(ErroRoteiro::VersaoNovaDemais(r.formato_versao));
    }
    Ok(r)
}

/// Roteiro do Curso de fábrica, embutido no app.
pub const ROTEIRO_DE_FABRICA_TOML: &str = include_str!("../dados/roteiro-fabrica.toml");

/// Lição do Curso de fábrica no roteiro.
pub const LICAO_DE_FABRICA: &str = "fabrica";

/// Id do exercício de fábrica `n` (1 a 10) no roteiro: `ex01` a `ex10`.
pub fn id_de_exercicio_de_fabrica(n: u8) -> String {
    format!("ex{n:02}")
}

pub fn roteiro_de_fabrica() -> Result<Roteiro, ErroRoteiro> {
    ler_roteiro(ROTEIRO_DE_FABRICA_TOML)
}

/// Falas que o professor pode **dizer** agora: as do `momento` (e `passo`, se o momento for
/// `Passo`) da lição e do exercício, **só com estado aprovada**. Falas sem `exercicio` valem
/// para qualquer exercício da lição. A ordem é a do arquivo.
pub fn falas_para_dizer<'a>(
    roteiro: &'a Roteiro,
    licao: &str,
    exercicio: Option<&str>,
    momento: Momento,
    passo: Option<usize>,
) -> Vec<&'a Fala> {
    roteiro
        .falas
        .iter()
        .filter(|f| {
            f.estado == EstadoConteudo::Aprovada
                && f.licao == licao
                && f.momento == momento
                && f.exercicio.as_deref().is_none_or(|e| Some(e) == exercicio)
                && (momento != Momento::Passo
                    || passo.is_some_and(|p| f.passo.map(|x| x as usize) == Some(p)))
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct PastaTemp(PathBuf);

    impl PastaTemp {
        fn nova(rotulo: &str) -> Self {
            static N: AtomicUsize = AtomicUsize::new(0);
            let p = std::env::temp_dir().join(format!(
                "piano-tutor-voz-{}-{}-{rotulo}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).expect("cria a pasta de teste");
            Self(p)
        }
    }

    impl Drop for PastaTemp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn arquivo(p: &Path, nome: &str, executavel: bool) -> PathBuf {
        let c = p.join(nome);
        fs::write(&c, b"x").expect("grava");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = if executavel { 0o755 } else { 0o644 };
            fs::set_permissions(&c, fs::Permissions::from_mode(modo)).expect("permissões");
        }
        #[cfg(not(unix))]
        let _ = executavel;
        c
    }

    // --- cache ---

    #[test]
    fn cache_depende_da_voz_e_do_texto_e_nao_sintetiza_duas_vezes() {
        let t = PastaTemp::nova("cache");
        let cache = t.0.join("cache-voz");
        let a = caminho_no_cache(&cache, "pt_BR-faber-medium", "Olá");
        assert_eq!(a, caminho_no_cache(&cache, "pt_BR-faber-medium", "Olá"));
        assert_ne!(a, caminho_no_cache(&cache, "pt_BR-faber-medium", "Oi"));
        assert_ne!(a, caminho_no_cache(&cache, "pt_BR-cadu-medium", "Olá"));
        // O nome é um SHA-256 em hexadecimal.
        let nome = a.file_stem().and_then(|s| s.to_str()).expect("nome");
        assert_eq!(nome.len(), 64);
        assert!(nome.chars().all(|c| c.is_ascii_hexdigit()));
        // Separador entre voz e texto: ("ab","c") difere de ("a","bc").
        assert_ne!(
            caminho_no_cache(&cache, "ab", "c"),
            caminho_no_cache(&cache, "a", "bc")
        );

        let mut chamadas = 0;
        let wav = garantir_wav(&cache, "v", "Olá", |saida| {
            chamadas += 1;
            assert!(saida.extension().is_some_and(|e| e == "part"));
            fs::write(saida, b"RIFF").map_err(|e| ErroVoz::Disco(e.to_string()))
        })
        .expect("sintetiza");
        assert!(wav.is_file());
        assert_eq!(chamadas, 1);
        // Segunda vez: vem do cache, sem chamar o sintetizador.
        let de_novo = garantir_wav(&cache, "v", "Olá", |_| {
            chamadas += 1;
            Ok(())
        })
        .expect("cache");
        assert_eq!(de_novo, wav);
        assert_eq!(chamadas, 1);
    }

    #[test]
    fn sintese_que_falha_nao_deixa_wav_pela_metade() {
        let t = PastaTemp::nova("falha");
        let cache = t.0.join("c");
        let r = garantir_wav(&cache, "v", "Olá", |saida| {
            fs::write(saida, b"meio").map_err(|e| ErroVoz::Disco(e.to_string()))?;
            Err(ErroVoz::Sintese("quebrou".to_owned()))
        });
        assert_eq!(r, Err(ErroVoz::Sintese("quebrou".to_owned())));
        let sobras: Vec<_> = fs::read_dir(&cache).expect("pasta").collect();
        assert!(sobras.is_empty());
    }

    // --- descoberta ---

    #[test]
    fn procura_executavel_na_ordem_das_pastas_e_ignora_arquivo_sem_permissao() {
        let t = PastaTemp::nova("exec");
        let (p1, p2) = (t.0.join("a"), t.0.join("b"));
        fs::create_dir_all(&p1).expect("a");
        fs::create_dir_all(&p2).expect("b");
        arquivo(&p1, "piper", false);
        let bom = arquivo(&p2, "piper", true);
        #[cfg(unix)]
        assert_eq!(
            procurar_executavel("piper", &[p1.clone(), p2.clone()]),
            Some(bom)
        );
        assert_eq!(procurar_executavel("naoexiste", &[p1, p2]), None);
    }

    #[test]
    fn escolhe_o_modelo_preferindo_pt_br_faber() {
        let m = |n: &str| PathBuf::from(format!("/v/{n}"));
        let todos = vec![
            m("en_US-lessac-medium.onnx"),
            m("pt_BR-cadu-medium.onnx"),
            m("pt_BR-edresson-low.onnx"),
            m("pt_BR-faber-medium.onnx"),
            m("pt_PT-tugao-medium.onnx"),
        ];
        assert_eq!(escolher_modelo(&todos), Some(&m("pt_BR-faber-medium.onnx")));
        let sem_faber = &todos[..3];
        assert_eq!(
            escolher_modelo(sem_faber),
            Some(&m("pt_BR-cadu-medium.onnx"))
        );
        assert_eq!(
            escolher_modelo(&[m("pt_BR-edresson-low.onnx"), m("pt_PT-tugao-medium.onnx")]),
            Some(&m("pt_BR-edresson-low.onnx"))
        );
        assert_eq!(
            escolher_modelo(&[m("pt_PT-tugao-medium.onnx")]),
            Some(&m("pt_PT-tugao-medium.onnx"))
        );
        // Outro idioma nunca serve.
        assert_eq!(escolher_modelo(&[m("en_US-lessac-medium.onnx")]), None);
        assert_eq!(escolher_modelo(&[]), None);
    }

    #[test]
    fn lista_modelos_onnx_em_ordem_e_ignora_o_json() {
        let t = PastaTemp::nova("modelos");
        arquivo(&t.0, "pt_BR-faber-medium.onnx", false);
        arquivo(&t.0, "pt_BR-faber-medium.onnx.json", false);
        arquivo(&t.0, "pt_BR-cadu-medium.onnx", false);
        let v = listar_modelos(&t.0);
        let nomes: Vec<_> = v.iter().filter_map(|p| p.file_name()?.to_str()).collect();
        assert_eq!(nomes, ["pt_BR-cadu-medium.onnx", "pt_BR-faber-medium.onnx"]);
        assert!(listar_modelos(&t.0.join("nao-existe")).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn players_seguem_a_ordem_pw_play_paplay_aplay_mpv() {
        let t = PastaTemp::nova("players");
        arquivo(&t.0, "mpv", true);
        let pastas = vec![t.0.clone()];
        assert_eq!(escolher_player(&pastas).map(|p| p.0), Some(Player::Mpv));
        arquivo(&t.0, "aplay", true);
        assert_eq!(escolher_player(&pastas).map(|p| p.0), Some(Player::Aplay));
        arquivo(&t.0, "paplay", true);
        assert_eq!(escolher_player(&pastas).map(|p| p.0), Some(Player::Paplay));
        arquivo(&t.0, "pw-play", true);
        assert_eq!(escolher_player(&pastas).map(|p| p.0), Some(Player::PwPlay));
        assert_eq!(escolher_player(&[]), None);
    }

    #[test]
    fn comandos_levam_os_argumentos_certos_sem_executar_nada() {
        let c = comando_piper(
            Path::new("/bin/piper"),
            Path::new("/v/m.onnx"),
            Path::new("/c/x.wav.part"),
        );
        assert_eq!(c.get_program(), "/bin/piper");
        let args: Vec<_> = c
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, ["-q", "-m", "/v/m.onnx", "-f", "/c/x.wav.part"]);
        let c = comando_player(Player::Mpv, Path::new("/bin/mpv"), Path::new("/c/x.wav"));
        let args: Vec<_> = c
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, ["--no-video", "--really-quiet", "/c/x.wav"]);
        let c = comando_player(
            Player::Aplay,
            Path::new("/bin/aplay"),
            Path::new("/c/x.wav"),
        );
        let args: Vec<_> = c
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, ["-q", "/c/x.wav"]);
    }

    #[test]
    fn deteccao_diz_o_que_falta_em_portugues() {
        let t = PastaTemp::nova("deteccao");
        let dados = t.0.join("dados");
        let bin = t.0.join("bin");
        fs::create_dir_all(&bin).expect("bin");
        let pastas = vec![bin.clone()];
        // Nada instalado.
        let e = VozPiper::detectar(&dados, &pastas).expect_err("sem piper");
        assert_eq!(e, ErroVoz::SemPiper);
        assert!(e.to_string().contains("instale o Piper"));
        #[cfg(unix)]
        {
            arquivo(&bin, "piper", true);
            let e = VozPiper::detectar(&dados, &pastas).expect_err("sem modelo");
            assert!(matches!(e, ErroVoz::SemModelo(_)));
            assert!(e.to_string().contains("voz em português"));
            fs::create_dir_all(pasta_de_vozes(&dados)).expect("vozes");
            arquivo(&pasta_de_vozes(&dados), "pt_BR-faber-medium.onnx", false);
            let e = VozPiper::detectar(&dados, &pastas).expect_err("sem player");
            assert_eq!(e, ErroVoz::SemPlayer);
            arquivo(&bin, "aplay", true);
            let v = VozPiper::detectar(&dados, &pastas).expect("tudo instalado");
            assert_eq!(v.nome_da_voz(), "pt_BR-faber-medium");
            assert_eq!(v.player(), Player::Aplay);
            assert_eq!(v.estado(), EstadoVoz::Pronta);
            assert!(!v.falando());
        }
    }

    // --- vozes sem som ---

    #[test]
    fn voz_muda_so_mostra_o_texto_e_explica_como_instalar() {
        let mut v = VozMuda::nova();
        assert_eq!(v.ultima_fala(), None);
        v.falar("  Vamos tocar.  ").expect("sempre ok");
        assert_eq!(v.ultima_fala(), Some("Vamos tocar."));
        v.falar("   ").expect("vazio não faz nada");
        assert_eq!(v.ultima_fala(), Some("Vamos tocar."));
        assert!(!v.falando());
        let EstadoVoz::Indisponivel(msg) = v.estado() else {
            panic!("a voz muda deve estar indisponível");
        };
        assert!(msg.contains("Piper") && msg.contains("vozes"));
        let v = VozMuda::por_causa_de(ErroVoz::SemPlayer);
        let EstadoVoz::Indisponivel(msg) = v.estado() else {
            panic!("indisponível");
        };
        assert!(msg.contains("player de áudio"));
        let mut v = v;
        v.parar(); // não faz nada, mas existe
    }

    #[test]
    fn voz_falsa_registra_pedidos_e_paradas() {
        let observador = VozFalsa::nova();
        let mut voz = observador.clone();
        voz.falar("Primeira.").expect("ok");
        assert!(voz.falando());
        voz.falar("").expect("vazio ignorado");
        voz.parar();
        assert!(!voz.falando());
        voz.falar("Segunda.").expect("ok");
        observador.terminar_a_fala();
        assert!(!voz.falando());
        assert_eq!(observador.pedidos(), ["Primeira.", "Segunda."]);
        assert_eq!(observador.vezes_que_parou(), 1);
        assert_eq!(voz.estado(), EstadoVoz::Pronta);
    }

    #[test]
    fn voz_pode_ser_usada_como_objeto() {
        let falsa = VozFalsa::nova();
        let mut v: Box<dyn Voz> = Box::new(falsa.clone());
        v.falar("Olá").expect("ok");
        assert_eq!(falsa.pedidos(), ["Olá"]);
        assert!(v.tomar_erro().is_none());
    }

    // --- roteiro ---

    const ROTEIRO_DE_TESTE: &str = r#"
formato_versao = 1

[[falas]]
id = "a"
licao = "s03-a14"
exercicio = "cinco-notas"
momento = "antes"
texto = "Vamos tocar as cinco notas."
estado = "aprovada"

[[falas]]
id = "b"
licao = "s03-a14"
exercicio = "cinco-notas"
momento = "antes"
texto = "Esta ainda é hipótese."
estado = "hipotese"

[[falas]]
id = "c"
licao = "s03-a14"
momento = "acerto"
texto = "Muito bem."
estado = "aprovada"

[[falas]]
id = "d"
licao = "s03-a14"
exercicio = "cinco-notas"
momento = "passo"
passo = 2
texto = "Agora o Mi."
estado = "aprovada"
origem = { aula = 14, inicio_s = 12.3, fim_s = 18.0 }

[[falas]]
id = "e"
licao = "s03-a14"
exercicio = "cinco-notas"
momento = "erro"
texto = "Fala rejeitada."
estado = "rejeitada"

[[falas]]
id = "f"
licao = "s03-a15"
momento = "acerto"
texto = "Outra lição."
estado = "aprovada"
"#;

    fn ids(v: &[&Fala]) -> Vec<String> {
        v.iter().map(|f| f.id.clone()).collect()
    }

    #[test]
    fn so_falas_aprovadas_viram_voz() {
        let r = ler_roteiro(ROTEIRO_DE_TESTE).expect("parseia");
        assert_eq!(r.falas.len(), 6);
        let antes = falas_para_dizer(&r, "s03-a14", Some("cinco-notas"), Momento::Antes, None);
        assert_eq!(ids(&antes), ["a"]); // "b" é hipótese
        // Fala sem exercício vale para qualquer exercício da lição.
        let acerto = falas_para_dizer(&r, "s03-a14", Some("outro"), Momento::Acerto, None);
        assert_eq!(ids(&acerto), ["c"]);
        // Fala de outro exercício não vale.
        assert!(falas_para_dizer(&r, "s03-a14", Some("outro"), Momento::Antes, None).is_empty());
        // Passo: precisa do número certo.
        let p2 = falas_para_dizer(&r, "s03-a14", Some("cinco-notas"), Momento::Passo, Some(2));
        assert_eq!(ids(&p2), ["d"]);
        assert_eq!(p2[0].origem.map(|o| o.aula), Some(14));
        assert!(
            falas_para_dizer(&r, "s03-a14", Some("cinco-notas"), Momento::Passo, Some(1))
                .is_empty()
        );
        assert!(
            falas_para_dizer(&r, "s03-a14", Some("cinco-notas"), Momento::Passo, None).is_empty()
        );
        // Rejeitada nunca.
        assert!(
            falas_para_dizer(&r, "s03-a14", Some("cinco-notas"), Momento::Erro, None).is_empty()
        );
        // Lição diferente.
        let outra = falas_para_dizer(&r, "s03-a15", None, Momento::Acerto, None);
        assert_eq!(ids(&outra), ["f"]);
    }

    #[test]
    fn roteiro_invalido_ou_de_versao_nova_e_recusado() {
        assert!(matches!(
            ler_roteiro("isto não é toml ="),
            Err(ErroRoteiro::Toml(_))
        ));
        assert!(matches!(
            ler_roteiro("formato_versao = 1\n[[falas]]\nid = \"x\"\n"),
            Err(ErroRoteiro::Toml(_))
        ));
        assert_eq!(
            ler_roteiro("formato_versao = 2"),
            Err(ErroRoteiro::VersaoNovaDemais(2))
        );
        // Estado desconhecido não passa (não vira aprovada por engano).
        let ruim = ROTEIRO_DE_TESTE.replace("\"rejeitada\"", "\"talvez\"");
        assert!(ler_roteiro(&ruim).is_err());
        let sem_falas = ler_roteiro("formato_versao = 1").expect("válido");
        assert!(sem_falas.falas.is_empty());
    }

    // --- roteiro de fábrica ---

    #[test]
    fn roteiro_de_fabrica_parseia_e_cobre_os_dez_exercicios() {
        use crate::exercicios::exercicios_de_fabrica;
        let r = roteiro_de_fabrica().expect("o arquivo parseia");
        let exercicios = exercicios_de_fabrica();
        assert_eq!(exercicios.len(), 10);
        // Todo exercício referenciado existe.
        let existentes: Vec<String> = exercicios
            .iter()
            .map(|e| id_de_exercicio_de_fabrica(e.id))
            .collect();
        for f in &r.falas {
            assert_eq!(f.licao, LICAO_DE_FABRICA, "fala {}", f.id);
            let e = f
                .exercicio
                .as_deref()
                .expect("fala de fábrica tem exercício");
            assert!(existentes.iter().any(|x| x == e), "fala {} cita {e}", f.id);
            assert!(!f.texto.trim().is_empty());
            // A voz lê por extenso: sem # nem símbolos de acidente.
            assert!(
                !f.texto.contains('#') && !f.texto.contains('♯'),
                "fala {}",
                f.id
            );
            // Curtas e encorajadoras: nada de parágrafo.
            assert!(f.texto.chars().count() <= 130, "fala {} longa", f.id);
        }
        // Cada exercício tem antes, acerto, erro e depois, e os ids são únicos.
        for e in &existentes {
            for m in [
                Momento::Antes,
                Momento::Acerto,
                Momento::Erro,
                Momento::Depois,
            ] {
                let n = r
                    .falas
                    .iter()
                    .filter(|f| f.exercicio.as_deref() == Some(e) && f.momento == m)
                    .count();
                assert_eq!(n, 1, "exercício {e}, momento {m:?}");
            }
        }
        assert_eq!(r.falas.len(), 40);
        let mut todos: Vec<&str> = r.falas.iter().map(|f| f.id.as_str()).collect();
        todos.sort_unstable();
        todos.dedup();
        assert_eq!(todos.len(), 40);
    }

    #[test]
    fn falas_de_fabrica_so_falam_depois_de_aprovadas() {
        let r = roteiro_de_fabrica().expect("parseia");
        // Hoje todas são hipótese: o professor fica calado até o Chico aprovar.
        assert!(r.falas.iter().all(|f| f.estado == EstadoConteudo::Hipotese));
        for m in [
            Momento::Antes,
            Momento::Acerto,
            Momento::Erro,
            Momento::Depois,
        ] {
            assert!(
                falas_para_dizer(&r, LICAO_DE_FABRICA, Some("ex01"), m, None).is_empty(),
                "{m:?}"
            );
        }
        // Aprovada uma, ela passa a ser dita.
        let mut r = r;
        r.falas[0].estado = EstadoConteudo::Aprovada;
        let id = r.falas[0].id.clone();
        let ditas = falas_para_dizer(&r, LICAO_DE_FABRICA, Some("ex01"), Momento::Antes, None);
        assert_eq!(ditas.len(), 1);
        assert_eq!(ditas[0].id, id);
    }

    /// Teste MANUAL (precisa do Piper e de uma voz instalados): gera um WAV no cache de uma
    /// pasta temporária, sem tocar, e confere o cache. Rode com
    /// `cargo test -p piano-tutor-app piper_de_verdade -- --ignored --nocapture`.
    #[cfg(unix)]
    #[test]
    #[ignore = "manual: executa o piper de verdade (sem tocar)"]
    fn piper_de_verdade_gera_wav_sem_tocar() {
        let real = pasta_de_dados_do_usuario().expect("pasta de dados");
        let t = PastaTemp::nova("manual");
        std::os::unix::fs::symlink(pasta_de_vozes(&real), pasta_de_vozes(&t.0)).expect("link");
        let v = VozPiper::detectar(&t.0, &pastas_de_programas()).expect("Piper e voz instalados");
        let inicio = std::time::Instant::now();
        let wav = v
            .sintetizar_para_o_cache("Vamos começar achando o Dó central.")
            .expect("sintetiza");
        let primeira = inicio.elapsed();
        let bytes = fs::read(&wav).expect("lê o WAV");
        assert_eq!(&bytes[..4], b"RIFF");
        assert!(bytes.len() > 10_000);
        let inicio = std::time::Instant::now();
        let de_novo = v
            .sintetizar_para_o_cache("Vamos começar achando o Dó central.")
            .expect("cache");
        assert_eq!(wav, de_novo);
        println!(
            "voz {}: {} bytes, 1ª vez {primeira:?}, do cache {:?}",
            v.nome_da_voz(),
            bytes.len(),
            inicio.elapsed()
        );
    }
}
