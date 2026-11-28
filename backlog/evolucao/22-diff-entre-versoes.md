# 22: Diff entre duas Versões, por compasso, com antes e depois (A10)

**Fase:** E01

**R-IDs/critérios:** R-E02, R-B06; A10; história 18

**Seam:** S1

**Modelo sugerido:** Sonnet / médio

**What to build:** Dadas duas Versões, a Oficina lista os compassos alterados com o antes e o depois (notas, durações, alturas, vozes), e permite ir ao compasso na partitura. O diff é semântico, não de texto: formatação não gera diferença.

**Blocked by:** 21 (Salvar como nova Versão, com nota de motivo, e listar as Versões)

**Status:** feito, aguarda revisão

- [x] A10: dadas duas Versões com edições em compassos conhecidos, o diff devolve exatamente esses compassos e, para cada um, o antes e o depois.
- [x] Duas Versões idênticas dão diff vazio; Versões com formatação diferente e conteúdo igual dão diff vazio (comparação semântica).
- [x] O diff de Versões não consecutivas funciona (ancestral comum não necessário) e é simétrico em compassos alterados.
- [x] Diff por IdNota detecta nota alterada, inserida e removida num compasso (três casos testados); letra, cifra e dedilhado alterados por edição indireta aparecem com marcação.
- [x] A Oficina mostra o diff em lista navegável com teclado; clicar numa linha leva ao compasso; sem diferenças, mostra mensagem clara. (`desenhar_comparacao` em `tela_partitura.rs`: "Comparar" com antes/depois, padrão = Versão selecionada vs. pai; setas/Enter com a lista em foco; clique/Enter seleciona a primeira nota do compasso e rola até ela. Testes egui sem janela: `comparar_*`, `lista_do_diff_*`. Leitura do foco do teclado dentro da janela real: NÃO VERIFICADO além do quadro simulado.)
- [x] O diff é calculado pelo serviço, sem janela, e é reutilizável por MCP (E06) sem mudança de contrato.

**Supervisor (2026-11-27):** aceito o pareamento por `id` do MusicXML e por sequência comum como substituto provisório do IdNota, porque o IdNota ainda não é estável entre Versões. Quando o ticket 17 trouxer o arquivo lateral de IdNota, o diff deve parear por ele; isso ficou registrado no ticket 17. A lista na tela entra depois da ligação das Versões à tela. `#[allow(clippy::large_enum_variant)]` aceito.
