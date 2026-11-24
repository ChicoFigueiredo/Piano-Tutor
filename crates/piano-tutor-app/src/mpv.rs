//! Mídia do Curso com o `mpv` (M4, etapa 5): o áudio do professor com cursor ("Ouvir o
//! professor") e o vídeo da Aula no instante do trecho ("Ver vídeo").
//!
//! - [`comando_audio`] e [`comando_video`] montam a linha de comando como dados; quem executa é
//!   um [`Executor`] injetado ([`ExecutorReal`] no app, falso nos testes: nenhum teste roda o
//!   `mpv`);
//! - o áudio abre o socket JSON IPC do `mpv` (`--input-ipc-server`) e pede
//!   `observe_property time-pos`; a cada quadro [`ReprodutorAudio::atualizar`] lê sem bloquear o
//!   que já chegou e guarda o último `time-pos` (o cursor da partitura segue esse instante);
//! - nada aqui espera o processo: `iniciar` volta na hora, a leitura do socket é não bloqueante
//!   e o processo é encerrado e recolhido ao parar ou ao largar o objeto (sair da tela).
//!
//! **NÃO VERIFICADO** com o `mpv` e uma saída de áudio reais nos testes automáticos: só a
//! montagem dos comandos, a leitura do IPC (contra um servidor falso no socket) e o ciclo de
//! vida dos processos são testados.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Como instalar o `mpv`, em português, para mostrar ao aluno.
pub const COMO_INSTALAR_O_MPV: &str = "O mpv não foi encontrado neste computador. Para ouvir o professor e ver os vídeos das Aulas, instale-o (no Kubuntu: sudo apt install mpv) e abra o Piano Tutor de novo.";

/// Quanto esperar o `mpv` criar o socket antes de desistir do cursor (a música toca igual).
const ESPERA_DO_SOCKET: Duration = Duration::from_secs(5);

/// Linha de comando de um processo, como dados (testável sem executar nada).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComandoMpv {
    pub programa: PathBuf,
    pub argumentos: Vec<OsString>,
}

/// `mpv --no-video --really-quiet --input-ipc-server=<socket> <arquivo>`.
pub fn comando_audio(mpv: &Path, arquivo: &Path, socket: &Path) -> ComandoMpv {
    let mut ipc = OsString::from("--input-ipc-server=");
    ipc.push(socket);
    ComandoMpv {
        programa: mpv.to_owned(),
        argumentos: vec![
            "--no-video".into(),
            "--really-quiet".into(),
            ipc,
            arquivo.as_os_str().to_owned(),
        ],
    }
}

/// `mpv --start=<inicio_s> <arquivo>`: a janela do `mpv` abre no instante do trecho.
pub fn comando_video(mpv: &Path, arquivo: &Path, inicio_s: f64) -> ComandoMpv {
    let inicio = if inicio_s.is_finite() {
        inicio_s.max(0.0)
    } else {
        0.0
    };
    ComandoMpv {
        programa: mpv.to_owned(),
        argumentos: vec![
            format!("--start={inicio:.3}").into(),
            arquivo.as_os_str().to_owned(),
        ],
    }
}

// --- Processos --------------------------------------------------------------------------------

/// Um processo em segundo plano.
pub trait Processo: Send {
    /// Ainda rodando (recolhe o processo se já terminou).
    fn vivo(&mut self) -> bool;
    /// Encerra e recolhe o processo. Pode ser chamado de novo sem efeito.
    fn encerrar(&mut self);
}

/// Quem inicia processos. No app é [`ExecutorReal`]; nos testes, um falso.
pub trait Executor: Send {
    fn iniciar(&self, comando: &ComandoMpv) -> io::Result<Box<dyn Processo>>;
}

/// Executa de verdade (`std::process::Command`), sem entrada e sem saída.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExecutorReal;

struct ProcessoReal(Option<Child>);

impl Processo for ProcessoReal {
    fn vivo(&mut self) -> bool {
        let Some(filho) = self.0.as_mut() else {
            return false;
        };
        match filho.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) | Err(_) => {
                self.0 = None;
                false
            }
        }
    }

    fn encerrar(&mut self) {
        if let Some(mut filho) = self.0.take() {
            let _ = filho.kill();
            let _ = filho.wait();
        }
    }
}

