//! Pacote de Curso do Piano Tutor (formato v1): leitor, verificador, empacotador e instalador.
//!
//! Um Pacote de Curso (`.ptcurso`) é um zip **só de dados** que leva um Curso de tutor ao app:
//! `manifesto.toml` (Curso → Seções → Lições → Exercícios, autor, licenciado e `[[arquivos]]`
//! com SHA-256), `assinatura.ed25519` (assinatura dos bytes exatos do manifesto), partituras
//! MusicXML, áudio opcional, Alinhamentos (`alinhamentos/<id>.json`) e Roteiro (`roteiro.toml`).
//! O contrato completo está em `docs/specs/FORMATO-PACOTE-DE-CURSO.md`; a decisão, na ADR-0009.
//!
//! Este crate não tem GUI, rede nem `unsafe`, e nunca executa nada que venha do pacote.
//!
//! # Visão geral da API
//!
//! * [`ler_pacote`] / [`ler_pacote_de_arquivo`]: verificam assinatura, hashes e estrutura e
//!   devolvem um [`PacoteVerificado`]. Qualquer divergência vira um [`ErroPacote`] em português.
//! * [`PacoteVerificado::confianca`]: [`Confianca::Confiavel`] só se a chave estiver na lista
//!   dada (veja [`chaves_confiaveis`]); assinatura válida de chave desconhecida é só autêntica.
//! * [`instalar`]: extrai para `<dados>/cursos/<id>/` de forma atômica; [`cursos_instalados`] lista.
//! * [`empacotar`]: pasta de origem + [`ChavePrivada`] → bytes do `.ptcurso`.
//! * [`importar`]: plano de importação + pasta da oficina → pasta-origem para `empacotar`.
//! * [`Exercicio::serve_de_gabarito`] e [`Manifesto::exercicios_para_aluno`]: só Exercício
//!   `aprovada` é Gabarito; `hipotese`, `em_revisao` e `rejeitada` nunca chegam ao aluno.
//! * [`Exercicio::utilizavel`], [`Manifesto::exercicios_para_aluno_com`] e
//!   [`Roteiro::falas_utilizaveis`]: uso pessoal do licenciado. Com `permitir_nao_revisado`,
//!   `hipotese` e `em_revisao` também podem ser usados (o app põe o selo "não revisado");
//!   `rejeitada` nunca. O Gabarito oficial continua sendo só o aprovado.
//!
//! ```no_run
//! use piano_tutor_curso::{chaves_confiaveis, instalar, ler_pacote_de_arquivo, Confianca};
//! # fn main() -> Result<(), piano_tutor_curso::ErroPacote> {
//! let dados = std::path::Path::new("/home/aluno/.local/share/piano-tutor");
//! let pacote = ler_pacote_de_arquivo(std::path::Path::new("curso.ptcurso"))?;
//! if pacote.confianca(&chaves_confiaveis(dados)) == Confianca::Desconhecida {
//!     println!("autor não reconhecido: chave {}", pacote.chave_publica.curta());
//! }
//! instalar(&pacote, dados)?;
//! # Ok(()) }
//! ```

mod chaves;
pub mod cli;
mod empacotamento;
mod erro;
mod importacao;
mod instalacao;
mod leitura;
mod modelo;
mod validacao;

pub use chaves::{
    CHAVES_EMBUTIDAS, ChavePrivada, ChavePublica, chaves_confiaveis, gerar_par, ler_chaves_da_pasta,
};
pub use empacotamento::empacotar;
pub use erro::ErroPacote;
pub use importacao::{DecideEstado, EstadoDoArquivo, Resumo, importar, importar_com};
pub use instalacao::{CursoInstalado, cursos_instalados, instalar};
pub use leitura::{Confianca, PacoteVerificado, ler_pacote, ler_pacote_de_arquivo};
pub use modelo::{
    Alinhamento, Arquivo, Aula, Autor, Contagem, EstadoConteudo, Exercicio, ExercicioNoCurso, Fala,
    Licao, Licenciado, Manifesto, Mao, Modo, Momento, NotaAlinhada, OrigemFala, Roteiro, Secao,
    TipoLicao, TrechoAula,
};
pub use validacao::{
    caminho_seguro, id_valido, validar_alinhamento, validar_manifesto, validar_roteiro,
};

/// Versão do formato que este crate lê e escreve.
pub const FORMATO_VERSAO: u32 = 1;
/// Extensão dos pacotes.
pub const EXTENSAO: &str = "ptcurso";

pub(crate) const ARQUIVO_MANIFESTO: &str = "manifesto.toml";
pub(crate) const ARQUIVO_ASSINATURA: &str = "assinatura.ed25519";
pub(crate) const ARQUIVO_ROTEIRO: &str = "roteiro.toml";

/// Limite do pacote: 200 MB (arquivo e conteúdo extraído).
pub const MAX_PACOTE_BYTES: u64 = 200 * 1024 * 1024;
/// Limite de cada arquivo dentro do pacote: 50 MB.
pub const MAX_ARQUIVO_BYTES: u64 = 50 * 1024 * 1024;
/// Limite de arquivos listados no manifesto.
pub const MAX_ARQUIVOS: usize = 2000;
/// Limite de cada documento de texto (manifesto, Roteiro, Alinhamento): 8 MB.
pub const MAX_DOCUMENTO_BYTES: u64 = 8 * 1024 * 1024;

/// Nomes que não podem ser id de curso: pastas reservadas dentro de `<dados>/cursos/`.
pub(crate) const PASTAS_RESERVADAS: &[&str] = &["chaves-confiaveis", "progresso"];

/// SHA-256 em hexadecimal minúsculo.
pub fn sha256_hex(dados: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    chaves::para_hex(Sha256::digest(dados).as_slice())
}
