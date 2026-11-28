# ADR-0018 — Motor de render da partitura: layout próprio em egui com a fonte Bravura

**Status:** PROPOSTA — 2026-11-27. Passa a ACEITA quando o titular assinar o [checklist visual](../evolucao-musical/e01/spike-render/CHECKLIST-VISUAL.md). Até lá, a legibilidade é **NÃO VERIFICADA**. · **Confirma:** [ADR-0015](ADR-0015-edicao-nativa-e-plataforma-musical.md) · **Supera parcialmente:** [ADR-0007](ADR-0007-partitura-somente-leitura.md) no ponto do render · **Evidência:** [relatório do spike](../evolucao-musical/e01/spike-render/RELATORIO.md) (ticket evolucao/09)

**Atualização do gate da E01 (ticket 27, 2026-11-28):** o status continua **PROPOSTA**: a legibilidade segue NÃO VERIFICADA e a aceitação é do titular. A evidência da implementação da E01 está na seção "Atualização do gate da E01" no fim deste arquivo. O ADR do modelo de documento é a [ADR-0017](ADR-0017-modelo-de-documento-hibrido.md); a rastreabilidade está na [matriz](../evolucao-musical/e01/MATRIZ-RASTREABILIDADE.md).

## Decisão

O render da partitura do editor continua sendo **layout próprio em Rust, desenhado pelo egui, com glifos da fonte SMuFL Bravura** (OFL-1.1). O Verovio **não** é motor de render: fica só como **oráculo de desenvolvimento**, em processo separado, rodado à mão por script, **nunca linkado, nunca na CI, nunca no pacote do app**.

Detalhes que valem:

1. **Fonte**: a Bravura 1.482 oficial (steinbergmedia/bravura), OTF e `Bravura.json` inteiros e **sem alteração**, com o texto da OFL, em `assets/fontes/bravura/` (versão, sha256 e origem no `LEIA-ME.md` de lá, e no inventário de licenças). A OFL reserva o nome "Bravura": um subconjunto seria versão modificada e teria de ser renomeado. Por isso embute-se o arquivo inteiro (889 KB). O aviso da OFL acompanha o app.
2. **Divisão de trabalho**: o layout (sistemas, espaçamento, vozes, vigas, empilhamento) segue em `partitura_visual/` e `geometria.rs`, sem tipos do egui (ticket 06). O que hoje é forma desenhada à mão (clave, armadura, fôrma, cabeça, pausa, acidente, bandeirola, ponto, número de tercina, ponto de repetição) passa a ser **glifo Bravura posicionado pela metadata** (caixas, âncoras e espessuras do `Bravura.json`). Hastes, vigas, linhas, barras e arcos permanecem geometria.
3. **Identidade e hit test**: cada cabeça carrega o `IdNota` na cena; o hit test é uma consulta sobre as caixas da cena (medido, ver evidência).
4. **Relayout incremental**: **adiado**. Só entra se a baseline de desempenho (ticket 26) mostrar que o relayout completo estoura o orçamento de um quadro.
5. **Regressão**: o golden de primitivas do ticket 06 segue passando sem alteração; o spike não mexe em código de produção.

## Contexto

A ADR-0007 mandava um spike de tempo limitado entre (a) layout próprio com Bravura e (b) Verovio, em ≥ 5 MusicXML, avaliando legibilidade a 100% e 150%, duas claves, desempenho, licença e esforço. A ADR-0015 já escolheu o caminho (a) por política (editor nativo, licença limpa, Verovio só como oráculo). Este spike **testa** essa escolha com um protótipo, em vez de só repeti-la.

## Evidência (o que foi medido; detalhes e tabelas no relatório)

