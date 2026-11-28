# Prompt de kickoff — Piano Tutor
Trabalhe em PT-BR. Parta da branch de especificação criada a partir da main, sem mexer na main:
```bash
git fetch origin
git switch -c feat/evolucao-musical origin/docs/evolucao-musical-agentiva
```
Leia docs/evolucao-musical/README.md e todos os documentos referenciados, AGENTS.md, CONTEXT.md, specs, ADRs, matriz de aceite e código atual. O titular autorizou expressamente rever e substituir a limitação do ADR-0007: editor nativo Rust é requisito, não integrar editor pronto. Preserve regressões, gates físicos e privacidade.
Use as skills Matt Pocock instaladas: grill-with-docs + research para contestar esta proposta e pesquisar fontes primárias/repos/alternativas independentes; entregue matriz de evidências, benchmark reproduzível e perguntas/decisões ao titular. Não trate hipóteses como fatos. Com aprovação, use to-spec para reconciliar docs canônicas/ADRs e to-tickets para histórias verticais com critérios, dependências e DoD. Aplique tdd antes de implementar, implement-spec por fase autorizada e code-review independente. Não implemente tudo de uma vez. Primeiro entregue E00 e aguarde aceite antes de iniciar E01. Nunca envie partituras compradas ao git/CI/LLM sem autorização específica; nunca declare teste não executado como PASS. Relate bloqueios, riscos, divergências, alternativas rejeitadas e evidências por requisito.
