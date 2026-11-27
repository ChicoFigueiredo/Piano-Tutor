# 06: Prefactor: geometria do layout sem tipos do egui, com ligação nota a nota

**Fase:** E01

**R-IDs/critérios:** R-E03, R-A01; ponto de atenção do diagnóstico (layout acoplado ao egui, seleção só por compasso)

**Seam:** Seam de render (S1 só indireto); S4 intocado

**Modelo sugerido:** Sonnet / alto (Opus para revisar a fronteira)

**What to build:** O layout produz hoje primitivas desenhadas com tipos do egui e só liga cada primitiva a um compasso e a um intervalo de tempo. Este ticket cria uma camada geométrica própria (pontos, retângulos, cores, formas em sp) com um adaptador fino para o egui, e acrescenta ao vínculo de cada primitiva uma identidade de nota estável (um espaço reservado preenchido pelo documento no ticket 10), sem mudar o desenho na tela. Isso permite testar posições e hit test sem janela e é pré-requisito do spike de render. Expandir antes de contrair: o tipo antigo coexiste até a migração terminar.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] O desenho de todas as partituras embutidas, a 100% e 150%, é pixel a pixel igual ao anterior (teste de captura por golden de primitivas, não por imagem do SO).
- [ ] As posições calculadas pelo layout (cabeças, hastes, barras, linhas suplementares) passam nos testes geométricos existentes sem alterar valores esperados.
- [ ] O layout é calculável e inspecionável em teste sem criar contexto do egui; a conversão para egui fica num único adaptador.
- [ ] Cada primitiva de nota expõe uma identidade de nota opcional e há um hit test (ponto em sp → nota ou compasso) coberto por testes com tolerância definida; sem identidade, o comportamento antigo por compasso é mantido.
- [ ] A migração segue expandir-contrair: um PR de expansão (novo tipo ao lado do antigo), batches por área do layout, e o PR que remove o tipo antigo só quando nenhum chamador resta. Se o tamanho exigir, este ticket é o de expansão e os demais batches viram tickets filhos no próprio PR de planejamento.
- [ ] Sem aumento mensurável no tempo de layout de uma peça de 100 compassos (medição informal registrada; a baseline formal é o ticket 26).

**Notas:** Wide refactor (to-tickets): sequenciar expandir/migrar/contrair; a CI deve ficar verde em cada batch.
