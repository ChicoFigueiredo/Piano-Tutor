# 13: Perfil mínimo de atalhos remapeáveis, com Pânico e Ctrl+K intactos (A11)

**Fase:** E01

**R-IDs/critérios:** R-E04, T9; A11; histórias 29, 30

**Seam:** S1 (mapa de ação para comando), integração com a casca

**Modelo sugerido:** Sonnet / médio

**What to build:** A Oficina reconhece um perfil padrão (estilo MuseScore, mínimo): setas, Ctrl+setas, Alt+Shift+setas, dígitos 1–9, ponto, Shift+letra, Ctrl+Z, Ctrl+Shift+Z, Delete, Ctrl+Shift+K para cifra (reservada). O perfil é remapeável, o conflito é detectado e sinalizado, e Ctrl+K (paleta da casca), Ctrl+. (Pânico) e Esc nunca são capturados pela Oficina.

**Blocked by:** 12 (Selecionar nota no render e navegar com setas e Tab, com foco visível)

**Status:** feito, aguarda revisão

- [x] A11: com a Oficina em foco, Ctrl+. dispara o Pânico e Ctrl+K abre a paleta da casca exatamente como antes (teste de roteamento de teclas).
- [x] Esc segue a prioridade definida pela casca; um teste prova que a Oficina só trata Esc quando a casca o entrega.
- [x] Um mapa de ações é uma estrutura de dados (ação → combinação); remapear uma ação a muda sem reiniciar; o mapa é salvo no perfil do usuário e lido de volta.
- [x] Tentar atribuir Ctrl+K, Ctrl+. ou um atalho já usado é recusado com mensagem que cita o conflito (teste).
- [x] Redo é Ctrl+Shift+Z e cifra Ctrl+Shift+K; as ações ainda sem comando associado ficam registradas como pendentes (não somem em silêncio).
- [x] Cada ação do perfil tem um rótulo em PT-BR mostrado numa lista de atalhos acessível na Oficina.

**Notas:** Este ticket só entrega o mapa e o roteamento; os comandos de edição entram nas fatias seguintes.

**Implementação:** `crates/piano-tutor-app/src/atalhos_partitura.rs` (mapa ação → combinação, reservados, conflitos, texto do perfil); seção `tela.atalhos` do perfil SQLite (ADR-0010) via `retratos.rs`; roteamento e lista de atalhos (com "Mudar", "Sem atalho" e restaurar) em `tela_partitura.rs`. Conflito com a tela Teclado é sinalizado, não recusado (outra tela); Ctrl+K, Ctrl+., Esc e atalhos da casca são recusados. Ações sem comando (edição) ficam como pendentes e a tecla avisa. Teclado físico e leitor de tela: NÃO VERIFICADO.
