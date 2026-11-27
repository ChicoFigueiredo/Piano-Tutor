//! Spike do ticket 09 (E01): protótipo de render com a fonte SMuFL **Bravura** (OFL-1.1) sobre o
//! layout próprio, para decidir o motor de render (ADR-0018) contra o Verovio como oráculo.
//!
//! **Só existe em testes** (`#[cfg(test)]`): nada daqui entra no app nem muda o desenho atual
//! (o golden de primitivas do ticket 06 segue valendo). O protótipo **reaproveita** do layout
//! atual o que é independente de fonte (quebra em sistemas, espaçamento horizontal, colunas por
//! tempo, análise de vozes, vigas e colunas de acidentes) e **redesenha** em glifos SMuFL tudo o
//! que hoje é forma própria: claves, armadura, fôrma, cabeças, pausas, acidentes, bandeirolas,
//! pontos de aumento, número de tercina, pontos de repetição e a chave. Acrescenta o que o modelo
//! atual não tem (letra, cifra e dedilhado), lidos direto do MusicXML, como texto junto às notas.
//!
//! A cena (`Cena`) é neutra: o mesmo conteúdo vai para o egui (`pintar_cena`), para a captura
//! em PNG (rasterização do que o egui tesselou) e para o relatório JSON comparado, em processo
//! separado, com o Verovio (`scripts/spike-render/comparar_verovio.py`). O Verovio **nunca**
//! é linkado nem roda na CI.

use super::*;
use crate::geometria::Alinha;
use bancada_casca::egui::{
    Align, Align2, FontFamily, FontId, Painter, Pos2, Rect, Shape, Stroke, pos2,
};
use serde_json::Value;
use std::collections::BTreeMap;

#[cfg(test)]
mod testes;

pub const BRAVURA_OTF: &[u8] = include_bytes!("../../../../assets/fontes/bravura/Bravura.otf");
pub const BRAVURA_JSON: &str = include_str!("../../../../assets/fontes/bravura/Bravura.json");
/// Nome da família registrada no egui.
pub const FAMILIA: &str = "Bravura";

// --- Glifos usados (nome SMuFL -> ponto de código; conferidos contra o OTF e a metadata) -----

const CODIGOS: &[(&str, char)] = &[
    ("gClef", '\u{E050}'),
    ("gClef8vb", '\u{E052}'),
    ("gClef8va", '\u{E053}'),
    ("cClef", '\u{E05C}'),
    ("fClef", '\u{E062}'),
    ("fClef8vb", '\u{E064}'),
    ("fClef8va", '\u{E065}'),
    ("timeSigCommon", '\u{E08A}'),
    ("timeSigCutCommon", '\u{E08B}'),
    ("noteheadDoubleWhole", '\u{E0A0}'),
    ("noteheadWhole", '\u{E0A2}'),
    ("noteheadHalf", '\u{E0A3}'),
    ("noteheadBlack", '\u{E0A4}'),
    ("augmentationDot", '\u{E1E7}'),
    ("flag8thUp", '\u{E240}'),
    ("flag8thDown", '\u{E241}'),
    ("flag16thUp", '\u{E242}'),
    ("flag16thDown", '\u{E243}'),
    ("flag32ndUp", '\u{E244}'),
    ("flag32ndDown", '\u{E245}'),
    ("flag64thUp", '\u{E246}'),
    ("flag64thDown", '\u{E247}'),
    ("flag128thUp", '\u{E248}'),
    ("flag128thDown", '\u{E249}'),
    ("restDoubleWhole", '\u{E4E2}'),
    ("restWhole", '\u{E4E3}'),
    ("restHalf", '\u{E4E4}'),
    ("restQuarter", '\u{E4E5}'),
    ("rest8th", '\u{E4E6}'),
    ("rest16th", '\u{E4E7}'),
    ("rest32nd", '\u{E4E8}'),
    ("rest64th", '\u{E4E9}'),
    ("rest128th", '\u{E4EA}'),
    ("accidentalFlat", '\u{E260}'),
    ("accidentalNatural", '\u{E261}'),
    ("accidentalSharp", '\u{E262}'),
    ("accidentalDoubleSharp", '\u{E263}'),
    ("accidentalDoubleFlat", '\u{E264}'),
    ("brace", '\u{E000}'),
    ("repeatDots", '\u{E043}'),
    ("repeatDot", '\u{E044}'),
    ("timeSig0", '\u{E080}'),
    ("timeSig1", '\u{E081}'),
    ("timeSig2", '\u{E082}'),
    ("timeSig3", '\u{E083}'),
    ("timeSig4", '\u{E084}'),
    ("timeSig5", '\u{E085}'),
    ("timeSig6", '\u{E086}'),
    ("timeSig7", '\u{E087}'),
    ("timeSig8", '\u{E088}'),
    ("timeSig9", '\u{E089}'),
    ("tuplet0", '\u{E880}'),
    ("tuplet1", '\u{E881}'),
    ("tuplet2", '\u{E882}'),
    ("tuplet3", '\u{E883}'),
    ("tuplet4", '\u{E884}'),
    ("tuplet5", '\u{E885}'),
    ("tuplet6", '\u{E886}'),
    ("tuplet7", '\u{E887}'),
    ("tuplet8", '\u{E888}'),
    ("tuplet9", '\u{E889}'),
    ("fingering0", '\u{ED10}'),
    ("fingering1", '\u{ED11}'),
    ("fingering2", '\u{ED12}'),
    ("fingering3", '\u{ED13}'),
    ("fingering4", '\u{ED14}'),
    ("fingering5", '\u{ED15}'),
];

const NOMES_HORA: [&str; 10] = [
    "timeSig0", "timeSig1", "timeSig2", "timeSig3", "timeSig4", "timeSig5", "timeSig6", "timeSig7",
    "timeSig8", "timeSig9",
];
const NOMES_TUPLA: [&str; 10] = [
    "tuplet0", "tuplet1", "tuplet2", "tuplet3", "tuplet4", "tuplet5", "tuplet6", "tuplet7",
    "tuplet8", "tuplet9",
];
const NOMES_DEDO: [&str; 6] = [
    "fingering0",
    "fingering1",
    "fingering2",
    "fingering3",
    "fingering4",
    "fingering5",
];

fn codigo(nome: &str) -> char {
    CODIGOS
        .iter()
        .find(|(n, _)| *n == nome)
        .map_or('\u{FFFD}', |(_, c)| *c)
}

// --- Métricas SMuFL (bravura_metadata) -------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct Medida {
    sw: Ponto,
    ne: Ponto,
    avanco: f32,
}

/// Métricas lidas do JSON oficial da Bravura (1 sp = distância entre linhas; y para cima).
pub struct Metricas {
    glifos: HashMap<String, Medida>,
    ancoras: HashMap<String, HashMap<String, Ponto>>,
    pub pauta: f32,
    pub haste: f32,
    pub viga: f32,
    pub viga_espaco: f32,
    pub suplementar: f32,
    pub suplementar_ext: f32,
    pub barra_fina: f32,
    pub barra_grossa: f32,
    pub barra_sep: f32,
}

fn num(v: &Value) -> f32 {
    v.as_f64().unwrap_or(0.0) as f32
}

fn par(v: &Value) -> Ponto {
    ponto(num(&v[0]), num(&v[1]))
}

impl Metricas {
    pub fn carregar() -> Self {
        let j: Value = serde_json::from_str(BRAVURA_JSON).expect("Bravura.json válido");
        let mut glifos = HashMap::new();
        if let Some(caixas) = j["glyphBBoxes"].as_object() {
            for (nome, c) in caixas {
                let avanco = num(&j["glyphAdvanceWidths"][nome.as_str()]);
                glifos.insert(
                    nome.clone(),
                    Medida {
                        sw: par(&c["bBoxSW"]),
                        ne: par(&c["bBoxNE"]),
                        avanco,
                    },
                );
            }
        }
        let mut ancoras: HashMap<String, HashMap<String, Ponto>> = HashMap::new();
        if let Some(o) = j["glyphsWithAnchors"].as_object() {
            for (nome, ancs) in o {
                let mut m = HashMap::new();
                if let Some(a) = ancs.as_object() {
                    for (k, v) in a {
                        m.insert(k.clone(), par(v));
                    }
                }
                ancoras.insert(nome.clone(), m);
            }
        }
        let d = &j["engravingDefaults"];
        Self {
            glifos,
            ancoras,
            pauta: num(&d["staffLineThickness"]),
            haste: num(&d["stemThickness"]),
            viga: num(&d["beamThickness"]),
            viga_espaco: num(&d["beamSpacing"]),
            suplementar: num(&d["legerLineThickness"]),
            suplementar_ext: num(&d["legerLineExtension"]),
            barra_fina: num(&d["thinBarlineThickness"]),
            barra_grossa: num(&d["thickBarlineThickness"]),
            barra_sep: num(&d["barlineSeparation"]),
        }
    }

