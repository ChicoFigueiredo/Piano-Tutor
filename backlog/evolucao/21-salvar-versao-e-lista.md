# 21: Salvar como nova Versão, com nota de motivo, e listar as Versões

**Fase:** E01

**R-IDs/critérios:** R-B06, R-E02, T8, D8; A8; histórias 15, 16, 17

**Seam:** S1

**Modelo sugerido:** Sonnet / alto (Opus revisa o formato de armazenamento)

**What to build:** O revisor salva o que fez como uma nova Versão, com nota opcional de motivo; cada Versão é imutável, identificada pelo hash do conteúdo, e ligada à Versão-pai, com autor e data. A lista de Versões mostra a Hipótese e a fonte como as primeiras, e as correções em sequência. A Hipótese e o arquivo original nunca são alterados. Nesta fase as Versões vivem no repositório de oficina; o Acervo (E02) as assume depois.

**Blocked by:** 14 (Selecionar nota e mudar a altura com seta (semitom, oitava, escala))

**Status:** em andamento — módulo pronto e revisado pelo supervisor; critérios de tela abertos até a UI ser ligada (após tickets 15 e 19) (módulo `versoes.rs`; falta ligar a UI: botão "Salvar como Versão", campo de nota e lista na tela da Oficina/Partitura)

- [x] A8: dada uma Hipótese aberta, salvar duas correções cria duas Versões encadeadas (pai→filho) e o hash da Hipótese permanece idêntico antes e depois (teste).
- [x] Salvar nunca sobrescreve arquivo de Versão existente; salvar sem mudanças não cria Versão vazia (ou pede confirmação, decisão registrada). (decisão: sem mudança = não cria Versão, devolve `SemMudancas`)
- [x] Cada Versão registra hash de conteúdo, hash do pai, autor, data, nota de motivo e estado de conteúdo (nova Versão de edição nasce Em revisão); estados reaproveitam a máquina existente.
- [ ] A lista de Versões mostra autor, data, nota e estado, e abrir uma Versão antiga a carrega em modo leitura ou como novo ponto de partida (decisão registrada). (modelo `linhas()`; decisão: abrir é somente leitura, ponto de partida = salvar com `pai` = hash; UI pendente)
- [x] Gravação atômica (arquivo temporário e renomear); queda no meio da gravação deixa a Versão anterior íntegra (teste de interrupção simulada).
- [x] O arquivo original (PDF e MusicXML de origem) é verificado por hash após qualquer ação da Oficina. (`verificar_originais()`; a tela deve chamá-lo após cada ação)
- [x] Material comprado nunca é copiado para fora da pasta de dados do usuário; o formato de armazenamento está documentado para a migração futura ao Acervo. (formato documentado no cabeçalho de `versoes.rs`)

**Notas:** Pergunta aberta: guardar as Versões no formato atual do repositório de oficina (estado em TOML + arquivos) ou já preparando o esquema do Acervo. Por padrão, o formato atual com histórico estendido.


**Supervisor (2026-11-27):** dois critérios reabertos porque só valem no módulo; a tela ainda não mostra a lista nem chama `verificar_originais()` após cada ação. A ligação da UI vai num passo seguinte, depois de 15 e 19. Também falta a trava entre processos: hoje dois salvamentos simultâneos podem perder uma entrada do índice.

**Ligação à tela (2026-11-28):** `tela_partitura.rs` (painel "Versões" na tela Partitura): botão "Salvar como Versão" com campo de nota (desativado com dica em arquivo solto ou em modo leitura), lista (`linhas()`: rótulo, autor, data, estado, nota), "Abrir" em modo leitura (o documento em edição fica guardado; "Voltar à edição"; editar, desfazer e salvar são recusados em leitura), `conferir_originais()` (`verificar_originais`) após editar, desfazer/refazer, salvar, abrir Versão e abrir partitura, com alerta vermelho se falhar, e `DocumentoPartitura::marcar_salvo` (ticket 15) ao salvar. Vínculo: a tela liga o arquivo a um item da Oficina quando o caminho aberto é `<oficina>/partituras/<id>.musicxml` com `<id>.estado.toml` ao lado (é como a Revisão abre o item, via `abrir_caminho`); qualquer outro arquivo é solto. Abrir não escreve nada: a origem (hashes) é registrada pelo primeiro salvar, então antes dele a conferência é "não registrada" sem alerta. Testes egui sem janela em `tela_partitura::testes`. Pendência: a Revisão do tutor embute a vista só de leitura, a edição acontece na aba Partitura; não há botão "editar este item" entre as abas. NÃO VERIFICADO em janela real.


**Supervisor (2026-11-28, revisão independente):** reabertos. R3: salvar passa sempre `pai = None` e não há como partir de uma Versão antiga pela tela. R2: Exportar/Sobrescrever pode reescrever a Hipótese. Ver docs/evolucao-musical/e01/REVISAO-CODIGO.md.