impl Drop for ProcessoReal {
    fn drop(&mut self) {
        self.encerrar();
    }
}

impl Executor for ExecutorReal {
    fn iniciar(&self, c: &ComandoMpv) -> io::Result<Box<dyn Processo>> {
        let filho = Command::new(&c.programa)
            .args(&c.argumentos)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(Box::new(ProcessoReal(Some(filho))))
    }
}

// --- IPC JSON do mpv --------------------------------------------------------------------------

/// Pedido que liga a observação de `time-pos` (o `mpv` responde e depois avisa a cada mudança).
pub const PEDIDO_OBSERVAR_POSICAO: &str = "{\"command\":[\"observe_property\",1,\"time-pos\"]}\n";

/// Junta o que chega do socket em linhas JSON e guarda o último `time-pos` (em segundos).
#[derive(Debug, Default)]
pub struct DecodificadorIpc {
    buffer: Vec<u8>,
    posicao: Option<f64>,
}

/// Teto do que fica em memória sem fim de linha (o `mpv` manda uma linha por evento).
const MAX_LINHA: usize = 64 * 1024;

impl DecodificadorIpc {
    pub fn alimentar(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
        while let Some(fim) = self.buffer.iter().position(|b| *b == b'\n') {
            let linha: Vec<u8> = self.buffer.drain(..=fim).collect();
            self.interpretar(&linha);
        }
        if self.buffer.len() > MAX_LINHA {
            self.buffer.clear();
        }
    }

    fn interpretar(&mut self, linha: &[u8]) {
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(linha) else {
            return;
        };
        if v.get("event").and_then(|e| e.as_str()) == Some("property-change")
            && v.get("name").and_then(|n| n.as_str()) == Some("time-pos")
            && let Some(t) = v.get("data").and_then(serde_json::Value::as_f64)
            && t.is_finite()
        {
            self.posicao = Some(t.max(0.0));
        }
    }

    /// Último `time-pos` visto, em segundos desde o início do arquivo.
    pub fn posicao(&self) -> Option<f64> {
        self.posicao
    }
}

#[cfg(unix)]
mod socket {
    use super::{DecodificadorIpc, ESPERA_DO_SOCKET, Instant, PEDIDO_OBSERVAR_POSICAO};
    use std::io::{ErrorKind, Read, Write};
    use std::os::unix::net::UnixStream;
    use std::path::{Path, PathBuf};

    enum Ligacao {
        Esperando(Instant),
        Ligada(UnixStream),
        Desistiu,
    }

    /// Conexão não bloqueante com o socket do `mpv`.
    pub struct Ipc {
        caminho: PathBuf,
        ligacao: Ligacao,
        pub decodificador: DecodificadorIpc,
    }

    impl Ipc {
        pub fn novo(caminho: &Path) -> Self {
            Self {
                caminho: caminho.to_owned(),
                ligacao: Ligacao::Esperando(Instant::now()),
                decodificador: DecodificadorIpc::default(),
            }
        }

        pub fn ligada(&self) -> bool {
            matches!(self.ligacao, Ligacao::Ligada(_))
        }

        /// Tenta ligar (o `mpv` cria o socket um instante depois de abrir) e lê o que chegou.
        pub fn atualizar(&mut self) {
            match &mut self.ligacao {
                Ligacao::Esperando(desde) => {
                    if self.caminho.exists()
                        && let Ok(mut s) = UnixStream::connect(&self.caminho)
                    {
                        let pronto = s.write_all(PEDIDO_OBSERVAR_POSICAO.as_bytes()).is_ok()
                            && s.set_nonblocking(true).is_ok();
                        self.ligacao = if pronto {
                            Ligacao::Ligada(s)
                        } else {
                            Ligacao::Desistiu
                        };
                    } else if desde.elapsed() > ESPERA_DO_SOCKET {
                        self.ligacao = Ligacao::Desistiu;
                    }
                }
                Ligacao::Ligada(s) => {
                    let mut bloco = [0u8; 4096];
                    // No máximo alguns blocos por quadro: a interface nunca fica presa aqui.
                    for _ in 0..16 {
                        match s.read(&mut bloco) {
                            Ok(0) => {
                                self.ligacao = Ligacao::Desistiu;
                                break;
                            }
                            Ok(n) => self.decodificador.alimentar(&bloco[..n]),
                            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                            Err(e) if e.kind() == ErrorKind::Interrupted => {}
                            Err(_) => {
                                self.ligacao = Ligacao::Desistiu;
                                break;
                            }
                        }
                    }
                }
                Ligacao::Desistiu => {}
            }
        }
    }
}

