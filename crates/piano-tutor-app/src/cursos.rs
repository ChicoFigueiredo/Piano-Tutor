//! Cursos (M4, etapa 5, parte do aluno): o estado puro por trás da tela de Cursos, sem janela.
//!
//! - o **seletor**: o Curso de fábrica (os dez exercícios guiados) e os Cursos de tutor
//!   instalados em `<dados>/cursos/<id>/`, cada um com título, autor, licenciado, chave curta
//!   e o selo "confiável", "chave desconhecida" ou "assinatura inválida" (nunca só cor);
//! - a **árvore** Seções → Lições: Lição `assistir` leva ao vídeo; Lição `exercicio` lista só os
//!   Exercícios `aprovada` e os **não revisados** (`hipotese` e `em_revisao`), estes só com a
//!   preferência "Usar conteúdo não revisado" (padrão: ligada; uso pessoal do licenciado, com o
//!   selo "não revisado"). Desligada, só os aprovados aparecem e os demais viram contagem
//!   ("aguardando revisão do tutor"). `rejeitada` nunca aparece para o aluno;
//! - o **progresso** separado por Curso, em `<dados>/cursos/progresso/<id>.txt`;
//! - o **Exercício de Curso** aberto: partitura (a do modo Praticar existente), áudio do
//!   professor com cursor ([`indice_da_nota_no_instante`]) e vídeo da Aula (`mpv --start`);
//! - a **instalação** de um `.ptcurso` em segundo plano ([`EstadoCursos::iniciar_instalacao`]).
//!
//! Nada aqui abre janela, executa `mpv` nem fala; as telas e o app ligam isso ao resto.

use crate::mpv::Midia;
use crate::partitura_visual::{Destaque, Mao as MaoVisual};
use crate::tela_partitura::EstadoPartitura;
use piano_tutor_curso::{
    Alinhamento, Aula, ErroPacote, Exercicio, Licao, Manifesto, Mao, Modo, Momento, Roteiro,
    TipoLicao, caminho_seguro, chaves_confiaveis, cursos_instalados, instalar,
    ler_pacote_de_arquivo, sha256_hex,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

/// Id do Curso de fábrica (o primeiro da lista).
pub const ID_FABRICA: &str = "fabrica";

/// Precisão mínima (em %) para um Exercício contar como **concluído**; abaixo disso só a
/// melhor precisão é guardada.
pub const LIMIAR_CONCLUIDO_PCT: f32 = 80.0;

/// Teto de um `manifesto.toml` lido da pasta instalada (o mesmo do leitor do pacote).
const MAX_MANIFESTO_BYTES: u64 = 8 * 1024 * 1024;

// --- Pastas -----------------------------------------------------------------------------------

/// `<dados>/cursos/progresso/<id>.txt`.
pub fn arquivo_de_progresso(dados: &Path, curso: &str) -> PathBuf {
    dados
        .join("cursos")
        .join("progresso")
        .join(format!("{curso}.txt"))
}

/// `<dados>/cursos/progresso/<id>-videos.txt`: a pasta onde o aluno guarda os vídeos do Curso.
pub fn arquivo_da_pasta_de_videos(dados: &Path, curso: &str) -> PathBuf {
    dados
        .join("cursos")
        .join("progresso")
        .join(format!("{curso}-videos.txt"))
}

// --- Progresso --------------------------------------------------------------------------------

/// Situação de um Exercício no progresso.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemDeProgresso {
    pub concluido: bool,
    /// Melhor precisão já obtida, em % (0 a 100).
    pub melhor_pct: f32,
}

/// Progresso de um Curso: exercícios concluídos e a melhor precisão de cada um.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProgressoCurso {
    itens: Vec<(String, ItemDeProgresso)>,
}

impl ProgressoCurso {
    /// Formato: uma linha `<id-do-exercicio>=<0|1>,<melhor %>`.
    pub fn para_texto(&self) -> String {
        let mut t = String::from("# Piano Tutor: progresso do Curso v1\n");
        for (id, i) in &self.itens {
            let _ = writeln!(t, "{id}={},{:.1}", u8::from(i.concluido), i.melhor_pct);
        }
        t
    }

    /// Lê o texto; linhas ruins, ids inválidos e números fora de 0 a 100 são ignorados.
    pub fn de_texto(texto: &str) -> Self {
        let mut p = Self::default();
        for linha in texto.lines() {
            let Some((id, resto)) = linha.trim().split_once('=') else {
                continue;
            };
            let Some((c, pct)) = resto.split_once(',') else {
                continue;
            };
            let Ok(pct) = pct.trim().parse::<f32>() else {
                continue;
            };
            let id = id.trim();
            if !piano_tutor_curso::id_valido(id)
                || !pct.is_finite()
                || !(0.0..=100.0).contains(&pct)
            {
                continue;
            }
            let concluido = match c.trim() {
                "1" => true,
                "0" => false,
                _ => continue,
            };
            p.itens.retain(|(x, _)| x != id);
            p.itens.push((
                id.to_owned(),
                ItemDeProgresso {
                    concluido,
                    melhor_pct: pct,
                },
            ));
        }
        p
    }

    pub fn ler(caminho: &Path) -> Self {
        match fs::metadata(caminho) {
            Ok(m) if m.len() <= 1024 * 1024 => fs::read_to_string(caminho)
                .map(|t| Self::de_texto(&t))
                .unwrap_or_default(),
            _ => Self::default(),
        }
    }

    /// Grava por um arquivo temporário e troca (nunca deixa o progresso pela metade).
    pub fn gravar(&self, caminho: &Path) -> std::io::Result<()> {
        if let Some(pai) = caminho.parent() {
            fs::create_dir_all(pai)?;
        }
        let parcial = caminho.with_extension("txt.part");
        fs::write(&parcial, self.para_texto())?;
        fs::rename(&parcial, caminho)
    }

    pub fn item(&self, exercicio: &str) -> Option<ItemDeProgresso> {
        self.itens
            .iter()
            .find(|(id, _)| id == exercicio)
            .map(|(_, i)| *i)
    }

    pub fn concluido(&self, exercicio: &str) -> bool {
        self.item(exercicio).is_some_and(|i| i.concluido)
    }

    /// Quantos Exercícios estão concluídos, entre os `ids` dados (só os que o aluno vê).
    pub fn concluidos_entre<'a>(&self, ids: impl IntoIterator<Item = &'a str>) -> usize {
        ids.into_iter().filter(|id| self.concluido(id)).count()
    }

    /// Registra um resultado: guarda a melhor precisão e marca concluído a partir de
    /// [`LIMIAR_CONCLUIDO_PCT`] (concluído não desfaz). Devolve `true` se algo mudou.
    pub fn registrar(&mut self, exercicio: &str, pct: f32) -> bool {
        if !piano_tutor_curso::id_valido(exercicio) || !pct.is_finite() {
            return false;
        }
        let pct = pct.clamp(0.0, 100.0);
        let atingiu = pct >= LIMIAR_CONCLUIDO_PCT;
        match self.itens.iter_mut().find(|(id, _)| id == exercicio) {
            Some((_, i)) => {
                let novo = ItemDeProgresso {
                    concluido: i.concluido || atingiu,
                    melhor_pct: i.melhor_pct.max(pct),
                };
                let mudou = *i != novo;
                *i = novo;
                mudou
            }
            None => {
                self.itens.push((
                    exercicio.to_owned(),
                    ItemDeProgresso {
                        concluido: atingiu,
                        melhor_pct: pct,
                    },
                ));
                true
            }
        }
    }
}

// --- Cursos listados --------------------------------------------------------------------------

/// O selo do Curso na lista: sempre com texto e símbolo, nunca só cor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selo {
    /// O Curso de fábrica, que faz parte do app.
    Fabrica,
    /// Assinado por uma chave em `chaves-confiaveis/` ou embutida no app.
    Confiavel,
    /// Assinatura válida, de uma chave que o app não conhece.
    ChaveDesconhecida,
    /// A assinatura guardada não confere mais com o manifesto instalado.
    AssinaturaInvalida,
}

impl Selo {
    pub const fn rotulo(self) -> &'static str {
        match self {
            Selo::Fabrica => "do Piano Tutor",
            Selo::Confiavel => "confiável",
            Selo::ChaveDesconhecida => "chave desconhecida",
            Selo::AssinaturaInvalida => "assinatura inválida",
        }
    }

    /// Símbolo em texto simples (a fonte da tela não tem ícones de marca).
    pub const fn simbolo(self) -> &'static str {
        match self {
            Selo::Fabrica | Selo::Confiavel => "[OK]",
            Selo::ChaveDesconhecida => "[!]",
            Selo::AssinaturaInvalida => "[X]",
        }
    }

    pub const fn explicacao(self) -> &'static str {
        match self {
            Selo::Fabrica => "Curso que já vem no Piano Tutor.",
            Selo::Confiavel => {
                "A chave de quem assinou este Curso está na sua lista de chaves confiáveis."
            }
            Selo::ChaveDesconhecida => {
                "O pacote é autêntico (a assinatura confere), mas o Piano Tutor não conhece esta chave. \
                 Só use se confia em quem lhe entregou o arquivo."
            }
            Selo::AssinaturaInvalida => {
                "O Curso instalado foi alterado depois da instalação. Instale o pacote de novo."
            }
        }
    }
}

/// Uma Lição na árvore do aluno.
#[derive(Clone, Debug, PartialEq)]
pub struct LicaoNaArvore {
    pub id: String,
    pub numero: u32,
    pub titulo: String,
    pub tipo: TipoLicao,
    pub aula: Option<Aula>,
    /// Os Exercícios que o aluno pode vir a abrir, na ordem do autor: `aprovada` e também os
    /// não revisados (`hipotese` e `em_revisao`). `rejeitada` nunca entra. Use
    /// [`LicaoNaArvore::visiveis`] para respeitar a preferência "Usar conteúdo não revisado".
    pub exercicios: Vec<Exercicio>,
    /// Quantos de `exercicios` ainda não foram revistos (`hipotese` ou `em_revisao`).
    pub aguardando: usize,
}

