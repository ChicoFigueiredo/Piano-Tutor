//! Revisão do conteúdo do tutor (M4, etapa 5): estado puro, sem janela e testável.
//!
//! A **oficina** de um tutor fica fora do git, em `<dados>/fontes/<tutor>/oficina/`:
//!
//! ```text
//! partituras/<id>.musicxml            transcrição (Hipótese)
//! partituras/<id>.estado.toml         estado, título, aula, confiança, proveniência, histórico
//! partituras/<id>.divergencias.md     conferência contra a página do PDF e a fala da Aula
//! paginas/<id>.png                    página do PDF renderizada
//! roteiro.toml                        falas do professor (Hipótese)
//! ```
//!
//! Regra do projeto (CONTEXT.md): **só o humano aprova**. Este módulo só muda um estado quando
//! a tela de revisão pede, e só por uma transição válida: Hipótese → Em revisão → Aprovada ou
//! Rejeitada, e de Aprovada ou Rejeitada de volta a Em revisão. Nada aqui promove sozinho.
//! Há um atalho do humano, a **aprovação direta** (Hipótese → Aprovada num clique, só a partir
//! de Hipótese), que fica marcada no registro: `direto = true` e a nota "aprovação direta" no
//! `[[historico]]` da partitura, e `aprovacao_direta = true` na fala do Roteiro.
//!
//! Os arquivos são editados com `toml_edit`: os campos que não são do estado, a ordem e os
//! comentários ficam como estavam. Cada mudança de estado de partitura acrescenta um registro
//! `[[historico]]` (`de`, `para`, `em`, `revisor`, `nota`). A gravação é atômica (arquivo
//! temporário e `rename`).
//!
//! O Roteiro de fábrica é embutido no app; a aprovação das suas falas vai para o arquivo
//! sobreposto `<dados>/roteiro-fabrica-revisao.toml` (por `id`, com `estado` e o `texto`
//! aprovado). [`roteiro_de_fabrica_revisado`] aplica a sobreposição por cima do embutido.
//!
//! Também aqui: a decodificação de PNG (crate `png`, Rust puro) das páginas do PDF e o
//! [`Lancador`] que abre o MusicXML no MuseScore. Nada disso foi exercido com o MuseScore ou a
//! voz reais nos testes (**NÃO VERIFICADO** em CI): os testes injetam executores falsos.

use crate::pdf_sincronizado::{EntradaDoMapa, MAX_PAGINAS, MapaCompassos, Regiao};
use crate::professor_falante::{
    ErroRoteiro, EstadoFala, Fala, ROTEIRO_DE_FABRICA_TOML, Roteiro, pastas_de_programas,
    procurar_executavel,
};
use serde::Deserialize;
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;
use toml_edit::{ArrayOfTables, DocumentMut, InlineTable, Item, Table, Value, value};

/// Estado do conteúdo em revisão (o mesmo tipo das falas do Roteiro).
pub type Estado = EstadoFala;

/// Quem assina as revisões feitas na tela.
pub const REVISOR: &str = "Chico";

/// Arquivo (em `<dados>`) com a aprovação das falas do Roteiro de fábrica.
pub const ARQUIVO_REVISAO_DA_FABRICA: &str = "roteiro-fabrica-revisao.toml";

/// Os quatro estados, na ordem do fluxo.
pub const ESTADOS: [Estado; 4] = [
    Estado::Hipotese,
    Estado::EmRevisao,
    Estado::Aprovada,
    Estado::Rejeitada,
];

/// Texto gravado nos arquivos (`snake_case`, igual ao do formato do Pacote de Curso).
pub const fn estado_para_texto(e: Estado) -> &'static str {
    match e {
        Estado::Hipotese => "hipotese",
        Estado::EmRevisao => "em_revisao",
        Estado::Aprovada => "aprovada",
        Estado::Rejeitada => "rejeitada",
    }
}

/// Lê o texto gravado nos arquivos. Aceita `em revisao` e `em-revisao` por tolerância.
pub fn estado_de_texto(t: &str) -> Option<Estado> {
    let t = t.trim().to_lowercase().replace([' ', '-'], "_");
    ESTADOS.into_iter().find(|e| estado_para_texto(*e) == t)
}

/// Rótulo para a tela, com acento.
pub const fn rotulo_do_estado(e: Estado) -> &'static str {
    match e {
        Estado::Hipotese => "Hipótese",
        Estado::EmRevisao => "Em revisão",
        Estado::Aprovada => "Aprovada",
        Estado::Rejeitada => "Rejeitada",
    }
}

/// Símbolo do estado em texto simples (a fonte da janela não tem ícones): a tela nunca usa só a
/// cor, e é o mesmo padrão `[OK]`, `[X]` dos Exercícios.
pub const fn simbolo_do_estado(e: Estado) -> &'static str {
    match e {
        Estado::Hipotese => "[?]",
        Estado::EmRevisao => "[...]",
        Estado::Aprovada => "[OK]",
        Estado::Rejeitada => "[X]",
    }
}

/// A transição `de` → `para` é permitida? Hipótese → Em revisão (ou Rejeitada, se for lixo);
/// Em revisão → Aprovada ou Rejeitada; Aprovada e Rejeitada voltam a Em revisão. Só o humano
/// chega a Aprovada, e sempre por Em revisão.
pub const fn transicao_valida(de: Estado, para: Estado) -> bool {
    matches!(
        (de, para),
        (Estado::Hipotese, Estado::EmRevisao | Estado::Rejeitada)
            | (Estado::EmRevisao, Estado::Aprovada | Estado::Rejeitada)
            | (Estado::Aprovada | Estado::Rejeitada, Estado::EmRevisao)
    )
}

/// Nota gravada no histórico quando o humano aprova uma Hipótese direto, sem passar por Em revisão.
pub const NOTA_DA_APROVACAO_DIRETA: &str = "aprovação direta";

/// A **aprovação direta** (Hipótese → Aprovada num clique) vale para este estado de partida?
/// Só para Hipótese: de Em revisão o caminho é "Aprovar", e Aprovada ou Rejeitada voltam por
/// Em revisão. Não muda [`transicao_valida`] (o fluxo em dois passos continua igual).
pub const fn aprovacao_direta_valida(de: Estado) -> bool {
    matches!(de, Estado::Hipotese)
}

// --- Erros ----------------------------------------------------------------------------------

/// Falha ao ler ou gravar conteúdo de revisão.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ErroRevisao {
    Disco(String),
    Toml(String),
    /// Mudança de estado fora do fluxo.
    TransicaoInvalida {
        de: Estado,
        para: Estado,
    },
    /// Fala (ou item) que não existe no arquivo.
    Inexistente(String),
    /// Campo com valor inesperado.
    Formato(String),
}

impl fmt::Display for ErroRevisao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disco(e) => write!(f, "problema no disco: {e}"),
            Self::Toml(e) => write!(f, "arquivo TOML inválido: {e}"),
            Self::TransicaoInvalida { de, para } => write!(
                f,
                "não dá para ir de {} para {}: o fluxo é Hipótese → Em revisão → Aprovada ou Rejeitada",
                rotulo_do_estado(*de),
                rotulo_do_estado(*para)
            ),
            Self::Inexistente(i) => write!(f, "não encontrei \"{i}\" no arquivo"),
            Self::Formato(e) => write!(f, "formato inesperado: {e}"),
        }
    }
}

impl std::error::Error for ErroRevisao {}

fn erro_de_disco(caminho: &Path, e: &std::io::Error) -> ErroRevisao {
    ErroRevisao::Disco(format!("{}: {e}", caminho.display()))
}

// --- Gravação atômica e edição com toml_edit ------------------------------------------------

/// Faz `fsync` do diretório `dir`, para o `rename` de dentro dele sobreviver a uma queda de
/// energia (sem isto o diretório pode voltar ao estado anterior: íntegro, mas sem a última
/// gravação). No Unix abre o diretório e sincroniza; onde não há essa operação (Windows) é um
/// no-op que devolve `Ok`. Revisão independente da E01, achado R8.
pub fn sincronizar_diretorio(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        fs::File::open(dir)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        Ok(())
    }
}

/// Grava `texto` em `caminho` por arquivo temporário vizinho e `rename`, e sincroniza o diretório
/// depois do `rename` (durabilidade; ver [`sincronizar_diretorio`]). Se `caminho` já é um arquivo
/// comum, o novo herda as permissões dele (segunda revisão da E01, achado N4).
pub fn gravar_atomico(caminho: &Path, texto: &str) -> Result<(), ErroRevisao> {
    let nome = caminho
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = caminho.with_file_name(format!(".{nome}.{}.tmp", std::process::id()));
    let r = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(texto.as_bytes())?;
        // Trocar o arquivo por `rename` não pode mudar as permissões dele (o temporário nasce
        // com o modo do `umask`): quem já existe como arquivo comum passa suas permissões ao novo.
        if let Ok(m) = fs::metadata(caminho)
            && m.is_file()
        {
            let _ = f.set_permissions(m.permissions());
        }
        f.sync_all()?;
        fs::rename(&tmp, caminho)
    })();
    if let Err(e) = r {
        let _ = fs::remove_file(&tmp);
        return Err(erro_de_disco(caminho, &e));
    }
    // O arquivo já está no lugar: falhar o `fsync` do diretório só tira a garantia contra queda de
    // energia, não desfaz a gravação, então não vira erro (nem avisa "falha" de algo que valeu).
    if let Some(dir) = caminho.parent() {
        let _ = sincronizar_diretorio(if dir.as_os_str().is_empty() {
            Path::new(".")
        } else {
            dir
        });
    }
    Ok(())
}

