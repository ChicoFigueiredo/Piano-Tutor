//! Versões de uma partitura da Oficina (ticket 21, A8): estado salvo e imutável, identificado
//! pelo **SHA-256 do conteúdo**, encadeado à Versão-pai, com autor, data, nota de motivo e estado.
//!
//! Estado puro, sem janela nem egui (seam S1): quem chama (a tela) só entrega o MusicXML
//! exportado pelo serviço de documento e lê a lista. A Hipótese (`<id>.musicxml`) e o arquivo de
//! origem (o PDF) **nunca** são escritos aqui; só lidos, para registrar e conferir o hash.
//!
//! # Formato de armazenamento (documentado para a migração ao Acervo, E02)
//!
//! Na E01 as Versões vivem no repositório da Oficina atual, ao lado da partitura, em uma pasta
//! própria. O `estado.toml` da partitura **não é tocado** (o ciclo Hipótese → Em revisão →
//! Aprovada continua nele; o ticket 23 liga a aprovação ao hash). O desenho é endereçado por
//! conteúdo, para a E02 mover tudo sem reescrever nada:
//!
//! ```text
//! <oficina>/partituras/<id>.versoes/
//!     indice.toml                  índice: origem + lista encadeada (uma `[[versao]]` por Versão)
//!     objetos/<sha256>.musicxml    conteúdo imutável de cada Versão, nome = hash do conteúdo
//! ```
//!
//! `indice.toml`:
//!
//! ```toml
//! formato = 1
//! partitura = "<id>"
//! [origem]                          # hashes dos originais no instante do registro
//! hipotese = "<sha256 do <id>.musicxml>"
//! pdf = "<sha256 do PDF>"           # opcional (só se o PDF existia e foi legível)
//! [[versao]]                        # em ordem de criação; a primeira é a Hipótese
//! hash = "<sha256>"                 # identidade = hash do conteúdo (64 hex minúsculos)
//! pai = "<sha256>"                  # ausente na Hipótese; ramificar é permitido (vários filhos)
//! tipo = "hipotese"                 # ou "correcao"
//! autor = "Chico"
//! em = "2026-11-27T10:00:00Z"
//! nota = "motivo"                   # opcional
//! estado = "em_revisao"             # mesma máquina de `revisao` (a Hipótese nasce `hipotese`)
//! bytes = 1234
//! ```
//!
//! Cada objeto é criado uma única vez (nunca sobrescrito); o índice é regravado por arquivo
//! temporário e `rename` ([`gravar_atomico`]). O objeto é gravado **antes** do índice: uma queda
//! no meio deixa, no pior caso, um objeto órfão (inofensivo) e o índice anterior íntegro.
//! A migração ao Acervo é copiar `objetos/` e importar as linhas `[[versao]]`; campos
//! desconhecidos do índice são preservados ao acrescentar uma Versão.
//!
//! Decisões registradas:
//! - Salvar sem mudança (conteúdo igual ao da Versão-pai) **não cria Versão**: devolve
//!   [`ResultadoDoSalvar::SemMudancas`]. Conteúdo igual ao de qualquer outra Versão devolve
//!   [`ResultadoDoSalvar::JaExiste`]: o hash é a identidade, então não há Versão duplicada.
//! - Abrir uma Versão antiga é **somente leitura** ([`VersaoAberta`]); para continuar a partir
//!   dela, salve passando o hash dela como `pai` (ramifica a cadeia).
//! - Material comprado nunca sai da pasta de dados do usuário: a pasta de Versões é derivada do
//!   `estado.toml` da própria partitura da Oficina, e o PDF é só lido para o hash (nunca copiado).
//! - A aprovação por hash (ticket 23) e o diff (ticket 22) usam `hash` e `pai` deste índice.
//!
//! # Aprovar, rejeitar e reabrir por hash (ticket 23, A9)
//!
//! A aprovação é **atributo da Versão (hash)**, nunca do arquivo em edição nem do nome dele.
//! [`Versoes::aprovar`], [`Versoes::rejeitar`], [`Versoes::reabrir`] e [`Versoes::aprovar_direto`]
//! só trocam o `estado` da `[[versao]]` no índice e acrescentam um registro `[[decisao]]`
//! (`hash`, `de`, `para`, `revisor`, `em`, `motivo`, `direto`); o **objeto nunca é tocado**, nem a
//! Hipótese. As duas mudanças saem na mesma gravação atômica do índice.
//!
//! ```toml
//! [[decisao]]                       # registro append-only, em ordem
//! hash = "<sha256 da Versão decidida>"
//! de = "em_revisao"
//! para = "aprovada"
//! revisor = "Chico"
//! em = "2026-11-28T09:30:00Z"
//! motivo = "conferi com o PDF"      # obrigatório, até `MAX_MOTIVO` caracteres
//! direto = true                     # só na aprovação direta (Hipótese → Aprovada)
//! ```
//!
//! Decisões registradas (ticket 23):
//! - As transições são **só** as de [`transicao_valida`] (a máquina de `revisao`): Aprovar =
//!   Em revisão → Aprovada; Rejeitar = Hipótese ou Em revisão → Rejeitada; Reabrir =
//!   Hipótese, Aprovada ou Rejeitada → Em revisão (de Hipótese é "iniciar a revisão"). Fora
//!   disso, [`ErroVersoes::TransicaoInvalida`], sem registro e sem mudar nada.
//! - A **aprovação direta** vale só a partir de Hipótese, fica marcada (`direto = true`) e, sem
//!   motivo escrito, usa a nota padrão "aprovação direta" (um clique). Não altera a máquina.
//! - Aprovar/rejeitar/reabrir **exigem motivo** (não vazio, até [`MAX_MOTIVO`]); um motivo
//!   longo é recusado, nunca truncado em silêncio. O revisor também é obrigatório.
//! - Uma Versão filha de uma aprovada nasce **Em revisão** (já era a regra do `salvar`): a
//!   aprovação não é herdada. Salvar conteúdo igual ao de uma Versão existente não cria Versão
//!   (então não "re-aprova" nem "des-aprova" ninguém).
//! - Consumidores (prática, Exercício, Gabarito) perguntam por **hash**: [`Versoes::aprovacao_de`],
//!   [`Versoes::aprovacao_do_conteudo`] e [`Versoes::aprovacao_do_arquivo`] (que hasheia o
//!   conteúdo lido: alterar o arquivo muda o hash e a aprovação não vale). `estado = "aprovada"`
//!   escrito à mão no índice, **sem** registro `[[decisao]]` correspondente, não vale: só decisão
//!   registrada de um revisor aprova (nada aprova sozinho, nem IA nem plugin).
//!
//! # Trava entre processos
//!
//! Toda mutação do índice (`registrar_origem`, `salvar`, decisões) é um ciclo ler-modificar-gravar
//! e, sem trava, dois processos (ou duas janelas) perderiam a atualização um do outro. Agora o
//! ciclo roda sob uma trava **exclusiva de arquivo** (`indice.lock`, `File::lock` da biblioteca
//! padrão: `flock` no Unix e `LockFileEx` no Windows; sem crate nova). A leitura não trava (o
//! índice é trocado por `rename` atômico). A trava é liberada ao fim do ciclo (ou se o processo
//! cair). Em sistema de arquivos que não suporta trava (`Unsupported`), segue sem ela.

use crate::revisao::{
    Estado, ItemPartitura, NOTA_DA_APROVACAO_DIRETA, aprovacao_direta_valida, estado_de_texto,
    estado_para_texto, gravar_atomico, rotulo_do_estado, transicao_valida,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

/// Versão do formato do índice.
pub const FORMATO_DO_INDICE: i64 = 1;
/// Nome do arquivo de índice dentro da pasta de Versões.
pub const ARQUIVO_DO_INDICE: &str = piano_tutor_curso::ARQUIVO_DO_INDICE;
/// Subpasta dos objetos (conteúdo por hash).
pub const PASTA_DOS_OBJETOS: &str = "objetos";
/// Tamanho máximo da nota de motivo, em caracteres.
pub const MAX_NOTA: usize = 500;
/// Tamanho máximo do motivo de uma decisão (aprovar, rejeitar, reabrir), em caracteres.
pub const MAX_MOTIVO: usize = 280;
/// Arquivo de trava (exclusiva, entre processos) ao lado do índice.
pub const ARQUIVO_DA_TRAVA: &str = "indice.lock";
/// Tamanho máximo de um conteúdo de Versão.
pub const MAX_BYTES_DA_VERSAO: usize = 64 * 1024 * 1024;

/// Hash SHA-256 em hexadecimal minúsculo (64 caracteres).
pub type Hash = String;

fn em_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 do conteúdo.
pub fn hash_do_conteudo(bytes: &[u8]) -> Hash {
    em_hex(&Sha256::digest(bytes))
}

/// `h` tem a forma de um hash (64 hex minúsculos)? Usado antes de montar qualquer caminho.
pub fn hash_valido(h: &str) -> bool {
    h.len() == 64 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Os 8 primeiros caracteres do hash, para a tela.
pub fn hash_curto(h: &str) -> &str {
    h.get(..8).unwrap_or(h)
}

/// Agora em UTC, `AAAA-MM-DDTHH:MM:SSZ`.
pub fn agora_utc() -> String {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let dias = i64::try_from(s / 86_400).unwrap_or(0);
    let resto = s % 86_400;
    format!(
        "{}T{:02}:{:02}:{:02}Z",
        crate::app::data_civil(dias),
        resto / 3_600,
        resto % 3_600 / 60,
        resto % 60
    )
}

/// Falha ao salvar, listar ou abrir Versões.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ErroVersoes {
    Disco(String),
    /// Índice ilegível ou com valor inesperado.
    Formato(String),
    /// A Versão-pai pedida não está no índice.
    PaiInexistente(String),
    /// A Versão pedida não está no índice.
    Inexistente(String),
    /// O arquivo da Versão não confere com o hash do índice (adulterado ou truncado).
    Corrompida(Hash),
    /// A Hipótese (`<id>.musicxml`) não existe: não há de onde começar a cadeia.
    SemHipotese,
    /// Conteúdo vazio ou grande demais.
    ConteudoInvalido(String),
    /// A máquina de estados não permite `de` → `para`.
    TransicaoInvalida {
        de: Estado,
        para: Estado,
    },
    /// A aprovação direta só vale a partir de Hipótese (este é o estado atual da Versão).
    AprovacaoDiretaInvalida(Estado),
    /// Aprovar, rejeitar e reabrir exigem um motivo.
    MotivoObrigatorio,
    /// O motivo passou do máximo (em caracteres).
    MotivoLongo(usize),
    /// A decisão precisa de revisor.
    RevisorObrigatorio,
}

impl fmt::Display for ErroVersoes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disco(e) => write!(f, "problema no disco: {e}"),
            Self::Formato(e) => write!(f, "índice de Versões inválido: {e}"),
            Self::PaiInexistente(h) => write!(f, "a Versão-pai {} não existe", hash_curto(h)),
            Self::Inexistente(h) => write!(f, "a Versão {} não existe", hash_curto(h)),
            Self::Corrompida(h) => write!(
                f,
                "o arquivo da Versão {} não confere com o hash registrado",
                hash_curto(h)
            ),
            Self::SemHipotese => write!(f, "a Hipótese (MusicXML) da partitura não foi encontrada"),
            Self::ConteudoInvalido(e) => write!(f, "conteúdo inválido: {e}"),
            Self::TransicaoInvalida { de, para } => write!(
                f,
                "não dá para ir de {} para {}: o fluxo é Hipótese → Em revisão → Aprovada ou \
                 Rejeitada, e Aprovada ou Rejeitada voltam por Em revisão",
                rotulo_do_estado(*de),
                rotulo_do_estado(*para)
            ),
            Self::AprovacaoDiretaInvalida(de) => write!(
                f,
                "a aprovação direta só vale a partir de Hipótese; esta Versão está {}",
                rotulo_do_estado(*de)
            ),
            Self::MotivoObrigatorio => write!(f, "escreva o motivo da decisão"),
            Self::MotivoLongo(max) => write!(f, "o motivo passou de {max} caracteres"),
            Self::RevisorObrigatorio => write!(f, "falta o nome do revisor"),
        }
    }
}

