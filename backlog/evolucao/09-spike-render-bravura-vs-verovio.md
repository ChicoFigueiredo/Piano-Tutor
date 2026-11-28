# 09: Spike: render egui + SMuFL/Bravura contra Verovio como oráculo (fecha a pendência da ADR-0007)

**Fase:** E01

**R-IDs/critérios:** R-E03; história 31; A18 (viabilidade de desenhar letra/cifra/dedilhado)

**Seam:** Seam de render; oráculo externo

**Modelo sugerido:** Sonnet / alto para implementar; Opus decide o ADR

**What to build:** Um protótipo desenha, com layout próprio em egui e a fonte Bravura (SMuFL, OFL-1.1), as peças do corpus CC0 (piano em duas pautas e lead sheet) e compara o resultado com o que o Verovio produz a partir do mesmo arquivo, rodando Verovio apenas como processo separado, num script de desenvolvimento, nunca linkado e nunca na CI. A comparação mede o que dá para medir (posições relativas de cabeças e compassos, contagem de símbolos, colisões) e deixa a legibilidade para julgamento humano. O resultado fecha a pendência de render da ADR-0007 com um ADR de motor de render.

**Blocked by:** 06 (Prefactor: geometria do layout sem tipos do egui, com ligação nota a nota), 07 (Corpus de fixtures da E01 e harness de round-trip e comparação semântica)

**Status:** feito, aguarda revisão (legibilidade NÃO VERIFICADA até o titular assinar o checklist)

- [x] Fonte Bravura incluída com licença e aviso corretos (OFL-1.1) e registrada no inventário de licenças; Verovio não aparece em nenhuma dependência do Cargo (verificado).
- [x] O protótipo desenha todas as fixtures do corpus (inclusive letra, cifra e dedilhado como texto junto às notas, tercina, anacruse e repetição) sem pânico, em tamanhos mínimos e largura zero.
- [x] Um script de comparação roda o Verovio localmente como processo separado, gera SVG por peça e produz um relatório objetivo (posição relativa dos compassos por sistema, contagem de cabeças e hastes, colisões detectadas no layout próprio); rodar o script é opcional e NÃO VERIFICADO na CI.
- [x] Uma lista de lacunas do render próprio contra o oráculo é registrada por elemento (ligaduras, tercinas, vigas, acidentes, repetições, letra, cifra, dedilhado), com severidade e custo estimado.
- [x] Hit test por nota funciona no protótipo sobre o layout próprio e é medido (tempo por consulta numa peça de 100 compassos, apenas registrado).
- [x] Capturas de tela do protótipo e do oráculo ficam anexadas para o checklist visual humano; a legibilidade permanece NÃO VERIFICADA até o titular assinar.
- [x] O ADR de motor de render (numeração livre seguinte) registra a decisão, os critérios de troca (o que faria voltar atrás) e declara que a ADR-0007 é parcialmente superada nesse ponto; relayout incremental só se a baseline pedir (decisão adiada).

**Notas:** Verovio é LGPL: nunca linkado. Se a instalação do Verovio não for possível no ambiente do agente, registrar NÃO VERIFICADO e entregar o protótipo e o relatório do lado próprio.

**Resultado (implementação):** protótipo de teste em `crates/piano-tutor-app/src/partitura_visual/spike_bravura.rs` (sem mudar código de produção; golden do ticket 06 inalterado); Bravura 1.482 em `assets/fontes/bravura/` (versão e sha256 no `LEIA-ME.md`); script em `scripts/spike-render/`; relatório, capturas e checklist em `docs/evolucao-musical/e01/spike-render/`; decisão em `docs/decisoes/ADR-0018-motor-de-render-egui-bravura.md` (PROPOSTA até o titular assinar). Ressalvas dos critérios: o script do Verovio (pacote PyPI 6.4.1) foi rodado à mão, **NÃO VERIFICADO na CI**; capturas feitas por rasterização em software do que o egui tesselou (**GPU, HiDPI, Windows e macOS NÃO VERIFICADOS**); **a legibilidade permanece NÃO VERIFICADA** até o titular assinar `CHECKLIST-VISUAL.md`; custos da lista de lacunas são estimativas.
