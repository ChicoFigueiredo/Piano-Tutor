> Numeração duplicada histórica; ver também ADR-0009-licoes-em-arquivos-toml.md

# ADR-0009 — Cursos de tutor chegam como Pacote de Curso

**Status:** ACEITA — 2026-11-24 · **Complementa:** [ADR-0008](ADR-0008-aulas-somente-locais.md)

Um Curso de tutor (o primeiro é o do Luciano Alves) nunca é parte do aplicativo. Ele chega num **Pacote de Curso**: um arquivo plugável **só de dados** (manifesto, MusicXML, áudio dos trechos tocados, Alinhamentos, Roteiro de falas), **assinado** (ed25519) e **sem DRM**. O app confere a assinatura, mostra autor e licenciado, e funciona igual sem nenhum pacote instalado. Vídeos completos não entram no pacote; o aluno pode apontar para os próprios arquivos e o app os abre no mpv no instante do trecho.

O repositório público guarda só o formato, o empacotador, o leitor e fixtures sintéticas. O pacote do Luciano é gerado e mantido no cerebro; só pode ser redistribuído ou vendido com licença escrita do autor (parceria).

**Por quê:** permite oferecer o app como parte do aprendizado em parceria com músicos sem misturar conteúdo protegido ao código aberto. Só dados evita executar código de terceiros; assinatura dá autenticidade sem servidor de ativação.

**Alternativas rejeitadas:** conteúdo embutido no app (viola direitos e trava parcerias); pacote com código WASM (sandbox e API estável cedo demais); criptografia com licença por máquina (exige servidor e suporte antes de existir venda).

## Nota (2026-11-24): uso pessoal de conteúdo não revisado

Para o uso pessoal do licenciado, o conteúdo não revisado (`hipotese` e `em_revisao`) pode ser usado, com selo "não revisado" na tela e na legenda do professor, controlado pela preferência "Usar conteúdo não revisado" (padrão ligada). `rejeitada` nunca é usada. O Gabarito oficial continua sendo só o aprovado: `serve_de_gabarito()`, `exercicios_para_aluno()` e `falas_aprovadas()` seguem estritos; o uso permissivo é pedido explicitamente por `utilizavel(permitir)`, `exercicios_para_aluno_com(permitir)` e `falas_utilizaveis(permitir)`. A revisão (Aprovar, Rejeitar, "Aprovar direto") continua sendo a fila.