#[cfg(not(unix))]
mod socket {
    use super::DecodificadorIpc;
    use std::path::Path;

    /// Sem socket Unix nesta plataforma: o áudio toca, mas o cursor não acompanha.
    pub struct Ipc {
        pub decodificador: DecodificadorIpc,
    }

    impl Ipc {
        pub fn novo(_caminho: &Path) -> Self {
            Self {
                decodificador: DecodificadorIpc::default(),
            }
        }

        pub fn ligada(&self) -> bool {
            false
        }

        pub fn atualizar(&mut self) {}
    }
}

use socket::Ipc;

// --- Áudio com posição ------------------------------------------------------------------------

/// O áudio de um Exercício tocando no `mpv`, com a posição lida pelo IPC.
pub struct ReprodutorAudio {
    processo: Box<dyn Processo>,
    ipc: Ipc,
    socket: PathBuf,
}

impl ReprodutorAudio {
    /// Inicia o `mpv` e volta na hora; o socket só é aberto nos quadros seguintes.
    pub fn iniciar(
        executor: &dyn Executor,
        mpv: &Path,
        arquivo: &Path,
        socket: PathBuf,
    ) -> io::Result<Self> {
        let _ = std::fs::remove_file(&socket);
        let processo = executor.iniciar(&comando_audio(mpv, arquivo, &socket))?;
        Ok(Self {
            processo,
            ipc: Ipc::novo(&socket),
            socket,
        })
    }

    /// Lê o IPC sem bloquear. Devolve `false` quando o `mpv` terminou.
    pub fn atualizar(&mut self) -> bool {
        self.ipc.atualizar();
        self.processo.vivo()
    }

    /// Posição atual do áudio (segundos), se o IPC já informou.
    pub fn posicao_s(&self) -> Option<f64> {
        self.ipc.decodificador.posicao()
    }

    pub fn ipc_ligado(&self) -> bool {
        self.ipc.ligada()
    }

    pub fn parar(&mut self) {
        self.processo.encerrar();
        let _ = std::fs::remove_file(&self.socket);
    }
}

impl Drop for ReprodutorAudio {
    fn drop(&mut self) {
        self.parar();
    }
}

// --- Conjunto ---------------------------------------------------------------------------------

/// O `mpv` do app: o áudio do professor e a janela de vídeo, se houver.
pub struct Midia {
    executor: Box<dyn Executor>,
    mpv: Option<PathBuf>,
    audio: Option<ReprodutorAudio>,
    video: Option<Box<dyn Processo>>,
    contador: u32,
}

impl Midia {
    /// Procura o `mpv` no PATH e em `~/.local/bin`.
    pub fn do_sistema() -> Self {
        let mpv = crate::professor_falante::procurar_executavel(
            "mpv",
            &crate::professor_falante::pastas_de_programas(),
        );
        Self::com(Box::new(ExecutorReal), mpv)
    }

    pub fn com(executor: Box<dyn Executor>, mpv: Option<PathBuf>) -> Self {
        Self {
            executor,
            mpv,
            audio: None,
            video: None,
            contador: 0,
        }
    }

    pub fn tem_mpv(&self) -> bool {
        self.mpv.is_some()
    }

    fn socket_novo(&mut self) -> PathBuf {
        self.contador += 1;
        let base = dirs::runtime_dir().unwrap_or_else(std::env::temp_dir);
        base.join(format!(
            "piano-tutor-mpv-{}-{}.sock",
            std::process::id(),
            self.contador
        ))
    }