impl LicaoNaArvore {
    /// Exercícios que o aluno vê agora: só os aprovados, ou também os não revisados se
    /// `permitir_nao_revisado`.
    pub fn visiveis(&self, permitir_nao_revisado: bool) -> impl Iterator<Item = &Exercicio> {
        self.exercicios
            .iter()
            .filter(move |e| e.utilizavel(permitir_nao_revisado))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SecaoNaArvore {
    pub id: String,
    pub numero: u32,
    pub titulo: String,
    pub licoes: Vec<LicaoNaArvore>,
}

/// Seções → Lições para o aluno: Exercícios `aprovada` e não revisados (estes contados em
/// `aguardando`); `rejeitada` fica de fora. Quem desenha filtra com [`LicaoNaArvore::visiveis`].
pub fn arvore_para_aluno(m: &Manifesto) -> Vec<SecaoNaArvore> {
    m.secoes
        .iter()
        .map(|s| SecaoNaArvore {
            id: s.id.clone(),
            numero: s.numero,
            titulo: s.titulo.clone(),
            licoes: s
                .licoes
                .iter()
                .map(|l| LicaoNaArvore {
                    id: l.id.clone(),
                    numero: l.numero,
                    titulo: l.titulo.clone(),
                    tipo: l.tipo,
                    aula: l.aula.clone(),
                    exercicios: l
                        .exercicios
                        .iter()
                        .filter(|e| e.utilizavel(true))
                        .cloned()
                        .collect(),
                    aguardando: l.exercicios.iter().filter(|e| e.nao_revisado()).count(),
                })
                .collect(),
        })
        .collect()
}

/// Um Curso na lista do seletor.
#[derive(Clone, Debug)]
pub struct CursoListado {
    pub id: String,
    pub titulo: String,
    pub autor: String,
    pub licenciado: Option<String>,
    pub licenca: Option<String>,
    pub versao: String,
    /// `abcd…1234`; `None` no Curso de fábrica.
    pub chave_curta: Option<String>,
    pub selo: Selo,
    /// Pasta instalada; `None` no Curso de fábrica.
    pub pasta: Option<PathBuf>,
    pub manifesto: Option<Manifesto>,
    pub arvore: Vec<SecaoNaArvore>,
    /// Exercícios aprovados.
    pub aprovados: usize,
    /// Exercícios que aguardam revisão do tutor (`hipotese` e `em_revisao`).
    pub aguardando: usize,
}

impl CursoListado {
    pub fn e_fabrica(&self) -> bool {
        self.id == ID_FABRICA
    }

    /// Quantos Exercícios o aluno pode abrir: os aprovados e, com `permitir_nao_revisado`, os
    /// não revisados.
    pub fn praticaveis(&self, permitir_nao_revisado: bool) -> usize {
        self.aprovados
            + if permitir_nao_revisado {
                self.aguardando
            } else {
                0
            }
    }

    /// O Curso de fábrica: os dez exercícios guiados, que fazem parte do app.
    pub fn de_fabrica() -> Self {
        Self {
            id: ID_FABRICA.to_owned(),
            titulo: "Curso de fábrica".to_owned(),
            autor: "Piano Tutor".to_owned(),
            licenciado: None,
            licenca: None,
            versao: crate::estado::VERSAO.to_owned(),
            chave_curta: None,
            selo: Selo::Fabrica,
            pasta: None,
            manifesto: None,
            arvore: Vec::new(),
            aprovados: crate::exercicios::exercicios_de_fabrica().len(),
            aguardando: 0,
        }
    }
}

fn ler_manifesto_instalado(pasta: &Path) -> Option<Manifesto> {
    let caminho = pasta.join("manifesto.toml");
    if fs::metadata(&caminho).ok()?.len() > MAX_MANIFESTO_BYTES {
        return None;
    }
    let texto = fs::read_to_string(caminho).ok()?;
    Manifesto::de_toml(&texto).ok()
}

/// Curso de fábrica e depois os Cursos de tutor instalados em `<dados>/cursos/`, por id.
/// O segundo valor é o aviso, se a pasta não pôde ser lida.
pub fn listar_cursos(dados: Option<&Path>) -> (Vec<CursoListado>, Option<String>) {
    let mut lista = vec![CursoListado::de_fabrica()];
    let Some(dados) = dados else {
        return (lista, None);
    };
    let confiaveis = chaves_confiaveis(dados);
    let instalados = match cursos_instalados(dados) {
        Ok(i) => i,
        Err(e) => {
            return (
                lista,
                Some(format!("Não consegui listar os Cursos instalados: {e}")),
            );
        }
    };
    for c in instalados {
        let manifesto = ler_manifesto_instalado(&c.pasta);
        let selo = if !c.assinatura_valida {
            Selo::AssinaturaInvalida
        } else if confiaveis.contains(&c.chave_publica) {
            Selo::Confiavel
        } else {
            Selo::ChaveDesconhecida
        };
        let arvore = manifesto
            .as_ref()
            .map(arvore_para_aluno)
            .unwrap_or_default();
        let aprovados = arvore
            .iter()
            .flat_map(|s| &s.licoes)
            .map(|l| l.exercicios.len() - l.aguardando)
            .sum();
        let aguardando = arvore
            .iter()
            .flat_map(|s| &s.licoes)
            .map(|l| l.aguardando)
            .sum();
        lista.push(CursoListado {
            licenca: manifesto
                .as_ref()
                .map(|m| m.licenciado.licenca.clone())
                .filter(|l| !l.is_empty()),
            id: c.id,
            titulo: c.titulo,
            autor: c.autor,
            licenciado: Some(c.licenciado),
            versao: c.versao,
            chave_curta: Some(c.chave_publica.curta()),
            selo,
            pasta: Some(c.pasta),
            manifesto,
            arvore,
            aprovados,
            aguardando,
        });
    }
    (lista, None)
}

// --- Alinhamento ------------------------------------------------------------------------------

/// Índice da nota que soa no instante `t_audio_s` do áudio: a de maior `t_audio_s` que já
/// começou (empate: a de maior índice). `None` antes da primeira nota. É o que posiciona o
/// cursor da partitura enquanto o professor toca.
pub fn indice_da_nota_no_instante(a: &Alinhamento, t_audio_s: f64) -> Option<u32> {
    if !t_audio_s.is_finite() {
        return None;
    }
    a.notas
        .iter()
        .filter(|n| n.t_audio_s <= t_audio_s)
        .max_by(|x, y| {
            x.t_audio_s
                .total_cmp(&y.t_audio_s)
                .then(x.indice.cmp(&y.indice))
        })
        .map(|n| n.indice)
}

// --- Vídeos da Aula ---------------------------------------------------------------------------

/// A pasta que o aluno indicou para os vídeos deste Curso, se ainda existir.
pub fn ler_pasta_de_videos(dados: &Path, curso: &str) -> Option<PathBuf> {
    let t = fs::read_to_string(arquivo_da_pasta_de_videos(dados, curso)).ok()?;
    let p = PathBuf::from(t.lines().next()?.trim());
    p.is_dir().then_some(p)
}

/// Guarda a pasta dos vídeos (precisa existir).
pub fn gravar_pasta_de_videos(dados: &Path, curso: &str, pasta: &Path) -> Result<(), String> {
    if !pasta.is_dir() {
        return Err(format!("{} não é uma pasta.", pasta.display()));
    }
    let arq = arquivo_da_pasta_de_videos(dados, curso);
    if let Some(pai) = arq.parent() {
        fs::create_dir_all(pai).map_err(|e| format!("{}: {e}", pai.display()))?;
    }
    fs::write(&arq, format!("{}\n", pasta.display())).map_err(|e| format!("{}: {e}", arq.display()))
}

/// Acha o vídeo `nome` na pasta do aluno: o nome exato, ou sem diferença de maiúsculas, na
/// pasta ou numa subpasta direta (os vídeos costumam ficar uma pasta por Seção).
pub fn achar_video(pasta: &Path, nome: &str) -> Option<PathBuf> {
    if nome.is_empty() || nome.contains(['/', '\\']) || nome == "." || nome == ".." {
        return None;
    }
    let direto = pasta.join(nome);
    if direto.is_file() {
        return Some(direto);
    }
    let minusculo = nome.to_lowercase();
    let procurar = |dir: &Path| -> Option<PathBuf> {
        fs::read_dir(dir)
            .ok()?
            .filter_map(Result::ok)
            .find(|e| {
                e.file_name().to_string_lossy().to_lowercase() == minusculo && e.path().is_file()
            })
            .map(|e| e.path())
    };
    if let Some(p) = procurar(pasta) {
        return Some(p);
    }
    let mut subpastas: Vec<PathBuf> = fs::read_dir(pasta)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    subpastas.sort();
    subpastas.iter().take(64).find_map(|d| {
        if d.join(nome).is_file() {
            Some(d.join(nome))
        } else {
            procurar(d)
        }
    })
}

/// Resultado da conferência do arquivo de vídeo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResultadoSha {
    Confere,
    Diferente,
    Erro(String),
}

/// Conferência do SHA-256 do vídeo em segundo plano (o arquivo pode ter gigabytes).
pub struct VerificacaoSha {
    rx: Receiver<ResultadoSha>,
}

impl VerificacaoSha {
    /// Calcula o SHA-256 de `arquivo` numa thread e compara com `esperado` (hexadecimal).
    pub fn iniciar(arquivo: PathBuf, esperado: String) -> Self {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(sha256_do_arquivo(&arquivo).map_or_else(
                |e| ResultadoSha::Erro(format!("{}: {e}", arquivo.display())),
                |h| {
                    if h.eq_ignore_ascii_case(esperado.trim()) {
                        ResultadoSha::Confere
                    } else {
                        ResultadoSha::Diferente
                    }
                },
            ));
        });
        Self { rx }
    }

    /// O resultado, quando a thread terminar (não bloqueia).
    pub fn consultar(&self) -> Option<ResultadoSha> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(ResultadoSha::Erro(
                "a conferência do vídeo parou sem resultado".to_owned(),
            )),
        }
    }
}

fn sha256_do_arquivo(caminho: &Path) -> std::io::Result<String> {
    let mut f = fs::File::open(caminho)?;
    let mut h = Sha256::new();
    let mut bloco = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut bloco)?;
        if n == 0 {
            break;
        }
        h.update(&bloco[..n]);
    }
    Ok(h.finalize().iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    }))
}

// --- Exercício de Curso -----------------------------------------------------------------------

/// A Mão do manifesto na clave da tela: `Ambas` não filtra.
pub fn mao_para_partitura(m: Mao) -> Option<MaoVisual> {
    match m {
        Mao::Direita => Some(MaoVisual::Direita),
        Mao::Esquerda => Some(MaoVisual::Esquerda),
        Mao::Ambas => None,
    }
}

/// Um Exercício de Curso carregado do pacote instalado, pronto para abrir.
#[derive(Clone, Debug)]
pub struct ExercicioAberto {
    pub curso_id: String,
    pub curso_titulo: String,
    pub licao_id: String,
    pub licao_numero: u32,
    pub licao_titulo: String,
    pub aula: Option<Aula>,
    pub exercicio: Exercicio,
    pub xml: String,
    pub alinhamento: Option<Alinhamento>,
    /// `audio/<id>.opus` instalado, se o Exercício tem áudio do professor.
    pub audio: Option<PathBuf>,
    pub roteiro: Option<Roteiro>,
}

impl ExercicioAberto {
    pub fn modo(&self) -> Modo {
        self.exercicio.modo
    }
}

/// Lê um arquivo da pasta instalada e confere tamanho e SHA-256 contra `[[arquivos]]`: se algo
/// mudou depois da instalação, o Exercício não abre.
fn ler_conferido(pasta: &Path, m: &Manifesto, caminho: &str) -> Result<Vec<u8>, String> {
    if !caminho_seguro(caminho) {
        return Err(format!("caminho inseguro no Curso: {caminho}"));
    }
    let Some(a) = m.arquivos.iter().find(|a| a.caminho == caminho) else {
        return Err(format!("{caminho} não consta no manifesto do Curso"));
    };
    let bytes = fs::read(pasta.join(caminho)).map_err(|e| format!("{caminho}: {e}"))?;
    if bytes.len() as u64 != a.bytes || sha256_hex(&bytes) != a.sha256 {
        return Err(format!(
            "{caminho} foi alterado depois da instalação. Instale o pacote do Curso de novo."
        ));
    }
    Ok(bytes)
}

fn texto_utf8(bytes: Vec<u8>, caminho: &str) -> Result<String, String> {
    String::from_utf8(bytes).map_err(|_| format!("{caminho} não é um texto UTF-8 válido"))
}

/// Carrega o Exercício `exercicio_id` da Lição `licao_id` de um Curso de tutor instalado.
/// Recusa o `rejeitada`, o não revisado (`hipotese`, `em_revisao`) quando
/// `permitir_nao_revisado` é falso, e o Curso com assinatura inválida. O Exercício aberto
/// guarda o estado: a tela põe o selo "não revisado".
pub fn carregar_exercicio(
    curso: &CursoListado,
    licao_id: &str,
    exercicio_id: &str,
    permitir_nao_revisado: bool,
) -> Result<ExercicioAberto, String> {
    let (Some(m), Some(pasta)) = (curso.manifesto.as_ref(), curso.pasta.as_ref()) else {
        return Err("Este Curso não tem pacote instalado.".to_owned());
    };
    if curso.selo == Selo::AssinaturaInvalida {
        return Err(Selo::AssinaturaInvalida.explicacao().to_owned());
    }
    let licao: &Licao = m
        .secoes
        .iter()
        .flat_map(|s| &s.licoes)
        .find(|l| l.id == licao_id)
        .ok_or_else(|| format!("Lição {licao_id} não existe neste Curso."))?;
    let ex = licao
        .exercicios
        .iter()
        .find(|e| e.id == exercicio_id)
        .ok_or_else(|| format!("Exercício {exercicio_id} não existe nesta Lição."))?;
    if ex.estado == piano_tutor_curso::EstadoConteudo::Rejeitada {
        return Err("Este exercício foi rejeitado na revisão e não pode ser usado.".to_owned());
    }
    if !ex.utilizavel(permitir_nao_revisado) {
        return Err(
            "Este exercício ainda aguarda a revisão do tutor. Ligue «Usar conteúdo não revisado» \
             para praticá-lo."
                .to_owned(),
        );
    }
    let xml = texto_utf8(ler_conferido(pasta, m, &ex.partitura)?, &ex.partitura)?;
    let alinhamento = match &ex.alinhamento {
        Some(c) => {
            let bytes = ler_conferido(pasta, m, c)?;
            Some(Alinhamento::de_json(&bytes).map_err(|e| e.to_string())?)
        }
        None => None,
    };
    let audio = match &ex.audio {
        Some(c) if m.arquivos.iter().any(|a| &a.caminho == c) && caminho_seguro(c) => {
            let p = pasta.join(c);
            p.is_file().then_some(p)
        }
        _ => None,
    };
    let roteiro = if m.arquivos.iter().any(|a| a.caminho == "roteiro.toml") {
        let texto = texto_utf8(ler_conferido(pasta, m, "roteiro.toml")?, "roteiro.toml")?;
        Some(Roteiro::de_toml(&texto).map_err(|e| e.to_string())?)
    } else {
        None
    };
    Ok(ExercicioAberto {
        curso_id: m.id.clone(),
        curso_titulo: m.titulo.clone(),
        licao_id: licao.id.clone(),
        licao_numero: licao.numero,
        licao_titulo: licao.titulo.clone(),
        aula: licao.aula.clone(),
        exercicio: ex.clone(),
        xml,
        alinhamento,
        audio,
        roteiro,
    })
}

