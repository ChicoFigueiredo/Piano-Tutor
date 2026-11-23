# F09 — Padrões de fábrica: autoria e licença (critério 6)

> Estado: **documentado; a confirmar pelo titular.** Registro de 2026-11-22, feito a partir do histórico do repositório e do código. Não é parecer jurídico.

## O que são

Os 11 padrões de `padroes_de_fabrica()` (`crates/piano-tutor-core/src/ritmo.rs`) são texto de passos em formato próprio (`.ritmo`, ver `Padrao::para_texto`): quais peças de bateria General MIDI soam em quais passos da grade de cada compasso. Não contêm áudio, samples, MIDI de terceiros nem partitura.

| # | Nome | Fórmula | Andamento | Compassos |
|---|---|---|---|---|
| 1 | Rock básico | 4/4 | 100 bpm | 1 |
| 2 | Pop balada | 4/4 | 76 bpm | 1 |
| 3 | Pop dançante | 4/4 | 118 bpm | 1 |
| 4 | Funk | 4/4 | 96 bpm | 1 |
| 5 | Reggae (one drop) | 4/4 | 76 bpm | 1 |
| 6 | Bossa nova | 4/4 | 128 bpm | 2 |
| 7 | Samba | 2/4 | 100 bpm | 2 |
| 8 | Baião | 2/4 | 95 bpm | 2 |
| 9 | Valsa | 3/4 | 90 bpm | 1 |
| 10 | Balada 6/8 | 6/8 | 58 bpm | 1 |
| 11 | Blues shuffle | 12/8 | 72 bpm | 1 |

O teste `ritmo::a_fabrica_tem_os_onze_padroes_documentados` (core) falha se a lista de nomes mudar sem esta tabela ser revista.

## Autoria

- Foram escritos dentro deste projeto: entraram no commit `c0a4edb` ("feat(core,app): ritmo ..."), de 2026-11-22, e o comentário no código diz "escritos aqui, sem material de terceiros".
- **Conteúdo original do projeto, a confirmar pelo titular.** Eu (agente) não consigo provar que cada grade não reproduz de memória uma transcrição publicada específica; o histórico mostra criação no projeto, nada mais.
- São esqueletos rítmicos curtos de estilos conhecidos (rock, pop, funk, reggae, bossa nova, samba, baião, valsa, balada em 6/8, shuffle de blues), de 1 a 2 compassos, sem melodia nem harmonia. Não imitam obra, gravação ou transcrição identificada. Não há nome de música, de artista nem de método nos arquivos.

## Licença

- O `README.md` (seção "Licenciamento, privacidade e segurança") diz que o projeto **não tem licença aberta atribuída** até decisão explícita do titular. Os padrões seguem essa regra: são autorais do projeto e não têm licença aberta própria. Distribuí-los a terceiros ou sob CC0/CC BY exige decisão do titular, registrada aqui.
- Os números de nota (36, 38, 42 e assim por diante) são a convenção General MIDI de percussão; não trazem conteúdo de terceiros.
- O **som** dos padrões depende do SoundFont carregado (FluidR3_GM, baixado na primeira execução, F03). A licença dele é tratada no registro da F03 e não foi reavaliada aqui. Nenhum SoundFont é versionado neste repositório.
- Nenhum sample de terceiros foi incorporado nesta fase.

## O que falta

- O titular confirmar a autoria (ou apontar qual padrão veio de outra fonte) e decidir se os padrões ganham uma licença aberta (por exemplo CC0 ou CC BY).
- Praticar de fato com o REVAS depende da calibração (F01) e de hardware: **NÃO VERIFICADO**.