impl std::error::Error for ErroVersoes {}

fn disco(caminho: &Path, e: &std::io::Error) -> ErroVersoes {
    ErroVersoes::Disco(format!("{}: {e}", caminho.display()))
}

/// O que a Versão é na cadeia.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TipoDeVersao {
    /// A transcrição original (Hipótese), raiz da cadeia.
    Hipotese,
    /// Uma correção salva pelo revisor.
    Correcao,
}

impl TipoDeVersao {
    const fn texto(self) -> &'static str {
        match self {
            Self::Hipotese => "hipotese",
            Self::Correcao => "correcao",
        }
    }
}

/// Uma Versão do índice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Versao {
    pub hash: Hash,
    /// Hash da Versão-pai; `None` só na Hipótese.
    pub pai: Option<Hash>,
    pub tipo: TipoDeVersao,
    pub autor: String,
    pub em: String,
    pub nota: Option<String>,
    pub estado: Estado,
    pub bytes: u64,
}

/// Hashes dos originais no instante do registro.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Origem {
    pub hipotese: Hash,
    pub pdf: Option<Hash>,
}

/// O que o revisor faz com uma Versão (cada ação leva a um estado-alvo).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcaoDeRevisao {
    Aprovar,
    Rejeitar,
    /// Volta a Em revisão (de Aprovada ou Rejeitada; de Hipótese é iniciar a revisão).
    Reabrir,
}

impl AcaoDeRevisao {
    /// O estado a que a ação leva.
    pub const fn alvo(self) -> Estado {
        match self {
            Self::Aprovar => Estado::Aprovada,
            Self::Rejeitar => Estado::Rejeitada,
            Self::Reabrir => Estado::EmRevisao,
        }
    }
}

/// Um registro de decisão do revisor sobre uma Versão (hash).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decisao {
    pub hash: Hash,
    pub de: Estado,
    pub para: Estado,
    pub revisor: String,
    pub em: String,
    pub motivo: String,
    /// `true` na aprovação direta (Hipótese → Aprovada num clique).
    pub direto: bool,
}

impl Decisao {
    /// Esta decisão vista como aprovação (vale se `para` é Aprovada).
    pub fn aprovacao(&self) -> Aprovacao {
        Aprovacao {
            hash: self.hash.clone(),
            revisor: self.revisor.clone(),
            em: self.em.clone(),
            motivo: self.motivo.clone(),
            direto: self.direto,
        }
    }
}

/// A aprovação vigente de uma Versão: quem, quando, por quê e se foi direta.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Aprovacao {
    pub hash: Hash,
    pub revisor: String,
    pub em: String,
    pub motivo: String,
    pub direto: bool,
}

/// Resultado de [`Versoes::salvar`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResultadoDoSalvar {
    /// Nova Versão criada (nasce Em revisão).
    Criada(Versao),
    /// O conteúdo é idêntico ao da Versão-pai: nada foi criado.
    SemMudancas { hash: Hash },
    /// O conteúdo já é o de outra Versão da cadeia: nada foi criado.
    JaExiste { hash: Hash },
}

/// Versão aberta para leitura (somente leitura: o conteúdo é conferido contra o hash).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersaoAberta {
    pub versao: Versao,
    pub conteudo: String,
}

/// Situação de um original diante do hash registrado.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SituacaoDoOriginal {
    Integro,
    /// O arquivo existe, mas o hash mudou.
    Alterado {
        esperado: Hash,
        atual: Hash,
    },
    /// O arquivo sumiu ou não pôde ser lido.
    Ausente,
    /// Ainda não há hash registrado (nenhum registro de origem).
    NaoRegistrado,
}

impl SituacaoDoOriginal {
    pub const fn integro(&self) -> bool {
        matches!(self, Self::Integro)
    }
}

/// Conferência dos originais (A8: verificada após qualquer ação da Oficina).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelatorioDeOriginais {
    pub hipotese: SituacaoDoOriginal,
    /// `None` quando a partitura não tem PDF registrado.
    pub pdf: Option<SituacaoDoOriginal>,
}

impl RelatorioDeOriginais {
    /// Todos os originais conferem?
    pub fn integro(&self) -> bool {
        self.hipotese.integro() && self.pdf.as_ref().is_none_or(SituacaoDoOriginal::integro)
    }
}

/// Uma linha da lista de Versões para a tela.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinhaDaLista {
    /// "Fonte (PDF)", "Hipótese" ou "Versão N".
    pub rotulo: String,
    pub hash: Hash,
    pub pai: Option<Hash>,
    pub autor: String,
    pub em: String,
    pub nota: Option<String>,
    /// `None` na fonte (o PDF não tem estado de conteúdo).
    pub estado: Option<Estado>,
    /// Pode ser aberta como Versão (a fonte PDF não).
    pub abrivel: bool,
}

impl LinhaDaLista {
    /// Texto de uma linha: rótulo, hash curto, autor, data, estado e nota.
    pub fn em_texto(&self) -> String {
        let estado = self.estado.map_or("-", rotulo_do_estado);
        let autor = if self.autor.is_empty() {
            "-"
        } else {
            &self.autor
        };
        let em = if self.em.is_empty() { "-" } else { &self.em };
        let mut t = format!(
            "{} [{}] {autor} {em} {estado}",
            self.rotulo,
            hash_curto(&self.hash)
        );
        if let Some(n) = &self.nota {
            t.push_str(" - ");
            t.push_str(n);
        }
        t
    }
}

// --- Leitura do índice ----------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
struct IndiceToml {
    #[serde(default)]
    formato: Option<i64>,
    #[serde(default)]
    origem: Option<OrigemToml>,
    #[serde(default)]
    versao: Vec<VersaoToml>,
    #[serde(default)]
    decisao: Vec<DecisaoToml>,
}

#[derive(Debug, Deserialize)]
struct DecisaoToml {
    hash: String,
    de: String,
    para: String,
    revisor: String,
    #[serde(default)]
    em: String,
    motivo: String,
    #[serde(default)]
    direto: bool,
}

#[derive(Debug, Deserialize)]
struct OrigemToml {
    hipotese: String,
    #[serde(default)]
    pdf: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VersaoToml {
    hash: String,
    #[serde(default)]
    pai: Option<String>,
    tipo: String,
    #[serde(default)]
    autor: String,
    #[serde(default)]
    em: String,
    #[serde(default)]
    nota: Option<String>,
    estado: String,
    #[serde(default)]
    bytes: u64,
}

/// O índice lido e validado.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Indice {
    pub origem: Origem,
    /// Em ordem de criação; a primeira é a Hipótese.
    pub versoes: Vec<Versao>,
    /// Decisões do revisor, em ordem (registro append-only).
    pub decisoes: Vec<Decisao>,
}

impl Indice {
    fn versao(&self, hash: &str) -> Option<&Versao> {
        self.versoes.iter().find(|v| v.hash == hash)
    }

    /// A aprovação vigente do `hash`: a Versão está Aprovada **e** a última decisão sobre ela é
    /// uma aprovação registrada. Estado escrito à mão, sem registro, não vale.
    pub fn aprovacao(&self, hash: &str) -> Option<Aprovacao> {
        let v = self.versao(hash)?;
        // Critério único, no crate do curso (o importador e a CLI usam a mesma função).
        piano_tutor_curso::decisao_que_aprova(
            estado_para_texto(v.estado),
            hash,
            &self.decisoes,
            |d| (d.hash.as_str(), estado_para_texto(d.para)),
        )
        .map(Decisao::aprovacao)
    }
}

fn converter(t: IndiceToml) -> Result<Indice, ErroVersoes> {
    if t.formato != Some(FORMATO_DO_INDICE) {
        return Err(ErroVersoes::Formato(format!(
            "formato {:?} não suportado (esperado {FORMATO_DO_INDICE})",
            t.formato
        )));
    }
    let o = t
        .origem
        .ok_or_else(|| ErroVersoes::Formato("falta [origem]".into()))?;
    if !hash_valido(&o.hipotese) || o.pdf.as_deref().is_some_and(|p| !hash_valido(p)) {
        return Err(ErroVersoes::Formato("hash inválido em [origem]".into()));
    }
    let mut versoes: Vec<Versao> = Vec::with_capacity(t.versao.len());
    for v in t.versao {
        let tipo = match v.tipo.as_str() {
            "hipotese" => TipoDeVersao::Hipotese,
            "correcao" => TipoDeVersao::Correcao,
            outro => {
                return Err(ErroVersoes::Formato(format!(
                    "tipo desconhecido: \"{outro}\""
                )));
            }
        };
        let estado = estado_de_texto(&v.estado).ok_or_else(|| {
            ErroVersoes::Formato(format!("estado desconhecido: \"{}\"", v.estado))
        })?;
        if !hash_valido(&v.hash) || v.pai.as_deref().is_some_and(|p| !hash_valido(p)) {
            return Err(ErroVersoes::Formato("hash inválido numa [[versao]]".into()));
        }
        if versoes.iter().any(|x| x.hash == v.hash) {
            return Err(ErroVersoes::Formato(format!(
                "Versão {} repetida",
                hash_curto(&v.hash)
            )));
        }
        match (&v.pai, tipo) {
            (Some(pai), _) if !versoes.iter().any(|x| &x.hash == pai) => {
                return Err(ErroVersoes::Formato(format!(
                    "o pai {} de {} não aparece antes dela",
                    hash_curto(pai),
                    hash_curto(&v.hash)
                )));
            }
            (None, TipoDeVersao::Correcao) => {
                return Err(ErroVersoes::Formato("correção sem pai".into()));
            }
            _ => {}
        }
        versoes.push(Versao {
            hash: v.hash,
            pai: v.pai,
            tipo,
            autor: v.autor,
            em: v.em,
            nota: v.nota,
            estado,
            bytes: v.bytes,
        });
    }
    let mut decisoes = Vec::with_capacity(t.decisao.len());
    for d in t.decisao {
        if !hash_valido(&d.hash) || !versoes.iter().any(|v| v.hash == d.hash) {
            return Err(ErroVersoes::Formato(format!(
                "decisão sobre a Versão {}, que não existe",
                hash_curto(&d.hash)
            )));
        }
        let estado = |t: &str| {
            estado_de_texto(t)
                .ok_or_else(|| ErroVersoes::Formato(format!("estado desconhecido: \"{t}\"")))
        };
        decisoes.push(Decisao {
            de: estado(&d.de)?,
            para: estado(&d.para)?,
            hash: d.hash,
            revisor: d.revisor,
            em: d.em,
            motivo: d.motivo,
            direto: d.direto,
        });
    }
    Ok(Indice {
        origem: Origem {
            hipotese: o.hipotese,
            pdf: o.pdf,
        },
        versoes,
        decisoes,
    })
}

/// Motivo limpo: controles viram espaço, extremos aparados; vazio e longo são recusados.
fn limpar_motivo(motivo: Option<&str>) -> Result<String, ErroVersoes> {
    let m: String = motivo
        .unwrap_or_default()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let m = m.trim();
    if m.is_empty() {
        return Err(ErroVersoes::MotivoObrigatorio);
    }
    if m.chars().count() > MAX_MOTIVO {
        return Err(ErroVersoes::MotivoLongo(MAX_MOTIVO));
    }
    Ok(m.to_owned())
}

