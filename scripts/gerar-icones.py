#!/usr/bin/env python3
"""Gera a fonte de ícones do Piano Tutor a partir dos SVGs em assets/icones/svg.

Cada SVG (grade 24x24, traço 1,5) vira um glifo monocromático na área de uso privado
(U+E000 em diante, na ordem alfabética dos nomes). Os traços viram preenchimento com picosvg
(skia-pathops) e o contorno entra na fonte com fontTools. A saída é
assets/icones/piano-tutor-icones.ttf e assets/icones/mapa.txt (nome<TAB>código).

Dependências só para gerar (não para compilar o app): fonttools, picosvg, skia-pathops (MIT/BSD/Apache).
Uso: python3 scripts/gerar-icones.py
"""
import pathlib
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.pens.cu2quPen import Cu2QuPen
from fontTools.pens.transformPen import TransformPen
from fontTools.svgLib.path import parse_path
from picosvg.svg import SVG

RAIZ = pathlib.Path(__file__).resolve().parent.parent
PASTA = RAIZ / "assets" / "icones"
EM, ASC, DESC = 1000, 880, -120
ESCALA = EM / 24.0
INICIO = 0xE000


def glifo(svg_path: pathlib.Path):
    pico = SVG.parse(str(svg_path)).topicosvg()
    pen = TTGlyphPen(None)
    cu = Cu2QuPen(pen, max_err=1.0, reverse_direction=True)
    tp = TransformPen(cu, (ESCALA, 0, 0, -ESCALA, 0, ASC))
    for forma in pico.shapes():
        parse_path(forma.d, tp)
    return pen.glyph()


def main():
    svgs = sorted((PASTA / "svg").glob("*.svg"))
    nomes = [".notdef"]
    glifos = {".notdef": TTGlyphPen(None).glyph()}
    cmap, mapa = {}, []
    for i, s in enumerate(svgs):
        nome = "ic_" + s.stem.replace("-", "_")
        nomes.append(nome)
        glifos[nome] = glifo(s)
        cmap[INICIO + i] = nome
        mapa.append(f"{s.stem}\t{INICIO + i:04X}")
    fb = FontBuilder(EM, isTTF=True)
    fb.setupGlyphOrder(nomes)
    fb.setupCharacterMap(cmap)
    fb.setupGlyf(glifos)
    fb.setupHorizontalMetrics({n: (EM, 0) for n in nomes})
    fb.setupHorizontalHeader(ascent=ASC, descent=DESC)
    fb.setupNameTable({"familyName": "Piano Tutor Icones", "styleName": "Regular"})
    fb.setupOS2(sTypoAscender=ASC, sTypoDescender=DESC, usWinAscent=ASC, usWinDescent=-DESC)
    fb.setupPost()
    fb.save(str(PASTA / "piano-tutor-icones.ttf"))
    (PASTA / "mapa.txt").write_text("\n".join(mapa) + "\n", encoding="utf-8")
    print(f"{len(svgs)} ícones gerados")


if __name__ == "__main__":
    main()
