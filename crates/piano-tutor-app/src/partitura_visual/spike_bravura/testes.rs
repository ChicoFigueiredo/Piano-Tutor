//! Testes e captura do spike Bravura (ticket 09). A captura pinta a cena no egui sem janela,
//! tessela e rasteriza em software o que o egui produziu (texturas de fonte inclusas), para o
//! checklist visual humano. Nada daqui roda em produção.

use super::*;
use bancada_casca::egui::epaint::{ImageData, ImageDelta, Primitive};
use bancada_casca::egui::{Context, RawInput, TextureId, Vec2, vec2};
use piano_tutor_fixtures::corpus::{Fixture, corpus};
use std::path::Path;
use std::time::Instant;

// ---- rasterização (software) do que o egui tesselou ----

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

struct Imagem {
    w: usize,
    h: usize,
    /// RGB, opaco.
    rgb: Vec<u8>,
}

struct Tempos {
    pintura_ms: f64,
    tesselacao_ms: f64,
}

fn entrada(tela: Vec2) -> RawInput {
    RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), tela)),
        ..Default::default()
    }
}

/// Pinta a cena no egui (sem janela), tessela e rasteriza em software.
fn capturar(cena: &Cena, met: &Metricas, esp: f32, largura_px: usize) -> (Imagem, Tempos) {
    let pal = Paleta::para_tema(false);
    let altura_px = ((cena.altura * esp).ceil() as usize + 8).clamp(8, 16_000);
    let largura_px = largura_px.clamp(8, 8_000);
    let tela = vec2(largura_px as f32, altura_px as f32);
    let ctx = Context::default();
    ctx.set_fonts(fontes_com_bravura());
    let mut texturas: HashMap<TextureId, Textura> = HashMap::new();
    // Primeiro quadro: aplica as fontes novas. Os deltas de textura de todos os quadros entram
    // na mesma tabela.
    let mut vazio = ctx.run_ui(entrada(tela), |_ui| {});
    for (id, ds) in &vazio.textures_delta.set {
        for d in ds {
            aplicar(&mut texturas, *id, d);
        }
    }
    vazio.textures_delta.clear();
    let t0 = Instant::now();
    let mut saida = ctx.run_ui(entrada(tela), |ui| {
        let p = ui.painter().clone();
        p.rect_filled(Rect::from_min_size(pos2(0.0, 0.0), tela), 0.0, pal.papel);
        pintar_cena(&p, pos2(4.0, 4.0), esp, cena, met, &pal);
    });
    let pintura_ms = t0.elapsed().as_secs_f64() * 1e3;
    for (id, ds) in &saida.textures_delta.set {
        for d in ds {
            aplicar(&mut texturas, *id, d);
        }
    }
    saida.textures_delta.clear();
    let t1 = Instant::now();
    let prims = ctx.tessellate(std::mem::take(&mut saida.shapes), 1.0);
    let tesselacao_ms = t1.elapsed().as_secs_f64() * 1e3;
    let fundo = pal.papel;
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
            let min_x = v
                .iter()
                .map(|q| q.pos.x)
                .fold(f32::MAX, f32::min)
                .max(clip.min.x);
            let max_x = v
                .iter()
                .map(|q| q.pos.x)
                .fold(f32::MIN, f32::max)
                .min(clip.max.x);
            let min_y = v
                .iter()
                .map(|q| q.pos.y)
                .fold(f32::MAX, f32::min)
                .max(clip.min.y);
            let max_y = v
                .iter()
                .map(|q| q.pos.y)
                .fold(f32::MIN, f32::max)
                .min(clip.max.y);
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
    (
        Imagem {
            w: largura_px,
            h: altura_px,
            rgb,
        },
        Tempos {
            pintura_ms,
            tesselacao_ms,
        },
    )
}

