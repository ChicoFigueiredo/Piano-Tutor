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
- [ ] O arquivo original (PDF e MusicXML de origem) é verificado por hash após qualquer ação da Oficina. (`verificar_originais()`; a tela deve chamá-lo após cada ação)
- [x] Material comprado nunca é copiado para fora da pasta de dados do usuário; o formato de armazenamento está documentado para a migração futura ao Acervo. (formato documentado no cabeçalho de `versoes.rs`)

**Notas:** Pergunta aberta: guardar as Versões no formato atual do repositório de oficina (estado em TOML + arquivos) ou já preparando o esquema do Acervo. Por padrão, o formato atual com histórico estendido.


**Supervisor (2026-11-27):** dois critérios reabertos porque só valem no módulo; a tela ainda não mostra a lista nem chama `verificar_originais()` após cada ação. A ligação da UI vai num passo seguinte, depois de 15 e 19. Também falta a trava entre processos: hoje dois salvamentos simultâneos podem perder uma entrada do índice.