fn limpar_nota(nota: Option<&str>) -> Option<String> {
    let n: String = nota?
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_NOTA)
        .collect();
    let n = n.trim();
    (!n.is_empty()).then(|| n.to_owned())
}

/// Caminho relativo e seguro dentro de `raiz` (sem `..`, sem raiz absoluta).
fn dentro_da_oficina(raiz: &Path, relativo: &str) -> Option<PathBuf> {
    let r = Path::new(relativo);
    let seguro = !relativo.trim().is_empty()
        && r.components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    seguro.then(|| raiz.join(r))
}

// --- Repositório de Versões -----------------------------------------------------------------

/// Onde interromper a gravação, para os testes de queda (nunca usado em produção).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Queda {
    Nenhuma,
    /// Depois de gravar o objeto e antes de tocar no índice.
    AposObjeto,
    /// No meio da gravação do índice: o temporário fica pela metade e o rename não acontece.
    NoMeioDoIndice,
}

/// As Versões de uma partitura da Oficina.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Versoes {
    id: String,
    pasta: PathBuf,
    hipotese: PathBuf,
    pdf: Option<PathBuf>,
}

impl Versoes {
    /// As Versões da partitura `item` (a pasta nasce ao lado do `estado.toml`). O PDF de origem,
    /// se declarado no `estado.toml`, é resolvido dentro da oficina (caminho relativo seguro) ou
    /// como caminho absoluto; só é lido para o hash.
    pub fn da_partitura(item: &ItemPartitura) -> Self {
        let partituras = item
            .estado_toml
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf);
        let raiz = partituras
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf);
        let pdf = item.pdf.as_deref().and_then(|p| {
            if Path::new(p).is_absolute() {
                Some(PathBuf::from(p))
            } else {
                dentro_da_oficina(&raiz, p)
            }
        });
        Self::em(&partituras, &item.id, pdf)
    }

    /// As Versões da partitura `id` que mora em `pasta_partituras` (`<id>.musicxml`).
    pub fn em(pasta_partituras: &Path, id: &str, pdf: Option<PathBuf>) -> Self {
        Self {
            id: id.to_owned(),
            pasta: piano_tutor_curso::pasta_das_versoes(pasta_partituras, id),
            hipotese: pasta_partituras.join(format!("{id}.musicxml")),
            pdf,
        }
    }

    /// Pasta do repositório (`<id>.versoes`).
    pub fn pasta(&self) -> &Path {
        &self.pasta
    }

    fn arquivo_do_indice(&self) -> PathBuf {
        self.pasta.join(ARQUIVO_DO_INDICE)
    }

    fn arquivo_do_objeto(&self, hash: &str) -> PathBuf {
        self.pasta
            .join(PASTA_DOS_OBJETOS)
            .join(format!("{hash}.musicxml"))
    }

    /// Lê o índice; `None` se ainda não existe (nenhuma Versão registrada).
    pub fn indice(&self) -> Result<Option<Indice>, ErroVersoes> {
        let caminho = self.arquivo_do_indice();
        let texto = match fs::read_to_string(&caminho) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(disco(&caminho, &e)),
        };
        let t: IndiceToml = toml::from_str(&texto)
            .map_err(|e| ErroVersoes::Formato(format!("{}: {e}", caminho.display())))?;
        converter(t).map(Some)
    }

    /// As Versões em ordem de criação (a Hipótese primeiro); vazio sem registro.
    pub fn listar(&self) -> Result<Vec<Versao>, ErroVersoes> {
        Ok(self.indice()?.map(|i| i.versoes).unwrap_or_default())
    }

    /// A última Versão criada (a ponta da cadeia); `None` sem registro.
    pub fn ponta(&self) -> Result<Option<Versao>, ErroVersoes> {
        Ok(self.listar()?.pop())
    }

    /// Registra a origem (a Hipótese como primeira Versão e os hashes do MusicXML e do PDF).
    /// Idempotente: se o índice já existe, não muda nada. O `salvar` chama isto sozinho, mas a
    /// tela pode chamá-lo ao abrir a partitura, para fixar os hashes cedo.
    pub fn registrar_origem(&self, autor: &str, em: &str) -> Result<Indice, ErroVersoes> {
        self.com_trava(|| self.registrar_origem_com(autor, em, Queda::Nenhuma))
    }

    /// Roda `f` com a trava exclusiva do índice (entre processos); veja o módulo. Não reentrante:
    /// `f` não pode chamar outro método público que trave.
    fn com_trava<T>(&self, f: impl FnOnce() -> Result<T, ErroVersoes>) -> Result<T, ErroVersoes> {
        fs::create_dir_all(&self.pasta).map_err(|e| disco(&self.pasta, &e))?;
        let caminho = self.pasta.join(ARQUIVO_DA_TRAVA);
        let arquivo = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&caminho)
            .map_err(|e| disco(&caminho, &e))?;
        // Sistema de arquivos sem trava (`Unsupported`): segue como antes (atômico, sem exclusão).
        if let Err(e) = arquivo.lock()
            && e.kind() != std::io::ErrorKind::Unsupported
        {
            return Err(disco(&caminho, &e));
        }
        let r = f();
        let _ = arquivo.unlock();
        r
    }

    fn registrar_origem_com(
        &self,
        autor: &str,
        em: &str,
        queda: Queda,
    ) -> Result<Indice, ErroVersoes> {
        if let Some(i) = self.indice()? {
            return Ok(i);
        }
        let bytes = fs::read(&self.hipotese).map_err(|_| ErroVersoes::SemHipotese)?;
        if bytes.is_empty() || bytes.len() > MAX_BYTES_DA_VERSAO {
            return Err(ErroVersoes::ConteudoInvalido(
                "a Hipótese está vazia ou é grande demais".into(),
            ));
        }
        let hash = hash_do_conteudo(&bytes);
        let pdf = self.pdf.as_deref().and_then(hash_do_arquivo);
        self.gravar_objeto(&hash, &bytes)?;
        let raiz = Versao {
            hash: hash.clone(),
            pai: None,
            tipo: TipoDeVersao::Hipotese,
            autor: autor.to_owned(),
            em: em.to_owned(),
            nota: None,
            estado: Estado::Hipotese,
            bytes: bytes.len() as u64,
        };
        let mut doc = DocumentMut::new();
        doc["formato"] = value(FORMATO_DO_INDICE);
        doc["partitura"] = value(self.id.as_str());
        let mut origem = Table::new();
        origem["hipotese"] = value(hash.as_str());
        if let Some(p) = &pdf {
            origem["pdf"] = value(p.as_str());
        }
        doc["origem"] = Item::Table(origem);
        let mut lista = ArrayOfTables::new();
        lista.push(tabela_da_versao(&raiz));
        doc["versao"] = Item::ArrayOfTables(lista);
        self.gravar_indice(&doc.to_string(), queda)?;
        Ok(Indice {
            origem: Origem {
                hipotese: hash,
                pdf,
            },
            versoes: vec![raiz],
            decisoes: Vec::new(),
        })
    }

    /// Salva `conteudo` (o MusicXML exportado) como nova Versão filha de `pai` (`None` = a ponta
    /// da cadeia), com nota de motivo opcional. Nunca sobrescreve Versão existente, nem a
    /// Hipótese. A nova Versão nasce **Em revisão**.
    pub fn salvar(
        &self,
        pai: Option<&str>,
        conteudo: &str,
        nota: Option<&str>,
        autor: &str,
        em: &str,
    ) -> Result<ResultadoDoSalvar, ErroVersoes> {
        self.salvar_com(pai, conteudo, nota, autor, em, Queda::Nenhuma)
    }

    pub(crate) fn salvar_com(
        &self,
        pai: Option<&str>,
        conteudo: &str,
        nota: Option<&str>,
        autor: &str,
        em: &str,
        queda: Queda,
    ) -> Result<ResultadoDoSalvar, ErroVersoes> {
        self.com_trava(|| self.salvar_sem_trava(pai, conteudo, nota, autor, em, queda))
    }

    fn salvar_sem_trava(
        &self,
        pai: Option<&str>,
        conteudo: &str,
        nota: Option<&str>,
        autor: &str,
        em: &str,
        queda: Queda,
    ) -> Result<ResultadoDoSalvar, ErroVersoes> {
        if conteudo.trim().is_empty() || conteudo.len() > MAX_BYTES_DA_VERSAO {
            return Err(ErroVersoes::ConteudoInvalido(
                "o conteúdo está vazio ou é grande demais".into(),
            ));
        }
        let indice = self.registrar_origem_com(autor, em, Queda::Nenhuma)?;
        let pai_hash = match pai {
            Some(h) => indice
                .versao(h)
                .map(|v| v.hash.clone())
                .ok_or_else(|| ErroVersoes::PaiInexistente(h.to_owned()))?,
            None => indice
                .versoes
                .last()
                .map(|v| v.hash.clone())
                .ok_or_else(|| ErroVersoes::Formato("índice sem Versões".into()))?,
        };
        let hash = hash_do_conteudo(conteudo.as_bytes());
        if hash == pai_hash {
            return Ok(ResultadoDoSalvar::SemMudancas { hash });
        }
        if indice.versao(&hash).is_some() {
            return Ok(ResultadoDoSalvar::JaExiste { hash });
        }
        // O objeto primeiro; o índice só depois (a queda no meio deixa o índice anterior íntegro).
        self.gravar_objeto(&hash, conteudo.as_bytes())?;
        if queda == Queda::AposObjeto {
            return Err(ErroVersoes::Disco("queda simulada após o objeto".into()));
        }
        let nova = Versao {
            hash,
            pai: Some(pai_hash),
            tipo: TipoDeVersao::Correcao,
            autor: autor.to_owned(),
            em: em.to_owned(),
            nota: limpar_nota(nota),
            estado: Estado::EmRevisao,
            bytes: conteudo.len() as u64,
        };
        let caminho = self.arquivo_do_indice();
        let texto = fs::read_to_string(&caminho).map_err(|e| disco(&caminho, &e))?;
        let mut doc = texto
            .parse::<DocumentMut>()
            .map_err(|e| ErroVersoes::Formato(format!("{}: {e}", caminho.display())))?;
        match doc.get_mut("versao") {
            Some(Item::ArrayOfTables(a)) => a.push(tabela_da_versao(&nova)),
            _ => {
                return Err(ErroVersoes::Formato(
                    "`versao` não é uma lista de tabelas".into(),
                ));
            }
        }
        self.gravar_indice(&doc.to_string(), queda)?;
        Ok(ResultadoDoSalvar::Criada(nova))
    }

    /// Aprova a Versão `hash` (Em revisão → Aprovada) com motivo. Só muda o estado no índice e
    /// registra revisor, data e hash; o conteúdo da Versão não é tocado.
    pub fn aprovar(
        &self,
        hash: &str,
        motivo: Option<&str>,
        revisor: &str,
        em: &str,
    ) -> Result<Decisao, ErroVersoes> {
        self.decidir(hash, AcaoDeRevisao::Aprovar, motivo, revisor, em)
    }

    /// Rejeita a Versão `hash` (Em revisão ou Hipótese → Rejeitada) com motivo.
    pub fn rejeitar(
        &self,
        hash: &str,
        motivo: Option<&str>,
        revisor: &str,
        em: &str,
    ) -> Result<Decisao, ErroVersoes> {
        self.decidir(hash, AcaoDeRevisao::Rejeitar, motivo, revisor, em)
    }

    /// Reabre a Versão `hash` (Aprovada ou Rejeitada → Em revisão; de Hipótese, inicia a
    /// revisão) com motivo. A aprovação deixa de valer.
    pub fn reabrir(
        &self,
        hash: &str,
        motivo: Option<&str>,
        revisor: &str,
        em: &str,
    ) -> Result<Decisao, ErroVersoes> {
        self.decidir(hash, AcaoDeRevisao::Reabrir, motivo, revisor, em)
    }

    /// Faz a ação de revisão sobre a Versão `hash`, pela máquina de `revisao`.
    pub fn decidir(
        &self,
        hash: &str,
        acao: AcaoDeRevisao,
        motivo: Option<&str>,
        revisor: &str,
        em: &str,
    ) -> Result<Decisao, ErroVersoes> {
        self.decidir_com(hash, acao, motivo, revisor, em, Queda::Nenhuma)
    }

    pub(crate) fn decidir_com(
        &self,
        hash: &str,
        acao: AcaoDeRevisao,
        motivo: Option<&str>,
        revisor: &str,
        em: &str,
        queda: Queda,
    ) -> Result<Decisao, ErroVersoes> {
        let motivo = limpar_motivo(motivo)?;
        self.registrar_decisao(hash, acao.alvo(), false, motivo, revisor, em, queda)
    }

    /// Aprovação direta: Hipótese → Aprovada num passo, **só a partir de Hipótese**, marcada
    /// como direta. Sem motivo escrito, usa a nota "aprovação direta".
    pub fn aprovar_direto(
        &self,
        hash: &str,
        motivo: Option<&str>,
        revisor: &str,
        em: &str,
    ) -> Result<Decisao, ErroVersoes> {
        let motivo = match limpar_motivo(motivo) {
            Err(ErroVersoes::MotivoObrigatorio) => NOTA_DA_APROVACAO_DIRETA.to_owned(),
            outro => outro?,
        };
        self.registrar_decisao(
            hash,
            Estado::Aprovada,
            true,
            motivo,
            revisor,
            em,
            Queda::Nenhuma,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn registrar_decisao(
        &self,
        hash: &str,
        para: Estado,
        direto: bool,
        motivo: String,
        revisor: &str,
        em: &str,
        queda: Queda,
    ) -> Result<Decisao, ErroVersoes> {
        let revisor = revisor.trim();
        if revisor.is_empty() {
            return Err(ErroVersoes::RevisorObrigatorio);
        }
        if !hash_valido(hash) {
            return Err(ErroVersoes::Inexistente(hash.to_owned()));
        }
        // Sem índice não há o que decidir (e não cria pasta nem trava à toa).
        let arquivo = self.arquivo_do_indice();
        if !arquivo.exists() {
            return Err(ErroVersoes::Inexistente(hash.to_owned()));
        }
        self.com_trava(|| {
            // Lê o índice já sob a trava: o estado que vale é o de agora.
            let indice = self
                .indice()?
                .ok_or_else(|| ErroVersoes::Inexistente(hash.to_owned()))?;
            let de = indice
                .versao(hash)
                .ok_or_else(|| ErroVersoes::Inexistente(hash.to_owned()))?
                .estado;
            if direto {
                if !aprovacao_direta_valida(de) {
                    return Err(ErroVersoes::AprovacaoDiretaInvalida(de));
                }
            } else if !transicao_de_versao_valida(de, para) {
                return Err(ErroVersoes::TransicaoInvalida { de, para });
            }
            let decisao = Decisao {
                hash: hash.to_owned(),
                de,
                para,
                revisor: revisor.to_owned(),
                em: em.to_owned(),
                motivo,
                direto,
            };
            let texto = fs::read_to_string(&arquivo).map_err(|e| disco(&arquivo, &e))?;
            let mut doc = texto
                .parse::<DocumentMut>()
                .map_err(|e| ErroVersoes::Formato(format!("{}: {e}", arquivo.display())))?;
            let Some(Item::ArrayOfTables(versoes)) = doc.get_mut("versao") else {
                return Err(ErroVersoes::Formato(
                    "`versao` não é uma lista de tabelas".into(),
                ));
            };
            let tabela = versoes
                .iter_mut()
                .find(|t| t.get("hash").and_then(Item::as_str) == Some(hash))
                .ok_or_else(|| ErroVersoes::Inexistente(hash.to_owned()))?;
            tabela["estado"] = value(estado_para_texto(para));
            let nova = tabela_da_decisao(&decisao);
            match doc.get_mut("decisao") {
                Some(Item::ArrayOfTables(a)) => a.push(nova),
                None => {
                    let mut a = ArrayOfTables::new();
                    a.push(nova);
                    doc["decisao"] = Item::ArrayOfTables(a);
                }
                Some(_) => {
                    return Err(ErroVersoes::Formato(
                        "`decisao` não é uma lista de tabelas".into(),
                    ));
                }
            }
            self.gravar_indice(&doc.to_string(), queda)?;
            Ok(decisao)
        })
    }

    /// A aprovação vigente da Versão `hash` (`None` se não está aprovada, não existe ou o hash é
    /// malformado). É por aqui, por hash, que a prática e os Exercícios devem perguntar.
    pub fn aprovacao_de(&self, hash: &str) -> Result<Option<Aprovacao>, ErroVersoes> {
        if !hash_valido(hash) {
            return Ok(None);
        }
        Ok(self.indice()?.and_then(|i| i.aprovacao(hash)))
    }

    /// A aprovação do `conteudo` informado (calcula o SHA-256 e consulta por hash).
    pub fn aprovacao_do_conteudo(&self, conteudo: &[u8]) -> Result<Option<Aprovacao>, ErroVersoes> {
        self.aprovacao_de(&hash_do_conteudo(conteudo))
    }

    /// A aprovação do conteúdo atual de um arquivo (o nome não importa, só o hash do que está
    /// lá): arquivo alterado, outro hash, sem aprovação. Arquivo ilegível: sem aprovação.
    pub fn aprovacao_do_arquivo(&self, caminho: &Path) -> Result<Option<Aprovacao>, ErroVersoes> {
        match hash_do_arquivo(caminho) {
            Some(h) => self.aprovacao_de(&h),
            None => Ok(None),
        }
    }

    /// Grava o objeto uma única vez: nunca troca um arquivo existente (que, se existir, tem de
    /// conferir com o hash).
    fn gravar_objeto(&self, hash: &str, bytes: &[u8]) -> Result<(), ErroVersoes> {
        let dir = self.pasta.join(PASTA_DOS_OBJETOS);
        fs::create_dir_all(&dir).map_err(|e| disco(&dir, &e))?;
        let destino = self.arquivo_do_objeto(hash);
        if destino.exists() {
            let atual = fs::read(&destino).map_err(|e| disco(&destino, &e))?;
            return if hash_do_conteudo(&atual) == hash {
                Ok(())
            } else {
                Err(ErroVersoes::Corrompida(hash.to_owned()))
            };
        }
        let tmp = dir.join(format!(".{hash}.{}.tmp", std::process::id()));
        let r = (|| {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
            // `hard_link` falha se o destino já existe: nunca substitui um objeto.
            match fs::hard_link(&tmp, &destino) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
                Err(_) if !destino.exists() => fs::rename(&tmp, &destino),
                Err(e) => Err(e),
            }
        })();
        let _ = fs::remove_file(&tmp);
        r.map_err(|e| disco(&destino, &e))
    }

    fn gravar_indice(&self, texto: &str, queda: Queda) -> Result<(), ErroVersoes> {
        fs::create_dir_all(&self.pasta).map_err(|e| disco(&self.pasta, &e))?;
        let caminho = self.arquivo_do_indice();
        if queda == Queda::NoMeioDoIndice {
            // Simula a queda: um temporário pela metade, sem o rename.
            let tmp = caminho.with_file_name(format!(".{ARQUIVO_DO_INDICE}.queda.tmp"));
            let metade = &texto.as_bytes()[..texto.len() / 2];
            fs::write(&tmp, metade).map_err(|e| disco(&tmp, &e))?;
            return Err(ErroVersoes::Disco(
                "queda simulada no meio do índice".into(),
            ));
        }
        gravar_atomico(&caminho, texto).map_err(|e| ErroVersoes::Disco(e.to_string()))
    }

    /// Abre uma Versão para leitura, conferindo o conteúdo contra o hash.
    pub fn abrir(&self, hash: &str) -> Result<VersaoAberta, ErroVersoes> {
        if !hash_valido(hash) {
            return Err(ErroVersoes::Inexistente(hash.to_owned()));
        }
        let indice = self
            .indice()?
            .ok_or_else(|| ErroVersoes::Inexistente(hash.to_owned()))?;
        let versao = indice
            .versao(hash)
            .cloned()
            .ok_or_else(|| ErroVersoes::Inexistente(hash.to_owned()))?;
        let caminho = self.arquivo_do_objeto(hash);
        let bytes = fs::read(&caminho).map_err(|e| disco(&caminho, &e))?;
        if hash_do_conteudo(&bytes) != hash {
            return Err(ErroVersoes::Corrompida(hash.to_owned()));
        }
        let conteudo = String::from_utf8(bytes)
            .map_err(|_| ErroVersoes::Formato("a Versão não é UTF-8".into()))?;
        Ok(VersaoAberta { versao, conteudo })
    }

    /// Confere os originais (Hipótese e PDF) contra os hashes registrados. Chamar após qualquer
    /// ação da Oficina. Só lê.
    pub fn verificar_originais(&self) -> Result<RelatorioDeOriginais, ErroVersoes> {
        let Some(indice) = self.indice()? else {
            return Ok(RelatorioDeOriginais {
                hipotese: SituacaoDoOriginal::NaoRegistrado,
                pdf: self.pdf.as_ref().map(|_| SituacaoDoOriginal::NaoRegistrado),
            });
        };
        let hipotese = situacao(Some(&self.hipotese), Some(&indice.origem.hipotese));
        let pdf = match (&self.pdf, &indice.origem.pdf) {
            (None, None) => None,
            (p, h) => Some(situacao(p.as_deref(), h.as_ref())),
        };
        Ok(RelatorioDeOriginais { hipotese, pdf })
    }

    /// A lista para a tela: a fonte (PDF) e a Hipótese primeiro, depois as correções em
    /// sequência de criação.
    pub fn linhas(&self) -> Result<Vec<LinhaDaLista>, ErroVersoes> {
        let Some(indice) = self.indice()? else {
            return Ok(Vec::new());
        };
        let mut linhas = Vec::new();
        if let Some(pdf) = &indice.origem.pdf {
            linhas.push(LinhaDaLista {
                rotulo: "Fonte (PDF)".into(),
                hash: pdf.clone(),
                pai: None,
                autor: String::new(),
                em: indice
                    .versoes
                    .first()
                    .map(|v| v.em.clone())
                    .unwrap_or_default(),
                nota: None,
                estado: None,
                abrivel: false,
            });
        }
        let mut n = 0usize;
        for v in &indice.versoes {
            let rotulo = if v.tipo == TipoDeVersao::Hipotese {
                "Hipótese".to_owned()
            } else {
                n += 1;
                format!("Versão {n}")
            };
            linhas.push(LinhaDaLista {
                rotulo,
                hash: v.hash.clone(),
                pai: v.pai.clone(),
                autor: v.autor.clone(),
                em: v.em.clone(),
                nota: v.nota.clone(),
                estado: Some(v.estado),
                abrivel: true,
            });
        }
        Ok(linhas)
    }
}

