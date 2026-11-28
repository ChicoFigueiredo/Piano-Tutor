//! Ticket 14 (E01): o comando tipado de mudar a altura do serviço de documento, sem janela e
//! sem teclado. Cobre A2 (só o compasso da nota muda, byte a byte, em vários fixtures), A3 (letra,
//! cifra e dedilhado preservados), A6 (anacruse, duas vozes, tercina e repetição; só a mudança
//! autorizada e as consequências declaradas), limites, enarmonia pela armadura, ligaduras,
//! reversibilidade do comando e atomicidade das recusas.

use piano_tutor_core::documento::{
    Altura, AvisoDeEdicao, ComandoDeEdicao, DocumentoAberto, ErroDeEdicao, IdNota, LimitesDeAltura,
    ResultadoDaEdicao, SentidoDaAltura, UnidadeDeAltura,
};
use piano_tutor_core::partitura::Partitura;
use piano_tutor_fixtures::corpus::{Fixture, corpus, fixture};
use piano_tutor_fixtures::harness::{
    ItemSemantico, comparar_bytes, comparar_semantica, extrair_semantica, verificar_golden,
};

const UNIDADES: [UnidadeDeAltura; 3] = [
    UnidadeDeAltura::Semitom,
    UnidadeDeAltura::GrauDaEscala,
    UnidadeDeAltura::Oitava,
];
const SENTIDOS: [SentidoDaAltura; 2] = [SentidoDaAltura::Acima, SentidoDaAltura::Abaixo];

fn mudar(nota: IdNota, unidade: UnidadeDeAltura, sentido: SentidoDaAltura) -> ComandoDeEdicao {
    ComandoDeEdicao::MudarAltura {
        nota,
        unidade,
        sentido,
    }
}

fn notas_afinadas(d: &DocumentoAberto) -> Vec<IdNota> {
    d.documento()
        .notas()
        .filter(|n| n.altura.is_some())
        .map(|n| n.id)
        .collect()
}

/// Até `max` notas afinadas, espalhadas pela peça.
fn amostra(d: &DocumentoAberto, max: usize) -> Vec<IdNota> {
    let todas = notas_afinadas(d);
    let passo = todas.len().div_ceil(max).max(1);
    todas.into_iter().step_by(passo).collect()
}

/// Dois itens iguais em tudo, menos na altura.
fn sem_altura(i: &ItemSemantico) -> ItemSemantico {
    match i {
        ItemSemantico::Nota { .. } => {
            let mut c = i.clone();
            if let ItemSemantico::Nota { altura, .. } = &mut c {
                *altura = None;
            }
            c
        }
        outro => outro.clone(),
    }
}

/// Compara a semântica de `original` e `novo` e devolve as notas cuja altura difere (compasso,
/// altura antes, altura depois). Tudo o mais (letra, dedilhado, ligaduras, duração, voz, acorde,
/// cifra, barras, direções) tem de ser igual, senão o teste falha aqui.
///
/// O extrator ordena as notas de um mesmo instante também pela altura; por isso a comparação é
/// por conjunto (nota sem altura, altura), não por posição na lista.
fn diferencas_de_altura(original: &str, novo: &str) -> Vec<(String, String, String)> {
    let (a, b) = (
        extrair_semantica(original).expect("semântica"),
        extrair_semantica(novo).expect("semântica"),
    );
    assert_eq!(a.partes, b.partes, "a lista de partes mudou");
    assert_eq!(a.compassos.len(), b.compassos.len(), "nº de compassos");
    let pares = |itens: &[ItemSemantico]| -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = itens
            .iter()
            .map(|i| {
                let altura = match i {
                    ItemSemantico::Nota { altura, .. } => altura.clone().unwrap_or_default(),
                    _ => String::new(),
                };
                (format!("{:?}", sem_altura(i)), altura)
            })
            .collect();
        v.sort();
        v
    };
    let mut v = Vec::new();
    for (ca, cb) in a.compassos.iter().zip(&b.compassos) {
        assert_eq!(ca.pos, cb.pos);
        let (mut pa, mut pb) = (pares(&ca.itens), pares(&cb.itens));
        assert_eq!(pa.len(), pb.len(), "itens em {}", ca.pos);
        // Tira o que é igual dos dois lados; sobra, no máximo, a nota editada.
        pa.retain(|x| {
            if let Some(i) = pb.iter().position(|y| y == x) {
                pb.remove(i);
                false
            } else {
                true
            }
        });
        match (pa.as_slice(), pb.as_slice()) {
            ([], []) => {}
            ([(ra, antes)], [(rb, depois)]) => {
                assert_eq!(ra, rb, "algo além da altura mudou em {}", ca.pos);
                v.push((ca.pos.to_string(), antes.clone(), depois.clone()));
            }
            (x, y) => panic!("mudou mais que uma altura em {}: {x:?} / {y:?}", ca.pos),
        }
    }
    v
}

