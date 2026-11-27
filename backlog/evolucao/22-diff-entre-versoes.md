# 22: Diff entre duas Versões, por compasso, com antes e depois (A10)

**Fase:** E01

**R-IDs/critérios:** R-E02, R-B06; A10; história 18

**Seam:** S1

**Modelo sugerido:** Sonnet / médio

**What to build:** Dadas duas Versões, a Oficina lista os compassos alterados com o antes e o depois (notas, durações, alturas, vozes), e permite ir ao compasso na partitura. O diff é semântico, não de texto: formatação não gera diferença.

**Blocked by:** 21 (Salvar como nova Versão, com nota de motivo, e listar as Versões)

**Status:** ready-for-agent

- [ ] A10: dadas duas Versões com edições em compassos conhecidos, o diff devolve exatamente esses compassos e, para cada um, o antes e o depois.
- [ ] Duas Versões idênticas dão diff vazio; Versões com formatação diferente e conteúdo igual dão diff vazio (comparação semântica).
- [ ] O diff de Versões não consecutivas funciona (ancestral comum não necessário) e é simétrico em compassos alterados.
- [ ] Diff por IdNota detecta nota alterada, inserida e removida num compasso (três casos testados); letra, cifra e dedilhado alterados por edição indireta aparecem com marcação.
- [ ] A Oficina mostra o diff em lista navegável com teclado; clicar numa linha leva ao compasso; sem diferenças, mostra mensagem clara.
- [ ] O diff é calculado pelo serviço, sem janela, e é reutilizável por MCP (E06) sem mudança de contrato.
