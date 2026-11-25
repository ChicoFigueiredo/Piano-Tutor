# Piloto do M4 — Curso do Luciano Alves, Seção 3 (Mão direita)

Decidido com o Chico em 2026-11-24 (grill em três rodadas). Vocabulário em [CONTEXT.md](../../CONTEXT.md); decisão de formato em [ADR-0009](../decisoes/ADR-0009-cursos-de-tutor-como-pacote.md).

## Fonte (somente no cerebro, nunca no repo)
- `/mnt/e2/Musica/Curso-Piano-1/` — 18 Seções, 163 Aulas (mp4 + .srt + `-Fala.Cronometrada.txt`), PDFs em `<seção>/Recursos/`, índice `00 - Índice do curso.md`, `catalogo-udemy.json`, `piano-1.db`.
- PDFs do Finale 2017 (vetoriais, fonte Maestro) com dedilhado e solfejo sob as notas. Convenção do autor: Dó central = Dó3.

## Escopo do piloto (fatia completa)
Seção 3, Aulas 12–24. Viram **Exercício com Gabarito**: 14 Cinco notas, 15 The smiling little dolphin, 16 Mary had a little lamb, 18 Jingle bells (2ª parte), 20 Exercício 1 MD, 21 Exercício 2 MD. As demais ficam como Aula para assistir.

## Etapas
1. **Formato do Pacote de Curso** (crate novo, só dados): manifesto versionado (Curso → Seções → Lições → Exercícios), MusicXML, áudio Opus dos trechos, Alinhamento por nota, Roteiro, assinatura ed25519, autor e licenciado. Leitor no app + empacotador CLI. Fixtures sintéticas nos testes.
2. **Importador**: lê índice/catálogo da fonte e gera o manifesto da Seção 3 com hashes.
3. **Transcrição PDF→MusicXML**: Audiveris (OMR) gera Hipótese; agente confere contra a imagem da página e contra o solfejo falado na transcrição cronometrada; divergências listadas. Estado Hipótese → Em revisão → Aprovada/Rejeitada (ou Hipótese → Aprovada pelo botão "Aprovar direto", marcado como `direto` no histórico). Correção humana no MuseScore.
4. **Alinhamento**: extrai o áudio (ffmpeg), detecta onsets/croma e alinha (DTW) à partitura aprovada; recorta o áudio do trecho tocado pelo Luciano.
5. **App**: seletor de Curso (fábrica + pacotes instalados, progresso separado); tela de revisão (PDF × partitura renderizada, com som, Aprovar/Rejeitar); Exercício com áudio alinhado e cursor, botão "ver vídeo" (mpv no instante).
6. **Professor falante**: Roteiro por passo, voz Piper pt-BR local; vale para Curso de fábrica e de tutor. Frases do Roteiro de tutor nascem como Hipótese da fala da Aula e só tocam depois de aprovadas.
7. **Chamada e resposta**: novo modo de Exercício — professor anuncia ("agora o acorde de Lá"), toca, e espera o aluno repetir; avalia com o comparador existente.
8. **QA**: testes automatizados (fixtures sintéticas) e roteiro de homologação no cerebro em TESTES-COM-HARDWARE.md.

## Pendências do Chico
- Instalar Audiveris, MuseScore, mpv, ffmpeg e Piper (se faltarem).
- Corrigir/aprovar as 6 partituras e o Roteiro da Seção 3.
- Homologar com o teclado.
- Redistribuir/vender só após licença escrita do Luciano.