// --- Observador da prática --------------------------------------------------------------------

/// Quantos compassos tem um trecho quando o Roteiro não marca as frases (falas `passo`).
pub const COMPASSOS_POR_TRECHO: usize = 2;

/// O que aconteceu na prática desde o último quadro (o professor reage a isto).
///
/// Nota errada ou certa isolada **não** vira evento: o retorno de cada nota é só o visual (verde
/// ou vermelho na pauta). O professor comenta acerto e erro por **trecho** ([`FimDeTrecho`]).
///
/// [`FimDeTrecho`]: EventoPratica::FimDeTrecho
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EventoPratica {
    /// A prática chegou à nota de índice `i` (momento `passo` do Roteiro).
    Passo(usize),
    /// Terminou um trecho (uma frase, ou [`COMPASSOS_POR_TRECHO`] compassos, ou o fim da
    /// prática): `limpo` diz se foi sem erro (momento `acerto`) ou com erro (momento `erro`).
    /// Vem sempre **antes** do `Passo` que abre o trecho seguinte e da `Concluida`.
    FimDeTrecho { limpo: bool },
    /// A prática terminou; `pct` é a precisão e `concluiu` diz se bateu o limiar.
    Concluida { pct: f32, concluiu: bool },
}

/// Índices de nota que o Roteiro marca como começo de frase: as falas `passo` do Exercício
/// (aprovadas ou não: a fronteira não depende de a fala já ser dita).
pub fn fronteiras_de_frase(roteiro: Option<&Roteiro>, licao: &str, exercicio: &str) -> Vec<usize> {
    let mut v: Vec<usize> = roteiro
        .into_iter()
        .flat_map(|r| r.falas.iter())
        .filter(|f| {
            f.momento == Momento::Passo
                && f.licao == licao
                && f.exercicio.as_deref().is_none_or(|e| e == exercicio)
        })
        .filter_map(|f| f.passo.map(|p| p as usize))
        .collect();
    v.sort_unstable();
    v.dedup();
    v
}

#[derive(Debug, Default)]
struct ObservadorPratica {
    /// Começos de frase do Roteiro; vazio = trechos de [`COMPASSOS_POR_TRECHO`] compassos.
    fronteiras: Vec<usize>,
    em_andamento: bool,
    registrada: bool,
    ultimo_indice: Option<usize>,
    /// Compasso da primeira nota vista (os blocos de compassos contam a partir dele).
    compasso_base: Option<usize>,
    /// Erros (notas a mais e esperadas perdidas) já contados quando o trecho atual começou.
    erros_ate_o_trecho: usize,
}

fn erros_do_resumo(r: &piano_tutor_core::pratica::Resumo) -> usize {
    r.extras + r.faltas
}

impl ObservadorPratica {
    fn com_fronteiras(fronteiras: Vec<usize>) -> Self {
        Self {
            fronteiras,
            ..Self::default()
        }
    }

    /// `i` abre um trecho novo em relação à nota anterior `antes`?
    fn abre_trecho(&self, p: &EstadoPartitura, antes: usize, i: usize) -> bool {
        if !self.fronteiras.is_empty() {
            return self.fronteiras.iter().any(|&b| antes < b && b <= i);
        }
        let bloco = |n: usize| {
            let c = p.destaque_da_nota(n)?.compasso;
            Some(c.saturating_sub(self.compasso_base?) / COMPASSOS_POR_TRECHO)
        };
        matches!((bloco(antes), bloco(i)), (Some(a), Some(b)) if b != a)
    }

    fn observar(&mut self, p: &EstadoPartitura) -> Vec<EventoPratica> {
        let mut eventos = Vec::new();
        if p.praticando() {
            if !self.em_andamento {
                *self = Self {
                    em_andamento: true,
                    ..Self::com_fronteiras(std::mem::take(&mut self.fronteiras))
                };
            }
            let erros = p.resumo_pratica().map_or(0, |r| erros_do_resumo(&r));
            if let Some(i) = p.indice_da_pratica()
                && self.ultimo_indice != Some(i)
            {
                if self.compasso_base.is_none() {
                    self.compasso_base = p.destaque_da_nota(i).map(|d| d.compasso);
                }
                if let Some(antes) = self.ultimo_indice
                    && self.abre_trecho(p, antes, i)
                {
                    eventos.push(EventoPratica::FimDeTrecho {
                        limpo: erros <= self.erros_ate_o_trecho,
                    });
                    self.erros_ate_o_trecho = erros;
                }
                self.ultimo_indice = Some(i);
                eventos.push(EventoPratica::Passo(i));
            }
        } else if self.em_andamento {
            self.em_andamento = false;
            if !self.registrada
                && let Some(r) = p.resumo_final_da_pratica()
            {
                self.registrada = true;
                let pct = r.precisao_pct() as f32;
                let concluiu = pct >= LIMIAR_CONCLUIDO_PCT;
                // O último trecho só é "limpo" se não teve erro e a prática bateu o limiar.
                eventos.push(EventoPratica::FimDeTrecho {
                    limpo: concluiu && erros_do_resumo(&r) <= self.erros_ate_o_trecho,
                });
                eventos.push(EventoPratica::Concluida { pct, concluiu });
            }
        }
        eventos
    }
}

// --- Observador do Curso de fábrica -----------------------------------------------------------

/// O que aconteceu no exercício de fábrica desde o último quadro.
///
/// Passo certo ou nota errada isolados não geram evento (o retorno de cada nota é só o visual).
/// Os 10 exercícios de fábrica têm de 1 a 10 passos, curtos: o trecho comentado é o exercício
/// inteiro, ao concluir (não há comentário a cada N passos).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventoFabrica {
    /// O exercício de id dado foi aberto ou trocado: momento `antes` do Roteiro.
    Abriu(String),
    /// O exercício foi concluído: `acerto` (`sem_erros`) ou `erro` sobre a tentativa toda e,
    /// em seguida, o momento `depois`.
    Concluiu { sem_erros: bool },
}

/// Olha o [`EstadoExercicios`] a cada quadro e diz o que o professor deve comentar.
#[derive(Debug, Default)]
pub struct ObservadorFabrica {
    id: Option<String>,
    vistos: usize,
    /// Notas erradas da tentativa em curso (zera ao reiniciar ou ao concluir).
    erros: usize,
}

impl ObservadorFabrica {
    /// Esquece o exercício visto (ao entrar na tela): o próximo quadro gera `Abriu`.
    pub fn reiniciar(&mut self) {
        *self = Self::default();
    }

    pub fn observar(&mut self, e: &crate::exercicios::EstadoExercicios) -> Vec<EventoFabrica> {
        use crate::exercicios::Classificacao::{Errada, ExercicioCompleto};
        let Some(ex) = e.exercicio() else {
            return Vec::new();
        };
        let registro = &e.tentativa.registro;
        if self.id.as_deref() != Some(ex.id.as_str()) {
            self.id = Some(ex.id.clone());
            self.vistos = registro.len();
            self.erros = 0;
            return vec![EventoFabrica::Abriu(ex.id.clone())];
        }
        // "Reiniciar" esvazia o registro: a tentativa recomeça sem erros.
        if registro.len() < self.vistos {
            self.erros = 0;
        }
        self.vistos = self.vistos.min(registro.len());
        let mut eventos = Vec::new();
        for ev in &registro[self.vistos..] {
            match ev.resultado {
                Errada => self.erros += 1,
                ExercicioCompleto => {
                    eventos.push(EventoFabrica::Concluiu {
                        sem_erros: self.erros == 0,
                    });
                    self.erros = 0;
                }
                _ => {}
            }
        }
        self.vistos = registro.len();
        eventos
    }
}

// --- Estado -----------------------------------------------------------------------------------

/// Mensagem para a tela: informação ou erro, sempre em português.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mensagem {
    pub erro: bool,
    pub texto: String,
}

impl Mensagem {
    pub fn info(t: impl Into<String>) -> Self {
        Self {
            erro: false,
            texto: t.into(),
        }
    }

    pub fn erro(t: impl Into<String>) -> Self {
        Self {
            erro: true,
            texto: t.into(),
        }
    }
}

/// O que a instalação de um pacote devolve para a tela.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResumoInstalacao {
    pub id: String,
    pub titulo: String,
    pub autor: String,
    pub chave_curta: String,
    pub confiavel: bool,
}

/// `~/` vira a pasta do usuário; aspas e espaços nas pontas saem (caminhos colados do gerenciador
/// de arquivos).
pub fn caminho_digitado(texto: &str) -> PathBuf {
    let t = texto.trim().trim_matches(['"', '\'']);
    match t.strip_prefix("~/") {
        Some(resto) => dirs::home_dir().map_or_else(|| PathBuf::from(t), |h| h.join(resto)),
        None => PathBuf::from(t),
    }
}

fn instalar_arquivo(caminho: &Path, dados: &Path) -> Result<ResumoInstalacao, String> {
    let pacote = ler_pacote_de_arquivo(caminho)
        .map_err(|e: ErroPacote| format!("Não instalei o pacote: {e}"))?;
    let confiavel =
        pacote.confianca(&chaves_confiaveis(dados)) == piano_tutor_curso::Confianca::Confiavel;
    instalar(&pacote, dados).map_err(|e| format!("Não instalei o pacote: {e}"))?;
    Ok(ResumoInstalacao {
        id: pacote.manifesto.id.clone(),
        titulo: pacote.manifesto.titulo.clone(),
        autor: pacote.manifesto.autor.nome.clone(),
        chave_curta: pacote.chave_publica.curta(),
        confiavel,
    })
}

/// Estado da tela de Cursos.
pub struct EstadoCursos {
    dados: Option<PathBuf>,
    pub cursos: Vec<CursoListado>,
    selecionado: usize,
    pub campo_pacote: String,
    pub mensagem: Option<Mensagem>,
    instalando: Option<Receiver<Result<ResumoInstalacao, String>>>,
    progressos: HashMap<String, ProgressoCurso>,
    pub aberto: Option<ExercicioAberto>,
    /// A partitura do Exercício de Curso: a mesma lógica e o mesmo modo Praticar da tela
    /// Partitura, numa instância própria.
    pub partitura: EstadoPartitura,
    pub midia: Midia,
    pub campo_videos: String,
    verificacao: Option<VerificacaoSha>,
    /// Aviso sobre o vídeo (arquivo não achado, falta de mpv, vídeo diferente do Curso).
    pub aviso_video: Option<Mensagem>,
    cursor_audio: Option<Destaque>,
    observador: ObservadorPratica,
    /// Preferência "Usar conteúdo não revisado" (a do app; padrão ligada). O app a mantém em
    /// dia com a do professor, que é quem a grava.
    pub usar_nao_revisado: bool,
    /// Player de Aula aberto (ADR-0011): vídeo embutido, trechos e Modo de acompanhamento.
    pub player: Option<crate::tela_player_aula::EstadoPlayer>,
    /// Notas que o aluno ligou desde o último quadro (o player as consome).
    pub notas_do_aluno: Vec<u8>,
}

impl std::fmt::Debug for EstadoCursos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EstadoCursos")
            .field("cursos", &self.cursos.len())
            .field("selecionado", &self.selecionado)
            .finish_non_exhaustive()
    }
}

