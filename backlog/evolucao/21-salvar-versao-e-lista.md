# 21: Salvar como nova Versão, com nota de motivo, e listar as Versões

**Fase:** E01

**R-IDs/critérios:** R-B06, R-E02, T8, D8; A8; histórias 15, 16, 17

**Seam:** S1

**Modelo sugerido:** Sonnet / alto (Opus revisa o formato de armazenamento)

**What to build:** O revisor salva o que fez como uma nova Versão, com nota opcional de motivo; cada Versão é imutável, identificada pelo hash do conteúdo, e ligada à Versão-pai, com autor e data. A lista de Versões mostra a Hipótese e a fonte como as primeiras, e as correções em sequência. A Hipótese e o arquivo original nunca são alterados. Nesta fase as Versões vivem no repositório de oficina; o Acervo (E02) as assume depois.

**Blocked by:** 14 (Selecionar nota e mudar a altura com seta (semitom, oitava, escala))

**Status:** feito, aguarda revisão — critérios reabertos pela revisão independente (R2, R3) corrigidos e fechados com teste; falta a re-revisão independente do gate 27 (item 3) e o aceite humano. NÃO VERIFICADO em janela real.

- [x] A8: dada uma Hipótese aberta, salvar duas correções cria duas Versões encadeadas (pai→filho) e o hash da Hipótese permanece idêntico antes e depois (teste).
- [x] Salvar nunca sobrescreve arquivo de Versão existente; salvar sem mudanças não cria Versão vazia (ou pede confirmação, decisão registrada). (decisão: sem mudança = não cria Versão, devolve `SemMudancas`)
- [x] Cada Versão registra hash de conteúdo, hash do pai, autor, data, nota de motivo e estado de conteúdo (nova Versão de edição nasce Em revisão); estados reaproveitam a máquina existente.
- [x] A lista de Versões mostra autor, data, nota e estado, e abrir uma Versão antiga a carrega em modo leitura ou como novo ponto de partida (decisão registrada). (modelo `linhas()`; "Abrir" = leitura; "Continuar a partir desta Versão" = a Versão vira o documento em edição e a próxima Versão salva é filha dela: `tela_partitura::testes_continuar` (7 testes: continuar de v1, da Hipótese, de uma Versão aberta em leitura, recusa com edição não salva, com rascunho pendente, Versão inexistente, quadro egui) e `testes_revisao::r3_*`)
- [x] Exportar nunca sobrescreve a Hipótese, o arquivo aberto, objetos de Versão nem o `.ids` lateral deles (comparação por caminho canonizado e, no Unix, por inode), e grava atômico (tmp+rename; sem "Sobrescrever", tmp+`hard_link`). (R2: `tela_partitura::testes_exportar` 7 testes; `documento_partitura::conferir_destino_de_exportacao`)
- [x] O `pai` de uma Versão salva é a Versão de onde o documento partiu (Hipótese reaberta = Hipótese; continuar de v1 = v1; mesma sessão = a última salva), nunca a ponta por default; cenário de duas sessões coberto. (R3: `testes_revisao::r3_duas_sessoes_*`, `Versoes::salvar_a_partir_de`)
- [x] Gravação atômica (arquivo temporário e renomear); queda no meio da gravação deixa a Versão anterior íntegra (teste de interrupção simulada).
- [x] O arquivo original (PDF e MusicXML de origem) é verificado por hash após qualquer ação da Oficina. (`verificar_originais()`; a tela deve chamá-lo após cada ação)
- [x] Material comprado nunca é copiado para fora da pasta de dados do usuário; o formato de armazenamento está documentado para a migração futura ao Acervo. (formato documentado no cabeçalho de `versoes.rs`)

**Notas:** Pergunta aberta: guardar as Versões no formato atual do repositório de oficina (estado em TOML + arquivos) ou já preparando o esquema do Acervo. Por padrão, o formato atual com histórico estendido.


**Supervisor (2026-11-27):** dois critérios reabertos porque só valem no módulo; a tela ainda não mostra a lista nem chama `verificar_originais()` após cada ação. A ligação da UI vai num passo seguinte, depois de 15 e 19. Também falta a trava entre processos: hoje dois salvamentos simultâneos podem perder uma entrada do índice.

**Ligação à tela (2026-11-28):** `tela_partitura.rs` (painel "Versões" na tela Partitura): botão "Salvar como Versão" com campo de nota (desativado com dica em arquivo solto ou em modo leitura), lista (`linhas()`: rótulo, autor, data, estado, nota), "Abrir" em modo leitura (o documento em edição fica guardado; "Voltar à edição"; editar, desfazer e salvar são recusados em leitura), `conferir_originais()` (`verificar_originais`) após editar, desfazer/refazer, salvar, abrir Versão e abrir partitura, com alerta vermelho se falhar, e `DocumentoPartitura::marcar_salvo` (ticket 15) ao salvar. Vínculo: a tela liga o arquivo a um item da Oficina quando o caminho aberto é `<oficina>/partituras/<id>.musicxml` com `<id>.estado.toml` ao lado (é como a Revisão abre o item, via `abrir_caminho`); qualquer outro arquivo é solto. Abrir não escreve nada: a origem (hashes) é registrada pelo primeiro salvar, então antes dele a conferência é "não registrada" sem alerta. Testes egui sem janela em `tela_partitura::testes`. Pendência: a Revisão do tutor embute a vista só de leitura, a edição acontece na aba Partitura; não há botão "editar este item" entre as abas. NÃO VERIFICADO em janela real.


**Supervisor (2026-11-28, revisão independente):** reabertos. R3: salvar passa sempre `pai = None` e não há como partir de uma Versão antiga pela tela. R2: Exportar/Sobrescrever pode reescrever a Hipótese. Ver docs/evolucao-musical/e01/REVISAO-CODIGO.md.


**Correção da revisão independente (2026-11-28):** R2 e R3 corrigidos, cada um com o teste que reproduz escrito antes (red registrado: 6 de 7 testes de `testes_exportar` e 4 dos 5 de `testes_revisao` falhavam; depois do código, verdes).
- R3: `salvar_versao` passa `d.base_de_ids()` a `Versoes::salvar_a_partir_de` (novo; `Pai::Partida`): a Versão de partida se está no índice, senão a Hipótese; `salvar(None)`/`salvar_com_ids(None)` seguem como "ponta" para quem chama a API antiga. A tela ganhou "Continuar a partir desta Versão" na lista (desativado com dica se há edição não salva ou rascunho pendente, para nada se perder em silêncio; edição não salva = salvar como Versão ou desfazer antes). Decisão registrada: abrir pela lista continua somente leitura; continuar é uma ação à parte.
- R2: a exportação recusa (com mensagem) destino igual ao arquivo aberto, à Hipótese do item vinculado, à Hipótese de qualquer item da Oficina (`partituras/<id>.musicxml` com `<id>.estado.toml`) ou a qualquer arquivo dentro de `<id>.versoes/`, e o `.ids` lateral de cada um; `gravar` confere o texto antes de gravar e grava por `gravar_atomico` (com Sobrescrever) ou tmp+`hard_link` (sem), e relê para comparar.
- Aceite dos critérios acima é de testes sem janela; Windows e macOS NÃO VERIFICADOS (a comparação por inode é só Unix; no Windows vale o caminho canonizado).