/// Aplica `cmd` num documento recém-aberto e confere tudo o que o serviço promete. `None` se o
/// comando foi recusado (limite) ou não mexeu em nada.
fn editar_e_conferir(
    nome: &str,
    original: &str,
    cmd: ComandoDeEdicao,
) -> Option<ResultadoDaEdicao> {
    let mut d = DocumentoAberto::abrir_texto(nome, original).expect("abre");
    let r = match d.aplicar(cmd.clone()) {
        Ok(r) => r,
        Err(ErroDeEdicao::ForaDosLimites { .. }) => return None,
        Err(e) => panic!("{nome}: {cmd:?}: {e}"),
    };
    if !r.alterou {
        return None;
    }
    let saida = d.exportar().expect("exporta");
    let afetados = r.compassos_afetados.clone();
    assert_eq!(afetados.len(), 1, "{nome}: um compasso afetado");
    assert_eq!(d.documento().compassos_sujos(), afetados, "{nome}: sujos");
    assert!(d.editado());

    // A2: só o compasso da nota difere, byte a byte; créditos, partes e vizinhos idênticos.
    let rel = comparar_bytes(original, &saida).expect("compara");
    assert_eq!(rel.compassos_diferentes, afetados, "{nome}: {cmd:?}");
    assert!(!rel.fora_dos_compassos, "{nome}: algo fora dos compassos");
    verificar_golden(original, &saida, &afetados, None)
        .unwrap_or_else(|v| panic!("{nome}: golden: {v:?}"));
    let pos = &afetados[0];
    let faixa = d
        .documento()
        .partes()
        .iter()
        .find(|p| p.id == pos.parte)
        .map(|p| p.compassos[pos.indice].faixa.clone())
        .expect("faixa");
    assert!(
        saida.starts_with(&original[..faixa.start]),
        "{nome}: prefixo"
    );
    assert!(saida.ends_with(&original[faixa.end..]), "{nome}: sufixo");

    // Semântica: só a altura (e só dessa nota) mudou.
    let sem = comparar_semantica(original, &saida).expect("semântica");
    assert_eq!(
        sem.compassos_diferentes, afetados,
        "{nome}: {:?}",
        sem.detalhes
    );
    let difs = diferencas_de_altura(original, &saida);
    assert_eq!(difs.len(), 1, "{nome}: {difs:?}");

    // O modelo de tocar já editado é igual ao que sai de reabrir o texto exportado, e as
    // identidades das notas são as mesmas.
    let reaberto = DocumentoAberto::abrir_texto(nome, &saida).expect("reabre");
    assert_eq!(
        reaberto.partitura(),
        d.partitura(),
        "{nome}: modelo de tocar"
    );
    assert_eq!(reaberto.ids(), d.ids(), "{nome}: identidades");
    d.conferir_exportacao(&saida).expect("conferência");

    // A nota continua sendo a mesma identidade, agora na altura nova.
    assert_eq!(r.nota, cmd.nota().expect("comando de nota"));
    assert_eq!(
        d.documento().nota(r.nota).and_then(|n| n.altura),
        Some(r.depois)
    );

    // Reversível: o inverso devolve o texto original, byte a byte.
    let mut volta = d.clone();
    let rv = volta.aplicar(r.inverso.clone()).expect("inverso");
    assert!(rv.alterou);
    assert_eq!(
        volta.exportar().expect("exporta"),
        original,
        "{nome}: o inverso de {cmd:?} não restaurou o texto"
    );
    assert_eq!(
        volta.partitura(),
        DocumentoAberto::abrir_texto(nome, original)
            .expect("abre")
            .partitura()
    );
    assert_eq!(rv.inverso, r.aplicado, "o inverso do inverso é o aplicado");
    Some(r)
}

fn varre(f: &Fixture, max_notas: usize) -> usize {
    let d = DocumentoAberto::abrir_texto(f.id, &f.conteudo).expect("abre");
    let mut feitos = 0;
    for id in amostra(&d, max_notas) {
        for u in UNIDADES {
            for s in SENTIDOS {
                feitos +=
                    usize::from(editar_e_conferir(f.id, &f.conteudo, mudar(id, u, s)).is_some());
            }
        }
    }
    feitos
}

#[test]
fn a2_corpus_so_o_compasso_da_nota_muda_em_varios_fixtures_com_duas_vozes() {
    let mut total = 0;
    let mut com_duas_vozes = 0;
    for f in corpus() {
        let d = DocumentoAberto::abrir_texto(f.id, &f.conteudo).expect("abre");
        let duas = d.documento().partes().iter().any(|p| {
            p.compassos
                .iter()
                .any(|c| c.notas.iter().any(|n| n.voz != c.notas[0].voz))
        });
        com_duas_vozes += usize::from(duas);
        total += varre(&f, 4);
    }
    assert!(total >= 200, "amostra pequena demais: {total}");
    assert!(
        com_duas_vozes >= 3,
        "o critério pede pelo menos três fixtures com duas vozes"
    );
}

/// Exaustivo (toda nota afinada, os seis comandos): rode em release com `--ignored`.
#[test]
#[ignore = "exaustivo; rodar em release"]
fn a2_exaustivo_toda_nota_afinada_com_os_seis_comandos() {
    let mut total = 0;
    for f in corpus() {
        total += varre(&f, usize::MAX);
    }
    assert!(total > 5000, "{total}");
}

