# 25: Praticar a Versão com os Exercícios existentes, e ouvir o compasso editado (A14)

**Fase:** E01

**R-IDs/critérios:** R-L01 (parcial), T8; A14; histórias 24, 25, 26

**Seam:** S4 (entrada nova: Versão); S1

**Modelo sugerido:** Opus / médio (contrato entre a Versão e a prática existente)

**What to build:** O aluno pratica a Versão salva (tocar, esperar, nota a nota, loops, andamento) com os Exercícios e a prática existentes, usando as notas corrigidas. Uma Versão Aprovada pratica normalmente; uma Hipótese só pode ser praticada em modo claramente sinalizado. O revisor também ouve o compasso editado para conferir de ouvido.

**Blocked by:** 23 (Aprovar, rejeitar e reabrir uma Versão com motivo, ligadas ao hash (A9))

**Status:** ready-for-agent

- [ ] A14: com uma Versão Aprovada, a prática existente funciona com as notas corrigidas (teste por simulação de eventos MIDI sobre as fixtures de acertos e erros já existentes, S4).
- [ ] Uma Hipótese ou Versão não aprovada só abre em modo sinalizado como Hipótese; nunca aparece como Gabarito oficial (teste do rótulo e do bloqueio).
- [ ] A prática lê as notas do serviço de documento da Versão por hash; mudar a Versão invalida a sessão de prática anterior (teste).
- [ ] Ouvir o compasso editado toca as notas do compasso pelo serviço de som existente; o teste valida o plano de reprodução gerado; o som real e o REVAS físico são NÃO VERIFICADOS.
- [ ] Todos os testes de prática existentes continuam verdes sem mudança de assertivas.
- [ ] Prática com o REVAS físico (61 teclas), latência MIDI→áudio/MIDI→tela com a Oficina aberta e estabilidade prolongada ficam registrados como NÃO VERIFICADO no relatório do ticket.

**Notas:** O texto de aceite A14 é verificado por simulação em CI; a versão com teclado real depende do PC de referência.
