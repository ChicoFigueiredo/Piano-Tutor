# Plano de marcos e entregas — Piano Tutor v1

> Resultado da sessão de questionamento de 2026-11-22 (grill-with-docs). Decisões registradas em [ADR-0004](../decisoes/ADR-0004-somente-linux.md) a [ADR-0008](../decisoes/ADR-0008-aulas-somente-locais.md). Vocabulário em [CONTEXT.md](../../CONTEXT.md).

## Premissas

- **Plataforma única:** Linux. O PC de referência é o **cerebro** (Kubuntu 26.04, Ryzen 7 3700X, 128 GB, RTX 4080 SUPER 16 GB), com áudio onboard Realtek (B550) via PipeWire.
- **Shell:** `bancada-casca` do repositório `bancada`, que o titular vai tornar **público** com licença **MIT OR Apache-2.0**. Com isso a CI compila sem credencial. *Pendência do titular:* mudar a visibilidade e a licença no repositório `bancada`.
- **Distribuição:** cada marco gera um `.deb` instalável no cerebro.
- **Conteúdo de vídeo:** Aulas de tutores ficam só locais (ADR-0008).

## Marcos

| Marco | Fases (ordem de execução) | Demonstração de aceite | Gates de hardware |
|---|---|---|---|
| **M1 — Tocar com som** | F00 → F01 → F02 → F03 | Instalar o `.deb`, ligar o REVAS, ver as 61 teclas acendendo e ouvir piano e um segundo timbre contrastante com p95 ≤ 50 ms | F01, F02, F03 |
| **M2 — O app ensina** | F04 → F08 → F06 → F05 | Fazer um Exercício com metrônomo, errar de propósito, receber feedback com compasso e ouvir o replay da Gravação | — |
| **M3 — Ler e ritmo** | spike de notação → F07 → F09 | Abrir um MusicXML licenciado em duas claves, acompanhar o playback e tocar com os pads | — |
| **M4 — Fábrica de Lições** | F10 → F11 → F12 → F13 | Pegar uma Aula local, gerar Hipóteses, revisar, Aprovar como Lição e clicar "Praticar este trecho" | — (aceite local) |
| **M5 — Homologação** | F14 | 60 min de uso real, instalação limpa do `.deb` sem Python, checklist assinado | F14 |

## Mudanças nas fases

| Fase | Mudança |
|---|---|
| F00 | Linux no lugar de Windows; "repo separado" = este repositório; a CI compila a `bancada` pública |
| F01 | REVAS testado em USB nativo no Linux (ALSA/PipeWire) |
| F03 | Cria o **relógio mestre** e mede o jitter; empacotamento mínimo `.deb`; segundo timbre pode ser qualquer preset GM contrastante; checklist de escuta |
| F05 | Executada depois de F06 e F08; o replay usa a Gravação de F06 |
| F07 | Só leitura e exportação, depois do spike de notação (ADR-0007) |
| F08 | Executada logo depois de F04; reaproveita o relógio mestre de F03 |
| F10–F13 | Aulas só locais; CI com fixtures sintéticas (ADR-0008) |
| F11, F12 | Meta experimental não atingida libera a fase como **beta** (ADR-0006) |
| F14 | Somente Linux, `.deb`; sem Windows |

## Regras transversais

Ver [ADR-0006](../decisoes/ADR-0006-regras-de-gate.md): NÃO VERIFICADO bloqueia F01/F02/F03/F14; bug crítico definido; checklist de escuta para critérios auditivos.

## Como o Claude orquestra a entrega

Ver [Fluxo de trabalho](../equipe/WORKFLOW.md#orquestração-no-claude).
