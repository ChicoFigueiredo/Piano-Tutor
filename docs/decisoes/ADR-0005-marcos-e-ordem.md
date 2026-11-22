# ADR-0005 — Marcos entregáveis e ordem de execução das fases

**Status:** ACEITA — 2026-11-22 · **Complementa:** [ADR-0001](ADR-0001-fatiamento-e-gates.md) e §7 da especificação

A numeração F00–F14 continua igual, para não quebrar issues e referências. A **ordem de execução** muda, e as fases passam a ser agrupadas em **marcos** (Marco), cada um entregue como pacote `.deb` instalável no cerebro:

| Marco | Fases, na ordem | O que você ganha |
|---|---|---|
| M1 | F00 → F01 → F02 → F03 | Tocar o REVAS com som, espelho visual e `.deb` |
| M2 | F04 → F08 → F06 → F05 | O app ensina: teoria, metrônomo, gravação e exercícios |
| M3 | spike de notação → F07 → F09 | Ler partitura e tocar com ritmo/pads |
| M4 | F10 → F11 → F12 → F13 | Fábrica de Lições a partir das Aulas |
| M5 | F14 | Homologação final e `.deb` de distribuição |

**Por quê:** o relógio mestre de áudio (ADR-005 da spec) só era medido em F08, mas F06 e F07 já dependiam dele. E F05 pedia replay do trecho e indicação de compasso, que só existem depois de F06/F08. Por isso: (1) **F03 cria o relógio mestre** e mede o jitter; (2) F08 vem logo depois de F04; (3) F06 vem antes de F05. O empacotamento mínimo `.deb` entra em F03, porque M1 precisa ser instalável. F07 entrega só leitura e exportação ([ADR-0007](ADR-0007-partitura-somente-leitura.md)).

**Consequências:** F03 também passa com qualquer **preset GM contrastante** do SF2 aprovado (por exemplo piano × órgão); o acordeon fica desejável, não obrigatório. F10 continua depois de F09.
