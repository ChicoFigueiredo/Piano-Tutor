# Baseline de desempenho dos comandos, peça de 100 compassos (E01, ticket 26, A16)

**Estado: baseline registrado nesta máquina de nuvem. Desempenho no PC de referência do titular (Linux de referência, REVAS, áudio): NÃO VERIFICADO.** Nenhuma meta foi alterada nem declarada atingida. A meta de 100 ms permanece **"a calibrar"**.

## Como foi medido

- **Peça:** `gerar_cem_compassos()` (`crates/piano-tutor-fixtures/src/gerador.rs`): 100 compassos, 4/4, Dó maior, 400 semínimas, sintética e CC0 (nenhum material do titular nem comprado). Uma adaptação determinística: `divisions` 1 vira 4 (e `duration` 1 vira 4), porque com `divisions` 1 o serviço recusa colcheia e ponto de aumento (`DuracaoExigeMudarDivisions`; mudar `divisions` é outro comando). Mesmas notas e estrutura.
- **Harness:** `crates/piano-tutor-app/tests/baseline_desempenho.rs`. Sem dependência nova (usa `tempfile` e `piano-tutor-fixtures`, que já eram dev-dependências). Perfil **release**.
- **Estatística:** 3 execuções de aquecimento descartadas, depois N amostras por comando (N = 1000 nos comandos leves, 250 nos que tocam disco: salvar, diff de Versões, conferência; 10000 nos hit tests). **Mediana**, **p95** (posto mais próximo) e **máximo**; as amostras brutas estão em `dados-release.json`.
- **Em cada comando de edição só o `aplicar` é cronometrado.** A preparação (remover antes de reinserir, montar o acorde antes de retirar uma altura) e o `desfazer` que devolve o documento ao estado de partida ficam fora da medição. Os alvos giram pelos 100 compassos.
- **Sem asserção de meta e fora do `cargo test` padrão:** o benchmark é `#[ignore]`; no `cargo test` roda só `fumaca_do_baseline` (1 execução por comando, sem olhar tempo) para o harness não apodrecer.

### Máquina

| Item | Valor |
| --- | --- |
| CPU | Intel Xeon (virtualizado) @ 2,10 GHz |
| Núcleos lógicos | 4 |
| SO | Ubuntu 24.04.5 LTS, kernel 6.18.44-fc-v880 |
| Rust | rustc 1.98.0 |
| Código | commit `60aa0f1` (base) mais o harness deste ticket |
| Disco | ext4 (`/dev/vda`), diretório temporário em `/tmp` |
| Carga | **máquina compartilhada e ocupada**: média de carga 3,2 / 4,4 / 3,7 em 4 núcleos ao gravar; sem áudio, sem GPU, sem janela |
| Data | 2026-11-28 |

Isto **não** é o PC de referência. É uma VM de nuvem sem GPU, com ruído de vizinhos: use os números como ordem de grandeza e para detectar regressão relativa nesta mesma máquina, não como promessa.

### Reprodução

Da raiz do repositório (o caminho de saída deve ser absoluto; o `cargo test` roda na pasta do crate):

```bash
PT_BASELINE_N=1000 PT_BASELINE_SAIDA="$PWD/docs/evolucao-musical/e01/desempenho" \
  cargo test --release -p piano-tutor-app --test baseline_desempenho -- --ignored --nocapture
```

Sem `PT_BASELINE_SAIDA` só imprime a tabela. Gera `dados-release.json` (amostras) e `dados-release.csv` (resumo).

## Resultados (release, ms)

| Comando | Ticket | N | Mediana | p95 | Máximo |
| --- | --- | ---: | ---: | ---: | ---: |
| Abrir MusicXML (`abrir_texto`) | 10 | 1000 | 1,937 | 2,695 | 4,910 |
| Abrir `.mxl` (inclui descompactar) | 10 | 1000 | 2,010 | 2,148 | 2,876 |
| Abrir como a tela (`DocumentoPartitura::de_texto`) | 19 | 1000 | 2,660 | 2,835 | 4,252 |
| Altura: 1 semitom | 14 | 1000 | 0,0057 | 0,0071 | 0,068 |
| Altura: 1 oitava | 14 | 1000 | 0,0049 | 0,0059 | 0,045 |
| Duração: trocar a figura | 16 | 1000 | 0,0197 | 0,0281 | 0,305 |
| Duração: ponto de aumento | 16 | 1000 | 0,0196 | 0,0270 | 0,193 |
| Remover nota (vira pausa) | 17 | 1000 | 0,0164 | 0,0215 | 0,051 |
| Inserir nota | 17 | 1000 | 0,0160 | 0,0214 | 0,077 |
| Acorde: acrescentar por letra | 18 | 1000 | 0,0195 | 0,0252 | 0,049 |
| Acorde: acrescentar por intervalo | 18 | 1000 | 0,0193 | 0,0247 | 0,073 |
| Acorde: retirar altura | 18 | 1000 | 0,0180 | 0,0220 | 0,132 |
| Desfazer | 15 | 1000 | 0,0005 | 0,0006 | 0,016 |
| Refazer | 15 | 1000 | 0,0004 | 0,0005 | 0,001 |
| Exportar, sem compasso sujo | 11 | 1000 | 0,126 | 0,140 | 0,194 |
| Exportar, 1 compasso sujo | 11 | 1000 | 0,128 | 0,141 | 0,181 |
| Exportar + conferir a exportação (sem disco) | 11 | 1000 | 2,404 | 3,484 | 4,685 |
| Layout sem identidades (sistemas quebrados) | 06 | 1000 | 0,154 | 0,177 | 0,283 |
| Layout com identidades de nota | 06 | 1000 | 0,170 | 0,255 | 0,558 |
| **Layout + exibição** (letra, cifra, dedilhado) | 19 | 1000 | 0,156 | 0,177 | 0,243 |
| Layout em fita contínua | 06 | 1000 | 0,149 | 0,170 | 0,549 |
| Hit test: `nota_em` | 06 | 10000 | 0,0015 | 0,0015 | 0,027 |
| Hit test: `acertar` | 06 | 10000 | 0,0015 | 0,0015 | 0,123 |
| **Salvar Versão** (disco desta máquina) | 21 | 250 | 4,016 | 5,328 | 7,795 |
| **Diff entre Versões** (abre as 2 do disco e compara) | 22 | 250 | 8,249 | 9,009 | 16,100 |
| Diff de documentos puro (sem disco) | 22 | 1000 | 1,788 | 3,030 | 4,271 |
| **Conferência de prática de Versão** | 25 | 250 | 1,999 | 2,075 | 4,042 |

