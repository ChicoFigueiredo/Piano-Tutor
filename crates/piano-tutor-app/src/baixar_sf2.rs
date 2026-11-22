//! Download do SoundFont padrão na primeira execução (F03): sem sudo, nas três plataformas.
//!
//! Quando nenhum SF2 é encontrado no sistema, o app baixa o FluidR3_GM (MIT, Frank Wen) da
//! release `soundfont-fluidr3-gm-1` do próprio repositório, confere tamanho e SHA-256 e grava
//! na pasta de dados do usuário (`dirs::data_dir()`: XDG no Linux, `%APPDATA%` no Windows,
//! `~/Library/Application Support` no macOS). O arquivo é escrito como `.part` e só ganha o
//! nome final depois de conferido, então um download interrompido nunca vira SF2 "válido".
//!
//! A rede fica atrás de [`AbridorDownload`], para os testes rodarem sem conexão.

use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

/// Um arquivo para baixar, com a identidade conferida.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FonteSf2 {
    pub nome: &'static str,
    pub url: &'static str,
    pub tamanho: u64,
    /// SHA-256 em hexadecimal minúsculo.
    pub sha256: &'static str,
}

/// FluidR3_GM.sf2 sem modificações, do pacote Ubuntu `fluid-soundfont-gm` 3.1-5.3build1.
pub const FLUIDR3_GM: FonteSf2 = FonteSf2 {
    nome: "FluidR3_GM.sf2",
    url: "https://github.com/ChicoFigueiredo/Piano-Tutor/releases/download/soundfont-fluidr3-gm-1/FluidR3_GM.sf2",
    tamanho: 148_398_306,
    sha256: "74594e8f4250680adf590507a306655a299935343583256f3b722c48a1bc1cb0",
};

/// Pasta dos SoundFonts do usuário dentro de uma pasta de dados.
pub fn pasta_soundfonts(dados: &Path) -> PathBuf {
    dados.join("piano-tutor").join("soundfonts")
}

/// Pasta dos SoundFonts do usuário nesta plataforma (`None` se o SO não informar).
pub fn pasta_soundfonts_do_usuario() -> Option<PathBuf> {
    dirs::data_dir().map(|d| pasta_soundfonts(&d))
}

/// Onde o SF2 padrão baixado fica (ou ficará) nesta plataforma.
pub fn caminho_baixado() -> Option<PathBuf> {
    pasta_soundfonts_do_usuario().map(|p| p.join(FLUIDR3_GM.nome))
}

#[derive(Debug)]
pub enum ErroDownload {
    Rede(String),
    Disco(String),
    Tamanho { esperado: u64, recebido: u64 },
    Hash { esperado: String, recebido: String },
    SemPasta,
}

impl fmt::Display for ErroDownload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErroDownload::Rede(e) => write!(f, "falha de rede: {e}"),
            ErroDownload::Disco(e) => write!(f, "falha ao gravar no disco: {e}"),
            ErroDownload::Tamanho { esperado, recebido } => {
                write!(f, "tamanho inesperado ({recebido} de {esperado} bytes)")
            }
            ErroDownload::Hash { .. } => write!(f, "o arquivo baixado não confere (SHA-256)"),
            ErroDownload::SemPasta => write!(f, "pasta de dados do usuário desconhecida"),
        }
    }
}

impl std::error::Error for ErroDownload {}

/// Abre a leitura de uma URL. A real usa `ureq` (respeita `HTTPS_PROXY`).
pub type AbridorDownload =
    Arc<dyn Fn(&str) -> Result<Box<dyn Read + Send>, ErroDownload> + Send + Sync + 'static>;

/// Abridor real por HTTPS.
pub fn abridor_http() -> AbridorDownload {
    Arc::new(|url: &str| {
        let resposta = ureq::get(url)
            .call()
            .map_err(|e| ErroDownload::Rede(e.to_string()))?;
        Ok(Box::new(resposta.into_body().into_reader()) as Box<dyn Read + Send>)
    })
}