    pub fn tem(&self, nome: &str) -> bool {
        self.glifos.contains_key(nome)
    }

    fn medida(&self, nome: &str) -> Medida {
        self.glifos.get(nome).copied().unwrap_or(Medida {
            sw: ponto(0.0, 0.0),
            ne: ponto(1.0, 1.0),
            avanco: 1.0,
        })
    }

    pub fn avanco(&self, nome: &str) -> f32 {
        self.medida(nome).avanco
    }

    fn ancora(&self, nome: &str, chave: &str) -> Option<Ponto> {
        self.ancoras.get(nome)?.get(chave).copied()
    }
}

// --- Cena ------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tipo {
    Pauta,
    Barra,
    Chave,
    Clave,
    Armadura,
    Forma,
    Cabeca,
    Acidente,
    Ponto,
    Pausa,
    Haste,
    Viga,
    Bandeirola,
    Suplementar,
    Arco,
    Tercina,
    Letra,
    Cifra,
    Dedilhado,
    NumeroCompasso,
    Volta,
    Repeticao,
}

impl Tipo {
    pub fn nome(self) -> &'static str {
        match self {
            Tipo::Pauta => "linha_de_pauta",
            Tipo::Barra => "barra",
            Tipo::Chave => "chave",
            Tipo::Clave => "clave",
            Tipo::Armadura => "armadura",
            Tipo::Forma => "forma_de_compasso",
            Tipo::Cabeca => "cabeca",
            Tipo::Acidente => "acidente",
            Tipo::Ponto => "ponto_de_aumento",
            Tipo::Pausa => "pausa",
            Tipo::Haste => "haste",
            Tipo::Viga => "viga",
            Tipo::Bandeirola => "bandeirola",
            Tipo::Suplementar => "linha_suplementar",
            Tipo::Arco => "arco",
            Tipo::Tercina => "tercina",
            Tipo::Letra => "letra",
            Tipo::Cifra => "cifra",
            Tipo::Dedilhado => "dedilhado",
            Tipo::NumeroCompasso => "numero_de_compasso",
            Tipo::Volta => "casa_de_repeticao",
            Tipo::Repeticao => "repeticao",
        }
    }

    /// Participa da contagem de colisões (símbolos e textos; hastes, vigas, linhas e arcos
    /// atravessam outros símbolos por construção).
    fn conta_colisao(self) -> bool {
        matches!(
            self,
            Tipo::Clave
                | Tipo::Armadura
                | Tipo::Forma
                | Tipo::Cabeca
                | Tipo::Acidente
                | Tipo::Ponto
                | Tipo::Pausa
                | Tipo::Bandeirola
                | Tipo::Tercina
                | Tipo::Letra
                | Tipo::Cifra
                | Tipo::Dedilhado
                | Tipo::NumeroCompasso
                | Tipo::Volta
        )
    }
}

#[derive(Clone, Debug)]
pub enum Desenho {
    /// `(x, y)` é a origem do glifo (linha de base), em sp.
    Glifo {
        nome: &'static str,
        x: f32,
        y: f32,
        escala: f32,
    },
    Linha {
        a: Ponto,
        b: Ponto,
        esp: f32,
    },
    Poligono {
        pontos: Vec<Ponto>,
    },
    Caminho {
        pontos: Vec<Ponto>,
        esp: f32,
    },
    Texto {
        pos: Ponto,
        texto: String,
        altura: f32,
        ancora: Ancora,
    },
}

#[derive(Clone, Debug)]
pub struct Elem {
    pub tipo: Tipo,
    pub desenho: Desenho,
    pub cor: Cor,
    pub nota: Option<IdNota>,
    pub compasso: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct SistemaCena {
    pub y_topo: f32,
    pub y_base: f32,
    pub x0: f32,
    pub x1: f32,
    pub primeiro: usize,
    pub fim: usize,
    /// Faixa de `Cena::elems` deste sistema.
    pub elems: std::ops::Range<usize>,
}

#[derive(Clone, Debug)]
pub struct MedidaCena {
    pub sistema: usize,
    pub x0: f32,
    pub x1: f32,
}

/// Cena: tudo o que o protótipo desenha, em sp, com a identidade das notas.
#[derive(Clone, Debug, Default)]
pub struct Cena {
    pub elems: Vec<Elem>,
    pub largura: f32,
    pub altura: f32,
    pub sistemas: Vec<SistemaCena>,
    pub compassos: Vec<MedidaCena>,
    /// Quantas notas receberam identidade (cabeças).
    pub notas: u64,
}

/// Texto ligado a uma nota ou compasso, extraído do MusicXML (o modelo atual os descarta).
#[derive(Clone, Debug, Default)]
pub struct Extras {
    pub letras: Vec<ExtraLetra>,
    pub cifras: Vec<ExtraCifra>,
    pub dedos: Vec<ExtraDedo>,
}

#[derive(Clone, Debug)]
pub struct ExtraLetra {
    pub parte: usize,
    pub compasso: usize,
    pub pauta: u8,
    pub pos: Fracao,
    pub verso: u8,
    pub texto: String,
}

#[derive(Clone, Debug)]
pub struct ExtraCifra {
    pub parte: usize,
    pub compasso: usize,
    pub pos: Fracao,
    pub texto: String,
}

#[derive(Clone, Debug)]
pub struct ExtraDedo {
    pub parte: usize,
    pub compasso: usize,
    pub pauta: u8,
    pub pos: Fracao,
    pub altura: Altura,
    pub texto: String,
}

fn filho<'a, 'b>(n: roxmltree::Node<'a, 'b>, nome: &str) -> Option<roxmltree::Node<'a, 'b>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == nome)
}

fn texto_de(n: roxmltree::Node<'_, '_>, nome: &str) -> Option<String> {
    filho(n, nome)
        .and_then(|c| c.text())
        .map(|t| t.trim().to_owned())
}

fn passo_de(letra: &str) -> Option<u8> {
    Some(match letra {
        "C" => 0,
        "D" => 1,
        "E" => 2,
        "F" => 3,
        "G" => 4,
        "A" => 5,
        "B" => 6,
        _ => return None,
    })
}

fn alteracao_texto(a: i8) -> &'static str {
    match a {
        1 => "#",
        -1 => "b",
        2 => "##",
        -2 => "bb",
        _ => "",
    }
}

fn nome_da_cifra(h: roxmltree::Node<'_, '_>) -> String {
    let nota = |raiz: roxmltree::Node<'_, '_>, passo: &str, alt: &str| -> String {
        let p = texto_de(raiz, passo).unwrap_or_default();
        let a = texto_de(raiz, alt)
            .and_then(|t| t.parse::<f32>().ok())
            .map_or(0, |v| v.round() as i8);
        format!("{p}{}", alteracao_texto(a))
    };
    let mut s = filho(h, "root")
        .map(|r| nota(r, "root-step", "root-alter"))
        .unwrap_or_default();
    if let Some(k) = filho(h, "kind") {
        let t = k.attribute("text").map(str::to_owned).unwrap_or_else(|| {
            match k.text().map(str::trim) {
                Some("minor") => "m".into(),
                Some("dominant") => "7".into(),
                Some("major-seventh") => "maj7".into(),
                Some("minor-seventh") => "m7".into(),
                Some("diminished") => "dim".into(),
                Some("augmented") => "aug".into(),
                _ => String::new(),
            }
        });
        s.push_str(&t);
    }
    if let Some(b) = filho(h, "bass") {
        s.push('/');
        s.push_str(&nota(b, "bass-step", "bass-alter"));
    }
    s
}

