# 26: Baseline de desempenho dos comandos numa peça de 100 compassos (A16)

**Fase:** E01

**R-IDs/critérios:** R-N02, T10; A16; história 27

**Seam:** S1 (medição); sem asserção de meta

**Modelo sugerido:** Haiku / médio (medição e documento); Sonnet se precisar de harness novo

**What to build:** Os comandos simples (alterar altura, duração, inserir, remover, acorde, undo, redo, abrir, exportar, layout, hit test) são medidos numa peça de 100 compassos do corpus CC0, e os números ficam registrados como baseline. A meta de 100 ms permanece 'a calibrar', nunca prometida nem rebaixada em silêncio.

**Blocked by:** 07 (Corpus de fixtures da E01 e harness de round-trip e comparação semântica), 18 (Acorde: adicionar e retirar alturas de uma nota)

**Status:** ready-for-agent

- [ ] A16: um benchmark reproduzível mede cada comando listado numa peça de 100 compassos e registra mediana, p95 e máximo, com versão do código, máquina e carga; o resultado é um documento no repositório.
- [ ] O benchmark não falha a CI por tempo (sem asserção de meta); ele só garante que roda (smoke).
- [ ] A comparação com a meta proposta (100 ms em 100 compassos) é registrada como observação, com o texto 'meta a calibrar'; nenhuma meta é alterada.
- [ ] O resultado distingue Linux de referência (PC de referência, NÃO VERIFICADO se medido em outra máquina) de outras máquinas; medição sob carga com áudio é NÃO VERIFICADA fora do PC de referência.
- [ ] Se algum comando ultrapassar 100 ms, abre-se um registro (não um conserto no mesmo ticket) com o perfil de onde está o custo.