impl EstadoCursos {
    pub fn novo(dados: Option<PathBuf>, midia: Midia) -> Self {
        let partitura = EstadoPartitura::novo(dados.clone());
        let mut e = Self {
            dados,
            cursos: Vec::new(),
            selecionado: 0,
            campo_pacote: String::new(),
            mensagem: None,
            instalando: None,
            progressos: HashMap::new(),
            aberto: None,
            partitura,
            midia,
            campo_videos: String::new(),
            verificacao: None,
            aviso_video: None,
            player: None,
            notas_do_aluno: Vec::new(),
            cursor_audio: None,
            observador: ObservadorPratica::default(),
            usar_nao_revisado: true,
        };
        e.recarregar();
        e
    }

    pub fn pasta_dados(&self) -> Option<&Path> {
        self.dados.as_deref()
    }

    /// Lê de novo a lista de Cursos instalados e o progresso de cada um.
    pub fn recarregar(&mut self) {
        let (cursos, aviso) = listar_cursos(self.dados.as_deref());
        self.cursos = cursos;
        if let Some(a) = aviso {
            self.mensagem = Some(Mensagem::erro(a));
        }
        self.selecionado = self.selecionado.min(self.cursos.len() - 1);
        self.progressos.clear();
        if let Some(d) = self.dados.as_deref() {
            for c in self.cursos.iter().filter(|c| !c.e_fabrica()) {
                self.progressos.insert(
                    c.id.clone(),
                    ProgressoCurso::ler(&arquivo_de_progresso(d, &c.id)),
                );
            }
        }
        self.sincronizar_campo_de_videos();
    }

    fn sincronizar_campo_de_videos(&mut self) {
        let atual = self.curso_selecionado().id.clone();
        self.campo_videos = self
            .dados
            .as_deref()
            .and_then(|d| ler_pasta_de_videos(d, &atual))
            .map(|p| p.display().to_string())
            .unwrap_or_default();
    }

    pub fn indice_selecionado(&self) -> usize {
        self.selecionado
    }

    pub fn curso_selecionado(&self) -> &CursoListado {
        &self.cursos[self.selecionado.min(self.cursos.len() - 1)]
    }

    pub fn selecionar(&mut self, i: usize) {
        if i < self.cursos.len() && i != self.selecionado {
            self.selecionado = i;
            self.sincronizar_campo_de_videos();
            self.aviso_video = None;
        }
    }

    pub fn selecionar_por_id(&mut self, id: &str) {
        if let Some(i) = self.cursos.iter().position(|c| c.id == id) {
            self.selecionar(i);
        }
    }

    pub fn progresso(&self, curso: &str) -> ProgressoCurso {
        self.progressos.get(curso).cloned().unwrap_or_default()
    }

    /// `(concluídos, praticáveis)` do Curso de tutor `curso`: os praticáveis são os aprovados e,
    /// com `usar_nao_revisado`, os não revisados.
    pub fn resumo_de_progresso(&self, curso: &CursoListado) -> (usize, usize) {
        let ids: Vec<&str> = curso
            .arvore
            .iter()
            .flat_map(|s| &s.licoes)
            .flat_map(|l| l.visiveis(self.usar_nao_revisado).map(|e| e.id.as_str()))
            .collect();
        let feitos = self
            .progressos
            .get(&curso.id)
            .map_or(0, |p| p.concluidos_entre(ids.iter().copied()));
        (feitos, curso.praticaveis(self.usar_nao_revisado))
    }

    // --- instalação ---

    pub fn instalando(&self) -> bool {
        self.instalando.is_some()
    }

    /// Instala o `.ptcurso` de `caminho` em segundo plano. O resultado chega por
    /// [`EstadoCursos::atualizar`]; erros viram `mensagem`, em português.
    pub fn iniciar_instalacao(&mut self, caminho: &str) {
        if self.instalando.is_some() {
            self.mensagem = Some(Mensagem::info("Já estou instalando um pacote; aguarde."));
            return;
        }
        let Some(dados) = self.dados.clone() else {
            self.mensagem = Some(Mensagem::erro(
                "Não há pasta de dados do usuário neste computador; não dá para instalar Cursos.",
            ));
            return;
        };
        if caminho.trim().is_empty() {
            self.mensagem = Some(Mensagem::erro(
                "Digite o caminho de um arquivo .ptcurso para instalar.",
            ));
            return;
        }
        let arquivo = caminho_digitado(caminho);
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(instalar_arquivo(&arquivo, &dados));
        });
        self.instalando = Some(rx);
        self.mensagem = Some(Mensagem::info("Instalando o pacote…"));
    }

    fn sondar_instalacao(&mut self) {
        let Some(rx) = &self.instalando else {
            return;
        };
        let r = match rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("A instalação parou sem resultado.".to_owned()),
        };
        self.instalando = None;
        match r {
            Ok(i) => {
                self.recarregar();
                self.selecionar_por_id(&i.id);
                let selo = if i.confiavel {
                    "confiável"
                } else {
                    "chave desconhecida"
                };
                self.mensagem = Some(Mensagem::info(format!(
                    "Curso «{}» instalado. Autor: {} · chave {} ({selo}).",
                    i.titulo, i.autor, i.chave_curta
                )));
                self.campo_pacote.clear();
            }
            Err(e) => self.mensagem = Some(Mensagem::erro(e)),
        }
    }

    // --- Exercício de Curso ---

    /// Abre o Exercício na partitura (com Praticar). Erros viram `mensagem` e a devolução.
    pub fn abrir_exercicio(&mut self, licao_id: &str, exercicio_id: &str) -> Result<(), String> {
        let permitir = self.usar_nao_revisado;
        let r = carregar_exercicio(self.curso_selecionado(), licao_id, exercicio_id, permitir)
            .and_then(|a| {
                self.partitura
                    .abrir_xml(&a.exercicio.titulo, a.xml.clone())?;
                Ok(a)
            });
        match r {
            Ok(a) => {
                self.midia.parar_tudo();
                self.cursor_audio = None;
                self.observador = ObservadorPratica::com_fronteiras(fronteiras_de_frase(
                    a.roteiro.as_ref(),
                    &a.licao_id,
                    &a.exercicio.id,
                ));
                self.partitura.limpar_pratica();
                if let Some(bpm) = a.exercicio.bpm {
                    self.partitura.definir_bpm(bpm as f32);
                }
                self.aviso_video = None;
                self.mensagem = None;
                self.aberto = Some(a);
                Ok(())
            }
            Err(e) => {
                self.mensagem = Some(Mensagem::erro(e.clone()));
                Err(e)
            }
        }
    }

    pub fn fechar_exercicio(&mut self) {
        self.midia.parar_tudo();
        self.partitura.limpar_pratica();
        self.cursor_audio = None;
        self.aberto = None;
    }

    /// "Ouvir o professor": toca `audio/<id>.opus` no mpv; o cursor segue o Alinhamento.
    pub fn ouvir_professor(&mut self) -> Result<(), String> {
        let Some(a) = &self.aberto else {
            return Err("Abra um exercício primeiro.".to_owned());
        };
        let Some(audio) = a.audio.clone() else {
            let m = "Este exercício não tem áudio do professor.".to_owned();
            self.mensagem = Some(Mensagem::erro(m.clone()));
            return Err(m);
        };
        self.partitura.parar_pratica();
        match self.midia.tocar_audio(&audio) {
            Ok(()) => {
                self.mensagem = None;
                Ok(())
            }
            Err(e) => {
                self.mensagem = Some(Mensagem::erro(e.clone()));
                Err(e)
            }
        }
    }

    pub fn parar_professor(&mut self) {
        self.midia.parar_audio();
        self.cursor_audio = None;
    }

    /// Onde o cursor do áudio do professor está agora (enquanto toca).
    pub fn cursor_do_audio(&self) -> Option<Destaque> {
        self.cursor_audio
    }

    // --- vídeo ---

    pub fn pasta_de_videos(&self) -> Option<PathBuf> {
        ler_pasta_de_videos(self.dados.as_deref()?, &self.curso_selecionado().id)
    }

    /// Guarda a pasta dos vídeos do Curso (a escolha é feita uma vez).
    pub fn definir_pasta_de_videos(&mut self, texto: &str) {
        let Some(dados) = self.dados.clone() else {
            self.aviso_video = Some(Mensagem::erro("Não há pasta de dados do usuário."));
            return;
        };
        let pasta = caminho_digitado(texto);
        let id = self.curso_selecionado().id.clone();
        self.aviso_video = Some(match gravar_pasta_de_videos(&dados, &id, &pasta) {
            Ok(()) => Mensagem::info(format!("Pasta dos vídeos: {}", pasta.display())),
            Err(e) => Mensagem::erro(e),
        });
    }

    /// "Ver vídeo": abre o vídeo da Aula da Lição `licao_id` no mpv, no início do trecho do
    /// Exercício `exercicio_id` (ou no começo, para uma Lição de assistir).
    pub fn ver_video(&mut self, licao_id: &str, exercicio_id: Option<&str>) {
        let r = self.preparar_video(licao_id, exercicio_id);
        self.aviso_video = Some(match r {
            Ok(m) => m,
            Err(e) => Mensagem::erro(e),
        });
    }

    /// Onde o editor de sincronia grava o Alinhamento do Exercício `exercicio`: na oficina do
    /// tutor que já tem esse Alinhamento (ou cujo nome começa o id do Curso); sem oficina, nos
    /// dados do aluno.
    pub fn destino_do_alinhamento(&self, exercicio: &str) -> Option<PathBuf> {
        let dados = self.dados.as_deref()?;
        let curso = &self.curso_selecionado().id;
        let oficinas = crate::revisao::descobrir_oficinas(dados, None);
        let nome = format!("{exercicio}.json");
        oficinas
            .iter()
            .map(|o| o.raiz.join("alinhamentos").join(&nome))
            .find(|p| p.is_file())
            .or_else(|| {
                oficinas
                    .iter()
                    .find(|o| curso.starts_with(&o.tutor))
                    .map(|o| o.raiz.join("alinhamentos").join(&nome))
            })
            .or_else(|| {
                Some(
                    dados
                        .join("cursos")
                        .join("alinhamentos-editados")
                        .join(curso)
                        .join(&nome),
                )
            })
    }

    /// Se o editor de sincronia já gravou um Alinhamento deste Exercício (ainda não
    /// empacotado), usa-o no Exercício aberto. Devolve se trocou.
    pub fn usar_alinhamento_editado(&mut self) -> bool {
        let Some(id) = self.aberto.as_ref().map(|a| a.exercicio.id.clone()) else {
            return false;
        };
        let Some(p) = self.destino_do_alinhamento(&id) else {
            return false;
        };
        let Some(al) = fs::read(&p)
            .ok()
            .and_then(|b| Alinhamento::de_json(&b).ok())
        else {
            return false;
        };
        if al.metodo != "editor" || al.exercicio != id {
            return false;
        }
        if let Some(a) = &mut self.aberto {
            a.alinhamento = Some(al);
        }
        true
    }

    /// O arquivo do vídeo da Aula da Lição `licao_id` na pasta que o aluno indicou.
    pub fn resolver_video(&self, licao_id: &str) -> Result<PathBuf, String> {
        let curso = self.curso_selecionado();
        let manifesto = curso
            .manifesto
            .as_ref()
            .ok_or("Este Curso não tem vídeos de Aula.")?;
        let licao = manifesto
            .secoes
            .iter()
            .flat_map(|s| &s.licoes)
            .find(|l| l.id == licao_id)
            .ok_or("Lição não encontrada neste Curso.")?;
        let aula = licao
            .aula
            .as_ref()
            .ok_or("Esta Lição não indica um vídeo de Aula.")?;
        let dados = self
            .dados
            .as_deref()
            .ok_or("Não há pasta de dados do usuário.")?;
        let pasta = ler_pasta_de_videos(dados, &curso.id).ok_or(
            "Indique a pasta onde estão os vídeos deste Curso (campo «Pasta dos vídeos») e tente de novo.",
        )?;
        achar_video(&pasta, &aula.nome_arquivo).ok_or_else(|| {
            format!(
                "Não achei o vídeo «{}» na pasta {}.",
                aula.nome_arquivo,
                pasta.display()
            )
        })
    }

    /// Os Exercícios da Lição que têm trecho na Aula, como trechos da linha do tempo.
    pub fn trechos_da_licao(&self, licao_id: &str) -> Vec<crate::player_aula::Trecho> {
        let permitir = self.usar_nao_revisado;
        let mut v: Vec<_> = self
            .curso_selecionado()
            .manifesto
            .iter()
            .flat_map(|m| &m.secoes)
            .flat_map(|s| &s.licoes)
            .filter(|l| l.id == licao_id)
            .flat_map(|l| &l.exercicios)
            .filter(|e| e.utilizavel(permitir))
            .filter_map(|e| {
                e.trecho_aula.map(|t| crate::player_aula::Trecho {
                    exercicio_id: e.id.clone(),
                    titulo: e.titulo.clone(),
                    inicio_s: t.inicio_s,
                    fim_s: t.fim_s,
                })
            })
            .collect();
        v.sort_by(|a, b| a.inicio_s.total_cmp(&b.inicio_s));
        v
    }

    /// Dedilhado do Exercício aberto: o do pacote (`dedilhados/<id>.json`) ou a Hipótese que a
    /// fábrica deixou nos dados do aluno (`cursos/dedilhados/<curso>/<id>.json`).
    pub fn dedilhado_do_aberto(&self) -> Option<crate::player_aula::Dedilhado> {
        let a = self.aberto.as_ref()?;
        let curso = self.curso_selecionado();
        let nome = format!("{}.json", a.exercicio.id);
        if !caminho_seguro(&nome) {
            return None;
        }
        let mut candidatos = Vec::new();
        if let (Some(m), Some(pasta)) = (curso.manifesto.as_ref(), curso.pasta.as_ref()) {
            let c = format!("dedilhados/{nome}");
            if m.arquivos.iter().any(|x| x.caminho == c) {
                candidatos.push(pasta.join(c));
            }
        }
        if let Some(d) = self.dados.as_deref() {
            candidatos.push(
                d.join("cursos")
                    .join("dedilhados")
                    .join(&curso.id)
                    .join(&nome),
            );
        }
        candidatos
            .into_iter()
            .find_map(|p| crate::player_aula::Dedilhado::de_json(&fs::read(p).ok()?).ok())
    }

    fn preparar_video(
        &mut self,
        licao_id: &str,
        exercicio_id: Option<&str>,
    ) -> Result<Mensagem, String> {
        let curso = self.curso_selecionado();
        let manifesto = curso
            .manifesto
            .as_ref()
            .ok_or("Este Curso não tem vídeos de Aula.")?;
        let licao = manifesto
            .secoes
            .iter()
            .flat_map(|s| &s.licoes)
            .find(|l| l.id == licao_id)
            .ok_or("Lição não encontrada neste Curso.")?;
        let aula = licao
            .aula
            .clone()
            .ok_or("Esta Lição não indica um vídeo de Aula.")?;
        let inicio_s = exercicio_id
            .and_then(|id| licao.exercicios.iter().find(|e| e.id == id))
            .and_then(|e| e.trecho_aula)
            .map_or(0.0, |t| t.inicio_s);
        let curso_id = curso.id.clone();
        let dados = self
            .dados
            .clone()
            .ok_or("Não há pasta de dados do usuário.")?;
        let pasta = ler_pasta_de_videos(&dados, &curso_id).ok_or(
            "Indique a pasta onde estão os vídeos deste Curso (campo «Pasta dos vídeos») e tente de novo.",
        )?;
        let arquivo = achar_video(&pasta, &aula.nome_arquivo).ok_or_else(|| {
            format!(
                "Não achei o vídeo «{}» na pasta {}.",
                aula.nome_arquivo,
                pasta.display()
            )
        })?;
        self.midia.abrir_video(&arquivo, inicio_s)?;
        self.verificacao = aula
            .sha256
            .clone()
            .map(|h| VerificacaoSha::iniciar(arquivo.clone(), h));
        Ok(Mensagem::info(format!(
            "Abrindo {} em {}.",
            aula.nome_arquivo,
            formatar_instante(inicio_s)
        )))
    }

    // --- progresso ---

    /// Registra a precisão de uma tentativa no Exercício aberto e grava o progresso do Curso.
    /// Devolve `true` se o Exercício acabou de ficar concluído.
    pub fn registrar_resultado(&mut self, pct: f32) -> bool {
        let Some(a) = &self.aberto else {
            return false;
        };
        let (curso, exercicio) = (a.curso_id.clone(), a.exercicio.id.clone());
        let p = self.progressos.entry(curso.clone()).or_default();
        let antes = p.concluido(&exercicio);
        if p.registrar(&exercicio, pct)
            && let Some(d) = self.dados.as_deref()
        {
            let _ = p.gravar(&arquivo_de_progresso(d, &curso));
        }
        !antes && p.concluido(&exercicio)
    }

    // --- quadro ---

    /// Um passo por quadro: lê a instalação, a posição do áudio, a conferência do vídeo e a
    /// prática (grava o progresso quando ela termina). Devolve o que a prática fez.
    pub fn atualizar(&mut self) -> Vec<EventoPratica> {
        self.sondar_instalacao();
        self.midia.atualizar();
        if let Some(v) = &self.verificacao
            && let Some(r) = v.consultar()
        {
            self.verificacao = None;
            match r {
                ResultadoSha::Confere => {}
                ResultadoSha::Diferente => {
                    self.aviso_video = Some(Mensagem::erro(
                        "Atenção: este arquivo não é igual ao vídeo usado pelo Curso (o SHA-256 \
                         difere). O trecho pode não bater com o que o professor tocou.",
                    ));
                }
                ResultadoSha::Erro(e) => {
                    self.aviso_video = Some(Mensagem::erro(format!(
                        "Não consegui conferir o vídeo: {e}"
                    )));
                }
            }
        }
        self.cursor_audio = match (&self.aberto, self.midia.posicao_audio_s()) {
            (Some(a), Some(t)) => a
                .alinhamento
                .as_ref()
                .and_then(|al| indice_da_nota_no_instante(al, t))
                .and_then(|i| self.partitura.destaque_da_nota(i as usize)),
            _ => None,
        };
        if self.aberto.is_none() {
            return Vec::new();
        }
        let eventos = self.observador.observar(&self.partitura);
        for e in &eventos {
            if let EventoPratica::Concluida { pct, .. } = e {
                self.registrar_resultado(*pct);
            }
        }
        eventos
    }

    /// Ao sair da tela: nenhum mpv fica vivo e a partitura para.
    pub fn ao_sair_da_tela(&mut self) {
        self.midia.parar_tudo();
        // O vídeo embutido não continua tocando fora da tela.
        self.player = None;
        self.notas_do_aluno.clear();
        self.cursor_audio = None;
        self.verificacao = None;
    }

    /// A tela precisa de ~60 quadros por segundo (áudio com cursor ou instalação em curso).
    pub fn precisa_de_quadros(&self) -> bool {
        self.midia.audio_tocando()
            || self.player.is_some()
            || self.instalando.is_some()
            || self.verificacao.is_some()
            || self.partitura.praticando()
            || self.partitura.tocando()
    }
}

