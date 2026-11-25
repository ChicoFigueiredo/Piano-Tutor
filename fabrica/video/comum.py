"""Funções compartilhadas da calibração e do dedilhado por vídeo (fábrica)."""
from __future__ import annotations

import os
from pathlib import Path

import cv2
import numpy as np

AQUI = Path(__file__).resolve().parent
MODELO_MAOS = AQUI / "modelos" / "hand_landmarker.task"
# Pontas dos dedos no MediaPipe Hands: polegar=4, indicador=8, médio=12, anelar=16, mínimo=20.
PONTAS = {1: 4, 2: 8, 3: 12, 4: 16, 5: 20}
# Dó=0 ... Si=11; teclas pretas por posição dentro da oitava (em larguras de tecla branca a partir do Dó).
CLASSE_BRANCAS = [0, 2, 4, 5, 7, 9, 11]
GRADE_PRETAS = [1, 2, 4, 5, 6]  # Dó#, Ré#, Fá#, Sol#, Lá#
CLASSE_PRETAS = [1, 3, 6, 8, 10]
NOMES = ["Dó", "Dó#", "Ré", "Ré#", "Mi", "Fá", "Fá#", "Sol", "Sol#", "Lá", "Lá#", "Si"]


def nome_nota(midi: int) -> str:
    return f"{NOMES[midi % 12]}{midi // 12 - 1}"


def abrir_video(caminho: str) -> cv2.VideoCapture:
    cap = cv2.VideoCapture(caminho)
    if not cap.isOpened():
        raise SystemExit(f"Não consegui abrir o vídeo: {caminho}")
    return cap


def info_video(cap: cv2.VideoCapture) -> dict:
    return {
        "largura": int(cap.get(cv2.CAP_PROP_FRAME_WIDTH)),
        "altura": int(cap.get(cv2.CAP_PROP_FRAME_HEIGHT)),
        "fps": float(cap.get(cv2.CAP_PROP_FPS)),
        "quadros": int(cap.get(cv2.CAP_PROP_FRAME_COUNT)),
    }


def quadro_em(cap: cv2.VideoCapture, t_s: float):
    """Quadro mais próximo do instante t_s (BGR) ou None."""
    fps = cap.get(cv2.CAP_PROP_FPS) or 30.0
    cap.set(cv2.CAP_PROP_POS_FRAMES, max(0, int(round(t_s * fps))))
    ok, q = cap.read()
    return q if ok else None


def criar_detector_maos(modelo: Path = MODELO_MAOS, max_maos: int = 2, conf: float = 0.3):
    """HandLandmarker do MediaPipe Tasks em modo IMAGE (CPU)."""
    import mediapipe as mp
    from mediapipe.tasks import python as mpp
    from mediapipe.tasks.python import vision

    if not modelo.exists():
        raise SystemExit(
            f"Modelo ausente: {modelo}\nVeja o README (baixar hand_landmarker.task do repositório oficial do MediaPipe)."
        )
    opcoes = vision.HandLandmarkerOptions(
        base_options=mpp.BaseOptions(model_asset_path=str(modelo)),
        running_mode=vision.RunningMode.IMAGE,
        num_hands=max_maos,
        min_hand_detection_confidence=conf,
        min_hand_presence_confidence=conf,
        min_tracking_confidence=conf,
    )
    return vision.HandLandmarker.create_from_options(opcoes), mp


def detectar_maos(detector, mp, quadro_bgr, recorte_y: tuple[int, int] | None = None, margem: int = 150) -> list[dict]:
    """Devolve, por mão: pontos (21x2 em pixels), rótulo do MediaPipe e lado geométrico.

    O rótulo do MediaPipe assume imagem espelhada de selfie e não é confiável numa vista de cima;
    por isso o lado também é deduzido da geometria (ver `lado_geometrico`).
    """
    # Com `recorte_y`, detecta só na faixa do teclado filmado, com margem replicada em cima e embaixo:
    # as mãos tocam a borda inferior do quadro e, assim, o MediaPipe não as perde (medido: sem a margem,
    # 0 mãos em quase todo o trecho da Aula 14; com ela, 2).
    y0 = 0
    img = quadro_bgr
    if recorte_y is not None:
        y0 = max(0, int(recorte_y[0]))
        img = cv2.copyMakeBorder(quadro_bgr[y0 : int(recorte_y[1])], margem, margem, 0, 0, cv2.BORDER_REPLICATE)
    h, w = img.shape[:2]
    rgb = cv2.cvtColor(img, cv2.COLOR_BGR2RGB)
    res = detector.detect(mp.Image(image_format=mp.ImageFormat.SRGB, data=rgb))
    maos = []
    desloc_y = y0 - (margem if recorte_y is not None else 0)
    for i, lms in enumerate(res.hand_landmarks):
        pts = np.array([[p.x * w, p.y * h + desloc_y] for p in lms], dtype=float)
        cat = res.handedness[i][0]
        maos.append(
            {
                "pontos": pts,
                "rotulo_mediapipe": cat.category_name,
                "pontuacao_rotulo": float(cat.score),
                "lado_geometrico": lado_geometrico(pts),
            }
        )
    return maos


def lado_geometrico(pts: np.ndarray) -> str:
    """Lado da mão pelo dorso visto de cima: indicador à esquerda do mínimo com os dedos para cima = direita.

    Usa o sinal do produto vetorial (mínimo-MCP − indicador-MCP) x (médio-MCP − pulso), com y para baixo.
    """
    v1 = pts[17] - pts[5]
    v2 = pts[9] - pts[0]
    z = v1[0] * v2[1] - v1[1] * v2[0]
    return "direita" if z < 0 else "esquerda"