- O protótipo desenha as 15 fixtures do corpus (inclusive letra, cifra, dedilhado, tercina, anacruse e repetição) pelo egui de verdade, sem pânico, em zoom de 0,5 a 3, larguras de 0 a normal e escalas degeneradas. A cabeça da Bravura chega ao egui dentro da caixa da metadata (teste).
- Contra o Verovio 6.4.1 (processo separado), cabeças e hastes **coincidem em todas** as peças comparadas (por exemplo, 663 e 493 no Fauré, 400 no `cem-compassos`); pausas, acidentes, pontos e linhas suplementares quase sempre; as diferenças são vigas (37 contra 45 no Schubert, 120 contra 136 no Fauré), ligaduras, sílabas, e o que o modelo atual descarta (dinâmica, ornamento, fermata). A posição das cabeças dentro do compasso difere em média de 4% a 9% da largura do compasso: diferença de espaçamento, não erro.
- Desempenho (release, só registrado): layout, cena e pintura de página inteira entre cerca de 1 e 12 ms nas peças medidas (42 compassos no Fauré: 6,8 ms de layout e cena, mais 4,6 ms de pintura); hit test de 0,55 µs por consulta na peça de 100 compassos (5,1 µs em depuração). O Verovio levou de 10 a ~180 ms por peça (carga mais SVG), medido dentro do processo Python, **sem** custo de partida de processo nem de transferência.
- Esforço: o protótipo (~1.900 linhas) reaproveitou o layout e a geometria existentes. Fechar as lacunas contra o oráculo é estimado em **20 a 30 dias de agente com revisão** (estimativa, não medida; tabela por elemento no relatório).

## Lacunas conhecidas (resumo; severidade e custo no relatório)

- **Alta**: letra. Sem reserva de espaço vertical (2 ou 3 versos invadem a pauta de baixo no Schubert e no Brahms), sem largura de sílaba no espaçamento, sem prolongamento de melisma. É problema de layout vertical, que o Verovio resolve e qualquer layout próprio teria de resolver.
- **Média**: cifra (encosta no número do compasso), ligaduras (espessura constante), agrupamento de vigas (regra herdada; na tercina o modelo agrupa 2+1), dinâmica, crescendo, pedal, ornamento, fermata e andamento (o modelo atual descarta; ticket 08 e E03).
- **Baixa**: dedilhado (sem substituição nem dedo por nota de acorde), tercina (colchete sem inclinação), repetição (sem segno, coda, D.C.), baixo cifrado, apojatura, ponto em acorde com segunda.

## Critérios de troca (o que faria voltar atrás)

Reabrir esta ADR e reavaliar o Verovio (ou outro motor) se **qualquer** um acontecer:

1. O titular **rejeita a legibilidade** no checklist e a causa não se resolve com as lacunas de severidade alta e média da lista (em particular, o passe de layout vertical da letra e da cifra falha em peça de 3 versos).
2. O custo restante das lacunas de severidade alta, depois de um corte real (não estimado) feito na E01, passa de **o dobro** da estimativa do relatório.
3. A baseline do ticket 26 mostra que o relayout completo passa de **um quadro (16 ms)** na peça-alvo e o relayout incremental se mostra inviável.
4. A OFL ou outra licença da cadeia (Bravura, egui) mudar de modo que impeça embutir o arquivo.

**Se for trocar:** o Verovio só entra como **processo separado** (LGPL-3.0), com SVG e mapa de `id`s para o `IdNota`, e exigiria rasterizar SVG no egui (dependência nova a aprovar). Linkar o Verovio continua proibido (política de licenças).

## Consequências

- A E01 segue com um layout só, que o app já usa; não existe segundo caminho de render para manter.
- O render ganha a Bravura inteira (889 KB no binário) e o aviso da OFL na distribuição. O inventário de licenças foi atualizado.
- O script de comparação com o Verovio (`scripts/spike-render/`) fica como ferramenta de desenvolvimento **opcional**; sua execução é manual e **NÃO VERIFICADA na CI** (o pacote do Verovio é LGPL-3.0 e não entra em nenhuma dependência do Cargo; um teste confere isso).
- A lista de lacunas vira insumo dos tickets de render da E01 e da E03; nenhuma dela foi corrigida aqui (fora do escopo do spike).

## Limites desta decisão (o que NÃO foi verificado)

- **Legibilidade**: só o titular julga; as capturas estão em `capturas/` e o checklist está em branco.
- **GPU real, HiDPI, Windows e macOS**: não testados. As capturas são o que o egui tesselou, rasterizado em software no teste.
- **Colisões**: contadas por caixa (superestimam, sobretudo bandeirola contra cabeça); servem para apontar onde olhar, não como medida de qualidade.
- **Verovio**: só o pacote Python 6.4.1, em processo próprio; o custo de partida do processo e a transferência do SVG não foram medidos.
- **Estimativas de esforço**: são do agente, não medidas.

## Atualização do gate da E01 (ticket 27, 2026-11-28)

