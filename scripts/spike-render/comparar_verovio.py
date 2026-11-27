#!/usr/bin/env python3
"""Compara o protótipo de render do Piano Tutor (egui + Bravura) com o Verovio, que só entra
aqui como ORÁCULO, num processo Python separado. Ticket evolucao/09 e ADR-0018.

Regras (T6/T10, ADR-0015): o Verovio é LGPL; nunca é linkado ao app, não está no Cargo e NÃO
roda na CI. Rodar este script é opcional e manual.

Uso:
  1. Gerar o lado próprio (MusicXML, PNGs do egui e relatorio-proprio.json):
       PT_SPIKE_SAIDA=/tmp/spike cargo test -p piano-tutor-app --lib exportar_corpus -- --ignored
  2. Instalar o oráculo e a rasterização de SVG em ambiente virtual (nada vai ao repositório):
       python3 -m venv /tmp/spike-venv && /tmp/spike-venv/bin/pip install verovio cairosvg pillow
  3. Rodar:
       /tmp/spike-venv/bin/python scripts/spike-render/comparar_verovio.py /tmp/spike [--recorte 560]
     `--recorte N` também grava, em comparacao/recortes/, só o topo (N px) de cada lado, em tons
     de cinza, para anexar ao repositório sem inflá-lo (as capturas completas ficam fora do git).

Saídas em <pasta>/comparacao/: um PNG por peça e zoom (protótipo em cima, Verovio embaixo),
svg/ (o SVG do Verovio por peça e zoom), comparacao.json e RELATORIO-OBJETIVO.md. Sem o pacote `verovio`, o relatório sai só com o lado
próprio e a marca NÃO VERIFICADO para o oráculo.
"""

import json
import re
import statistics
import sys
import time
import xml.etree.ElementTree as ET
from pathlib import Path

try:
    import verovio  # noqa: F401  (processo separado, nunca linkado ao app)

    VEROVIO_OK = True
except ImportError:  # pragma: no cover
    VEROVIO_OK = False

try:
    import cairosvg
    from PIL import Image, ImageDraw

    IMAGENS_OK = True
except ImportError:  # pragma: no cover
    IMAGENS_OK = False

SVG_NS = "{http://www.w3.org/2000/svg}"
# 1 sp do protótipo = ESP_BASE_PX (9,5) px a 100%; no Verovio 1 sp = 18 unidades de página a
# scale 100 (unit 9), então scale = 9,5 * zoom / 18 * 100.
ESP_BASE_PX = 9.5


def classes(el):
    return (el.get("class") or "").split()


def translate_de(el):
    m = re.search(r"translate\(\s*([-\d.]+)[ ,]+([-\d.]+)\s*\)", el.get("transform") or "")
    return (float(m.group(1)), float(m.group(2))) if m else None


def x_do_path(d):
    nums = [float(n) for n in re.findall(r"-?\d+(?:\.\d+)?", d)]
    return nums[0::2], nums[1::2]


def varrer(el, acc, ancestrais=()):
    """Percorre o SVG juntando contagens e posições."""
    cls = classes(el)
    caminho = ancestrais + tuple(cls)
    for c in cls:
        acc["classes"][c] = acc["classes"].get(c, 0) + 1
    tag = el.tag.replace(SVG_NS, "")
    if tag == "use" and "notehead" in ancestrais:
        t = translate_de(el)
        if t:
            acc["cabecas"].append(t)
    if tag == "ellipse" and "dots" in ancestrais:
        acc["pontos"] += 1
    if tag == "use" and "accid" in ancestrais:
        acc["acidentes"] += 1
    if tag == "polygon" and "beam" in ancestrais:
        acc["vigas"] += 1
    if tag == "path" and "ledgerLines" in ancestrais:
        acc["suplementares"] += (el.get("d") or "").count("M")
    for f in el:
        varrer(f, acc, caminho)


