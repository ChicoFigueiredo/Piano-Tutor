//! Testes do ticket 19 sobre o layout de letra, cifra e dedilhado e sobre a captura de tela do
//! render de produção (A18: cada texto tem posição e está associado à `IdNota`; a legibilidade
//! continua NÃO VERIFICADA até o checklist visual do titular).

use super::captura_testes::capturar_partitura;
use super::textos::{ALTURA_CIFRA, ALTURA_LETRA, PASSO_DE_ESTROFE};
use super::*;
use piano_tutor_core::documento::DocumentoAberto;
use piano_tutor_fixtures::exibicao::COMBINADA;

fn aberto() -> DocumentoAberto {
    DocumentoAberto::abrir_texto("combinada", COMBINADA).expect("abre")
}

fn itens(l: &Layout) -> Vec<&Item> {
    l.sistemas.iter().flat_map(|s| s.itens.iter()).collect()
}

fn cabeca_de(l: &Layout, id: IdNota) -> Option<(f32, f32)> {
    itens(l)
        .into_iter()
        .find_map(|it| match (&it.forma, it.nota) {
            (Forma::Cabeca { centro, .. }, Some(n)) if n == id => Some((centro.x, centro.y)),
            _ => None,
        })
}

/// Posição (x, y) dos itens de texto com `texto` ligados à nota `id`.
fn texto_da_nota(l: &Layout, id: IdNota, texto: &str) -> Vec<(f32, f32)> {
    itens(l)
        .into_iter()
        .filter_map(|it| match (&it.forma, it.nota) {
            (Forma::Texto { pos, texto: t, .. }, Some(n)) if n == id && t == texto => {
                Some((pos.x, pos.y))
            }
            _ => None,
        })
        .collect()
}

/// Posições dos textos de cifra com `texto` (a cifra não carrega identidade no item).
fn textos_de_cifra(l: &Layout, texto: &str) -> Vec<(f32, f32)> {
    itens(l)
        .into_iter()
        .filter_map(|it| match &it.forma {
            Forma::Texto {
                pos,
                texto: t,
                altura,
                ..
            } if t == texto && (*altura - ALTURA_CIFRA).abs() < 1e-3 => Some((pos.x, pos.y)),
            _ => None,
        })
        .collect()
}

/// Topo (y, em sp) de cada pauta de um sistema: as linhas horizontais de pauta, de cinco em cinco.
fn topos_das_pautas(s: &Sistema) -> Vec<f32> {
    let mut ys: Vec<f32> = s
        .itens
        .iter()
        .filter_map(|it| match &it.forma {
            Forma::Linha { a, b, espessura }
                if (*espessura - LINHA_PAUTA).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6 =>
            {
                Some(a.y)
            }
            _ => None,
        })
        .collect();
    ys.sort_by(f32::total_cmp);
    ys.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    ys.chunks(5).map(|c| c[0]).collect()
}

fn texto_de_letra(it: &Item) -> Option<(f32, f32, &str)> {
    match &it.forma {
        Forma::Texto {
            pos, altura, texto, ..
        } if (*altura - ALTURA_LETRA).abs() < 1e-3 && texto != "-" => {
            Some((pos.x, pos.y, texto.as_str()))
        }
        _ => None,
    }
}