/// Lê letra, cifra e dedilhado do MusicXML (posição em semínimas dentro do compasso).
pub fn extrair_extras(xml: &str) -> Extras {
    let mut ex = Extras::default();
    let Ok(doc) = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            nodes_limit: 4_000_000,
            ..Default::default()
        },
    ) else {
        return ex;
    };
    let partes = doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "part");
    for (ip, parte) in partes.enumerate() {
        let mut divisoes: i64 = 1;
        let medidas = parte
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "measure");
        for (im, medida) in medidas.enumerate() {
            let mut cursor: i64 = 0;
            let mut ultimo_inicio: i64 = 0;
            for el in medida.children().filter(roxmltree::Node::is_element) {
                match el.tag_name().name() {
                    "attributes" => {
                        if let Some(d) = texto_de(el, "divisions").and_then(|t| t.parse().ok()) {
                            divisoes = i64::max(d, 1);
                        }
                    }
                    "backup" => {
                        let d: i64 = texto_de(el, "duration")
                            .and_then(|t| t.parse().ok())
                            .unwrap_or(0);
                        cursor -= d;
                    }
                    "forward" => {
                        let d: i64 = texto_de(el, "duration")
                            .and_then(|t| t.parse().ok())
                            .unwrap_or(0);
                        cursor += d;
                    }
                    "harmony" => {
                        ex.cifras.push(ExtraCifra {
                            parte: ip,
                            compasso: im,
                            pos: Fracao::nova(cursor, divisoes),
                            texto: nome_da_cifra(el),
                        });
                    }
                    "note" => {
                        let acorde = filho(el, "chord").is_some();
                        let grace = filho(el, "grace").is_some();
                        let dur: i64 = texto_de(el, "duration")
                            .and_then(|t| t.parse().ok())
                            .unwrap_or(0);
                        let inicio = if acorde { ultimo_inicio } else { cursor };
                        let pos = Fracao::nova(inicio, divisoes);
                        let pauta: u8 = texto_de(el, "staff")
                            .and_then(|t| t.parse().ok())
                            .unwrap_or(1);
                        for l in el
                            .children()
                            .filter(|c| c.is_element() && c.tag_name().name() == "lyric")
                        {
                            let verso: u8 = l
                                .attribute("number")
                                .and_then(|t| t.parse().ok())
                                .unwrap_or(1);
                            let mut t = texto_de(l, "text").unwrap_or_default();
                            if matches!(
                                texto_de(l, "syllabic").as_deref(),
                                Some("begin" | "middle")
                            ) {
                                t.push('-');
                            }
                            if !t.is_empty() {
                                ex.letras.push(ExtraLetra {
                                    parte: ip,
                                    compasso: im,
                                    pauta,
                                    pos,
                                    verso,
                                    texto: t,
                                });
                            }
                        }
                        let altura = filho(el, "pitch").and_then(|pt| {
                            Some(Altura {
                                passo: passo_de(&texto_de(pt, "step")?)?,
                                alteracao: texto_de(pt, "alter")
                                    .and_then(|t| t.parse::<f32>().ok())
                                    .map_or(0, |v| v.round() as i8),
                                oitava: texto_de(pt, "octave")?.parse().ok()?,
                            })
                        });
                        if let (Some(altura), Some(not)) = (altura, filho(el, "notations"))
                            && let Some(tec) = filho(not, "technical")
                        {
                            for f in tec
                                .children()
                                .filter(|c| c.is_element() && c.tag_name().name() == "fingering")
                            {
                                if let Some(t) = f.text().map(str::trim)
                                    && !t.is_empty()
                                {
                                    ex.dedos.push(ExtraDedo {
                                        parte: ip,
                                        compasso: im,
                                        pauta,
                                        pos,
                                        altura,
                                        texto: t.to_owned(),
                                    });
                                }
                            }
                        }
                        if !grace {
                            if !acorde {
                                ultimo_inicio = cursor;
                                cursor += dur;
                            }
                        } else {
                            ultimo_inicio = cursor;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    ex
}

// --- Geometria de glifos ---------------------------------------------------------------------

impl Elem {
    /// Retângulo envolvente em sp (texto: estimativa de 0,55 da altura por caractere).
    pub fn caixa(&self, met: &Metricas) -> Retangulo {
        match &self.desenho {
            Desenho::Glifo { nome, x, y, escala } => {
                let m = met.medida(nome);
                Retangulo::from_min_max(
                    ponto(x + m.sw.x * escala, y - m.ne.y * escala),
                    ponto(x + m.ne.x * escala, y - m.sw.y * escala),
                )
            }
            Desenho::Linha { a, b, esp } => Retangulo::from_two_pos(*a, *b).expand(esp / 2.0),
            Desenho::Poligono { pontos } | Desenho::Caminho { pontos, .. } => {
                let mut r = Retangulo::NOTHING;
                for p in pontos {
                    r.extend_with(*p);
                }
                r
            }
            Desenho::Texto {
                pos,
                texto,
                altura,
                ancora,
            } => {
                let w = texto.chars().count() as f32 * 0.55 * altura;
                let h = *altura;
                let x0 = match ancora.0[0] {
                    Alinha::Min => pos.x,
                    Alinha::Centro => pos.x - w / 2.0,
                    Alinha::Max => pos.x - w,
                };
                let y0 = match ancora.0[1] {
                    Alinha::Min => pos.y,
                    Alinha::Centro => pos.y - h / 2.0,
                    Alinha::Max => pos.y - h,
                };
                Retangulo::from_min_max(ponto(x0, y0), ponto(x0 + w, y0 + h))
            }
        }
    }
}

impl Cena {
    /// Contagem de elementos por tipo (para o relatório).
    pub fn contagens(&self) -> BTreeMap<&'static str, usize> {
        let mut m = BTreeMap::new();
        for e in &self.elems {
            *m.entry(e.tipo.nome()).or_insert(0) += 1;
        }
        m
    }

    /// Hit test por nota no layout do protótipo: a cabeça (retângulo do glifo, alargado em `tol`
    /// sp) mais próxima do ponto. Só varre os sistemas cuja faixa vertical contém o ponto.
    pub fn nota_em(&self, met: &Metricas, p: Ponto, tol: f32) -> Option<IdNota> {
        let mut melhor: Option<(f32, IdNota)> = None;
        for s in &self.sistemas {
            if p.y < s.y_topo - 4.0 || p.y > s.y_base + 4.0 {
                continue;
            }
            for e in &self.elems[s.elems.clone()] {
                let (Some(id), Tipo::Cabeca) = (e.nota, e.tipo) else {
                    continue;
                };
                let c = e.caixa(met).expand(tol);
                if c.contains(p) {
                    let cx = (c.min.x + c.max.x) / 2.0;
                    let cy = (c.min.y + c.max.y) / 2.0;
                    let d = (p.x - cx).hypot(p.y - cy);
                    if melhor.is_none_or(|(m, _)| d < m) {
                        melhor = Some((d, id));
                    }
                }
            }
        }
        melhor.map(|(_, id)| id)
    }

    /// Colisões entre símbolos e textos (caixas encolhidas em 0,06 sp por lado; interseção de
    /// área maior que 0,01 sp²). Devolve os pares de índices em `elems`.
    pub fn colisoes(&self, met: &Metricas) -> Vec<(usize, usize)> {
        let mut caixas: Vec<(usize, Retangulo)> = self
            .elems
            .iter()
            .enumerate()
            .filter(|(_, e)| e.tipo.conta_colisao())
            .map(|(i, e)| (i, e.caixa(met).expand(-0.06)))
            .filter(|(_, r)| r.min.x <= r.max.x && r.min.y <= r.max.y)
            .collect();
        caixas.sort_by(|a, b| a.1.min.x.total_cmp(&b.1.min.x));
        let mut pares = Vec::new();
        for i in 0..caixas.len() {
            for j in (i + 1)..caixas.len() {
                if caixas[j].1.min.x > caixas[i].1.max.x {
                    break;
                }
                let (a, b) = (caixas[i].1, caixas[j].1);
                let w = a.max.x.min(b.max.x) - a.min.x.max(b.min.x);
                let h = a.max.y.min(b.max.y) - a.min.y.max(b.min.y);
                if w > 0.0 && h > 0.0 && w * h > 0.01 {
                    let (ia, ib) = (caixas[i].0, caixas[j].0);
                    pares.push((ia.min(ib), ia.max(ib)));
                }
            }
        }
        pares.sort_unstable();
        pares
    }
}

// --- Construção da cena ----------------------------------------------------------------------

struct Cons<'a> {
    met: &'a Metricas,
    elems: Vec<Elem>,
    compasso: Option<usize>,
}

impl Cons<'_> {
    fn push(&mut self, tipo: Tipo, desenho: Desenho, nota: Option<IdNota>) {
        self.elems.push(Elem {
            tipo,
            desenho,
            cor: if matches!(tipo, Tipo::NumeroCompasso) {
                Cor::Suave
            } else {
                Cor::Tinta
            },
            nota,
            compasso: self.compasso,
        });
    }

    fn glifo(&mut self, tipo: Tipo, nome: &'static str, x: f32, y: f32, escala: f32) {
        self.push(tipo, Desenho::Glifo { nome, x, y, escala }, None);
    }

    fn glifo_nota(
        &mut self,
        tipo: Tipo,
        nome: &'static str,
        x: f32,
        y: f32,
        escala: f32,
        id: Option<IdNota>,
    ) {
        self.push(tipo, Desenho::Glifo { nome, x, y, escala }, id);
    }

    /// Glifo centrado na caixa em `(cx, cy)`.
    fn glifo_centrado(&mut self, tipo: Tipo, nome: &'static str, cx: f32, cy: f32, escala: f32) {
        let m = self.met.medida(nome);
        let x = cx - (m.sw.x + m.ne.x) / 2.0 * escala;
        let y = cy + (m.sw.y + m.ne.y) / 2.0 * escala;
        self.glifo(tipo, nome, x, y, escala);
    }

    fn linha(&mut self, tipo: Tipo, a: Ponto, b: Ponto, esp: f32) {
        self.push(tipo, Desenho::Linha { a, b, esp }, None);
    }

    fn poligono(&mut self, tipo: Tipo, pontos: Vec<Ponto>) {
        self.push(tipo, Desenho::Poligono { pontos }, None);
    }

    fn texto(&mut self, tipo: Tipo, pos: Ponto, texto: &str, altura: f32, ancora: Ancora) {
        self.push(
            tipo,
            Desenho::Texto {
                pos,
                texto: texto.to_owned(),
                altura,
                ancora,
            },
            None,
        );
    }

    /// Número de vários algarismos centrado em `(cx, cy)`.
    fn numero(
        &mut self,
        tipo: Tipo,
        tabela: &[&'static str],
        n: u32,
        cx: f32,
        cy: f32,
        escala: f32,
    ) {
        let digs: Vec<&'static str> = n
            .to_string()
            .chars()
            .filter_map(|c| c.to_digit(10))
            .filter_map(|d| tabela.get(d as usize).copied())
            .collect();
        let total: f32 = digs.iter().map(|d| self.met.avanco(d) * escala).sum();
        let mut x = cx - total / 2.0;
        for d in digs {
            let w = self.met.avanco(d) * escala;
            self.glifo_centrado(tipo, d, x + w / 2.0, cy, escala);
            x += w;
        }
    }
}

fn nome_cabeca(f: Figura) -> &'static str {
    match f {
        Figura::Breve => "noteheadDoubleWhole",
        Figura::Semibreve => "noteheadWhole",
        Figura::Minima => "noteheadHalf",
        _ => "noteheadBlack",
    }
}

fn nome_pausa(f: Figura) -> &'static str {
    match f {
        Figura::Breve => "restDoubleWhole",
        Figura::Semibreve => "restWhole",
        Figura::Minima => "restHalf",
        Figura::Seminima => "restQuarter",
        Figura::Colcheia => "rest8th",
        Figura::Semicolcheia => "rest16th",
        Figura::Fusa => "rest32nd",
        Figura::Semifusa => "rest64th",
        Figura::Quasifusa => "rest128th",
    }
}

