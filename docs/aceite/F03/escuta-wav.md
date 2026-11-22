# F03 — WAV de escuta (piano × acordeon)

> **Ambiente:** render offline na nuvem, sem dispositivo de áudio. Estes arquivos **não** substituem o checklist de escuta assinado pelo titular (ADR-0006): **NÃO VERIFICADO** até a assinatura.

| Arquivo | Programa GM (banco 0) | Preset no SF2 | Nota | Duração | Pico | RMS | Recortes |
|---|---|---|---|---|---|---|---|
| `f03-gm000-piano-c4.wav` | 0 | Yamaha Grand Piano | MIDI 60 (Dó4), velocidade 100 | 2,0 s (nota solta em 1,2 s) | 0,066 | 0,011 | 0 |
| `f03-gm021-acordeon-c4.wav` | 21 | Accordian | MIDI 60 (Dó4), velocidade 100 | 2,0 s (nota solta em 1,2 s) | 0,105 | 0,018 | 0 |

Formato: WAV PCM 16 bits, mono, 44,1 kHz, 176.444 bytes cada. Volume padrão do motor (ganho 1,0; volume interno do `rustysynth` 0,5; reverb e chorus ligados), sem normalização — por isso o nível é baixo.

**Versionamento:** o `.gitignore` do projeto exclui `*.wav`; os arquivos não entram no commit por padrão. Regenere com o comando abaixo (render determinístico; confira o SHA-256) ou versione com `git add -f` se o titular decidir.

SHA-256:

- `0fc5c6d94c0d578198e52cc32373ef5b0372dc6ce64281e699544b17b6bf9e69  f03-gm000-piano-c4.wav`
- `6604789cb1aee6d8eb43b7a1fb0139a32e0b2ae6011a534638f8f4fc86104eb1  f03-gm021-acordeon-c4.wav`

## Origem e licença

Renderizados com `rustysynth` 1.3.8 a partir de **FluidR3_GM** (pacote Debian main `fluid-soundfont-gm` 3.1-5.3), licença **MIT**, © 2000-2002, 2008 Frank Wen; © 2008 Toby Smithe. O SF2 não está no repositório (ver [licenças](../../seguranca/LICENCAS-E-DADOS.md)).

## Como reproduzir

```text
PT_SF2=/usr/share/sounds/sf2/FluidR3_GM.sf2 PT_WAV_DIR=docs/aceite/F03 \
  cargo test --release -p piano-tutor-audio --test sf2_real -- --ignored --nocapture
```

## Checklist de escuta (a preencher pelo titular)

- [ ] Piano e acordeon reconhecivelmente diferentes na mesma nota
- [ ] Sem estalo no início e no fim
- [ ] Sem distorção/clipping audível
- Assinatura / data: ____________________
