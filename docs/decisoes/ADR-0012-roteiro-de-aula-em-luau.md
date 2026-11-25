# ADR-0012 — Roteiros de Aula em Luau, dentro do Pacote de Curso

**Status:** PROPOSTA — 2026-11-25 · **Pedido do titular:** "cada vídeo é um desafio, crie módulos em forma de plugin com reuso, podendo ser em Lua para reuso". **Altera** o ADR-0009 (pacote como só dados).

## Contexto

Cada Aula do Luciano tem um fluxo próprio: em uma ele toca e pede para repetir, em outra fala bastante antes de tocar, em outra alterna mãos. Um formato só de dados (TOML) não descreve bem esses fluxos sem crescer a cada caso. O titular quer módulos reaproveitáveis, programáveis por Aula.

## Decisão

1. Cada Aula pode ter um **Roteiro de Aula**: um script **Luau** que conduz o Player de Aula (tocar trecho, esperar o aluno, repetir, avançar, sugerir Modo de acompanhamento, chamar falas do Roteiro). Módulos comuns ficam numa biblioteca compartilhada do Curso e são reaproveitados por `require`.
2. Os scripts **viajam no Pacote de Curso**, cobertos pela mesma assinatura ed25519 dos dados.
3. **Sandbox:** Luau via `mlua`, sem `io`, `os`, rede nem processos, sem `require` fora do pacote, com limite de memória e de instruções por chamada. A única saída do script é a **API do player**, versionada.
4. Sem Roteiro de Aula, o player usa o fluxo padrão (trechos dos Exercícios em ordem, Modo de acompanhamento do aluno).

## Alternativas descartadas

- **Lua 5.4 padrão:** exige montar o sandbox à mão, removendo bibliotecas; Luau já nasce para rodar scripts de terceiros.
- **Scripts só locais:** mantém o pacote só com dados, mas quem recebe o pacote fica sem os roteiros.
- **TOML declarativo:** cresce com um campo por caso e não reaproveita lógica.

## Consequências

- O pacote passa a levar código; a confiança vem da assinatura do autor do pacote e do sandbox. O leitor do pacote valida a versão da API pedida pelo script.
- A API do player precisa de testes de contrato, e o app precisa de um modo de depuração do Roteiro (log do script na tela).
- "Plugin" continua fora do glossário: o termo é Roteiro de Aula.