Repetições anteriores no mesmo dia (N = 200 e N = 1000) deram medianas dentro de poucos por cento nos comandos de CPU. Nos que tocam disco a variação é maior (a conferência foi de 0,38 a 2,0 ms entre execuções; o salvar teve máximo de 26 ms numa execução), coerente com a máquina ocupada e disco virtual.

## Comparação com a meta (observação, "meta a calibrar")

A meta proposta é 100 ms em 100 compassos. **Nesta máquina e neste perfil, nenhum comando passou de 100 ms**: o pior máximo isolado é 16,1 ms (diff entre Versões) e a pior mediana é 8,2 ms (a mesma). Os comandos de edição ficam em dezenas de microssegundos, ordens de grandeza abaixo. Isto é uma observação sobre esta VM, **não** uma declaração de que a meta foi atingida: no PC de referência é **NÃO VERIFICADO**, e a medição sob carga com áudio (callback de som, MIDI→tela) é **NÃO VERIFICADA** em qualquer máquina. Nenhuma meta foi alterada. Como nenhum comando excedeu 100 ms, não se abriu registro de estouro com perfil.

## Pendências herdadas e leituras

- **Tempo de layout do ticket 06 (delegado a este ticket):** o layout de 100 compassos custa mediana 0,15 a 0,17 ms (p95 até 0,26 ms), ou cerca de 1% de um quadro de 60 Hz (16,7 ms). Isso dá o **valor absoluto**, mas **não responde** à pergunta original do ticket 06 ("sem aumento mensurável"): não há medição do layout antigo (pré-ticket 06) nesta mesma máquina e harness, e a medição informal do ticket 06 (0,84 a 1,7 ms, 42 compassos, outra máquina) não é comparável. A afirmação de "nenhum aumento" continua **NÃO COMPROVADA**; o que fica registrado é que o custo absoluto atual é desprezível frente à meta e ao quadro. Para comprovar, seria preciso compilar o commit anterior ao ticket 06 com o mesmo harness (não feito aqui).
- **Layout + exibição (19):** a letra, a cifra e o dedilhado não somam custo mensurável sobre o layout puro nesta peça. A peça gerada **não tem letra, cifra nem dedilhado**, então este número é o do caminho vazio, não o de uma peça cantada. NÃO VERIFICADO com letra densa.
- **Conferência de prática (25), hoje limitada a 1/s:** custa cerca de 2 ms (0,4 a 2 ms entre execuções), dominada pela leitura do arquivo e pelo SHA-256 da Versão. Uma vez por segundo é 0,2% do tempo; **a cada quadro seria cerca de 12% do orçamento de 16,7 ms** nesta máquina, por isso o limite de 1/s continua justificado e **não deve ser removido** sem medir de novo.
- **Abrir (1,9 a 2,7 ms), exportar + conferir (2,4 ms) e diff (1,8 a 8,2 ms)** são os mais caros, e ainda assim ~40 vezes abaixo de 100 ms. Exportar sem compasso sujo ou com um sujo custa o mesmo (~0,13 ms): o custo é a cópia do texto, não a regeneração. A conferência da exportação é a parte cara do salvar na tela.
- **Peça muito diferente:** só uma peça sintética de uma pauta, uma voz, semínimas. Peças reais (várias pautas, acordes, ornamentos) podem custar mais; NÃO VERIFICADO.
- **Não medido:** autosave (ticket 17b, outra equipe), desenho egui/GPU (só o cálculo do layout é medido, não a pintura), prática ao vivo com áudio, macOS, arquivos grandes.
