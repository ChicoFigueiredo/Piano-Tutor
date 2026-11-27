# 29: Épico E03: Editor completo

**Fase:** E03

**R-IDs/critérios:** R-E01 (ampliação), R-E03, R-E04, R-E06, D2, D13

**Seam:** S1

**Modelo sugerido:** Opus / alto para o to-tickets do épico; os tickets filhos recebem modelo e effort próprios quando forem criados

**What to build:** **Épico (não quebrar agora).** Objetivo demonstrável: corrigir uma lead sheet e uma peça de piano com letra, cifra, dedilhado, ligaduras, tercinas, vozes e repetições, com o perfil de atalhos MuseScore e remapeamento.

**Decisões já tomadas na especificação:** novos comandos tipados no mesmo serviço de documento; matriz por elemento MusicXML (importar, renderizar, tocar, editar, exportar) publicada com 'não suportado' explícito; índices de letra e cifra revisados após edição, com órfãos sinalizados; escopo piano em duas pautas e lead sheet (D2), multi-partes fora.

Este ticket é um marcador de fase: quando seus bloqueios fecharem, o orquestrador roda `to-tickets` sobre ele e gera as fatias verticais (prefactors primeiro) antes de qualquer implementação.

**Blocked by:** 27 (Gate de encerramento da E01: matriz de rastreabilidade, ADRs e demonstração)

**Status:** ready-for-agent

- [ ] Dado um compasso com duas estrofes, alterar uma altura preserva ritmo, voz, sílabas e dedilhado salvo alteração explícita, e o undo restaura o modelo.
- [ ] Dada uma tercina criada, a soma do compasso fecha e a exportação reimporta igual.
- [ ] Dado conflito de atalho remapeado, ele é sinalizado.
- [ ] O round-trip do corpus crítico não tem perda silenciosa.
- [ ] Gate da fase: testes de polifonia, anacruse, ligaduras, tercinas, vozes e repetições; aceite do titular.
- [ ] NÃO VERIFICADO sem hardware: qualidade tipográfica percebida; leitor de tela real; macOS.
- [ ] Este épico só é considerado pronto quando seus tickets filhos (a criar) estiverem aceitos e o titular registrar o aceite humano da fase.
