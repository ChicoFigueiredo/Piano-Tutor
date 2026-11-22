# App "Olá Piano Tutor": evidência de nuvem (F00, #26)

> Ambiente: **NUVEM / Xvfb** (container Claude Code remoto, Ubuntu 24.04, OpenGL por software). **NÃO é o cerebro** e não comprova a janela no KDE (X11/Wayland reais), nem avaliação humana de temas.

## O que foi executado

- `bancada-casca` da `bancada` pública, rev `94818fd95dc74b5a42a7757b1d258ed17822ed06`, como dependência git do crate `piano-tutor-app`: compilou no Linux sem credencial (gate de compilação, ADR-0004).
- Testes de unidade e "sem janela" (`egui::Context::run_ui` com entrada sintética): registro de ações, alternância de tema (Ctrl+T), Ctrl+F1 recolhe e expande o Ribbon, texto da barra de status, desenho em 3 temas × 3 modos de Ribbon sem pânico.
- Binário sob `xvfb-run` (tela 1280x800x24): abriu e permaneceu vivo por 4 s, sem saída de erro em stderr; encerrado por SIGTERM. Captura: [`app-nuvem.png`](app-nuvem.png) (Xvfb, `import -window root`).

## NÃO VERIFICADO

- Abrir a janela nativamente no KDE do cerebro (X11 e Wayland).
- Aparência dos temas Noite, Claro e Ébano avaliada por humano. Só os contrastes foram calculados (WCAG 2): texto/fundo >= 15:1 e texto suave/fundo >= 5,3:1 nos três temas.
- Ctrl+F1 e Ctrl+T com teclado real (o teste usa eventos sintéticos).
- Auditoria de licença das dependências transitivas (ver `docs/seguranca/LICENCAS-E-DADOS.md`).
