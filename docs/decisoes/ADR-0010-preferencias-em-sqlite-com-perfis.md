# ADR-0010 — Preferências em SQLite, com perfis e um perfil "De fábrica"

**Status:** PROPOSTA — 2026-11-24 · **Pedido do titular:** "persistir as minhas preferências em disco (recomendo SQLite), com perfis de preferência, e um deles é o de fábrica".

## Contexto

As preferências estavam em três arquivos de texto (`preferencias.txt` de som, `preferencias-exibicao.txt`, `preferencias-partitura.txt`), sem noção de perfil e sem jeito de voltar ao padrão. A arquitetura-alvo já previa SQLite local.

## Decisão

1. **Um banco SQLite** em `~/.local/share/piano-tutor/piano-tutor.sqlite`, via `rusqlite` com o SQLite **embutido** (`bundled`): o `.deb` não depende de biblioteca do sistema.
2. **Tabelas:** `perfis (id, nome único, de_fabrica)`, `preferencias (perfil, secao, valor)` e `meta (chave, valor)` com `esquema`, `perfil_ativo` e `legado_importado`.
3. **Seções** (`som`, `exibicao`, `partitura`, `tema`) guardam o mesmo texto `chave=valor` que cada módulo já lê e escreve; seção ausente = padrão. Assim os leitores tolerantes (chave desconhecida ou valor inválido vira padrão) e seus testes continuam valendo.
4. **"De fábrica"** existe sempre, não tem linhas e não pode ser alterado, renomeado nem apagado. Escolhê-lo devolve tudo ao padrão. Mudar algo com ele ativo cria (ou reusa) **"Meu perfil"**, que passa a ser o ativo, e o app avisa.
5. **Ribbon > Início > Perfil:** escolher o perfil, criar um novo com as preferências atuais, renomear e apagar o perfil em uso (volta ao de fábrica).
6. **Migração:** na primeira abertura, os `.txt` antigos viram "Meu perfil". Os arquivos ficam como cópia de segurança e não são mais gravados.
7. **Toda opção de tela vai para o perfil, na hora.** Além de som, dedilhado e partitura, cada tela tem um retrato `chave=valor` (`crates/piano-tutor-app/src/retratos.rs`): Teclado (nomes, acidentes, oitava, zoom, ajustar à largura, nomes nas teclas, pulso), Acordes e escalas (modo, tonalidade, fundamental, qualidade, inversão, região, escala, conjunto, de ouvido, dica, treino com andamento), Ritmos (metrônomo, bateria, subdivisão, contagem, volume, padrão inteiro com andamento, tolerância e compensação do pulso), Gravação (contagem e andamento) e Partitura (modo de prática). O app compara os retratos a cada quadro e grava o que mudou; num arrasto (zoom, volume), grava ao soltar. A janela (tema, densidade, escala, Ribbon recolhido, painéis, tela cheia) vai em JSON na seção `casca` e é aplicada antes de a janela abrir.
8. **Fora do perfil:** a calibração do teclado (`teclado.txt`) descreve o hardware, não gosto pessoal, e continua no arquivo próprio. O progresso dos exercícios também continua em `progresso.txt`.

## Consequências

- Sem pasta de dados ou com erro ao abrir o banco, o app usa um banco na memória: funciona, mas não guarda (mensagem no terminal).
- Seções em texto dentro do banco não permitem consultar uma preferência isolada por SQL. Se isso fizer falta, uma migração de esquema (`meta.esquema = 2`) pode quebrar as seções em linhas.
- Licenças: `rusqlite` é MIT; o SQLite é de domínio público.
