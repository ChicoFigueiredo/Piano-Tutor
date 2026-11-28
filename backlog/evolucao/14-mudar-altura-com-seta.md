# 14: Selecionar nota e mudar a altura com seta (semitom, oitava, escala)

**Fase:** E01

**R-IDs/critérios:** R-E02, R-E04, R-E01; A2, A3, A6, A12; histórias 5, 6, 7; D14

**Seam:** S1

**Modelo sugerido:** Opus / alto (primeiro comando: define o contrato `Comando`/`Resultado` do serviço)

**What to build:** Com uma nota selecionada, ↑/↓ muda a altura em semitom, Alt+Shift+↑/↓ pela escala (respeitando a armadura) e Ctrl+↑/↓ em uma oitava. A edição passa por um comando tipado do serviço de documento, que valida, marca só o compasso afetado como sujo, regenera só esse compasso e devolve um resultado com o modelo novo e avisos. A tela redesenha e pode exportar.

**Blocked by:** 08 (Spike: modelo híbrido de documento (IdNota + faixa de bytes), prova A1/A2 no corpus CC0), 10 (Abrir uma partitura na Oficina e exportar idêntico (serviço de documento, 1ª fatia)), 12 (Selecionar nota no render e navegar com setas e Tab, com foco visível), 13 (Perfil mínimo de atalhos remapeáveis, com Pânico e Ctrl+K intactos (A11))

**Status:** feito, aguarda revisão

- [x] A12: ↑, Alt+Shift+↑ e Ctrl+↑ produzem o mesmo resultado que o comando equivalente chamado direto no serviço (teste que compara os dois caminhos).
- [x] A2: num compasso com duas vozes e uma nota errada, alterar a altura e exportar difere do original só nesse compasso; os outros compassos, créditos, lista de partes e elementos desconhecidos são idênticos byte a byte (teste golden em pelo menos três fixtures).
- [x] A3: numa partitura com letra, cifra e dedilhado, editar a altura de uma nota e exportar mantém letra, cifra e dedilhado do compasso e dos demais associados às mesmas notas (comparação semântica).
- [x] A6: numa partitura com anacruse, duas vozes, tercina e repetição, editar uma nota e exportar produz só a mudança autorizada e as consequências declaradas (comparação semântica); nenhuma duração é preenchida sem aviso.
- [x] Alturas fora dos limites (MIDI 0–127 e extremos do teclado de referência) são recusadas com mensagem; falha não altera nada (propriedade: modelo antes = modelo depois).
- [x] Alteração pela escala respeita a armadura e acidentes em vigor no compasso; os casos de enarmonia estão em tabela de testes.
- [x] A nota selecionada permanece selecionada após o comando (mesma IdNota); a nota editada soa uma vez ao ser alterada se o som estiver ligado (som real NÃO VERIFICADO).
- [x] Ligaduras de prolongamento e de expressão e vigas da nota são preservadas; se a edição as invalidar, aparece aviso e não há alteração silenciosa.

**Notas:** Undo ainda não existe; a tela pode manter um único nível de recuperação temporária até o ticket 15, mas sem prometer.

**Implementação:** contrato no núcleo, sem GUI: `crates/piano-tutor-core/src/documento/comando.rs` (`ComandoDeEdicao`, `ResultadoDaEdicao` com `aplicado` e `inverso`, `AvisoDeEdicao`, `ErroDeEdicao`, `LimitesDeAltura`; `DocumentoAberto::aplicar`) e regras de grafia puras em `grafia.rs` (semitom, grau, oitava, acidente em vigor); a edição reutiliza a mecânica do spike (`Documento::definir_altura_e_acidente` em `edicao.rs`). App: `atalhos_partitura.rs` (seis ações de altura viram `Destino::Altura`, não mais pendentes), `tela_partitura.rs` (`mudar_altura`, mensagem, audição) e `documento_partitura.rs` (`aplicar` recalcula música e ordem de leitura). Testes: `crates/piano-tutor-fixtures/tests/documento_comando.rs` (A2, A3, A6, limites, tabela de enarmonia, ligaduras, inverso byte a byte, atomicidade) e os de `tela_partitura.rs` (A12: tecla == comando).

**Decisões (para revisão):**
- Grafia do semitom: 1) a que a armadura já altera, 2) tecla branca, 3) pela armadura (sustenido com sustenidos, bemol com bemóis; sem armadura, sustenido subindo e bemol descendo). Grau da escala: letra vizinha com a alteração da armadura. Acidente impresso só quando a alteração difere da em vigor no compasso (armadura ou último acidente da mesma letra e oitava na pauta); acidente que a nota já tinha e ainda vale fica como está. Fora do alcance: acidentes de compasso anterior e ligadura entre compassos; sétimo grau da menor harmônica (sempre acidente explícito).
- Limites: MIDI 0 a 127 e oitava 0 a 9 do MusicXML sempre (efetivo Dó0 a Sol9); além deles, o "teclado de referência" é o piano de 88 teclas (Lá0 a Dó8) por padrão (`LimitesDeAltura::PIANO_88`, configurável por `definir_limites`). O REVAS de 61 teclas tem faixa calibrada por instrumento e não foi usado como limite de partitura. Pedir confirmação do titular.
- Consequências declaradas, nunca alteradas em silêncio: nota em ligadura de prolongamento (agora liga alturas diferentes) e nota seguinte do compasso que dependia do acidente da editada. Ligaduras de expressão, vigas, letra, cifra e dedilhado não são tocados.
- O comando devolve o `inverso` (valor `DefinirAltura`); o desfazer é o ticket 15. Sem arquivo gravado: a edição vive em memória e a exportação continua sendo a ação manual do usuário (autosave e Versão são o ticket 21). A edição é recusada durante a reprodução e a prática.
- Correção necessária: a vista da partitura passou a capturar também ↑ e ↓ (`vertical_arrows`), senão o egui movia o foco para outro controle ao primeiro ↑.
- Som: a nota editada soa uma vez (400 ms) quando o som está pronto; som real no PC de referência e teclado físico: NÃO VERIFICADO.
