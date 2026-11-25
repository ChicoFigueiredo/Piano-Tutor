#!/usr/bin/env python3
"""Calibração de vídeo automática: acha as teclas no quadro da câmera de cima.

Uso: calibrar.py <video> --saida calibracao.json [--quadro-s T] [--faixa auto|superior|inferior]

Saída: JSON com o mapa de teclas por número MIDI e um PNG (mesmo nome, .png) para o revisor conferir.
Tudo aqui é Hipótese: o revisor confere o PNG e, se preciso, corrige à mão.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import cv2
import numpy as np

import comum as C


def quadros_de_amostra(cap, info, quadro_s, janela):
    """Quadros para a mediana (remove mãos móveis). Devolve (lista_bgr, descrição)."""
    dur = info["quadros"] / info["fps"]
    if quadro_s is not None:
        ts = [max(0.0, quadro_s + d) for d in np.linspace(-1.0, 1.0, 9)]
    else:
        a, b = janela if janela else (0.05 * dur, 0.95 * dur)
        ts = list(np.linspace(a, b, 21))
    qs = [q for q in (C.quadro_em(cap, t) for t in ts) if q is not None]
    return qs, [round(float(t), 2) for t in ts]


def faixas_de_pretas(cinza, escala):
    """Acha faixas horizontais com 'pente' de teclas pretas (várias corridas escuras estreitas por linha).

    Funciona mesmo quando as pretas encostam na madeira escura acima (a madeira é uma corrida única e larga).
    """
    h, w = cinza.shape
    escuro = (cinza < 90).astype(np.uint8)
    wmin, wmax = int(12 * escala), int(95 * escala)
    contagem = np.zeros(h, int)
    for y in range(h):
        linha = np.concatenate([[0], escuro[y], [0]])
        d = np.diff(linha.astype(np.int8))
        ini, fim = np.where(d == 1)[0], np.where(d == -1)[0]
        comp = fim - ini
        contagem[y] = int(np.sum((comp >= wmin) & (comp <= wmax)))
    pente = contagem >= 8
    faixas, y = [], 0
    while y < h:
        if pente[y]:
            y0 = y
            lacuna = 0
            while y < h and (pente[y] or lacuna < 4):
                lacuna = 0 if pente[y] else lacuna + 1
                y += 1
            y1 = y - lacuna
            if y1 - y0 >= 0.06 * h:
                faixas.append((y0, y1))
        else:
            y += 1
    return escuro, contagem, faixas


def pretas_na_faixa(escuro, faixa, escala):
    y0, y1 = faixa
    # recorta o miolo da faixa (evita bordas arredondadas) e acha colunas majoritariamente escuras
    ya, yb = int(y0 + 0.15 * (y1 - y0)), int(y0 + 0.75 * (y1 - y0))
    frac = escuro[ya:yb].mean(axis=0)
    col = np.concatenate([[0], (frac > 0.6).astype(np.int8), [0]])
    d = np.diff(col)
    ini, fim = np.where(d == 1)[0], np.where(d == -1)[0]
    teclas = []
    for i0, i1 in zip(ini, fim):
        larg = i1 - i0
        if 12 * escala <= larg <= 95 * escala:
            teclas.append((i0 + i1) / 2.0)
    # base e topo reais das pretas: linhas onde a maioria das teclas está escura
    larguras = [(f - i) for i, f in zip(ini, fim) if 12 * escala <= f - i <= 95 * escala]
    return np.array(teclas), float(np.median(larguras)) if larguras else 0.0


def atribuir_grade(centros, w):
    """Atribui a cada preta uma posição na grade 2-3 (Dó#,Ré#,Fá#,Sol#,Lá#) pelos vãos entre vizinhas.

    Vãos esperados (em larguras de branca): Dó#→Ré# 1, Ré#→Fá# 2, Fá#→Sol# 1, Sol#→Lá# 1, Lá#→Dó# 2.
    Escolhe a fase inicial que minimiza o erro e tolera pretas não detectadas (vão maior).
    Devolve (índices_absolutos_na_grade_de_5, erro_médio_em_w, n_pulos).
    """
    vaos = [1, 2, 1, 1, 2]  # vão após a preta de posição p
    melhor = None
    for fase in range(5):
        pos, erro, pulos = [fase], 0.0, 0
        for g in np.diff(centros) / w:
            acum, p, m = 0.0, pos[-1], 0
            while True:
                acum += vaos[p % 5]
                p += 1
                m += 1
                if acum > g - 0.5 or m >= 4:
                    break
            erro += abs(acum - g) + (0.25 * (m - 1))
            pulos += m - 1
            pos.append(p)
        if melhor is None or erro < melhor[1]:
            melhor = (pos, erro, pulos, fase)
    pos, erro, pulos, fase = melhor
    return pos, erro / max(1, len(centros) - 1), pulos, fase


def ajustar_grade(centros, pos):
    """Mínimos quadrados: x = x0 + w * g, onde g é a posição na grade de brancas (Dó da primeira oitava = 0)."""
    g = np.array([7 * (p // 5) + C.GRADE_PRETAS[p % 5] for p in pos], float)
    A = np.stack([np.ones_like(g), g], axis=1)
    (x0, w), *_ = np.linalg.lstsq(A, np.asarray(centros), rcond=None)
    resid = np.asarray(centros) - (x0 + w * g)
    # Pretas reais não ficam exatamente na fronteira entre brancas (Dó#/Fá# vão para a esquerda, Ré#/Lá# para a
    # direita): estima o desvio médio por posição da grade e reajusta uma vez.
    classes = np.array([p % 5 for p in pos])
    desvio = np.zeros(5)
    for c in range(5):
        if np.sum(classes == c) >= 2:
            desvio[c] = resid[classes == c].mean()
    g2 = g + desvio[classes] / max(w, 1e-6)
    A = np.stack([np.ones_like(g2), g2], axis=1)
    (x0, w), *_ = np.linalg.lstsq(A, np.asarray(centros), rcond=None)
    resid = np.asarray(centros) - (x0 + w * g2)
    return float(x0), float(w), resid, (desvio / w).tolist()


def poligono_branca(i, x0, w, y_topo, y_pretas, y_base, bw):
    """Polígono em T da tecla branca de índice i (cortes onde há pretas)."""
    classe = C.CLASSE_BRANCAS[i % 7]
    preta_esq = classe in (2, 4, 7, 9, 11)  # Ré Mi Sol Lá Si têm preta à esquerda
    preta_dir = classe in (0, 2, 5, 7, 9)   # Dó Ré Fá Sol Lá têm preta à direita
    xa, xb = x0 + i * w, x0 + (i + 1) * w
    ea = xa + bw / 2 if preta_esq else xa
    eb = xb - bw / 2 if preta_dir else xb
    return [[ea, y_topo], [eb, y_topo], [eb, y_pretas], [xb, y_pretas], [xb, y_base], [xa, y_base], [xa, y_pretas], [ea, y_pretas]]


def construir(x0, w, desvio_w, bw, y_topo, y_pretas, y_base, largura, centros_detectados, conf_global):
    # Dó4: Dó (índice múltiplo de 7) cujo centro está mais perto do centro horizontal do quadro
    ks = np.arange(-4, 12)
    k4 = int(ks[np.argmin(np.abs(x0 + (7 * ks + 0.5) * w - largura / 2))])
    midi_dobase = 60 - 12 * k4
    teclas = {}
    i_min, i_max = int(np.floor(-x0 / w)) - 1, int(np.ceil((largura - x0) / w)) + 1
    for i in range(i_min, i_max + 1):
        xa, xb = x0 + i * w, x0 + (i + 1) * w
        vis = (min(xb, largura) - max(xa, 0)) / w
        if vis <= 0.05:
            continue
        midi = midi_dobase + 12 * (i // 7) + C.CLASSE_BRANCAS[i % 7]
        poly = poligono_branca(i, x0, w, y_topo, y_pretas, y_base, bw)
        xs, ys = [p[0] for p in poly], [p[1] for p in poly]
        teclas[midi] = {
            "tipo": "branca",
            "nome": C.nome_nota(midi),
            "poligono": [[round(a, 1), round(b, 1)] for a, b in poly],
            "retangulo": [round(min(xs), 1), round(min(ys), 1), round(max(xs), 1), round(max(ys), 1)],
            "fonte": "grade",
            "confianca": round(conf_global * min(1.0, 0.5 + vis / 2), 3),
        }
    for o in range(-1, 14):
        for p in range(5):
            g = 7 * o + C.GRADE_PRETAS[p]
            cx = x0 + (g + desvio_w[p]) * w
            if not (0 <= cx <= largura):
                continue
            midi = midi_dobase + 12 * o + C.CLASSE_PRETAS[p]
            det = any(abs(cx - c) < 0.3 * w for c in centros_detectados)
            r = [cx - bw / 2, y_topo, cx + bw / 2, y_pretas]
            teclas[midi] = {
                "tipo": "preta",
                "nome": C.nome_nota(midi),
                "poligono": [[round(r[0], 1), r[1]], [round(r[2], 1), r[1]], [round(r[2], 1), r[3]], [round(r[0], 1), r[3]]],
                "retangulo": [round(v, 1) for v in r],
                "fonte": "detectada" if det else "inferida",
                "confianca": round(conf_global * (1.0 if det else 0.7), 3),
            }
    return dict(sorted(teclas.items())), midi_dobase, k4


def base_das_brancas(cinza, x0, w, y_pretas, altura, h_preta):
    """Fim das teclas brancas: onde o brilho das colunas de branca cai (madeira escura)."""
    cols = [int(x0 + (i + 0.5) * w) for i in range(int(-x0 // w), int((cinza.shape[1] - x0) // w))]
    cols = [c for c in cols if 3 <= c < cinza.shape[1] - 3]
    perfil = np.array([np.median([cinza[y, c - 2:c + 3].mean() for c in cols]) for y in range(y_pretas, altura)])
    ref = np.median(perfil[: max(5, int(0.1 * h_preta))])
    for k, v in enumerate(perfil):
        if v < 0.55 * ref:
            y = y_pretas + k
            if 1.3 * h_preta <= y - (y_pretas - h_preta) + 0 <= 2.0 * h_preta + h_preta:
                return float(y), "borda detectada"
            break
    return float(y_pretas + 0.55 * h_preta), "razão 1,55 (padrão de piano)"


def desenhar(quadro, cal, caminho):
    img = quadro.copy()
    over = img.copy()
    for midi, t in cal["teclas"].items():
        pts = np.array(t["poligono"], np.int32)
        cor = (255, 120, 40) if t["tipo"] == "preta" else (60, 200, 60)
        if int(midi) == 60:
            cor = (0, 0, 255)
        cv2.fillPoly(over, [pts], cor)
    img = cv2.addWeighted(over, 0.28, img, 0.72, 0)
    for midi, t in cal["teclas"].items():
        pts = np.array(t["poligono"], np.int32)
        cv2.polylines(img, [pts], True, (0, 0, 255) if int(midi) == 60 else (0, 255, 255), 2 if int(midi) == 60 else 1)
        if t["nome"].startswith("Dó") and t["tipo"] == "branca":
            x = int(t["retangulo"][0] + 3)
            y = int(t["retangulo"][3] - 8)
            rot = f"C{int(midi) // 12 - 1} ({midi})"  # só ASCII: o putText do OpenCV não desenha acentos
            cv2.putText(img, rot, (x, y), cv2.FONT_HERSHEY_SIMPLEX, 0.45, (0, 0, 0), 3)
            cv2.putText(img, rot, (x, y), cv2.FONT_HERSHEY_SIMPLEX, 0.45, (255, 255, 255), 1)
    txt = f"confianca {cal['confianca']:.2f} | C4 (vermelho) = C mais perto do centro: CONFIRA"
    cv2.rectangle(img, (0, img.shape[0] - 28), (640, img.shape[0]), (0, 0, 0), -1)
    cv2.putText(img, txt, (10, img.shape[0] - 8), cv2.FONT_HERSHEY_SIMPLEX, 0.5, (255, 255, 255), 1)
    cv2.imwrite(str(caminho), img)


def escolher_faixa(faixas, qs, modo):
    """Escolhe a faixa de pretas do teclado real (a filmada), não a de um teclado gráfico sobreposto.

    'auto': a faixa mais próxima das pontas dos dedos detectadas pelo MediaPipe nos quadros de amostra
    (sem mãos detectadas: a mais baixa no quadro).
    """
    if len(faixas) == 1:
        return faixas[0], "única candidata"
    if modo == "superior":
        return faixas[0], "forçada: superior"
    if modo == "inferior":
        return faixas[-1], "forçada: inferior"
    ys = []
    try:
        det, mp = C.criar_detector_maos()
        for q in qs[:: max(1, len(qs) // 8)]:
            for m in C.detectar_maos(det, mp, q):
                ys += [m["pontos"][i][1] for i in C.PONTAS.values()]
        det.close()
    except SystemExit:
        raise
    if ys:
        ym = float(np.median(ys))
        f = min(faixas, key=lambda f: abs((f[0] + f[1]) / 2 - ym) if not (f[0] <= ym <= f[1] + 0.8 * (f[1] - f[0])) else 0)
        return f, f"mais próxima das pontas dos dedos (y mediano {ym:.0f}px)"
    return faixas[-1], "sem mãos detectadas: a mais baixa"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("video")
    ap.add_argument("--saida", default="calibracao.json")
    ap.add_argument("--quadro-s", type=float, help="instante (s) de um quadro sem mão; usa a mediana de 9 quadros em volta")
    ap.add_argument("--janela-s", help="a,b: restringe a amostra de quadros (ex.: só o trecho do exercício)")
    ap.add_argument("--faixa", choices=["auto", "superior", "inferior"], default="auto")
    a = ap.parse_args()

    cap = C.abrir_video(a.video)
    info = C.info_video(cap)
    janela = tuple(map(float, a.janela_s.split(","))) if a.janela_s else None
    qs, ts = quadros_de_amostra(cap, info, a.quadro_s, janela)
    if not qs:
        raise SystemExit("Nenhum quadro lido do vídeo.")
    mediana = np.median(np.stack(qs), axis=0).astype(np.uint8)
    cinza = cv2.cvtColor(mediana, cv2.COLOR_BGR2GRAY)
    H, W = cinza.shape
    escala = W / 1280.0

    escuro, contagem, faixas = faixas_de_pretas(cinza, escala)
    if not faixas:
        raise SystemExit("Nenhuma faixa de teclas pretas encontrada: vídeo sem a vista de cima do teclado?")
    faixa, por_que = escolher_faixa(faixas, qs, a.faixa)
    centros, bw = pretas_na_faixa(escuro, faixa, escala)
    if len(centros) < 5:
        raise SystemExit(f"Só {len(centros)} teclas pretas detectadas na faixa {faixa}; calibração impossível.")
    w_est = float(np.median(np.diff(centros)[np.diff(centros) < 1.4 * np.min(np.diff(centros))]))
    pos, erro_vaos, pulos, fase = atribuir_grade(centros, w_est)
    x0, w, resid, desvio_w = ajustar_grade(centros, pos)
    rms = float(np.sqrt(np.mean(resid**2)))

    y_topo, y_pretas = float(faixa[0]), float(faixa[1])
    h_preta = y_pretas - y_topo
    y_base, y_base_metodo = base_das_brancas(cinza, x0, w, int(y_pretas), H, h_preta)

    # confiança global: ajuste da grade (resíduo relativo), padrão 2-3 coerente, nº de pretas
    c_ajuste = float(np.clip(1.0 - rms / (0.15 * w), 0, 1))
    c_padrao = float(np.clip(1.0 - erro_vaos, 0, 1))
    c_qtd = float(min(1.0, len(centros) / 15))
    conf = round(0.4 * c_ajuste + 0.4 * c_padrao + 0.2 * c_qtd, 3)

    teclas, midi_dobase, k4 = construir(x0, w, desvio_w, bw, y_topo, y_pretas, y_base, W, list(centros), conf)
    cal = {
        "formato_versao": 1,
        "estado": "hipotese",
        "metodo": "opencv-pretas-padrao-2-3",
        "proveniencia": {
            "ferramenta": "fabrica/video/calibrar.py",
            "video": Path(a.video).name,
            "quadros_mediana_s": ts,
            "bibliotecas": {"opencv": cv2.__version__, "numpy": np.__version__},
        },
        "quadro": {"largura": W, "altura": H, "fps": info["fps"]},
        "confianca": conf,
        "confianca_detalhe": {"ajuste_grade": round(c_ajuste, 3), "padrao_2_3": round(c_padrao, 3), "quantidade_pretas": round(c_qtd, 3)},
        "faixa_teclado": {
            "escolha": por_que,
            "candidatas_y": [[int(a_), int(b_)] for a_, b_ in faixas],
            "y_topo_pretas": y_topo,
            "y_base_pretas": y_pretas,
            "y_base_brancas": y_base,
            "y_base_brancas_metodo": y_base_metodo,
        },
        "grade": {
            "x0_dó_referencia": round(x0, 2),
            "largura_branca_px": round(w, 2),
            "largura_preta_px": round(bw, 2),
            "residuo_rms_px": round(rms, 2),
            "desvio_pretas_por_posicao_em_w": [round(d, 3) for d in desvio_w],
            "pretas_detectadas_x": [round(float(c), 1) for c in centros],
            "pretas_puladas": pulos,
        },
        "suposicoes": [
            {
                "regra": "Dó mais próximo do centro horizontal do quadro = Dó central (C4, midi 60)",
                "confirmar": "O revisor deve conferir no PNG; o Luciano costuma pôr o polegar da mão direita no Dó central no começo.",
                "dó_central_x_px": round(x0 + (7 * k4 + 0.5) * w, 1),
            }
        ],
        "teclas": {str(m): t for m, t in teclas.items()},
    }
    saida = Path(a.saida)
    saida.write_text(json.dumps(cal, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    png = saida.with_suffix(".png")
    desenhar(mediana, cal, png)
    print(f"Calibração: confiança {conf} | brancas {w:.1f}px | pretas detectadas {len(centros)} | resíduo {rms:.2f}px | faixa {faixa} ({por_que})")
    print(f"C4 (midi 60) em x≈{cal['suposicoes'][0]['dó_central_x_px']}px; base das brancas y={y_base:.0f} ({y_base_metodo})")
    print(f"Gravado: {saida} e {png}")


if __name__ == "__main__":
    sys.exit(main())
