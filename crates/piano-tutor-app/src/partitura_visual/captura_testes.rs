//! Captura de tela sem janela do render **de produção** (ticket 19): pinta a partitura pelo
//! mesmo caminho do app (`desenhar_partitura`, com a Bravura registrada), tessela e rasteriza em
//! software o que o egui produziu (texturas de fonte inclusas). A rasterização é a do spike do
//! ticket 09 (`spike_bravura/testes.rs`), reduzida ao necessário. Só testes; as capturas servem à
//! revisão do titular e **não** provam legibilidade (GPU real, HiDPI e olho humano: NÃO VERIFICADO).

use super::*;
use bancada_casca::egui::epaint::{ImageData, ImageDelta, Primitive};
use bancada_casca::egui::{Context, Rect, TextureId, Vec2, pos2, vec2};
use std::collections::HashMap;
use std::path::Path;

pub(super) struct Imagem {
    pub w: usize,
    pub h: usize,
    /// RGB, opaco.
    pub rgb: Vec<u8>,
}

impl Imagem {
    /// Quantos pixels de tinta (bem mais escuros que o papel branco) há na faixa de linhas
    /// `[y0, y1)` e colunas `[x0, x1)`.
    pub fn tinta_em(&self, x0: usize, x1: usize, y0: usize, y1: usize) -> usize {
        let mut n = 0;
        for y in y0.min(self.h)..y1.min(self.h) {
            for x in x0.min(self.w)..x1.min(self.w) {
                let i = (y * self.w + x) * 3;
                if self.rgb[i] < 140 && self.rgb[i + 1] < 140 && self.rgb[i + 2] < 140 {
                    n += 1;
                }
            }
        }
        n
    }

    pub fn gravar_png(&self, caminho: &Path) {
        let arq = std::fs::File::create(caminho).expect("criar png");
        let mut enc = png::Encoder::new(std::io::BufWriter::new(arq), self.w as u32, self.h as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().expect("cabeçalho png");
        w.write_image_data(&self.rgb).expect("dados png");
    }

    /// Recorte (em pixels) como nova imagem.
    pub fn recortar(&self, x0: usize, y0: usize, w: usize, h: usize) -> Imagem {
        let (x0, y0) = (x0.min(self.w), y0.min(self.h));
        let (w, h) = (w.min(self.w - x0), h.min(self.h - y0));
        let mut rgb = Vec::with_capacity(w * h * 3);
        for y in y0..y0 + h {
            rgb.extend_from_slice(&self.rgb[(y * self.w + x0) * 3..(y * self.w + x0 + w) * 3]);
        }
        Imagem { w, h, rgb }
    }
}

struct Textura {
    w: usize,
    h: usize,
    px: Vec<[u8; 4]>,
}

fn aplicar(texturas: &mut HashMap<TextureId, Textura>, id: TextureId, d: &ImageDelta) {
    let ImageData::Color(img) = &d.image;
    let [w, h] = img.size;
    let novos: Vec<[u8; 4]> = img.pixels.iter().map(|c| c.to_array()).collect();
    match d.pos {
        None => {
            texturas.insert(id, Textura { w, h, px: novos });
        }
        Some([x0, y0]) => {
            if let Some(t) = texturas.get_mut(&id) {
                for y in 0..h {
                    for x in 0..w {
                        let (tx, ty) = (x0 + x, y0 + y);
                        if tx < t.w && ty < t.h {
                            t.px[ty * t.w + tx] = novos[y * w + x];
                        }
                    }
                }
            }
        }
    }
}

fn amostrar(t: &Textura, u: f32, v: f32) -> [f32; 4] {
    let x = (u * t.w as f32 - 0.5).clamp(0.0, t.w as f32 - 1.0);
    let y = (v * t.h as f32 - 0.5).clamp(0.0, t.h as f32 - 1.0);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(t.w - 1), (y0 + 1).min(t.h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let mut out = [0.0f32; 4];
    for c in 0..4 {
        let a =
            f32::from(t.px[y0 * t.w + x0][c]) * (1.0 - fx) + f32::from(t.px[y0 * t.w + x1][c]) * fx;
        let b =
            f32::from(t.px[y1 * t.w + x0][c]) * (1.0 - fx) + f32::from(t.px[y1 * t.w + x1][c]) * fx;
        out[c] = (a * (1.0 - fy) + b * fy) / 255.0;
    }
    out
}

fn entrada(tela: Vec2) -> bancada_casca::egui::RawInput {
    bancada_casca::egui::RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), tela)),
        ..Default::default()
    }
}

