//! Diff semântico entre dois documentos (ticket 22, A10): compara **música**, não texto.
//!
//! Por compasso (parte + índice) lista o antes e o depois das notas (altura, duração, voz,
//! pauta), as notas alteradas, inseridas e removidas, e as marcas indiretas: letra, dedilhado
//! (por nota) e cifra (por compasso). Formatação (posição, espaços, `<print>`, ordem de
//! atributos, comentários) não entra no resumo e portanto não gera diferença.
//!
//! Pareamento de notas num compasso: primeiro pelo `id` do MusicXML (quando os dois lados têm o
//! mesmo `id` único); o resto por subsequência comum de conteúdo igual; o que sobra entre duas
//! âncoras é pareado em ordem como "alterada" e o excedente vira inserida/removida. Não depende
//! de `IdNota`, que é atribuído por leitura e não é comparável entre dois textos.
//!
//! Só leitura, sem janela, áudio, rede nem disco (seam S1): reutilizável pelo MCP sem mudar o
//! contrato. A lista de compassos alterados é simétrica: `diff(a, b)` e `diff(b, a)` apontam os
//! mesmos compassos (com antes e depois trocados).

use super::grafia::nome_da_altura;
use super::{Documento, Nota, PosCompasso};
use crate::partitura::Fracao;
use std::collections::HashMap;
use std::ops::Range;

/// Resumo musical de uma nota, o que o diff compara.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResumoDeNota {
    /// Ordinal da nota no compasso (0 = primeira).
    pub ordinal: usize,
    /// Atributo `id` da nota no MusicXML, quando existir.
    pub id_xml: Option<String>,
    /// Nome da altura (ex.: `C#4`); `None` em pausa.
    pub altura: Option<String>,
    /// Em semínimas; `None` em nota de graça.
    pub duracao: Option<Fracao>,
    pub voz: String,
    pub pauta: String,
    pub acorde: bool,
    pub pausa: bool,
    pub graca: bool,
    /// Sílabas da letra (um item por `<lyric>`, no formato `número:texto`).
    pub letra: Vec<String>,
    /// Dedilhados (um item por `<fingering>`).
    pub dedilhado: Vec<String>,
}

impl ResumoDeNota {
    /// Igualdade de conteúdo (ignora o ordinal).
    fn mesmo_conteudo(&self, o: &Self) -> bool {
        self.id_xml == o.id_xml && self.campos_alterados(o).is_empty()
    }

    fn campos_alterados(&self, o: &Self) -> Vec<CampoAlterado> {
        let mut v = Vec::new();
        if self.altura != o.altura {
            v.push(CampoAlterado::Altura);
        }
        if self.duracao != o.duracao {
            v.push(CampoAlterado::Duracao);
        }
        if self.voz != o.voz {
            v.push(CampoAlterado::Voz);
        }
        if self.pauta != o.pauta {
            v.push(CampoAlterado::Pauta);
        }
        if self.acorde != o.acorde || self.pausa != o.pausa || self.graca != o.graca {
            v.push(CampoAlterado::Tipo);
        }
        if self.letra != o.letra {
            v.push(CampoAlterado::Letra);
        }
        if self.dedilhado != o.dedilhado {
            v.push(CampoAlterado::Dedilhado);
        }
        v
    }
}

/// O que mudou numa nota alterada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampoAlterado {
    Altura,
    Duracao,
    Voz,
    Pauta,
    /// Pausa, acorde ou nota de graça.
    Tipo,
    /// Marcação indireta: letra.
    Letra,
    /// Marcação indireta: dedilhado.
    Dedilhado,
}

impl CampoAlterado {
    /// Letra e dedilhado são marcas (mudam por edição indireta, sem tocar a nota em si).
    pub const fn e_marcacao(self) -> bool {
        matches!(self, Self::Letra | Self::Dedilhado)
    }
}