/// `1:35` ou `0:07`, para mostrar o instante do trecho.
pub fn formatar_instante(s: f64) -> String {
    let t = if s.is_finite() { s.max(0.0) } else { 0.0 };
    let total = t as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

#[cfg(test)]
pub(crate) mod apoio_testes {
    //! Pacote sintético (nada de curso real) para os testes das telas de Curso.
    use super::*;
    use piano_tutor_curso::{ChavePublica, NotaAlinhada, empacotar, gerar_par};

    pub const ID_CURSO: &str = "teste.sintetico";

    /// MusicXML de `compassos` compassos de 4/4 com semínimas subindo e descendo.
    pub fn musicxml(compassos: u32) -> String {
        let passos = ["C", "D", "E", "F", "G", "A", "B", "C"];
        let mut x = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<score-partwise version=\"4.0\">\n  <part-list><score-part id=\"P1\"><part-name>Piano</part-name></score-part></part-list>\n  <part id=\"P1\">\n",
        );
        for c in 0..compassos {
            let _ = writeln!(x, "    <measure number=\"{}\">", c + 1);
            if c == 0 {
                x.push_str("      <attributes><divisions>1</divisions><key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes>\n");
            }
            for n in 0..4 {
                let i = ((c * 4 + n) % 8) as usize;
                let oitava = if i == 7 { 5 } else { 4 };
                let _ = writeln!(
                    x,
                    "      <note><pitch><step>{}</step><octave>{oitava}</octave></pitch><duration>1</duration><type>quarter</type></note>",
                    passos[i]
                );
            }
            x.push_str("    </measure>\n");
        }
        x.push_str("  </part>\n</score-partwise>\n");
        x
    }

    fn manifesto(sha_video: Option<&str>) -> String {
        let sha = sha_video.map_or(String::new(), |h| format!("sha256 = \"{h}\"\n"));
        format!(
            r#"formato_versao = 1
id = "{ID_CURSO}"
titulo = "Curso Sintético de Teste"
versao = "0.1.0"
gerado_em = "2026-01-01T00:00:00Z"
idioma = "pt-BR"
convencao_oitava = "do3-central"

[autor]
nome = "Autor Sintético"

[licenciado]
nome = "Aluno de Teste"
licenca = "Somente para testes automatizados."

[[secoes]]
id = "s03"
numero = 3
titulo = "Mão direita"

[[secoes.licoes]]
id = "s03-a12"
numero = 12
titulo = "Para assistir"
tipo = "assistir"

[secoes.licoes.aula]
nome_arquivo = "12 - Para assistir.mp4"

[[secoes.licoes]]
id = "s03-a14"
numero = 14
titulo = "Cinco notas"
tipo = "exercicio"

[secoes.licoes.aula]
nome_arquivo = "14 - Cinco notas.mp4"
duracao_s = 312.4
{sha}
[[secoes.licoes.exercicios]]
id = "cinco-notas"
titulo = "Cinco notas no piano"
modo = "praticar"
mao = "direita"
estado = "aprovada"
partitura = "partituras/cinco-notas.musicxml"
audio = "audio/cinco-notas.opus"
alinhamento = "alinhamentos/cinco-notas.json"
bpm = 90

[secoes.licoes.exercicios.trecho_aula]
inicio_s = 95.0
fim_s = 140.5

[[secoes.licoes.exercicios]]
id = "ida-e-volta"
titulo = "Ida e volta (chamada e resposta)"
modo = "chamada_e_resposta"
mao = "direita"
estado = "aprovada"
partitura = "partituras/ida-e-volta.musicxml"

[[secoes.licoes.exercicios]]
id = "ainda-hipotese"
titulo = "Ainda hipótese"
modo = "praticar"
mao = "direita"
estado = "hipotese"
partitura = "partituras/ainda-hipotese.musicxml"

[[secoes.licoes.exercicios]]
id = "em-revisao"
titulo = "Em revisão"
modo = "praticar"
mao = "direita"
estado = "em_revisao"
partitura = "partituras/ainda-hipotese.musicxml"

[[secoes.licoes.exercicios]]
id = "rejeitado"
titulo = "Rejeitado"
modo = "praticar"
mao = "direita"
estado = "rejeitada"
partitura = "partituras/ainda-hipotese.musicxml"
"#
        )
    }

    const ROTEIRO: &str = r#"formato_versao = 1

[[falas]]
id = "a14-antes"
licao = "s03-a14"
momento = "antes"
texto = "Vamos tocar as cinco notas."
estado = "aprovada"

[[falas]]
id = "a14-passo4"
licao = "s03-a14"
exercicio = "cinco-notas"
momento = "passo"
passo = 4
texto = "Agora o Sol."
estado = "aprovada"

[[falas]]
id = "a14-acerto-1"
licao = "s03-a14"
momento = "acerto"
texto = "Muito bem."
estado = "aprovada"

[[falas]]
id = "a14-acerto-2"
licao = "s03-a14"
momento = "acerto"
texto = "Isso mesmo."
estado = "aprovada"

[[falas]]
id = "a14-erro"
licao = "s03-a14"
momento = "erro"
texto = "Calma, confira a nota."
estado = "aprovada"

[[falas]]
id = "a14-depois"
licao = "s03-a14"
momento = "depois"
texto = "Terminamos este exercício."
estado = "aprovada"

[[falas]]
id = "a14-hipotese"
licao = "s03-a14"
momento = "antes"
texto = "Esta ainda é hipótese."
estado = "hipotese"
"#;

    /// Alinhamento de 16 notas, uma a cada 0,5 s a partir de 0,5 s.
    pub fn alinhamento(n: u32) -> Alinhamento {
        Alinhamento {
            formato_versao: 1,
            exercicio: "cinco-notas".to_owned(),
            audio: Some("audio/cinco-notas.opus".to_owned()),
            estado: piano_tutor_curso::EstadoConteudo::Aprovada,
            metodo: "teste".to_owned(),
            offset_aula_s: 95.0,
            notas: (0..n)
                .map(|i| NotaAlinhada {
                    indice: i,
                    compasso: i / 4 + 1,
                    midi: 60,
                    t_partitura_s: f64::from(i) * 0.5,
                    t_audio_s: 0.5 + f64::from(i) * 0.5,
                    confianca: 0.9,
                })
                .collect(),
        }
    }

    /// Escreve a pasta de origem do pacote sintético.
    pub fn escrever_origem(pasta: &Path, sha_video: Option<&str>) {
        for sub in ["partituras", "audio", "alinhamentos"] {
            fs::create_dir_all(pasta.join(sub)).expect("subpasta");
        }
        fs::write(pasta.join("manifesto.toml"), manifesto(sha_video)).expect("manifesto");
        fs::write(pasta.join("roteiro.toml"), ROTEIRO).expect("roteiro");
        fs::write(pasta.join("partituras/cinco-notas.musicxml"), musicxml(4)).expect("partitura");
        fs::write(pasta.join("partituras/ida-e-volta.musicxml"), musicxml(4)).expect("partitura");
        fs::write(
            pasta.join("partituras/ainda-hipotese.musicxml"),
            musicxml(1),
        )
        .expect("partitura");
        fs::write(pasta.join("audio/cinco-notas.opus"), b"OggS sintetico").expect("audio");
        fs::write(
            pasta.join("alinhamentos/cinco-notas.json"),
            alinhamento(16).para_json().expect("json"),
        )
        .expect("alinhamento");
    }

    /// Empacota o Curso sintético em `base/curso.ptcurso`. Devolve o arquivo e a chave pública.
    pub fn criar_pacote(base: &Path, sha_video: Option<&str>) -> (PathBuf, ChavePublica) {
        let origem = base.join("origem");
        escrever_origem(&origem, sha_video);
        let (privada, publica) = gerar_par();
        let bytes = empacotar(&origem, &privada).expect("empacota");
        let arquivo = base.join("curso.ptcurso");
        fs::write(&arquivo, bytes).expect("grava o pacote");
        (arquivo, publica)
    }

    /// Instala o Curso sintético direto em `<dados>/cursos/` (sem passar pela tela).
    pub fn instalar_sintetico(dados: &Path, sha_video: Option<&str>) -> ChavePublica {
        let tmp = tempfile::tempdir().expect("pasta");
        let (arquivo, chave) = criar_pacote(tmp.path(), sha_video);
        let pacote = ler_pacote_de_arquivo(&arquivo).expect("lê");
        instalar(&pacote, dados).expect("instala");
        chave
    }

    /// Espera a instalação em segundo plano terminar (com limite de tempo).
    pub fn esperar_instalacao(e: &mut EstadoCursos) {
        let fim = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while e.instalando() {
            assert!(std::time::Instant::now() < fim, "a instalação não terminou");
            std::thread::sleep(std::time::Duration::from_millis(5));
            e.atualizar();
        }
    }
}

