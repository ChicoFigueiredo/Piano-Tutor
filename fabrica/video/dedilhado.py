#!/usr/bin/env python3
"""Hipótese de dedilhado a partir do vídeo filmado de cima (ADR-0013).

Uso:
  dedilhado.py <video> --calibracao calibracao.json --alinhamento cinco-notas.json \
      [--trecho-inicio S] --saida dedilhado-cinco-notas.json [--mao auto|direita|esquerda] [--depurar PASTA]

O instante de cada nota no vídeo da Aula é `trecho_inicio + t_audio_s`. Por padrão o início vem do campo
`offset_aula_s` do próprio Alinhamento; --trecho-inicio força outro valor (e o script avisa se divergirem).
"""
from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict
from pathlib import Path

import cv2
import numpy as np

import comum as C


def ponto_em_poligono(p, poly) -> bool:
    return cv2.pointPolygonTest(np.array(poly, np.float32).reshape(-1, 1, 2), (float(p[0]), float(p[1])), False) >= 0


def tecla_sob(ponto, teclas, y_margem=0.0):
    """Número MIDI da tecla sob o ponto (pretas têm prioridade, pois ficam por cima) ou None."""
    for pref in ("preta", "branca"):
        for midi, t in teclas.items():
            if t["tipo"] == pref and ponto_em_poligono(ponto, t["poligono"]):
                return int(midi)
    return None


def pontuacao_dedo(ponta, tecla, w_branca, y_base, y_topo):
    """0..1: quão 'sobre a tecla' está a ponta do dedo (gaussiana no x em torno do centro da tecla)."""
    x0, y0, x1, y1 = tecla["retangulo"]
    if tecla["tipo"] == "preta":
        cx, sigma = (x0 + x1) / 2, 0.30 * w_branca
    else:
        cx, sigma = (x0 + x1) / 2, 0.30 * w_branca
    if not (y_topo - 10 <= ponta[1] <= y_base + 25):
        return 0.0
    return float(np.exp(-0.5 * ((ponta[0] - cx) / sigma) ** 2))


def ler_quadros(cap, fps, intervalos):
    """Lê sequencialmente só os quadros dentro dos intervalos (s). Devolve {índice_quadro: bgr}."""
    precisa = set()
    for a, b in intervalos:
        precisa.update(range(int(np.floor(a * fps)), int(np.ceil(b * fps)) + 1))
    if not precisa:
        return {}
    ini, fim = min(precisa), max(precisa)
    cap.set(cv2.CAP_PROP_POS_FRAMES, ini)
    out = {}
    for i in range(ini, fim + 1):
        ok, q = cap.read()
        if not ok:
            break
        if i in precisa:
            out[i] = q
    return out