/// Estado de uma Versão de edição ao nascer (ticket 21): Em revisão.
pub const ESTADO_DA_NOVA_VERSAO: Estado = Estado::EmRevisao;

/// A transição de estado de uma Versão é permitida? Reaproveita a máquina de `revisao` (para o
/// ticket 23, que liga a aprovação ao hash).
pub const fn transicao_de_versao_valida(de: Estado, para: Estado) -> bool {
    transicao_valida(de, para)
}

fn tabela_da_versao(v: &Versao) -> Table {
    let mut t = Table::new();
    t["hash"] = value(v.hash.as_str());
    if let Some(p) = &v.pai {
        t["pai"] = value(p.as_str());
    }
    t["tipo"] = value(v.tipo.texto());
    t["autor"] = value(v.autor.as_str());
    t["em"] = value(v.em.as_str());
    if let Some(n) = &v.nota {
        t["nota"] = value(n.as_str());
    }
    t["estado"] = value(estado_para_texto(v.estado));
    t["bytes"] = value(i64::try_from(v.bytes).unwrap_or(i64::MAX));
    t
}

fn tabela_da_decisao(d: &Decisao) -> Table {
    let mut t = Table::new();
    t["hash"] = value(d.hash.as_str());
    t["de"] = value(estado_para_texto(d.de));
    t["para"] = value(estado_para_texto(d.para));
    t["revisor"] = value(d.revisor.as_str());
    t["em"] = value(d.em.as_str());
    t["motivo"] = value(d.motivo.as_str());
    if d.direto {
        t["direto"] = value(true);
    }
    t
}