/// Pinta `p` pelo caminho do app (ids e exibição opcionais) e rasteriza. `largura_px` é a da
/// janela; a altura acompanha o layout inteiro. `registrar_bravura` liga a família Bravura (como
/// a casca faz na partida do app); sem ela, o dedilhado cai no algarismo comum.
pub(super) fn capturar_partitura(
    p: &Partitura,
    ids: Option<&IdsPorNota>,
    exibicao: Option<&Exibicao>,
    zoom: f32,
    largura_px: usize,
    registrar_bravura: bool,
) -> Imagem {
    let esp = ESP_BASE_PX * zoom;
    let largura_sp = ((largura_px as f32 - 12.0) / esp).max(LARGURA_MINIMA);
    let layout = match (ids, exibicao) {
        (Some(i), Some(e)) => calcular_layout_com_exibicao(p, i, e, largura_sp),
        (Some(i), None) => calcular_layout_com_ids(p, i, largura_sp),
        _ => calcular_layout(p, largura_sp),
    };
    let altura_px = ((layout.altura * esp).ceil() as usize + 24).clamp(40, 16_000);
    let tela = vec2(largura_px as f32, altura_px as f32);
    let ctx = Context::default();
    if registrar_bravura {
        ctx.set_fonts(bravura::fontes_com_bravura());
    }
    let mut texturas: HashMap<TextureId, Textura> = HashMap::new();
    let mut estado = EstadoVisual::default();
    let opc = OpcoesVisuais {
        zoom,
        seguir: false,
        modo: ModoRolagem::Sistemas,
        papel_branco: true,
    };
    let marcas = Marcas {
        ids,
        exibicao,
        ..Marcas::default()
    };
    // Dois quadros: o primeiro aplica as fontes; os deltas de textura de todos entram na tabela.
    let mut resultado = None;
    for _ in 0..2 {
        let mut saida = ctx.run_ui(entrada(tela), |ui| {
            desenhar_partitura(ui, p, &mut estado, opc, marcas);
        });
        for (id, ds) in &saida.textures_delta.set {
            for d in ds {
                aplicar(&mut texturas, *id, d);
            }
        }
        saida.textures_delta.clear();
        resultado = Some(std::mem::take(&mut saida.shapes));
    }
    let shapes = resultado.unwrap_or_default();
    let prims = ctx.tessellate(shapes, 1.0);
    let fundo = Paleta::para_tema_com(false, false).papel;
    let mut buf: Vec<[f32; 3]> = vec![
        [
            f32::from(fundo.r()) / 255.0,
            f32::from(fundo.g()) / 255.0,
            f32::from(fundo.b()) / 255.0
        ];
        largura_px * altura_px
    ];
    for prim in &prims {
        let Primitive::Mesh(mesh) = &prim.primitive else {
            continue;
        };
        let Some(tex) = texturas.get(&mesh.texture_id) else {
            continue;
        };
        let clip = prim.clip_rect;
        for t in mesh.indices.as_chunks::<3>().0 {
            let v = [
                mesh.vertices[t[0] as usize],
                mesh.vertices[t[1] as usize],
                mesh.vertices[t[2] as usize],
            ];
            let area = (v[1].pos.x - v[0].pos.x) * (v[2].pos.y - v[0].pos.y)
                - (v[2].pos.x - v[0].pos.x) * (v[1].pos.y - v[0].pos.y);
            if area.abs() < 1e-9 {
                continue;
            }
            let lim =
                |f: fn(f32, f32) -> f32,
                 ini: f32,
                 eixo: fn(&bancada_casca::egui::epaint::Vertex) -> f32| {
                    v.iter().map(eixo).fold(ini, f)
                };
            let min_x = lim(f32::min, f32::MAX, |q| q.pos.x).max(clip.min.x);
            let max_x = lim(f32::max, f32::MIN, |q| q.pos.x).min(clip.max.x);
            let min_y = lim(f32::min, f32::MAX, |q| q.pos.y).max(clip.min.y);
            let max_y = lim(f32::max, f32::MIN, |q| q.pos.y).min(clip.max.y);
            let (xa, xb) = (
                min_x.floor().max(0.0) as usize,
                max_x.ceil().max(0.0) as usize,
            );
            let (ya, yb) = (
                min_y.floor().max(0.0) as usize,
                max_y.ceil().max(0.0) as usize,
            );
            for y in ya..yb.min(altura_px) {
                for x in xa..xb.min(largura_px) {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let e = |a: usize, b: usize| {
                        (v[b].pos.x - v[a].pos.x) * (py - v[a].pos.y)
                            - (v[b].pos.y - v[a].pos.y) * (px - v[a].pos.x)
                    };
                    let (w0, w1, w2) = (e(1, 2) / area, e(2, 0) / area, e(0, 1) / area);
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let u = w0 * v[0].uv.x + w1 * v[1].uv.x + w2 * v[2].uv.x;
                    let vv = w0 * v[0].uv.y + w1 * v[1].uv.y + w2 * v[2].uv.y;
                    let tx = amostrar(tex, u, vv);
                    let mut col = [0.0f32; 4];
                    let (c0, c1, c2) = (
                        v[0].color.to_array(),
                        v[1].color.to_array(),
                        v[2].color.to_array(),
                    );
                    for c in 0..4 {
                        let mix =
                            w0 * f32::from(c0[c]) + w1 * f32::from(c1[c]) + w2 * f32::from(c2[c]);
                        col[c] = tx[c] * mix / 255.0;
                    }
                    let dst = &mut buf[y * largura_px + x];
                    for c in 0..3 {
                        dst[c] = col[c] + dst[c] * (1.0 - col[3]);
                    }
                }
            }
        }
    }
    let mut rgb = Vec::with_capacity(buf.len() * 3);
    for p in &buf {
        for c in p {
            rgb.push((c.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    Imagem {
        w: largura_px,
        h: altura_px,
        rgb,
    }
}
