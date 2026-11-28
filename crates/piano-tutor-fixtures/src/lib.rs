//! Corpus de fixtures MusicXML e harness de golden (E01, ticket 07).
//!
//! Só para testes: nenhum crate de produção depende deste. Reúne:
//! - [`corpus`]: fixtures com licença e origem registradas (índice em `fixtures/INDICE.md`);
//! - [`harness`]: comparação byte a byte (com lista de compassos que diferem) e semântica;
//! - [`leitura`]: relatório do que o modelo atual descarta por fixture;
//! - [`documento`]: reexporta o modelo híbrido de documento, que agora vive no núcleo (ticket 10);
//! - [`exibicao`]: peça mínima com letra, cifra, dedilhado e elementos sem suporte (ticket 19);
//! - [`gerador`]: peça de 100 compassos e `.mxl` válidos/maliciosos gerados por script.

pub mod corpus;
pub mod exibicao;
/// O modelo híbrido de documento (ADR-0017) vive em `piano_tutor_core::documento`; este crate o
/// consome sem duplicar (os testes do spike continuam importando por aqui).
pub use piano_tutor_core::documento;
pub mod gerador;
pub mod harness;
pub mod leitura;
