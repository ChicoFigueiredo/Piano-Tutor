//! Modelo de dados do Pacote de Curso (formato v1): Manifesto, Alinhamento e Roteiro.
//!
//! Os nomes dos campos são os do contrato em `docs/specs/FORMATO-PACOTE-DE-CURSO.md`.

use serde::{Deserialize, Serialize};

use crate::erro::ErroPacote;

/// Estado de um conteúdo gerado por IA e revisado por humano (ver `CONTEXT.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoConteudo {
    Hipotese,
    EmRevisao,
    Aprovada,
    Rejeitada,
}

impl EstadoConteudo {
    pub const TODOS: [EstadoConteudo; 4] = [
        EstadoConteudo::Hipotese,
        EstadoConteudo::EmRevisao,
        EstadoConteudo::Aprovada,
        EstadoConteudo::Rejeitada,
    ];

    /// Nome para mostrar ao usuário.
    pub fn rotulo(self) -> &'static str {
        match self {
            Self::Hipotese => "hipótese",
            Self::EmRevisao => "em revisão",
            Self::Aprovada => "aprovada",
            Self::Rejeitada => "rejeitada",
        }
    }

    fn posicao(self) -> usize {
        match self {
            Self::Hipotese => 0,
            Self::EmRevisao => 1,
            Self::Aprovada => 2,
            Self::Rejeitada => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoLicao {
    /// Só vídeo da Aula, sem Exercícios.
    Assistir,
    Exercicio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modo {
    Praticar,
    ChamadaEResposta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mao {
    Direita,
    Esquerda,
    Ambas,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Autor {
    pub nome: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Licenciado {
    pub nome: String,
    #[serde(default)]
    pub licenca: String,
}

/// Chave pública declarada pelo pacote (hexadecimal). Vazia só na origem, antes de empacotar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssinaturaInfo {
    #[serde(default)]
    pub chave_publica: String,
}

impl AssinaturaInfo {
    /// Sem chave, a seção `[assinatura]` não vai para o manifesto da pasta-origem.
    fn vazia(&self) -> bool {
        self.chave_publica.is_empty()
    }
}

/// Arquivo do pacote com SHA-256 e tamanho (`[[arquivos]]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arquivo {
    pub caminho: String,
    pub sha256: String,
    pub bytes: u64,
}

/// Referência ao vídeo da Aula, que NÃO entra no pacote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Aula {
    /// Sugestão para o aluno localizar o próprio arquivo (só o nome, sem pasta).
    pub nome_arquivo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duracao_s: Option<f64>,
    /// Opcional: confere se o arquivo do aluno é o mesmo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// Trecho do vídeo da Aula em que o tutor toca o Exercício.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrechoAula {
    pub inicio_s: f64,
    pub fim_s: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Exercicio {
    pub id: String,
    pub titulo: String,
    pub modo: Modo,
    pub mao: Mao,
    pub estado: EstadoConteudo,
    pub partitura: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alinhamento: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bpm: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trecho_aula: Option<TrechoAula>,
}

impl Exercicio {
    /// Só Exercício `aprovada` serve de Gabarito; nunca saída bruta de IA.
    pub fn serve_de_gabarito(&self) -> bool {
        self.estado == EstadoConteudo::Aprovada
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Licao {
    pub id: String,
    /// Número da Aula no curso do autor.
    pub numero: u32,
    pub titulo: String,
    pub tipo: TipoLicao,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aula: Option<Aula>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exercicios: Vec<Exercicio>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Secao {
    pub id: String,
    pub numero: u32,
    pub titulo: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub licoes: Vec<Licao>,
}

/// Um Exercício com a Seção e a Lição a que pertence.
#[derive(Debug, Clone, Copy)]
pub struct ExercicioNoCurso<'a> {
    pub secao: &'a Secao,
    pub licao: &'a Licao,
    pub exercicio: &'a Exercicio,
}

/// `manifesto.toml`: Curso → Seções → Lições → Exercícios, autor, licenciado e arquivos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifesto {
    pub formato_versao: u32,
    pub id: String,
    pub titulo: String,
    pub versao: String,
    pub gerado_em: String,
    pub idioma: String,
    pub convencao_oitava: String,
    pub autor: Autor,
    pub licenciado: Licenciado,
    #[serde(default, skip_serializing_if = "AssinaturaInfo::vazia")]
    pub assinatura: AssinaturaInfo,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secoes: Vec<Secao>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arquivos: Vec<Arquivo>,
}

/// Quantidades de um Curso, com os Exercícios separados por estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Contagem {
    pub secoes: usize,
    pub licoes: usize,
    pub exercicios: usize,
    por_estado: [usize; 4],
}

impl Contagem {
    pub fn por_estado(&self, estado: EstadoConteudo) -> usize {
        self.por_estado[estado.posicao()]
    }
}

impl Manifesto {
    pub fn de_toml(texto: &str) -> Result<Self, ErroPacote> {
        toml::from_str(texto)
            .map_err(|e| ErroPacote::ManifestoInvalido(format!("manifesto.toml: {e}")))
    }

    pub fn para_toml(&self) -> Result<String, ErroPacote> {
        toml::to_string_pretty(self).map_err(|e| {
            ErroPacote::ManifestoInvalido(format!("não consegui gravar o manifesto: {e}"))
        })
    }

    /// Todos os Exercícios, em qualquer estado, na ordem do autor.
    pub fn exercicios(&self) -> impl Iterator<Item = ExercicioNoCurso<'_>> {
        self.secoes.iter().flat_map(|secao| {
            secao.licoes.iter().flat_map(move |licao| {
                licao
                    .exercicios
                    .iter()
                    .map(move |exercicio| ExercicioNoCurso {
                        secao,
                        licao,
                        exercicio,
                    })
            })
        })
    }

    /// Exercícios que o aluno pode praticar: só os que servem de Gabarito (`aprovada`).
    /// Exclui `hipotese`, `em_revisao` e `rejeitada`.
    pub fn exercicios_para_aluno(&self) -> impl Iterator<Item = ExercicioNoCurso<'_>> {
        self.exercicios()
            .filter(|e| e.exercicio.serve_de_gabarito())
    }

    pub fn contagem(&self) -> Contagem {
        let mut c = Contagem {
            secoes: self.secoes.len(),
            ..Contagem::default()
        };
        for secao in &self.secoes {
            c.licoes += secao.licoes.len();
        }
        for e in self.exercicios() {
            c.exercicios += 1;
            c.por_estado[e.exercicio.estado.posicao()] += 1;
        }
        c
    }
}

// ---------- Alinhamento ----------

/// Nota da partitura ligada ao instante em que soa no áudio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotaAlinhada {
    /// Ordem de `piano_tutor_core::comparador::esperados_da_partitura`.
    pub indice: u32,
    pub compasso: u32,
    pub midi: u8,
    pub t_partitura_s: f64,
    /// Relativo ao início de `audio`.
    pub t_audio_s: f64,
    pub confianca: f64,
}

/// `alinhamentos/<id>.json`. O instante no vídeo é `offset_aula_s + t_audio_s`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alinhamento {
    pub formato_versao: u32,
    pub exercicio: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<String>,
    pub estado: EstadoConteudo,
    pub metodo: String,
    pub offset_aula_s: f64,
    pub notas: Vec<NotaAlinhada>,
}