/// SHA-256 do arquivo, lido em blocos; `None` se não existe ou não lê.
fn hash_do_arquivo(caminho: &Path) -> Option<Hash> {
    use std::io::Read as _;
    let mut f = fs::File::open(caminho).ok()?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Some(em_hex(&h.finalize()))
}

fn situacao(caminho: Option<&Path>, esperado: Option<&Hash>) -> SituacaoDoOriginal {
    let Some(esperado) = esperado else {
        return SituacaoDoOriginal::NaoRegistrado;
    };
    match caminho.and_then(hash_do_arquivo) {
        None => SituacaoDoOriginal::Ausente,
        Some(a) if &a == esperado => SituacaoDoOriginal::Integro,
        Some(atual) => SituacaoDoOriginal::Alterado {
            esperado: esperado.clone(),
            atual,
        },
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const HIPOTESE: &str = "<score-partwise><!-- hipotese --></score-partwise>";
    const CORRECAO_1: &str = "<score-partwise><!-- correcao 1 --></score-partwise>";
    const CORRECAO_2: &str = "<score-partwise><!-- correcao 2 --></score-partwise>";
    const EM: &str = "2026-11-27T10:00:00Z";

    struct Cenario {
        _d: tempfile::TempDir,
        partituras: PathBuf,
        v: Versoes,
    }

    fn cenario() -> Cenario {
        let d = tempfile::tempdir().expect("pasta");
        let partituras = d.path().join("oficina/partituras");
        fs::create_dir_all(&partituras).expect("cria");
        fs::write(partituras.join("ode.musicxml"), HIPOTESE).expect("hipotese");
        fs::write(d.path().join("oficina/ode.pdf"), b"%PDF-falso-de-teste").expect("pdf");
        let v = Versoes::em(&partituras, "ode", Some(d.path().join("oficina/ode.pdf")));
        Cenario {
            _d: d,
            partituras,
            v,
        }
    }

    fn criada(r: ResultadoDoSalvar) -> Versao {
        match r {
            ResultadoDoSalvar::Criada(v) => v,
            outro => panic!("esperava Versão criada, veio {outro:?}"),
        }
    }

    fn hash_do_arquivo_de(c: &Cenario) -> Hash {
        hash_do_conteudo(&fs::read(c.partituras.join("ode.musicxml")).expect("lê"))
    }

    #[test]
    fn hash_e_sha256_conhecido() {
        assert_eq!(
            hash_do_conteudo(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(hash_valido(&hash_do_conteudo(b"x")));
        assert!(!hash_valido("../etc/passwd"));
        assert!(!hash_valido(&"A".repeat(64)));
    }

    /// A8: duas correções = duas Versões encadeadas; o hash da Hipótese não muda.
    #[test]
    fn duas_correcoes_encadeadas_e_hipotese_intacta() {
        let c = cenario();
        let antes = hash_do_arquivo_de(&c);
        let v1 = criada(
            c.v.salvar(None, CORRECAO_1, Some("corrigi o compasso 3"), "Chico", EM)
                .expect("salva 1"),
        );
        let v2 = criada(
            c.v.salvar(None, CORRECAO_2, None, "Chico", "2026-11-27T10:05:00Z")
                .expect("salva 2"),
        );
        let lista = c.v.listar().expect("lista");
        assert_eq!(lista.len(), 3, "Hipótese + duas correções");
        assert_eq!(lista[0].tipo, TipoDeVersao::Hipotese);
        assert_eq!(lista[0].pai, None);
        assert_eq!(
            v1.pai.as_deref(),
            Some(antes.as_str()),
            "v1 filha da Hipótese"
        );
        assert_eq!(v2.pai.as_deref(), Some(v1.hash.as_str()), "v2 filha de v1");
        assert_eq!(v1.hash, hash_do_conteudo(CORRECAO_1.as_bytes()));
        assert_eq!(v1.estado, Estado::EmRevisao, "a edição nasce Em revisão");
        assert_eq!(v1.autor, "Chico");
        assert_eq!(v1.em, EM);
        assert_eq!(v1.nota.as_deref(), Some("corrigi o compasso 3"));
        assert_eq!(v2.nota, None);
        assert_eq!(antes, hash_do_arquivo_de(&c), "o hash da Hipótese não muda");
        assert_eq!(lista[0].hash, antes);
        assert_eq!(lista[0].estado, Estado::Hipotese);
    }

    #[test]
    fn pai_explicito_ramifica_a_cadeia() {
        let c = cenario();
        let v1 = criada(c.v.salvar(None, CORRECAO_1, None, "Chico", EM).expect("v1"));
        let raiz = c.v.listar().expect("l")[0].hash.clone();
        let v2 = criada(
            c.v.salvar(Some(&raiz), CORRECAO_2, None, "Chico", EM)
                .expect("v2 sobre a Hipótese"),
        );
        assert_eq!(v2.pai.as_deref(), Some(raiz.as_str()));
        assert_ne!(v1.hash, v2.hash);
        let r = c.v.salvar(Some(&"0".repeat(64)), "<x/>", None, "Chico", EM);
        assert!(matches!(r, Err(ErroVersoes::PaiInexistente(_))));
    }

    #[test]
    fn salvar_sem_mudancas_nao_cria_versao_vazia() {
        let c = cenario();
        let r = c.v.salvar(None, HIPOTESE, None, "Chico", EM).expect("ok");
        assert!(matches!(r, ResultadoDoSalvar::SemMudancas { .. }));
        assert_eq!(c.v.listar().expect("l").len(), 1, "só a Hipótese");
        let v1 = criada(c.v.salvar(None, CORRECAO_1, None, "Chico", EM).expect("v1"));
        let r = c.v.salvar(None, CORRECAO_1, None, "Chico", EM).expect("ok");
        assert_eq!(
            r,
            ResultadoDoSalvar::SemMudancas {
                hash: v1.hash.clone()
            }
        );
        // Voltar ao conteúdo de uma Versão antiga não duplica a identidade.
        criada(c.v.salvar(None, CORRECAO_2, None, "Chico", EM).expect("v2"));
        let r = c.v.salvar(None, CORRECAO_1, None, "Chico", EM).expect("ok");
        assert_eq!(r, ResultadoDoSalvar::JaExiste { hash: v1.hash });
        assert_eq!(c.v.listar().expect("l").len(), 3);
    }

    #[test]
    fn salvar_nunca_sobrescreve_arquivo_de_versao_existente() {
        let c = cenario();
        let v1 = criada(c.v.salvar(None, CORRECAO_1, None, "Chico", EM).expect("v1"));
        let objeto = c.v.arquivo_do_objeto(&v1.hash);
        let antes = fs::read(&objeto).expect("lê");
        // Mesmo conteúdo de novo (a partir da Hipótese): o arquivo segue idêntico.
        let raiz = c.v.listar().expect("l")[0].hash.clone();
        let _ = c.v.salvar(Some(&raiz), CORRECAO_1, None, "Outro", EM);
        assert_eq!(fs::read(&objeto).expect("lê"), antes);
        // Um objeto adulterado no lugar é recusado em vez de ser trocado em silêncio.
        fs::write(&objeto, "adulterado").expect("adultera");
        assert!(matches!(
            c.v.gravar_objeto(&v1.hash, CORRECAO_1.as_bytes()),
            Err(ErroVersoes::Corrompida(_))
        ));
        assert_eq!(fs::read_to_string(&objeto).expect("lê"), "adulterado");
        assert!(matches!(
            c.v.abrir(&v1.hash),
            Err(ErroVersoes::Corrompida(_))
        ));
    }

    #[test]
    fn abrir_versao_antiga_devolve_o_conteudo_conferido() {
        let c = cenario();
        let v1 = criada(
            c.v.salvar(None, CORRECAO_1, Some("n"), "Chico", EM)
                .expect("v1"),
        );
        criada(c.v.salvar(None, CORRECAO_2, None, "Chico", EM).expect("v2"));
        let aberta = c.v.abrir(&v1.hash).expect("abre");
        assert_eq!(aberta.conteudo, CORRECAO_1);
        assert_eq!(aberta.versao, v1);
        let raiz = c.v.listar().expect("l")[0].hash.clone();
        assert_eq!(c.v.abrir(&raiz).expect("abre").conteudo, HIPOTESE);
        assert!(matches!(
            c.v.abrir("../../etc/passwd"),
            Err(ErroVersoes::Inexistente(_))
        ));
        assert!(matches!(
            c.v.abrir(&"a".repeat(64)),
            Err(ErroVersoes::Inexistente(_))
        ));
    }

    #[test]
    fn lista_mostra_fonte_e_hipotese_primeiro_e_as_correcoes_em_sequencia() {
        let c = cenario();
        assert!(
            c.v.linhas().expect("l").is_empty(),
            "sem registro, sem linhas"
        );
        criada(
            c.v.salvar(None, CORRECAO_1, Some("primeira"), "Chico", EM)
                .expect("v1"),
        );
        criada(
            c.v.salvar(None, CORRECAO_2, Some("segunda"), "Ana", EM)
                .expect("v2"),
        );
        let l = c.v.linhas().expect("linhas");
        let rotulos: Vec<&str> = l.iter().map(|x| x.rotulo.as_str()).collect();
        assert_eq!(rotulos, ["Fonte (PDF)", "Hipótese", "Versão 1", "Versão 2"]);
        assert!(!l[0].abrivel && l[1].abrivel);
        assert_eq!(l[0].estado, None);
        assert_eq!(l[3].autor, "Ana");
        assert_eq!(l[3].estado, Some(Estado::EmRevisao));
        let t = l[2].em_texto();
        assert!(t.contains("Versão 1") && t.contains("Chico") && t.contains("primeira"));
        assert!(t.contains("Em revisão") && t.contains(EM), "{t}");
    }

    #[test]
    fn sem_pdf_a_lista_comeca_na_hipotese() {
        let c = cenario();
        let v = Versoes::em(&c.partituras, "ode", None);
        criada(v.salvar(None, CORRECAO_1, None, "Chico", EM).expect("v1"));
        assert_eq!(v.linhas().expect("l")[0].rotulo, "Hipótese");
        assert_eq!(v.verificar_originais().expect("v").pdf, None);
    }

    #[test]
    fn originais_sao_verificados_por_hash() {
        let c = cenario();
        let r = c.v.verificar_originais().expect("sem registro");
        assert_eq!(r.hipotese, SituacaoDoOriginal::NaoRegistrado);
        assert!(!r.integro());
        criada(c.v.salvar(None, CORRECAO_1, None, "Chico", EM).expect("v1"));
        assert!(c.v.verificar_originais().expect("ok").integro());
        // O PDF muda por fora: detectado.
        fs::write(c.partituras.join("../ode.pdf"), b"outro").expect("troca");
        let r = c.v.verificar_originais().expect("ok");
        assert!(r.hipotese.integro());
        assert!(matches!(r.pdf, Some(SituacaoDoOriginal::Alterado { .. })));
        assert!(!r.integro());
        // A Hipótese some: ausente.
        fs::remove_file(c.partituras.join("ode.musicxml")).expect("rm");
        assert_eq!(
            c.v.verificar_originais().expect("ok").hipotese,
            SituacaoDoOriginal::Ausente
        );
        // A cópia da Hipótese em objetos/ continua abrível (a raiz é imutável).
        let raiz = c.v.listar().expect("l")[0].hash.clone();
        assert_eq!(c.v.abrir(&raiz).expect("abre").conteudo, HIPOTESE);
    }

    /// Queda no meio da gravação do índice: a Versão anterior segue íntegra e salvar de novo
    /// funciona.
    #[test]
    fn queda_no_meio_da_gravacao_do_indice_deixa_a_versao_anterior_integra() {
        let c = cenario();
        let v1 = criada(c.v.salvar(None, CORRECAO_1, None, "Chico", EM).expect("v1"));
        let indice_antes = fs::read_to_string(c.v.arquivo_do_indice()).expect("lê");
        let r =
            c.v.salvar_com(None, CORRECAO_2, None, "Chico", EM, Queda::NoMeioDoIndice);
        assert!(r.is_err());
        assert_eq!(
            fs::read_to_string(c.v.arquivo_do_indice()).expect("lê"),
            indice_antes,
            "o índice não mudou"
        );
        let lista = c.v.listar().expect("lista ainda legível");
        assert_eq!(lista.len(), 2);
        assert_eq!(c.v.abrir(&v1.hash).expect("abre").conteudo, CORRECAO_1);
        // O temporário órfão não atrapalha e a Versão pode ser salva depois.
        let v2 = criada(c.v.salvar(None, CORRECAO_2, None, "Chico", EM).expect("v2"));
        assert_eq!(v2.pai.as_deref(), Some(v1.hash.as_str()));
        assert_eq!(c.v.listar().expect("l").len(), 3);
    }

    #[test]
    fn queda_apos_o_objeto_nao_cria_versao_nem_estraga_o_indice() {
        let c = cenario();
        criada(c.v.salvar(None, CORRECAO_1, None, "Chico", EM).expect("v1"));
        let r =
            c.v.salvar_com(None, CORRECAO_2, None, "Chico", EM, Queda::AposObjeto);
        assert!(r.is_err());
        assert_eq!(
            c.v.listar().expect("l").len(),
            2,
            "a Versão não existe sem o índice"
        );
        // O objeto órfão é reaproveitado (mesmo hash, conferido) no salvar seguinte.
        let v2 = criada(c.v.salvar(None, CORRECAO_2, None, "Chico", EM).expect("v2"));
        assert_eq!(c.v.abrir(&v2.hash).expect("abre").conteudo, CORRECAO_2);
    }

    #[test]
    fn queda_ao_registrar_a_origem_nao_deixa_indice_pela_metade() {
        let c = cenario();
        let r = c.v.registrar_origem_com("Chico", EM, Queda::NoMeioDoIndice);
        assert!(r.is_err());
        assert_eq!(c.v.indice().expect("sem índice"), None, "nada registrado");
        assert!(c.v.registrar_origem("Chico", EM).is_ok(), "tenta de novo");
        assert_eq!(c.v.listar().expect("l").len(), 1);
    }

    #[test]
    fn indice_corrompido_vira_erro_de_formato() {
        let c = cenario();
        criada(c.v.salvar(None, CORRECAO_1, None, "Chico", EM).expect("v1"));
        fs::write(c.v.arquivo_do_indice(), "isto = não é toml [[").expect("estraga");
        assert!(matches!(c.v.listar(), Err(ErroVersoes::Formato(_))));
        // Hash com caminho dentro do índice é recusado antes de virar caminho.
        fs::write(
            c.v.arquivo_do_indice(),
            "formato = 1\n[origem]\nhipotese = \"../../x\"\n",
        )
        .expect("escreve");
        assert!(matches!(c.v.listar(), Err(ErroVersoes::Formato(_))));
    }

    #[test]
    fn sem_hipotese_nao_ha_de_onde_comecar() {
        let c = cenario();
        fs::remove_file(c.partituras.join("ode.musicxml")).expect("rm");
        assert_eq!(
            c.v.salvar(None, CORRECAO_1, None, "Chico", EM),
            Err(ErroVersoes::SemHipotese)
        );
    }

    #[test]
    fn conteudo_vazio_e_recusado_e_nota_e_limpa() {
        let c = cenario();
        assert!(matches!(
            c.v.salvar(None, "   ", None, "Chico", EM),
            Err(ErroVersoes::ConteudoInvalido(_))
        ));
        let longa = format!("a\nb\t{}", "x".repeat(2 * MAX_NOTA));
        let v = criada(
            c.v.salvar(None, CORRECAO_1, Some(&longa), "Chico", EM)
                .expect("v"),
        );
        let n = v.nota.expect("nota");
        assert!(n.chars().count() <= MAX_NOTA && !n.contains('\n'));
        let v2 = criada(
            c.v.salvar(None, CORRECAO_2, Some("  "), "Chico", EM)
                .expect("v"),
        );
        assert_eq!(v2.nota, None, "nota em branco não é gravada");
        assert_eq!(
            c.v.listar().expect("l")[1].nota.as_deref(),
            Some(n.as_str())
        );
    }

    #[test]
    fn nota_com_aspas_e_acentos_volta_igual() {
        let c = cenario();
        let nota = "corrigi \"dó\" e o \\ compasso — ç";
        criada(
            c.v.salvar(None, CORRECAO_1, Some(nota), "Chico", EM)
                .expect("v"),
        );
        assert_eq!(c.v.listar().expect("l")[1].nota.as_deref(), Some(nota));
    }

    #[test]
    fn estado_da_versao_reaproveita_a_maquina_existente() {
        assert_eq!(ESTADO_DA_NOVA_VERSAO, Estado::EmRevisao);
        assert!(transicao_de_versao_valida(
            Estado::EmRevisao,
            Estado::Aprovada
        ));
        assert!(!transicao_de_versao_valida(
            Estado::Hipotese,
            Estado::Aprovada
        ));
    }

    // --- Ticket 23: aprovar, rejeitar e reabrir por hash ---------------------------------

    const REV: &str = "Chico";
    const EM2: &str = "2026-11-28T09:30:00Z";

    fn salvar_correcao(c: &Cenario, pai: Option<&str>, conteudo: &str) -> Versao {
        criada(
            c.v.salvar(pai, conteudo, Some("ajuste"), "Chico", EM)
                .expect("salva"),
        )
    }

    fn estado_de(c: &Cenario, hash: &str) -> Estado {
        c.v.listar()
            .expect("l")
            .into_iter()
            .find(|v| v.hash == hash)
            .expect("existe")
            .estado
    }

    fn hash_da_hipotese(c: &Cenario) -> Hash {
        c.v.registrar_origem(REV, EM).expect("origem").versoes[0]
            .hash
            .clone()
    }

    #[test]
    fn a9_editar_versao_aprovada_cria_versao_em_revisao_sem_herdar_a_aprovacao() {
        let c = cenario();
        let v1 = salvar_correcao(&c, None, CORRECAO_1);
        c.v.aprovar(&v1.hash, Some("conferi com o PDF"), REV, EM2)
            .expect("aprova");
        assert_eq!(estado_de(&c, &v1.hash), Estado::Aprovada);
        // Edição a partir da Versão aprovada, seguida de salvar.
        let v2 = salvar_correcao(&c, Some(&v1.hash), CORRECAO_2);
        assert_ne!(v2.hash, v1.hash);
        assert_eq!(v2.pai.as_deref(), Some(v1.hash.as_str()));
        assert_eq!(v2.estado, Estado::EmRevisao);
        assert_eq!(estado_de(&c, &v2.hash), Estado::EmRevisao);
        assert!(c.v.aprovacao_de(&v2.hash).expect("l").is_none());
        // A aprovação da anterior continua valendo para ela, e só para ela.
        assert!(c.v.aprovacao_de(&v1.hash).expect("l").is_some());
        assert_eq!(estado_de(&c, &v1.hash), Estado::Aprovada);
    }

    #[test]
    fn aprovar_rejeitar_e_reabrir_registram_revisor_data_hash_e_motivo() {
        let c = cenario();
        let v1 = salvar_correcao(&c, None, CORRECAO_1);
        let a =
            c.v.aprovar(&v1.hash, Some("  conferi o compasso 3 "), REV, EM2)
                .expect("aprova");
        assert_eq!(a.hash, v1.hash);
        assert_eq!(a.revisor, REV);
        assert_eq!(a.em, EM2);
        assert_eq!(a.motivo, "conferi o compasso 3");
        assert!(!a.direto);
        assert_eq!(c.v.aprovacao_de(&v1.hash).expect("l"), Some(a.aprovacao()));

        let d =
            c.v.reabrir(&v1.hash, Some("achei um erro de oitava"), REV, EM2)
                .expect("reabre");
        assert_eq!((d.de, d.para), (Estado::Aprovada, Estado::EmRevisao));
        assert!(c.v.aprovacao_de(&v1.hash).expect("l").is_none());

        let r =
            c.v.rejeitar(&v1.hash, Some("transcrição errada"), REV, EM2)
                .expect("rejeita");
        assert_eq!((r.de, r.para), (Estado::EmRevisao, Estado::Rejeitada));
        assert_eq!(estado_de(&c, &v1.hash), Estado::Rejeitada);
        assert!(c.v.aprovacao_de(&v1.hash).expect("l").is_none());

        // O registro fica no índice, em ordem, com revisor, data, hash e motivo.
        let reg = c.v.indice().expect("l").expect("há").decisoes;
        assert_eq!(reg.len(), 3);
        assert!(reg.iter().all(|d| d.hash == v1.hash && d.revisor == REV));
        assert!(reg.iter().all(|d| d.em == EM2 && !d.direto));
        assert_eq!(reg[1].motivo, "achei um erro de oitava");
        assert_eq!(reg[2].para, Estado::Rejeitada);
        // Reabrir a rejeitada também é válido.
        c.v.reabrir(&v1.hash, Some("revi"), REV, EM2)
            .expect("reabre");
        assert_eq!(estado_de(&c, &v1.hash), Estado::EmRevisao);
    }

    #[test]
    fn transicoes_invalidas_sao_recusadas_com_mensagem_e_nada_muda() {
        let c = cenario();
        let v1 = salvar_correcao(&c, None, CORRECAO_1);
        let raiz = c.v.listar().expect("l")[0].hash.clone();
        // Hipótese não vira Aprovada por `aprovar` (só pela aprovação direta).
        let e = c.v.aprovar(&raiz, Some("m"), REV, EM2).expect_err("recusa");
        assert_eq!(
            e,
            ErroVersoes::TransicaoInvalida {
                de: Estado::Hipotese,
                para: Estado::Aprovada
            }
        );
        let msg = e.to_string();
        assert!(
            msg.contains("Hipótese") && msg.contains("Aprovada"),
            "{msg}"
        );
        // Em revisão não "reabre"; Aprovada não aprova nem rejeita; Rejeitada não aprova.
        assert!(matches!(
            c.v.reabrir(&v1.hash, Some("m"), REV, EM2),
            Err(ErroVersoes::TransicaoInvalida { .. })
        ));
        c.v.aprovar(&v1.hash, Some("m"), REV, EM2).expect("aprova");
        assert!(matches!(
            c.v.aprovar(&v1.hash, Some("m"), REV, EM2),
            Err(ErroVersoes::TransicaoInvalida { .. })
        ));
        assert!(matches!(
            c.v.rejeitar(&v1.hash, Some("m"), REV, EM2),
            Err(ErroVersoes::TransicaoInvalida { .. })
        ));
        c.v.reabrir(&v1.hash, Some("m"), REV, EM2).expect("reabre");
        c.v.rejeitar(&v1.hash, Some("m"), REV, EM2)
            .expect("rejeita");
        assert!(matches!(
            c.v.aprovar(&v1.hash, Some("m"), REV, EM2),
            Err(ErroVersoes::TransicaoInvalida { .. })
        ));
        // As recusas não deixaram registro: só as 3 decisões válidas.
        assert_eq!(c.v.indice().expect("l").expect("há").decisoes.len(), 3);
        assert_eq!(estado_de(&c, &raiz), Estado::Hipotese);
    }

    #[test]
    fn as_acoes_alcancam_exatamente_o_que_a_maquina_de_revisao_permite() {
        for de in crate::revisao::ESTADOS {
            for (acao, para) in [
                (AcaoDeRevisao::Aprovar, Estado::Aprovada),
                (AcaoDeRevisao::Rejeitar, Estado::Rejeitada),
                (AcaoDeRevisao::Reabrir, Estado::EmRevisao),
            ] {
                assert_eq!(acao.alvo(), para);
                assert_eq!(
                    transicao_de_versao_valida(de, acao.alvo()),
                    transicao_valida(de, para)
                );
            }
        }
    }

    #[test]
    fn hipotese_pode_ser_rejeitada_ou_ir_para_revisao_como_manda_a_maquina() {
        let c = cenario();
        let raiz = hash_da_hipotese(&c);
        c.v.reabrir(&raiz, Some("começo a revisar"), REV, EM2)
            .expect("Hipótese → Em revisão");
        assert_eq!(estado_de(&c, &raiz), Estado::EmRevisao);
        let c2 = cenario();
        let raiz2 = hash_da_hipotese(&c2);
        c2.v.rejeitar(&raiz2, Some("transcrição lixo"), REV, EM2)
            .expect("Hipótese → Rejeitada");
        assert_eq!(estado_de(&c2, &raiz2), Estado::Rejeitada);
    }

    #[test]
    fn aprovar_rejeitar_e_reabrir_exigem_motivo_curto() {
        let c = cenario();
        let v1 = salvar_correcao(&c, None, CORRECAO_1);
        for motivo in [None, Some(""), Some("   "), Some("\t\n ")] {
            assert_eq!(
                c.v.aprovar(&v1.hash, motivo, REV, EM2),
                Err(ErroVersoes::MotivoObrigatorio)
            );
            assert_eq!(
                c.v.rejeitar(&v1.hash, motivo, REV, EM2),
                Err(ErroVersoes::MotivoObrigatorio)
            );
            assert_eq!(
                c.v.reabrir(&v1.hash, motivo, REV, EM2),
                Err(ErroVersoes::MotivoObrigatorio)
            );
        }
        let longo = "x".repeat(MAX_MOTIVO + 1);
        let e =
            c.v.aprovar(&v1.hash, Some(&longo), REV, EM2)
                .expect_err("longo");
        assert_eq!(e, ErroVersoes::MotivoLongo(MAX_MOTIVO));
        assert!(e.to_string().contains(&MAX_MOTIVO.to_string()));
        assert_eq!(
            c.v.aprovar(&v1.hash, Some("ok"), "  ", EM2),
            Err(ErroVersoes::RevisorObrigatorio)
        );
        // Motivo no limite passa, e nada foi registrado pelas recusas.
        let no_limite = "x".repeat(MAX_MOTIVO);
        c.v.aprovar(&v1.hash, Some(&no_limite), REV, EM2)
            .expect("ok");
        assert_eq!(c.v.indice().expect("l").expect("há").decisoes.len(), 1);
    }

    #[test]
    fn aprovacao_direta_so_de_hipotese_e_fica_marcada_como_direta() {
        let c = cenario();
        let raiz = hash_da_hipotese(&c);
        let a = c.v.aprovar_direto(&raiz, None, REV, EM2).expect("direta");
        assert!(a.direto);
        assert_eq!(a.motivo, crate::revisao::NOTA_DA_APROVACAO_DIRETA);
        assert_eq!(c.v.aprovacao_de(&raiz).expect("l"), Some(a.aprovacao()));
        let d = &c.v.indice().expect("l").expect("há").decisoes[0];
        assert!(d.direto);
        assert_eq!((d.de, d.para), (Estado::Hipotese, Estado::Aprovada));
        let texto = fs::read_to_string(c.v.pasta().join(ARQUIVO_DO_INDICE)).expect("lê");
        assert!(texto.contains("direto = true"), "{texto}");
        // A aprovação normal não marca `direto`.
        let v1 = salvar_correcao(&c, Some(&raiz), CORRECAO_1);
        c.v.aprovar(&v1.hash, Some("ok"), REV, EM2).expect("aprova");
        let reg = c.v.indice().expect("l").expect("há").decisoes;
        assert!(!reg[1].direto);
        // Direta só a partir de Hipótese: de Em revisão ou Aprovada é recusada.
        let v2 = salvar_correcao(&c, Some(&v1.hash), CORRECAO_2);
        for h in [&v2.hash, &v1.hash] {
            let e = c.v.aprovar_direto(h, None, REV, EM2).expect_err("recusa");
            assert!(matches!(e, ErroVersoes::AprovacaoDiretaInvalida(_)), "{e}");
            assert!(e.to_string().contains("Hipótese"), "{e}");
        }
        // Motivo escrito na direta vale (e continua marcada).
        let c2 = cenario();
        let r2 = hash_da_hipotese(&c2);
        let a2 =
            c2.v.aprovar_direto(&r2, Some("é a transcrição fiel"), REV, EM2)
                .expect("direta");
        assert!(a2.direto);
        assert_eq!(a2.motivo, "é a transcrição fiel");
    }

    #[test]
    fn consumidor_consulta_por_hash_e_alterar_o_arquivo_invalida_a_aprovacao() {
        let c = cenario();
        let v1 = salvar_correcao(&c, None, CORRECAO_1);
        c.v.aprovar(&v1.hash, Some("ok"), REV, EM2).expect("aprova");
        // O mesmo conteúdo, de qualquer arquivo/nome, é a Versão aprovada.
        assert!(
            c.v.aprovacao_do_conteudo(CORRECAO_1.as_bytes())
                .expect("l")
                .is_some()
        );
        // Um arquivo exportado e depois alterado muda o hash: a aprovação não vale.
        let exportado = c.partituras.join("ode-aprovada.musicxml");
        fs::write(&exportado, CORRECAO_1).expect("exporta");
        assert!(c.v.aprovacao_do_arquivo(&exportado).expect("l").is_some());
        fs::write(&exportado, format!("{CORRECAO_1}<!-- mexi -->")).expect("altera");
        assert!(c.v.aprovacao_do_arquivo(&exportado).expect("l").is_none());
        // Arquivo inexistente: sem aprovação (nunca "aprovado por padrão").
        assert!(
            c.v.aprovacao_do_arquivo(&c.partituras.join("nao-existe.musicxml"))
                .expect("l")
                .is_none()
        );
        // Hash desconhecido ou malformado: sem aprovação.
        assert!(c.v.aprovacao_de(&"0".repeat(64)).expect("l").is_none());
        assert!(c.v.aprovacao_de("../../x").expect("l").is_none());
    }

    #[test]
    fn t8_hipotese_original_fica_intacta_apos_aprovar_uma_versao_derivada() {
        let c = cenario();
        let raiz = hash_da_hipotese(&c);
        let obj =
            c.v.pasta()
                .join(PASTA_DOS_OBJETOS)
                .join(format!("{raiz}.musicxml"));
        let arquivo = c.partituras.join("ode.musicxml");
        let arquivo_antes = fs::read(&arquivo).expect("lê");
        let obj_antes = fs::read(&obj).expect("lê");
        let v1 = salvar_correcao(&c, None, CORRECAO_1);
        c.v.aprovar(&v1.hash, Some("ok"), REV, EM2).expect("aprova");
        assert_eq!(fs::read(&arquivo).expect("lê"), arquivo_antes);
        assert_eq!(fs::read(&obj).expect("lê"), obj_antes);
        assert_eq!(hash_do_arquivo_de(&c), raiz);
        assert_eq!(estado_de(&c, &raiz), Estado::Hipotese);
        assert!(c.v.aprovacao_de(&raiz).expect("l").is_none());
        assert!(c.v.verificar_originais().expect("v").integro());
    }

    #[test]
    fn aprovacao_so_muda_o_estado_no_indice_nunca_o_objeto() {
        let c = cenario();
        let v1 = salvar_correcao(&c, None, CORRECAO_1);
        let obj =
            c.v.pasta()
                .join(PASTA_DOS_OBJETOS)
                .join(format!("{}.musicxml", v1.hash));
        let antes = fs::read(&obj).expect("lê");
        c.v.aprovar(&v1.hash, Some("ok"), REV, EM2).expect("aprova");
        c.v.reabrir(&v1.hash, Some("revi"), REV, EM2)
            .expect("reabre");
        c.v.rejeitar(&v1.hash, Some("não"), REV, EM2)
            .expect("rejeita");
        assert_eq!(fs::read(&obj).expect("lê"), antes);
        assert_eq!(c.v.abrir(&v1.hash).expect("abre").conteudo, CORRECAO_1);
        assert!(c.v.verificar_originais().expect("v").integro());
    }

    #[test]
    fn decidir_hash_inexistente_ou_malformado_e_erro() {
        let c = cenario();
        salvar_correcao(&c, None, CORRECAO_1);
        assert!(matches!(
            c.v.aprovar(&"a".repeat(64), Some("m"), REV, EM2),
            Err(ErroVersoes::Inexistente(_))
        ));
        assert!(matches!(
            c.v.aprovar("../../etc", Some("m"), REV, EM2),
            Err(ErroVersoes::Inexistente(_))
        ));
        // Sem índice nenhum também é "inexistente", e não cria nada.
        let vazio = cenario();
        assert!(matches!(
            vazio.v.aprovar(&"a".repeat(64), Some("m"), REV, EM2),
            Err(ErroVersoes::Inexistente(_))
        ));
        assert!(!vazio.v.pasta().join(ARQUIVO_DO_INDICE).exists());
    }

    #[test]
    fn estado_aprovada_escrito_a_mao_sem_registro_nao_vale_como_aprovacao() {
        let c = cenario();
        let v1 = salvar_correcao(&c, None, CORRECAO_1);
        let caminho = c.v.pasta().join(ARQUIVO_DO_INDICE);
        let texto = fs::read_to_string(&caminho).expect("lê");
        let adulterado = texto.replacen("estado = \"em_revisao\"", "estado = \"aprovada\"", 1);
        assert_ne!(texto, adulterado);
        fs::write(&caminho, adulterado).expect("grava");
        assert_eq!(estado_de(&c, &v1.hash), Estado::Aprovada);
        assert!(c.v.aprovacao_de(&v1.hash).expect("l").is_none());
    }

    #[test]
    fn decisao_preserva_campos_desconhecidos_e_resiste_a_queda() {
        let c = cenario();
        let v1 = salvar_correcao(&c, None, CORRECAO_1);
        let caminho = c.v.pasta().join(ARQUIVO_DO_INDICE);
        let texto = fs::read_to_string(&caminho).expect("lê");
        // Campo desconhecido no nível da Versão (o `[[versao]]` é a última tabela do arquivo).
        fs::write(
            &caminho,
            format!("# comentário do futuro\n{texto}futuro = \"fica\"\n"),
        )
        .expect("grava");
        // Queda no meio da gravação do índice: o estado anterior continua íntegro.
        let e = c.v.decidir_com(
            &v1.hash,
            AcaoDeRevisao::Aprovar,
            Some("ok"),
            REV,
            EM2,
            Queda::NoMeioDoIndice,
        );
        assert!(e.is_err());
        assert_eq!(estado_de(&c, &v1.hash), Estado::EmRevisao);
        assert!(c.v.indice().expect("l").expect("há").decisoes.is_empty());
        c.v.aprovar(&v1.hash, Some("ok"), REV, EM2).expect("aprova");
        let depois = fs::read_to_string(&caminho).expect("lê");
        assert!(depois.contains("# comentário do futuro"), "{depois}");
        assert!(depois.contains("futuro = \"fica\""), "{depois}");
        assert!(depois.contains("[[decisao]]"), "{depois}");
    }

    #[test]
    fn indice_com_decisao_de_hash_desconhecido_e_erro_de_formato() {
        let c = cenario();
        let v1 = salvar_correcao(&c, None, CORRECAO_1);
        c.v.aprovar(&v1.hash, Some("ok"), REV, EM2).expect("aprova");
        let caminho = c.v.pasta().join(ARQUIVO_DO_INDICE);
        let texto = fs::read_to_string(&caminho).expect("lê");
        let alvo = format!("[[decisao]]\nhash = \"{}\"", v1.hash);
        assert!(texto.contains(&alvo), "{texto}");
        let falso = format!("[[decisao]]\nhash = \"{}\"", "b".repeat(64));
        fs::write(&caminho, texto.replace(&alvo, &falso)).expect("grava");
        assert!(matches!(c.v.indice(), Err(ErroVersoes::Formato(_))));
    }

    #[test]
    fn varios_gravadores_ao_mesmo_tempo_nao_perdem_atualizacoes() {
        // Vários "processos" (threads, cada uma com a sua Versoes) salvam e decidem no mesmo índice.
        let c = cenario();
        let base = salvar_correcao(&c, None, CORRECAO_1);
        let mut juntas = Vec::new();
        for i in 0..8 {
            let v = c.v.clone();
            let pai = base.hash.clone();
            juntas.push(std::thread::spawn(move || {
                let conteudo = format!("<score-partwise><!-- ramo {i} --></score-partwise>");
                let nova = match v.salvar(Some(&pai), &conteudo, Some("ramo"), "Chico", EM) {
                    Ok(ResultadoDoSalvar::Criada(n)) => n,
                    outro => panic!("salvar falhou: {outro:?}"),
                };
                v.aprovar(&nova.hash, Some("ok"), REV, EM2).expect("aprova");
                nova.hash
            }));
        }
        let hashes: Vec<Hash> = juntas
            .into_iter()
            .map(|j| j.join().expect("thread"))
            .collect();
        let lista = c.v.listar().expect("l");
        assert_eq!(lista.len(), 1 + 1 + 8, "nenhuma Versão se perdeu");
        for h in &hashes {
            assert_eq!(estado_de(&c, h), Estado::Aprovada, "decisão perdida");
        }
        assert_eq!(c.v.indice().expect("l").expect("há").decisoes.len(), 8);
        // A trava é um arquivo ao lado do índice.
        assert!(c.v.pasta().join(ARQUIVO_DA_TRAVA).exists());
    }

    #[test]
    fn da_partitura_deriva_a_pasta_ao_lado_do_estado_toml() {
        let d = tempfile::tempdir().expect("pasta");
        let raiz = d.path().join("fontes/t/oficina");
        let pasta = raiz.join("partituras");
        fs::create_dir_all(&pasta).expect("cria");
        fs::write(pasta.join("a.musicxml"), HIPOTESE).expect("x");
        let estado_original = "estado = \"hipotese\"\npdf = \"a.pdf\"\n";
        fs::write(pasta.join("a.estado.toml"), estado_original).expect("t");
        fs::write(raiz.join("a.pdf"), b"%PDF").expect("p");
        let of = crate::revisao::Oficina {
            tutor: "t".into(),
            raiz: raiz.clone(),
        };
        let item = ItemPartitura::ler(&of, "a");
        let v = Versoes::da_partitura(&item);
        assert_eq!(v.pasta(), pasta.join("a.versoes"));
        criada(
            v.salvar(None, CORRECAO_1, None, crate::revisao::REVISOR, EM)
                .expect("v"),
        );
        // O estado.toml (a Hipótese) segue como estava.
        assert_eq!(
            fs::read_to_string(pasta.join("a.estado.toml")).expect("l"),
            estado_original
        );
        let r = v.verificar_originais().expect("v");
        assert!(r.integro());
        assert!(r.pdf.is_some(), "o PDF relativo da oficina foi resolvido");
        // As Versões ficam só dentro da oficina.
        assert!(v.pasta().starts_with(&raiz));
    }

    #[test]
    fn pdf_com_caminho_inseguro_nao_e_lido() {
        let d = tempfile::tempdir().expect("pasta");
        let raiz = d.path().join("oficina");
        let pasta = raiz.join("partituras");
        fs::create_dir_all(&pasta).expect("cria");
        fs::write(pasta.join("a.musicxml"), HIPOTESE).expect("x");
        fs::write(d.path().join("segredo.pdf"), b"%PDF").expect("p");
        fs::write(
            pasta.join("a.estado.toml"),
            "estado = \"hipotese\"\npdf = \"../../segredo.pdf\"\n",
        )
        .expect("t");
        let of = crate::revisao::Oficina {
            tutor: "t".into(),
            raiz,
        };
        let v = Versoes::da_partitura(&ItemPartitura::ler(&of, "a"));
        v.registrar_origem(crate::revisao::REVISOR, EM).expect("ok");
        assert_eq!(v.indice().expect("i").expect("há").origem.pdf, None);
    }

    /// A8 de ponta a ponta: a Hipótese vira documento, duas edições reais saem pelo serviço de
    /// documento e viram duas Versões encadeadas; o arquivo da Hipótese não muda um byte.
    #[test]
    fn a8_edicoes_reais_do_servico_viram_versoes_encadeadas() {
        use crate::documento_partitura::DocumentoPartitura;
        use crate::tela_partitura::EMBUTIDAS;
        use piano_tutor_core::documento::{ComandoDeEdicao, SentidoDaAltura, UnidadeDeAltura};

        let d = tempfile::tempdir().expect("pasta");
        let partituras = d.path().join("oficina/partituras");
        fs::create_dir_all(&partituras).expect("cria");
        let original = EMBUTIDAS[3].1;
        fs::write(partituras.join("ode.musicxml"), original).expect("hipotese");
        let v = Versoes::em(&partituras, "ode", None);
        let antes = fs::read(partituras.join("ode.musicxml")).expect("lê");

        let mut doc =
            DocumentoPartitura::de_texto("ode".into(), original.to_owned()).expect("abre");
        let nota = doc.leitura.primeira().expect("há nota");
        let comando = |sentido| ComandoDeEdicao::MudarAltura {
            nota,
            unidade: UnidadeDeAltura::Semitom,
            sentido,
        };
        doc.aplicar(comando(SentidoDaAltura::Acima))
            .expect("edita 1");
        let v1 = criada(
            v.salvar(
                None,
                &doc.exportado().expect("exporta"),
                Some("subi um semitom"),
                "Chico",
                EM,
            )
            .expect("salva 1"),
        );
        doc.aplicar(comando(SentidoDaAltura::Acima))
            .expect("edita 2");
        let v2 = criada(
            v.salvar(None, &doc.exportado().expect("exporta"), None, "Chico", EM)
                .expect("salva 2"),
        );
        assert_eq!(v2.pai.as_deref(), Some(v1.hash.as_str()));
        let raiz = v.listar().expect("l")[0].clone();
        assert_eq!(v1.pai.as_deref(), Some(raiz.hash.as_str()));
        assert_eq!(raiz.hash, hash_do_conteudo(original.as_bytes()));
        assert_eq!(
            fs::read(partituras.join("ode.musicxml")).expect("lê"),
            antes
        );
        assert_ne!(v1.hash, v2.hash);
        // Cada Versão reabre idêntica ao exportado na hora.
        assert_ne!(v.abrir(&v1.hash).expect("abre").conteudo, original);
        assert_eq!(v.abrir(&raiz.hash).expect("abre").conteudo, original);
    }

    /// Fonte única: o app (`Indice::aprovacao`) e o curso (importador/CLI) leem o mesmo índice e
    /// têm de concordar em TODOS os casos (estado da Versão x histórico de decisões).
    #[test]
    fn app_e_curso_nao_divergem_na_regra_de_aprovacao() {
        let d = tempfile::tempdir().expect("t");
        let pasta = d.path().join("p.versoes");
        fs::create_dir_all(&pasta).expect("cria");
        let conteudo = b"<conteudo-da-versao/>";
        let hash = hash_do_conteudo(conteudo);
        let outro = hash_do_conteudo(b"outro");
        let estados = ["hipotese", "em_revisao", "aprovada", "rejeitada"];
        let para = ["hipotese", "em_revisao", "aprovada", "rejeitada"];
        // Históricos: vazio; uma decisão; duas (a última manda); decisão só de outra Versão.
        let mut historicos: Vec<Vec<(String, &str)>> = vec![vec![]];
        for a in para {
            historicos.push(vec![(hash.clone(), a)]);
            historicos.push(vec![(outro.clone(), a)]);
            for b in para {
                historicos.push(vec![(hash.clone(), a), (hash.clone(), b)]);
                historicos.push(vec![(hash.clone(), a), (outro.clone(), b)]);
            }
        }
        let mut aprovados = 0;
        for estado in estados {
            for h in &historicos {
                let mut t = format!(
                    "formato = 1\npartitura = \"p\"\n[origem]\nhipotese = \"{hash}\"\n\
                     [[versao]]\nhash = \"{hash}\"\ntipo = \"hipotese\"\nestado = \"{estado}\"\n\
                     [[versao]]\nhash = \"{outro}\"\ntipo = \"correcao\"\npai = \"{hash}\"\nestado = \"hipotese\"\n"
                );
                for (hh, p) in h {
                    t.push_str(&format!(
                        "[[decisao]]\nhash = \"{hh}\"\nde = \"em_revisao\"\npara = \"{p}\"\n\
                         revisor = \"r\"\nem = \"x\"\nmotivo = \"m\"\n"
                    ));
                }
                fs::write(pasta.join(ARQUIVO_DO_INDICE), &t).expect("grava");
                let v = Versoes::em(d.path(), "p", None);
                let no_app = v
                    .indice()
                    .expect("app lê")
                    .and_then(|i| i.aprovacao(&hash))
                    .is_some();
                let no_curso = piano_tutor_curso::estado_do_conteudo_no_indice(
                    &pasta.join(ARQUIVO_DO_INDICE),
                    conteudo,
                )
                .expect("curso lê")
                    == Some(piano_tutor_curso::EstadoConteudo::Aprovada);
                assert_eq!(no_app, no_curso, "estado {estado}, decisões {h:?}");
                aprovados += usize::from(no_app);
            }
        }
        assert!(aprovados > 0, "a matriz tem de incluir casos aprovados");
    }
}
