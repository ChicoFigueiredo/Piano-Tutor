# 08: Spike: modelo híbrido de documento (IdNota + faixa de bytes), prova A1/A2 no corpus CC0

**Fase:** E01

**R-IDs/critérios:** R-E01, R-E02, R-A01; A1, A2 (prova), A6 (parcial); histórias 20, 32

**Seam:** S1

**Modelo sugerido:** Opus / alto (decisão que trava ADR e contrato de todo o serviço)

**What to build:** Um protótipo time-boxed prova (ou refuta) a hipótese do modelo híbrido: abrir um MusicXML atribuindo IdNota estável a cada nota, guardar para cada parte, compasso e nota a faixa de bytes de origem e um indicador de sujo, exportar copiando byte a byte as entidades não sujas e regenerando só o compasso sujo (reaproveitando filhos desconhecidos), tudo sobre o corpus CC0 inteiro. O resultado é uma decisão registrada: a hipótese vale, vale com ajustes (quais) ou não vale (alternativa), com evidência. O código do spike só é reaproveitado no ticket 10 se passar nos critérios abaixo.

**Blocked by:** 05 (Prefactor: separar o documento da partitura do estado da tela), 07 (Corpus de fixtures da E01 e harness de round-trip e comparação semântica)

**Status:** feito, aguarda revisão

- [x] A1 provado: abrir e exportar sem editar devolve bytes idênticos para 100% das fixtures do corpus; qualquer falha é listada com a causa (BOM, fim de linha, DTD, entidades, atributos) e a regra proposta.
- [x] A2 provado: alterar a altura de uma nota num compasso com duas vozes difere do original só nesse compasso; créditos, lista de partes, identificação, comentários e filhos desconhecidos são idênticos byte a byte; teste automatizado sobre pelo menos três fixtures diferentes.
- [x] Regeneração do compasso sujo mantém a ordem do esquema e os filhos desconhecidos do compasso; um teste prova que um elemento desconhecido dentro do compasso editado sobrevive.
- [x] Durações em Fracao exata: exportação com `divisions` igual ao MMC dos denominadores; um valor não representável vira erro explícito, nunca arredondamento (teste).
- [x] IdNota: monotônico, nunca reutilizado, atribuído na leitura; persistência pelo `id` do MusicXML quando existir, e senão por arquivo lateral; o spike mede se o arquivo lateral é necessário no corpus e conclui sim ou não.
- [x] As faixas de bytes só valem para o texto original: um teste mostra que aplicar patches de trás para frente, uma única vez, produz o texto esperado, e que uma segunda aplicação sobre o texto novo é recusada ou impossível por construção.
- [x] O ADR do modelo de documento (novo, numeração livre seguinte; ADR-0015 só é referenciada) é escrito com decisão, alternativas descartadas e evidência; riscos remanescentes listados.
- [x] Resultado do spike inclui estimativa de custo de manutenção (linhas, casos especiais) e uma recomendação explícita ao orquestrador: seguir com o ticket 10 ou replanejar.

**Notas:** Se a hipótese falhar, o ticket 10 e os seguintes são replanejados antes de qualquer outro trabalho de S1.

**Resultado (spike):** a hipótese **vale com ajustes** ([ADR-0017](../../docs/decisoes/ADR-0017-modelo-de-documento-hibrido.md), proposta com evidência em `docs/evolucao-musical/e01/evidencias/spike-08-medicao.txt`). A1: 16/16 fixtures e 192/192 variações adversas idênticas; A2: 1717/1717 edições de altura mudam só o compasso da nota. Não passa sem ajuste: mudar `divisions` suja a parte inteira (recusado por padrão; alternativa proposta e não implementada). Arquivo lateral de ids: desnecessário no corpus para edição de altura e duração (0 de 1804 notas com `id` no XML), necessário a partir de edição estrutural (ticket 17). **Recomendação: seguir com o ticket 10** (mover `documento` e `IdNota` para o núcleo; decisão do titular sobre o arquivo lateral antes do ticket 17). Código do spike em `crates/piano-tutor-fixtures/src/documento/`. A estimativa de custo (1356 linhas, 13 casos especiais) está na ADR.
