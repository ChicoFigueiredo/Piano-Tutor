# 14: Selecionar nota e mudar a altura com seta (semitom, oitava, escala)

**Fase:** E01

**R-IDs/critérios:** R-E02, R-E04, R-E01; A2, A3, A6, A12; histórias 5, 6, 7; D14

**Seam:** S1

**Modelo sugerido:** Opus / alto (primeiro comando: define o contrato `Comando`/`Resultado` do serviço)

**What to build:** Com uma nota selecionada, ↑/↓ muda a altura em semitom, Alt+Shift+↑/↓ pela escala (respeitando a armadura) e Ctrl+↑/↓ em uma oitava. A edição passa por um comando tipado do serviço de documento, que valida, marca só o compasso afetado como sujo, regenera só esse compasso e devolve um resultado com o modelo novo e avisos. A tela redesenha e pode exportar.

**Blocked by:** 08 (Spike: modelo híbrido de documento (IdNota + faixa de bytes), prova A1/A2 no corpus CC0), 10 (Abrir uma partitura na Oficina e exportar idêntico (serviço de documento, 1ª fatia)), 12 (Selecionar nota no render e navegar com setas e Tab, com foco visível), 13 (Perfil mínimo de atalhos remapeáveis, com Pânico e Ctrl+K intactos (A11))

**Status:** ready-for-agent

- [ ] A12: ↑, Alt+Shift+↑ e Ctrl+↑ produzem o mesmo resultado que o comando equivalente chamado direto no serviço (teste que compara os dois caminhos).
- [ ] A2: num compasso com duas vozes e uma nota errada, alterar a altura e exportar difere do original só nesse compasso; os outros compassos, créditos, lista de partes e elementos desconhecidos são idênticos byte a byte (teste golden em pelo menos três fixtures).
- [ ] A3: numa partitura com letra, cifra e dedilhado, editar a altura de uma nota e exportar mantém letra, cifra e dedilhado do compasso e dos demais associados às mesmas notas (comparação semântica).
- [ ] A6: numa partitura com anacruse, duas vozes, tercina e repetição, editar uma nota e exportar produz só a mudança autorizada e as consequências declaradas (comparação semântica); nenhuma duração é preenchida sem aviso.
- [ ] Alturas fora dos limites (MIDI 0–127 e extremos do teclado de referência) são recusadas com mensagem; falha não altera nada (propriedade: modelo antes = modelo depois).
- [ ] Alteração pela escala respeita a armadura e acidentes em vigor no compasso; os casos de enarmonia estão em tabela de testes.
- [ ] A nota selecionada permanece selecionada após o comando (mesma IdNota); a nota editada soa uma vez ao ser alterada se o som estiver ligado (som real NÃO VERIFICADO).
- [ ] Ligaduras de prolongamento e de expressão e vigas da nota são preservadas; se a edição as invalidar, aparece aviso e não há alteração silenciosa.

**Notas:** Undo ainda não existe; a tela pode manter um único nível de recuperação temporária até o ticket 15, mas sem prometer.
