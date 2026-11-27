#!/usr/bin/env python3
"""Preparo do corpus CC0 (OpenScore Lieder) para o benchmark de OMR (E05, ticket 04).

Uso de preparo de dados: o app e a CI NAO dependem deste script.
Ambiente: scripts/corpus/requirements.txt (versoes fixadas) e `git`.

Subcomandos (rodar nesta ordem; todos leem/escrevem scripts/corpus/manifesto.json):
  baixar      clona OpenScore/Lieder no SHA fixado e confere a licenca de cada peca
  selecionar  (opcional) regenera selecao.json com semente fixa
  converter   mscx -> MusicXML com MuseScore CLI; sem MuseScore, usa o .mxl oficial do
              espelho e marca a conversao NAO VERIFICADO; confere compassos/notas
  pdf         MusicXML -> PDF vetorial limpo e PNG 300 dpi (verovio + cairosvg)
  degradar    PNG limpo -> scans degradados (augraphy) com semente e parametros fixos
  manifesto   resume contagens e confere sha256 de tudo que existe

Corpus gerado fica em data/corpus-omr/ (ignorado pelo git; ver .gitignore: /data/).
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import io
import json
import os
import random
import re
import shutil
import subprocess
import sys
import zipfile
import xml.etree.ElementTree as ET
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
CORPUS = RAIZ / "data" / "corpus-omr"
FONTE = CORPUS / "fonte" / "Lieder"
MANIFESTO = AQUI / "manifesto.json"
SELECAO = AQUI / "selecao.json"

REPO_URL = "https://github.com/OpenScore/Lieder"
# SHA do conjunto usado (acessado em 2026-11-27). Mudar so de forma deliberada.
REPO_SHA = "38c5db510224d9facdc4b08d741fc788cfb58ea8"
DATA_ACESSO = "2026-11-27"
LICENCA = "CC0-1.0"
SEMENTE_SELECAO = 20261127
N_VOZ_PIANO = 12
N_DUAS_PAUTAS = 2

# Presets de degradacao. A semente de cada (peca, preset) deriva de SEMENTE_BASE.
SEMENTE_BASE = 20261127
PRESETS = {
    "leve": {"descricao": "scan bom: InkBleed + Jpeg + rotacao <= 1 grau"},
    "medio": {"descricao": "InkBleed forte + NoiseTexturize + Folding + Jpeg"},
    "dificil": {"descricao": "InkBleed forte + Squish + Faxify + Folding + ruido + Jpeg"},
}
DPI = 300
LARGURA_PX, ALTURA_PX = 2480, 3508  # A4 a 300 dpi; o SVG do verovio tem proporcao A4 (2100x2970)


def sha256(p: Path | bytes) -> str:
    h = hashlib.sha256()
    if isinstance(p, bytes):
        h.update(p)
    else:
        with open(p, "rb") as f:
            for bloco in iter(lambda: f.read(1 << 20), b""):
                h.update(bloco)
    return h.hexdigest()


def rodar(cmd: list[str], **kw) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, check=True, text=True, capture_output=True, **kw)


def ler_manifesto() -> dict:
    if MANIFESTO.exists():
        return json.loads(MANIFESTO.read_text(encoding="utf-8"))
    return {"pecas": {}}


def gravar_manifesto(m: dict) -> None:
    MANIFESTO.write_text(
        json.dumps(m, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def versao(pacote: str) -> str:
    from importlib.metadata import version

    return version(pacote)


# ---------------------------------------------------------------- baixar
def cmd_baixar(_a) -> None:
    FONTE.parent.mkdir(parents=True, exist_ok=True)
    if not (FONTE / ".git").exists():
        rodar(["git", "init", "-q", str(FONTE)])
        rodar(["git", "-C", str(FONTE), "remote", "add", "origin", REPO_URL])
    rodar(["git", "-C", str(FONTE), "fetch", "-q", "--depth", "1", "origin", REPO_SHA])
    rodar(["git", "-C", str(FONTE), "checkout", "-q", "--force", REPO_SHA])
    head = rodar(["git", "-C", str(FONTE), "rev-parse", "HEAD"]).stdout.strip()
    assert head == REPO_SHA, f"SHA inesperado: {head}"
    lic = FONTE / "LICENSE.txt"
    assert "CC0 1.0 Universal" in lic.read_text(encoding="utf-8"), "LICENSE.txt nao e CC0"
    m = ler_manifesto()
    m["fonte"] = {
        "nome": "OpenScore Lieder Corpus (espelho GitHub de musescore.com/openscore-lieder-corpus)",
        "url": REPO_URL,
        "sha": REPO_SHA,
        "data_acesso": DATA_ACESSO,
        "licenca": LICENCA,
        "licenca_arquivo_sha256": sha256(lic),
        "comando": f"git init; git remote add origin {REPO_URL}; git fetch --depth 1 origin {REPO_SHA}; git checkout {REPO_SHA}",
    }
    gravar_manifesto(m)
    print("ok", head)


# ---------------------------------------------------------------- analise mscx / musicxml
def licenca_da_peca(mscx: Path) -> str | None:
    """Le o metaTag copyright do .mscx; devolve o texto se declarar CC0, senao None."""
    raiz = ET.parse(mscx).getroot()
    for t in raiz.iter("metaTag"):
        if t.get("name") == "copyright":
            txt = " ".join((t.text or "").split())
            return txt if "CC0" in txt else None
    return None


def contagem_mscx(mscx: Path) -> dict:
    sc = ET.parse(mscx).getroot().find("Score")
    pautas = sc.findall("Staff")
    return {
        "pautas": len(sc.findall("Part/Staff")),
        "compassos_por_pauta": [len(s.findall("Measure")) for s in pautas],
        "notas": sum(len(s.findall("Measure//Chord/Note")) for s in pautas),
        "pausas": sum(len(s.findall("Measure//Rest")) for s in pautas),
        "letra": any(True for _ in sc.iter("Lyrics")),
    }


def contagem_musicxml(xml_bytes: bytes) -> dict:
    raiz = ET.fromstring(xml_bytes)
    partes = raiz.findall("part")
    compassos = [len(p.findall("measure")) for p in partes]
    notas = pausas = 0
    for p in partes:
        for n in p.iter("note"):
            if n.find("rest") is not None:
                pausas += 1
            else:
                notas += 1
    pautas = 0
    for p in partes:
        st = p.find("measure/attributes/staves")
        pautas += int(st.text) if st is not None else 1
    return {"partes": len(partes), "pautas": pautas, "compassos_por_parte": compassos, "notas": notas, "pausas": pausas}


def ler_mxl(mxl: Path) -> bytes:
    with zipfile.ZipFile(mxl) as z:
        cont = ET.fromstring(z.read("META-INF/container.xml"))
        alvo = cont.find(".//{*}rootfile").get("full-path")
        return z.read(alvo)


def software_musicxml(xml_bytes: bytes) -> list[str]:
    raiz = ET.fromstring(xml_bytes)
    return [s.text for s in raiz.iterfind(".//encoding/software") if s.text]


# ---------------------------------------------------------------- selecionar
def cmd_selecionar(_a) -> None:
    candidatas = []
    for mscx in sorted((FONTE / "scores").glob("*/*/*/*.mscx")):
        d = mscx.parent
        if not (d / f"{mscx.stem}.mxl").exists() or not (d / f"{mscx.stem}.txt").exists():
            continue
        if licenca_da_peca(mscx) is None or mscx.stat().st_size > 250_000:
            continue
        c = contagem_mscx(mscx)
        if not 12 <= c["compassos_por_pauta"][0] <= 40 or not c["letra"]:
            continue
        candidatas.append((mscx, c))
    rng = random.Random(SEMENTE_SELECAO)
    rng.shuffle(candidatas)
    escolhidas, compositores = [], set()
    for mscx, c in candidatas:
        comp = mscx.relative_to(FONTE / "scores").parts[0]
        if c["pautas"] != 3 or comp in compositores:
            continue
        compositores.add(comp)
        escolhidas.append(mscx)
        if len(escolhidas) == N_VOZ_PIANO:
            break
    # Pecas de 2 pautas (candidatas a piano solo / sem letra): criterio proprio
    duas = []
    for mscx in sorted((FONTE / "scores").glob("*/*/*/*.mscx")):
        d = mscx.parent
        if not (d / f"{mscx.stem}.mxl").exists() or licenca_da_peca(mscx) is None:
            continue
        c = contagem_mscx(mscx)
        if c["pautas"] == 2 and 8 <= c["compassos_por_pauta"][0] <= 60 and mscx.stat().st_size <= 250_000:
            duas.append(mscx)
    rng.shuffle(duas)
    escolhidas += duas[:N_DUAS_PAUTAS]
    SELECAO.write_text(
        json.dumps(
            {
                "semente": SEMENTE_SELECAO,
                "sha": REPO_SHA,
                "criterios": "CC0 no metaTag; .mxl e .txt presentes; 12-40 compassos; <=250 kB; 3 pautas (voz+piano, 1 por compositor) + ate 2 de 2 pautas",
                "pecas": [str(p.relative_to(FONTE)) for p in escolhidas],
            },
            ensure_ascii=False,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    print(len(escolhidas), "pecas")


def lista_selecao() -> list[Path]:
    return [FONTE / p for p in json.loads(SELECAO.read_text(encoding="utf-8"))["pecas"]]


def id_peca(mscx: Path) -> str:
    return mscx.stem  # lc<id>, unico no corpus


# ---------------------------------------------------------------- converter
def achar_musescore() -> str | None:
    for nome in ("mscore", "musescore", "mscore4", "musescore4", "MuseScore4", "musescore3", "mscore3"):
        if shutil.which(nome):
            return shutil.which(nome)
    return None


def cmd_converter(_a) -> None:
    m = ler_manifesto()
    ms = achar_musescore()
    saida = CORPUS / "musicxml"
    saida.mkdir(parents=True, exist_ok=True)
    ms_ver = None
    if ms:
        ms_ver = rodar([ms, "--version"]).stdout.strip()
    for mscx in lista_selecao():
        pid = id_peca(mscx)
        lic = licenca_da_peca(mscx)
        assert lic, f"{pid}: sem CC0 no metaTag"
        rel = str(mscx.relative_to(FONTE))
        mxl_oficial = mscx.with_suffix(".mxl")
        reg = m["pecas"].setdefault(pid, {})
        parts = mscx.relative_to(FONTE / "scores").parts
        reg.update(
            {
                "id": pid,
                "compositor": parts[0],
                "conjunto": parts[1],
                "titulo": parts[2],
                "origem_mscx": f"{REPO_URL}/blob/{REPO_SHA}/{rel}",
                "licenca": LICENCA,
                "licenca_metadado_peca": lic,
                "licenca_fonte": f"metaTag copyright no .mscx (+ LICENSE.txt do repositorio, sha {REPO_SHA[:10]})",
                "mscx_sha256": sha256(mscx),
                "mxl_oficial_sha256": sha256(mxl_oficial),
                "mscx_contagem": contagem_mscx(mscx),
            }
        )
        destino = saida / f"{pid}.musicxml"
        if ms:
            tmp = saida / f"{pid}.tmp.musicxml"
            cmd = [ms, str(mscx), "-o", str(tmp)]
            rodar(cmd)
            xml = tmp.read_bytes()
            tmp.unlink()
            reg["conversao"] = {
                "origem": "MuseScore CLI",
                "comando": " ".join(["mscore", f"<{rel}>", "-o", f"<{pid}.musicxml>"]),
                "versao": ms_ver,
                "status": "VERIFICADO (rodou neste ambiente)",
            }
        else:
            xml = ler_mxl(mxl_oficial)
            sw = software_musicxml(xml)
            reg["conversao"] = {
                "origem": ".mxl oficial do espelho (gerado pelo mantenedor com MuseScore)",
                "comando_para_regerar_no_cerebro": f"mscore '{rel}' -o '{pid}.musicxml'   # ou: mscore -j data/corpus_conversion.json (lote oficial)",
                "versao_software_declarada_no_xml": sw,
                "status": "NAO VERIFICADO: MuseScore CLI ausente neste ambiente; usado o .mxl oficial do espelho; regerar no cerebro com `python scripts/corpus/corpus_omr.py converter`",
            }
        destino.write_bytes(xml)
        cm = contagem_musicxml(xml)
        cs = reg["mscx_contagem"]
        ok_comp = cs["compassos_por_pauta"][0] == cm["compassos_por_parte"][0]
        ok_notas = cs["notas"] == cm["notas"]
        reg["musicxml"] = {"arquivo": f"data/corpus-omr/musicxml/{pid}.musicxml", "sha256": sha256(destino), "contagem": cm}
        reg["conferencia_conversao"] = {
            "compassos_iguais": ok_comp,
            "notas_iguais": ok_notas,
            "mscx": {"compassos": cs["compassos_por_pauta"][0], "notas": cs["notas"]},
            "musicxml": {"compassos": cm["compassos_por_parte"][0], "notas": cm["notas"]},
        }
        print(pid, "compassos", ok_comp, "notas", ok_notas, cs["notas"], cm["notas"])
    m["ferramentas_conversao"] = {"musescore_cli": ms_ver or "NAO VERIFICADO (ausente)"}
    gravar_manifesto(m)


# ---------------------------------------------------------------- pdf
def cmd_pdf(_a) -> None:
    import cairosvg
    import verovio

    os.environ["SOURCE_DATE_EPOCH"] = "1700000000"  # PDF sem data de criacao variavel
    m = ler_manifesto()
    out = CORPUS / "pdf"
    png_dir = CORPUS / "limpo"
    out.mkdir(parents=True, exist_ok=True)
    png_dir.mkdir(parents=True, exist_ok=True)
    opcoes = {"pageWidth": 2100, "pageHeight": 2970, "scale": 40, "adjustPageHeight": False,
              "footer": "none", "header": "auto", "breaks": "auto"}
    for pid, reg in sorted(m["pecas"].items()):
        xml = (CORPUS / "musicxml" / f"{pid}.musicxml").read_text(encoding="utf-8")
        tk = verovio.toolkit()
        tk.setOptions(opcoes)
        assert tk.loadData(xml), f"verovio nao leu {pid}"
        paginas = tk.getPageCount()
        pdfs, pngs = [], []
        for i in range(1, paginas + 1):
            svg = tk.renderToSVG(i).encode("utf-8")
            pdf_i = out / f"{pid}.p{i}.pdf"
            png_i = png_dir / f"{pid}.p{i}.png"
            cairosvg.svg2pdf(bytestring=svg, write_to=str(pdf_i))
            cairosvg.svg2png(bytestring=svg, write_to=str(png_i), output_width=LARGURA_PX, output_height=ALTURA_PX, background_color="white")
            pdfs.append(pdf_i)
            pngs.append(png_i)
        final = out / f"{pid}.pdf"
        if len(pdfs) > 1 and shutil.which("pdfunite"):
            rodar(["pdfunite", *map(str, pdfs), str(final)])
            for p in pdfs:
                p.unlink()
        else:
            pdfs[0].rename(final)
        fontes = ""
        if shutil.which("pdffonts"):
            fontes = rodar(["pdffonts", str(final)]).stdout
        reg["pdf"] = {
            "arquivo": f"data/corpus-omr/pdf/{pid}.pdf",
            "sha256_PDF": sha256(final),
            "paginas": paginas,
            "gerador": f"verovio {versao('verovio')} -> SVG -> cairosvg {versao('cairosvg')} (svg2pdf)",
            "opcoes_verovio": opcoes,
            "fontes_embutidas_pdffonts": [l.split()[0] for l in fontes.splitlines()[2:] if l.strip()],
            "glifos_musicais": "paths (verovio+cairosvg): serve para raster; NAO serve para extracao vetorial por fonte. Fontes embutidas sao so do texto (titulo/letra).",
            "png_300dpi": {Path(p).name: sha256(p) for p in pngs},
            "reproduzivel_byte_a_byte": "PNG limpo: sim (sha256 estavel em 2 execucoes). PDF: NAO (cairo/pdfunite embutem ids/datas variaveis mesmo com SOURCE_DATE_EPOCH; sha256_PDF muda entre execucoes); a fonte da verdade reproduzivel e o PNG 300 dpi.",
        }
        print(pid, paginas, "paginas")
    m["ferramentas_pdf"] = {"verovio": versao("verovio"), "cairosvg": versao("cairosvg"),
                            "lilypond": "NAO VERIFICADO (ausente)" if not shutil.which("lilypond") else "presente",
                            "nota": "Para PDF com fontes musicais como texto, renderizar tambem com MuseScore/LilyPond no cerebro (ver 02-PESQUISA-OMR)."}
    gravar_manifesto(m)


# ---------------------------------------------------------------- degradar
def semente_de(pid: str, preset: str, pagina: int) -> int:
    return int(hashlib.sha256(f"{SEMENTE_BASE}:{pid}:{preset}:{pagina}".encode()).hexdigest()[:8], 16)


def pipeline_augraphy(preset: str):
    from augraphy import (AugraphyPipeline, Faxify, Folding, InkBleed, Jpeg,
                          NoiseTexturize, Geometric, Squish)

    if preset == "leve":
        ink = [InkBleed(intensity_range=(0.1, 0.2), p=1), Geometric(rotate_range=(-1, 1), p=1)]
        post = [Jpeg(quality_range=(60, 75), p=1)]
    elif preset == "medio":
        ink = [InkBleed(intensity_range=(0.3, 0.4), p=1), NoiseTexturize(sigma_range=(2, 4), p=1)]
        post = [Folding(fold_x=1240, fold_deviation=(0, 0), fold_count=1, fold_noise=0.0, fold_angle_range=(3, 3), p=1), Jpeg(quality_range=(40, 60), p=1)]
    else:
        ink = [InkBleed(intensity_range=(0.4, 0.5), p=1), Squish(p=1),
               Faxify(p=1)]
        post = [Folding(fold_x=900, fold_deviation=(0, 0), fold_count=1, fold_noise=0.0, fold_angle_range=(3, 3), p=1), NoiseTexturize(sigma_range=(3, 8), p=1), Jpeg(quality_range=(30, 50), p=1)]
    return AugraphyPipeline(ink_phase=ink, paper_phase=[], post_phase=post)


def degradar_um(png: Path, pid: str, preset: str, pagina: int):
    import cv2
    import numpy as np

    seed = semente_de(pid, preset, pagina)
    random.seed(seed)
    np.random.seed(seed % (2**32))
    cv2.setRNGSeed(seed % (2**31))
    img = cv2.imread(str(png))
    res = pipeline_augraphy(preset)(img)
    if isinstance(res, dict):
        res = res["output"]
    ok, buf = cv2.imencode(".png", res)
    assert ok
    return seed, buf.tobytes()


def cmd_degradar(_a) -> None:
    m = ler_manifesto()
    out = CORPUS / "scans"
    out.mkdir(parents=True, exist_ok=True)
    for pid, reg in sorted(m["pecas"].items()):
        reg["scans"] = {}
        for png in sorted((CORPUS / "limpo").glob(f"{pid}.p*.png")):
            pagina = int(re.search(r"\.p(\d+)\.png$", png.name).group(1))
            for preset in PRESETS:
                seed, dados = degradar_um(png, pid, preset, pagina)
                dest = out / f"{pid}.p{pagina}.{preset}.png"
                dest.write_bytes(dados)
                reg["scans"][dest.name] = {"preset": preset, "semente": seed, "sha256": sha256(dest),
                                           "origem_limpa_sha256": sha256(png)}
        print(pid, len(reg["scans"]), "scans")
    m["ferramentas_degradacao"] = {"augraphy": versao("augraphy"), "opencv-python": versao("opencv-python"),
                                   "numpy": versao("numpy"), "semente_base": SEMENTE_BASE, "presets": PRESETS,
                                   "dpi": DPI,
                                   "reproducibilidade": "sha256 por scan fixa a saida para as versoes acima; rode `manifesto --conferir` para checar",
                                   "nota_badphotocopy": "augraphy.BadPhotoCopy NAO foi reproduzivel em combinacao (testado: varia entre execucoes mesmo com random, numpy e cv2 semeados); removido dos presets; `BleedThrough` tambem variou (2 saidas distintas em 8 execucoes identicas) e foi trocado por InkBleed forte.", "nota_hashseed": "por precaucao o script reexecuta com PYTHONHASHSEED=0.",
                                   "nota_folding": "augraphy.Folding com parametros aleatorios NAO e reproduzivel mesmo com random/np.random semeados (testado 2x); por isso as dobras usam fold_x fixo, deviation=(0,0), noise=0 (testado: reproduzivel)."}
    gravar_manifesto(m)


# ---------------------------------------------------------------- manifesto
def cmd_manifesto(a) -> None:
    m = ler_manifesto()
    falhas = 0
    for pid, reg in sorted(m["pecas"].items()):
        c = reg.get("conferencia_conversao", {})
        print(pid, "conv:", c.get("compassos_iguais"), c.get("notas_iguais"), "pdf:", "pdf" in reg, "scans:", len(reg.get("scans", {})))
        if a.conferir:
            mx = CORPUS / "musicxml" / f"{pid}.musicxml"
            if mx.exists() and sha256(mx) != reg["musicxml"]["sha256"]:
                print("  DIVERGE musicxml", pid)
                falhas += 1
            for nome, s in reg.get("scans", {}).items():
                p = CORPUS / "scans" / nome
                if p.exists() and sha256(p) != s["sha256"]:
                    print("  DIVERGE scan", nome)
                    falhas += 1
    m["gerado_em"] = dt.date.today().isoformat() if not a.conferir else m.get("gerado_em")
    if not a.conferir:
        gravar_manifesto(m)
    sys.exit(1 if falhas else 0)


def main() -> None:
    # augraphy (ex.: Faxify/BadPhotoCopy) depende da ordem de hash de sets: sem PYTHONHASHSEED fixo
    # a saida varia entre processos (observado). Reexecuta com PYTHONHASHSEED=0.
    if os.environ.get("PYTHONHASHSEED") != "0":
        os.environ["PYTHONHASHSEED"] = "0"
        os.execv(sys.executable, [sys.executable, *sys.argv])
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    for nome, fn in [("baixar", cmd_baixar), ("selecionar", cmd_selecionar), ("converter", cmd_converter),
                     ("pdf", cmd_pdf), ("degradar", cmd_degradar)]:
        sub.add_parser(nome).set_defaults(fn=fn)
    p = sub.add_parser("manifesto")
    p.add_argument("--conferir", action="store_true")
    p.set_defaults(fn=cmd_manifesto)
    a = ap.parse_args()
    a.fn(a)


if __name__ == "__main__":
    main()