fn nome_bandeirola(n: u8, cima: bool) -> &'static str {
    match (n, cima) {
        (1, true) => "flag8thUp",
        (1, false) => "flag8thDown",
        (2, true) => "flag16thUp",
        (2, false) => "flag16thDown",
        (3, true) => "flag32ndUp",
        (3, false) => "flag32ndDown",
        (4, true) => "flag64thUp",
        (4, false) => "flag64thDown",
        (_, true) => "flag128thUp",
        (_, false) => "flag128thDown",
    }
}

fn nome_acidente(a: Acidente) -> &'static str {
    match a {
        Acidente::Sustenido => "accidentalSharp",
        Acidente::Bemol => "accidentalFlat",
        Acidente::Bequadro => "accidentalNatural",
        Acidente::DobradoSustenido => "accidentalDoubleSharp",
        Acidente::DobradoBemol => "accidentalDoubleFlat",
    }
}

fn nome_clave(c: Clave) -> &'static str {
    match (c.sinal, c.oitava.signum()) {
        (SinalClave::Fa, -1) => "fClef8vb",
        (SinalClave::Fa, 1) => "fClef8va",
        (SinalClave::Fa, _) => "fClef",
        (SinalClave::Do, _) => "cClef",
        (_, -1) => "gClef8vb",
        (_, 1) => "gClef8va",
        _ => "gClef",
    }
}

/// Linhas de pauta (y do topo de cada pauta) recuperadas dos itens do layout atual.
fn ys_do_sistema(s: &Sistema, n_pautas: usize) -> Vec<f32> {
    let mut ys: Vec<f32> = s
        .itens
        .iter()
        .filter_map(|it| match &it.forma {
            Forma::Linha { a, b, espessura }
                if (*espessura - LINHA_PAUTA).abs() < 1e-6
                    && (a.y - b.y).abs() < 1e-6
                    && (a.x - s.x0).abs() < 1e-4
                    && (b.x - s.x1).abs() < 1e-4 =>
            {
                Some(a.y)
            }
            _ => None,
        })
        .collect();
    ys.sort_by(f32::total_cmp);
    ys.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    let mut topos: Vec<f32> = ys.chunks(5).map(|c| c[0]).collect();
    // Defesa: o layout sempre emite 5 linhas por pauta; se não bater, empilha a partir do topo.
    while topos.len() < n_pautas {
        let ultimo = topos.last().copied().unwrap_or(s.y_topo);
        topos.push(ultimo + 10.0);
    }
    topos.truncate(n_pautas);
    topos
}

fn coluna_em(a: &[(Fracao, f32)], pos: Fracao) -> Option<f32> {
    a.iter().find(|(p, _)| *p == pos).map(|(_, x)| *x)
}

struct Haste {
    x: f32,
    base: f32,
    ponta: f32,
}