#[cfg(test)]
mod testes {
    use super::apoio_testes::*;
    use super::*;
    use crate::mpv::apoio_testes::midia_falsa;

    fn estado(dados: &Path) -> (EstadoCursos, crate::mpv::apoio_testes::ExecutorFalso) {
        let (midia, ex) = midia_falsa();
        (EstadoCursos::novo(Some(dados.to_owned()), midia), ex)
    }

    #[test]
    fn lista_a_fabrica_e_os_cursos_instalados_com_selo_e_chave() {
        let dados = tempfile::tempdir().expect("dados");
        let (e, _) = estado(dados.path());
        assert_eq!(e.cursos.len(), 1);
        let f = &e.cursos[0];
        assert!(f.e_fabrica());
        assert_eq!(
            (f.titulo.as_str(), f.autor.as_str()),
            ("Curso de fábrica", "Piano Tutor")
        );
        assert_eq!(f.aprovados, 10);
        assert_eq!(f.selo, Selo::Fabrica);
        assert_eq!(f.chave_curta, None);

        let chave = instalar_sintetico(dados.path(), None);
        let (e, _) = estado(dados.path());
        assert_eq!(e.cursos.len(), 2);
        let c = &e.cursos[1];
        assert_eq!(c.id, ID_CURSO);
        assert_eq!(c.titulo, "Curso Sintético de Teste");
        assert_eq!(c.autor, "Autor Sintético");
        assert_eq!(c.licenciado.as_deref(), Some("Aluno de Teste"));
        assert_eq!(c.chave_curta.as_deref(), Some(chave.curta().as_str()));
        assert_eq!(
            c.selo,
            Selo::ChaveDesconhecida,
            "ninguém confiou nesta chave"
        );
        assert_eq!((c.aprovados, c.aguardando), (2, 2));

        // Pôr a chave em chaves-confiaveis/ muda o selo (e só isso).
        let pasta = dados.path().join("cursos").join("chaves-confiaveis");
        fs::create_dir_all(&pasta).expect("pasta");
        chave
            .gravar(&pasta.join("autor.pub"))
            .expect("grava a chave");
        let (e, _) = estado(dados.path());
        assert_eq!(e.cursos[1].selo, Selo::Confiavel);
        assert_eq!(Selo::Confiavel.rotulo(), "confiável");
        assert_eq!(Selo::ChaveDesconhecida.rotulo(), "chave desconhecida");
        assert!(
            Selo::ChaveDesconhecida.simbolo().starts_with('['),
            "nunca só cor"
        );
    }

    #[test]
    fn manifesto_alterado_depois_de_instalar_vira_assinatura_invalida_e_nao_abre() {
        let dados = tempfile::tempdir().expect("dados");
        instalar_sintetico(dados.path(), None);
        let man = dados
            .path()
            .join("cursos")
            .join(ID_CURSO)
            .join("manifesto.toml");
        let t = fs::read_to_string(&man).expect("lê");
        fs::write(
            &man,
            t.replace("Curso Sintético de Teste", "Curso Adulterado"),
        )
        .expect("grava");
        let (mut e, _) = estado(dados.path());
        assert_eq!(e.cursos[1].selo, Selo::AssinaturaInvalida);
        e.selecionar(1);
        let r = e.abrir_exercicio("s03-a14", "cinco-notas");
        assert!(r.expect_err("recusa").contains("alterado"));
        assert!(e.aberto.is_none());
    }

    #[test]
    fn arvore_traz_aprovados_e_nao_revisados_e_so_mostra_os_ultimos_com_a_preferencia() {
        let dados = tempfile::tempdir().expect("dados");
        instalar_sintetico(dados.path(), None);
        let (e, _) = estado(dados.path());
        let c = &e.cursos[1];
        assert_eq!(c.arvore.len(), 1);
        let s = &c.arvore[0];
        assert_eq!((s.numero, s.titulo.as_str()), (3, "Mão direita"));
        assert_eq!(s.licoes.len(), 2);
        let assistir = &s.licoes[0];
        assert_eq!(assistir.tipo, TipoLicao::Assistir);
        assert!(assistir.exercicios.is_empty());
        assert_eq!(
            assistir.aula.as_ref().map(|a| a.nome_arquivo.as_str()),
            Some("12 - Para assistir.mp4")
        );
        let ex = &s.licoes[1];
        let ids =
            |permitir| -> Vec<&str> { ex.visiveis(permitir).map(|e| e.id.as_str()).collect() };
        assert_eq!(
            ids(false),
            ["cinco-notas", "ida-e-volta"],
            "só os aprovados"
        );
        assert_eq!(
            ids(true),
            ["cinco-notas", "ida-e-volta", "ainda-hipotese", "em-revisao"],
            "com a preferência, também os não revisados; o rejeitado nunca"
        );
        // hipótese e em revisão aguardam; rejeitado nem aparece na conta.
        assert_eq!(ex.aguardando, 2);
        assert_eq!(ex.exercicios.len(), 4);
        assert_eq!((c.aprovados, c.aguardando), (2, 2));
        assert_eq!((c.praticaveis(false), c.praticaveis(true)), (2, 4));
    }

    #[test]
    fn exercicio_nao_aprovado_nao_abre_nem_por_engano() {
        let dados = tempfile::tempdir().expect("dados");
        instalar_sintetico(dados.path(), None);
        let (mut e, _) = estado(dados.path());
        e.selecionar(1);
        e.usar_nao_revisado = false;
        for id in ["ainda-hipotese", "em-revisao"] {
            let erro = e.abrir_exercicio("s03-a14", id).expect_err(id);
            assert!(erro.contains("aguarda"), "{id}: {erro}");
            assert!(e.aberto.is_none());
        }
        // Rejeitado nunca abre, com a preferência ligada ou desligada.
        for permitir in [false, true] {
            e.usar_nao_revisado = permitir;
            let erro = e
                .abrir_exercicio("s03-a14", "rejeitado")
                .expect_err("rejeitado");
            assert!(erro.contains("rejeitado"), "{erro}");
            assert!(e.aberto.is_none());
        }
        assert!(e.abrir_exercicio("s03-a14", "nao-existe").is_err());
        assert!(e.abrir_exercicio("nao-existe", "cinco-notas").is_err());
        // O de fábrica não tem pacote.
        e.selecionar(0);
        assert!(e.abrir_exercicio("x", "y").is_err());
    }

    #[test]
    fn com_a_preferencia_ligada_hipotese_e_em_revisao_abrem_com_o_estado_a_mostra() {
        let dados = tempfile::tempdir().expect("dados");
        instalar_sintetico(dados.path(), None);
        let (mut e, _) = estado(dados.path());
        e.selecionar(1);
        assert!(e.usar_nao_revisado, "padrão: ligada");
        for (id, estado) in [
            (
                "ainda-hipotese",
                piano_tutor_curso::EstadoConteudo::Hipotese,
            ),
            ("em-revisao", piano_tutor_curso::EstadoConteudo::EmRevisao),
        ] {
            e.abrir_exercicio("s03-a14", id).expect(id);
            let a = e.aberto.as_ref().expect("aberto");
            assert_eq!(a.exercicio.estado, estado);
            assert!(a.exercicio.nao_revisado(), "a tela põe o selo");
            assert!(!a.exercicio.serve_de_gabarito(), "Gabarito segue estrito");
        }
        e.abrir_exercicio("s03-a14", "cinco-notas")
            .expect("aprovado");
        assert!(!e.aberto.as_ref().expect("aberto").exercicio.nao_revisado());
        // Progresso conta os não revisados como praticáveis só com a preferência.
        assert_eq!(e.resumo_de_progresso(&e.cursos[1]), (0, 4));
        e.usar_nao_revisado = false;
        assert_eq!(e.resumo_de_progresso(&e.cursos[1]), (0, 2));
    }

    #[test]
    fn abrir_carrega_partitura_alinhamento_audio_e_roteiro_conferidos() {
        let dados = tempfile::tempdir().expect("dados");
        instalar_sintetico(dados.path(), None);
        let (mut e, _) = estado(dados.path());
        e.selecionar(1);
        e.abrir_exercicio("s03-a14", "cinco-notas").expect("abre");
        let a = e.aberto.as_ref().expect("aberto");
        assert_eq!(a.curso_id, ID_CURSO);
        assert_eq!(a.exercicio.id, "cinco-notas");
        assert_eq!(a.alinhamento.as_ref().map(|x| x.notas.len()), Some(16));
        assert!(
            a.audio
                .as_ref()
                .is_some_and(|p| p.ends_with("audio/cinco-notas.opus"))
        );
        assert_eq!(a.roteiro.as_ref().map(|r| r.falas.len()), Some(7));
        assert_eq!(e.partitura.total_de_notas(), 16);
        assert!((e.partitura.bpm - 90.0).abs() < 1e-3, "bpm do manifesto");

        // Arquivo trocado depois da instalação: não abre.
        let alterado = dados
            .path()
            .join("cursos")
            .join(ID_CURSO)
            .join("partituras/cinco-notas.musicxml");
        fs::write(&alterado, musicxml(2)).expect("grava");
        let erro = e
            .abrir_exercicio("s03-a14", "cinco-notas")
            .expect_err("alterado");
        assert!(erro.contains("alterado depois da instalação"), "{erro}");
    }

