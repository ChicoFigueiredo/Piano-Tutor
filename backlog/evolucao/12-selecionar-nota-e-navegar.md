# 12: Selecionar nota no render e navegar com setas e Tab, com foco visível

**Fase:** E01

**R-IDs/critérios:** R-E03, R-E04 (mínimo), R-B06; histórias 3, 4, 28; T9

**Seam:** S1 (cursor e seleção por IdNota); seam de render

**Modelo sugerido:** Sonnet / médio–alto

**What to build:** Na Oficina, clicar numa nota a seleciona, com destaque visível e de alto contraste; as setas ← e → andam entre as notas na ordem de leitura e Tab troca de voz e pauta. Tudo funciona sem mouse, com foco visível. A seleção é guardada por identidade de nota (não por posição), de modo que sobrevive a uma edição.

**Blocked by:** 06 (Prefactor: geometria do layout sem tipos do egui, com ligação nota a nota), 09 (Spike: render egui + SMuFL/Bravura contra Verovio como oráculo (fecha a pendência da ADR-0007)), 10 (Abrir uma partitura na Oficina e exportar idêntico (serviço de documento, 1ª fatia))

**Status:** feito, aguarda revisão

- [x] Dado um clique sobre uma cabeça de nota, a nota correspondente fica selecionada e o serviço informa a mesma IdNota (teste de hit test sobre o layout e do estado do cursor, sem janela).
- [x] ← e → andam para a nota anterior e seguinte na ordem de leitura, inclusive entre compassos e acordes; nos limites, o cursor permanece e há aviso discreto (testado no serviço e na tela).
- [x] Tab alterna entre voz/pauta existentes e nunca para uma voz inexistente.
- [x] O foco do teclado é visível com contraste verificável por teste de cor (relação mínima documentada) e há opção de alto contraste; leitor de tela real é NÃO VERIFICADO.
- [x] Clique fora de nota seleciona o compasso (comportamento atual preservado) e não perde a seleção por engano.
- [x] Digitar em campo de texto da casca não move a seleção (a Oficina só recebe teclas quando está em foco).
- [x] Nenhuma edição é possível ainda; abrir e navegar não marca o documento como sujo.

## Notas da implementação

- Serviço sem janela: `crates/piano-tutor-app/src/selecao_partitura.rs` (`IndiceLeitura`, `SelecaoPartitura`, `Comando`, `Aviso`, `razao_de_contraste`). A tela (`tela_partitura.rs`) só traduz clique e teclas; a tabela de teclas é a função interna `comando_da_tecla`, que o ticket 13 substitui.
- Trilha = parte + pauta + voz. `←`/`→` andam na trilha em ordem de leitura (acorde do grave ao agudo, atravessando compassos); `Tab`/`Shift+Tab` trocam de trilha em ciclo, na nota mais próxima no tempo. Pausas não são selecionáveis; voz só de pausas não existe para o `Tab`. Com uma trilha só, o `Tab` não é capturado (segue o foco da janela).
- Seleção por `IdNota`; `reancorar` mantém a nota ao trocar o índice (edição do ticket 14) e limpa com aviso se ela sumir. Abrir outro documento limpa a seleção.
- Clique em nota seleciona a nota e não muda o compasso de partida; clique fora de nota alterna o compasso de partida (como antes) e mantém a nota; clique fora de tudo não muda nada.
- Foco: a vista só lê teclas com o foco do teclado (clique ou `Tab` da janela); `Esc` devolve o foco (sem armadilha de teclado). Ctrl/Alt/Cmd/Shift com setas nunca são tomados (Ctrl+K e Ctrl+. intactos, testado na casca).
- Contraste da seleção contra o papel (WCAG): normal >= 3:1 (critério 1.4.11); alto contraste >= 7:1. Medidos: Noite 6,59:1, Claro 8,2:1, alto contraste Noite 14,99:1, Claro 8,59:1. A preferência "Alto contraste na seleção" é gravada no perfil.
- NÃO VERIFICADO: leitor de tela real (há rótulo AccessKit e linha de texto com a nota selecionada, sem teste com leitor), aparência real em janela (anel, moldura de foco, rolagem até a nota) e clique de mouse físico (o clique foi testado pelo hit test do layout e pelo serviço, não por ponteiro sintético no egui).