/// Monta a cena do protótipo para `p`, sobre o `layout` já calculado pelo layout próprio.
pub fn montar_cena(p: &Partitura, layout: &Layout, extras: &Extras, met: &Metricas) -> Cena {
    let n_comp = p.compassos.len();
    let pautas: Vec<PautaGlobal> = p
        .partes
        .iter()
        .enumerate()
        .flat_map(|(i, pt)| (1..=pt.pautas.max(1)).map(move |k| PautaGlobal { parte: i, pauta: k }))
        .collect();
    let n_pautas = pautas.len();
    let claves: Vec<EstadoClaves> = pautas
        .iter()
        .map(|g| claves_da_pauta(p, g.parte, g.pauta))
        .collect();
    let mut cena = Cena {
        largura: layout.largura,
        altura: layout.altura,
        compassos: vec![
            MedidaCena {
                sistema: 0,
                x0: 0.0,
                x1: 0.0
            };
            n_comp
        ],
        ..Cena::default()
    };
    let mut proximo_id: u64 = 1;
    for (is, sis) in layout.sistemas.iter().enumerate() {
        let mut c = Cons {
            met,
            elems: Vec::new(),
            compasso: None,
        };
        let ys = ys_do_sistema(sis, n_pautas);
        let y_base = ys.last().map_or(sis.y_base, |y| y + ALTURA_PAUTA);
        let x_inicio = sis.x0;
        // Linhas de pauta.
        for k in 0..n_pautas {
            for l in 0..5 {
                let y = ys[k] + l as f32;
                c.linha(Tipo::Pauta, ponto(sis.x0, y), ponto(sis.x1, y), met.pauta);
            }
        }
        // Barra inicial e chaves (uma por parte com mais de uma pauta).
        c.linha(
            Tipo::Barra,
            ponto(x_inicio, ys[0]),
            ponto(x_inicio, y_base),
            met.barra_fina,
        );
        let mut grupos: Vec<(usize, usize)> = Vec::new();
        let mut k = 0;
        while k < n_pautas {
            let mut fim = k;
            while fim + 1 < n_pautas && pautas[fim + 1].parte == pautas[k].parte {
                fim += 1;
            }
            grupos.push((k, fim));
            if fim > k {
                let (y0, y1) = (ys[k], ys[fim] + ALTURA_PAUTA);
                let m = met.medida("brace");
                let altura_glifo = m.ne.y - m.sw.y;
                let escala = if altura_glifo > 0.0 {
                    (y1 - y0) / altura_glifo
                } else {
                    1.0
                };
                let x = x_inicio - 0.2 - m.ne.x * escala;
                let y = y1 + m.sw.y * escala;
                c.glifo(Tipo::Chave, "brace", x, y, escala);
            }
            k = fim + 1;
        }

        let mut marcas: Vec<Vec<MarcaArco>> = vec![Vec::new(); n_pautas];
        let mut medidas: Vec<(usize, f32, f32)> = Vec::new();
        let primeiro_sistema = is == 0;
        for m in sis.primeiro..sis.fim {
            c.compasso = Some(m);
            let inf = &p.compassos[m];
            let lay = &layout.compassos[m];
            let x = lay.x0;
            let eh_primeiro = m == sis.primeiro;
            let maior_clave = (0..n_pautas)
                .map(|i| largura_clave(claves[i].inicio[m]))
                .fold(0.0, f32::max);
            // Prefixo (mesmas larguras do layout atual: o espaçamento é o do layout próprio).
            let x_conteudo = if eh_primeiro {
                for k in 0..n_pautas {
                    let cl = claves[k].inicio[m];
                    let y = ys[k] + 5.0 - f32::from(cl.linha.clamp(1, 5));
                    c.glifo(Tipo::Clave, nome_clave(cl), x + 0.4, y, 1.0);
                }
                let xa = x + maior_clave + 0.8;
                for k in 0..n_pautas {
                    armadura_cena(&mut c, inf.armadura, claves[k].inicio[m], xa, ys[k]);
                }
                let mut w = maior_clave + 1.2 + largura_armadura(inf.armadura);
                if primeiro_sistema || inf.forma_mudou {
                    let xt = xa + largura_armadura(inf.armadura).max(0.0);
                    for k in 0..n_pautas {
                        forma_compasso(&mut c, inf, xt, ys[k]);
                    }
                    w += LARGURA_FORMA;
                }
                x + w
            } else {
                let mut xq = x;
                if claves.iter().any(|cl| cl.troca_no_inicio[m]) {
                    for k in 0..n_pautas {
                        if claves[k].troca_no_inicio[m] {
                            let cl = claves[k].inicio[m];
                            let y = ys[k] + 5.0 - f32::from(cl.linha.clamp(1, 5));
                            c.glifo(Tipo::Clave, nome_clave(cl), xq + 0.3, y, 0.8);
                        }
                    }
                    xq += 2.6;
                }
                if inf.armadura_mudou {
                    for k in 0..n_pautas {
                        armadura_cena(&mut c, inf.armadura, claves[k].inicio[m], xq + 0.2, ys[k]);
                    }
                    xq += largura_armadura(inf.armadura) + 0.5;
                }
                if inf.forma_mudou {
                    for k in 0..n_pautas {
                        forma_compasso(&mut c, inf, xq, ys[k]);
                    }
                    xq += LARGURA_FORMA;
                }
                xq
            };
            let x_fim = lay.x1;
            let x_centro = (x_conteudo + x_fim) / 2.0;
            let colunas: Vec<(Fracao, f32)> = {
                // Colunas do layout atual (tempo em f64) de volta a fração exata do modelo.
                let mut v = Vec::new();
                for (t, xc) in &lay.colunas {
                    for pauta in &pautas {
                        for n in &p.partes[pauta.parte].compassos[m].notas {
                            if (n.posicao.como_f64() - *t).abs() < 1e-9 {
                                v.push((n.posicao, *xc));
                            }
                        }
                    }
                }
                v.sort_by_key(|(f, _)| *f);
                v.dedup_by_key(|(f, _)| *f);
                v
            };
            let x_de = |pos: Fracao| -> f32 {
                coluna_em(&colunas, pos)
                    .or_else(|| layout.x_no_compasso(m, pos.como_f64()))
                    .unwrap_or(x_conteudo)
            };

            for k in 0..n_pautas {
                let analise = analisar_pauta(
                    &p.partes[pautas[k].parte].compassos[m],
                    pautas[k].pauta,
                    claves[k].inicio[m],
                    &claves[k].internas[m],
                );
                emitir_pauta(
                    &mut c,
                    &analise,
                    m,
                    k as u8,
                    ys[k],
                    &x_de,
                    x_centro,
                    &mut marcas[k],
                    &mut proximo_id,
                );
                // Trocas de clave no meio do compasso.
                for (pos, cl) in &analise.trocas {
                    let xc = x_de(*pos) - 1.6;
                    let y = ys[k] + 5.0 - f32::from(cl.linha.clamp(1, 5));
                    c.glifo(Tipo::Clave, nome_clave(*cl), xc, y, 0.8);
                }
                // Letra, cifra e dedilhado (lidos do XML).
                texto_extra(
                    &mut c,
                    extras,
                    &pautas[k],
                    grupos.iter().any(|(a, _)| *a == k),
                    m,
                    ys[k],
                    &analise,
                    claves[k].inicio[m],
                    &x_de,
                );
            }
            // Barras de compasso e repetições, por grupo de pautas (parte).
            for (a, b) in &grupos {
                let (y0, y1) = (ys[*a], ys[*b] + ALTURA_PAUTA);
                barra_final(&mut c, x_fim, y0, y1, inf.barra_final, inf.repeticao_fim);
                if inf.repeticao_fim {
                    pontos_repeticao(&mut c, x_fim - 1.9, ys[*a]);
                    if b > a {
                        pontos_repeticao(&mut c, x_fim - 1.9, ys[*b]);
                    }
                }
                if inf.repeticao_inicio {
                    let xc = x_conteudo;
                    let grossa = met.barra_grossa;
                    c.poligono(
                        Tipo::Repeticao,
                        vec![
                            ponto(xc + 0.2, y0),
                            ponto(xc + 0.2 + grossa, y0),
                            ponto(xc + 0.2 + grossa, y1),
                            ponto(xc + 0.2, y1),
                        ],
                    );
                    let xf = xc + 0.2 + grossa + met.barra_sep + met.barra_fina / 2.0;
                    c.linha(
                        Tipo::Repeticao,
                        ponto(xf, y0),
                        ponto(xf, y1),
                        met.barra_fina,
                    );
                    pontos_repeticao(&mut c, xf + 0.9, ys[*a]);
                    if b > a {
                        pontos_repeticao(&mut c, xf + 0.9, ys[*b]);
                    }
                }
            }
            medidas.push((m, x_conteudo, x_fim));
            cena.compassos[m] = MedidaCena {
                sistema: is,
                x0: x,
                x1: x_fim,
            };
        }
        c.compasso = None;
        // Arcos (ligaduras): mesma geração do layout atual, em glifos próprios apenas nas pontas.
        for k in 0..n_pautas {
            let gerados = gerar_arcos(&mut marcas[k], x_inicio + 3.5, sis.x1 - 0.5);
            for mut it in gerados {
                it.forma.deslocar(0.0, ys[k]);
                match it.forma {
                    Forma::Caminho { pontos, espessura } => c.push(
                        Tipo::Arco,
                        Desenho::Caminho {
                            pontos,
                            esp: espessura,
                        },
                        None,
                    ),
                    Forma::Preenchido { pontos } => c.poligono(Tipo::Arco, pontos),
                    _ => {}
                }
            }
        }
        // Número do compasso e casas de repetição (iguais ao layout atual).
        if let Some(&(m0, x_primeiro, _)) = medidas.first()
            && p.compassos[m0].numero != "0"
        {
            c.compasso = Some(m0);
            c.texto(
                Tipo::NumeroCompasso,
                ponto(x_primeiro - 0.2, ys[0] - 1.1),
                &p.compassos[m0].numero,
                1.25,
                Ancora::LEFT_BOTTOM,
            );
            c.compasso = None;
        }
        let mut i = 0;
        while i < medidas.len() {
            let (m, xc, _) = medidas[i];
            let casas = &p.compassos[m].casas;
            if casas.is_empty() {
                i += 1;
                continue;
            }
            let mut f = i;
            while f + 1 < medidas.len() && p.compassos[medidas[f + 1].0].casas == *casas {
                f += 1;
            }
            let (xa, xb) = (xc + 0.2, medidas[f].2 - 0.2);
            let yv = ys[0] - 2.4;
            c.linha(Tipo::Volta, ponto(xa, yv), ponto(xb, yv), 0.12);
            c.linha(Tipo::Volta, ponto(xa, yv), ponto(xa, yv + 1.2), 0.12);
            let nome: Vec<String> = casas.iter().map(|n| format!("{n}.")).collect();
            c.texto(
                Tipo::Volta,
                ponto(xa + 0.5, yv + 0.2),
                &nome.join(" "),
                1.25,
                Ancora::LEFT_TOP,
            );
            i = f + 1;
        }
        let ini = cena.elems.len();
        cena.elems.append(&mut c.elems);
        cena.sistemas.push(SistemaCena {
            y_topo: ys[0],
            y_base,
            x0: sis.x0,
            x1: sis.x1,
            primeiro: sis.primeiro,
            fim: sis.fim,
            elems: ini..cena.elems.len(),
        });
    }
    cena.notas = proximo_id - 1;
    cena
}

