# Checklist visual do spike de render (ticket 09)

**Estado: NÃO ASSINADO. A legibilidade é NÃO VERIFICADA até o titular preencher e assinar este checklist.** A impressão do agente que escreveu o protótipo não conta como aceite.

Como olhar: abra as capturas em `capturas/` (protótipo em cima, Verovio embaixo; tons de cinza; nomes `<peça>-<zoom>.png`). Para ver as páginas inteiras e os dois zooms, rode o script de comparação (ver `RELATORIO.md`, seção 7) e abra `comparacao/*.png`. Se possível, repita o teste de pintura **na janela real do app** (GPU, sua tela), que é o que as capturas não cobrem.

Marque cada item com `[x]` (aceito), ou escreva a ressalva ao lado. Item sem marca = não olhado.

## A. Legibilidade a 100% e 150%

- [ ] Cabeças, hastes, vigas e pausas distinguem-se com clareza (`brilha-brilha`, `beethoven-ode`, `faure-barcarolle`).
- [ ] Claves de sol e de fá, armadura e fôrma de compasso são reconhecíveis a 100% (`brahms-wiegenlied`, `faure-barcarolle`).
- [ ] A 150% nada fica borrado, cortado ou com traço grosso demais (`brilha-brilha-150`, `faure-barcarolle-150`).
- [ ] Acidentes (sustenido, bemol, bequadro) e pontos de aumento ficam ao lado da cabeça certa.
- [ ] Ligaduras (valor e expressão) deixam ver a que notas se ligam (`faure-barcarolle`, `brahms-wiegenlied`).

## B. Lead sheet e letra (a lacuna que mais pesa)

- [ ] Letra de 1 verso é legível junto às notas (`letra-100`).
- [ ] Letra de 2 ou 3 versos é legível? Esperado: **não** (invade a pauta de baixo em `schubert-heidenroeslein-100` e `brahms-wiegenlied-100`). Registre se o titular considera isso bloqueio para a E01 (ADR-0018, critério de troca 1).
- [ ] Cifra acima da pauta é legível e não esconde nada (`cifra-100`); anote se o encontro com o número do compasso incomoda.
- [ ] Dedilhado (dígitos junto às cabeças) é legível (`dedilhado-100`).
- [ ] Tercina (número e colchete) é entendida (`tercina-100`); anote o agrupamento 2+1 do modelo.
- [ ] Repetição (barras com pontos, casas 1 e 2) é entendida (`repeticao-100`).

## C. Comparação com o oráculo

- [ ] O protótipo é "bom o suficiente" como base do editor da E01, mesmo com as lacunas da seção 4 do `RELATORIO.md` (sim / não, e por quê).
- [ ] Em quais elementos o Verovio é claramente melhor e isso importa para a prática ao piano: ______________________
- [ ] Dinâmica, crescendo, ornamento e fermata ausentes no protótipo: aceitáveis para a E01 (sim / não).

## D. O que as capturas não mostram (testar na janela real, quando houver)

- [ ] Hinting e antisserrilhado dos glifos em GPU, a 100% e 150%: ______________________
- [ ] Tela HiDPI (`pixels_per_point` 1,5 ou 2): ______________________
- [ ] Windows: ______________________   macOS: ______________________

## Veredito

- [ ] **Aceito:** seguir com o layout próprio em egui + Bravura (ADR-0018 passa de PROPOSTA a ACEITA).
- [ ] **Aceito com ressalvas:** ______________________
- [ ] **Rejeitado:** acionar os critérios de troca da ADR-0018.

Titular: ______________________   Data: ____/____/______