def desenhar_nota(q, cal, nota, resultado, maos_frame, caminho):
    img = q.copy()
    t = cal["teclas"][str(nota["midi"])]
    cv2.polylines(img, [np.array(t["poligono"], np.int32)], True, (0, 0, 255), 2)
    cores = {"direita": (255, 200, 0), "esquerda": (0, 200, 255)}
    for m in maos_frame:
        for d, idx in C.PONTAS.items():
            x, y = m["pontos"][idx]
            esc = resultado["dedo"] == d and resultado["mao"] == m["lado"]
            cv2.circle(img, (int(x), int(y)), 9 if esc else 5, (0, 255, 0) if esc else cores[m["lado"]], -1 if esc else 2)
            cv2.putText(img, str(d), (int(x) + 6, int(y) - 6), cv2.FONT_HERSHEY_SIMPLEX, 0.6, (255, 255, 255), 2)
    txt = f"nota {nota['indice']} midi {nota['midi']}: dedo {resultado['dedo']} mao {resultado['mao']} conf {resultado['confianca']:.2f}"
    cv2.rectangle(img, (0, 0), (640, 34), (0, 0, 0), -1)  # só ASCII: o putText do OpenCV não desenha acentos
    cv2.putText(img, txt, (10, 24), cv2.FONT_HERSHEY_SIMPLEX, 0.6, (255, 255, 255), 1)
    cv2.imwrite(str(caminho), img)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("video")
    ap.add_argument("--calibracao", required=True)
    ap.add_argument("--alinhamento", required=True)
    ap.add_argument("--trecho-inicio", type=float, help="início do trecho no vídeo da Aula (s); padrão: offset_aula_s do Alinhamento")
    ap.add_argument("--saida", required=True)
    ap.add_argument("--mao", choices=["auto", "direita", "esquerda"], default="auto")
    ap.add_argument("--janela-ms", default="-80,40", help="janela em volta do instante da nota, em ms (padrão -80,40)")
    ap.add_argument("--depurar", metavar="PASTA", help="grava um PNG por nota com as pontas dos dedos e a escolha")
    a = ap.parse_args()

    cal = json.loads(Path(a.calibracao).read_text(encoding="utf-8"))
    ali = json.loads(Path(a.alinhamento).read_text(encoding="utf-8"))
    offset_ali = ali.get("offset_aula_s")
    if a.trecho_inicio is not None:
        inicio = a.trecho_inicio
        if offset_ali is not None and abs(offset_ali - inicio) > 0.05:
            print(f"AVISO: --trecho-inicio {inicio} difere de offset_aula_s do Alinhamento ({offset_ali}).", file=sys.stderr)
    elif offset_ali is not None:
        inicio = float(offset_ali)
    else:
        raise SystemExit("Informe --trecho-inicio (o Alinhamento não tem offset_aula_s).")
    j0, j1 = (float(v) / 1000 for v in a.janela_ms.split(","))

    cap = C.abrir_video(a.video)
    info = C.info_video(cap)
    fps = info["fps"]
    teclas = cal["teclas"]
    w_branca = cal["grade"]["largura_branca_px"]
    y_topo = cal["faixa_teclado"]["y_topo_pretas"]
    y_base = cal["faixa_teclado"]["y_base_brancas"]
    notas = ali["notas"]
    tempos = [inicio + n["t_audio_s"] for n in notas]
    quadros = ler_quadros(cap, fps, [(t + j0, t + j1) for t in tempos])
    det, mp = C.criar_detector_maos()
    cache = {}

    def maos_do_quadro(i):
        if i not in cache:
            cache[i] = C.detectar_maos(det, mp, quadros[i], recorte_y=(y_topo - 30, quadros[i].shape[0]))
            for m in cache[i]:
                m["lado"] = m["lado_geometrico"]
        return cache[i]

    saida_notas, depur = [], Path(a.depurar) if a.depurar else None
    if depur:
        depur.mkdir(parents=True, exist_ok=True)
    for n, t in zip(notas, tempos):
        midi = n["midi"]
        tecla = teclas.get(str(midi))
        res = {"indice": n["indice"], "midi": midi, "dedo": None, "mao": None, "confianca": 0.0}
        if tecla is None:
            res["observacao"] = "tecla fora do quadro calibrado"
            saida_notas.append(res)
            continue
        ids = [i for i in quadros if t + j0 - 1e-9 <= i / fps <= t + j1 + 1e-9]
        votos = defaultdict(float)
        n_com_mao = 0
        melhor_quadro, melhor_val = None, -1.0
        for i in ids:
            maos = maos_do_quadro(i)
            if not maos:
                continue
            n_com_mao += 1
            for m in maos:
                if a.mao != "auto" and m["lado"] != a.mao:
                    continue
                for d, idx in C.PONTAS.items():
                    s = pontuacao_dedo(m["pontos"][idx], tecla, w_branca, y_base, y_topo)
                    votos[(m["lado"], d)] += s
                    if s > melhor_val:
                        melhor_val, melhor_quadro = s, i
        total = sum(votos.values())
        if total > 0:
            (lado, dedo), v = max(votos.items(), key=lambda kv: kv[1])
            partilha = v / total  # fatia dos votos do vencedor (1 = nenhum concorrente)
            cobertura = n_com_mao / max(1, len(ids))
            forca = min(1.0, v / max(1, len(ids)))  # quão bem o vencedor cobre a tecla, em média
            conf = partilha * cobertura * forca * min(1.0, tecla["confianca"] / 0.8)
            ordenados = sorted(votos.values(), reverse=True)
            res.update(
                dedo=dedo,
                mao=lado,
                confianca=round(float(conf), 3),
                quadros_usados=len(ids),
                quadros_com_mao=n_com_mao,
                segundo_colocado=None,
            )
            if len(ordenados) > 1 and ordenados[1] > 0:
                seg = sorted(votos.items(), key=lambda kv: -kv[1])[1][0]
                res["segundo_colocado"] = {"mao": seg[0], "dedo": seg[1]}
        else:
            res["observacao"] = "nenhum dedo sobre a tecla nos quadros da janela"
        res["t_video_s"] = round(t, 3)
        saida_notas.append(res)
        if depur and melhor_quadro is not None and res["dedo"]:
            desenhar_nota(quadros[melhor_quadro], cal, n, res, maos_do_quadro(melhor_quadro), depur / f"nota-{n['indice']:02d}.png")

    out = {
        "formato_versao": 1,
        "estado": "hipotese",
        "metodo": "mediapipe-hand-landmarker+calibracao-de-video",
        "proveniencia": {
            "ferramenta": "fabrica/video/dedilhado.py",
            "video": Path(a.video).name,
            "alinhamento": Path(a.alinhamento).name,
            "calibracao": Path(a.calibracao).name,
            "calibracao_confianca": cal["confianca"],
            "modelo_maos": "hand_landmarker.task (MediaPipe, float16)",
            "bibliotecas": {"mediapipe": __import__("mediapipe").__version__, "opencv": cv2.__version__},
            "trecho_inicio_s": inicio,
            "trecho_inicio_origem": "--trecho-inicio" if a.trecho_inicio is not None else "offset_aula_s do Alinhamento",
            "janela_ms": [j0 * 1000, j1 * 1000],
            "lado_da_mao": "geometria do dorso visto de cima (o rótulo do MediaPipe não é usado)",
            "regra_de_voto": "soma, nos quadros da janela, de gaussiana (sigma 0,3 tecla) do x da ponta do dedo ao centro da tecla; vence o par (mão, dedo) de maior soma",
        },
        "exercicio": ali.get("exercicio"),
        "notas": saida_notas,
    }
    Path(a.saida).write_text(json.dumps(out, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    det.close()
    for r in saida_notas:
        print(f"nota {r['indice']:2d} midi {r['midi']} ({C.nome_nota(r['midi'])}) dedo {r['dedo']} mao {r['mao']} conf {r['confianca']}")
    print(f"Gravado: {a.saida}")


if __name__ == "__main__":
    sys.exit(main())