/// Mudança de uma nota num compasso.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum MudancaDeNota {
    Alterada {
        antes: ResumoDeNota,
        depois: ResumoDeNota,
        campos: Vec<CampoAlterado>,
    },
    Inserida(ResumoDeNota),
    Removida(ResumoDeNota),
}

/// Um compasso que difere entre os dois documentos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompassoAlterado {
    /// Posição; `numero` é o rótulo do lado "depois" (ou do "antes" se o compasso sumiu).
    pub pos: PosCompasso,
    /// `false` se o compasso não existe no lado "antes" (compasso novo).
    pub existe_antes: bool,
    /// `false` se o compasso não existe no lado "depois" (compasso removido).
    pub existe_depois: bool,
    pub antes: Vec<ResumoDeNota>,
    pub depois: Vec<ResumoDeNota>,
    pub mudancas: Vec<MudancaDeNota>,
    /// Cifras do compasso antes e depois, quando diferem (marcação indireta).
    pub cifra: Option<(Vec<String>, Vec<String>)>,
}

impl CompassoAlterado {
    /// `true` se a única diferença são marcas (letra, dedilhado ou cifra), sem mudar a música.
    pub fn so_marcacao(&self) -> bool {
        self.existe_antes
            && self.existe_depois
            && self.mudancas.iter().all(|m| match m {
                MudancaDeNota::Alterada { campos, .. } => campos.iter().all(|c| c.e_marcacao()),
                _ => false,
            })
    }
}

/// Resultado: compassos alterados, na ordem das partes e dos compassos.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiffDeDocumentos {
    pub compassos: Vec<CompassoAlterado>,
}

impl DiffDeDocumentos {
    pub fn vazio(&self) -> bool {
        self.compassos.is_empty()
    }

    /// Posições dos compassos alterados.
    pub fn posicoes(&self) -> Vec<PosCompasso> {
        self.compassos.iter().map(|c| c.pos.clone()).collect()
    }
}

/// Compara dois documentos, por compasso, de forma semântica.
pub fn diff_de_documentos(antes: &Documento, depois: &Documento) -> DiffDeDocumentos {
    let mut compassos = Vec::new();
    let mut ids_partes: Vec<&str> = antes.partes().iter().map(|p| p.id.as_str()).collect();
    for p in depois.partes() {
        if !ids_partes.contains(&p.id.as_str()) {
            ids_partes.push(p.id.as_str());
        }
    }
    for id in ids_partes {
        let pa = antes.partes().iter().find(|p| p.id == id);
        let pd = depois.partes().iter().find(|p| p.id == id);
        let na = pa.map_or(0, |p| p.compassos.len());
        let nd = pd.map_or(0, |p| p.compassos.len());
        for i in 0..na.max(nd) {
            let ca = pa.and_then(|p| p.compassos.get(i));
            let cd = pd.and_then(|p| p.compassos.get(i));
            let (ra, ha) = ca.map_or((Vec::new(), Vec::new()), |c| {
                resumir(antes.original(), &c.notas, &c.faixa)
            });
            let (rd, hd) = cd.map_or((Vec::new(), Vec::new()), |c| {
                resumir(depois.original(), &c.notas, &c.faixa)
            });
            let mudancas = parear(&ra, &rd);
            let cifra_difere = ha != hd;
            if ca.is_some() == cd.is_some() && mudancas.is_empty() && !cifra_difere {
                continue;
            }
            let numero = cd.or(ca).map(|c| c.numero.clone()).unwrap_or_default();
            compassos.push(CompassoAlterado {
                pos: PosCompasso {
                    parte: id.to_owned(),
                    indice: i,
                    numero,
                },
                existe_antes: ca.is_some(),
                existe_depois: cd.is_some(),
                antes: ra,
                depois: rd,
                mudancas,
                cifra: cifra_difere.then_some((ha, hd)),
            });
        }
    }
    DiffDeDocumentos { compassos }
}