fn armadura_cena(c: &mut Cons<'_>, a: Armadura, clave: Clave, x: f32, y0: f32) {
    let nome = if a.quintas > 0 {
        "accidentalSharp"
    } else {
        "accidentalFlat"
    };
    for (i, s) in posicoes_armadura(a.quintas, clave).iter().enumerate() {
        c.glifo(
            Tipo::Armadura,
            nome,
            x + i as f32 * 1.1,
            y0 + y_de_s(*s),
            1.0,
        );
    }
}

fn forma_compasso(c: &mut Cons<'_>, inf: &InfoCompasso, x: f32, y0: f32) {
    let f = inf.forma;
    let centro = x + 1.3;
    match f.simbolo {
        Some(SimboloForma::Comum) => {
            c.glifo_centrado(Tipo::Forma, "timeSigCommon", centro, y0 + 2.0, 1.0)
        }
        Some(SimboloForma::Cortado) => {
            c.glifo_centrado(Tipo::Forma, "timeSigCutCommon", centro, y0 + 2.0, 1.0)
        }
        None => {
            c.numero(Tipo::Forma, &NOMES_HORA, f.batidas, centro, y0 + 1.0, 1.0);
            c.numero(Tipo::Forma, &NOMES_HORA, f.unidade, centro, y0 + 3.0, 1.0);
        }
    }
}

fn barra_final(c: &mut Cons<'_>, x: f32, y0: f32, y1: f32, b: Barra, rep_fim: bool) {
    let (fina, grossa, sep) = (c.met.barra_fina, c.met.barra_grossa, c.met.barra_sep);
    let grossa_final = |c: &mut Cons<'_>| {
        c.poligono(
            Tipo::Barra,
            vec![
                ponto(x - grossa, y0),
                ponto(x, y0),
                ponto(x, y1),
                ponto(x - grossa, y1),
            ],
        );
    };
    if rep_fim || b == Barra::Final {
        let xf = x - grossa - sep - fina / 2.0;
        c.linha(Tipo::Barra, ponto(xf, y0), ponto(xf, y1), fina);
        grossa_final(c);
        return;
    }
    match b {
        Barra::Simples => c.linha(Tipo::Barra, ponto(x, y0), ponto(x, y1), fina),
        Barra::Dupla => {
            c.linha(Tipo::Barra, ponto(x - 0.45, y0), ponto(x - 0.45, y1), fina);
            c.linha(Tipo::Barra, ponto(x, y0), ponto(x, y1), fina);
        }
        _ => {}
    }
}

fn pontos_repeticao(c: &mut Cons<'_>, x: f32, y_topo: f32) {
    for dy in [1.5, 2.5] {
        c.glifo_centrado(Tipo::Repeticao, "repeatDot", x, y_topo + dy, 1.0);
    }
}