    #[test]
    fn progresso_ida_e_volta_separado_por_curso_e_so_o_melhor_vale() {
        let dados = tempfile::tempdir().expect("dados");
        instalar_sintetico(dados.path(), None);
        let (mut e, _) = estado(dados.path());
        e.selecionar(1);
        e.abrir_exercicio("s03-a14", "cinco-notas").expect("abre");
        assert!(
            !e.registrar_resultado(60.0),
            "abaixo do limiar: não conclui"
        );
        assert!(e.registrar_resultado(92.5), "agora concluiu");
        assert!(!e.registrar_resultado(70.0), "pior não desfaz");
        let p = e.progresso(ID_CURSO);
        assert_eq!(
            p.item("cinco-notas"),
            Some(ItemDeProgresso {
                concluido: true,
                melhor_pct: 92.5
            })
        );
        // Gravado em <dados>/cursos/progresso/<id>.txt e relido por outro estado.
        let arq = arquivo_de_progresso(dados.path(), ID_CURSO);
        assert!(arq.is_file());
        let (e2, _) = estado(dados.path());
        assert_eq!(e2.progresso(ID_CURSO), p);
        let (feitos, total) = e2.resumo_de_progresso(&e2.cursos[1]);
        assert_eq!((feitos, total), (1, 4), "padrão: com os não revisados");
        let mut e2 = e2;
        e2.usar_nao_revisado = false;
        assert_eq!(e2.resumo_de_progresso(&e2.cursos[1]), (1, 2));
        // O Curso de fábrica não mistura com o de tutor.
        assert_eq!(e2.progresso(ID_FABRICA), ProgressoCurso::default());
    }

    #[test]
    fn progresso_texto_ida_e_volta_e_lixo_ignorado() {
        let mut p = ProgressoCurso::default();
        assert!(p.registrar("a", 100.0));
        assert!(p.registrar("b", 40.0));
        assert!(!p.registrar("b", 30.0), "nada mudou");
        assert!(p.registrar("b", 50.5));
        assert!(!p.registrar("Inválido!", 90.0));
        assert!(!p.registrar("c", f32::NAN));
        let t = p.para_texto();
        assert_eq!(ProgressoCurso::de_texto(&t), p);
        let sujo = ProgressoCurso::de_texto(
            "futuro\nx=2,50\ny=1,150\nz=1,abc\nBAD ID=1,50\nok=1,80.0\nok=0,10.0\n",
        );
        // A última linha válida de um id vence; o resto é ignorado.
        assert_eq!(sujo.itens.len(), 1);
        assert_eq!(sujo.item("ok").map(|i| i.concluido), Some(false));
        assert!(!p.concluido("b"));
        assert_eq!(p.concluidos_entre(["a", "b", "nao"]), 1);
    }

    #[test]
    fn mapeia_o_instante_do_audio_para_o_indice_da_nota_pelo_alinhamento() {
        let a = alinhamento(16); // nota i em 0,5 + 0,5·i
        assert_eq!(
            indice_da_nota_no_instante(&a, 0.0),
            None,
            "antes da primeira"
        );
        assert_eq!(indice_da_nota_no_instante(&a, 0.49), None);
        assert_eq!(indice_da_nota_no_instante(&a, 0.5), Some(0));
        assert_eq!(indice_da_nota_no_instante(&a, 0.99), Some(0));
        assert_eq!(indice_da_nota_no_instante(&a, 1.0), Some(1));
        assert_eq!(indice_da_nota_no_instante(&a, 4.2), Some(7));
        assert_eq!(
            indice_da_nota_no_instante(&a, 600.0),
            Some(15),
            "depois do fim"
        );
        assert_eq!(indice_da_nota_no_instante(&a, f64::NAN), None);
        // Notas fora de ordem de tempo: vale a que começou por último.
        let mut b = alinhamento(3);
        b.notas[1].t_audio_s = 5.0;
        assert_eq!(indice_da_nota_no_instante(&b, 3.0), Some(2));
        assert_eq!(indice_da_nota_no_instante(&b, 6.0), Some(1));
    }