fn ler_documento(caminho: &Path) -> Result<DocumentMut, ErroRevisao> {
    let texto = fs::read_to_string(caminho).map_err(|e| erro_de_disco(caminho, &e))?;
    texto
        .parse::<DocumentMut>()
        .map_err(|e| ErroRevisao::Toml(format!("{}: {e}", caminho.display())))
}

/// Troca o texto de uma chave de string sem perder o comentário que vem depois do valor.
fn definir_texto(tabela: &mut Table, chave: &str, novo: &str) {
    if let Some(v) = tabela.get_mut(chave).and_then(Item::as_value_mut) {
        let decoracao = v.decor().clone();
        *v = Value::from(novo);
        *v.decor_mut() = decoracao;
    } else {
        tabela[chave] = value(novo);
    }
}

fn estado_na_tabela(t: &Table) -> Result<Estado, ErroRevisao> {
    let texto = t
        .get("estado")
        .and_then(Item::as_str)
        .ok_or_else(|| ErroRevisao::Formato("falta o campo `estado`".into()))?;
    estado_de_texto(texto)
        .ok_or_else(|| ErroRevisao::Formato(format!("estado desconhecido: \"{texto}\"")))
}

// --- Oficina e partituras -------------------------------------------------------------------

/// Pasta `oficina` de um tutor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Oficina {
    /// Nome da pasta do tutor em `fontes/` (ou da própria pasta, no caminho extra).
    pub tutor: String,
    pub raiz: PathBuf,
}

impl Oficina {
    pub fn pasta_de_partituras(&self) -> PathBuf {
        self.raiz.join("partituras")
    }

    pub fn arquivo_do_roteiro(&self) -> PathBuf {
        self.raiz.join("roteiro.toml")
    }

    pub fn tem_roteiro(&self) -> bool {
        self.arquivo_do_roteiro().is_file()
    }

    /// Partituras da oficina (`partituras/*.estado.toml`), por aula e depois por id.
    pub fn partituras(&self) -> Vec<ItemPartitura> {
        let Ok(entradas) = fs::read_dir(self.pasta_de_partituras()) else {
            return Vec::new();
        };
        let mut itens: Vec<ItemPartitura> = entradas
            .flatten()
            .filter_map(|e| {
                let nome = e.file_name().to_string_lossy().into_owned();
                let id = nome.strip_suffix(".estado.toml")?.to_owned();
                (!id.is_empty() && !id.starts_with('.')).then(|| ItemPartitura::ler(self, &id))
            })
            .collect();
        itens.sort_by(|a, b| {
            (a.aula.unwrap_or(u32::MAX), &a.id).cmp(&(b.aula.unwrap_or(u32::MAX), &b.id))
        });
        itens
    }
}

/// Oficinas em `<dados>/fontes/*/oficina` e, se houver, a pasta extra (que pode ser a própria
/// `oficina` ou a pasta do tutor que a contém). Sem repetição, em ordem de nome.
pub fn descobrir_oficinas(dados: &Path, extra: Option<&Path>) -> Vec<Oficina> {
    let mut v: Vec<Oficina> = Vec::new();
    if let Ok(entradas) = fs::read_dir(dados.join("fontes")) {
        for e in entradas.flatten() {
            let raiz = e.path().join("oficina");
            if raiz.is_dir() {
                v.push(Oficina {
                    tutor: e.file_name().to_string_lossy().into_owned(),
                    raiz,
                });
            }
        }
    }
    v.sort_by(|a, b| a.tutor.cmp(&b.tutor));
    if let Some(x) = extra.filter(|x| x.is_dir()) {
        let raiz = if x.join("oficina").is_dir() {
            x.join("oficina")
        } else {
            x.to_path_buf()
        };
        let tutor = if raiz.file_name().is_some_and(|n| n == "oficina") {
            raiz.parent()
                .and_then(Path::file_name)
                .map(|n| n.to_string_lossy().into_owned())
        } else {
            raiz.file_name().map(|n| n.to_string_lossy().into_owned())
        }
        .unwrap_or_else(|| "extra".to_owned());
        if !v.iter().any(|o| mesmo_lugar(&o.raiz, &raiz)) {
            v.push(Oficina { tutor, raiz });
        }
    }
    v
}

fn mesmo_lugar(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// Um registro do `[[historico]]` de uma partitura.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct RegistroDeHistorico {
    pub de: String,
    pub para: String,
    pub em: String,
    #[serde(default)]
    pub revisor: String,
    #[serde(default)]
    pub nota: Option<String>,
    /// `true` na aprovação direta (Hipótese → Aprovada sem passar por Em revisão).
    #[serde(default)]
    pub direto: bool,
}

/// Campos do `estado.toml` que a tela lê (o resto do arquivo é ignorado aqui e preservado na
/// gravação).
#[derive(Debug, Default, Deserialize)]
struct EstadoToml {
    #[serde(default)]
    titulo: Option<String>,
    #[serde(default)]
    aula: Option<u32>,
    #[serde(default)]
    estado: Option<String>,
    #[serde(default)]
    confianca: Option<f64>,
    #[serde(default)]
    proveniencia: Option<String>,
    #[serde(default)]
    pdf: Option<String>,
    #[serde(default)]
    pagina_png: Option<String>,
    /// Páginas PNG do PDF de origem, em ordem (opcional; sem ela vale só `pagina_png`).
    #[serde(default)]
    paginas: Vec<String>,
    /// Mapa compasso → página (e região) opcional (ticket 20); números a partir de 1.
    #[serde(default)]
    mapa_compassos: Vec<EntradaDoMapaToml>,
    #[serde(default)]
    historico: Vec<RegistroDeHistorico>,
}

/// Linha de `[[mapa_compassos]]`: `compasso` e `pagina` a partir de 1; `regiao` opcional
/// `[x, y, largura, altura]` em frações da página (0 a 1).
#[derive(Debug, Default, Deserialize)]
struct EntradaDoMapaToml {
    compasso: usize,
    pagina: usize,
    #[serde(default)]
    regiao: Option<[f32; 4]>,
}

/// Uma partitura da oficina, como a lista da tela a mostra.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemPartitura {
    pub id: String,
    pub titulo: String,
    pub aula: Option<u32>,
    pub estado: Estado,
    pub confianca: Option<f64>,
    pub proveniencia: String,
    /// Caminho do PDF de origem (só informativo: o PDF fica fora do app).
    pub pdf: Option<String>,
    /// PNG da página, já resolvido dentro da oficina (`None` se não informado ou inseguro).
    pub pagina_png: Option<PathBuf>,
    /// Páginas PNG do PDF de origem, em ordem (já resolvidas dentro da oficina). Vem de
    /// `paginas`; sem ela, só `pagina_png` (se houver). Vazio = sem PDF para mostrar.
    pub paginas: Vec<PathBuf>,
    /// Mapa compasso → página (e região) do `estado.toml`; vazio sem `mapa_compassos`.
    pub mapa: MapaCompassos,
    pub musicxml: PathBuf,
    pub estado_toml: PathBuf,
    pub divergencias: PathBuf,
    pub historico: Vec<RegistroDeHistorico>,
    /// Por que o `estado.toml` não pôde ser lido por inteiro; com problema, o item não muda.
    pub problema: Option<String>,
}

/// Caminho relativo e seguro dentro de `raiz` (sem `..`, sem raiz absoluta).
fn dentro_de(raiz: &Path, relativo: &str) -> Option<PathBuf> {
    let r = Path::new(relativo);
    let seguro = !relativo.trim().is_empty()
        && r.components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    seguro.then(|| raiz.join(r))
}

/// Páginas do PDF: `paginas` (todas seguras, ou a lista inteira é ignorada para não deslocar os
/// números de página do mapa), senão só `pagina_png`. No máximo [`MAX_PAGINAS`].
fn paginas_da_oficina(raiz: &Path, listadas: &[String], unica: Option<&PathBuf>) -> Vec<PathBuf> {
    let seguras: Option<Vec<PathBuf>> = listadas.iter().map(|p| dentro_de(raiz, p)).collect();
    match seguras {
        Some(v) if !v.is_empty() => v.into_iter().take(MAX_PAGINAS).collect(),
        _ => unica.cloned().into_iter().collect(),
    }
}

/// Converte as linhas do TOML (a partir de 1) no mapa (a partir de 0). Linha com número zero é
/// ignorada; região inválida vira "sem região" (a página continua valendo).
fn mapa_do_toml(paginas: usize, linhas: &[EntradaDoMapaToml]) -> MapaCompassos {
    MapaCompassos::novo(
        paginas,
        linhas.iter().filter_map(|l| {
            let compasso = l.compasso.checked_sub(1)?;
            let pagina = l.pagina.checked_sub(1)?;
            let regiao = l.regiao.and_then(|[x, y, w, h]| Regiao::nova(x, y, w, h));
            Some(EntradaDoMapa {
                compasso,
                pagina,
                regiao,
            })
        }),
    )
}