    /// Toca `arquivo` (troca o áudio anterior). Sem `mpv`: o texto de como instalar.
    pub fn tocar_audio(&mut self, arquivo: &Path) -> Result<(), String> {
        let Some(mpv) = self.mpv.clone() else {
            return Err(COMO_INSTALAR_O_MPV.to_owned());
        };
        self.parar_audio();
        let socket = self.socket_novo();
        let r = ReprodutorAudio::iniciar(self.executor.as_ref(), &mpv, arquivo, socket)
            .map_err(|e| format!("Não consegui abrir o mpv: {e}"))?;
        self.audio = Some(r);
        Ok(())
    }

    /// Abre o vídeo no instante `inicio_s` (troca o vídeo anterior).
    pub fn abrir_video(&mut self, arquivo: &Path, inicio_s: f64) -> Result<(), String> {
        let Some(mpv) = self.mpv.clone() else {
            return Err(COMO_INSTALAR_O_MPV.to_owned());
        };
        self.parar_video();
        let p = self
            .executor
            .iniciar(&comando_video(&mpv, arquivo, inicio_s))
            .map_err(|e| format!("Não consegui abrir o mpv: {e}"))?;
        self.video = Some(p);
        Ok(())
    }

    /// Um passo por quadro: lê o IPC do áudio e solta o que já terminou.
    pub fn atualizar(&mut self) {
        if let Some(a) = self.audio.as_mut()
            && !a.atualizar()
        {
            self.audio = None;
        }
        if let Some(v) = self.video.as_mut()
            && !v.vivo()
        {
            self.video = None;
        }
    }

    pub fn audio_tocando(&self) -> bool {
        self.audio.is_some()
    }

    pub fn video_aberto(&self) -> bool {
        self.video.is_some()
    }

    /// Posição do áudio em segundos, se estiver tocando e o IPC já informou.
    pub fn posicao_audio_s(&self) -> Option<f64> {
        self.audio.as_ref().and_then(ReprodutorAudio::posicao_s)
    }

    pub fn parar_audio(&mut self) {
        self.audio = None;
    }

    pub fn parar_video(&mut self) {
        if let Some(mut v) = self.video.take() {
            v.encerrar();
        }
    }

    /// Encerra tudo (ao sair da tela).
    pub fn parar_tudo(&mut self) {
        self.parar_audio();
        self.parar_video();
    }
}

impl Drop for Midia {
    fn drop(&mut self) {
        self.parar_tudo();
    }
}

impl std::fmt::Debug for Midia {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Midia")
            .field("mpv", &self.mpv)
            .field("audio", &self.audio.is_some())
            .field("video", &self.video.is_some())
            .finish()
    }
}

#[cfg(test)]
pub(crate) mod apoio_testes {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Executor que só registra os comandos e entrega processos controláveis.
    #[derive(Clone, Default)]
    pub struct ExecutorFalso {
        pub comandos: Arc<Mutex<Vec<ComandoMpv>>>,
        pub vivos: Arc<Mutex<Vec<Arc<Mutex<bool>>>>>,
        pub falhar: Arc<Mutex<bool>>,
    }

    struct ProcessoFalso(Arc<Mutex<bool>>);

    impl Processo for ProcessoFalso {
        fn vivo(&mut self) -> bool {
            *self.0.lock().expect("trava")
        }
        fn encerrar(&mut self) {
            *self.0.lock().expect("trava") = false;
        }
    }

    impl Executor for ExecutorFalso {
        fn iniciar(&self, c: &ComandoMpv) -> io::Result<Box<dyn Processo>> {
            if *self.falhar.lock().expect("trava") {
                return Err(io::Error::other("falha de teste"));
            }
            self.comandos.lock().expect("trava").push(c.clone());
            let vivo = Arc::new(Mutex::new(true));
            self.vivos.lock().expect("trava").push(Arc::clone(&vivo));
            Ok(Box::new(ProcessoFalso(vivo)))
        }
    }