def analisar_svg(svg):
    raiz = ET.fromstring(svg)
    acc = {"classes": {}, "cabecas": [], "pontos": 0, "acidentes": 0, "vigas": 0, "suplementares": 0}
    varrer(raiz, acc)
    c = acc["classes"]
    # Espaço de pauta em unidades de página: distância entre linhas da primeira pauta.
    sp = 180.0
    sistemas = []
    for s in raiz.iter(SVG_NS + "g"):
        if "system" not in classes(s):
            continue
        medidas = []
        for m in s.findall(SVG_NS + "g"):
            if "measure" not in classes(m):
                continue
            staff = next((g for g in m.findall(SVG_NS + "g") if "staff" in classes(g)), None)
            if staff is None:
                continue
            linhas = [p for p in staff.findall(SVG_NS + "path")]
            if len(linhas) >= 2:
                ys = sorted({x_do_path(p.get("d"))[1][0] for p in linhas})
                if len(ys) >= 2 and not sistemas:
                    sp = ys[1] - ys[0]
                xs, _ = x_do_path(linhas[0].get("d"))
                medidas.append({"x0": min(xs), "x1": max(xs), "cabecas": [], "_el": m})
        sistemas.append(medidas)
    todas = [m for s in sistemas for m in s]
    # Cabeças por compasso: centro = x da origem + meia largura da cabeça preta (1,18 sp / 2).
    for m in todas:
        sub = {"classes": {}, "cabecas": [], "pontos": 0, "acidentes": 0, "vigas": 0, "suplementares": 0}
        varrer(m.pop("_el"), sub)
        m["cabecas"] = [tx + 0.59 * sp for tx, _ty in sub["cabecas"]]
    return {
        "sp": sp,
        "contagens": {
            "cabecas": len(acc["cabecas"]),
            "hastes": c.get("stem", 0),
            "vigas_poligonos": acc["vigas"],
            "grupos_de_viga": c.get("beam", 0),
            "bandeirolas": c.get("flag", 0),
            "pausas": c.get("rest", 0) + c.get("mRest", 0),
            "acidentes_de_nota": acc["acidentes"],
            "pontos": acc["pontos"],
            "ligaduras": c.get("slur", 0) + c.get("tie", 0),
            "grupos_de_tercina": c.get("tuplet", 0),
            "silabas": c.get("syl", 0),
            "cifras": c.get("harm", 0) - c.get("fb", 0),
            "baixo_cifrado": c.get("fb", 0),
            "dedilhados": c.get("fing", 0),
            "dinamicas": c.get("dynam", 0),
            "ornamentos": sum(c.get(k, 0) for k in ("trill", "mordent", "turn", "fermata", "ornam")),
            "linhas_suplementares": acc["suplementares"],
            "claves": c.get("clef", 0),
            "sistemas": len(sistemas),
            "compassos": len(todas),
        },
        "sistemas": sistemas,
    }


def rodar_verovio(xml_path, zoom, largura_px):
    scale = ESP_BASE_PX * zoom / 18.0 * 100.0
    tk = verovio.toolkit()
    page_w = int(round((largura_px - 12) * 100.0 / scale))
    tk.setOptions(
        {
            "pageWidth": page_w,
            "pageHeight": 60000,
            "scale": int(round(scale)),
            "adjustPageHeight": True,
            "breaks": "auto",
            "header": "none",
            "footer": "none",
            "pageMarginLeft": 20,
            "pageMarginRight": 20,
            "pageMarginTop": 40,
            "pageMarginBottom": 40,
        }
    )
    t0 = time.perf_counter()
    ok = tk.loadFile(str(xml_path))
    t1 = time.perf_counter()
    if not ok:
        return None
    svgs = [tk.renderToSVG(i + 1) for i in range(tk.getPageCount())]
    t2 = time.perf_counter()
    return {"svg": svgs[0], "paginas": len(svgs), "carga_ms": (t1 - t0) * 1e3, "render_ms": (t2 - t1) * 1e3}


def comparar_posicoes(proprio_sistemas, v_sistemas):
    """Posição relativa dos compassos por sistema e das cabeças por compasso."""
    own_par = [[(m["x0"], m["x1"]) for m in s["compassos"]] for s in proprio_sistemas]
    v_par = [[(m["x0"], m["x1"]) for m in s] for s in v_sistemas]
    own_n = [len(s) for s in own_par]
    v_n = [len(s) for s in v_par]
    out = {"compassos_por_sistema_proprio": own_n, "compassos_por_sistema_verovio": v_n}
    desvios = []
    for a, b in zip(own_par, v_par):
        if len(a) != len(b) or not a:
            continue
        wa = a[-1][1] - a[0][0]
        wb = b[-1][1] - b[0][0]
        for (a0, a1), (b0, b1) in zip(a, b):
            desvios.append(abs((a1 - a[0][0]) / wa - (b1 - b[0][0]) / wb))
    out["sistemas_com_mesma_quebra"] = sum(1 for a, b in zip(own_n, v_n) if a == b)
    out["desvio_max_barra_de_compasso"] = max(desvios) if desvios else None
    out["desvio_medio_barra_de_compasso"] = statistics.mean(desvios) if desvios else None
    # Cabeças como fração do compasso, comparadas por medida quando as contagens batem.
    own_medidas = [m for s in proprio_sistemas for m in s["compassos"]]
    v_medidas = [m for s in v_sistemas for m in s]
    out["compassos_proprio"] = len(own_medidas)
    out["compassos_verovio"] = len(v_medidas)
    difs, comparados = [], 0
    if len(own_medidas) == len(v_medidas):
        for mo, mv in zip(own_medidas, v_medidas):
            fo = sorted(mo["cabecas"])
            w = mv["x1"] - mv["x0"]
            fv = sorted((x - mv["x0"]) / w for x in mv["cabecas"]) if w > 0 else []
            if fo and len(fo) == len(fv):
                comparados += 1
                difs.append(statistics.mean(abs(a - b) for a, b in zip(fo, fv)))
    out["compassos_comparados_cabecas"] = comparados
    out["desvio_medio_posicao_cabeca"] = statistics.mean(difs) if difs else None
    out["desvio_max_posicao_cabeca"] = max(difs) if difs else None
    return out