#[test]
fn a2_duas_vozes_nota_errada_numa_voz_so_muda_aquele_compasso() {
    let f = fixture("duas-vozes");
    let d = DocumentoAberto::abrir_texto(f.id, &f.conteudo).expect("abre");
    // Uma nota de cada voz do compasso com `backup`.
    for voz in ["1", "2"] {
        let alvo = d
            .documento()
            .notas()
            .find(|n| n.voz == voz && n.altura.is_some())
            .expect("nota")
            .id;
        let r = editar_e_conferir(
            f.id,
            &f.conteudo,
            mudar(alvo, UnidadeDeAltura::Semitom, SentidoDaAltura::Acima),
        )
        .expect("edita");
        assert_eq!(r.compassos_afetados.len(), 1);
    }
}

/// Partitura com letra, cifra e dedilhado nas mesmas notas, em dois compassos.
const LETRA_CIFRA_DEDILHADO: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<score-partwise version=\"4.0\">
  <part-list><score-part id=\"P1\"><part-name>Voz</part-name></score-part></part-list>
  <part id=\"P1\">
    <measure number=\"1\"><attributes><divisions>1</divisions><key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes>
      <harmony><root><root-step>C</root-step></root><kind text=\"\">major</kind></harmony>
      <note><pitch><step>C</step><octave>5</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><notations><technical><fingering>1</fingering></technical></notations><lyric number=\"1\"><syllabic>begin</syllabic><text>Pi</text></lyric></note>
      <note><pitch><step>D</step><octave>5</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><notations><technical><fingering>2</fingering></technical></notations><lyric number=\"1\"><syllabic>end</syllabic><text>a</text></lyric></note>
      <harmony><root><root-step>G</root-step></root><kind text=\"7\">dominant</kind><bass><bass-step>B</bass-step></bass></harmony>
      <note><pitch><step>E</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>half</type><notations><technical><fingering>3</fingering></technical></notations><lyric number=\"1\"><syllabic>single</syllabic><text>no</text></lyric><lyric number=\"2\"><syllabic>single</syllabic><text>ré</text></lyric></note>
    </measure>
    <measure number=\"2\">
      <harmony><root><root-step>F</root-step></root><kind text=\"\">major</kind></harmony>
      <note><pitch><step>F</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>half</type><notations><technical><fingering>4</fingering></technical></notations><lyric number=\"1\"><syllabic>single</syllabic><text>Tu</text></lyric></note>
      <note><pitch><step>G</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>half</type><notations><technical><fingering>5</fingering></technical></notations><lyric number=\"1\"><syllabic>single</syllabic><text>lá</text></lyric></note>
    <barline location=\"right\"><bar-style>light-heavy</bar-style></barline></measure>
  </part>
</score-partwise>
";

fn contar(texto: &str, trecho: &str) -> usize {
    texto.matches(trecho).count()
}

#[test]
fn a3_letra_cifra_e_dedilhado_do_compasso_e_dos_demais_ficam_nas_mesmas_notas() {
    let d = DocumentoAberto::abrir_texto("lcd", LETRA_CIFRA_DEDILHADO).expect("abre");
    let ids = notas_afinadas(&d);
    assert_eq!(ids.len(), 5);
    for id in &ids {
        for u in UNIDADES {
            for s in SENTIDOS {
                let Some(r) = editar_e_conferir("lcd", LETRA_CIFRA_DEDILHADO, mudar(*id, u, s))
                else {
                    continue;
                };
                // Comparação semântica: só a altura mudou; letra, cifra e dedilhado idênticos
                // e nas mesmas notas (a comparação por item já falha se algo se deslocou).
                let mut d2 =
                    DocumentoAberto::abrir_texto("lcd", LETRA_CIFRA_DEDILHADO).expect("abre");
                d2.aplicar(mudar(*id, u, s)).expect("aplica");
                let saida = d2.exportar().expect("exporta");
                assert_eq!(
                    contar(&saida, "<lyric"),
                    contar(LETRA_CIFRA_DEDILHADO, "<lyric")
                );
                assert_eq!(
                    contar(&saida, "<harmony"),
                    contar(LETRA_CIFRA_DEDILHADO, "<harmony")
                );
                assert_eq!(
                    contar(&saida, "<fingering"),
                    contar(LETRA_CIFRA_DEDILHADO, "<fingering")
                );
                assert!(r.alterou);
            }
        }
    }
}

/// Anacruse (compasso 0 implícito), duas vozes (com `backup`), tercina e repetição.
const ANACRUSE_VOZES_TERCINA_REPETICAO: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<score-partwise version=\"4.0\">
  <part-list><score-part id=\"P1\"><part-name>Piano</part-name></score-part></part-list>
  <part id=\"P1\">
    <measure number=\"0\" implicit=\"yes\"><attributes><divisions>6</divisions><key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes>
      <note><pitch><step>G</step><octave>4</octave></pitch><duration>6</duration><voice>1</voice><type>quarter</type></note>
    </measure>
    <measure number=\"1\">
      <barline location=\"left\"><bar-style>heavy-light</bar-style><repeat direction=\"forward\"/></barline>
      <note><pitch><step>C</step><octave>5</octave></pitch><duration>12</duration><voice>1</voice><type>half</type></note>
      <note><pitch><step>E</step><octave>5</octave></pitch><duration>6</duration><voice>1</voice><type>quarter</type></note>
      <note><pitch><step>D</step><octave>5</octave></pitch><duration>6</duration><voice>1</voice><type>quarter</type></note>
      <backup><duration>24</duration></backup>
      <note><pitch><step>C</step><octave>3</octave></pitch><duration>24</duration><voice>2</voice><type>whole</type></note>
    </measure>
    <measure number=\"2\">
      <note><pitch><step>E</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>eighth</type><time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification><notations><tuplet type=\"start\"/></notations></note>
      <note><pitch><step>F</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>eighth</type><time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification></note>
      <note><pitch><step>G</step><octave>5</octave></pitch><duration>2</duration><voice>1</voice><type>eighth</type><time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification><notations><tuplet type=\"stop\"/></notations></note>
      <note><pitch><step>G</step><octave>5</octave></pitch><duration>6</duration><voice>1</voice><type>quarter</type></note>
      <note><pitch><step>A</step><octave>5</octave></pitch><duration>12</duration><voice>1</voice><type>half</type></note>
      <barline location=\"right\"><bar-style>light-heavy</bar-style><repeat direction=\"backward\"/></barline>
    </measure>
  </part>