#[allow(clippy::too_many_arguments)]
fn texto_extra(
    c: &mut Cons<'_>,
    extras: &Extras,
    pauta: &PautaGlobal,
    primeira_da_parte: bool,
    m: usize,
    y0: f32,
    analise: &AnalisePauta,
    clave: Clave,
    x_de: &dyn Fn(Fracao) -> f32,
) {
    for l in extras
        .letras
        .iter()
        .filter(|l| l.parte == pauta.parte && l.compasso == m && l.pauta == pauta.pauta)
    {
        c.texto(
            Tipo::Letra,
            ponto(
                x_de(l.pos),
                y0 + ALTURA_PAUTA + 2.8 + f32::from(l.verso.max(1) - 1) * 1.6,
            ),
            &l.texto,
            1.3,
            Ancora::CENTER_CENTER,
        );
    }
    if primeira_da_parte {
        for h in extras
            .cifras
            .iter()
            .filter(|h| h.parte == pauta.parte && h.compasso == m)
        {
            c.texto(
                Tipo::Cifra,
                ponto(x_de(h.pos), y0 - 2.2),
                &h.texto,
                1.5,
                Ancora::CENTER_CENTER,
            );
        }
    }
    for d in extras
        .dedos
        .iter()
        .filter(|d| d.parte == pauta.parte && d.compasso == m && d.pauta == pauta.pauta)
    {
        let s = s_de(d.altura, clave);
        let ev = analise
            .eventos
            .iter()
            .find(|e| e.pos == d.pos && e.notas.iter().any(|n| n.s == s));
        let (cima, y) = ev.map_or((true, y_de_s(s)), |e| (e.cima, y_de_s(s)));
        // Do lado oposto da haste.
        let dy = if cima { 1.6 } else { -1.6 };
        let cx = x_de(d.pos);
        let digito = d
            .texto
            .chars()
            .next()
            .and_then(|ch| ch.to_digit(10))
            .filter(|_| d.texto.chars().count() == 1)
            .and_then(|dg| NOMES_DEDO.get(dg as usize).copied());
        match digito {
            Some(nome) => c.glifo_centrado(Tipo::Dedilhado, nome, cx, y0 + y + dy, 1.0),
            None => c.texto(
                Tipo::Dedilhado,
                ponto(cx, y0 + y + dy),
                &d.texto,
                1.2,
                Ancora::CENTER_CENTER,
            ),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emitir_pauta(
    c: &mut Cons<'_>,
    a: &AnalisePauta,
    m: usize,
    pauta_idx: u8,
    y0: f32,
    x_de: &dyn Fn(Fracao) -> f32,
    x_centro: f32,
    marcas: &mut Vec<MarcaArco>,
    proximo_id: &mut u64,
) {
    let met = c.met;
    let mut pontas: Vec<Option<(f32, f32)>> = vec![None; a.eventos.len()];
    // Larguras e âncoras da cabeça de cada evento.
    let escala_de = |ev: &Evento| if ev.adorno { 0.7 } else { 1.0 };
    let x_ev = |ev: &Evento| x_de(ev.pos) - if ev.adorno { adorno_recuo(a, ev) } else { 0.0 };
    // Hastes.
    let mut hastes: Vec<Option<Haste>> = Vec::new();
    for ev in &a.eventos {
        if ev.pausa || ev.notas.is_empty() || !ev.figura.tem_haste() {
            hastes.push(None);
            continue;
        }
        let escala = escala_de(ev);
        let cab = nome_cabeca(ev.figura);
        let w = met.avanco(cab) * escala;
        let x = x_ev(ev);
        let up = met.ancora(cab, "stemUpSE").unwrap_or(ponto(w, 0.168));
        let dn = met.ancora(cab, "stemDownNW").unwrap_or(ponto(0.0, -0.168));
        let (base_s, ponta_s) = (ev.notas[0].s, ev.notas[ev.notas.len() - 1].s);
        let comp = HASTE_COMPRIMENTO * escala.max(0.75);
        let h = if ev.cima {
            Haste {
                x: x - w / 2.0 + up.x * escala - met.haste / 2.0,
                base: y_de_s(base_s) - up.y * escala,
                ponta: (y_de_s(ponta_s) - comp).min(ALTURA_PAUTA / 2.0),
            }
        } else {
            Haste {
                x: x - w / 2.0 + dn.x * escala + met.haste / 2.0,
                base: y_de_s(ponta_s) - dn.y * escala,
                ponta: (y_de_s(base_s) + comp).max(ALTURA_PAUTA / 2.0),
            }
        };
        hastes.push(Some(h));
    }
    // Cabeças, pausas, acidentes, pontos, suplementares e marcas de ligadura.
    for ev in &a.eventos {
        let escala = escala_de(ev);
        let x = x_ev(ev);
        if ev.pausa {
            let dy = if ev.n_vozes >= 2 {
                if ev.rank == 0 { -1.0 } else { 1.0 }
            } else {
                0.0
            };
            let (xr, figura) = if ev.pausa_compasso {
                (x_centro, Figura::Semibreve)
            } else {
                (x, ev.figura)
            };
            let nome = nome_pausa(figura);
            let linha_base = match figura {
                Figura::Semibreve | Figura::Breve => 1.0,
                _ => 2.0,
            };
            let mm = met.medida(nome);
            let xg = xr - (mm.sw.x + mm.ne.x) / 2.0;
            c.glifo(Tipo::Pausa, nome, xg, y0 + linha_base + dy, 1.0);
            for k in 0..ev.pontos {
                c.glifo_centrado(
                    Tipo::Ponto,
                    "augmentationDot",
                    xg + mm.ne.x + 0.5 + 0.5 * f32::from(k),
                    y0 + 2.0 + dy - 0.5,
                    1.0,
                );
            }
            continue;
        }
        let cab = nome_cabeca(ev.figura);
        let w = met.avanco(cab) * escala;
        let hw = w / 2.0;
        for n in &ev.notas {
            let id = IdNota(*proximo_id);
            *proximo_id += 1;
            let xc = x + f32::from(n.desloc) * w * 0.98;
            let yc = y0 + y_de_s(n.s);
            c.glifo_nota(Tipo::Cabeca, cab, xc - hw, yc, escala, Some(id));
            // Linhas suplementares.
            let passo = if n.s <= -2 {
                -2
            } else if n.s >= 10 {
                2
            } else {
                0
            };
            if escala >= 0.99 && passo != 0 {
                let mut sl = if passo < 0 { -2 } else { 10 };
                while (passo < 0 && sl >= n.s) || (passo > 0 && sl <= n.s) {
                    let y = y0 + y_de_s(sl);
                    c.linha(
                        Tipo::Suplementar,
                        ponto(xc - hw - met.suplementar_ext, y),
                        ponto(xc + hw + met.suplementar_ext, y),
                        met.suplementar,
                    );
                    sl += passo;
                }
            }
            // Pontos de aumento.
            for k in 0..ev.pontos {
                let dy = if n.s % 2 == 0 { -0.5 } else { 0.0 };
                let xd = xc + hw + 0.5 + 0.5 * f32::from(k);
                c.glifo_centrado(Tipo::Ponto, "augmentationDot", xd, yc + dy, escala);
            }
            // Acidente.
            if let Some(ac) = n.acc {
                let nome = nome_acidente(ac);
                let larg = met.avanco(nome);
                let borda = x
                    - hw
                    - 0.25
                    - if ev.notas.iter().any(|o| o.desloc < 0) {
                        1.25
                    } else {
                        0.0
                    }
                    - f32::from(n.acc_col) * 1.2;
                c.glifo_nota(Tipo::Acidente, nome, borda - larg, yc, 1.0, Some(id));
            }
            // Marcas de ligaduras (mesma regra do layout atual).
            let acima = if ev.n_vozes >= 2 { ev.cima } else { !ev.cima };
            let lado = if acima { -0.65 } else { 0.65 };
            let ordem = (m, ev.pos);
            let ycr = y_de_s(n.s);
            if n.liga_prox {
                marcas.push(MarcaArco {
                    tipo: 0,
                    chave: (pauta_idx, n.midi.unwrap_or(0)),
                    inicio: true,
                    x: xc + hw + 0.2,
                    y: ycr + lado,
                    acima,
                    ordem,
                });
            }
            for mk in &n.ligaduras {
                marcas.push(MarcaArco {
                    tipo: 1,
                    chave: (pauta_idx, mk.numero),
                    inicio: mk.inicio,
                    x: xc + if mk.inicio { 0.2 } else { -0.2 },
                    y: ycr + lado * 1.4,
                    acima,
                    ordem,
                });
            }
            if n.liga_ant {
                marcas.push(MarcaArco {
                    tipo: 0,
                    chave: (pauta_idx, n.midi.unwrap_or(0)),
                    inicio: false,
                    x: xc - hw - 0.2,
                    y: ycr + lado,
                    acima,
                    ordem,
                });
            }
        }
    }
    // Vigas (mesmo algoritmo do layout atual) e hastes.
    let mut feitos: HashSet<usize> = HashSet::new();
    for g in &a.grupos {
        let hs: Vec<&Haste> = g
            .eventos
            .iter()
            .filter_map(|i| hastes[*i].as_ref())
            .collect();
        if hs.len() != g.eventos.len() {
            continue;
        }
        let (x1, x2) = (hs[0].x, hs[hs.len() - 1].x);
        let t_de = |x: f32| if x2 > x1 { (x - x1) / (x2 - x1) } else { 0.0 };
        let sinal = if g.cima { 1.0 } else { -1.0 };
        let dy_bruto = hs[hs.len() - 1].ponta - hs[0].ponta;
        let mut dy = dy_bruto.clamp(-VIGA_DESNIVEL_MAX, VIGA_DESNIVEL_MAX);
        if hs.len() > 2 {
            let interior_extremo = hs[1..hs.len() - 1].iter().any(|h| {
                if g.cima {
                    h.ponta < hs[0].ponta.min(hs[hs.len() - 1].ponta)
                } else {
                    h.ponta > hs[0].ponta.max(hs[hs.len() - 1].ponta)
                }
            });
            if interior_extremo {
                dy = 0.0;
            }
        }
        let comp_min = HASTE_COMPRIMENTO
            + 0.75
                * (a.eventos[g.eventos[0]]
                    .figura
                    .bandeirolas()
                    .saturating_sub(2)) as f32;
        let extra = (comp_min - HASTE_COMPRIMENTO).max(0.0);
        let mut y1 = if g.cima { f32::MAX } else { f32::MIN };
        for h in &hs {
            let v = h.ponta - sinal * extra - dy * t_de(h.x);
            y1 = if g.cima { y1.min(v) } else { y1.max(v) };
        }
        let y_viga = |x: f32| y1 + dy * t_de(x);
        for (k, i) in g.eventos.iter().enumerate() {
            let h = hs[k];
            let ponta = y_viga(h.x);
            pontas[*i] = Some((h.x, ponta));
            c.linha(
                Tipo::Haste,
                ponto(h.x, y0 + h.base),
                ponto(h.x, y0 + ponta),
                met.haste,
            );
            feitos.insert(*i);
        }
        let max_nivel = g
            .eventos
            .iter()
            .map(|i| a.eventos[*i].vigas.len())
            .max()
            .unwrap_or(1);
        for nivel in 0..max_nivel {
            let deslocamento = nivel as f32 * (met.viga + met.viga_espaco);
            let mut inicio: Option<usize> = None;
            let mut segmentos: Vec<(f32, f32)> = Vec::new();
            for (k, i) in g.eventos.iter().enumerate() {
                match a.eventos[*i].vigas.get(nivel) {
                    Some(EstadoViga::Inicio) => inicio = Some(k),
                    Some(EstadoViga::Continua) => {}
                    Some(EstadoViga::Fim) => {
                        if let Some(s) = inicio.take() {
                            segmentos.push((hs[s].x, hs[k].x));
                        }
                    }
                    Some(EstadoViga::GanchoFrente) => {
                        segmentos.push((hs[k].x, (hs[k].x + 1.1).min(x2.max(hs[k].x + 0.6))))
                    }
                    Some(EstadoViga::GanchoAtras) => {
                        segmentos.push(((hs[k].x - 1.1).max(x1.min(hs[k].x - 0.6)), hs[k].x))
                    }
                    None => inicio = None,
                }
            }
            for (xa, xb) in segmentos {
                let (xa, xb) = (xa - met.haste / 2.0, xb + met.haste / 2.0);
                let (ya, yb) = if g.cima {
                    (y_viga(xa) + deslocamento, y_viga(xb) + deslocamento)
                } else {
                    (y_viga(xa) - deslocamento, y_viga(xb) - deslocamento)
                };
                let esp = if g.cima { met.viga } else { -met.viga };
                c.poligono(
                    Tipo::Viga,
                    vec![
                        ponto(xa, y0 + ya),
                        ponto(xb, y0 + yb),
                        ponto(xb, y0 + yb + esp),
                        ponto(xa, y0 + ya + esp),
                    ],
                );
            }
        }
    }
    for (i, ev) in a.eventos.iter().enumerate() {
        if feitos.contains(&i) {
            continue;
        }
        let Some(h) = &hastes[i] else { continue };
        pontas[i] = Some((h.x, h.ponta));
        c.linha(
            Tipo::Haste,
            ponto(h.x, y0 + h.base),
            ponto(h.x, y0 + h.ponta),
            met.haste,
        );
        let n = ev.figura.bandeirolas();
        if n > 0 {
            c.glifo(
                Tipo::Bandeirola,
                nome_bandeirola(n, ev.cima),
                h.x - met.haste / 2.0,
                y0 + h.ponta,
                escala_de(ev),
            );
        }
    }
    // Tercinas: colchete e número em glifos.
    let mut i = 0;
    while i < a.eventos.len() {
        if a.eventos[i].inicio_tuplo {
            let voz = a.eventos[i].voz;
            let mut j = i;
            while j < a.eventos.len() && !(a.eventos[j].fim_tuplo && a.eventos[j].voz == voz) {
                j += 1;
            }
            if j >= a.eventos.len() {
                break;
            }
            let membros: Vec<usize> = (i..=j).filter(|k| a.eventos[*k].voz == voz).collect();
            let numero = a.eventos[i].tuplo.unwrap_or(3);
            let cima = a.eventos[i].cima;
            let xs: Vec<f32> = membros.iter().map(|k| x_de(a.eventos[*k].pos)).collect();
            let (xa, xb) = (
                xs.iter().copied().fold(f32::MAX, f32::min),
                xs.iter().copied().fold(f32::MIN, f32::max),
            );
            let mut ys: Vec<f32> = membros
                .iter()
                .filter_map(|k| pontas[*k].map(|(_, y)| y))
                .collect();
            if ys.is_empty() {
                ys.push(if cima { 0.0 } else { ALTURA_PAUTA });
            }
            let extremo = if cima {
                ys.iter().copied().fold(f32::MAX, f32::min)
            } else {
                ys.iter().copied().fold(f32::MIN, f32::max)
            };
            let tudo_vigado = membros.iter().all(|k| a.eventos[*k].grupo.is_some());
            let y = y0 + extremo + if cima { -1.0 } else { 1.0 };
            let xm = (xa + xb) / 2.0;
            if !tudo_vigado && xb > xa + 1.0 {
                let tick = if cima { 0.5 } else { -0.5 };
                c.linha(Tipo::Tercina, ponto(xa, y + tick), ponto(xa, y), 0.1);
                c.linha(Tipo::Tercina, ponto(xa, y), ponto(xm - 0.9, y), 0.1);
                c.linha(Tipo::Tercina, ponto(xm + 0.9, y), ponto(xb, y), 0.1);
                c.linha(Tipo::Tercina, ponto(xb, y), ponto(xb, y + tick), 0.1);
            }
            c.numero(Tipo::Tercina, &NOMES_TUPLA, numero, xm, y, 1.0);
            i = j + 1;
        } else {
            i += 1;
        }
    }
}

// --- Pintura no egui -------------------------------------------------------------------------

/// Registra a Bravura como família `Bravura` (além das fontes padrão do egui).
pub fn fontes_com_bravura() -> bancada_casca::egui::FontDefinitions {
    use bancada_casca::egui::{FontData, FontDefinitions};
    let mut defs = FontDefinitions::default();
    defs.font_data.insert(
        FAMILIA.to_owned(),
        std::sync::Arc::new(FontData::from_static(BRAVURA_OTF)),
    );
    defs.families
        .insert(FontFamily::Name(FAMILIA.into()), vec![FAMILIA.to_owned()]);
    defs
}

fn alinha_egui(a: Alinha) -> Align {
    match a {
        Alinha::Min => Align::Min,
        Alinha::Centro => Align::Center,
        Alinha::Max => Align::Max,
    }
}

/// Pinta a cena. `origem` é o canto superior esquerdo; `esp` os pixels por sp. Com `esp` não
/// positivo ou não finito, não pinta nada (largura zero e tamanhos mínimos nunca derrubam).
pub fn pintar_cena(
    painter: &Painter,
    origem: Pos2,
    esp: f32,
    cena: &Cena,
    met: &Metricas,
    pal: &Paleta,
) {
    if !(esp.is_finite() && esp > 0.0) || !origem.x.is_finite() || !origem.y.is_finite() {
        return;
    }
    let px = |p: Ponto| pos2(origem.x + p.x * esp, origem.y + p.y * esp);
    let corte = painter.clip_rect().expand(MARGEM_CORTE_SP * esp);
    for e in &cena.elems {
        let caixa = e.caixa(met);
        if !(caixa.min.is_finite() && caixa.max.is_finite()) {
            continue;
        }
        if !corte.intersects(Rect::from_min_max(px(caixa.min), px(caixa.max))) {
            continue;
        }
        let cor = match e.cor {
            Cor::Tinta => pal.tinta,
            Cor::Suave => pal.suave,
        };
        match &e.desenho {
            Desenho::Glifo { nome, x, y, escala } => {
                let corpo = 4.0 * esp * escala;
                if !(corpo.is_finite() && corpo > 0.05) {
                    continue;
                }
                let galley = painter.layout_no_wrap(
                    codigo(nome).to_string(),
                    FontId::new(corpo, FontFamily::Name(FAMILIA.into())),
                    cor,
                );
                // A linha de base do glifo no galley: posição da linha + `pos` do glifo.
                let (bx, by) = galley.rows.first().map_or((0.0, 0.0), |r| {
                    let g = r.glyphs.first();
                    (
                        r.pos.x + g.map_or(0.0, |g| g.pos.x),
                        r.pos.y + g.map_or(0.0, |g| g.pos.y),
                    )
                });
                let alvo = px(ponto(*x, *y));
                painter.galley(pos2(alvo.x - bx, alvo.y - by), galley, cor);
            }
            Desenho::Linha { a, b, esp: e } => {
                let (mut pa, mut pb) = (px(*a), px(*b));
                if (pa.y - pb.y).abs() < 1e-3 {
                    pa.y = pa.y.round();
                    pb.y = pb.y.round();
                }
                painter.add(Shape::line_segment(
                    [pa, pb],
                    Stroke::new((e * esp).max(1.0), cor),
                ));
            }
            Desenho::Poligono { pontos } => {
                painter.add(Shape::convex_polygon(
                    pontos.iter().map(|p| px(*p)).collect(),
                    cor,
                    Stroke::NONE,
                ));
            }
            Desenho::Caminho { pontos, esp: e } => {
                painter.add(Shape::line(
                    pontos.iter().map(|p| px(*p)).collect(),
                    Stroke::new((e * esp).max(0.8), cor),
                ));
            }
            Desenho::Texto {
                pos,
                texto,
                altura,
                ancora,
            } => {
                let corpo = (altura * esp / 0.72).max(11.0);
                painter.text(
                    px(*pos),
                    Align2([alinha_egui(ancora.0[0]), alinha_egui(ancora.0[1])]),
                    texto,
                    FontId::proportional(corpo),
                    cor,
                );
            }
        }
    }
}