/// Copia `leitor` para `destino`, conferindo tamanho e SHA-256 de `fonte`. Escreve em
/// `destino.part` e renomeia só no fim; em qualquer erro o `.part` é apagado e `destino`
/// não é tocado. `progresso` recebe o total de bytes já gravados.
pub fn baixar_para(
    fonte: &FonteSf2,
    leitor: &mut dyn Read,
    destino: &Path,
    mut progresso: impl FnMut(u64),
) -> Result<PathBuf, ErroDownload> {
    let disco = |e: io::Error| ErroDownload::Disco(e.to_string());
    if let Some(pai) = destino.parent() {
        fs::create_dir_all(pai).map_err(disco)?;
    }
    let parcial = destino.with_extension("sf2.part");
    let r = (|| {
        let mut saida = BufWriter::new(File::create(&parcial).map_err(disco)?);
        let mut hash = Sha256::new();
        let mut buf = vec![0u8; 256 * 1024];
        let mut total = 0u64;
        loop {
            let n = match leitor.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(ErroDownload::Rede(e.to_string())),
            };
            total += n as u64;
            if total > fonte.tamanho {
                return Err(ErroDownload::Tamanho {
                    esperado: fonte.tamanho,
                    recebido: total,
                });
            }
            hash.update(&buf[..n]);
            saida.write_all(&buf[..n]).map_err(disco)?;
            progresso(total);
        }
        saida.flush().map_err(disco)?;
        drop(saida);
        if total != fonte.tamanho {
            return Err(ErroDownload::Tamanho {
                esperado: fonte.tamanho,
                recebido: total,
            });
        }
        let recebido = hex(&hash.finalize());
        if recebido != fonte.sha256 {
            return Err(ErroDownload::Hash {
                esperado: fonte.sha256.to_owned(),
                recebido,
            });
        }
        fs::rename(&parcial, destino).map_err(disco)?;
        Ok(destino.to_path_buf())
    })();
    if r.is_err() {
        let _ = fs::remove_file(&parcial);
    }
    r
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Estado do download visto pela UI.
#[derive(Clone, Debug, PartialEq)]
pub enum EstadoDownload {
    Parado,
    Baixando { recebidos: u64, total: u64 },
    Concluido(PathBuf),
    Falhou(String),
}

enum Msg {
    Progresso(u64),
    Fim(Result<PathBuf, ErroDownload>),
}

/// Download em segundo plano, consultado a cada quadro pela thread da UI.
pub struct BaixadorSf2 {
    fonte: FonteSf2,
    abridor: AbridorDownload,
    aviso: Option<Arc<dyn Fn() + Send + Sync>>,
    rx: Option<Receiver<Msg>>,
    estado: EstadoDownload,
}

impl fmt::Debug for BaixadorSf2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BaixadorSf2")
            .field("fonte", &self.fonte.nome)
            .field("estado", &self.estado)
            .finish_non_exhaustive()
    }
}

impl BaixadorSf2 {
    pub fn novo(fonte: FonteSf2, abridor: AbridorDownload) -> Self {
        Self {
            fonte,
            abridor,
            aviso: None,
            rx: None,
            estado: EstadoDownload::Parado,
        }
    }

    /// Chamado a cada avanço (pedir um quadro à UI).
    pub fn definir_aviso(&mut self, aviso: Arc<dyn Fn() + Send + Sync>) {
        self.aviso = Some(aviso);
    }

    pub fn estado(&self) -> &EstadoDownload {
        &self.estado
    }

    pub fn baixando(&self) -> bool {
        matches!(self.estado, EstadoDownload::Baixando { .. })
    }