def contagens_proprias(z):
    c = z["contagens"]
    return {
        "cabecas": c.get("cabeca", 0),
        "hastes": c.get("haste", 0),
        "vigas_poligonos": c.get("viga", 0),
        "bandeirolas": c.get("bandeirola", 0),
        "pausas": c.get("pausa", 0),
        "acidentes_de_nota": c.get("acidente", 0),
        "pontos": c.get("ponto_de_aumento", 0),
        "ligaduras": c.get("arco", 0),
        "silabas": c.get("letra", 0),
        "cifras": c.get("cifra", 0),
        "baixo_cifrado": 0,
        # O modelo atual descarta dinâmica e ornamento; o protótipo não os desenha.
        "dinamicas": 0,
        "ornamentos": 0,
        "dedilhados": c.get("dedilhado", 0),
        "linhas_suplementares": c.get("linha_suplementar", 0),
        "claves": c.get("clave", 0),
        "sistemas": z["n_sistemas"],
        "compassos": z["n_compassos"],
    }


def montar_png(proprio_png, svg, destino, rotulo, recorte=None):
    if not IMAGENS_OK:
        return False
    a = Image.open(proprio_png).convert("RGB")
    b_png = destino.with_suffix(".verovio.png")
    cairosvg.svg2png(bytestring=svg.encode("utf-8"), write_to=str(b_png), background_color="white")
    b = Image.open(b_png).convert("RGB")
    faixa = 22
    w = max(a.width, b.width)
    img = Image.new("RGB", (w, a.height + b.height + 2 * faixa), "white")
    d = ImageDraw.Draw(img)
    d.text((6, 4), f"{rotulo}: prototipo (egui + Bravura 1.482)", fill=(0, 0, 0))
    img.paste(a, (0, faixa))
    d.text((6, faixa + a.height + 4), f"{rotulo}: Verovio (oraculo, processo separado)", fill=(0, 0, 0))
    img.paste(b, (0, 2 * faixa + a.height))
    img.save(destino)
    if recorte:
        pasta = destino.parent / "recortes"
        pasta.mkdir(exist_ok=True)
        topo = Image.new("L", (w, 2 * faixa + 2 * recorte), 255)
        topo.paste(img.crop((0, 0, w, faixa + min(recorte, a.height))).convert("L"), (0, 0))
        topo.paste(
            img.crop((0, faixa + a.height, w, faixa + a.height + faixa + min(recorte, b.height))).convert("L"),
            (0, faixa + recorte),
        )
        topo.save(pasta / destino.name, optimize=True)
    b_png.unlink(missing_ok=True)
    return True


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    pasta = Path(sys.argv[1])
    recorte = int(sys.argv[sys.argv.index("--recorte") + 1]) if "--recorte" in sys.argv else None
    rel = json.loads((pasta / "relatorio-proprio.json").read_text())
    saida = pasta / "comparacao"
    saida.mkdir(exist_ok=True)
    (saida / "svg").mkdir(exist_ok=True)
    resultado = {"verovio": None, "pecas": []}
    if VEROVIO_OK:
        resultado["verovio"] = verovio.toolkit().getVersion()
    else:
        print("NÃO VERIFICADO: o pacote `verovio` não está instalado; só o lado próprio sai no relatório.")
    for peca in rel["pecas"]:
        item = {"id": peca["id"], "zooms": []}
        if peca.get("erro_modelo"):
            item["erro_modelo"] = peca["erro_modelo"]
            resultado["pecas"].append(item)
            continue
        for z in peca["zooms"]:
            r = {"zoom": z["zoom"], "proprio": contagens_proprias(z), "tempos_proprio": z["tempos"],
                 "n_colisoes_proprio": z["n_colisoes"],
                 "colisoes_por_par": z.get("colisoes_por_par", {})}
            if VEROVIO_OK:
                v = rodar_verovio(pasta / f"{peca['id']}.musicxml", z["zoom"], z["largura_px"])
                if v is None:
                    r["verovio_erro"] = "o Verovio não carregou o arquivo"
                else:
                    an = analisar_svg(v["svg"])
                    r["verovio"] = an["contagens"]
                    r["tempos_verovio"] = {"carga_ms": v["carga_ms"], "render_ms": v["render_ms"],
                                           "paginas": v["paginas"], "svg_bytes": len(v["svg"])}
                    r["posicoes"] = comparar_posicoes(z["sistemas"], an["sistemas"])
                    (saida / "svg" / f"{peca['id']}-{int(z['zoom'] * 100)}.svg").write_text(v["svg"], encoding="utf-8")
                    destino = saida / f"{peca['id']}-{int(z['zoom'] * 100)}.png"
                    r["png"] = destino.name if montar_png(pasta / z["png"], v["svg"], destino, f"{peca['id']} {int(z['zoom']*100)}%", recorte) else None
            item["zooms"].append(r)
        resultado["pecas"].append(item)
    (saida / "comparacao.json").write_text(json.dumps(resultado, indent=2, ensure_ascii=False))
    (saida / "RELATORIO-OBJETIVO.md").write_text(relatorio_md(resultado), encoding="utf-8")
    print(f"escrito em {saida}")
    return 0