    impl ExecutorFalso {
        pub fn comandos(&self) -> Vec<ComandoMpv> {
            self.comandos.lock().expect("trava").clone()
        }

        pub fn vivos(&self) -> Vec<bool> {
            self.vivos
                .lock()
                .expect("trava")
                .iter()
                .map(|v| *v.lock().expect("trava"))
                .collect()
        }

        /// Simula o fim do processo `i` (o `mpv` fechou sozinho).
        pub fn terminar(&self, i: usize) {
            *self.vivos.lock().expect("trava")[i].lock().expect("trava") = false;
        }
    }

    pub fn midia_falsa() -> (Midia, ExecutorFalso) {
        let ex = ExecutorFalso::default();
        let m = Midia::com(Box::new(ex.clone()), Some(PathBuf::from("/usr/bin/mpv")));
        (m, ex)
    }
}

#[cfg(test)]
mod testes {
    use super::apoio_testes::*;
    use super::*;

    fn args(c: &ComandoMpv) -> Vec<String> {
        c.argumentos
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn comandos_levam_os_argumentos_do_roteiro_sem_executar_nada() {
        let a = comando_audio(
            Path::new("/bin/mpv"),
            Path::new("/c/audio/x.opus"),
            Path::new("/run/s.sock"),
        );
        assert_eq!(a.programa, Path::new("/bin/mpv"));
        assert_eq!(
            args(&a),
            [
                "--no-video",
                "--really-quiet",
                "--input-ipc-server=/run/s.sock",
                "/c/audio/x.opus"
            ]
        );
        let v = comando_video(Path::new("/bin/mpv"), Path::new("/v/14.mp4"), 95.0);
        assert_eq!(args(&v), ["--start=95.000", "/v/14.mp4"]);
        // Instante inválido vira zero, nunca um argumento estranho.
        let v = comando_video(Path::new("/bin/mpv"), Path::new("/v/a.mp4"), f64::NAN);
        assert_eq!(args(&v)[0], "--start=0.000");
        let v = comando_video(Path::new("/bin/mpv"), Path::new("/v/a.mp4"), -3.0);
        assert_eq!(args(&v)[0], "--start=0.000");
    }

    #[test]
    fn decodificador_guarda_o_ultimo_time_pos_e_ignora_o_resto() {
        let mut d = DecodificadorIpc::default();
        assert_eq!(d.posicao(), None);
        // Resposta do pedido, evento de outra propriedade e lixo não mexem na posição.
        d.alimentar(b"{\"data\":null,\"request_id\":0,\"error\":\"success\"}\n");
        d.alimentar(
            b"{\"event\":\"property-change\",\"id\":2,\"name\":\"pause\",\"data\":false}\n",
        );
        d.alimentar(b"isto nao e json\n");
        assert_eq!(d.posicao(), None);
        d.alimentar(
            b"{\"event\":\"property-change\",\"id\":1,\"name\":\"time-pos\",\"data\":0.5}\n",
        );
        assert_eq!(d.posicao(), Some(0.5));
        // Linha partida em dois pedaços.
        d.alimentar(b"{\"event\":\"property-change\",\"id\":1,\"na");
        assert_eq!(d.posicao(), Some(0.5));
        d.alimentar(b"me\":\"time-pos\",\"data\":1.25}\n{\"event\":\"idle\"}\n");
        assert_eq!(d.posicao(), Some(1.25));
        // Fim do arquivo (data nulo) mantém a última posição.
        d.alimentar(b"{\"event\":\"property-change\",\"id\":1,\"name\":\"time-pos\"}\n");
        assert_eq!(d.posicao(), Some(1.25));
        // Linha gigante sem fim não estoura a memória.
        d.alimentar(&vec![b'x'; MAX_LINHA + 10]);
        assert!(d.buffer.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn le_a_posicao_pelo_socket_sem_bloquear_com_um_mpv_falso() {
        use std::io::{Read, Write};
        use std::os::unix::net::UnixListener;
        let pasta = tempfile::tempdir().expect("pasta");
        let socket = pasta.path().join("mpv.sock");
        let ex = ExecutorFalso::default();
        let mut r = ReprodutorAudio::iniciar(
            &ex,
            Path::new("/bin/mpv"),
            Path::new("/a.opus"),
            socket.clone(),
        )
        .expect("inicia");
        // Ainda sem socket: o quadro volta na hora e o processo segue vivo.
        assert!(r.atualizar());
        assert!(!r.ipc_ligado());
        assert_eq!(r.posicao_s(), None);

        // O "mpv" cria o socket, recebe o pedido e manda posições.
        let servidor = UnixListener::bind(&socket).expect("socket");
        let fio = std::thread::spawn(move || {
            let (mut c, _) = servidor.accept().expect("aceita");
            let mut pedido = vec![0u8; PEDIDO_OBSERVAR_POSICAO.len()];
            c.read_exact(&mut pedido).expect("lê o pedido");
            assert_eq!(pedido, PEDIDO_OBSERVAR_POSICAO.as_bytes());
            c.write_all(
                b"{\"event\":\"property-change\",\"id\":1,\"name\":\"time-pos\",\"data\":0.40}\n",
            )
            .expect("manda");
            c.write_all(
                b"{\"event\":\"property-change\",\"id\":1,\"name\":\"time-pos\",\"data\":0.95}\n",
            )
            .expect("manda");
            std::thread::sleep(Duration::from_millis(300));
        });
        let fim = Instant::now() + Duration::from_secs(5);
        while r.posicao_s() != Some(0.95) {
            assert!(Instant::now() < fim, "a posição não chegou");
            assert!(r.atualizar());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(r.ipc_ligado());
        fio.join().expect("servidor falso");
        r.parar();
        assert!(!socket.exists(), "o socket é apagado ao parar");
    }

    #[test]
    fn midia_toca_audio_e_video_encerra_e_troca_sem_travar() {
        let (mut m, ex) = midia_falsa();
        assert!(m.tem_mpv());
        m.tocar_audio(Path::new("/c/audio/a.opus")).expect("toca");
        m.abrir_video(Path::new("/v/14.mp4"), 12.5).expect("abre");
        assert!(m.audio_tocando() && m.video_aberto());
        let cmds = ex.comandos();
        assert_eq!(cmds.len(), 2);
        assert!(args(&cmds[0])[2].starts_with("--input-ipc-server="));
        assert_eq!(args(&cmds[1]), ["--start=12.500", "/v/14.mp4"]);

        // Trocar o áudio encerra o anterior.
        m.tocar_audio(Path::new("/c/audio/b.opus")).expect("toca");
        assert_eq!(ex.vivos(), [false, true, true]);
        // O vídeo fecha sozinho: a próxima atualização o solta.
        ex.terminar(1);
        m.atualizar();
        assert!(!m.video_aberto());
        assert!(m.audio_tocando());
        // Sair da tela encerra tudo.
        m.parar_tudo();
        assert_eq!(ex.vivos(), [false, false, false]);
        assert!(!m.audio_tocando());
    }

    #[test]
    fn largar_a_midia_encerra_os_processos() {
        let (mut m, ex) = midia_falsa();
        m.tocar_audio(Path::new("/c/a.opus")).expect("toca");
        m.abrir_video(Path::new("/v/a.mp4"), 0.0).expect("abre");
        drop(m);
        assert_eq!(ex.vivos(), [false, false]);
    }

    #[test]
    fn sem_mpv_ou_com_falha_a_mensagem_e_clara_e_nada_trava() {
        let mut m = Midia::com(Box::new(ExecutorFalso::default()), None);
        assert!(!m.tem_mpv());
        let e = m.tocar_audio(Path::new("/a.opus")).expect_err("sem mpv");
        assert!(e.contains("instale") && e.contains("mpv"), "{e}");
        assert_eq!(m.abrir_video(Path::new("/a.mp4"), 1.0), Err(e));
        let (mut m, ex) = midia_falsa();
        *ex.falhar.lock().expect("trava") = true;
        let e = m.tocar_audio(Path::new("/a.opus")).expect_err("falha");
        assert!(e.starts_with("Não consegui abrir o mpv"), "{e}");
        assert!(!m.audio_tocando());
    }
}