    /// Começa a baixar para `destino` (ignorado se já estiver baixando).
    pub fn iniciar(&mut self, destino: Option<PathBuf>) {
        if self.baixando() {
            return;
        }
        let Some(destino) = destino else {
            self.estado = EstadoDownload::Falhou(ErroDownload::SemPasta.to_string());
            return;
        };
        let (tx, rx) = mpsc::channel();
        let fonte = self.fonte;
        let abridor = Arc::clone(&self.abridor);
        let aviso = self.aviso.clone();
        let avisar = move || {
            if let Some(a) = &aviso {
                a();
            }
        };
        let r = std::thread::Builder::new()
            .name("piano-tutor-baixar-sf2".to_owned())
            .spawn(move || {
                let mut ultimo = 0u64;
                let r = abridor(fonte.url).and_then(|mut leitor| {
                    baixar_para(&fonte, &mut *leitor, &destino, |n| {
                        // Um aviso a cada ~1 MiB basta para a barra andar.
                        if n - ultimo >= 1 << 20 || n == fonte.tamanho {
                            ultimo = n;
                            let _ = tx.send(Msg::Progresso(n));
                            avisar();
                        }
                    })
                });
                let _ = tx.send(Msg::Fim(r));
                avisar();
            });
        match r {
            Ok(_) => {
                self.rx = Some(rx);
                self.estado = EstadoDownload::Baixando {
                    recebidos: 0,
                    total: self.fonte.tamanho,
                };
            }
            Err(e) => self.estado = EstadoDownload::Falhou(e.to_string()),
        }
    }

