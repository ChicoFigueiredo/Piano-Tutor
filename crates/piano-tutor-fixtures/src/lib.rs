//! Corpus de fixtures MusicXML e harness de golden (E01, ticket 07).
//!
//! Só para testes: nenhum crate de produção depende deste. Reúne:
//! - [`corpus`]: fixtures com licença e origem registradas (índice em `fixtures/INDICE.md`);
//! - [`harness`]: comparação byte a byte (com lista de compassos que diferem) e semântica;
//! - [`leitura`]: relatório do que o modelo atual descarta por fixture;
//! - [`gerador`]: peça de 100 compassos e `.mxl` válidos/maliciosos gerados por script.

pub mod corpus;
pub mod gerador;
pub mod harness;
pub mod leitura;
