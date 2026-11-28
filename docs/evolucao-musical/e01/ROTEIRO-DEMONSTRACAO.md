# Roteiro de demonstração da E01 para o titular

Para o PC Linux "cerebro". Duração estimada: 20 a 30 minutos. Este roteiro **prepara** a demonstração; ele não foi executado numa janela real por quem o escreveu (não havia display). Tudo que o roteiro mostra pela primeira vez numa janela real aparece na seção NÃO VERIFICADO da [matriz de rastreabilidade](MATRIZ-RASTREABILIDADE.md). Se algo não se comportar como descrito, anote o passo e o que viu: não é para "dar um jeito".

Atalhos: os do mapa padrão em `crates/piano-tutor-app/src/atalhos_partitura.rs`. Dentro do app, a lista completa e o remapeamento ficam em "Atalhos de teclado da Oficina" (tela Partitura).

## 0. Preparação (terminal, 3 minutos)

```bash
cd ~/Piano-Tutor
cargo run --release -p piano-tutor-app
```

O app lê as oficinas em `~/.local/share/piano-tutor/fontes/<tutor>/oficina/`. Use uma partitura que já esteja lá. Se não houver nenhuma, monte uma oficina de ensaio (só material CC0 do repositório; nada comprado):

```bash
OF=~/.local/share/piano-tutor/fontes/ensaio/oficina
mkdir -p "$OF/partituras" "$OF/paginas"
cp assets/partituras/beethoven-ode-a-alegria-tema.musicxml "$OF/partituras/ode.musicxml"
cp docs/evolucao-musical/e01/capturas-19/brahms-wiegenlied-100.png "$OF/paginas/ode.png"   # qualquer PNG serve de "página do PDF"
printf 'id = "ode"\ntitulo = "Ode à Alegria (ensaio)"\nestado = "hipotese"\nproveniencia = "ensaio da demonstração, CC0"\npagina_png = "paginas/ode.png"\n' > "$OF/partituras/ode.estado.toml"
```

Anote o hash da Hipótese **antes** de mexer em qualquer coisa (troque o caminho se usar outra partitura):

```bash
sha256sum ~/.local/share/piano-tutor/fontes/*/oficina/partituras/*.musicxml
```

Guarde a linha da partitura da demonstração. Ela é a prova do passo 9.

## 1. Abrir com o PDF ao lado (R5 corrigido em 2026-11-28)

**Como deve ficar (R5 corrigido em 2026-11-28, em implementação por outra equipe):** na tela **Partitura**, abra o arquivo da Oficina (passo 2) e a coluna **Página do PDF** aparece ao lado da partitura editável, com ◀ ▶, "Ajustar", "100%" e Ctrl+roda para zoom. Clicar num compasso leva o PDF à página dele; sem mapa de compassos o painel diz que o mapa é por página; com região mapeada, destaca a região. Os passos 3 a 10 se fazem nessa mesma tela, com o PDF sempre à vista.

**Enquanto R5 não entrar (como é hoje, pelo código):** o PDF ao lado só existe na tela **Revisão do tutor** (grupo Tutor, aba Partituras, escolher a oficina e a partitura; a vista ali é somente leitura). Mostre o PDF ali, depois vá ao passo 2 e alterne de tela para editar. Nesse caso o "ao lado" se perde na edição.

**Observe:** o PDF não muda de página sozinho e a partitura continua legível a 100% e 150%. O destaque, a rolagem e o zoom do PDF numa janela real nunca foram vistos (A17.b).

## 2. Abrir a mesma partitura no editor

1. Menu da esquerda, item **Partitura**.
2. No campo "/caminho/arquivo.musicxml" cole o caminho completo de `.../oficina/partituras/<id>.musicxml` e clique **Abrir arquivo** (ou Enter).
3. Só um arquivo dentro de `<oficina>/partituras/` com o `<id>.estado.toml` ao lado liga a partitura à Oficina; qualquer outro arquivo é "solto" e o botão "Salvar como Versão" fica desativado com uma dica.
4. Abrir **não grava nada** (nenhum rascunho, nenhuma Versão).