CHAVES = [
    ("cabecas", "cabeças"),
    ("hastes", "hastes"),
    ("vigas_poligonos", "vigas (polígonos)"),
    ("bandeirolas", "bandeirolas"),
    ("pausas", "pausas"),
    ("acidentes_de_nota", "acidentes"),
    ("pontos", "pontos de aumento"),
    ("ligaduras", "ligaduras (valor e expressão)"),
    ("silabas", "sílabas de letra"),
    ("cifras", "cifras"),
    ("baixo_cifrado", "baixo cifrado"),
    ("dedilhados", "dedilhados"),
    ("dinamicas", "dinâmicas"),
    ("ornamentos", "ornamentos e fermata"),
    ("linhas_suplementares", "linhas suplementares"),
    ("claves", "claves"),
    ("sistemas", "sistemas"),
    ("compassos", "compassos"),
]


def relatorio_md(res):
    L = ["# Relatório objetivo: protótipo (egui + Bravura) contra o Verovio", ""]
    L.append(f"Oráculo: {res['verovio'] or 'NÃO VERIFICADO (verovio não instalado neste ambiente)'}.")
    L.append("Contagens iguais não provam legibilidade; o julgamento visual é humano (NÃO VERIFICADO).")
    L.append("")
    for p in res["pecas"]:
        L.append(f"## {p['id']}")
        if p.get("erro_modelo"):
            L.append(f"O modelo atual não leu o arquivo: {p['erro_modelo']}")
            continue
        for z in p["zooms"]:
            L.append(f"### {int(z['zoom'] * 100)}%")
            L.append("| elemento | protótipo | Verovio |")
            L.append("| --- | ---: | ---: |")
            for k, nome in CHAVES:
                v = z.get("verovio", {}).get(k, "n/d") if "verovio" in z else "NÃO VERIFICADO"
                L.append(f"| {nome} | {z['proprio'].get(k, 'n/d')} | {v} |")
            por_par = ", ".join(f"{k}: {v}" for k, v in sorted(z.get("colisoes_por_par", {}).items(), key=lambda kv: -kv[1]))
            L.append(f"\nColisões detectadas no protótipo: {z['n_colisoes_proprio']}" + (f" ({por_par})." if por_par else "."))
            if "posicoes" in z:
                pos = z["posicoes"]
                L.append(
                    f"Compassos por sistema: protótipo {pos['compassos_por_sistema_proprio']}, "
                    f"Verovio {pos['compassos_por_sistema_verovio']}; "
                    f"desvio máx. da barra de compasso (fração do sistema, onde a quebra coincide): "
                    f"{pos['desvio_max_barra_de_compasso']}; "
                    f"desvio médio da posição das cabeças dentro do compasso "
                    f"({pos['compassos_comparados_cabecas']} compassos comparados): "
                    f"{pos['desvio_medio_posicao_cabeca']}."
                )
            tp = z["tempos_proprio"]
            L.append(
                f"Tempo (ms): protótipo layout+cena {tp['layout_e_cena_ms']:.1f}, pintura egui "
                f"{tp['pintura_egui_ms']:.1f}, tesselação {tp['tesselacao_ms']:.1f}; hit test "
                f"{tp['hit_test_us_por_consulta']:.2f} us/consulta."
            )
            if "tempos_verovio" in z:
                tv = z["tempos_verovio"]
                L.append(f"Verovio: carga {tv['carga_ms']:.1f} ms, render SVG {tv['render_ms']:.1f} ms, {tv['paginas']} página(s).")
            L.append("")
    return "\n".join(L)


if __name__ == "__main__":
    sys.exit(main())