impl ItemPartitura {
    /// Lê `partituras/<id>.estado.toml`. Erros viram `problema`, nunca pânico.
    pub fn ler(oficina: &Oficina, id: &str) -> Self {
        let pasta = oficina.pasta_de_partituras();
        let estado_toml = pasta.join(format!("{id}.estado.toml"));
        let mut item = Self {
            id: id.to_owned(),
            titulo: id.to_owned(),
            aula: None,
            estado: Estado::Hipotese,
            confianca: None,
            proveniencia: String::new(),
            pdf: None,
            pagina_png: None,
            paginas: Vec::new(),
            mapa: MapaCompassos::default(),
            musicxml: pasta.join(format!("{id}.musicxml")),
            estado_toml: estado_toml.clone(),
            divergencias: pasta.join(format!("{id}.divergencias.md")),
            historico: Vec::new(),
            problema: None,
        };
        let lido = fs::read_to_string(&estado_toml)
            .map_err(|e| e.to_string())
            .and_then(|t| toml::from_str::<EstadoToml>(&t).map_err(|e| e.to_string()));
        let t = match lido {
            Ok(t) => t,
            Err(e) => {
                item.problema = Some(format!("estado.toml ilegível: {e}"));
                return item;
            }
        };
        if let Some(titulo) = t.titulo.filter(|s| !s.trim().is_empty()) {
            item.titulo = titulo;
        }
        item.aula = t.aula;
        item.confianca = t.confianca;
        item.proveniencia = t.proveniencia.unwrap_or_default();
        item.pdf = t.pdf;
        item.pagina_png = t
            .pagina_png
            .as_deref()
            .and_then(|p| dentro_de(&oficina.raiz, p));
        item.paginas = paginas_da_oficina(&oficina.raiz, &t.paginas, item.pagina_png.as_ref());
        item.mapa = mapa_do_toml(item.paginas.len(), &t.mapa_compassos);
        item.historico = t.historico;
        match t.estado.as_deref() {
            None => item.problema = Some("falta o campo `estado`".into()),
            Some(s) => match estado_de_texto(s) {
                Some(e) => item.estado = e,
                None => item.problema = Some(format!("estado desconhecido: \"{s}\"")),
            },
        }
        item
    }

    /// Texto de `divergencias.md`, se existir.
    pub fn divergencias_em_texto(&self) -> Option<String> {
        fs::read_to_string(&self.divergencias).ok()
    }

    pub fn musicxml_existe(&self) -> bool {
        self.musicxml.is_file()
    }

    /// Data de modificação do MusicXML (para detectar edição no MuseScore).
    pub fn mtime_do_musicxml(&self) -> Option<SystemTime> {
        fs::metadata(&self.musicxml).and_then(|m| m.modified()).ok()
    }

    /// Muda o estado no `estado.toml` (veja [`mudar_estado_da_partitura`]).
    pub fn mudar_estado(
        &self,
        para: Estado,
        nota: Option<&str>,
        em: &str,
    ) -> Result<(), ErroRevisao> {
        mudar_estado_da_partitura(&self.estado_toml, para, nota, em)
    }

    /// Aprova a Hipótese direto (veja [`aprovar_direto_a_partitura`]).
    pub fn aprovar_direto(&self, em: &str) -> Result<(), ErroRevisao> {
        aprovar_direto_a_partitura(&self.estado_toml, em)
    }
}

/// Muda o estado de uma partitura no seu `estado.toml` e acrescenta o registro
/// `[[historico]] { de, para, em, revisor = "Chico", nota }`. O estado de partida é o do
/// **arquivo agora** (não o da tela), então uma edição feita por fora não é atropelada.
/// Os outros campos, a ordem e os comentários ficam como estavam.
pub fn mudar_estado_da_partitura(
    estado_toml: &Path,
    para: Estado,
    nota: Option<&str>,
    em: &str,
) -> Result<(), ErroRevisao> {
    mudar_estado_gravando(estado_toml, para, nota, em, false)
}

/// Aprovação direta: Hipótese → Aprovada num passo só, **só a partir de Hipótese** (o estado é
/// o do arquivo agora). O histórico recebe `de = "hipotese"`, `para = "aprovada"`,
/// `direto = true` e a nota [`NOTA_DA_APROVACAO_DIRETA`].
pub fn aprovar_direto_a_partitura(estado_toml: &Path, em: &str) -> Result<(), ErroRevisao> {
    mudar_estado_gravando(
        estado_toml,
        Estado::Aprovada,
        Some(NOTA_DA_APROVACAO_DIRETA),
        em,
        true,
    )
}

fn mudar_estado_gravando(
    estado_toml: &Path,
    para: Estado,
    nota: Option<&str>,
    em: &str,
    direto: bool,
) -> Result<(), ErroRevisao> {
    let mut doc = ler_documento(estado_toml)?;
    let de = estado_na_tabela(doc.as_table())?;
    let permitida = if direto {
        aprovacao_direta_valida(de)
    } else {
        transicao_valida(de, para)
    };
    if !permitida {
        return Err(ErroRevisao::TransicaoInvalida { de, para });
    }
    definir_texto(doc.as_table_mut(), "estado", estado_para_texto(para));

    let mut registro = Table::new();
    registro["de"] = value(estado_para_texto(de));
    registro["para"] = value(estado_para_texto(para));
    registro["em"] = value(em);
    registro["revisor"] = value(REVISOR);
    if direto {
        registro["direto"] = value(true);
    }
    if let Some(n) = nota.map(str::trim).filter(|n| !n.is_empty()) {
        registro["nota"] = value(n);
    }
    let tabela = doc.as_table_mut();
    match tabela.get_mut("historico") {
        None => {
            let mut arr = ArrayOfTables::new();
            arr.push(registro);
            tabela["historico"] = Item::ArrayOfTables(arr);
        }
        Some(Item::ArrayOfTables(arr)) => arr.push(registro),
        Some(Item::Value(Value::Array(arr))) => {
            let mut inline = InlineTable::new();
            for (k, v) in &registro {
                if let Some(v) = v.as_value() {
                    inline.insert(k, v.clone());
                }
            }
            arr.push(Value::InlineTable(inline));
        }
        Some(_) => {
            return Err(ErroRevisao::Formato(
                "`historico` não é uma lista de registros".into(),
            ));
        }
    }
    gravar_atomico(estado_toml, &doc.to_string())
}

// --- Roteiro (oficina e fábrica) ------------------------------------------------------------

/// O que mudar numa fala.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MudancaDeFala {
    pub estado: Option<Estado>,
    pub texto: Option<String>,
    /// Aprovação direta: Hipótese → Aprovada num passo (só vale a partir de Hipótese). Com
    /// `direto`, `estado` é ignorado e a fala fica com `aprovacao_direta = true`.
    pub direto: bool,
}

impl MudancaDeFala {
    /// A aprovação direta de uma fala em Hipótese.
    pub fn aprovacao_direta() -> Self {
        Self {
            direto: true,
            ..Self::default()
        }
    }
}

/// Campo gravado na fala do Roteiro (oficina ou sobreposição de fábrica) aprovada direto.
pub const CAMPO_APROVACAO_DIRETA: &str = "aprovacao_direta";

/// De onde vem um Roteiro na tela de revisão.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FonteDeRoteiro {
    /// `roteiro.toml` da oficina de um tutor (editado no lugar).
    Oficina { tutor: String, arquivo: PathBuf },
    /// Roteiro de fábrica embutido; a revisão vai para `<dados>/roteiro-fabrica-revisao.toml`.
    Fabrica { dados: PathBuf },
}

impl FonteDeRoteiro {
    pub fn rotulo(&self) -> String {
        match self {
            Self::Oficina { tutor, .. } => format!("Roteiro do tutor {tutor}"),
            Self::Fabrica { .. } => "Roteiro de fábrica (embutido)".to_owned(),
        }
    }

    /// As fontes de Roteiro: a de fábrica (se há pasta de dados) e a de cada oficina com
    /// `roteiro.toml`.
    pub fn listar(dados: Option<&Path>, oficinas: &[Oficina]) -> Vec<Self> {
        let mut v = Vec::new();
        if let Some(d) = dados {
            v.push(Self::Fabrica {
                dados: d.to_path_buf(),
            });
        }
        v.extend(
            oficinas
                .iter()
                .filter(|o| o.tem_roteiro())
                .map(|o| Self::Oficina {
                    tutor: o.tutor.clone(),
                    arquivo: o.arquivo_do_roteiro(),
                }),
        );
        v
    }

    /// Carrega o Roteiro (o de fábrica já com a revisão aplicada).
    pub fn carregar(&self) -> Result<Roteiro, ErroRevisao> {
        match self {
            Self::Oficina { arquivo, .. } => {
                let t = fs::read_to_string(arquivo).map_err(|e| erro_de_disco(arquivo, &e))?;
                Roteiro::de_toml(&t).map_err(|e| ErroRevisao::Toml(e.to_string()))
            }
            Self::Fabrica { dados } => {
                roteiro_de_fabrica_revisado(dados).map_err(|e| ErroRevisao::Toml(e.to_string()))
            }
        }
    }

    /// Aplica a mudança à fala `id` e devolve o estado que ela ficou. Mudança de estado só pelo
    /// fluxo válido; **editar o texto de uma fala Aprovada a devolve a Em revisão** (o humano
    /// aprovou o texto antigo, não este).
    pub fn aplicar(&self, id: &str, mudanca: &MudancaDeFala) -> Result<Estado, ErroRevisao> {
        if let Some(t) = &mudanca.texto
            && t.trim().is_empty()
        {
            return Err(ErroRevisao::Formato(
                "o texto da fala não pode ficar vazio".into(),
            ));
        }
        match self {
            Self::Oficina { arquivo, .. } => aplicar_no_roteiro_da_oficina(arquivo, id, mudanca),
            Self::Fabrica { dados } => aplicar_na_fabrica(dados, id, mudanca),
        }
    }
}

/// Estado final de uma fala: `pedido` (conferido contra o fluxo), ou, se o texto mudou numa
/// fala Aprovada, Em revisão.
fn estado_apos(
    atual: Estado,
    mudanca: &MudancaDeFala,
    texto_atual: &str,
) -> Result<Estado, ErroRevisao> {
    if mudanca.direto {
        if !aprovacao_direta_valida(atual) {
            return Err(ErroRevisao::TransicaoInvalida {
                de: atual,
                para: Estado::Aprovada,
            });
        }
        return Ok(Estado::Aprovada);
    }
    let mut novo = atual;
    let texto_mudou = mudanca
        .texto
        .as_deref()
        .is_some_and(|t| t.trim() != texto_atual.trim());
    if texto_mudou && atual == Estado::Aprovada {
        novo = Estado::EmRevisao;
    }
    if let Some(p) = mudanca.estado
        && p != novo
    {
        if !transicao_valida(novo, p) {
            return Err(ErroRevisao::TransicaoInvalida { de: novo, para: p });
        }
        novo = p;
    }
    Ok(novo)
}

