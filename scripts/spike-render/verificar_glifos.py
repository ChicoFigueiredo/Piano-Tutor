#!/usr/bin/env python3
"""Confere, para cada glifo SMuFL usado pelo protótipo (spike_bravura.rs), que o ponto de código
existe no mapa de caracteres da Bravura.otf e que a caixa do contorno no OTF bate com o
`glyphBBoxes` do Bravura.json (1 sp = 250 unidades de 1.000 por em; tolerância 6 unidades).
Script de desenvolvimento, fora da CI. Precisa de `fonttools` (pip install fonttools).

Uso: python3 scripts/spike-render/verificar_glifos.py assets/fontes/bravura/Bravura.otf assets/fontes/bravura/Bravura.json
Resultado conhecido (Bravura 1.482): fingering6 a fingering9 aparecem como DIVERGE (causa não
investigada); o protótipo só usa fingering0 a fingering5 (dedilhado de piano), que conferem.
"""

import json, sys
from fontTools.ttLib import TTFont
from fontTools.pens.boundsPen import BoundsPen

otf, meta = sys.argv[1], sys.argv[2]
f = TTFont(otf)
cmap = f.getBestCmap()
gs = f.getGlyphSet()
m = json.load(open(meta))
tabela = {
    "gClef": 0xE050, "gClef8vb": 0xE052, "gClef8va": 0xE053, "cClef": 0xE05C, "fClef": 0xE062,
    "fClef8vb": 0xE064, "fClef8va": 0xE065, "timeSigCommon": 0xE08A, "timeSigCutCommon": 0xE08B,
    "noteheadDoubleWhole": 0xE0A0, "noteheadWhole": 0xE0A2, "noteheadHalf": 0xE0A3, "noteheadBlack": 0xE0A4,
    "augmentationDot": 0xE1E7, "flag8thUp": 0xE240, "flag8thDown": 0xE241, "flag16thUp": 0xE242,
    "flag16thDown": 0xE243, "flag32ndUp": 0xE244, "flag32ndDown": 0xE245, "flag64thUp": 0xE246,
    "flag64thDown": 0xE247, "flag128thUp": 0xE248, "flag128thDown": 0xE249,
    "restDoubleWhole": 0xE4E2, "restWhole": 0xE4E3, "restHalf": 0xE4E4, "restQuarter": 0xE4E5,
    "rest8th": 0xE4E6, "rest16th": 0xE4E7, "rest32nd": 0xE4E8, "rest64th": 0xE4E9, "rest128th": 0xE4EA,
    "accidentalFlat": 0xE260, "accidentalNatural": 0xE261, "accidentalSharp": 0xE262,
    "accidentalDoubleSharp": 0xE263, "accidentalDoubleFlat": 0xE264, "brace": 0xE000,
    "repeatDots": 0xE043, "repeatDot": 0xE044, "tupletColon": 0xE88A,
    "dynamicPiano": 0xE520, "dynamicMezzo": 0xE521, "dynamicForte": 0xE522,
    "fermataAbove": 0xE4C0, "segno": 0xE047, "coda": 0xE048, "ornamentTrill": 0xE566,
}
for d in range(10):
    tabela[f"timeSig{d}"] = 0xE080 + d
    tabela[f"tuplet{d}"] = 0xE880 + d
    tabela[f"fingering{d}"] = 0xED10 + d
ruim = 0
for nome, cp in tabela.items():
    if cp not in cmap:
        print("SEM CMAP", nome, hex(cp)); ruim += 1; continue
    if nome not in m["glyphBBoxes"]:
        print("SEM METADATA", nome); ruim += 1; continue
    bp = BoundsPen(gs)
    gs[cmap[cp]].draw(bp)
    b = bp.bounds  # unidades (1000/em); 1 sp = 250
    sw, ne = m["glyphBBoxes"][nome]["bBoxSW"], m["glyphBBoxes"][nome]["bBoxNE"]
    esperado = (sw[0] * 250, sw[1] * 250, ne[0] * 250, ne[1] * 250)
    dif = max(abs(a - c) for a, c in zip(b, esperado)) if b else 0
    marca = "ok" if dif < 6 else "DIVERGE"
    if marca != "ok":
        ruim += 1
    print(f"{nome:28s} U+{cp:04X} {marca} dif={dif:.1f}")
print("problemas:", ruim)