    /// Lê as novidades. Devolve o caminho uma única vez, quando o download acaba bem.
    pub fn atualizar(&mut self) -> Option<PathBuf> {
        let rx = self.rx.as_ref()?;
        loop {
            match rx.try_recv() {
                Ok(Msg::Progresso(n)) => {
                    self.estado = EstadoDownload::Baixando {
                        recebidos: n,
                        total: self.fonte.tamanho,
                    };
                }
                Ok(Msg::Fim(r)) => {
                    self.rx = None;
                    return match r {
                        Ok(p) => {
                            self.estado = EstadoDownload::Concluido(p.clone());
                            Some(p)
                        }
                        Err(e) => {
                            self.estado = EstadoDownload::Falhou(e.to_string());
                            None
                        }
                    };
                }
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    self.rx = None;
                    self.estado = EstadoDownload::Falhou("o download parou sem resposta".into());
                    return None;
                }
            }
        }
    }

    /// Texto curto para a UI, se houver algo a mostrar.
    pub fn texto(&self) -> Option<String> {
        match &self.estado {
            EstadoDownload::Parado | EstadoDownload::Concluido(_) => None,
            EstadoDownload::Baixando { recebidos, total } => Some(format!(
                "Baixando o SoundFont FluidR3_GM: {} de {} MB ({}%)",
                recebidos / 1_000_000,
                total / 1_000_000,
                recebidos * 100 / (*total).max(1)
            )),
            EstadoDownload::Falhou(e) => Some(format!(
                "Não foi possível baixar o SoundFont ({e}). Verifique a internet e tente de novo \
                 na tela Timbres."
            )),
        }
    }

    /// Fração de 0 a 1 enquanto baixa.
    pub fn fracao(&self) -> Option<f32> {
        match self.estado {
            EstadoDownload::Baixando { recebidos, total } => {
                Some(recebidos as f32 / total.max(1) as f32)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::time::{Duration, Instant};

    const CONTEUDO: &[u8] = b"RIFF\x10\x00\x00\x00sfbk conteudo de teste do soundfont";

    fn fonte_de_teste() -> FonteSf2 {
        // SHA-256 de CONTEUDO calculado uma vez e fixado aqui.
        FonteSf2 {
            nome: "Teste.sf2",
            url: "https://exemplo.invalido/Teste.sf2",
            tamanho: CONTEUDO.len() as u64,
            sha256: SHA_CONTEUDO,
        }
    }
    const SHA_CONTEUDO: &str = "98124efdb0f4470fed35d051bbebaf2d6c134aa89feecab0cb629865485f3ec7";

    fn pasta(nome: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("pt-baixar-{nome}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn sha_de_teste_confere_com_o_conteudo() {
        assert_eq!(hex(&Sha256::digest(CONTEUDO)), SHA_CONTEUDO);
    }

    #[test]
    fn baixa_confere_e_renomeia_sem_deixar_parcial() {
        let dir = pasta("ok");
        let destino = dir.join("Teste.sf2");
        let mut vistos = Vec::new();
        let r = baixar_para(&fonte_de_teste(), &mut &CONTEUDO[..], &destino, |n| {
            vistos.push(n)
        });
        assert_eq!(r.expect("download ok"), destino);
        assert_eq!(fs::read(&destino).expect("lê destino"), CONTEUDO);
        assert!(!destino.with_extension("sf2.part").exists());
        assert_eq!(vistos.last(), Some(&(CONTEUDO.len() as u64)));
    }

    #[test]
    fn hash_errado_nao_cria_o_arquivo_final() {
        let dir = pasta("hash");
        let destino = dir.join("Teste.sf2");
        let mut fonte = fonte_de_teste();
        fonte.sha256 = "00";
        let r = baixar_para(&fonte, &mut &CONTEUDO[..], &destino, |_| {});
        assert!(matches!(r, Err(ErroDownload::Hash { .. })), "{r:?}");
        assert!(!destino.exists());
        assert!(!destino.with_extension("sf2.part").exists());
    }

    #[test]
    fn arquivo_incompleto_ou_maior_falha_por_tamanho() {
        let dir = pasta("tam");
        let destino = dir.join("Teste.sf2");
        let r = baixar_para(&fonte_de_teste(), &mut &CONTEUDO[..10], &destino, |_| {});
        assert!(
            matches!(r, Err(ErroDownload::Tamanho { recebido: 10, .. })),
            "{r:?}"
        );
        let mut maior = CONTEUDO.to_vec();
        maior.extend_from_slice(b"extra");
        let r = baixar_para(&fonte_de_teste(), &mut &maior[..], &destino, |_| {});
        assert!(matches!(r, Err(ErroDownload::Tamanho { .. })), "{r:?}");
        assert!(!destino.exists());
    }

    #[test]
    fn pasta_fica_dentro_da_pasta_de_dados() {
        assert_eq!(
            pasta_soundfonts(Path::new("/dados")),
            PathBuf::from("/dados/piano-tutor/soundfonts")
        );
    }

    fn esperar(b: &mut BaixadorSf2) -> Option<PathBuf> {
        let fim = Instant::now() + Duration::from_secs(5);
        while Instant::now() < fim {
            if let Some(p) = b.atualizar() {
                return Some(p);
            }
            if matches!(b.estado(), EstadoDownload::Falhou(_)) {
                return None;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("download de teste não terminou");
    }

    #[test]
    fn baixador_em_segundo_plano_entrega_o_caminho_uma_vez() {
        let dir = pasta("thread");
        let abridor: AbridorDownload =
            Arc::new(|_| Ok(Box::new(io::Cursor::new(CONTEUDO.to_vec())) as Box<dyn Read + Send>));
        let mut b = BaixadorSf2::novo(fonte_de_teste(), abridor);
        b.iniciar(Some(dir.join("Teste.sf2")));
        assert!(b.baixando());
        assert!(b.texto().expect("texto").contains("Baixando"));
        let p = esperar(&mut b).expect("deveria concluir");
        assert_eq!(p, dir.join("Teste.sf2"));
        assert_eq!(b.atualizar(), None);
        assert_eq!(b.texto(), None);
    }

    #[test]
    fn falha_de_rede_vira_texto_para_a_ui() {
        let abridor: AbridorDownload = Arc::new(|_| Err(ErroDownload::Rede("sem conexão".into())));
        let mut b = BaixadorSf2::novo(fonte_de_teste(), abridor);
        b.iniciar(Some(pasta("rede").join("Teste.sf2")));
        assert_eq!(esperar(&mut b), None);
        let t = b.texto().expect("texto");
        assert!(t.contains("sem conexão") && t.contains("Timbres"), "{t}");
    }

    #[test]
    fn sem_pasta_de_dados_falha_sem_thread() {
        let abridor: AbridorDownload = Arc::new(|_| panic!("não deveria abrir"));
        let mut b = BaixadorSf2::novo(fonte_de_teste(), abridor);
        b.iniciar(None);
        assert!(matches!(b.estado(), EstadoDownload::Falhou(_)));
    }
}