impl Alinhamento {
    pub fn de_json(bytes: &[u8]) -> Result<Self, ErroPacote> {
        serde_json::from_slice(bytes)
            .map_err(|e| ErroPacote::ManifestoInvalido(format!("alinhamento: {e}")))
    }

    pub fn para_json(&self) -> Result<String, ErroPacote> {
        serde_json::to_string_pretty(self).map_err(|e| {
            ErroPacote::ManifestoInvalido(format!("não consegui gravar o alinhamento: {e}"))
        })
    }
}

// ---------- Roteiro ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Momento {
    Antes,
    Passo,
    Acerto,
    Erro,
    Depois,
}

/// Trecho da fala da Aula de onde a frase veio.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OrigemFala {
    pub aula: u32,
    pub inicio_s: f64,
    pub fim_s: f64,
}

/// Fala do professor, ligada a uma Lição e (opcionalmente) a um Exercício.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fala {
    pub id: String,
    pub licao: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exercicio: Option<String>,
    pub momento: Momento,
    /// Só com `momento = "passo"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passo: Option<u32>,
    pub texto: String,
    pub estado: EstadoConteudo,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origem: Option<OrigemFala>,
}

/// `roteiro.toml`: falas do professor. Só as `aprovada` são ditas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Roteiro {
    pub formato_versao: u32,
    #[serde(default)]
    pub falas: Vec<Fala>,
}

impl Roteiro {
    pub fn de_toml(texto: &str) -> Result<Self, ErroPacote> {
        toml::from_str(texto)
            .map_err(|e| ErroPacote::ManifestoInvalido(format!("roteiro.toml: {e}")))
    }

    pub fn para_toml(&self) -> Result<String, ErroPacote> {
        toml::to_string_pretty(self).map_err(|e| {
            ErroPacote::ManifestoInvalido(format!("não consegui gravar o roteiro: {e}"))
        })
    }

    /// Falas que o professor pode dizer (estado `aprovada`).
    pub fn falas_aprovadas(&self) -> impl Iterator<Item = &Fala> {
        self.falas
            .iter()
            .filter(|f| f.estado == EstadoConteudo::Aprovada)
    }
}