</score-partwise>
";

fn duracoes(d: &DocumentoAberto) -> Vec<(i64, i64)> {
    d.partitura()
        .partes
        .iter()
        .flat_map(|p| p.compassos.iter())
        .flat_map(|c| c.notas.iter())
        .map(|n| (n.duracao.numerador(), n.duracao.denominador()))
        .collect()
}

#[test]
fn a6_anacruse_duas_vozes_tercina_e_repeticao_so_a_mudanca_autorizada() {
    let base = DocumentoAberto::abrir_texto("a6", ANACRUSE_VOZES_TERCINA_REPETICAO).expect("abre");
    assert_eq!(base.partitura().compassos.len(), 3);
    let antes_duracoes = duracoes(&base);
    let mut feitos = 0;
    for id in notas_afinadas(&base) {
        for u in UNIDADES {
            for s in SENTIDOS {
                let Some(r) =
                    editar_e_conferir("a6", ANACRUSE_VOZES_TERCINA_REPETICAO, mudar(id, u, s))
                else {
                    continue;
                };
                feitos += 1;
                // Nenhuma duração foi preenchida, apagada ou mudada, e não há aviso de duração.
                let mut d = DocumentoAberto::abrir_texto("a6", ANACRUSE_VOZES_TERCINA_REPETICAO)
                    .expect("abre");
                d.aplicar(mudar(id, u, s)).expect("aplica");
                assert_eq!(duracoes(&d), antes_duracoes);
                let saida = d.exportar().expect("exporta");
                for marca in [
                    "<backup>",
                    "<repeat ",
                    "<tuplet ",
                    "<time-modification>",
                    "<duration>",
                ] {
                    assert_eq!(
                        contar(&saida, marca),
                        contar(ANACRUSE_VOZES_TERCINA_REPETICAO, marca),
                        "{marca}"
                    );
                }
                // As únicas consequências declaradas possíveis aqui são de acidente: nunca de
                // duração, de tercina nem de repetição.
                assert!(
                    r.avisos
                        .iter()
                        .all(|a| matches!(a, AvisoDeEdicao::AcidenteDeNotaSeguinteMudou { .. })),
                    "{:?}",
                    r.avisos
                );
            }
        }
    }
    assert!(feitos >= 40, "{feitos}");
}

#[test]
fn a6_a_tercina_editada_continua_tercina_e_a_anacruse_continua_anacruse() {
    let mut d = DocumentoAberto::abrir_texto("a6", ANACRUSE_VOZES_TERCINA_REPETICAO).expect("abre");
    let fa = d
        .documento()
        .notas()
        .find(|n| n.altura.is_some_and(|a| a.passo == 'F' && a.oitava == 5))
        .expect("fá da tercina")
        .id;
    let r = d
        .aplicar(mudar(fa, UnidadeDeAltura::Semitom, SentidoDaAltura::Acima))
        .expect("aplica");
    assert_eq!(
        r.depois,
        Altura {
            passo: 'F',
            alter: 1,
            oitava: 5
        }
    );
    let saida = d.exportar().expect("exporta");
    assert!(saida.contains(
        "<pitch><step>F</step><alter>1</alter><octave>5</octave></pitch><duration>2</duration>"
    ));
    assert!(saida.contains("<accidental>sharp</accidental>"));
    assert!(saida.contains("<measure number=\"0\" implicit=\"yes\">"));
    let nota = d.partitura().partes[0].compassos[2].notas[1].clone();
    assert_eq!(nota.tuplo, Some((3, 2)));
    assert_eq!(d.partitura().compassos[0].numero, "0");
}

// --- Enarmonia pela armadura (tabela) -------------------------------------------------------

/// Partitura mínima de um compasso com a armadura dada e as `notas` (`<note>` completas).
fn com_armadura(quintas: i8, notas: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<score-partwise version=\"4.0\">\n\
<part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list>\n\
<part id=\"P1\"><measure number=\"1\"><attributes><divisions>1</divisions><key><fifths>{quintas}</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes>\n{notas}\n</measure></part>\n</score-partwise>\n"
    )
}

fn nota_xml(passo: char, alter: i8, oitava: i8, acidente: Option<&str>, duracao: i32) -> String {
    let alter = if alter == 0 {
        String::new()
    } else {
        format!("<alter>{alter}</alter>")
    };
    let acidente = acidente.map_or(String::new(), |a| format!("<accidental>{a}</accidental>"));
    format!(
        "<note><pitch><step>{passo}</step>{alter}<octave>{oitava}</octave></pitch><duration>{duracao}</duration><voice>1</voice><type>quarter</type>{acidente}</note>"
    )
}

