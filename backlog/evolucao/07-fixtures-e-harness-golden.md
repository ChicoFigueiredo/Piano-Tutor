# 07: Corpus de fixtures da E01 e harness de round-trip e comparação semântica

**Fase:** E01

**R-IDs/critérios:** R-E01, R-E02; Decisões de teste (corpus CC0, fixtures próprias, comparação semântica)

**Seam:** S1 (golden de round-trip, comparação semântica)

**Modelo sugerido:** Sonnet / médio

**What to build:** Existe um corpus de fixtures MusicXML de licença verificada, com as peças CC0 já presentes e fixtures próprias mínimas para cada caso crítico (anacruse, duas vozes numa pauta, tercina, repetição, letra, cifra, dedilhado, dinâmica, ornamento, um elemento sem suporte de renderização, uma peça de 100 compassos), mais um harness reutilizável que compara dois MusicXML byte a byte nas partes não editadas e semanticamente nas editadas. Todos os tickets seguintes escrevem seus testes sobre este harness.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Cada fixture tem licença e origem registradas num índice do corpus; nenhuma é material comprado (T2); as fixtures próprias são descritas como tais.
- [ ] O corpus contém, no mínimo, um arquivo para cada caso listado acima; o de 100 compassos é de licença verificada ou gerado por script versionado.
- [ ] O harness oferece: igualdade byte a byte, lista de compassos que diferem (por posição) e comparação semântica (notas, durações, vozes, letra, cifra, dedilhado) que ignora diferença de formatação.
- [ ] O harness tem testes próprios: duas cópias idênticas passam; uma alteração de altura num compasso é reportada exatamente nesse compasso; uma alteração fora dele reprova.
- [ ] Um teste de leitura (modelo atual) abre todas as fixtures sem erro e registra, num relatório, o que o modelo atual descarta por fixture (insumo para a lista de não suportados e para o spike de modelo).
- [ ] Fixtures de MXL válidas e de MXL maliciosas (zip bomb, caminho fora do escopo) existem como arquivos gerados por script, sem payload perigoso real.

**Notas:** O MusicXML do ticket 04 pode ser acrescentado depois como fixtures adicionais; este ticket não espera por ele.
