# 10: Abrir uma partitura na Oficina e exportar idêntico (serviço de documento, 1ª fatia)

**Fase:** E01

**R-IDs/critérios:** R-E01, R-E02, R-A01, R-B06 (abrir); A1, A6 (exportação sem edição); histórias 1 (abrir), 20

**Seam:** S1

**Modelo sugerido:** Opus / médio (fixa o contrato do serviço que as próximas fatias usam)

**What to build:** Na Oficina, o revisor abre uma partitura MusicXML/MXL que já existe no repositório de oficina/revisão atual e a vê como antes, e pode exportá-la. A abertura passa pelo serviço de documento do núcleo (sem janela), que atribui as identidades de nota, guarda as origens por faixa de bytes e exporta idêntico ao original quando nada foi editado. A tela passa a usar o serviço, e as identidades de nota chegam ao layout (ticket 06).

**Blocked by:** 05 (Prefactor: separar o documento da partitura do estado da tela), 08 (Spike: modelo híbrido de documento (IdNota + faixa de bytes), prova A1/A2 no corpus CC0)

**Status:** feito, aguarda revisão

- [x] A1: para cada fixture do corpus, abrir e exportar produz bytes idênticos ao original (teste golden sobre todo o corpus, via serviço, sem interface).
- [x] A tela da Oficina abre um arquivo MusicXML e um MXL do repositório de oficina existente e mostra a partitura; o aspecto é o mesmo de antes (testes de tela existentes passam).
- [x] Exportar a partir da tela grava arquivo idêntico ao original e continua pedindo confirmação antes de sobrescrever; a releitura de conferência passa.
- [x] A interface do serviço é a única porta usada pela tela para abrir e exportar (nenhum parse direto remanescente nessa rota); um teste do serviço roda sem GUI, sem áudio e sem rede.
- [x] O modelo exposto pelo serviço inclui a identidade estável de cada nota e as primitivas do layout carregam essas identidades (verificado por teste de layout).
- [x] Documento aberto sem edição não grava nada em disco (nem autosave nem Versão).
- [x] Não há regressão nos testes existentes de reprodução e prática (S4 intocado).

**Notas:** Contrato (tipos, erros, ciclo de vida) deve sair documentado no PR em linguagem de domínio; os próximos tickets só o estendem. Fora do escopo: edição, Versão, PDF.

## Entregue (ticket 10)

- **Serviço no núcleo:** `piano_tutor_core::documento` (módulo promovido do spike, com `IdNota` e `PosCompasso` do núcleo) e a porta `documento::servico::DocumentoAberto` (`abrir_texto`, `abrir_bytes` com `Formato::{MusicXml, Mxl}`, `exportar`, `conferir_exportacao`, `editado`, `ids`). O contrato (tipos, erros, ciclo de vida, o que é recusado) está no cabeçalho de `crates/piano-tutor-core/src/documento/servico.rs`, em linguagem de domínio.
- **Erros tipados (`ErroServico`):** `Partitura` (as recusas e mensagens de sempre: XML, limites, `.mxl`), `Documento` (UTF-8, entidade que expande elementos, faixas), `Inconsistente`, `ReleituraDifere`.
- **Tela:** `documento_partitura.rs` só lê/grava arquivos; abrir e exportar passam pelo serviço (teste de fonte garante que não há parse direto na rota). Layout: `calcular_layout_com_ids` e `calcular_layout_fita_com_ids`; `Item::nota` é preenchido na cabeça, nos pontos de aumento e no acidente; `calcular_layout` sem ids segue idêntico (assinaturas golden iguais).
- **Decisões do supervisor aplicadas:** sem arquivo lateral de ids (só no ticket 17); mudança de `divisions` recusada por padrão (`definir_duracao`; a variante que reescala fica documentada como só após confirmação explícita, ticket 16).
- **Não verificado:** aspecto visual numa janela real (só testes de layout/tessituração sem GPU); memória do documento (ADR-0017 já a marca NÃO MEDIDA); `.mxl` de exportadores reais.