O status **continua PROPOSTA** e a legibilidade **continua NÃO VERIFICADA**: nenhum dos dois checklists visuais foi preenchido (o do spike em `spike-render/CHECKLIST-VISUAL.md` e o do ticket 19 em `capturas-19/LEIA-ME.md`). Esta seção só acrescenta o que a E01 construiu e mediu depois do spike. Rastreabilidade na [matriz](../evolucao-musical/e01/MATRIZ-RASTREABILIDADE.md); o modelo de documento está na [ADR-0017](ADR-0017-modelo-de-documento-hibrido.md).

### O que a E01 confirmou da decisão

| Ponto da decisão | Evidência da E01 | Matriz |
|---|---|---|
| Layout e geometria sem tipos do egui | Camada própria (`geometria.rs`) com adaptador único para o egui; o golden de primitivas das 5 partituras embutidas a 100% e 150% passa idêntico ao capturado antes da migração (ticket 06). | T06.1 a T06.4 |
| Identidade de nota e hit test | `Item::nota` é preenchido (cabeça, ponto de aumento, acidente); `Layout::nota_em` e `acertar` com tolerância definida; clique numa cabeça seleciona a mesma `IdNota` do serviço (tickets 10 e 12). Medido: 0,55 µs por consulta no spike; mediana 1,5 µs na peça de 100 compassos no ticket 26 (release, VM de nuvem). | T06.4, T12.1, T26.1 |
| Lacuna de severidade alta: letra | **Fechada em parte** (ticket 19): `calcular_layout_com_exibicao` reserva uma linha de altura por estrofe e a pauta seguinte desce; a largura da sílaba entra no espaçamento; a cifra fica acima do número do compasso; o dedilhado usa glifos SMuFL (`partitura_visual/bravura.rs`, fonte registrada em `app::config`). O caso do Schubert com 3 estrofes, que invadia a pauta de baixo no spike, passou a ter teste (`tres_estrofes_reservam_uma_linha_cada_e_nao_invadem_a_pauta_de_baixo`). **Ainda falta:** linha de prolongamento de melisma, elisão desenhada, alinhamento à esquerda da cabeça, `♯` e `♭` como glifos na cifra. | A18, T19.1 |
| Foco e seleção acessíveis | Contraste da seleção contra o papel medido (WCAG): normal ≥ 3:1 e alto contraste ≥ 7:1; valores: Noite 6,59, Claro 8,2, alto contraste 14,99 e 8,59. Leitor de tela real: NÃO VERIFICADO. | T12.4 |
| Caminho sem exibição intacto | O golden do ticket 06 não foi alterado pelo ticket 19 (teste de igualdade `sem_exibicao_ou_com_exibicao_vazia_o_layout_e_o_de_antes`). | T19.1 |

### Critérios de troca (reavaliados)

1. **Legibilidade rejeitada pelo titular:** **não avaliável**, o checklist está em branco. É o que impede a ADR de virar ACEITA.
2. **Custo das lacunas de severidade alta depois de um corte real:** **não medido.** Só a letra foi atacada (parcialmente); a estimativa de 20 a 30 dias de agente do spike não foi reconferida.
3. **Baseline do ticket 26 acima de um quadro (16 ms):** **não disparou nesta VM.** Layout de 100 compassos: mediana 0,15 a 0,17 ms (p95 até 0,26 ms), com exibição 0,156 ms; hit test 1,5 µs. O relayout incremental continua **adiado**. Ressalvas: é uma VM de nuvem ocupada, sem GPU, medindo só o cálculo do layout (não a pintura egui); a peça sintética não tem letra, cifra nem dedilhado (o número com letra densa é NÃO VERIFICADO); e a comparação com o layout anterior ao ticket 06 nunca foi feita ("sem aumento mensurável" é NÃO COMPROVADO).
4. **Licença:** sem mudança; a Bravura 1.482 e a OFL seguem em `assets/fontes/bravura/`; Verovio não está em nenhuma dependência do Cargo (teste `verovio_nao_e_dependencia_do_cargo` passa).

### O que continua NÃO VERIFICADO

Legibilidade (capturas 09 e 19, em branco); GPU real, HiDPI, Windows e macOS (as capturas são rasterização em software do que o egui tesselou); desempenho no PC de referência e a pintura em janela; o script do Verovio na CI (manual por definição); estimativas de esforço (do agente, não medidas).
