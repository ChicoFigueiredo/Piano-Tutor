# Tela Espelho: evidência de nuvem (F02, #34, parte de UI)

> Ambiente: **NUVEM / Xvfb** (container Claude Code remoto, OpenGL por software, sem gerenciador de janelas). **NÃO é o cerebro** e **não usa MIDI real**: toda nota da captura veio do teclado do PC (modo simulação), por eventos X sintéticos (XTest).

## O que foi executado

- Tela Espelho (`crates/piano-tutor-app/src/espelho.rs` e `tela_espelho.rs`): 61 teclas a partir de `Teclado61`, destaque das teclas ligadas em `EstadoTeclas`, painel "nota atual", zoom, nomes PT-BR/internacionais, convenção de oitava C4/C3, Dó central marcado, aviso permanente de simulação e aviso "faixa não calibrada" (faixa padrão de teste 36–96).
- Testes sem janela (`egui::Context::run_ui` com entrada sintética): clique em cada uma das 61 teclas liga a nota certa e soltar não deixa nota presa; tecla do PC liga e solta; 10 notas simultâneas destacadas e soltar uma preserva as outras 9; aviso de simulação presente; desenho em 3 temas; sair da tela solta a simulação; perda de foco solta a simulação.
- Captura [`espelho-nuvem.png`](espelho-nuvem.png): 1280x800, tema Noite, acorde Dó4 Mi4 Sol4 mais Dó♯4 e Fá♯4 e depois Lá4 (A, D, G, W, T, H pressionadas e mantidas), tecla ligada com cor, contorno grosso e marca de forma (círculo nas brancas, losango nas pretas).

## NÃO VERIFICADO

- Latência de evento MIDI recebido → destaque em tela, p95 ≤ 60 ms no PC de referência (nada de MIDI real nesta entrega; metodologia e medição são de outro ticket).
- 30 min de uso sem nota visualmente presa (inclui reconexão do dispositivo).
- Nenhum teclado físico, KB-330 ou REVAS usado; calibração real das 61 posições é da F01.
- Janela no KDE do cerebro (X11 e Wayland) e avaliação visual humana dos 3 temas.
- Mouse e teclado do PC com dispositivos reais (a captura e os testes usam eventos sintéticos).

## Limitações conhecidas

- O egui embutido não tem os glifos ♯ e ♭ (apareciam como quadrado): na tela os acidentes vão como `#` e `b`; o núcleo e o número MIDI não mudam.
- Teclas pretas ficam centradas na divisa das brancas (sem o pequeno deslocamento dos pianos reais).
- Não há tratamento dos KeyTips: com eles ativos (toque em Alt), uma letra pode valer como KeyTip e também como nota.