fn aplicar_no_roteiro_da_oficina(
    arquivo: &Path,
    id: &str,
    mudanca: &MudancaDeFala,
) -> Result<Estado, ErroRevisao> {
    let mut doc = ler_documento(arquivo)?;
    let falas = doc
        .get_mut("falas")
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| ErroRevisao::Formato("o roteiro não tem `[[falas]]`".into()))?;
    let fala = falas
        .iter_mut()
        .find(|t| t.get("id").and_then(Item::as_str) == Some(id))
        .ok_or_else(|| ErroRevisao::Inexistente(id.to_owned()))?;
    let atual = estado_na_tabela(fala)?;
    let texto_atual = fala
        .get("texto")
        .and_then(Item::as_str)
        .unwrap_or_default()
        .to_owned();
    let novo = estado_apos(atual, mudanca, &texto_atual)?;
    if let Some(t) = &mudanca.texto {
        definir_texto(fala, "texto", t.trim());
    }
    if novo != atual {
        definir_texto(fala, "estado", estado_para_texto(novo));
        // O registro de aprovação direta vale só para a aprovação que ele descreve.
        fala.remove(CAMPO_APROVACAO_DIRETA);
        if mudanca.direto {
            fala[CAMPO_APROVACAO_DIRETA] = value(true);
        }
    }
    gravar_atomico(arquivo, &doc.to_string())?;
    Ok(novo)
}

/// Caminho da sobreposição de revisão do Roteiro de fábrica.
pub fn arquivo_de_revisao_da_fabrica(dados: &Path) -> PathBuf {
    dados.join(ARQUIVO_REVISAO_DA_FABRICA)
}

fn aplicar_na_fabrica(
    dados: &Path,
    id: &str,
    mudanca: &MudancaDeFala,
) -> Result<Estado, ErroRevisao> {
    let atual_roteiro =
        roteiro_de_fabrica_revisado(dados).map_err(|e| ErroRevisao::Toml(e.to_string()))?;
    let fala = atual_roteiro
        .falas
        .iter()
        .find(|f| f.id == id)
        .ok_or_else(|| ErroRevisao::Inexistente(id.to_owned()))?;
    let novo = estado_apos(fala.estado, mudanca, &fala.texto)?;
    let texto_novo = mudanca
        .texto
        .as_deref()
        .map_or_else(|| fala.texto.clone(), |t| t.trim().to_owned());

    let caminho = arquivo_de_revisao_da_fabrica(dados);
    let mut doc = match fs::read_to_string(&caminho) {
        Ok(t) => t
            .parse::<DocumentMut>()
            .map_err(|e| ErroRevisao::Toml(format!("{}: {e}", caminho.display())))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut d = DocumentMut::new();
            d["formato_versao"] = value(1);
            d
        }
        Err(e) => return Err(erro_de_disco(&caminho, &e)),
    };
    if doc.get("falas").is_none() {
        doc["falas"] = Item::ArrayOfTables(ArrayOfTables::new());
    }
    let falas = doc
        .get_mut("falas")
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| ErroRevisao::Formato("`falas` não é `[[falas]]`".into()))?;
    let existente = falas
        .iter_mut()
        .find(|t| t.get("id").and_then(Item::as_str) == Some(id));
    // Grava sempre o texto junto: a revisão guarda o que foi aprovado, mesmo que uma versão
    // futura do app mude o texto embutido.
    match existente {
        Some(t) => {
            definir_texto(t, "estado", estado_para_texto(novo));
            definir_texto(t, "texto", &texto_novo);
            if novo != fala.estado {
                t.remove(CAMPO_APROVACAO_DIRETA);
                if mudanca.direto {
                    t[CAMPO_APROVACAO_DIRETA] = value(true);
                }
            }
        }
        None => {
            let mut t = Table::new();
            t["id"] = value(id);
            t["estado"] = value(estado_para_texto(novo));
            t["texto"] = value(texto_novo);
            if mudanca.direto {
                t[CAMPO_APROVACAO_DIRETA] = value(true);
            }
            falas.push(t);
        }
    }
    gravar_atomico(&caminho, &doc.to_string())?;
    Ok(novo)
}

/// Uma linha da sobreposição de revisão do Roteiro de fábrica.
#[derive(Debug, Deserialize)]
struct FalaRevisada {
    id: String,
    #[serde(default)]
    estado: Option<EstadoFala>,
    #[serde(default)]
    texto: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SobreposicaoToml {
    #[serde(default)]
    falas: Vec<FalaRevisada>,
}

/// Aplica por cima de `roteiro` o texto da sobreposição (`id` → `estado`/`texto`). Ids que não
/// existem no roteiro são ignorados. Devolve quantas falas foram tocadas.
pub fn aplicar_sobreposicao(roteiro: &mut Roteiro, texto: &str) -> Result<usize, ErroRoteiro> {
    let s: SobreposicaoToml =
        toml::from_str(texto).map_err(|e| ErroRoteiro::Toml(format!("revisão: {e}")))?;
    let mut tocadas = HashSet::new();
    for r in s.falas {
        let Some(f) = roteiro.falas.iter_mut().find(|f| f.id == r.id) else {
            continue;
        };
        if let Some(e) = r.estado {
            f.estado = e;
        }
        if let Some(t) = r.texto.filter(|t| !t.trim().is_empty()) {
            f.texto = t;
        }
        tocadas.insert(r.id);
    }
    Ok(tocadas.len())
}

/// O Roteiro de fábrica com a revisão do Chico aplicada: as falas aprovadas (e os textos
/// ajustados) de `<dados>/roteiro-fabrica-revisao.toml` por cima do roteiro embutido. Sem esse
/// arquivo, é o embutido (tudo em Hipótese). É a função que as telas de Curso usam.
pub fn roteiro_de_fabrica_revisado(dados: &Path) -> Result<Roteiro, ErroRoteiro> {
    let mut r = crate::professor_falante::ler_roteiro(ROTEIRO_DE_FABRICA_TOML)?;
    let caminho = arquivo_de_revisao_da_fabrica(dados);
    match fs::read_to_string(&caminho) {
        Ok(t) => {
            aplicar_sobreposicao(&mut r, &t)?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(ErroRoteiro::Toml(format!("{}: {e}", caminho.display())));
        }
    }
    Ok(r)
}

/// Quantas falas há por estado, na ordem de [`ESTADOS`].
pub fn contar_por_estado(falas: &[Fala]) -> [usize; 4] {
    let mut n = [0; 4];
    for f in falas {
        if let Some(i) = ESTADOS.iter().position(|e| *e == f.estado) {
            n[i] += 1;
        }
    }
    n
}

// --- Imagem da página (PNG) -----------------------------------------------------------------

/// Limite de pixels de uma página (A4 a 300 dpi tem ~8,7 milhões).
pub const MAX_PIXELS_DA_PAGINA: u64 = 40_000_000;
/// Limite do arquivo PNG lido.
pub const MAX_BYTES_DO_PNG: u64 = 64 * 1024 * 1024;

/// Imagem decodificada em RGBA de 8 bits (não pré-multiplicado).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImagemRgba {
    pub largura: usize,
    pub altura: usize,
    pub rgba: Vec<u8>,
}

/// Decodifica um PNG de qualquer tipo de cor e profundidade para RGBA de 8 bits.
pub fn decodificar_png(bytes: &[u8]) -> Result<ImagemRgba, String> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut leitor = dec.read_info().map_err(|e| format!("PNG inválido: {e}"))?;
    let (largura, altura) = {
        let i = leitor.info();
        (i.width, i.height)
    };
    if u64::from(largura) * u64::from(altura) > MAX_PIXELS_DA_PAGINA {
        return Err(format!("imagem grande demais ({largura}x{altura})"));
    }
    let tam = leitor
        .output_buffer_size()
        .ok_or_else(|| "imagem grande demais".to_owned())?;
    let mut buf = vec![0u8; tam];
    let quadro = leitor
        .next_frame(&mut buf)
        .map_err(|e| format!("PNG inválido: {e}"))?;
    let dados = &buf[..quadro.buffer_size()];
    let rgba: Vec<u8> = match quadro.color_type {
        png::ColorType::Rgba => dados.to_vec(),
        png::ColorType::Rgb => dados
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|[r, g, b]| [*r, *g, *b, 255])
            .collect(),
        png::ColorType::Grayscale => dados.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => dados
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|[g, a]| [*g, *g, *g, *a])
            .collect(),
        png::ColorType::Indexed => {
            return Err("PNG indexado não expandido".into());
        }
    };
    let esperado = largura as usize * altura as usize * 4;
    if rgba.len() != esperado {
        return Err("PNG com dados incompletos".into());
    }
    Ok(ImagemRgba {
        largura: largura as usize,
        altura: altura as usize,
        rgba,
    })
}

