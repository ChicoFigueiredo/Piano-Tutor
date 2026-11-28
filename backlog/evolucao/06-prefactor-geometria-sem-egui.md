# 06: Prefactor: geometria do layout sem tipos do egui, com ligação nota a nota

**Fase:** E01

**R-IDs/critérios:** R-E03, R-A01; ponto de atenção do diagnóstico (layout acoplado ao egui, seleção só por compasso)

**Seam:** Seam de render (S1 só indireto); S4 intocado

**Modelo sugerido:** Sonnet / alto (Opus para revisar a fronteira)

**What to build:** O layout produz hoje primitivas desenhadas com tipos do egui e só liga cada primitiva a um compasso e a um intervalo de tempo. Este ticket cria uma camada geométrica própria (pontos, retângulos, cores, formas em sp) com um adaptador fino para o egui, e acrescenta ao vínculo de cada primitiva uma identidade de nota estável (um espaço reservado preenchido pelo documento no ticket 10), sem mudar o desenho na tela. Isso permite testar posições e hit test sem janela e é pré-requisito do spike de render. Expandir antes de contrair: o tipo antigo coexiste até a migração terminar.

**Blocked by:** None (can start immediately)

**Status:** feito, aguarda revisão (critério de tempo de layout inconclusivo, ver abaixo)

- [x] O desenho de todas as partituras embutidas, a 100% e 150%, é pixel a pixel igual ao anterior (teste de captura por golden de primitivas, não por imagem do SO). (golden FNV de layout e malha tesselada, 5 partituras x 100%/150%, capturado no código antigo e idêntico no novo: `golden_de_primitivas_das_partituras_embutidas`)
- [x] As posições calculadas pelo layout (cabeças, hastes, barras, linhas suplementares) passam nos testes geométricos existentes sem alterar valores esperados.
- [x] O layout é calculável e inspecionável em teste sem criar contexto do egui; a conversão para egui fica num único adaptador. (`crates/piano-tutor-app/src/geometria.rs` + `partitura_visual/adaptador_egui.rs`)
- [x] Cada primitiva de nota expõe uma identidade de nota opcional e há um hit test (ponto em sp → nota ou compasso) coberto por testes com tolerância definida; sem identidade, o comportamento antigo por compasso é mantido. (`Layout::nota_em`/`acertar`, tolerância `TOLERANCIA_NOTA_SP`; `Item.nota` fica `None` até o ticket 10)
- [x] A migração segue expandir-contrair: um PR de expansão (novo tipo ao lado do antigo), batches por área do layout, e o PR que remove o tipo antigo só quando nenhum chamador resta. Se o tamanho exigir, este ticket é o de expansão e os demais batches viram tickets filhos no próprio PR de planejamento. (expansão e contração no mesmo PR: nenhum chamador externo usava as formas com tipos do egui, então não restou tipo antigo)
- [ ] Sem aumento mensurável no tempo de layout de uma peça de 100 compassos (medição informal registrada; a baseline formal é o ticket 26). NÃO CONCLUÍDO: medição informal e ruidosa (0,84 a 0,87 ms antes; 0,86 a 1,7 ms depois, 42 compassos, release, máquina compartilhada); não dá para afirmar ausência de aumento. Baseline formal é o ticket 26.

**Notas:** Wide refactor (to-tickets): sequenciar expandir/migrar/contrair; a CI deve ficar verde em cada batch.

**Supervisor (2026-11-28, após o ticket 26):** o ticket 26 mediu o valor absoluto do layout em 100 compassos: mediana de 0,156 ms (p95 0,177), em nuvem e em release. O critério acima continua aberto, porque a comparação com o commit anterior ao ticket 06 não foi feita e "sem aumento mensurável" segue NÃO COMPROVADO. Ele não bloqueia o uso, já que o tempo fica abaixo de 1% de um quadro, mas aparece como pendência no gate do ticket 27.
