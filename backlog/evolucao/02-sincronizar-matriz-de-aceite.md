# 02: Sincronizar a MATRIZ-DE-ACEITE com o progresso.json (só documentação)

**Fase:** Infra (E00, PR separado)

**R-IDs/critérios:** R-N03 (deriva documental, ponto de atenção do diagnóstico)

**Seam:** N/A (só docs)

**Modelo sugerido:** Haiku / baixo

**What to build:** A matriz de aceite e o arquivo de progresso deixam de divergir: cada fase F00–F14 tem na matriz o mesmo estado que o progresso registra, e a diferença que restar fica explicada. Este ticket só toca documentação e vai em PR próprio, separado de qualquer código (decisão do supervisor).

**Blocked by:** None (can start immediately)

**Status:** parcial: tabela de conciliação feita (docs/aceite/MATRIZ-DE-ACEITE.md); 8 divergências e 1 a confirmar aguardam decisão do titular. Coluna Estado da matriz não alterada.

- [x] Uma tabela de comparação (fase, estado na matriz, estado no progresso) mostra zero divergências não explicadas ao final.
- [x] Nenhum estado é promovido a PASS sem evidência já existente; o que não tem evidência permanece NÃO VERIFICADO, e itens de hardware (F01, F02, F03, F14) permanecem NÃO VERIFICADO.
- [x] O script de verificação de documentação existente passa.
- [x] O PR não contém nenhuma mudança fora de documentação.

**Notas:** Fonte do progresso: o arquivo de planejamento já versionado. Em caso de conflito entre os dois, não decidir: registrar a divergência e perguntar ao titular.