fn altura(passo: char, alter: i8, oitava: i8) -> Altura {
    Altura {
        passo,
        alter,
        oitava,
    }
}

type Caso = (
    i8,
    (char, i8, i8, Option<&'static str>),
    UnidadeDeAltura,
    SentidoDaAltura,
    (char, i8, i8),
    Option<&'static str>,
);

#[test]
fn enarmonia_e_acidente_impresso_pela_armadura_em_tabela() {
    use SentidoDaAltura::{Abaixo, Acima};
    use UnidadeDeAltura::{GrauDaEscala, Oitava, Semitom};
    let casos: &[Caso] = &[
        // (quintas, nota de partida, unidade, sentido, nota esperada, acidente esperado)
        (
            0,
            ('C', 0, 4, None),
            Semitom,
            Acima,
            ('C', 1, 4),
            Some("sharp"),
        ),
        (
            0,
            ('C', 1, 4, Some("sharp")),
            Semitom,
            Acima,
            ('D', 0, 4),
            None,
        ),
        (
            0,
            ('D', 0, 4, None),
            Semitom,
            Abaixo,
            ('D', -1, 4),
            Some("flat"),
        ),
        (
            0,
            ('D', -1, 4, Some("flat")),
            Semitom,
            Abaixo,
            ('C', 0, 4),
            None,
        ),
        (0, ('E', 0, 4, None), Semitom, Acima, ('F', 0, 4), None),
        (0, ('B', 0, 4, None), Semitom, Acima, ('C', 0, 5), None),
        (0, ('C', 0, 4, None), Semitom, Abaixo, ('B', 0, 3), None),
        (1, ('F', 1, 4, None), Semitom, Acima, ('G', 0, 4), None),
        (
            1,
            ('E', 0, 4, None),
            Semitom,
            Acima,
            ('F', 0, 4),
            Some("natural"),
        ),
        (1, ('G', 0, 4, None), Semitom, Abaixo, ('F', 1, 4), None),
        (
            1,
            ('F', 0, 4, Some("natural")),
            Semitom,
            Acima,
            ('F', 1, 4),
            None,
        ),
        (-3, ('D', 0, 4, None), Semitom, Acima, ('E', -1, 4), None),
        (
            -3,
            ('E', -1, 4, None),
            Semitom,
            Acima,
            ('E', 0, 4),
            Some("natural"),
        ),
        (-3, ('E', -1, 4, None), Semitom, Abaixo, ('D', 0, 4), None),
        (
            -3,
            ('C', 0, 4, None),
            Semitom,
            Acima,
            ('D', -1, 4),
            Some("flat"),
        ),
        (
            -3,
            ('A', 0, 4, Some("natural")),
            Semitom,
            Abaixo,
            ('A', -1, 4),
            None,
        ),
        (
            7,
            ('B', 0, 3, Some("natural")),
            Semitom,
            Acima,
            ('B', 1, 3),
            None,
        ),
        (-7, ('B', -1, 3, None), Semitom, Acima, ('C', -1, 4), None),
        // Grau da escala: a alteração vem da armadura.
        (0, ('B', 0, 4, None), GrauDaEscala, Acima, ('C', 0, 5), None),
        (
            0,
            ('C', 0, 4, None),
            GrauDaEscala,
            Abaixo,
            ('B', 0, 3),
            None,
        ),
        (1, ('E', 0, 4, None), GrauDaEscala, Acima, ('F', 1, 4), None),
        (
            1,
            ('G', 0, 4, None),
            GrauDaEscala,
            Abaixo,
            ('F', 1, 4),
            None,
        ),
        (
            -3,
            ('D', 0, 4, None),
            GrauDaEscala,
            Acima,
            ('E', -1, 4),
            None,
        ),
        (
            -3,
            ('G', 0, 4, None),
            GrauDaEscala,
            Acima,
            ('A', -1, 4),
            None,
        ),
        (
            0,
            ('C', 1, 4, Some("sharp")),
            GrauDaEscala,
            Acima,
            ('D', 0, 4),
            None,
        ),
        // Oitava: mesma letra e alteração; o acidente que a nota tinha fica.
        (0, ('C', 0, 4, None), Oitava, Acima, ('C', 0, 5), None),
        (
            0,
            ('C', 1, 4, Some("sharp")),
            Oitava,
            Acima,
            ('C', 1, 5),
            Some("sharp"),
        ),
        (1, ('F', 1, 4, None), Oitava, Abaixo, ('F', 1, 3), None),
        (-3, ('B', -1, 4, None), Oitava, Acima, ('B', -1, 5), None),
    ];
    for (q, (p, a, o, ac), unidade, sentido, esperada, acidente) in casos {
        let xml = com_armadura(*q, &nota_xml(*p, *a, *o, *ac, 4));
        let mut d = DocumentoAberto::abrir_texto("t", &xml).expect("abre");
        let id = notas_afinadas(&d)[0];
        let r = d
            .aplicar(mudar(id, *unidade, *sentido))
            .unwrap_or_else(|e| panic!("quintas {q} {p}{a}{o} {unidade:?} {sentido:?}: {e}"));
        let (ep, ea, eo) = *esperada;
        assert_eq!(
            r.depois,
            altura(ep, ea, eo),
            "quintas {q}, {p}{a:+}{o}, {unidade:?} {sentido:?}"
        );
        let saida = d.exportar().expect("exporta");
        let achado = saida
            .split("<accidental>")
            .nth(1)
            .and_then(|r| r.split("</accidental>").next());
        assert_eq!(
            achado, *acidente,
            "acidente impresso: quintas {q}, {p}{a:+}{o}, {unidade:?} {sentido:?}\n{saida}"
        );
        // Uma só mudança sonora de acordo com a unidade.
        let semitons = |x: Altura| x.numero_midi().expect("midi");
        let delta = semitons(r.depois) - semitons(r.antes);
        match unidade {
            Semitom => assert_eq!(delta.abs(), 1),
            Oitava => assert_eq!(delta.abs(), 12),
            GrauDaEscala => assert!(
                (1..=2).contains(&delta.abs()) || delta.abs() == 3,
                "{delta}"
            ),
        }
        assert_eq!(delta.signum(), if *sentido == Acima { 1 } else { -1 });
    }
}

#[test]
fn acidente_em_vigor_no_compasso_decide_o_sinal_da_nota_editada() {
    // Dó sustenido (com sinal) e depois Ré: descer um grau leva o Ré a Dó natural, e o Dó
    // sustenido anterior ainda vale no compasso: o bequadro é necessário.
    let xml = com_armadura(
        0,
        &format!(
            "{}\n{}",
            nota_xml('C', 1, 4, Some("sharp"), 2),
            nota_xml('D', 0, 4, None, 2)
        ),
    );
    let mut d = DocumentoAberto::abrir_texto("t", &xml).expect("abre");
    let ids = notas_afinadas(&d);
    let r = d
        .aplicar(mudar(
            ids[1],
            UnidadeDeAltura::GrauDaEscala,
            SentidoDaAltura::Abaixo,
        ))
        .expect("aplica");
    assert_eq!(r.depois, altura('C', 0, 4));
    assert!(
        d.exportar()
            .expect("exporta")
            .contains("<accidental>natural</accidental>")
    );
    assert_eq!(
        d.partitura().partes[0].compassos[0].notas[1].acidente,
        Some(piano_tutor_core::partitura::Acidente::Bequadro)
    );
    assert!(r.avisos.is_empty());
}

#[test]
fn nota_seguinte_que_dependia_do_acidente_da_editada_gera_aviso_e_nao_e_mexida() {
    // Fá sustenido com sinal e, depois, outro Fá sustenido sem sinal (vale o do compasso).
    // Subir o primeiro tira o Fá sustenido de vigor: o segundo precisaria do sinal.
    let xml = com_armadura(
        0,
        &format!(
            "{}\n{}",
            nota_xml('F', 1, 4, Some("sharp"), 2),
            nota_xml('F', 1, 4, None, 2)
        ),
    );
    let mut d = DocumentoAberto::abrir_texto("t", &xml).expect("abre");
    let ids = notas_afinadas(&d);
    let r = d
        .aplicar(mudar(
            ids[0],
            UnidadeDeAltura::Semitom,
            SentidoDaAltura::Acima,
        ))
        .expect("aplica");
    assert_eq!(r.depois, altura('G', 0, 4));
    assert_eq!(
        r.avisos,
        vec![AvisoDeEdicao::AcidenteDeNotaSeguinteMudou { nota: ids[1] }]
    );
    assert!(!r.avisos[0].texto().is_empty());
    // A segunda nota segue exatamente como estava: sem mudança silenciosa.
    let saida = d.exportar().expect("exporta");
    assert!(saida.contains(&nota_xml('F', 1, 4, None, 2)));
    assert_eq!(
        d.documento().nota(ids[1]).and_then(|n| n.altura),
        Some(altura('F', 1, 4))
    );
}

// --- Limites e atomicidade -----------------------------------------------------------------

/// Tudo o que um comando recusado não pode ter mexido.
#[derive(Debug, PartialEq)]
struct Retrato {
    texto: String,
    modelo: Partitura,
    notas_com_id: usize,
    sujos: Vec<String>,
    editado: bool,
    alturas: Vec<(IdNota, Option<Altura>)>,
}

fn retrato(d: &DocumentoAberto) -> Retrato {
    Retrato {
        texto: d.exportar().expect("exporta"),
        modelo: d.partitura().clone(),
        notas_com_id: d.ids().total(),
        sujos: d
            .documento()
            .compassos_sujos()
            .iter()
            .map(ToString::to_string)
            .collect(),
        editado: d.editado(),
        alturas: d.documento().notas().map(|n| (n.id, n.altura)).collect(),
    }
}

fn recusado(d: &mut DocumentoAberto, cmd: ComandoDeEdicao) -> ErroDeEdicao {
    let antes = retrato(d);
    let e = d.aplicar(cmd).expect_err("devia recusar");
    assert_eq!(retrato(d), antes, "a recusa alterou o modelo: {e}");
    assert!(!e.to_string().is_empty());
    e
}

type CasoDeLimite = (
    LimitesDeAltura,
    (char, i8, i8),
    UnidadeDeAltura,
    SentidoDaAltura,
);

#[test]
fn alturas_fora_dos_limites_sao_recusadas_com_mensagem_e_nada_muda() {
    use SentidoDaAltura::{Abaixo, Acima};
    use UnidadeDeAltura::{GrauDaEscala, Oitava, Semitom};
    let casos: &[CasoDeLimite] = &[
        // Teclado de referência (padrão): Lá0 (21) e Dó8 (108).
        (LimitesDeAltura::PIANO_88, ('A', 0, 0), Semitom, Abaixo),
        (LimitesDeAltura::PIANO_88, ('A', 0, 0), GrauDaEscala, Abaixo),
        (LimitesDeAltura::PIANO_88, ('C', 0, 8), Semitom, Acima),
        (LimitesDeAltura::PIANO_88, ('C', 0, 8), Oitava, Acima),
        (LimitesDeAltura::PIANO_88, ('C', 0, 1), Oitava, Abaixo),
        // MIDI completo: Dó0 (12, a oitava 0 do MusicXML) e Sol9 (127).
        (LimitesDeAltura::MIDI_COMPLETO, ('C', 0, 0), Semitom, Abaixo),
        (LimitesDeAltura::MIDI_COMPLETO, ('G', 0, 9), Semitom, Acima),
        (
            LimitesDeAltura::MIDI_COMPLETO,
            ('G', 0, 9),
            GrauDaEscala,
            Acima,
        ),
        (LimitesDeAltura::MIDI_COMPLETO, ('C', 0, 9), Oitava, Acima),
        (LimitesDeAltura::MIDI_COMPLETO, ('D', 0, 0), Oitava, Abaixo),
    ];
    for (limites, (p, a, o), unidade, sentido) in casos {
        let xml = com_armadura(0, &nota_xml(*p, *a, *o, None, 4));
        let mut d = DocumentoAberto::abrir_texto("t", &xml).expect("abre");
        d.definir_limites(*limites);
        let id = notas_afinadas(&d)[0];
        let e = recusado(&mut d, mudar(id, *unidade, *sentido));
        assert!(
            matches!(e, ErroDeEdicao::ForaDosLimites { .. }),
            "{p}{a}{o} {unidade:?} {sentido:?}: {e:?}"
        );
        assert!(e.to_string().contains("fora dos limites"), "{e}");
        assert!(e.to_string().contains("não foi alterada"), "{e}");
        assert!(!d.editado());
    }
}

#[test]
fn limites_aceitam_o_extremo_exato() {
    let xml = com_armadura(
        0,
        &format!(
            "{}\n{}",
            nota_xml('A', 0, 0, None, 2),
            nota_xml('C', 0, 8, None, 2)
        ),
    );
    let mut d = DocumentoAberto::abrir_texto("t", &xml).expect("abre");
    let ids = notas_afinadas(&d);
    // Lá0 sobe e Dó8 desce sem problema; Dó8 descer uma oitava e Lá0 subir também.
    d.aplicar(mudar(
        ids[0],
        UnidadeDeAltura::Semitom,
        SentidoDaAltura::Acima,
    ))
    .expect("Lá0 sobe");
    d.aplicar(mudar(
        ids[1],
        UnidadeDeAltura::Semitom,
        SentidoDaAltura::Abaixo,
    ))
    .expect("Dó8 desce");
    // Chegar exatamente em Lá0 (21) e em Dó8 (108) vale.
    let r = d
        .aplicar(ComandoDeEdicao::DefinirAltura {
            nota: ids[0],
            altura: altura('A', 0, 0),
            acidente: None,
        })
        .expect("volta ao Lá0");
    assert_eq!(r.depois, altura('A', 0, 0));
    d.aplicar(ComandoDeEdicao::DefinirAltura {
        nota: ids[1],
        altura: altura('C', 0, 8),
        acidente: None,
    })
    .expect("volta ao Dó8");
    // E um passo além, pela forma resolvida, é recusado.
    let e = recusado(
        &mut d,
        ComandoDeEdicao::DefinirAltura {
            nota: ids[1],
            altura: altura('C', 1, 8),
            acidente: None,
        },
    );
    assert!(
        matches!(e, ErroDeEdicao::ForaDosLimites { midi: 109, .. }),
        "{e:?}"
    );
}

#[test]
fn pausa_nota_inexistente_e_altura_malformada_sao_recusadas_sem_mudar_nada() {
    let xml = com_armadura(
        0,
        "<note><rest/><duration>2</duration><voice>1</voice><type>half</type></note>\n<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type></note>",
    );
    let mut d = DocumentoAberto::abrir_texto("t", &xml).expect("abre");
    let pausa = d.documento().notas().find(|n| n.pausa).expect("pausa").id;
    let nota = notas_afinadas(&d)[0];
    let e = recusado(
        &mut d,
        mudar(pausa, UnidadeDeAltura::Semitom, SentidoDaAltura::Acima),
    );
    assert_eq!(e, ErroDeEdicao::NotaSemAltura(pausa));
    let e = recusado(
        &mut d,
        mudar(
            IdNota(9999),
            UnidadeDeAltura::Oitava,
            SentidoDaAltura::Acima,
        ),
    );
    assert_eq!(e, ErroDeEdicao::NotaInexistente(IdNota(9999)));
    for ruim in [
        altura('H', 0, 4),
        altura('C', 3, 4),
        altura('C', 0, 10),
        altura('C', 0, -1),
    ] {
        let e = recusado(
            &mut d,
            ComandoDeEdicao::DefinirAltura {
                nota,
                altura: ruim,
                acidente: None,
            },
        );
        assert!(
            matches!(
                e,
                ErroDeEdicao::Documento(_) | ErroDeEdicao::ForaDosLimites { .. }
            ),
            "{ruim:?}: {e:?}"
        );
    }
    let e = recusado(
        &mut d,
        ComandoDeEdicao::DefinirAltura {
            nota,
            altura: altura('C', 0, 4),
            acidente: Some("<x/>".into()),
        },
    );
    assert!(matches!(e, ErroDeEdicao::Documento(_)), "{e:?}");
}

#[test]
fn definir_a_altura_que_a_nota_ja_tem_nao_suja_nada() {
    let f = fixture("anacruse");
    let mut d = DocumentoAberto::abrir_texto(f.id, &f.conteudo).expect("abre");
    let id = notas_afinadas(&d)[0];
    let atual = d
        .documento()
        .nota(id)
        .and_then(|n| n.altura)
        .expect("altura");
    let r = d
        .aplicar(ComandoDeEdicao::DefinirAltura {
            nota: id,
            altura: atual,
            acidente: None,
        })
        .expect("aplica");
    assert!(!r.alterou && r.compassos_afetados.is_empty());
    assert!(!d.editado());
    assert_eq!(d.exportar().expect("exporta"), f.conteudo);
}

// --- Ligaduras, vigas e o resto do compasso ------------------------------------------------

#[test]
fn ligadura_de_valor_avisa_e_expressao_viga_e_prolongamento_ficam_intactos() {
    for id_fixture in ["brahms-wiegenlied", "faure-barcarolle"] {
        let f = fixture(id_fixture);
        let d0 = DocumentoAberto::abrir_texto(f.id, &f.conteudo).expect("abre");
        let ligadas: Vec<IdNota> = notas_afinadas(&d0)
            .into_iter()
            .filter(|id| d0.documento().tem_ligadura_de_valor(*id).expect("lê"))
            .collect();
        assert!(!ligadas.is_empty(), "{id_fixture}: sem nota ligada");
        let mut avisadas = 0;
        for id in ligadas.into_iter().step_by(7).take(12) {
            let mut d = d0.clone();
            let r = match d.aplicar(mudar(id, UnidadeDeAltura::Oitava, SentidoDaAltura::Acima)) {
                Ok(r) => r,
                Err(ErroDeEdicao::ForaDosLimites { .. }) => continue,
                Err(e) => panic!("{e}"),
            };
            assert!(
                r.avisos
                    .contains(&AvisoDeEdicao::LigaduraDeValorComAlturasDiferentes { nota: id }),
                "{id_fixture}: {:?}",
                r.avisos
            );
            avisadas += 1;
            let saida = d.exportar().expect("exporta");
            for marca in [
                "<tie ",
                "<tied ",
                "<slur ",
                "<beam ",
                "<lyric",
                "<notations>",
                "<stem>",
            ] {
                assert_eq!(
                    contar(&saida, marca),
                    contar(&f.conteudo, marca),
                    "{id_fixture}: {marca}"
                );
            }
            // O modelo de tocar continua com as mesmas ligaduras e vigas na nota.
            let reaberto = DocumentoAberto::abrir_texto(f.id, &saida).expect("reabre");
            assert_eq!(reaberto.partitura(), d.partitura());
        }
        assert!(avisadas > 0, "{id_fixture}");
    }
}

#[test]
fn nota_sem_ligadura_de_valor_editada_nao_gera_aviso_de_ligadura() {
    let f = fixture("brilha-brilha");
    let d0 = DocumentoAberto::abrir_texto(f.id, &f.conteudo).expect("abre");
    for id in amostra(&d0, 10) {
        let mut d = d0.clone();
        if let Ok(r) = d.aplicar(mudar(id, UnidadeDeAltura::Semitom, SentidoDaAltura::Acima)) {
            assert!(
                !r.avisos.iter().any(|a| matches!(
                    a,
                    AvisoDeEdicao::LigaduraDeValorComAlturasDiferentes { .. }
                )),
                "{:?}",
                r.avisos
            );
        }
    }
}

#[test]
fn o_mesmo_comando_dos_dois_lados_da_porta_da_o_mesmo_resultado() {
    // A forma resolvida devolvida (`aplicado`) reaplicada num documento novo dá o mesmo texto.
    for f in corpus().into_iter().take(8) {
        let d0 = DocumentoAberto::abrir_texto(f.id, &f.conteudo).expect("abre");
        for id in amostra(&d0, 3) {
            for u in UNIDADES {
                let mut a = d0.clone();
                let Ok(r) = a.aplicar(mudar(id, u, SentidoDaAltura::Acima)) else {
                    continue;
                };
                let mut b = d0.clone();
                b.aplicar(r.aplicado.clone()).expect("aplicado reaplica");
                assert_eq!(
                    a.exportar().expect("exporta"),
                    b.exportar().expect("exporta"),
                    "{}",
                    f.id
                );
                assert_eq!(a.partitura(), b.partitura(), "{}", f.id);
            }
        }
    }
}