    #[test]
    fn cursor_do_audio_segue_a_posicao_do_mpv_pelo_alinhamento() {
        let dados = tempfile::tempdir().expect("dados");
        instalar_sintetico(dados.path(), None);
        let (mut e, ex) = estado(dados.path());
        e.selecionar(1);
        e.abrir_exercicio("s03-a14", "cinco-notas").expect("abre");
        e.ouvir_professor().expect("toca");
        let cmd = ex.comandos();
        assert_eq!(cmd.len(), 1);
        let args: Vec<String> = cmd[0]
            .argumentos
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[0], "--no-video");
        assert!(args[3].ends_with("audio/cinco-notas.opus"), "{args:?}");
        // Sem posição do IPC ainda: sem cursor.
        e.atualizar();
        assert_eq!(e.cursor_do_audio(), None);
        // Injeta a posição como o IPC faria (a leitura do socket é testada em mpv.rs).
        let esperado = e.partitura.destaque_da_nota(5).expect("destaque");
        assert_eq!(esperado.compasso, 1);
        assert!((esperado.tempo - 1.0).abs() < 1e-9);
        e.midia.forcar_posicao_de_teste(3.2); // nota 5 começa em 3,0 s
        e.atualizar();
        assert_eq!(e.cursor_do_audio(), Some(esperado));
        e.parar_professor();
        e.atualizar();
        assert_eq!(e.cursor_do_audio(), None);
    }

    #[test]
    fn video_abre_no_instante_do_trecho_e_pede_a_pasta_uma_vez() {
        let dados = tempfile::tempdir().expect("dados");
        instalar_sintetico(dados.path(), None);
        let (mut e, ex) = estado(dados.path());
        e.selecionar(1);
        e.abrir_exercicio("s03-a14", "cinco-notas").expect("abre");
        // Sem a pasta: orienta e não abre nada.
        e.ver_video("s03-a14", Some("cinco-notas"));
        let m = e.aviso_video.clone().expect("aviso");
        assert!(m.erro && m.texto.contains("Indique a pasta"), "{m:?}");
        assert!(ex.comandos().is_empty());

        // Pasta existente, arquivo ausente.
        let videos = tempfile::tempdir().expect("videos");
        e.definir_pasta_de_videos(&videos.path().display().to_string());
        assert!(!e.aviso_video.as_ref().expect("aviso").erro);
        assert_eq!(e.pasta_de_videos(), Some(videos.path().to_owned()));
        e.ver_video("s03-a14", Some("cinco-notas"));
        let m = e.aviso_video.clone().expect("aviso");
        assert!(m.erro && m.texto.contains("14 - Cinco notas.mp4"), "{m:?}");

        // Arquivo numa subpasta da Seção, com a caixa diferente: acha e abre em 95 s.
        let sub = videos.path().join("Seção 3");
        fs::create_dir_all(&sub).expect("subpasta");
        fs::write(sub.join("14 - CINCO NOTAS.MP4"), b"video").expect("grava");
        e.ver_video("s03-a14", Some("cinco-notas"));
        let cmds = ex.comandos();
        assert_eq!(cmds.len(), 1);
        let args: Vec<String> = cmds[0]
            .argumentos
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[0], "--start=95.000");
        assert!(args[1].ends_with("14 - CINCO NOTAS.MP4"));
        assert!(!e.aviso_video.as_ref().expect("aviso").erro);
        assert!(e.midia.video_aberto());

        // Lição de assistir: começa do zero.
        fs::write(videos.path().join("12 - Para assistir.mp4"), b"video").expect("grava");
        e.ver_video("s03-a12", None);
        let cmds = ex.comandos();
        assert!(
            cmds[1].argumentos[0]
                .to_string_lossy()
                .contains("--start=0.000")
        );
        // A pasta fica guardada para a próxima abertura.
        let (e2, _) = estado(dados.path());
        assert_eq!(e2.pasta_de_videos(), None, "o selecionado é a fábrica");
        assert_eq!(
            ler_pasta_de_videos(dados.path(), ID_CURSO),
            Some(videos.path().to_owned())
        );
    }

    #[test]
    fn conferencia_do_sha256_do_video_avisa_em_segundo_plano_quando_difere() {
        let dados = tempfile::tempdir().expect("dados");
        let videos = tempfile::tempdir().expect("videos");
        let conteudo = b"conteudo do video";
        fs::write(videos.path().join("14 - Cinco notas.mp4"), conteudo).expect("grava");
        let certo = sha256_hex(conteudo);

        for (declarado, esperado_aviso) in [(certo.clone(), false), ("0".repeat(64), true)] {
            let dados_i = tempfile::tempdir().expect("dados");
            let _ = &dados;
            instalar_sintetico(dados_i.path(), Some(&declarado));
            let (mut e, _) = estado(dados_i.path());
            e.selecionar(1);
            e.definir_pasta_de_videos(&videos.path().display().to_string());
            e.ver_video("s03-a14", Some("cinco-notas"));
            assert!(e.midia.video_aberto(), "abre sem esperar a conferência");
            let fim = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while e.verificacao.is_some() {
                assert!(std::time::Instant::now() < fim);
                e.atualizar();
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            let aviso = e.aviso_video.clone().expect("aviso");
            if esperado_aviso {
                assert!(
                    aviso.erro && aviso.texto.contains("não é igual"),
                    "{aviso:?}"
                );
            } else {
                assert!(!aviso.erro, "{aviso:?}");
            }
        }
    }

    #[test]
    fn instala_pacote_em_segundo_plano_e_mostra_autor_chave_e_selo() {
        let dados = tempfile::tempdir().expect("dados");
        let tmp = tempfile::tempdir().expect("tmp");
        let (arquivo, chave) = criar_pacote(tmp.path(), None);
        let (mut e, _) = estado(dados.path());
        e.iniciar_instalacao(&format!("  \"{}\"  ", arquivo.display()));
        assert!(
            e.instalando(),
            "volta na hora; o trabalho é em segundo plano"
        );
        esperar_instalacao(&mut e);
        assert_eq!(e.cursos.len(), 2);
        assert_eq!(
            e.curso_selecionado().id,
            ID_CURSO,
            "seleciona o recém-instalado"
        );
        let m = e.mensagem.clone().expect("mensagem");
        assert!(!m.erro);
        assert!(m.texto.contains("Autor Sintético"), "{m:?}");
        assert!(m.texto.contains(&chave.curta()), "{m:?}");
        assert!(m.texto.contains("chave desconhecida"), "{m:?}");
        assert!(
            dados
                .path()
                .join("cursos")
                .join(ID_CURSO)
                .join("manifesto.toml")
                .is_file()
        );
    }

    #[test]
    fn erros_de_instalacao_saem_em_portugues_e_nao_instalam_nada() {
        let dados = tempfile::tempdir().expect("dados");
        let (mut e, _) = estado(dados.path());
        e.iniciar_instalacao("   ");
        assert!(
            e.mensagem
                .as_ref()
                .expect("msg")
                .texto
                .contains("Digite o caminho")
        );
        assert!(!e.instalando());

        e.iniciar_instalacao("/caminho/que/nao/existe.ptcurso");
        esperar_instalacao(&mut e);
        let m = e.mensagem.clone().expect("msg");
        assert!(
            m.erro && m.texto.starts_with("Não instalei o pacote:"),
            "{m:?}"
        );

        // Arquivo cortado no meio: não é um zip, e a mensagem diz isso.
        let tmp = tempfile::tempdir().expect("tmp");
        let (arquivo, _) = criar_pacote(tmp.path(), None);
        let mut bytes = fs::read(&arquivo).expect("lê");
        bytes.truncate(bytes.len() / 2);
        fs::write(&arquivo, bytes).expect("grava");
        e.iniciar_instalacao(&arquivo.display().to_string());
        esperar_instalacao(&mut e);
        let m = e.mensagem.clone().expect("msg");
        assert!(m.erro && m.texto.contains("zip"), "{m:?}");
        assert_eq!(e.cursos.len(), 1, "nada instalado");

        // Sem pasta de dados.
        let (midia, _) = midia_falsa();
        let mut sem = EstadoCursos::novo(None, midia);
        sem.iniciar_instalacao("/x.ptcurso");
        assert!(sem.mensagem.as_ref().expect("msg").erro);
    }

    #[test]
    fn sair_da_tela_encerra_o_mpv_e_o_cursor() {
        let dados = tempfile::tempdir().expect("dados");
        instalar_sintetico(dados.path(), None);
        let (mut e, ex) = estado(dados.path());
        e.selecionar(1);
        e.abrir_exercicio("s03-a14", "cinco-notas").expect("abre");
        e.ouvir_professor().expect("toca");
        assert!(e.precisa_de_quadros());
        e.ao_sair_da_tela();
        assert_eq!(ex.vivos(), [false]);
        assert!(!e.midia.audio_tocando());
    }

    #[test]
    fn sem_mpv_ouvir_o_professor_explica_como_instalar() {
        let dados = tempfile::tempdir().expect("dados");
        instalar_sintetico(dados.path(), None);
        let mut e = EstadoCursos::novo(
            Some(dados.path().to_owned()),
            Midia::com(
                Box::new(crate::mpv::apoio_testes::ExecutorFalso::default()),
                None,
            ),
        );
        e.selecionar(1);
        e.abrir_exercicio("s03-a14", "cinco-notas").expect("abre");
        let erro = e.ouvir_professor().expect_err("sem mpv");
        assert!(erro.contains("instale") && erro.contains("mpv"), "{erro}");
        assert_eq!(e.mensagem, Some(Mensagem::erro(erro)));
    }

    #[test]
    fn pratica_concluida_grava_progresso_e_avisa_o_professor() {
        let dados = tempfile::tempdir().expect("dados");
        instalar_sintetico(dados.path(), None);
        let (mut e, _) = estado(dados.path());
        e.selecionar(1);
        e.abrir_exercicio("s03-a14", "cinco-notas").expect("abre");
        e.partitura.iniciar_pratica(1_000);
        assert!(e.partitura.praticando());
        let mut vistos = e.atualizar();
        assert_eq!(vistos, [EventoPratica::Passo(0)]);
        // Toca as 16 notas certas (Espera): o professor é avisado de cada passo.
        let notas: Vec<u8> = (0..16)
            .map(|i| [60u8, 62, 64, 65, 67, 69, 71, 72][i % 8])
            .collect();
        for (i, n) in notas.iter().enumerate() {
            e.partitura.nota_tocada(
                true,
                piano_tutor_core::NotaMidi::nova(*n).expect("nota"),
                2_000 + i as u64,
            );
            vistos.extend(e.atualizar());
        }
        // O Roteiro marca a frase em 4 (falas `passo`): o trecho 0..4 fecha limpo ANTES do passo 4.
        let pos = |ev: EventoPratica| vistos.iter().position(|v| *v == ev);
        let fim = pos(EventoPratica::FimDeTrecho { limpo: true }).expect("fim da frase");
        assert!(
            fim < pos(EventoPratica::Passo(4)).expect("passo 4"),
            "{vistos:?}"
        );
        let concluida = EventoPratica::Concluida {
            pct: 100.0,
            concluiu: true,
        };
        // O último trecho fecha limpo e logo antes da conclusão.
        let n = vistos.len();
        assert_eq!(
            &vistos[n - 2..],
            [EventoPratica::FimDeTrecho { limpo: true }, concluida]
        );
        assert!(e.progresso(ID_CURSO).concluido("cinco-notas"));
        // Nova rodada: nota errada isolada NÃO gera evento nenhum (nem fala); parar encerra com
        // faltas e o último trecho fecha com erro.
        e.partitura.limpar_pratica();
        e.partitura.iniciar_pratica(10_000);
        e.atualizar();
        e.partitura.nota_tocada(
            true,
            piano_tutor_core::NotaMidi::nova(61).expect("nota"),
            11_000,
        );
        assert!(e.atualizar().is_empty(), "nota errada isolada não comenta");
        e.partitura.parar_pratica();
        let fim = e.atualizar();
        assert!(
            matches!(
                fim.as_slice(),
                [
                    EventoPratica::FimDeTrecho { limpo: false },
                    EventoPratica::Concluida {
                        concluiu: false,
                        ..
                    }
                ]
            ),
            "{fim:?}"
        );
        // O melhor resultado continua 100 %.
        assert_eq!(
            e.progresso(ID_CURSO)
                .item("cinco-notas")
                .map(|i| i.melhor_pct),
            Some(100.0)
        );
    }

    /// Toca as primeiras `n` notas certas do Exercício sintético, depois de uma nota errada
    /// opcional, e junta os eventos de cada quadro.
    fn tocar_sintetico(e: &mut EstadoCursos, errar_primeiro: bool, n: usize) -> Vec<EventoPratica> {
        let mut vistos = e.atualizar();
        if errar_primeiro {
            e.partitura.nota_tocada(
                true,
                piano_tutor_core::NotaMidi::nova(61).expect("nota"),
                1_500,
            );
            vistos.extend(e.atualizar());
        }
        for i in 0..n {
            let nota = [60u8, 62, 64, 65, 67, 69, 71, 72][i % 8];
            e.partitura.nota_tocada(
                true,
                piano_tutor_core::NotaMidi::nova(nota).expect("nota"),
                2_000 + i as u64,
            );
            vistos.extend(e.atualizar());
        }
        vistos
    }

    #[test]
    fn trecho_com_erro_fecha_com_erro_e_o_seguinte_limpo_fecha_com_acerto() {
        let dados = tempfile::tempdir().expect("dados");
        instalar_sintetico(dados.path(), None);
        let (mut e, _) = estado(dados.path());
        e.selecionar(1);
        e.abrir_exercicio("s03-a14", "cinco-notas").expect("abre");
        e.partitura.iniciar_pratica(1_000);
        let vistos = tocar_sintetico(&mut e, true, 16);
        let trechos: Vec<EventoPratica> = vistos
            .iter()
            .copied()
            .filter(|v| matches!(v, EventoPratica::FimDeTrecho { .. }))
            .collect();
        // Frase 0..4 com a nota errada; frase 4..16 limpa e concluída (100 % das esperadas).
        assert_eq!(
            trechos,
            [
                EventoPratica::FimDeTrecho { limpo: false },
                EventoPratica::FimDeTrecho { limpo: true }
            ],
            "{vistos:?}"
        );
    }

    #[test]
    fn sem_falas_de_passo_o_trecho_e_de_dois_compassos() {
        let dados = tempfile::tempdir().expect("dados");
        instalar_sintetico(dados.path(), None);
        let (mut e, _) = estado(dados.path());
        e.selecionar(1);
        e.abrir_exercicio("s03-a14", "cinco-notas").expect("abre");
        // Sem fronteiras do Roteiro: a cada 2 compassos (8 notas em 4/4).
        e.observador = ObservadorPratica::com_fronteiras(Vec::new());
        e.partitura.iniciar_pratica(1_000);
        let vistos = tocar_sintetico(&mut e, true, 16);
        let trechos: Vec<(usize, EventoPratica)> = vistos
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, v)| matches!(v, EventoPratica::FimDeTrecho { .. }))
            .collect();
        assert_eq!(
            trechos.iter().map(|t| t.1).collect::<Vec<_>>(),
            [
                EventoPratica::FimDeTrecho { limpo: false },
                EventoPratica::FimDeTrecho { limpo: true }
            ],
            "{vistos:?}"
        );
        // O primeiro fecha logo antes do passo 8 (3.º compasso), não antes.
        assert_eq!(
            vistos[trechos[0].0 + 1],
            EventoPratica::Passo(8),
            "{vistos:?}"
        );
    }

    #[test]
    fn fronteiras_de_frase_vem_das_falas_passo_do_exercicio() {
        let r = Roteiro::de_toml(
            r#"formato_versao = 1
[[falas]]
id = "a"
licao = "l1"
exercicio = "e1"
momento = "passo"
passo = 8
texto = "x"
estado = "hipotese"
[[falas]]
id = "b"
licao = "l1"
momento = "passo"
passo = 4
texto = "x"
estado = "aprovada"
[[falas]]
id = "c"
licao = "l1"
exercicio = "outro"
momento = "passo"
passo = 2
texto = "x"
estado = "aprovada"
[[falas]]
id = "d"
licao = "l1"
momento = "erro"
texto = "x"
estado = "aprovada"
"#,
        )
        .expect("roteiro");
        assert_eq!(fronteiras_de_frase(Some(&r), "l1", "e1"), [4, 8]);
        assert!(fronteiras_de_frase(None, "l1", "e1").is_empty());
    }

    #[test]
    fn observador_de_fabrica_so_comenta_ao_concluir_o_exercicio() {
        use crate::exercicios::EstadoExercicios;
        use piano_tutor_core::NotaMidi;
        let n = |x| NotaMidi::nova(x).expect("nota");
        let mut e = EstadoExercicios::novo(None);
        let mut o = ObservadorFabrica::default();
        assert_eq!(
            o.observar(&e),
            [EventoFabrica::Abriu("primeiros-passos/1".into())]
        );
        assert!(o.observar(&e).is_empty());
        // Ex. 1: errar (61) não gera evento; acertar (60) conclui, e a tentativa teve erro.
        e.classificar(true, n(61), 1_000);
        assert!(o.observar(&e).is_empty(), "nota errada isolada não comenta");
        e.classificar(true, n(60), 2_000);
        assert_eq!(
            o.observar(&e),
            [EventoFabrica::Concluiu { sem_erros: false }]
        );
        assert!(o.observar(&e).is_empty());
        // Ex. 2 (10 notas): passos certos não geram evento; o erro conta só na tentativa em
        // curso; reiniciar zera.
        assert!(e.selecionar(1));
        assert_eq!(
            o.observar(&e),
            [EventoFabrica::Abriu("primeiros-passos/2".into())]
        );
        e.classificar(true, n(60), 3_000);
        assert!(o.observar(&e).is_empty(), "passo certo isolado não comenta");
        e.classificar(true, n(61), 3_500);
        assert!(o.observar(&e).is_empty());
        e.reiniciar();
        assert!(o.observar(&e).is_empty());
        let mut t = 4_000;
        for x in [60, 62, 64, 65, 67, 67, 65, 64, 62, 60] {
            e.classificar(true, n(x), t);
            t += 1_000;
        }
        assert_eq!(
            o.observar(&e),
            [EventoFabrica::Concluiu { sem_erros: true }]
        );
        // Ao entrar na tela de novo, o professor abre o exercício outra vez.
        o.reiniciar();
        assert_eq!(
            o.observar(&e),
            [EventoFabrica::Abriu("primeiros-passos/2".into())]
        );
    }

    #[test]
    fn caminho_digitado_tira_aspas_espacos_e_expande_til() {
        assert_eq!(
            caminho_digitado("  '/a/b.ptcurso' "),
            PathBuf::from("/a/b.ptcurso")
        );
        assert_eq!(caminho_digitado("\"/x y/z\""), PathBuf::from("/x y/z"));
        if let Some(h) = dirs::home_dir() {
            assert_eq!(caminho_digitado("~/c.ptcurso"), h.join("c.ptcurso"));
        }
        assert_eq!(formatar_instante(95.0), "1:35");
        assert_eq!(formatar_instante(-1.0), "0:00");
        assert_eq!(formatar_instante(f64::NAN), "0:00");
    }

    #[test]
    fn achar_video_nao_sai_da_pasta() {
        let t = tempfile::tempdir().expect("tmp");
        assert_eq!(achar_video(t.path(), "../x.mp4"), None);
        assert_eq!(achar_video(t.path(), "a/b.mp4"), None);
        assert_eq!(achar_video(t.path(), ""), None);
        assert_eq!(achar_video(t.path(), ".."), None);
        fs::write(t.path().join("a.mp4"), b"x").expect("grava");
        assert_eq!(achar_video(t.path(), "a.mp4"), Some(t.path().join("a.mp4")));
        assert_eq!(achar_video(t.path(), "A.MP4"), Some(t.path().join("a.mp4")));
    }
}