/// Lê e decodifica um PNG do disco, com limite de tamanho do arquivo.
pub fn ler_png(caminho: &Path) -> Result<ImagemRgba, String> {
    let meta = fs::metadata(caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
    if !meta.is_file() || meta.len() > MAX_BYTES_DO_PNG {
        return Err(format!(
            "{} não é um arquivo PNG de tamanho aceitável",
            caminho.display()
        ));
    }
    let bytes = fs::read(caminho).map_err(|e| format!("{}: {e}", caminho.display()))?;
    decodificar_png(&bytes)
}

// --- MuseScore ------------------------------------------------------------------------------

/// Nomes do programa MuseScore, do mais comum ao menos.
pub const NOMES_DO_MUSESCORE: [&str; 5] =
    ["mscore", "musescore", "mscore4", "musescore4", "MuseScore4"];

/// Mensagem quando não há MuseScore.
pub const SEM_MUSESCORE: &str = "O MuseScore não foi encontrado (procurei `mscore` e `musescore` no PATH e em ~/.local/bin). Instale o MuseScore para corrigir a partitura; no MuseScore, exporte o MusicXML sobre o mesmo arquivo e clique em Recarregar.";

/// Primeiro MuseScore achado nas `pastas`.
pub fn procurar_musescore(pastas: &[PathBuf]) -> Option<PathBuf> {
    NOMES_DO_MUSESCORE
        .iter()
        .find_map(|n| procurar_executavel(n, pastas))
}

/// Abre um arquivo no editor de partituras (injetável nos testes).
pub trait Lancador: Send {
    /// Abre `arquivo` sem travar a janela. O erro vem pronto para a tela.
    fn abrir_no_editor(&mut self, arquivo: &Path) -> Result<(), String>;
}

/// O MuseScore do sistema: `mscore <arquivo>` em processo separado.
#[derive(Debug, Default)]
pub struct LancadorMuseScore;

impl Lancador for LancadorMuseScore {
    fn abrir_no_editor(&mut self, arquivo: &Path) -> Result<(), String> {
        let programa = procurar_musescore(&pastas_de_programas()).ok_or(SEM_MUSESCORE)?;
        let mut filho = Command::new(&programa)
            .arg(arquivo)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Não consegui abrir {}: {e}", programa.display()))?;
        // Colhe o processo quando o MuseScore fechar, sem segurar a janela.
        std::thread::spawn(move || {
            let _ = filho.wait();
        });
        Ok(())
    }
}

/// Lançador de teste: registra o pedido e devolve a resposta combinada.
#[derive(Debug)]
pub struct LancadorFalso {
    pub pedidos: Vec<PathBuf>,
    pub resposta: Result<(), String>,
}

impl LancadorFalso {
    pub fn novo() -> Self {
        Self {
            pedidos: Vec::new(),
            resposta: Ok(()),
        }
    }
}

impl Default for LancadorFalso {
    fn default() -> Self {
        Self::novo()
    }
}

impl Lancador for LancadorFalso {
    fn abrir_no_editor(&mut self, arquivo: &Path) -> Result<(), String> {
        self.pedidos.push(arquivo.to_path_buf());
        self.resposta.clone()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::fs;

    // --- R8 (revisão independente da E01): durabilidade do rename -----------------------------

    #[test]
    fn r8_sincronizar_diretorio_abre_e_sincroniza_o_diretorio_no_unix() {
        let d = tempfile::tempdir().expect("pasta");
        sincronizar_diretorio(d.path()).expect("sincroniza um diretório que existe");
        #[cfg(unix)]
        {
            let falta = d.path().join("nao-existe");
            assert!(
                sincronizar_diretorio(&falta).is_err(),
                "no Unix o diretório é de fato aberto (fsync de verdade, não um no-op)"
            );
        }
    }

    #[test]
    fn r8_gravar_atomico_troca_o_arquivo_inteiro_e_nao_deixa_temporario() {
        let d = tempfile::tempdir().expect("pasta");
        let alvo = d.path().join("indice.toml");
        gravar_atomico(&alvo, "um\n").expect("grava");
        gravar_atomico(&alvo, "dois\n").expect("regrava");
        assert_eq!(fs::read_to_string(&alvo).expect("lê"), "dois\n");
        let nomes: Vec<String> = fs::read_dir(d.path())
            .expect("lê a pasta")
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(nomes, vec!["indice.toml".to_owned()]);
    }

    const ESTADO_TOML: &str = r#"id = "cinco-notas"
aula = 14
titulo = "Cinco Notas no Piano"
estado = "hipotese"
proveniencia = "transcrição sintética de teste"
confianca = 0.95
pdf = "/algum/lugar/sintetico.pdf"
pagina_png = "paginas/cinco-notas.png"
compassos = 8
# comentário que deve sobreviver
trecho_aula = { inicio_s = 202.4, fim_s = 222.2 }   # estimado
"#;

    const MUSICXML: &str =
        include_str!("../../../assets/partituras/tradicional-brilha-brilha-estrelinha.musicxml");

    /// Oficina sintética: duas partituras (uma com PNG) e um roteiro.
    fn oficina_sintetica(dados: &Path) -> Oficina {
        let raiz = dados.join("fontes").join("tutor-teste").join("oficina");
        let pasta = raiz.join("partituras");
        fs::create_dir_all(&pasta).expect("pasta");
        fs::create_dir_all(raiz.join("paginas")).expect("paginas");
        fs::write(pasta.join("cinco-notas.estado.toml"), ESTADO_TOML).expect("estado");
        fs::write(pasta.join("cinco-notas.musicxml"), MUSICXML).expect("xml");
        fs::write(
            pasta.join("cinco-notas.divergencias.md"),
            "# Divergências\n- dúvida sintética\n",
        )
        .expect("div");
        fs::write(
            pasta.join("a-primeira.estado.toml"),
            "aula = 3\ntitulo = \"A primeira\"\nestado = \"em_revisao\"\n",
        )
        .expect("estado2");
        fs::write(
            raiz.join("roteiro.toml"),
            r#"formato_versao = 1

# comentário do roteiro
[[falas]]
id = "s03-a14-antes"
licao = "s03-a14"
momento = "antes"
texto = "Esta música usa só cinco notas."
estado = "hipotese"
origem = { aula = 14, inicio_s = 2.1, fim_s = 79.9 }

[[falas]]
id = "s03-a14-passo-3"
licao = "s03-a14"
exercicio = "cinco-notas"
momento = "passo"
passo = 3
texto = "Abra a mão."
estado = "em_revisao"
"#,
        )
        .expect("roteiro");
        Oficina {
            tutor: "tutor-teste".into(),
            raiz,
        }
    }

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn descobre_oficinas_em_fontes_e_aceita_caminho_extra_sem_repetir() {
        let d = tmp();
        let o = oficina_sintetica(d.path());
        let achadas = descobrir_oficinas(d.path(), None);
        assert_eq!(achadas, vec![o.clone()]);

        // Outra oficina em lugar qualquer, passada como extra (pela pasta do tutor).
        let fora = tmp();
        fs::create_dir_all(fora.path().join("tutor-b").join("oficina")).expect("b");
        let com_extra = descobrir_oficinas(d.path(), Some(&fora.path().join("tutor-b")));
        assert_eq!(com_extra.len(), 2);
        assert_eq!(com_extra[1].tutor, "tutor-b");
        // O extra que já foi descoberto não se repete (mesmo pelo caminho da `oficina`).
        assert_eq!(descobrir_oficinas(d.path(), Some(&o.raiz)).len(), 1);
        // Sem a pasta `fontes` e sem extra, nada.
        assert!(descobrir_oficinas(fora.path(), None).is_empty());
        // Extra que não existe é ignorado.
        assert!(descobrir_oficinas(fora.path(), Some(Path::new("/nao/existe/aqui"))).is_empty());
    }

    #[test]
    fn lista_partituras_com_estado_ordenadas_por_aula() {
        let d = tmp();
        let o = oficina_sintetica(d.path());
        let itens = o.partituras();
        assert_eq!(itens.len(), 2);
        assert_eq!(itens[0].id, "a-primeira");
        assert_eq!(itens[0].aula, Some(3));
        assert_eq!(itens[0].estado, Estado::EmRevisao);
        assert_eq!(itens[0].problema, None);

        let c = &itens[1];
        assert_eq!(c.id, "cinco-notas");
        assert_eq!(c.titulo, "Cinco Notas no Piano");
        assert_eq!(c.aula, Some(14));
        assert_eq!(c.estado, Estado::Hipotese);
        assert_eq!(c.confianca, Some(0.95));
        assert_eq!(c.proveniencia, "transcrição sintética de teste");
        assert_eq!(c.pdf.as_deref(), Some("/algum/lugar/sintetico.pdf"));
        assert_eq!(
            c.pagina_png.as_deref(),
            Some(o.raiz.join("paginas/cinco-notas.png").as_path())
        );
        assert!(c.musicxml_existe());
        assert!(c.mtime_do_musicxml().is_some());
        assert!(
            c.divergencias_em_texto()
                .expect("divergências")
                .contains("dúvida sintética")
        );
        assert!(itens[0].divergencias_em_texto().is_none());
    }

    #[test]
    fn estado_toml_ruim_vira_problema_e_nao_pode_mudar() {
        let d = tmp();
        let o = oficina_sintetica(d.path());
        let p = o.pasta_de_partituras();
        fs::write(p.join("quebrado.estado.toml"), "estado = [").expect("w");
        fs::write(p.join("esquisito.estado.toml"), "estado = \"quase\"\n").expect("w");
        fs::write(
            p.join("fuga.estado.toml"),
            "estado = \"hipotese\"\npagina_png = \"../../segredo.png\"\n",
        )
        .expect("w");
        let itens = o.partituras();
        let por_id = |id: &str| itens.iter().find(|i| i.id == id).expect(id);
        assert!(por_id("quebrado").problema.is_some());
        assert!(
            por_id("esquisito")
                .problema
                .as_deref()
                .is_some_and(|m| m.contains("quase"))
        );
        // Caminho que sai da oficina é descartado.
        assert_eq!(por_id("fuga").pagina_png, None);
        let e = por_id("esquisito").mudar_estado(Estado::EmRevisao, None, "2026-11-24");
        assert!(matches!(e, Err(ErroRevisao::Formato(_))), "{e:?}");
    }

    #[test]
    fn paginas_e_mapa_do_estado_toml_valem_e_sem_eles_cai_na_pagina_unica() {
        let d = tmp();
        let o = oficina_sintetica(d.path());
        let p = o.pasta_de_partituras();
        fs::write(
            p.join("duas.estado.toml"),
            "estado = \"hipotese\"\npaginas = [\"paginas/a-1.png\", \"paginas/a-2.png\"]\n\
             [[mapa_compassos]]\ncompasso = 1\npagina = 1\nregiao = [0.1, 0.2, 0.8, 0.1]\n\
             [[mapa_compassos]]\ncompasso = 5\npagina = 2\n\
             [[mapa_compassos]]\ncompasso = 9\npagina = 7\n\
             [[mapa_compassos]]\ncompasso = 0\npagina = 1\n\
             [[mapa_compassos]]\ncompasso = 3\npagina = 1\nregiao = [0.9, 0.9, 0.5, 0.5]\n",
        )
        .expect("w");
        // Uma lista com caminho inseguro é ignorada por inteiro (os números de página não deslocam).
        fs::write(
            p.join("insegura.estado.toml"),
            "estado = \"hipotese\"\npagina_png = \"paginas/x.png\"\npaginas = [\"paginas/a.png\", \"../fora.png\"]\n",
        )
        .expect("w");
        let itens = o.partituras();
        let por_id = |id: &str| itens.iter().find(|i| i.id == id).expect(id);

        let duas = por_id("duas");
        assert_eq!(duas.paginas.len(), 2);
        assert_eq!(duas.paginas[1], o.raiz.join("paginas/a-2.png"));
        let m = &duas.mapa;
        assert_eq!(m.paginas(), 2);
        // Compasso 9 aponta página 7 (fora), compasso 0 e região inválida são tratados sem pânico.
        assert_eq!(m.entradas().len(), 3);
        assert!(m.destino_do_compasso(0).is_some_and(|d| d.regiao.is_some()));
        assert!(
            m.destino_do_compasso(2)
                .is_some_and(|d| d.regiao.is_none() && d.por_pagina)
        );
        assert_eq!(m.destino_do_compasso(4).map(|d| d.pagina), Some(1));

        assert_eq!(
            por_id("insegura").paginas,
            vec![o.raiz.join("paginas/x.png")]
        );
        // Só `pagina_png`: uma página, sem mapa (todos os compassos vão a ela, por página).
        let cinco = por_id("cinco-notas");
        assert_eq!(cinco.paginas.len(), 1);
        assert_eq!(
            cinco.mapa.destino_do_compasso(3).map(|d| d.por_pagina),
            Some(true)
        );
    }

    #[test]
    fn transicoes_seguem_o_fluxo_e_nada_pula_para_aprovada() {
        use crate::professor_falante::EstadoFala::*;
        for (de, para, ok) in [
            (Hipotese, EmRevisao, true),
            (Hipotese, Rejeitada, true),
            (Hipotese, Aprovada, false),
            (EmRevisao, Aprovada, true),
            (EmRevisao, Rejeitada, true),
            (EmRevisao, Hipotese, false),
            (Aprovada, EmRevisao, true),
            (Rejeitada, EmRevisao, true),
            (Aprovada, Rejeitada, false),
            (Rejeitada, Aprovada, false),
            (Aprovada, Hipotese, false),
            (Hipotese, Hipotese, false),
            (Aprovada, Aprovada, false),
        ] {
            assert_eq!(transicao_valida(de, para), ok, "{de:?} -> {para:?}");
        }
    }

    #[test]
    fn aprovacao_direta_so_vale_a_partir_de_hipotese_e_nao_muda_o_fluxo() {
        use crate::professor_falante::EstadoFala::*;
        for (de, ok) in [
            (Hipotese, true),
            (EmRevisao, false),
            (Aprovada, false),
            (Rejeitada, false),
        ] {
            assert_eq!(aprovacao_direta_valida(de), ok, "{de:?}");
        }
        // O fluxo em dois passos continua o mesmo: Hipótese → Aprovada segue fora de
        // `transicao_valida` (o atalho é outro caminho, marcado no registro).
        assert!(!transicao_valida(Hipotese, Aprovada));
        assert!(transicao_valida(Hipotese, EmRevisao));
        assert!(transicao_valida(EmRevisao, Aprovada));
    }

    #[test]
    fn aprovar_direto_a_partitura_marca_o_historico_como_direto() {
        let d = tmp();
        let o = oficina_sintetica(d.path());
        let item = ItemPartitura::ler(&o, "cinco-notas");
        assert_eq!(item.estado, Estado::Hipotese);

        item.aprovar_direto("2026-11-24").expect("aprovação direta");

        let relido = ItemPartitura::ler(&o, "cinco-notas");
        assert_eq!(relido.estado, Estado::Aprovada);
        assert_eq!(
            relido.historico,
            [RegistroDeHistorico {
                de: "hipotese".into(),
                para: "aprovada".into(),
                em: "2026-11-24".into(),
                revisor: "Chico".into(),
                nota: Some("aprovação direta".into()),
                direto: true,
            }]
        );
        let texto = fs::read_to_string(&item.estado_toml).expect("ler");
        assert!(texto.contains("direto = true"), "{texto}");
        // Os campos e comentários do arquivo continuam.
        assert!(texto.contains("compassos = 8"), "{texto}");
    }

    #[test]
    fn aprovar_direto_de_outro_estado_e_recusado_sem_tocar_o_arquivo() {
        let d = tmp();
        let o = oficina_sintetica(d.path());
        // "a-primeira" já está Em revisão; "cinco-notas" fica Aprovada pelo fluxo normal.
        let em_revisao = ItemPartitura::ler(&o, "a-primeira");
        let antes = fs::read_to_string(&em_revisao.estado_toml).expect("ler");
        let e = em_revisao.aprovar_direto("2026-11-24");
        assert!(
            matches!(
                e,
                Err(ErroRevisao::TransicaoInvalida {
                    de: Estado::EmRevisao,
                    para: Estado::Aprovada
                })
            ),
            "{e:?}"
        );
        assert_eq!(
            fs::read_to_string(&em_revisao.estado_toml).expect("ler"),
            antes
        );

        let item = ItemPartitura::ler(&o, "cinco-notas");
        item.mudar_estado(Estado::EmRevisao, None, "2026-11-24")
            .expect("em revisão");
        item.mudar_estado(Estado::Aprovada, None, "2026-11-25")
            .expect("aprovada");
        assert!(item.aprovar_direto("2026-11-26").is_err(), "já Aprovada");
        // O fluxo antigo continua gravando sem `direto`.
        let relido = ItemPartitura::ler(&o, "cinco-notas");
        assert_eq!(relido.historico.len(), 2);
        assert!(relido.historico.iter().all(|r| !r.direto));
        let texto = fs::read_to_string(&item.estado_toml).expect("ler");
        assert!(!texto.contains("direto"), "{texto}");
    }

    #[test]
    fn aprovar_direto_uma_fala_do_roteiro_da_oficina_marca_aprovacao_direta() {
        let d = tmp();
        let o = oficina_sintetica(d.path());
        let fonte = FonteDeRoteiro::Oficina {
            tutor: o.tutor.clone(),
            arquivo: o.arquivo_do_roteiro(),
        };
        // Em revisão (a fala 2) não aprova direto.
        let e = fonte.aplicar("s03-a14-passo-3", &MudancaDeFala::aprovacao_direta());
        assert!(
            matches!(e, Err(ErroRevisao::TransicaoInvalida { .. })),
            "{e:?}"
        );
        // Hipótese (a fala 1) aprova direto.
        let novo = fonte
            .aplicar("s03-a14-antes", &MudancaDeFala::aprovacao_direta())
            .expect("direto");
        assert_eq!(novo, Estado::Aprovada);
        assert_eq!(
            fonte.carregar().expect("reler").falas[0].estado,
            Estado::Aprovada
        );
        let texto = fs::read_to_string(o.arquivo_do_roteiro()).expect("ler");
        assert!(texto.contains("aprovacao_direta = true"), "{texto}");
        assert!(texto.contains("# comentário do roteiro"));
        // Editar o texto devolve a Em revisão e apaga a marca (a aprovação direta era do texto
        // antigo).
        fonte
            .aplicar(
                "s03-a14-antes",
                &MudancaDeFala {
                    texto: Some("Texto novo.".into()),
                    ..MudancaDeFala::default()
                },
            )
            .expect("editar");
        let texto = fs::read_to_string(o.arquivo_do_roteiro()).expect("ler");
        assert!(!texto.contains("aprovacao_direta"), "{texto}");
    }

    #[test]
    fn aprovar_direto_uma_fala_de_fabrica_grava_na_sobreposicao_e_vale_por_cima() {
        let d = tmp();
        let fonte = FonteDeRoteiro::Fabrica {
            dados: d.path().to_path_buf(),
        };
        let id = "fabrica-ex02-acerto";
        fonte
            .aplicar(id, &MudancaDeFala::aprovacao_direta())
            .expect("direto");
        let r = roteiro_de_fabrica_revisado(d.path()).expect("revisado");
        assert_eq!(
            r.falas.iter().find(|f| f.id == id).expect("fala").estado,
            Estado::Aprovada
        );
        let t = fs::read_to_string(arquivo_de_revisao_da_fabrica(d.path())).expect("ler");
        assert!(t.contains("aprovacao_direta = true"), "{t}");
        // Já Aprovada: não aprova direto de novo; o fluxo normal (Em revisão → Aprovada) apaga
        // a marca.
        assert!(
            fonte
                .aplicar(id, &MudancaDeFala::aprovacao_direta())
                .is_err()
        );
        for para in [Estado::EmRevisao, Estado::Aprovada] {
            fonte
                .aplicar(
                    id,
                    &MudancaDeFala {
                        estado: Some(para),
                        ..MudancaDeFala::default()
                    },
                )
                .expect("fluxo normal");
        }
        let t = fs::read_to_string(arquivo_de_revisao_da_fabrica(d.path())).expect("ler");
        assert!(!t.contains("aprovacao_direta"), "{t}");
    }

    #[test]
    fn textos_do_estado_vao_e_voltam() {
        for e in ESTADOS {
            assert_eq!(estado_de_texto(estado_para_texto(e)), Some(e));
            assert!(!rotulo_do_estado(e).is_empty());
            assert!(!simbolo_do_estado(e).is_empty());
        }
        assert_eq!(estado_de_texto("Em revisao"), Some(Estado::EmRevisao));
        assert_eq!(estado_de_texto("tanto faz"), None);
    }

    #[test]
    fn muda_estado_grava_historico_e_preserva_os_outros_campos() {
        let d = tmp();
        let o = oficina_sintetica(d.path());
        let item = ItemPartitura::ler(&o, "cinco-notas");

        item.mudar_estado(Estado::EmRevisao, None, "2026-11-24")
            .expect("em revisão");
        item.mudar_estado(Estado::Aprovada, Some("conferi no MuseScore"), "2026-11-25")
            .expect("aprovada");

        let relido = ItemPartitura::ler(&o, "cinco-notas");
        assert_eq!(relido.estado, Estado::Aprovada);
        assert_eq!(relido.titulo, "Cinco Notas no Piano");
        assert_eq!(relido.aula, Some(14));
        assert_eq!(relido.confianca, Some(0.95));
        assert_eq!(relido.proveniencia, "transcrição sintética de teste");
        assert_eq!(relido.historico.len(), 2);
        assert_eq!(
            relido.historico[0],
            RegistroDeHistorico {
                de: "hipotese".into(),
                para: "em_revisao".into(),
                em: "2026-11-24".into(),
                revisor: "Chico".into(),
                nota: None,
                direto: false,
            }
        );
        assert_eq!(relido.historico[1].de, "em_revisao");
        assert_eq!(relido.historico[1].para, "aprovada");
        assert_eq!(
            relido.historico[1].nota.as_deref(),
            Some("conferi no MuseScore")
        );

        // Campos e comentários que a tela não conhece continuam no arquivo.
        let texto = fs::read_to_string(&item.estado_toml).expect("ler");
        assert!(texto.contains("compassos = 8"), "{texto}");
        assert!(
            texto.contains("# comentário que deve sobreviver"),
            "{texto}"
        );
        assert!(
            texto.contains("trecho_aula = { inicio_s = 202.4, fim_s = 222.2 }"),
            "{texto}"
        );
        assert!(texto.contains("# estimado"), "{texto}");
        assert!(texto.contains("estado = \"aprovada\""), "{texto}");
        // Nenhum temporário sobra na pasta.
        let sobras: Vec<_> = fs::read_dir(o.pasta_de_partituras())
            .expect("dir")
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(sobras.is_empty());
    }

    #[test]
    fn transicao_invalida_nao_toca_o_arquivo() {
        let d = tmp();
        let o = oficina_sintetica(d.path());
        let item = ItemPartitura::ler(&o, "cinco-notas");
        let antes = fs::read_to_string(&item.estado_toml).expect("ler");
        let e = item.mudar_estado(Estado::Aprovada, None, "2026-11-24");
        assert_eq!(
            e,
            Err(ErroRevisao::TransicaoInvalida {
                de: Estado::Hipotese,
                para: Estado::Aprovada
            })
        );
        assert_eq!(fs::read_to_string(&item.estado_toml).expect("ler"), antes);
    }

    #[test]
    fn a_transicao_vale_para_o_estado_do_arquivo_nao_o_da_tela() {
        // A tela ainda mostra Hipótese, mas por fora alguém já aprovou: não se atropela.
        let d = tmp();
        let o = oficina_sintetica(d.path());
        let velho = ItemPartitura::ler(&o, "cinco-notas");
        velho
            .mudar_estado(Estado::EmRevisao, None, "2026-11-24")
            .expect("1");
        let e = velho.mudar_estado(Estado::EmRevisao, None, "2026-11-24");
        assert!(
            matches!(e, Err(ErroRevisao::TransicaoInvalida { .. })),
            "{e:?}"
        );
    }

    #[test]
    fn rejeitar_com_nota_e_voltar_a_em_revisao() {
        let d = tmp();
        let o = oficina_sintetica(d.path());
        let item = ItemPartitura::ler(&o, "a-primeira");
        item.mudar_estado(Estado::Rejeitada, Some("  notas erradas  "), "2026-11-24")
            .expect("rejeitar");
        let r = ItemPartitura::ler(&o, "a-primeira");
        assert_eq!(r.estado, Estado::Rejeitada);
        // Arquivo sem `historico` antes: o registro cria a lista.
        assert_eq!(r.historico.len(), 1);
        assert_eq!(r.historico[0].nota.as_deref(), Some("notas erradas"));
        r.mudar_estado(Estado::EmRevisao, Some(""), "2026-11-26")
            .expect("voltar");
        let r2 = ItemPartitura::ler(&o, "a-primeira");
        assert_eq!(r2.estado, Estado::EmRevisao);
        assert_eq!(r2.historico.len(), 2);
        assert_eq!(r2.historico[1].nota, None, "nota vazia não é gravada");
    }

    #[test]
    fn historico_em_lista_inline_tambem_recebe_o_registro() {
        let d = tmp();
        let o = oficina_sintetica(d.path());
        let p = o.pasta_de_partituras().join("inline.estado.toml");
        fs::write(
            &p,
            "estado = \"hipotese\"\nhistorico = [{ de = \"x\", para = \"hipotese\", em = \"2026-01-01\" }]\n",
        )
        .expect("w");
        mudar_estado_da_partitura(&p, Estado::EmRevisao, None, "2026-11-24").expect("mudar");
        let item = ItemPartitura::ler(&o, "inline");
        assert_eq!(item.estado, Estado::EmRevisao);
        assert_eq!(item.historico.len(), 2);
    }

    // --- Roteiro ---

    #[test]
    fn roteiro_da_oficina_carrega_e_aprovar_grava_e_rele() {
        let d = tmp();
        let o = oficina_sintetica(d.path());
        let fonte = FonteDeRoteiro::Oficina {
            tutor: o.tutor.clone(),
            arquivo: o.arquivo_do_roteiro(),
        };
        let r = fonte.carregar().expect("carregar");
        assert_eq!(r.falas.len(), 2);
        assert_eq!(r.falas[0].estado, Estado::Hipotese);

        // Hipótese não vira Aprovada direto.
        let e = fonte.aplicar(
            "s03-a14-antes",
            &MudancaDeFala {
                estado: Some(Estado::Aprovada),
                texto: None,
                ..MudancaDeFala::default()
            },
        );
        assert!(
            matches!(e, Err(ErroRevisao::TransicaoInvalida { .. })),
            "{e:?}"
        );

        // Em revisão → Aprovada (a fala 2 já está em revisão).
        let novo = fonte
            .aplicar(
                "s03-a14-passo-3",
                &MudancaDeFala {
                    estado: Some(Estado::Aprovada),
                    texto: None,
                    ..MudancaDeFala::default()
                },
            )
            .expect("aprovar");
        assert_eq!(novo, Estado::Aprovada);
        let r = fonte.carregar().expect("reler");
        assert_eq!(r.falas[1].estado, Estado::Aprovada);
        assert_eq!(r.falas[1].passo, Some(3));
        // O resto permaneceu: ordem, origem, comentário.
        assert_eq!(r.falas[0].id, "s03-a14-antes");
        assert_eq!(r.falas[0].origem.map(|o| o.aula), Some(14));
        let texto = fs::read_to_string(o.arquivo_do_roteiro()).expect("ler");
        assert!(texto.contains("# comentário do roteiro"));

        // Fala inexistente.
        assert!(matches!(
            fonte.aplicar("nao-existe", &MudancaDeFala::default()),
            Err(ErroRevisao::Inexistente(_))
        ));
    }

    #[test]
    fn editar_o_texto_de_fala_aprovada_a_devolve_a_em_revisao() {
        let d = tmp();
        let o = oficina_sintetica(d.path());
        let fonte = FonteDeRoteiro::Oficina {
            tutor: o.tutor.clone(),
            arquivo: o.arquivo_do_roteiro(),
        };
        fonte
            .aplicar(
                "s03-a14-passo-3",
                &MudancaDeFala {
                    estado: Some(Estado::Aprovada),
                    texto: None,
                    ..MudancaDeFala::default()
                },
            )
            .expect("aprovar");
        let novo = fonte
            .aplicar(
                "s03-a14-passo-3",
                &MudancaDeFala {
                    estado: None,
                    texto: Some("  Abra bem a mão, com calma.  ".into()),
                    ..MudancaDeFala::default()
                },
            )
            .expect("editar");
        assert_eq!(novo, Estado::EmRevisao);
        let r = fonte.carregar().expect("reler");
        assert_eq!(r.falas[1].texto, "Abra bem a mão, com calma.");
        assert_eq!(r.falas[1].estado, Estado::EmRevisao);
        // Editar uma Hipótese mantém Hipótese; texto igual não rebaixa nada.
        let h = fonte
            .aplicar(
                "s03-a14-antes",
                &MudancaDeFala {
                    estado: None,
                    texto: Some("Texto novo.".into()),
                    ..MudancaDeFala::default()
                },
            )
            .expect("editar hipótese");
        assert_eq!(h, Estado::Hipotese);
        // Texto vazio é recusado.
        assert!(matches!(
            fonte.aplicar(
                "s03-a14-antes",
                &MudancaDeFala {
                    estado: None,
                    texto: Some("   ".into()),
                    ..MudancaDeFala::default()
                }
            ),
            Err(ErroRevisao::Formato(_))
        ));
    }

    #[test]
    fn roteiro_de_fabrica_sem_revisao_e_o_embutido_em_hipotese() {
        let d = tmp();
        let r = roteiro_de_fabrica_revisado(d.path()).expect("fábrica");
        assert!(r.falas.len() >= 40);
        assert!(r.falas.iter().all(|f| f.estado == Estado::Hipotese));
        assert!(!arquivo_de_revisao_da_fabrica(d.path()).exists());
    }

    #[test]
    fn aprovacao_da_fabrica_vai_para_o_arquivo_sobreposto_e_vale_por_cima_do_embutido() {
        let d = tmp();
        let fonte = FonteDeRoteiro::Fabrica {
            dados: d.path().to_path_buf(),
        };
        let id = "fabrica-ex01-antes";
        let embutido = crate::professor_falante::roteiro_de_fabrica().expect("embutido");
        let original = embutido
            .falas
            .iter()
            .find(|f| f.id == id)
            .expect("fala")
            .clone();

        let e = fonte.aplicar(
            id,
            &MudancaDeFala {
                estado: Some(Estado::Aprovada),
                texto: None,
                ..MudancaDeFala::default()
            },
        );
        assert!(
            matches!(e, Err(ErroRevisao::TransicaoInvalida { .. })),
            "{e:?}"
        );
        fonte
            .aplicar(
                id,
                &MudancaDeFala {
                    estado: Some(Estado::EmRevisao),
                    texto: None,
                    ..MudancaDeFala::default()
                },
            )
            .expect("em revisão");
        fonte
            .aplicar(
                id,
                &MudancaDeFala {
                    estado: Some(Estado::Aprovada),
                    texto: Some("Vamos achar o Dó central, juntos.".into()),
                    ..MudancaDeFala::default()
                },
            )
            .expect("aprovar com texto ajustado");

        let r = roteiro_de_fabrica_revisado(d.path()).expect("revisado");
        let f = r.falas.iter().find(|f| f.id == id).expect("fala");
        assert_eq!(f.estado, Estado::Aprovada);
        assert_eq!(f.texto, "Vamos achar o Dó central, juntos.");
        assert_eq!(f.licao, original.licao);
        assert_eq!(f.exercicio, original.exercicio);
        // As outras continuam em Hipótese e o embutido não mudou (nem a ordem).
        assert_eq!(r.falas.len(), embutido.falas.len());
        assert_eq!(contar_por_estado(&r.falas), [r.falas.len() - 1, 0, 1, 0]);
        for (a, b) in r.falas.iter().zip(&embutido.falas) {
            assert_eq!(a.id, b.id);
        }
        // O arquivo da sobreposição guarda o que foi aprovado.
        let t = fs::read_to_string(arquivo_de_revisao_da_fabrica(d.path())).expect("ler");
        assert!(t.contains("formato_versao = 1"), "{t}");
        assert!(t.contains("aprovada"), "{t}");
        assert!(t.contains("Vamos achar o Dó central, juntos."), "{t}");
        // A fonte da tela enxerga o mesmo.
        let via_fonte = fonte.carregar().expect("carregar");
        assert_eq!(via_fonte, r);
        // E a fala aprovada passa a ser dita; as outras não.
        let dizer = crate::professor_falante::falas_para_dizer(
            &r,
            crate::professor_falante::LICAO_DE_FABRICA,
            original.exercicio.as_deref(),
            original.momento,
            None,
        );
        assert_eq!(dizer.len(), 1);
        assert_eq!(dizer[0].id, id);
    }

    #[test]
    fn sobreposicao_ignora_ids_desconhecidos_e_recusa_toml_quebrado() {
        let mut r = Roteiro::de_toml(ROTEIRO_DE_FABRICA_TOML).expect("embutido");
        let n = aplicar_sobreposicao(
            &mut r,
            "formato_versao = 1\n[[falas]]\nid = \"nao-existe\"\nestado = \"aprovada\"\n",
        )
        .expect("aplicar");
        assert_eq!(n, 0);
        assert!(r.falas.iter().all(|f| f.estado == Estado::Hipotese));
        assert!(aplicar_sobreposicao(&mut r, "falas = [").is_err());
        assert!(
            aplicar_sobreposicao(
                &mut r,
                "[[falas]]\nid = \"fabrica-ex01-antes\"\nestado = \"feito\"\n"
            )
            .is_err()
        );
    }

    #[test]
    fn fontes_de_roteiro_listam_fabrica_e_oficinas_com_roteiro() {
        let d = tmp();
        let o = oficina_sintetica(d.path());
        let fontes = FonteDeRoteiro::listar(Some(d.path()), std::slice::from_ref(&o));
        assert_eq!(fontes.len(), 2);
        assert!(matches!(fontes[0], FonteDeRoteiro::Fabrica { .. }));
        assert!(matches!(fontes[1], FonteDeRoteiro::Oficina { .. }));
        assert!(FonteDeRoteiro::listar(None, &[]).is_empty());
    }

    // --- PNG ---

    fn gerar_png(
        largura: u32,
        altura: u32,
        cor: png::ColorType,
        profundidade: png::BitDepth,
        dados: &[u8],
    ) -> Vec<u8> {
        let mut saida = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut saida, largura, altura);
            enc.set_color(cor);
            enc.set_depth(profundidade);
            let mut w = enc.write_header().expect("cabeçalho");
            w.write_image_data(dados).expect("dados");
        }
        saida
    }

    #[test]
    fn decodifica_png_rgb_rgba_cinza_e_16_bits() {
        use png::{BitDepth, ColorType};
        // RGB 2x1: vermelho e verde.
        let rgb = gerar_png(
            2,
            1,
            ColorType::Rgb,
            BitDepth::Eight,
            &[255, 0, 0, 0, 255, 0],
        );
        let i = decodificar_png(&rgb).expect("rgb");
        assert_eq!((i.largura, i.altura), (2, 1));
        assert_eq!(i.rgba, vec![255, 0, 0, 255, 0, 255, 0, 255]);

        // RGBA 1x1 semitransparente.
        let rgba = gerar_png(1, 1, ColorType::Rgba, BitDepth::Eight, &[10, 20, 30, 128]);
        assert_eq!(
            decodificar_png(&rgba).expect("rgba").rgba,
            vec![10, 20, 30, 128]
        );

        // Cinza 2x2.
        let cinza = gerar_png(
            2,
            2,
            ColorType::Grayscale,
            BitDepth::Eight,
            &[0, 50, 100, 255],
        );
        let i = decodificar_png(&cinza).expect("cinza");
        assert_eq!(i.rgba.len(), 16);
        assert_eq!(&i.rgba[4..8], &[50, 50, 50, 255]);

        // Cinza de 16 bits vira 8 bits.
        let c16 = gerar_png(1, 1, ColorType::Grayscale, BitDepth::Sixteen, &[0x80, 0x00]);
        let i = decodificar_png(&c16).expect("16 bits");
        assert_eq!(i.rgba, vec![0x80, 0x80, 0x80, 255]);

        // Indexado (paleta) é expandido.
        let mut saida = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut saida, 2, 1);
            enc.set_color(ColorType::Indexed);
            enc.set_depth(BitDepth::Eight);
            enc.set_palette(vec![255, 0, 0, 0, 0, 255]);
            let mut w = enc.write_header().expect("cabeçalho");
            w.write_image_data(&[1, 0]).expect("dados");
        }
        let i = decodificar_png(&saida).expect("indexado");
        assert_eq!(i.rgba, vec![0, 0, 255, 255, 255, 0, 0, 255]);
    }

    #[test]
    fn png_invalido_ou_enorme_e_recusado_sem_panico() {
        assert!(decodificar_png(b"isto nao e png").is_err());
        assert!(decodificar_png(&[]).is_err());
        let ok = gerar_png(3, 3, png::ColorType::Rgb, png::BitDepth::Eight, &[7; 27]);
        assert!(decodificar_png(&ok[..ok.len() / 2]).is_err(), "truncado");
        let d = tmp();
        assert!(ler_png(&d.path().join("nao-existe.png")).is_err());
        fs::write(d.path().join("p.png"), &ok).expect("w");
        assert_eq!(ler_png(&d.path().join("p.png")).expect("ler").largura, 3);
    }

    // --- MuseScore ---

    #[test]
    fn procura_musescore_nas_pastas_dadas_e_diz_quando_nao_ha() {
        let d = tmp();
        assert_eq!(procurar_musescore(&[d.path().to_path_buf()]), None);
        let falso = d.path().join("mscore");
        fs::write(&falso, "#!/bin/sh\nexit 0\n").expect("w");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut p = fs::metadata(&falso).expect("meta").permissions();
            p.set_mode(0o755);
            fs::set_permissions(&falso, p).expect("chmod");
            assert_eq!(procurar_musescore(&[d.path().to_path_buf()]), Some(falso));
        }
        assert!(SEM_MUSESCORE.contains("MuseScore"));
    }

    #[test]
    fn lancador_falso_registra_o_arquivo_e_devolve_o_erro_combinado() {
        let mut l = LancadorFalso::novo();
        assert!(l.abrir_no_editor(Path::new("/x/a.musicxml")).is_ok());
        l.resposta = Err("sem mscore".into());
        assert_eq!(
            l.abrir_no_editor(Path::new("/x/b.musicxml")),
            Err("sem mscore".to_owned())
        );
        assert_eq!(l.pedidos.len(), 2);
    }
}