#[test]
fn a18_letra_cifra_e_dedilhado_sao_desenhados_junto_das_notas_a_que_pertencem() {
    let d = aberto();
    let ex = d.documento().exibicao();
    let l = calcular_layout_com_exibicao(d.partitura(), d.ids(), &ex, 110.0);
    let mut silabas = 0;
    let mut dedos = 0;
    for (id, e) in ex.notas() {
        let cabeca = cabeca_de(&l, id).expect("a nota tem cabeça no layout");
        for s in &e.silabas {
            // Cada sílaba é um texto, com posição, ligado à IdNota e centrado na cabeça.
            let achados = texto_da_nota(&l, id, &s.texto);
            // (a mesma sílaba pode se repetir em outra estrofe da mesma nota)
            let esperadas = e.silabas.iter().filter(|o| o.texto == s.texto).count();
            assert_eq!(
                achados.len(),
                esperadas,
                "sílaba {:?} da nota {id:?}",
                s.texto
            );
            assert!(
                (achados[0].0 - cabeca.0).abs() < 0.2,
                "a sílaba {:?} está a {} sp da cabeça",
                s.texto,
                achados[0].0 - cabeca.0
            );
            assert!(achados[0].1 > cabeca.1, "a letra fica abaixo da nota");
            silabas += 1;
        }
        if !e.dedos.is_empty() {
            let glifos: Vec<(f32, f32, char)> = itens(&l)
                .into_iter()
                .filter_map(|it| match (&it.forma, it.nota) {
                    (Forma::Glifo { pos, caractere, .. }, Some(n)) if n == id => {
                        Some((pos.x, pos.y, *caractere))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(glifos.len(), e.dedos.len(), "glifos do dedilhado de {id:?}");
            for (gx, gy, c) in &glifos {
                assert!(('\u{ED10}'..='\u{ED15}').contains(c), "glifo SMuFL: {c:?}");
                assert!((gx - cabeca.0).abs() < 2.0, "dedilhado junto da cabeça");
                // Do lado oposto da haste e perto da cabeça (nunca em cima dela).
                let dy = (gy - cabeca.1).abs();
                assert!((1.0..4.5).contains(&dy), "dedilhado a {dy} sp da cabeça");
            }
            dedos += e.dedos.len();
        }
    }
    assert_eq!(silabas, ex.total_de_silabas());
    assert_eq!(dedos, ex.total_de_dedos());
    for c in ex.cifras() {
        let id = c.nota.expect("âncora");
        let textos = textos_de_cifra(&l, &c.texto);
        assert!(!textos.is_empty(), "cifra {} desenhada", c.texto);
        if let Some((cx, _)) = cabeca_de(&l, id) {
            assert!(
                textos.iter().any(|(x, _)| (x - cx).abs() < 0.5),
                "a cifra {} está sobre a nota {id:?}",
                c.texto
            );
        }
    }
}

#[test]
fn tres_estrofes_reservam_uma_linha_cada_e_nao_invadem_a_pauta_de_baixo() {
    let d = aberto();
    let ex = d.documento().exibicao();
    let com = calcular_layout_com_exibicao(d.partitura(), d.ids(), &ex, 110.0);
    let sem = calcular_layout_com_ids(d.partitura(), d.ids(), 110.0);
    for s in &com.sistemas {
        let topos = topos_das_pautas(s);
        assert_eq!(topos.len(), 3, "voz e piano de duas pautas");
        let mut linhas: Vec<f32> = s
            .itens
            .iter()
            .filter_map(texto_de_letra)
            .map(|(_, y, _)| y)
            .collect();
        assert!(!linhas.is_empty());
        linhas.sort_by(f32::total_cmp);
        linhas.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
        assert_eq!(linhas.len(), 3, "uma linha por estrofe: {linhas:?}");
        assert!((linhas[1] - linhas[0] - PASSO_DE_ESTROFE).abs() < 1e-4);
        assert!((linhas[2] - linhas[1] - PASSO_DE_ESTROFE).abs() < 1e-4);
        // A primeira linha está abaixo da pauta da voz e a última termina antes da pauta
        // seguinte (a altura de algarismo é ALTURA_LETRA; o corpo, ~1,4 vez isso).
        assert!(linhas[0] > topos[0] + ALTURA_PAUTA);
        let fim_da_ultima = linhas[2] + ALTURA_LETRA * 1.4;
        assert!(
            fim_da_ultima < topos[1] - 0.5,
            "a 3ª estrofe termina em {fim_da_ultima} e a pauta de baixo começa em {}",
            topos[1]
        );
    }
    // O espaço entre a voz e o piano cresceu por causa da letra (era o defeito do spike).
    let gap = |l: &Layout| {
        let t = topos_das_pautas(&l.sistemas[0]);
        t[1] - t[0]
    };
    assert!(
        gap(&com) > gap(&sem) + 2.0,
        "com letra {} sp, sem letra {} sp",
        gap(&com),
        gap(&sem)
    );
}

#[test]
fn a_cifra_fica_acima_da_pauta_e_do_numero_do_compasso() {
    let d = aberto();
    let ex = d.documento().exibicao();
    let l = calcular_layout_com_exibicao(d.partitura(), d.ids(), &ex, 110.0);
    let mut cifras = 0;
    for s in &l.sistemas {
        let topo = topos_das_pautas(s)[0];
        for it in &s.itens {
            let Forma::Texto {
                pos, altura, texto, ..
            } = &it.forma
            else {
                continue;
            };
            if (*altura - ALTURA_CIFRA).abs() < 1e-3 {
                // Âncora embaixo: a base do texto fica acima do número do compasso (que ocupa
                // de topo-1,1 até uns 1,3 sp acima) e das casas de repetição.
                assert!(
                    pos.y <= topo - 2.9,
                    "cifra {texto} em {} (pauta {topo})",
                    pos.y
                );
                cifras += 1;
            }
        }
    }
    assert_eq!(cifras, ex.cifras().len());
}

#[test]
fn silabas_da_mesma_linha_nao_se_tocam_em_pecas_reais() {
    // A lacuna 1 do spike: "sílabas se tocam" porque a largura do texto não entrava no
    // espaçamento. Aqui, em peças reais de voz e piano, cada texto começa depois do anterior.
    for arq in [
        include_str!("../../../../assets/partituras/brahms-wiegenlied-op49-4.musicxml"),
        include_str!("../../../../assets/partituras/schubert-heidenroeslein-d257.musicxml"),
    ] {
        let d = DocumentoAberto::abrir_texto("lieder", arq).expect("abre");
        let ex = d.documento().exibicao();
        assert!(ex.total_de_silabas() > 50);
        let l = calcular_layout_com_exibicao(d.partitura(), d.ids(), &ex, 110.0);
        for s in &l.sistemas {
            // (y, x, largura estimada) de cada sílaba, ordenadas por linha e por x.
            let mut por_linha: Vec<(f32, f32, f32)> = s
                .itens
                .iter()
                .filter_map(texto_de_letra)
                .map(|(x, y, t)| (y, x, t.chars().count() as f32 * ALTURA_LETRA * 0.66))
                .collect();
            por_linha.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
            for par in por_linha.windows(2) {
                let (a, b) = (par[0], par[1]);
                if (a.0 - b.0).abs() > 1e-4 {
                    continue;
                }
                let folga = b.1 - a.1 - (a.2 + b.2) / 2.0;
                assert!(
                    folga > 0.0,
                    "sílabas se tocam: folga {folga} sp entre {a:?} e {b:?}"
                );
            }
        }
    }
}

#[test]
fn sem_exibicao_ou_com_exibicao_vazia_o_layout_e_o_de_antes() {
    for texto in [
        include_str!("../../../../assets/partituras/beethoven-ode-a-alegria-tema.musicxml"),
        include_str!("../../../../assets/partituras/brahms-wiegenlied-op49-4.musicxml"),
    ] {
        let d = DocumentoAberto::abrir_texto("x", texto).expect("abre");
        let antes = calcular_layout_com_ids(d.partitura(), d.ids(), 110.0);
        let vazia = Exibicao::default();
        let depois = calcular_layout_com_exibicao(d.partitura(), d.ids(), &vazia, 110.0);
        assert_eq!(antes.altura, depois.altura);
        assert_eq!(antes.sistemas.len(), depois.sistemas.len());
        for (a, b) in antes.sistemas.iter().zip(&depois.sistemas) {
            // A ordem de desenho de itens equivalentes não é determinística: compara como conjunto.
            let ordenados = |s: &Sistema| {
                let mut v: Vec<String> = s.itens.iter().map(|i| format!("{i:?}")).collect();
                v.sort();
                v
            };
            assert_eq!(ordenados(a), ordenados(b));
            assert_eq!((a.y_topo, a.y_base), (b.y_topo, b.y_base));
        }
    }
}

#[test]
fn o_texto_nunca_rouba_o_clique_da_nota() {
    let d = aberto();
    let ex = d.documento().exibicao();
    let l = calcular_layout_com_exibicao(d.partitura(), d.ids(), &ex, 110.0);
    let mut n = 0;
    for it in itens(&l) {
        if let (Forma::Cabeca { centro, .. }, Some(id)) = (&it.forma, it.nota) {
            // O hit test só considera cabeças: letra, cifra e dedilhado não são alvo de clique.
            assert_eq!(l.nota_em(*centro, 0.0), Some(id));
            n += 1;
        }
    }
    assert_eq!(
        n,
        d.documento().notas().filter(|n| n.altura.is_some()).count()
    );
}

#[test]
fn captura_de_tela_mostra_tinta_na_faixa_da_letra() {
    let d = aberto();
    let ex = d.documento().exibicao();
    let (zoom, largura) = (1.0f32, 1100usize);
    let img = capturar_partitura(d.partitura(), Some(d.ids()), Some(&ex), zoom, largura, true);
    let esp = ESP_BASE_PX * zoom;
    let l =
        calcular_layout_com_exibicao(d.partitura(), d.ids(), &ex, (largura as f32 - 12.0) / esp);
    let topos = topos_das_pautas(&l.sistemas[0]);
    // O topo do layout fica a poucos pixels do canto da imagem: a faixa tem folga de 24 px.
    let faixa =
        |y0: f32, y1: f32| img.tinta_em(0, img.w, (y0 * esp) as usize, (y1 * esp) as usize + 24);
    let entre = faixa(topos[0] + ALTURA_PAUTA + 1.0, topos[1] - 0.5);
    assert!(entre > 300, "só {entre} pixels de tinta na faixa da letra");
    assert!(img.tinta_em(0, img.w, 0, img.h) > 5_000);
}

#[test]
fn dedilhado_sem_a_familia_bravura_cai_no_algarismo_sem_panico() {
    let d = aberto();
    let ex = d.documento().exibicao();
    let img = capturar_partitura(d.partitura(), Some(d.ids()), Some(&ex), 1.0, 900, false);
    assert!(img.tinta_em(0, img.w, 0, img.h) > 3_000);
}

#[test]
fn desenha_em_zoom_extremo_fita_e_largura_minima_sem_panico() {
    let d = aberto();
    let ex = d.documento().exibicao();
    for zoom in [ZOOM_MIN, ZOOM_MAX] {
        let _ = capturar_partitura(d.partitura(), Some(d.ids()), Some(&ex), zoom, 300, true);
    }
    let fita = calcular_layout_fita_com_exibicao(d.partitura(), d.ids(), &ex);
    assert_eq!(fita.sistemas.len(), 1);
    assert!(
        fita.sistemas[0]
            .itens
            .iter()
            .all(|it| forma_finita(&it.forma))
    );
}

/// Gera as capturas para o titular quando `PT_CAPTURAS_19` aponta a pasta de saída; sem a
/// variável, não grava nada (os testes comuns não escrevem no repositório).
#[test]
fn gera_capturas_para_revisao_quando_pedido() {
    let Ok(pasta) = std::env::var("PT_CAPTURAS_19") else {
        return;
    };
    let pasta = std::path::PathBuf::from(pasta);
    std::fs::create_dir_all(&pasta).expect("pasta");
    let d = aberto();
    let ex = d.documento().exibicao();
    for (nome, zoom) in [("combinada-100.png", 1.0), ("combinada-150.png", 1.5)] {
        let img = capturar_partitura(d.partitura(), Some(d.ids()), Some(&ex), zoom, 1100, true);
        img.gravar_png(&pasta.join(nome));
    }
    for (nome, arq) in [
        (
            "brahms-wiegenlied-100.png",
            include_str!("../../../../assets/partituras/brahms-wiegenlied-op49-4.musicxml"),
        ),
        (
            "schubert-heidenroeslein-100.png",
            include_str!("../../../../assets/partituras/schubert-heidenroeslein-d257.musicxml"),
        ),
    ] {
        let d = DocumentoAberto::abrir_texto(nome, arq).expect("abre");
        let ex = d.documento().exibicao();
        let img = capturar_partitura(d.partitura(), Some(d.ids()), Some(&ex), 1.0, 1100, true);
        // Só o topo da peça: o suficiente para ver letra e piano sem arquivos enormes.
        img.recortar(0, 0, img.w, img.h.min(560))
            .gravar_png(&pasta.join(nome));
    }
}
