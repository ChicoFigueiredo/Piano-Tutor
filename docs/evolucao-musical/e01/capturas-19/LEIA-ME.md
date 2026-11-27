# Capturas do ticket 19 (letra, cifra e dedilhado)

**Estado: NÃO REVISADO.** Estas imagens são para o titular olhar. **A legibilidade é NÃO VERIFICADA** até o checklist abaixo ser preenchido por uma pessoa. A impressão do agente que gerou as capturas não vale como aceite.

## O que são

PNGs rasterizados **em software** a partir do que o egui tesselou, pelo **mesmo caminho de desenho do app** (`desenhar_partitura`, com a fonte Bravura registrada como a casca faz na partida): sem janela, **sem GPU real, sem HiDPI, sem Windows ou macOS** (tudo isso NÃO VERIFICADO). Fundo branco, tinta escura, tema claro.

| Arquivo | O que mostra |
| --- | --- |
| `combinada-100.png` e `combinada-150.png` | Peça mínima inventada (`piano_tutor_fixtures::exibicao::COMBINADA`): voz com **três estrofes**, **cifras** (C, G7/B, Am, Bb9, F, C) e piano de duas pautas com **dedilhado** (inclusive a troca `1-2`). 100% e 150%. A dinâmica, o ornamento, a articulação, a fermata e os dois elementos desconhecidos dessa peça **não aparecem** (estão na lista da Oficina). |
| `schubert-heidenroeslein-100.png` | Lied real do corpus (OpenScore, CC0), **três estrofes**: o caso que, no spike do ticket 09, invadia a pauta de baixo. Só o topo da página. |
| `brahms-wiegenlied-100.png` | Lied real do corpus (OpenScore, CC0), **duas estrofes**, voz e piano. Só o topo da página. |

## Como reproduzir

```
PT_CAPTURAS_19=<pasta> CARGO_PROFILE_DEV_DEBUG=0 cargo test -p piano-tutor-app --lib gera_capturas_para_revisao_quando_pedido
```

Sem a variável `PT_CAPTURAS_19`, o teste não grava nada.

## O que mudou em relação ao spike (lacunas do `RELATORIO.md`, seção 4)

- **Lacuna 1 (letra, severidade alta):** cada estrofe tem uma linha própria, e a altura é **reservada** na pauta (abaixo de tudo o que a pauta já tem embaixo), então a pauta seguinte desce. A largura do texto entra no espaçamento horizontal (sílabas não se tocam, medido por estimativa de largura). **Ainda falta:** linha de prolongamento de melisma, elisão desenhada, alinhamento à esquerda da cabeça (hoje centrado).
- **Lacuna 2 (cifra):** a cifra fica acima do número do compasso e das casas de repetição (teste). **Ainda falta:** `♯` e `♭` como glifos (hoje `#` e `b`) e extensões em sobrescrito.
- **Lacuna 3 (dedilhado):** glifos SMuFL 1 a 5, do lado oposto da haste, com a troca `1-2`. **Ainda falta:** desvio de linha suplementar e de ligadura.

## Checklist visual do titular (preencher; item sem marca = não olhado)

- [ ] Letra de 1 estrofe é legível junto às notas.
- [ ] Letra de 2 e 3 estrofes é legível e **não invade a pauta de baixo** (`schubert-heidenroeslein-100`, `brahms-wiegenlied-100`, `combinada-100`).
- [ ] O hífen entre sílabas de uma mesma palavra é entendido; sílabas de notas vizinhas não se tocam.
- [ ] A cifra é legível, fica acima da pauta e não esconde o número do compasso (`combinada-100`).
- [ ] O dedilhado (1 a 5, e a troca `1-2`) é legível e fica junto da nota certa, sem cobrir a cabeça nem a haste.
- [ ] A 150% (`combinada-150`) nada fica cortado ou borrado.
- [ ] Ressalvas e o que mudar: ______________________

Assinatura e data do titular: ______________________