fn resumir(
    original: &str,
    notas: &[Nota],
    faixa: &Range<usize>,
) -> (Vec<ResumoDeNota>, Vec<String>) {
    let resumos = notas
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let (letra, dedilhado) = marcas_da_nota(original.get(n.faixa.clone()).unwrap_or(""));
            ResumoDeNota {
                ordinal: i,
                id_xml: n.id_xml.clone(),
                altura: n.altura.map(nome_da_altura),
                duracao: n.duracao,
                voz: n.voz.clone(),
                pauta: n.pauta.clone(),
                acorde: n.acorde,
                pausa: n.pausa,
                graca: n.graca,
                letra,
                dedilhado,
            }
        })
        .collect();
    let cifras = original
        .get(faixa.clone())
        .map(cifras_do_compasso)
        .unwrap_or_default();
    (resumos, cifras)
}

fn texto_de(no: roxmltree::Node<'_, '_>) -> String {
    no.descendants()
        .filter(|d| d.is_text())
        .filter_map(|d| d.text())
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Letra e dedilhado de uma nota, lidos do trecho original dela.
fn marcas_da_nota(trecho: &str) -> (Vec<String>, Vec<String>) {
    let Ok(arvore) = roxmltree::Document::parse(trecho) else {
        return (Vec::new(), Vec::new());
    };
    let mut letra = Vec::new();
    let mut dedilhado = Vec::new();
    for d in arvore
        .root_element()
        .descendants()
        .filter(|d| d.is_element())
    {
        match d.tag_name().name() {
            "lyric" => {
                let numero = d.attribute("number").unwrap_or("");
                let texto: String = d
                    .children()
                    .filter(|c| c.has_tag_name("text"))
                    .map(texto_de)
                    .collect();
                letra.push(format!("{numero}:{texto}"));
            }
            "fingering" => dedilhado.push(texto_de(d)),
            _ => {}
        }
    }
    (letra, dedilhado)
}

/// Cifras (`<harmony>`) filhas diretas do compasso, em forma canônica.
fn cifras_do_compasso(trecho: &str) -> Vec<String> {
    let Ok(arvore) = roxmltree::Document::parse(trecho) else {
        return Vec::new();
    };
    arvore
        .root_element()
        .children()
        .filter(|c| c.has_tag_name("harmony"))
        .map(|h| {
            let parte = |pai: &str, filho: &str| {
                h.children()
                    .find(|c| c.has_tag_name(pai))
                    .and_then(|p| p.children().find(|c| c.has_tag_name(filho)))
                    .map(texto_de)
                    .unwrap_or_default()
            };
            let tipo = h.children().find(|c| c.has_tag_name("kind"));
            format!(
                "{}{}|{}|{}|{}{}",
                parte("root", "root-step"),
                parte("root", "root-alter"),
                tipo.map(texto_de).unwrap_or_default(),
                tipo.and_then(|k| k.attribute("text")).unwrap_or(""),
                parte("bass", "bass-step"),
                parte("bass", "bass-alter"),
            )
        })
        .collect()
}

fn alterada(a: &ResumoDeNota, d: &ResumoDeNota) -> MudancaDeNota {
    MudancaDeNota::Alterada {
        antes: a.clone(),
        depois: d.clone(),
        campos: a.campos_alterados(d),
    }
}

/// Pareia as notas dos dois lados e devolve só o que mudou, na ordem do compasso.
fn parear(a: &[ResumoDeNota], d: &[ResumoDeNota]) -> Vec<MudancaDeNota> {
    let mut par_a: Vec<Option<usize>> = vec![None; a.len()];
    let mut par_d: Vec<Option<usize>> = vec![None; d.len()];

    // 1) mesmo id do MusicXML, único nos dois lados.
    let unicos = |r: &[ResumoDeNota]| {
        let mut m: HashMap<String, Option<usize>> = HashMap::new();
        for (i, n) in r.iter().enumerate() {
            if let Some(id) = &n.id_xml {
                m.entry(id.clone())
                    .and_modify(|e| *e = None)
                    .or_insert(Some(i));
            }
        }
        m
    };
    let (ua, ud) = (unicos(a), unicos(d));
    for (id, ia) in &ua {
        if let (Some(i), Some(Some(j))) = (ia, ud.get(id)) {
            par_a[*i] = Some(*j);
            par_d[*j] = Some(*i);
        }
    }

    // 2) subsequência comum de conteúdo igual entre as não pareadas.
    let la: Vec<usize> = (0..a.len()).filter(|&i| par_a[i].is_none()).collect();
    let ld: Vec<usize> = (0..d.len()).filter(|&j| par_d[j].is_none()).collect();
    let mut t = vec![vec![0usize; ld.len() + 1]; la.len() + 1];
    for i in (0..la.len()).rev() {
        for j in (0..ld.len()).rev() {
            t[i][j] = if a[la[i]].mesmo_conteudo(&d[ld[j]]) {
                t[i + 1][j + 1] + 1
            } else {
                t[i + 1][j].max(t[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < la.len() && j < ld.len() {
        if a[la[i]].mesmo_conteudo(&d[ld[j]]) {
            par_a[la[i]] = Some(ld[j]);
            par_d[ld[j]] = Some(la[i]);
            i += 1;
            j += 1;
        } else if t[i + 1][j] >= t[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }

    // 3) pares com conteúdo diferente (mesmo id) são alterados; entre âncoras, as sobras são
    //    pareadas em ordem como alteradas e o excedente é inserido/removido.
    // Chave de ordenação: (posição no "depois", desempate, ordinal no "antes").
    let mut saida: Vec<((usize, u8, usize), MudancaDeNota)> = Vec::new();
    for (i, p) in par_a.iter().enumerate() {
        if let Some(j) = *p
            && !a[i].mesmo_conteudo(&d[j])
        {
            saida.push(((j, 0, i), alterada(&a[i], &d[j])));
        }
    }
    // Segmento de uma sobra = quantas notas pareadas a precedem no próprio lado.
    let segmento =
        |par: &[Option<usize>], k: usize| par[..k].iter().filter(|p| p.is_some()).count();
    let mut sobras_a: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut sobras_d: HashMap<usize, Vec<usize>> = HashMap::new();
    for (k, p) in par_a.iter().enumerate() {
        if p.is_none() {
            sobras_a.entry(segmento(&par_a, k)).or_default().push(k);
        }
    }
    for (k, p) in par_d.iter().enumerate() {
        if p.is_none() {
            sobras_d.entry(segmento(&par_d, k)).or_default().push(k);
        }
    }
    // Posição no "depois" da pareada que fecha o segmento s (ou o fim).
    let pareadas_d: Vec<usize> = (0..d.len()).filter(|&k| par_d[k].is_some()).collect();
    let mut segmentos: Vec<usize> = sobras_a.keys().chain(sobras_d.keys()).copied().collect();
    segmentos.sort_unstable();
    segmentos.dedup();
    for s in segmentos {
        let ga = sobras_a.get(&s).cloned().unwrap_or_default();
        let gd = sobras_d.get(&s).cloned().unwrap_or_default();
        let n = ga.len().min(gd.len());
        for k in 0..n {
            saida.push(((gd[k], 0, ga[k]), alterada(&a[ga[k]], &d[gd[k]])));
        }
        for &k in &gd[n..] {
            saida.push(((k, 0, 0), MudancaDeNota::Inserida(d[k].clone())));
        }
        let fecha = pareadas_d.get(s).copied().unwrap_or(d.len());
        for &k in &ga[n..] {
            saida.push(((fecha, 1, k), MudancaDeNota::Removida(a[k].clone())));
        }
    }
    saida.sort_by_key(|(k, _)| *k);
    saida.into_iter().map(|(_, m)| m).collect()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::documento::DocumentoAberto;

    fn compasso(notas: &str) -> String {
        format!(
            "<?xml version=\"1.0\"?>\n<score-partwise version=\"4.0\">\n<part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list>\n<part id=\"P1\">\n{notas}</part>\n</score-partwise>\n"
        )
    }

    fn nota(passo: char, dur: u32, extra: &str) -> String {
        format!(
            "<note><pitch><step>{passo}</step><octave>4</octave></pitch><duration>{dur}</duration><voice>1</voice><type>quarter</type>{extra}</note>"
        )
    }

    fn medida(n: u32, notas: &[String]) -> String {
        let attrs = if n == 1 {
            "<attributes><divisions>1</divisions></attributes>"
        } else {
            ""
        };
        format!(
            "<measure number=\"{n}\">{attrs}{}</measure>\n",
            notas.concat()
        )
    }

    fn abrir(texto: &str) -> DocumentoAberto {
        DocumentoAberto::abrir_texto("t.musicxml", texto).expect("abre")
    }

    fn diff(a: &str, b: &str) -> DiffDeDocumentos {
        diff_de_documentos(abrir(a).documento(), abrir(b).documento())
    }

    fn base() -> String {
        compasso(&format!(
            "{}{}{}",
            medida(1, &[nota('C', 1, ""), nota('D', 1, ""), nota('E', 1, "")]),
            medida(2, &[nota('F', 1, ""), nota('G', 1, "")]),
            medida(3, &[nota('A', 2, "")]),
        ))
    }

    #[test]
    fn identicas_dao_diff_vazio() {
        assert!(diff(&base(), &base()).vazio());
    }

    #[test]
    fn formatacao_diferente_conteudo_igual_da_diff_vazio() {
        let formatada = base()
            .replace("<note>", "\n      <!-- c --><note>")
            .replace(
                "<voice>1</voice><type>quarter</type>",
                "<voice>1</voice> <type>quarter</type>",
            );
        assert_ne!(formatada, base());
        assert!(diff(&base(), &formatada).vazio());
        let com_posicao = base().replace("<note>", "<note default-x=\"10\">");
        assert!(diff(&base(), &com_posicao).vazio());
    }

    #[test]
    fn devolve_exatamente_os_compassos_editados_com_antes_e_depois() {
        let b = base().replace("<step>D</step>", "<step>B</step>");
        let b = b.replacen("<duration>2</duration>", "<duration>1</duration>", 1);
        let d = diff(&base(), &b);
        let idx: Vec<usize> = d.compassos.iter().map(|c| c.pos.indice).collect();
        assert_eq!(idx, vec![0, 2]);
        let c0 = &d.compassos[0];
        assert_eq!(c0.antes.len(), 3);
        assert_eq!(c0.depois.len(), 3);
        assert_eq!(c0.antes[1].altura.as_deref(), Some("Ré 4"));
        assert_eq!(c0.depois[1].altura.as_deref(), Some("Si 4"));
        match &c0.mudancas[..] {
            [MudancaDeNota::Alterada { campos, .. }] => {
                assert_eq!(campos, &[CampoAlterado::Altura]);
            }
            outro => panic!("esperava uma alterada, veio {outro:?}"),
        }
        let c2 = &d.compassos[1];
        match &c2.mudancas[..] {
            [MudancaDeNota::Alterada { campos, .. }] => {
                assert_eq!(campos, &[CampoAlterado::Duracao]);
            }
            outro => panic!("esperava duração alterada, veio {outro:?}"),
        }
        assert_eq!(c2.pos.numero, "3");
    }

    #[test]
    fn nota_inserida_e_removida_no_compasso() {
        let ins = base().replacen(
            &nota('F', 1, ""),
            &format!("{}{}", nota('F', 1, ""), nota('B', 1, "")),
            1,
        );
        let d = diff(&base(), &ins);
        assert_eq!(d.compassos.len(), 1);
        assert_eq!(d.compassos[0].pos.indice, 1);
        match &d.compassos[0].mudancas[..] {
            [MudancaDeNota::Inserida(n)] => assert_eq!(n.altura.as_deref(), Some("Si 4")),
            outro => panic!("esperava inserida, veio {outro:?}"),
        }
        // simetria: o inverso é a remoção no mesmo compasso.
        let inv = diff(&ins, &base());
        assert_eq!(inv.posicoes(), d.posicoes());
        match &inv.compassos[0].mudancas[..] {
            [MudancaDeNota::Removida(n)] => assert_eq!(n.altura.as_deref(), Some("Si 4")),
            outro => panic!("esperava removida, veio {outro:?}"),
        }
    }

    #[test]
    fn com_ids_xml_nota_alterada_nao_vira_remocao_mais_insercao() {
        let com_id = |p: char, id: &str| {
            nota(p, 1, "").replacen("<note>", &format!("<note id=\"{id}\">"), 1)
        };
        let a = compasso(&medida(1, &[com_id('C', "n1"), com_id('D', "n2")]));
        let b = compasso(&medida(1, &[com_id('C', "n1"), com_id('F', "n2")]));
        let d = diff(&a, &b);
        match &d.compassos[0].mudancas[..] {
            [MudancaDeNota::Alterada { campos, .. }] => {
                assert_eq!(campos, &[CampoAlterado::Altura]);
            }
            outro => panic!("esperava alterada, veio {outro:?}"),
        }
    }

    #[test]
    fn letra_dedilhado_e_cifra_aparecem_como_marcacao() {
        let letra = |t: &str| {
            format!("<lyric number=\"1\"><syllabic>single</syllabic><text>{t}</text></lyric>")
        };
        let a = compasso(&medida(1, &[nota('C', 1, &letra("la")), nota('D', 1, "")]));
        let b = compasso(&medida(1, &[nota('C', 1, &letra("li")), nota('D', 1, "")]));
        let d = diff(&a, &b);
        assert_eq!(d.compassos.len(), 1);
        assert!(d.compassos[0].so_marcacao());
        match &d.compassos[0].mudancas[..] {
            [MudancaDeNota::Alterada { campos, .. }] => {
                assert_eq!(campos, &[CampoAlterado::Letra]);
            }
            outro => panic!("{outro:?}"),
        }

        let dedo = |n: u8| {
            format!("<notations><technical><fingering>{n}</fingering></technical></notations>")
        };
        let a = compasso(&medida(1, &[nota('C', 1, &dedo(1))]));
        let b = compasso(&medida(1, &[nota('C', 1, &dedo(3))]));
        match &diff(&a, &b).compassos[0].mudancas[..] {
            [MudancaDeNota::Alterada { campos, .. }] => {
                assert_eq!(campos, &[CampoAlterado::Dedilhado]);
            }
            outro => panic!("{outro:?}"),
        }

        let harm = |k: &str| {
            format!("<harmony><root><root-step>C</root-step></root><kind>{k}</kind></harmony>")
        };
        let a = compasso(&medida(1, &[harm("major"), nota('C', 1, "")]));
        let b = compasso(&medida(1, &[harm("minor"), nota('C', 1, "")]));
        let d = diff(&a, &b);
        assert_eq!(d.compassos.len(), 1);
        assert!(d.compassos[0].mudancas.is_empty());
        assert!(d.compassos[0].cifra.is_some());
        assert!(d.compassos[0].so_marcacao());
    }

    #[test]
    fn compasso_a_mais_ou_a_menos() {
        let menor = compasso(&format!(
            "{}{}",
            medida(1, &[nota('C', 1, ""), nota('D', 1, ""), nota('E', 1, "")]),
            medida(2, &[nota('F', 1, ""), nota('G', 1, "")]),
        ));
        let d = diff(&menor, &base());
        assert_eq!(d.compassos.len(), 1);
        let c = &d.compassos[0];
        assert!(!c.existe_antes && c.existe_depois);
        assert_eq!(c.depois.len(), 1);
        let inv = diff(&base(), &menor);
        assert!(inv.compassos[0].existe_antes && !inv.compassos[0].existe_depois);
    }
}