## 3. Selecionar e navegar

| Ação | Como |
|---|---|
| Selecionar nota | Clique na cabeça da nota (o anel de seleção aparece) |
| Dar foco à partitura | Clique nela. Esc devolve o foco à janela |
| Nota anterior / seguinte | ← e → (atravessam acordes e compassos) |
| Trocar voz ou pauta | Tab e Shift+Tab (só quando existe mais de uma) |
| Alto contraste da seleção | Caixa "Alto contraste na seleção" |

Nos limites o cursor fica e aparece um aviso discreto. Clique fora de qualquer nota escolhe o compasso de partida, sem perder a nota selecionada.

## 4. Corrigir altura e duração

Altura (nota selecionada, partitura em foco):

| Tecla | Efeito |
|---|---|
| ↑ / ↓ | semitom |
| Alt+Shift+↑ / ↓ | um grau da escala (respeita a armadura) |
| Ctrl+↑ / ↓ | uma oitava |

Duração (teclas de 1 a 8; a 5 é a semínima): 1 semifusa, 2 fusa, 3 semicolcheia, 4 colcheia, 5 semínima, 6 mínima, 7 semibreve, 8 breve. `.` liga e desliga o ponto de aumento. A tecla 9 (longa) é recusada, com explicação.

**Observe:**
- Encurtar uma nota cria pausa exata depois dela, com aviso visível. Alongar só cresce sobre pausas que seguem a nota.
- Alongar uma nota num compasso cheio é **recusado** com mensagem em português e nada muda (critério A4). Experimente.
- Altura fora do piano de 88 teclas (Lá0 a Dó8) é recusada com mensagem. A faixa de 88 teclas é provisória e espera a sua decisão.
- Se o som estiver ligado, a nota editada soa uma vez. **Este som é a primeira vez que ele é ouvido em áudio real:** diga se ouviu.

## 5. Remover a nota falsa

Selecione a nota que o OMR inventou e aperte **Delete**. Ela vira pausa de mesma duração e posição; as outras notas do compasso não se movem. Letra, dedilhado ou ligadura que estavam nela aparecem como aviso, nunca somem em silêncio.

## 6. Inserir pausa e acorde

Inserir (cursor de entrada, marcado na partitura):

| Tecla | Efeito |
|---|---|
| Clique num compasso | leva o cursor à coluna mais próxima |
| Ctrl+← / Ctrl+→ | cursor anterior / seguinte (pausas incluídas) |
| Alt+↑ / Alt+↓ | cursor na voz ou pauta de cima / de baixo |
| A a G | insere a nota (oitava mais próxima da última da voz) com a duração atual |
| 0 | insere pausa |

Inserir onde não há espaço é recusado com mensagem. Inserir pausa sobre nota é recusado e pede a remoção.

Acorde (nota selecionada):

| Tecla | Efeito |
|---|---|
| Shift+letra (A a G) | acrescenta essa altura (a mais próxima) ao acorde |
| Alt+2 a Alt+9 | acrescenta o intervalo (segunda a nona) acima |
| Ctrl+Delete | retira a altura selecionada (a última vira pausa) |

Altura repetida (inclusive enarmônica) é recusada.

## 7. Desfazer e refazer

Ctrl+Z desfaz e Ctrl+Shift+Z refaz, quantas vezes quiser. A seleção volta para a nota do gesto. Desfaça tudo: a partitura deve voltar ao que era no passo 2. Refaça tudo.

## 8. Fechar sem salvar e recuperar

**Atenção (R1, P0 da revisão, em correção):** não salve uma Versão entre a queda e o "Recuperar" do passo 4: hoje isso apaga o rascunho pendente. Recupere primeiro. Em tempo, não use "Exportar" com "Sobrescrever" sobre o arquivo da Hipótese (R2) e lembre que o "pai" das Versões pode estar errado (R3) até as correções.

