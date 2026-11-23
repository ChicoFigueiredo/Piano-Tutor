# Arquivos MIDI de referência (F06)

Três arquivos sintéticos, escritos para o Piano Tutor pelo gerador de testes em
`src/gravacao.rs` (`gerar_arquivos_de_referencia`). Não vêm de nenhuma música, curso ou
biblioteca de terceiros. **Licença: CC0 1.0 (domínio público).**

| Arquivo | Conteúdo |
|---|---|
| `referencia-1-tipo1-multicanal.mid` | Tipo 1, 4 trilhas, 480 PPQ, 120 bpm. Canais 0 (melodia Dó-Mi-Sol), 1 (acorde de duas notas) e 9 (percussão). |
| `referencia-2-mudanca-de-andamento.mid` | Tipo 0, 480 PPQ. 120 bpm, depois 60 bpm no tick 1920 e 240 bpm no tick 2880. 10 notas. |
| `referencia-3-pedal-acordes-velocidades.mid` | Tipo 0, 480 PPQ, 120 bpm. Pedal CC64 (127, 0, 64, 0), acordes de 4 e 2 notas, velocidades de 1 a 127 e `NoteOn` com velocidade 0 como soltar. |

Para regenerar (só se o gerador mudar de propósito):

```
cargo test -p piano-tutor-app gravacao::testes::gerar_arquivos_de_referencia -- --ignored
```

O teste `arquivos_de_referencia_commitados_sao_os_gerados` falha se os arquivos versionados
diferirem do que o gerador produz.