fn gravar_png(caminho: &Path, img: &Imagem) {
    let arq = std::fs::File::create(caminho).expect("criar png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(arq), img.w as u32, img.h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().expect("cabeçalho png");
    w.write_image_data(&img.rgb).expect("dados png");
}

// ---- ajudantes ----

fn cena_do_arquivo(
    f: &Fixture,
    largura_sp: f32,
    met: &Metricas,
) -> Result<(Partitura, Layout, Cena), String> {
    let p = Partitura::de_musicxml(&f.conteudo).map_err(|e| e.to_string())?;
    let layout = calcular_layout(&p, largura_sp);
    let extras = extrair_extras(&f.conteudo);
    let cena = montar_cena(&p, &layout, &extras, met);
    Ok((p, layout, cena))
}

/// Pinta sem janela, só para provar que nada derruba (largura zero, tela mínima, escala ruim).
fn pintar_headless(cena: &Cena, met: &Metricas, esp: f32, tela: Vec2) {
    let ctx = Context::default();
    ctx.set_fonts(fontes_com_bravura());
    let mut primeiro = ctx.run_ui(entrada(tela), |_ui| {});
    primeiro.textures_delta.clear();
    let mut saida = ctx.run_ui(entrada(tela), |ui| {
        let p = ui.painter().clone();
        pintar_cena(&p, pos2(0.0, 0.0), esp, cena, met, &Paleta::para_tema(true));
    });
    saida.textures_delta.clear();
    let _ = ctx.tessellate(saida.shapes, 1.0);
}

// ---- testes ----

#[test]
fn todo_glifo_usado_existe_na_metadata_da_bravura() {
    let met = Metricas::carregar();
    for (nome, c) in CODIGOS {
        assert!(met.tem(nome), "glifo SMuFL ausente da metadata: {nome}");
        assert!(('\u{E000}'..='\u{F8FF}').contains(c), "{nome} fora da PUA");
    }
    // Valores de referência do próprio JSON oficial.
    assert!((met.pauta - 0.13).abs() < 1e-6);
    assert!((met.haste - 0.12).abs() < 1e-6);
    let cab = met.medida("noteheadBlack");
    assert!((cab.avanco - 1.18).abs() < 1e-6);
}

#[test]
fn verovio_nao_e_dependencia_do_cargo() {
    // O Verovio (LGPL) só pode existir como processo separado, num script de desenvolvimento.
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for arq in ["Cargo.lock", "Cargo.toml"] {
        let t = std::fs::read_to_string(raiz.join(arq)).expect("ler");
        assert!(
            !t.to_lowercase().contains("verovio"),
            "{arq} menciona verovio"
        );
    }
    for entrada in std::fs::read_dir(raiz.join("crates")).expect("crates") {
        let toml = entrada.expect("entrada").path().join("Cargo.toml");
        if let Ok(t) = std::fs::read_to_string(&toml) {
            assert!(
                !t.to_lowercase().contains("verovio"),
                "{} menciona verovio",
                toml.display()
            );
        }
    }
}

#[test]
fn corpus_inteiro_desenha_sem_panico_em_tamanhos_extremos() {
    let met = Metricas::carregar();
    let mut nao_lidas: Vec<(&str, String)> = Vec::new();
    let mut n = 0;
    for f in corpus() {
        for zoom in [0.5f32, 1.0, 1.5, 3.0] {
            let esp = ESP_BASE_PX * zoom;
            // Larguras: zero, menor que o mínimo, o mínimo e a normal da tela.
            for largura_sp in [0.0f32, 10.0, 45.0, 1000.0 / esp] {
                match cena_do_arquivo(&f, largura_sp, &met) {
                    Ok((_, _, cena)) => {
                        for tela in [vec2(0.0, 0.0), vec2(1.0, 1.0), vec2(300.0, 200.0)] {
                            pintar_headless(&cena, &met, esp, tela);
                            n += 1;
                        }
                        // Escalas degeneradas: nunca pintam nem derrubam.
                        for ruim in [0.0, -1.0, f32::NAN, f32::INFINITY, 0.001] {
                            pintar_headless(&cena, &met, ruim, vec2(100.0, 100.0));
                            n += 1;
                        }
                    }
                    Err(e) => {
                        if !nao_lidas.iter().any(|(id, _)| *id == f.id) {
                            nao_lidas.push((f.id, e));
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "SPIKE pinturas sem pânico: {n}; fixtures que o modelo atual rejeitou: {nao_lidas:?}"
    );
    assert!(
        nao_lidas.is_empty(),
        "o modelo atual rejeitou: {nao_lidas:?}"
    );
}

#[test]
fn cena_tem_letra_cifra_dedilhado_tercina_repeticao_e_identidade_de_nota() {
    let met = Metricas::carregar();
    let c = |id: &str| {
        let f = piano_tutor_fixtures::corpus::fixture(id);
        cena_do_arquivo(&f, 100.0, &met).expect("lê").2
    };
    let letra = c("letra").contagens();
    assert_eq!(letra.get("letra"), Some(&6), "{letra:?}");
    let cifra = c("cifra").contagens();
    assert_eq!(cifra.get("cifra"), Some(&3), "{cifra:?}");
    let dedo = c("dedilhado").contagens();
    assert!(dedo.get("dedilhado").copied().unwrap_or(0) >= 6, "{dedo:?}");
    let tercina = c("tercina").contagens();
    assert!(
        tercina.get("tercina").copied().unwrap_or(0) >= 1,
        "{tercina:?}"
    );
    let rep = c("repeticao").contagens();
    assert!(rep.get("repeticao").copied().unwrap_or(0) >= 2, "{rep:?}");
    assert!(c("anacruse").notas >= 1);
}

#[test]
fn glifo_da_bravura_chega_ao_egui_na_posicao_da_metadata() {
    // Cabeça preta com a origem em (3, 4) sp e 20 px/sp: a tinta tem de cobrir a caixa da
    // metadata, com folga de 1,5 px.
    let met = Metricas::carregar();
    let cena = Cena {
        elems: vec![Elem {
            tipo: Tipo::Cabeca,
            desenho: Desenho::Glifo {
                nome: "noteheadBlack",
                x: 3.0,
                y: 4.0,
                escala: 1.0,
            },
            cor: Cor::Tinta,
            nota: Some(IdNota(1)),
            compasso: None,
        }],
        largura: 20.0,
        altura: 10.0,
        ..Cena::default()
    };
    let esp = 20.0;
    let (img, _) = capturar(&cena, &met, esp, 600);
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for y in 0..img.h {
        for x in 0..img.w {
            let i = (y * img.w + x) * 3;
            // Tinta: bem mais escura que o papel branco.
            if img.rgb[i] < 128 && img.rgb[i + 1] < 128 {
                x0 = x0.min(x);
                x1 = x1.max(x);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
    }
    assert!(
        x0 != usize::MAX,
        "nenhum pixel de tinta: a Bravura não chegou ao egui"
    );
    let m = met.medida("noteheadBlack");
    let esperado = [
        4.0 + (3.0 + m.sw.x) * esp,
        4.0 + (4.0 - m.ne.y) * esp,
        4.0 + (3.0 + m.ne.x) * esp,
        4.0 + (4.0 - m.sw.y) * esp,
    ];
    let obtido = [x0 as f32, y0 as f32, x1 as f32 + 1.0, y1 as f32 + 1.0];
    eprintln!("SPIKE caixa esperada {esperado:?} obtida {obtido:?}");
    for (a, b) in esperado.iter().zip(obtido.iter()) {
        assert!(
            (a - b).abs() <= 1.5,
            "esperado {esperado:?}, obtido {obtido:?}"
        );
    }
}

#[test]
fn hit_test_por_nota_acha_a_cabeca_e_rejeita_o_vazio() {
    let met = Metricas::carregar();
    let f = piano_tutor_fixtures::corpus::fixture("brilha-brilha");
    let (_, _, cena) = cena_do_arquivo(&f, 100.0, &met).expect("lê");
    let cabecas: Vec<&Elem> = cena
        .elems
        .iter()
        .filter(|e| e.tipo == Tipo::Cabeca)
        .collect();
    assert!(!cabecas.is_empty());
    for e in cabecas.iter().take(40) {
        let c = e.caixa(&met);
        let centro = ponto((c.min.x + c.max.x) / 2.0, (c.min.y + c.max.y) / 2.0);
        assert_eq!(cena.nota_em(&met, centro, 0.0), e.nota, "{:?}", e.desenho);
    }
    assert_eq!(
        cena.nota_em(&met, ponto(-50.0, -50.0), TOLERANCIA_NOTA_SP),
        None
    );
    assert_eq!(cena.notas as usize, cabecas.len());
}

#[test]
fn colisoes_so_contam_simbolos_e_textos() {
    let met = Metricas::carregar();
    let f = piano_tutor_fixtures::corpus::fixture("duas-vozes");
    let (_, _, cena) = cena_do_arquivo(&f, 100.0, &met).expect("lê");
    for (a, b) in cena.colisoes(&met) {
        assert!(cena.elems[a].tipo.conta_colisao());
        assert!(cena.elems[b].tipo.conta_colisao());
    }
}

// ---- exportação para o oráculo (opcional): PT_SPIKE_SAIDA=<pasta> ----

fn desc_elem(e: &Elem) -> Value {
    let nome = match &e.desenho {
        Desenho::Glifo { nome, .. } => (*nome).to_owned(),
        Desenho::Texto { texto, .. } => texto.clone(),
        _ => String::new(),
    };
    serde_json::json!({"tipo": e.tipo.nome(), "nome": nome, "compasso": e.compasso})
}

/// Gera, para cada fixture do corpus: o MusicXML, a captura em PNG a 100% e 150% (pelo egui,
/// com a Bravura) e o relatório JSON do lado próprio, que o script em Python junta ao
/// resultado do Verovio. Não roda por padrão (precisa da pasta de saída).
#[test]
#[ignore = "gera artefatos de comparação; rode com PT_SPIKE_SAIDA=<pasta> -- --ignored"]
fn exportar_corpus_para_comparacao() {
    let Ok(saida) = std::env::var("PT_SPIKE_SAIDA") else {
        eprintln!("PT_SPIKE_SAIDA não definido: nada a exportar");
        return;
    };
    let saida = Path::new(&saida);
    std::fs::create_dir_all(saida).expect("pasta de saída");
    let met = Metricas::carregar();
    let mut pecas = Vec::new();
    for f in corpus() {
        std::fs::write(saida.join(format!("{}.musicxml", f.id)), &f.conteudo).expect("xml");
        let mut zooms = Vec::new();
        let mut erro: Option<String> = None;
        for zoom in [1.0f32, 1.5] {
            let esp = ESP_BASE_PX * zoom;
            let largura_px = 1100usize;
            let t0 = Instant::now();
            let r = cena_do_arquivo(&f, (largura_px as f32 - 12.0) / esp, &met);
            let (p, layout, cena) = match r {
                Ok(v) => v,
                Err(e) => {
                    erro = Some(e);
                    break;
                }
            };
            let montagem_ms = t0.elapsed().as_secs_f64() * 1e3;
            let (img, tempos) = capturar(&cena, &met, esp, largura_px);
            let nome_png = format!("{}-{}.png", f.id, (zoom * 100.0) as u32);
            gravar_png(&saida.join(&nome_png), &img);
            let colisoes = cena.colisoes(&met);
            let sistemas: Vec<Value> = cena
                .sistemas
                .iter()
                .map(|s| {
                    let ms: Vec<Value> = (s.primeiro..s.fim)
                        .map(|m| {
                            let (x0, x1) = (cena.compassos[m].x0, cena.compassos[m].x1);
                            // Centro de cada cabeça como fração da largura do compasso.
                            let mut cabecas: Vec<f32> = cena
                                .elems
                                .iter()
                                .filter(|e| e.tipo == Tipo::Cabeca && e.compasso == Some(m))
                                .map(|e| {
                                    let c = e.caixa(&met);
                                    ((c.min.x + c.max.x) / 2.0 - x0) / (x1 - x0).max(1e-6)
                                })
                                .collect();
                            cabecas.sort_by(f32::total_cmp);
                            serde_json::json!({
                                "compasso": p.compassos[m].numero,
                                "sistema": cena.compassos[m].sistema,
                                "x0": x0,
                                "x1": x1,
                                "cabecas": cabecas,
                            })
                        })
                        .collect();
                    serde_json::json!({"x0": s.x0, "x1": s.x1, "y_topo": s.y_topo,
                        "y_base": s.y_base, "compassos": ms})
                })
                .collect();
            // Hit test: 2.000 consultas espalhadas sobre a peça.
            let t1 = Instant::now();
            let mut achou = 0usize;
            let n_consultas = 2000usize;
            for i in 0..n_consultas {
                let fx = (i * 7919 % 1000) as f32 / 1000.0;
                let fy = (i * 104_729 % 1000) as f32 / 1000.0;
                let pt = ponto(fx * cena.largura, fy * cena.altura);
                if cena.nota_em(&met, pt, TOLERANCIA_NOTA_SP).is_some() {
                    achou += 1;
                }
            }
            let hit_us = t1.elapsed().as_secs_f64() * 1e6 / n_consultas as f64;
            let mut por_par: BTreeMap<String, usize> = BTreeMap::new();
            for (a, b) in &colisoes {
                let (ta, tb) = (cena.elems[*a].tipo.nome(), cena.elems[*b].tipo.nome());
                let (ta, tb) = if ta <= tb { (ta, tb) } else { (tb, ta) };
                *por_par.entry(format!("{ta} x {tb}")).or_insert(0) += 1;
            }
            let col_desc: Vec<Value> = colisoes
                .iter()
                .take(40)
                .map(|(a, b)| {
                    serde_json::json!({"a": desc_elem(&cena.elems[*a]), "b": desc_elem(&cena.elems[*b])})
                })
                .collect();
            zooms.push(serde_json::json!({
                "zoom": zoom,
                "esp_px": esp,
                "largura_px": largura_px,
                "png": nome_png,
                "n_sistemas": cena.sistemas.len(),
                "n_compassos": p.compassos.len(),
                "contagens": cena.contagens(),
                "n_colisoes": colisoes.len(),
                "colisoes_por_par": por_par,
                "colisoes_amostra": col_desc,
                "sistemas": sistemas,
                "layout_atual": {"n_sistemas": layout.sistemas.len()},
                "tempos": {
                    "layout_e_cena_ms": montagem_ms,
                    "pintura_egui_ms": tempos.pintura_ms,
                    "tesselacao_ms": tempos.tesselacao_ms,
                    "hit_test_us_por_consulta": hit_us,
                    "hit_test_acertos_em_2000": achou,
                },
            }));
        }
        pecas.push(
            serde_json::json!({"id": f.id, "arquivo": f.arquivo, "erro_modelo": erro, "zooms": zooms}),
        );
    }
    let doc = serde_json::json!({"bravura": "1.482", "pecas": pecas});
    std::fs::write(
        saida.join("relatorio-proprio.json"),
        serde_json::to_string_pretty(&doc).expect("json"),
    )
    .expect("gravar relatório");
}