1. Faça duas ou três edições e **espere mais de 30 segundos** sem editar (o autosave padrão é de 30 s e só grava com edição).
2. Feche a janela sem salvar. Para simular queda de verdade, em outro terminal: `pkill -9 -x piano-tutor-app`.
3. Abra o app de novo, vá em **Partitura** e abra o mesmo arquivo da Oficina.
4. Aparece "Há trabalho não salvo de uma sessão anterior" com **Recuperar** e **Descartar**. Recupere.

**Observe:** o documento volta como **modificado**, o histórico de desfazer foi perdido (aviso explícito) e você perdeu, no máximo, o que fez nos últimos 30 s. Este é o teste de queda real que ainda não existe na matriz (A7.b).

## 9. Salvar Versão e conferir a Hipótese byte a byte

1. No painel **Versões**, escreva um motivo em "nota de motivo (opcional)" e clique **Salvar como Versão**. A Versão aparece na lista com autor, data, nota e estado **Em revisão**.
2. Salvar de novo sem mudanças **não** cria Versão nova.
3. No terminal, repita o `sha256sum` do passo 0 e compare com a linha que você guardou:

```bash
sha256sum ~/.local/share/piano-tutor/fontes/*/oficina/partituras/*.musicxml
```

**Esperado: o hash da Hipótese é idêntico ao do passo 0.** As Versões ficam em `<oficina>/partituras/<id>.versoes/objetos/<hash>.musicxml`, com o hash no nome:

```bash
ls ~/.local/share/piano-tutor/fontes/*/oficina/partituras/*.versoes/objetos/
sha256sum ~/.local/share/piano-tutor/fontes/*/oficina/partituras/*.versoes/objetos/*.musicxml
```

O hash que o `sha256sum` imprime tem de ser o do nome do arquivo. O PDF/PNG de origem também não pode mudar de hash. Se aparecer um alerta vermelho de "original alterado", pare e anote.

Opcional, ainda neste painel: **Comparar** (padrão: a Versão selecionada contra a anterior) lista os compassos alterados com antes e depois; ↑/↓ e Enter navegam, e um clique leva ao compasso.

## 10. Aprovar e praticar

1. Escreva o "Motivo da decisão" (obrigatório, até 280 caracteres) e clique **Aprovar** na linha da Versão. Só o humano aprova: nada aprova sozinho. Há também Rejeitar, Reabrir e, na Hipótese, **Aprovar direto** (fica marcado como direto).
2. Clique **Praticar esta Versão**. Aprovada vale como Gabarito. Uma Versão Em revisão só pratica depois de "Praticar como Hipótese" e aparece o selo **HIPÓTESE, não é o Gabarito oficial**. Rejeitada não pratica.
3. Toque no teclado (os quatro modos da prática de sempre) e confira que vale a nota **corrigida**.
4. **Ouvir compasso** toca só o compasso em foco, para conferir de ouvido. Som real: também novo. Diga se ouviu.
5. **Voltar à edição** devolve o documento em edição.
6. Contraprova: edite de novo a Versão aprovada e salve. A nova Versão nasce **Em revisão** e a aprovação da anterior não vale para ela (critério A9).

## O que esta demonstração ainda não prova

Ao final, marque o que você viu. Cada item vira PASS na matriz **só depois** do seu registro aqui:

| Item | Visto? | Observação |
|---|---|---|
| Legibilidade das partituras a 100% e 150% (olhe também `capturas-19/` e `spike-render/capturas/`) | | |
| Anel de seleção, cursor de entrada e rolagem na janela real | | |
| PDF ao lado: destaque e rolagem | | |
| Som ao editar e "Ouvir compasso" | | |
| Queda real e recuperação (passo 8) | | |
| Prática com o REVAS de 61 teclas, sem latência perceptível com a Oficina aberta | | |
| Uso prolongado (horas) sem travar | | |

Aceite humano da E01 (nome e data): **aguardando aceite**.
