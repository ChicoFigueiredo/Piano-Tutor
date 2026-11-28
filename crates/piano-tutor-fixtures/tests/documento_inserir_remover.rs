//! Ticket 17 (E01): inserir nota ou pausa numa posição do compasso e remover nota (vira pausa),
//! no serviço de documento (sem janela e sem teclado). Cobre a soma do compasso (A4), desfazer
//! e refazer byte a byte (A5), a recusa sem rastro, os avisos de letra, dedilhado e ornamento
//! de uma nota removida, as identidades (arquivo lateral) e a varredura do corpus inteiro.

use piano_tutor_core::documento::{
    Altura, AvisoDeEdicao, ComandoDeEdicao, DocumentoAberto, ErroDeEdicao, IdNota, Nota,
    PosCompasso, TabelaIds,
};
use piano_tutor_core::partitura::{Figura, Fracao, Partitura};
use piano_tutor_fixtures::corpus::{Fixture, corpus};
use piano_tutor_fixtures::harness::{comparar_bytes, verificar_golden};
use std::collections::BTreeMap;

fn abrir(texto: &str) -> DocumentoAberto {
    DocumentoAberto::abrir_texto("t", texto).expect("abre")
}

fn f(n: i64, d: i64) -> Fracao {
    Fracao::nova(n, d)
}

fn alt(passo: char, oitava: i8) -> Altura {
    Altura {
        passo,
        alter: 0,
        oitava,
    }
}

/// Tudo o que o documento sabe, para comparar dois estados por igualdade.
#[derive(Debug, PartialEq)]
struct Retrato {
    texto: String,
    notas: Vec<Nota>,
    sujos: Vec<String>,
    partitura: Partitura,
    ids: Vec<Option<IdNota>>,
    editado: bool,
    modificado: bool,
}

fn retrato(d: &DocumentoAberto) -> Retrato {
    let p = d.partitura();
    let mut ids = Vec::new();
    for (pi, parte) in p.partes.iter().enumerate() {
        for (ci, c) in parte.compassos.iter().enumerate() {
            for oi in 0..c.notas.len() {
                ids.push(d.ids().id(pi, ci, oi));
            }
        }
    }
    Retrato {
        texto: d.exportar().expect("exporta"),
        notas: d.documento().notas().cloned().collect(),
        sujos: d
            .documento()
            .compassos_sujos()
            .iter()
            .map(ToString::to_string)
            .collect(),
        partitura: p.clone(),
        ids,
        editado: d.editado(),
        modificado: d.modificado(),
    }
}

/// Soma das durações de cada voz (sem notas de acorde nem de adorno) por parte e compasso.
fn somas_por_voz(p: &Partitura) -> BTreeMap<(usize, usize, u8, u8), Fracao> {
    let mut m = BTreeMap::new();
    for (pi, parte) in p.partes.iter().enumerate() {
        for (ci, c) in parte.compassos.iter().enumerate() {
            for n in c.notas.iter().filter(|n| !n.acorde && !n.adorno) {
                let e = m.entry((pi, ci, n.pauta, n.voz)).or_insert(Fracao::ZERO);
                *e = *e + n.duracao;
            }
        }
    }
    m
}

/// Compasso de 4/4 (divisions 1) numa parte só.
fn peca(medida: &str) -> String {
    peca_com(1, medida, "")
}

fn peca_com(div: u32, medida: &str, extra_attrs: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<score-partwise version=\"4.0\">\n  \
         <part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list>\n  \
         <part id=\"P1\">\n    <measure number=\"1\"><attributes><divisions>{div}</divisions>\
         <key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time>\
         {extra_attrs}</attributes>\n{medida}    </measure>\n  </part>\n</score-partwise>\n"
    )
}

fn nota_xml(passo: char, duracao: u32, tipo: &str, extra: &str) -> String {
    format!(
        "      <note><pitch><step>{passo}</step><octave>4</octave></pitch><duration>{duracao}\
         </duration><voice>1</voice><type>{tipo}</type>{extra}</note>\n"
    )
}

fn pausa_xml(duracao: u32, tipo: &str) -> String {
    format!(
        "      <note><rest/><duration>{duracao}</duration><voice>1</voice><type>{tipo}</type>\
         </note>\n"
    )
}

/// `[Ré q][pausa h][Ré q]`, 4/4.
fn re_pausa_re() -> String {
    peca(&format!(
        "{}{}{}",
        nota_xml('D', 1, "quarter", ""),
        pausa_xml(2, "half"),
        nota_xml('D', 1, "quarter", "")
    ))
}

fn pos(d: &DocumentoAberto) -> PosCompasso {
    PosCompasso {
        parte: d.documento().partes()[0].id.clone(),
        indice: 0,
        numero: "1".into(),
    }
}

fn inserir(
    d: &DocumentoAberto,
    posicao: Fracao,
    altura: Altura,
    figura: Figura,
    pontos: u8,
) -> ComandoDeEdicao {
    ComandoDeEdicao::InserirNota {
        compasso: pos(d),
        voz: "1".into(),
        pauta: "1".into(),
        posicao,
        altura,
        acidente: None,
        figura,
        pontos,
    }
}

fn inserir_pausa(d: &DocumentoAberto, posicao: Fracao, figura: Figura) -> ComandoDeEdicao {
    ComandoDeEdicao::InserirPausa {
        compasso: pos(d),
        voz: "1".into(),
        pauta: "1".into(),
        posicao,
        figura,
        pontos: 0,
    }
}

/// (posição, duração, é pausa, letra) das notas da parte 0, compasso 0, no modelo de tocar.
fn linha(d: &DocumentoAberto) -> Vec<(Fracao, Fracao, bool, Option<char>)> {
    d.partitura().partes[0].compassos[0]
        .notas
        .iter()
        .map(|n| {
            (
                n.posicao,
                n.duracao,
                n.e_pausa(),
                n.altura
                    .map(|a| ['C', 'D', 'E', 'F', 'G', 'A', 'B'][usize::from(a.passo)]),
            )
        })
        .collect()
}

// --- inserir nota -----------------------------------------------------------------------------

#[test]
fn inserir_nota_onde_ha_pausa_livre_a_substitui_e_preserva_a_soma_do_compasso() {
    let texto = re_pausa_re();
    let mut d = abrir(&texto);
    let somas = somas_por_voz(d.partitura());
    let antes = retrato(&d);
    // A posição vem da coluna do layout (o início da pausa no modelo de tocar), não de pixel.
    let coluna = d.partitura().partes[0].compassos[0].notas[1].posicao;
    assert_eq!(coluna, Fracao::inteiro(1));
    let r = d
        .aplicar(inserir(&d, coluna, alt('E', 4), Figura::Seminima, 0))
        .expect("insere");
    assert!(r.alterou);
    assert_eq!(r.compassos_afetados.len(), 1);
    // [Ré][Mi][pausa q][Ré]
    assert_eq!(
        linha(&d),
        vec![
            (f(0, 1), f(1, 1), false, Some('D')),
            (f(1, 1), f(1, 1), false, Some('E')),
            (f(2, 1), f(1, 1), true, None),
            (f(3, 1), f(1, 1), false, Some('D')),
        ]
    );
    assert_eq!(
        somas_por_voz(d.partitura()),
        somas,
        "a soma da voz não muda"
    );
    let e = r.estrutura.as_ref().expect("estrutura");
    assert!(e.alvo_soa);
    assert_eq!(d.ids().id(0, 0, 1), Some(e.alvo));
    assert_eq!(r.nota, e.alvo);
    assert_eq!(e.criadas.len(), 1, "a pausa que sobrou");
    assert_eq!(e.removidas.len(), 1, "a pausa de mínima que a nota ocupou");
    assert!(
        r.avisos
            .iter()
            .any(|a| matches!(a, AvisoDeEdicao::PausasConsumidas { .. }))
    );
    // O compasso continua único e o resto do arquivo, idêntico.
    let saida = d.exportar().expect("exporta");
    let rel = comparar_bytes(&texto, &saida).expect("compara");
    assert_eq!(rel.compassos_diferentes, r.compassos_afetados);
    verificar_golden(&texto, &saida, &r.compassos_afetados, None).expect("golden");
    assert_eq!(abrir(&saida).partitura(), d.partitura());
    d.conferir_exportacao(&saida).expect("confere");
    // A5: desfazer volta byte a byte; refazer volta ao editado.
    let editado = retrato(&d);
    d.desfazer().expect("desfaz");
    assert_eq!(retrato(&d), antes);
    assert_eq!(d.exportar().expect("exporta"), texto);
    d.refazer().expect("refaz");
    assert_eq!(retrato(&d), editado);
    d.desfazer().expect("desfaz de novo");
    assert_eq!(retrato(&d), antes);
}

#[test]
fn inserir_no_meio_da_pausa_deixa_pausa_antes_e_depois_com_a_duracao_exata() {
    // divisions 2: [Ré q][pausa h][Ré q]. A pausa vai de 1 a 3; uma colcheia no tempo 3/2 deixa
    // 1/2 antes e 1 depois.
    let texto = peca_com(
        2,
        &format!(
            "{}{}{}",
            nota_xml('D', 2, "quarter", ""),
            pausa_xml(4, "half"),
            nota_xml('D', 2, "quarter", "")
        ),
        "",
    );
    let mut d = abrir(&texto);
    let r = d
        .aplicar(inserir(&d, f(3, 2), alt('G', 4), Figura::Colcheia, 0))
        .expect("insere");
    assert_eq!(
        linha(&d),
        vec![
            (f(0, 1), f(1, 1), false, Some('D')),
            (f(1, 1), f(1, 2), true, None),
            (f(3, 2), f(1, 2), false, Some('G')),
            (f(2, 1), f(1, 1), true, None),
            (f(3, 1), f(1, 1), false, Some('D')),
        ]
    );
    let avisos: Vec<_> = r
        .avisos
        .iter()
        .filter(|a| matches!(a, AvisoDeEdicao::PausaCriada { .. }))
        .collect();
    assert_eq!(avisos.len(), 2, "uma pausa à esquerda e outra à direita");
    let saida = d.exportar().expect("exporta");
    assert_eq!(abrir(&saida).partitura(), d.partitura());
    d.desfazer().expect("desfaz");
    assert_eq!(d.exportar().expect("exporta"), texto);
}

#[test]
fn inserir_onde_nao_ha_espaco_e_recusado_com_mensagem_sem_mudar_nada() {
    let texto = re_pausa_re();
    let mut d = abrir(&texto);
    let antes = retrato(&d);
    // Em cima de uma nota (tempo 0): recusado.
    let e = d
        .aplicar(inserir(&d, Fracao::ZERO, alt('E', 4), Figura::Seminima, 0))
        .expect_err("ocupado");
    assert!(
        matches!(e, ErroDeEdicao::SemEspacoParaInserir { .. }),
        "{e:?}"
    );
    let msg = e.to_string();
    assert!(msg.contains("compasso 1"), "{msg}");
    assert!(msg.contains("Nada foi alterado"), "{msg}");
    // Figura que não cabe na pausa (semibreve no tempo 1): recusado.
    let e = d
        .aplicar(inserir(
            &d,
            Fracao::inteiro(1),
            alt('E', 4),
            Figura::Semibreve,
            0,
        ))
        .expect_err("não cabe");
    assert!(
        matches!(e, ErroDeEdicao::SemEspacoParaInserir { .. }),
        "{e:?}"
    );
    // Mínima pontuada também não cabe (3 > 2) e passaria da nota seguinte.
    let e = d
        .aplicar(inserir(
            &d,
            Fracao::inteiro(1),
            alt('E', 4),
            Figura::Minima,
            1,
        ))
        .expect_err("não cabe");
    assert!(
        matches!(e, ErroDeEdicao::SemEspacoParaInserir { .. }),
        "{e:?}"
    );
    // Depois do fim do compasso: posição inválida.
    let e = d
        .aplicar(inserir(
            &d,
            Fracao::inteiro(4),
            alt('E', 4),
            Figura::Seminima,
            0,
        ))
        .expect_err("fora");
    assert!(matches!(e, ErroDeEdicao::PosicaoInvalida { .. }), "{e:?}");
    assert!(!e.to_string().is_empty());
    // Nada mudou e o undo não ganhou entrada.
    assert_eq!(retrato(&d), antes);
    assert_eq!(d.entradas_de_desfazer(), 0);
    assert!(!d.pode_desfazer());
    assert!(!d.editado());
    assert!(!d.modificado());
    assert_eq!(d.exportar().expect("exporta"), texto);
}

#[test]
fn recusa_no_meio_do_historico_nao_cria_entrada_nem_descarta_o_refazer() {
    let mut d = abrir(&re_pausa_re());
    d.aplicar(inserir(
        &d,
        Fracao::inteiro(1),
        alt('E', 4),
        Figura::Seminima,
        0,
    ))
    .expect("insere");
    d.desfazer().expect("desfaz");
    let (desf, ref_) = (d.entradas_de_desfazer(), d.entradas_de_refazer());
    assert_eq!((desf, ref_), (0, 1));
    let antes = retrato(&d);
    d.aplicar(inserir(&d, Fracao::ZERO, alt('E', 4), Figura::Seminima, 0))
        .expect_err("ocupado");
    assert_eq!(
        (d.entradas_de_desfazer(), d.entradas_de_refazer()),
        (desf, ref_)
    );
    assert_eq!(retrato(&d), antes);
}

#[test]
fn inserir_com_a_nota_alterada_imprime_o_acidente_devido() {
    let mut d = abrir(&re_pausa_re());
    let fa_sustenido = Altura {
        passo: 'F',
        alter: 1,
        oitava: 4,
    };
    d.aplicar(inserir(
        &d,
        Fracao::inteiro(1),
        fa_sustenido,
        Figura::Seminima,
        0,
    ))
    .expect("insere");
    let n = &d.partitura().partes[0].compassos[0].notas[1];
    assert_eq!(n.altura.expect("altura").alteracao, 1);
    assert_eq!(
        n.acidente,
        Some(piano_tutor_core::partitura::Acidente::Sustenido),
        "o acidente devido é impresso"
    );
    let saida = d.exportar().expect("exporta");
    assert!(saida.contains("<accidental>sharp</accidental>"), "{saida}");
    assert_eq!(abrir(&saida).partitura(), d.partitura());
}

#[test]
fn inserir_numa_pausa_de_compasso_inteiro_a_divide_sem_deixar_a_marca_de_compasso() {
    let texto =
        peca("      <note><rest measure=\"yes\"/><duration>4</duration><voice>1</voice></note>\n");
    let mut d = abrir(&texto);
    let r = d
        .aplicar(inserir(
            &d,
            Fracao::inteiro(2),
            alt('C', 5),
            Figura::Minima,
            0,
        ))
        .expect("insere");
    assert!(r.alterou);
    assert_eq!(
        linha(&d),
        vec![
            (f(0, 1), f(2, 1), true, None),
            (f(2, 1), f(2, 1), false, Some('C')),
        ]
    );
    let saida = d.exportar().expect("exporta");
    assert!(!saida.contains("measure=\"yes\""), "{saida}");
    assert_eq!(abrir(&saida).partitura(), d.partitura());
    d.desfazer().expect("desfaz");
    assert_eq!(d.exportar().expect("exporta"), texto);
}

#[test]
fn inserir_numa_voz_secundaria_nao_mexe_na_outra_voz() {
    let texto = peca(
        "      <note><pitch><step>C</step><octave>5</octave></pitch><duration>4</duration>\
         <voice>1</voice><type>whole</type></note>\n      <backup><duration>4</duration></backup>\n      \
         <note><rest/><duration>4</duration><voice>2</voice><type>whole</type></note>\n",
    );
    let mut d = abrir(&texto);
    let somas = somas_por_voz(d.partitura());
    let c = ComandoDeEdicao::InserirNota {
        compasso: pos(&d),
        voz: "2".into(),
        pauta: "1".into(),
        posicao: Fracao::inteiro(1),
        altura: alt('E', 4),
        acidente: None,
        figura: Figura::Seminima,
        pontos: 0,
    };
    d.aplicar(c).expect("insere na voz 2");
    assert_eq!(somas_por_voz(d.partitura()), somas);
    let notas = &d.partitura().partes[0].compassos[0].notas;
    assert_eq!(notas[0].voz, 1);
    assert_eq!(notas[0].duracao, Fracao::inteiro(4));
    let da_voz_2: Vec<_> = notas.iter().filter(|n| n.voz == 2).collect();
    assert_eq!(da_voz_2.len(), 3);
    assert_eq!(da_voz_2[1].posicao, Fracao::inteiro(1));
    assert!(!da_voz_2[1].e_pausa());
    let saida = d.exportar().expect("exporta");
    assert_eq!(abrir(&saida).partitura(), d.partitura());
}

#[test]
fn inserir_numa_tercina_de_pausas_herda_a_quialtera_com_fracao_exata() {
    // divisions 3: semínima = 3, colcheia de tercina = 1. Uma tercina de colcheias em pausa.
    let tercina = "      <note><rest/><duration>1</duration><voice>1</voice><type>eighth</type>\
         <time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes>\
         </time-modification></note>\n";
    let texto = peca_com(
        3,
        &format!(
            "{}{}{}{}",
            tercina,
            tercina,
            tercina,
            "      <note><rest/><duration>9</duration><voice>1</voice><type>half</type>\
             <dot/></note>\n"
        ),
        "",
    );
    let mut d = abrir(&texto);
    let somas = somas_por_voz(d.partitura());
    // Insere uma colcheia (de tercina, pois as pausas ao redor são de tercina) no 2º terço.
    let r = d
        .aplicar(inserir(&d, f(1, 3), alt('E', 4), Figura::Colcheia, 0))
        .expect("insere");
    assert!(r.alterou);
    let n = &d.partitura().partes[0].compassos[0].notas[1];
    assert_eq!(n.duracao, f(1, 3));
    assert_eq!(n.tuplo, Some((3, 2)));
    assert_eq!(somas_por_voz(d.partitura()), somas);
    let saida = d.exportar().expect("exporta");
    assert_eq!(abrir(&saida).partitura(), d.partitura());
}

#[test]
fn inserir_quando_o_valor_nao_cabe_na_grade_de_tempo_e_erro_explicito() {
    // divisions 1: uma colcheia (1/2) não é inteira.
    let mut d = abrir(&re_pausa_re());
    let antes = retrato(&d);
    let e = d
        .aplicar(inserir(
            &d,
            Fracao::inteiro(1),
            alt('E', 4),
            Figura::Colcheia,
            0,
        ))
        .expect_err("divisions");
    assert!(
        matches!(e, ErroDeEdicao::DuracaoExigeMudarDivisions { .. }),
        "{e:?}"
    );
    assert_eq!(retrato(&d), antes);
    assert_eq!(d.entradas_de_desfazer(), 0);
}

// --- inserir pausa ----------------------------------------------------------------------------

#[test]
fn inserir_pausa_em_posicao_valida_e_aceito_e_regrupa_as_pausas() {
    let texto = re_pausa_re();
    let mut d = abrir(&texto);
    let somas = somas_por_voz(d.partitura());
    let antes = retrato(&d);
    let r = d
        .aplicar(inserir_pausa(&d, Fracao::inteiro(2), Figura::Seminima))
        .expect("pausa sobre pausa");
    assert!(r.alterou);
    assert_eq!(
        linha(&d),
        vec![
            (f(0, 1), f(1, 1), false, Some('D')),
            (f(1, 1), f(1, 1), true, None),
            (f(2, 1), f(1, 1), true, None),
            (f(3, 1), f(1, 1), false, Some('D')),
        ]
    );
    assert_eq!(somas_por_voz(d.partitura()), somas);
    assert!(!r.estrutura.as_ref().expect("estrutura").alvo_soa);
    assert_eq!(r.midi_depois(), None, "pausa não soa");
    d.desfazer().expect("desfaz");
    assert_eq!(retrato(&d), antes);
    d.refazer().expect("refaz");
    d.desfazer().expect("desfaz");
    assert_eq!(d.exportar().expect("exporta"), texto);
}

#[test]
fn inserir_pausa_sobre_nota_e_recusado_e_pede_a_remocao_explicita() {
    // Decisão registrada neste teste: pausa em cima de nota é recusada (a nota some só por
    // remoção explícita, que a transforma em pausa); nada é apagado em silêncio.
    let mut d = abrir(&re_pausa_re());
    let antes = retrato(&d);
    let e = d
        .aplicar(inserir_pausa(&d, Fracao::ZERO, Figura::Seminima))
        .expect_err("sobre nota");
    assert!(matches!(e, ErroDeEdicao::PausaSobreNota { .. }), "{e:?}");
    let msg = e.to_string();
    assert!(msg.contains("remova") || msg.contains("Remova"), "{msg}");
    assert_eq!(retrato(&d), antes);
    assert_eq!(d.entradas_de_desfazer(), 0);
}

// --- remover nota -----------------------------------------------------------------------------

#[test]
fn remover_uma_nota_a_transforma_em_pausa_de_mesma_duracao_e_posicao() {
    let texto = peca(&format!(
        "{}{}{}",
        nota_xml('C', 1, "quarter", ""),
        nota_xml('D', 1, "quarter", ""),
        nota_xml('E', 2, "half", "")
    ));
    let mut d = abrir(&texto);
    let antes = retrato(&d);
    let somas = somas_por_voz(d.partitura());
    let meio = d.documento().partes()[0].compassos[0].notas[1].id;
    let r = d
        .aplicar(ComandoDeEdicao::RemoverNota { nota: meio })
        .expect("remove");
    assert!(r.alterou);
    assert_eq!(r.nota, meio, "a identidade fica com a pausa");
    assert!(!r.estrutura.as_ref().expect("estrutura").alvo_soa);
    assert_eq!(r.midi_depois(), None);
    assert_eq!(
        linha(&d),
        vec![
            (f(0, 1), f(1, 1), false, Some('C')),
            (f(1, 1), f(1, 1), true, None),
            (f(2, 1), f(2, 1), false, Some('E')),
        ]
    );
    assert_eq!(d.ids().id(0, 0, 1), Some(meio));
    assert!(d.documento().nota(meio).expect("nota").pausa);
    assert_eq!(somas_por_voz(d.partitura()), somas);
    let saida = d.exportar().expect("exporta");
    assert!(saida.contains("<rest/>"), "{saida}");
    let rel = comparar_bytes(&texto, &saida).expect("compara");
    assert_eq!(rel.compassos_diferentes, r.compassos_afetados);
    verificar_golden(&texto, &saida, &r.compassos_afetados, None).expect("golden");
    assert_eq!(abrir(&saida).partitura(), d.partitura());
    d.conferir_exportacao(&saida).expect("confere");
    // A5.
    let editado = retrato(&d);
    d.desfazer().expect("desfaz");
    assert_eq!(retrato(&d), antes);
    assert_eq!(d.exportar().expect("exporta"), texto);
    d.refazer().expect("refaz");
    assert_eq!(retrato(&d), editado);
    // O inverso comandável devolve a nota (com identidade nova, como qualquer inserção).
    d.desfazer().expect("desfaz");
    let r2 = d
        .aplicar(ComandoDeEdicao::RemoverNota { nota: meio })
        .expect("remove");
    let volta = d.aplicar(r2.inverso.clone()).expect("inverso");
    assert!(volta.estrutura.as_ref().expect("estrutura").alvo_soa);
    let n = &d.partitura().partes[0].compassos[0].notas[1];
    assert_eq!(
        n.altura,
        antes.partitura.partes[0].compassos[0].notas[1].altura
    );
}

#[test]
fn remover_a_ultima_nota_de_uma_voz_deixa_pausa_e_a_soma_valida() {
    let texto = peca(&nota_xml('C', 4, "whole", ""));
    let mut d = abrir(&texto);
    let somas = somas_por_voz(d.partitura());
    let unica = d.documento().partes()[0].compassos[0].notas[0].id;
    d.aplicar(ComandoDeEdicao::RemoverNota { nota: unica })
        .expect("remove");
    assert_eq!(linha(&d), vec![(f(0, 1), f(4, 1), true, None)]);
    assert_eq!(somas_por_voz(d.partitura()), somas);
    let saida = d.exportar().expect("exporta");
    assert_eq!(abrir(&saida).partitura(), d.partitura());
    d.desfazer().expect("desfaz");
    assert_eq!(d.exportar().expect("exporta"), texto);
}

#[test]
fn remover_numa_voz_secundaria_so_toca_nela() {
    let texto = peca(
        "      <note><pitch><step>C</step><octave>5</octave></pitch><duration>4</duration>\
         <voice>1</voice><type>whole</type></note>\n      <backup><duration>4</duration></backup>\n      \
         <note><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration>\
         <voice>2</voice><type>whole</type></note>\n",
    );
    let mut d = abrir(&texto);
    let antes = d.partitura().partes[0].compassos[0].notas.clone();
    let v2 = d.documento().partes()[0].compassos[0].notas[1].id;
    d.aplicar(ComandoDeEdicao::RemoverNota { nota: v2 })
        .expect("remove");
    let depois = &d.partitura().partes[0].compassos[0].notas;
    assert_eq!(depois[0], antes[0], "a voz 1 não se move");
    assert!(depois[1].e_pausa());
    assert_eq!(
        (
            depois[1].voz,
            depois[1].pauta,
            depois[1].posicao,
            depois[1].duracao
        ),
        (
            antes[1].voz,
            antes[1].pauta,
            antes[1].posicao,
            antes[1].duracao
        )
    );
}

#[test]
fn remover_pausa_e_recusado_sem_mudar_nada() {
    let mut d = abrir(&re_pausa_re());
    let antes = retrato(&d);
    let pausa = d.documento().partes()[0].compassos[0].notas[1].id;
    let e = d
        .aplicar(ComandoDeEdicao::RemoverNota { nota: pausa })
        .expect_err("pausa");
    assert!(matches!(e, ErroDeEdicao::PausaNaoRemovivel(_)), "{e:?}");
    let e = d
        .aplicar(ComandoDeEdicao::RemoverNota { nota: IdNota(9999) })
        .expect_err("inexistente");
    assert!(matches!(e, ErroDeEdicao::NotaInexistente(_)), "{e:?}");
    assert_eq!(retrato(&d), antes);
    assert_eq!(d.entradas_de_desfazer(), 0);
}

#[test]
fn remover_nota_de_acorde_tira_so_ela_e_a_primeira_passa_o_chord_para_a_seguinte() {
    let texto = peca(
        "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration>\
         <voice>1</voice><type>whole</type></note>\n      <note><chord/><pitch><step>E</step>\
         <octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note>\n      \
         <note><chord/><pitch><step>G</step><octave>4</octave></pitch><duration>4</duration>\
         <voice>1</voice><type>whole</type></note>\n",
    );
    let mut d = abrir(&texto);
    let somas = somas_por_voz(d.partitura());
    let (c, e, g) = {
        let n = &d.documento().partes()[0].compassos[0].notas;
        (n[0].id, n[1].id, n[2].id)
    };
    // Tira a do meio: sobram C e G (G ainda é acorde).
    let r = d
        .aplicar(ComandoDeEdicao::RemoverNota { nota: e })
        .expect("remove Mi");
    assert_eq!(d.partitura().partes[0].compassos[0].notas.len(), 2);
    assert_eq!(d.ids().id(0, 0, 1), Some(g));
    assert_eq!(r.estrutura.as_ref().expect("estrutura").removidas, vec![e]);
    // Tira a principal: o G vira a nota principal (sem <chord/>), duração e soma intactas.
    d.aplicar(ComandoDeEdicao::RemoverNota { nota: c })
        .expect("remove Dó");
    let notas = &d.partitura().partes[0].compassos[0].notas;
    assert_eq!(notas.len(), 1);
    assert!(!notas[0].acorde);
    assert_eq!(notas[0].duracao, Fracao::inteiro(4));
    assert_eq!(somas_por_voz(d.partitura()), somas);
    let saida = d.exportar().expect("exporta");
    assert_eq!(abrir(&saida).partitura(), d.partitura());
    d.conferir_exportacao(&saida).expect("confere");
    // Tira a última do acorde: aí sim vira pausa.
    d.aplicar(ComandoDeEdicao::RemoverNota { nota: g })
        .expect("remove Sol");
    assert!(d.partitura().partes[0].compassos[0].notas[0].e_pausa());
    assert_eq!(somas_por_voz(d.partitura()), somas);
    // Desfaz as três.
    d.desfazer().expect("1");
    d.desfazer().expect("2");
    d.desfazer().expect("3");
    assert_eq!(d.exportar().expect("exporta"), texto);
}

#[test]
fn remover_nota_com_letra_dedilhado_ornamento_e_ligadura_avisa_um_a_um() {
    let extra = "<notations><tied type=\"start\"/><articulations><staccato/></articulations>\
         <ornaments><trill-mark/></ornaments><technical><fingering>3</fingering></technical>\
         <slur type=\"start\" number=\"1\"/></notations><lyric number=\"1\"><syllabic>single</syllabic>\
         <text>la</text></lyric>";
    let texto = peca(&format!(
        "{}{}",
        nota_xml('C', 2, "half", &format!("<tie type=\"start\"/>{extra}")),
        nota_xml('C', 2, "half", "<tie type=\"stop\"/>")
    ));
    let mut d = abrir(&texto);
    let primeira = d.documento().partes()[0].compassos[0].notas[0].id;
    let r = d
        .aplicar(ComandoDeEdicao::RemoverNota { nota: primeira })
        .expect("remove");
    let tem = |p: &dyn Fn(&AvisoDeEdicao) -> bool| r.avisos.iter().any(p);
    assert!(tem(&|a| matches!(
        a,
        AvisoDeEdicao::LetraRemovida { texto, .. } if texto.contains("la")
    )));
    assert!(tem(&|a| matches!(
        a,
        AvisoDeEdicao::DedilhadoRemovido { texto, .. } if texto == "3"
    )));
    assert!(tem(&|a| matches!(
        a,
        AvisoDeEdicao::OrnamentoRemovido { nome, .. } if nome == "trill-mark"
    )));
    assert!(tem(&|a| matches!(
        a,
        AvisoDeEdicao::OrnamentoRemovido { nome, .. } if nome == "staccato"
    )));
    assert!(tem(&|a| matches!(
        a,
        AvisoDeEdicao::LigaduraDeValorRemovida { .. }
    )));
    assert!(tem(&|a| matches!(
        a,
        AvisoDeEdicao::LigaduraDeExpressaoRemovida { .. }
    )));
    // Nenhum aviso é texto vazio.
    assert!(r.avisos.iter().all(|a| !a.texto().is_empty()));
    // A nota virou pausa limpa e o arquivo reabre igual.
    let saida = d.exportar().expect("exporta");
    assert!(!saida.contains("trill-mark"), "{saida}");
    assert!(!saida.contains("<lyric"), "{saida}");
    assert_eq!(abrir(&saida).partitura(), d.partitura());
    // A letra da nota removida deixou de aparecer na exibição.
    assert!(d.documento().exibicao().da_nota(primeira).is_none());
    // Desfazer devolve tudo, byte a byte.
    d.desfazer().expect("desfaz");
    assert_eq!(d.exportar().expect("exporta"), texto);
}

#[test]
fn remover_no_meio_de_um_grupo_de_vigas_refaz_as_vigas_e_avisa() {
    // Três colcheias vigadas; remover a do meio não pode deixar a viga aberta nem pausa vigada.
    let b = |passo: char, estado: &str| {
        nota_xml(
            passo,
            1,
            "eighth",
            &format!("<beam number=\"1\">{estado}</beam>"),
        )
    };
    let texto = peca_com(
        2,
        &format!(
            "{}{}{}{}{}",
            b('C', "begin"),
            b('D', "continue"),
            b('E', "end"),
            pausa_xml(1, "eighth"),
            pausa_xml(4, "half"),
        ),
        "",
    );
    let mut d = abrir(&texto);
    let meio = d.documento().partes()[0].compassos[0].notas[1].id;
    let r = d
        .aplicar(ComandoDeEdicao::RemoverNota { nota: meio })
        .expect("remove");
    assert!(r.alterou);
    assert!(
        r.avisos
            .iter()
            .any(|a| matches!(a, AvisoDeEdicao::VigasRefeitas { .. })),
        "{:?}",
        r.avisos
    );
    let notas = &d.partitura().partes[0].compassos[0].notas;
    assert!(notas[1].e_pausa());
    assert!(notas[1].vigas.is_empty(), "pausa não leva viga");
    let saida = d.exportar().expect("exporta");
    assert_eq!(abrir(&saida).partitura(), d.partitura());
    d.desfazer().expect("desfaz");
    assert_eq!(d.exportar().expect("exporta"), texto);
}

#[test]
fn remover_nota_de_tercina_mantem_a_marca_do_grupo_e_a_quialtera() {
    let tercina = |passo: char, notations: &str| {
        format!(
            "      <note><pitch><step>{passo}</step><octave>4</octave></pitch><duration>1</duration>\
             <voice>1</voice><type>eighth</type><time-modification><actual-notes>3</actual-notes>\
             <normal-notes>2</normal-notes></time-modification>{notations}</note>\n"
        )
    };
    let texto = peca_com(
        3,
        &format!(
            "{}{}{}{}",
            tercina(
                'C',
                "<notations><tuplet type=\"start\"/><articulations><staccato/></articulations>\
                 </notations>"
            ),
            tercina('D', ""),
            tercina('E', "<notations><tuplet type=\"stop\"/></notations>"),
            "      <note><rest/><duration>9</duration><voice>1</voice><type>half</type><dot/></note>\n"
        ),
        "",
    );
    let mut d = abrir(&texto);
    let primeira = d.documento().partes()[0].compassos[0].notas[0].id;
    let r = d
        .aplicar(ComandoDeEdicao::RemoverNota { nota: primeira })
        .expect("remove");
    assert!(
        r.avisos.iter().any(|a| matches!(
            a,
            AvisoDeEdicao::OrnamentoRemovido { nome, .. } if nome == "staccato"
        )),
        "{:?}",
        r.avisos
    );
    let saida = d.exportar().expect("exporta");
    assert!(saida.contains("<tuplet type=\"start\"/>"), "{saida}");
    assert!(!saida.contains("staccato"), "{saida}");
    let n = &d.partitura().partes[0].compassos[0].notas[0];
    assert!(n.e_pausa());
    assert_eq!(n.tuplo, Some((3, 2)));
    assert!(
        n.inicio_tuplo,
        "a marca de início do grupo continua na pausa"
    );
    assert_eq!(n.duracao, f(1, 3));
    assert_eq!(abrir(&saida).partitura(), d.partitura());
}

#[test]
fn varias_edicoes_de_estrutura_desfeitas_voltam_ao_original_byte_a_byte() {
    let texto = re_pausa_re();
    let mut d = abrir(&texto);
    let mut retratos = vec![retrato(&d)];
    // Ré, pausa(h), Ré: insere um Mi em 1, remove o Ré do começo, insere um Sol em 2.
    d.aplicar(inserir(
        &d,
        Fracao::inteiro(1),
        alt('E', 4),
        Figura::Seminima,
        0,
    ))
    .expect("1");
    retratos.push(retrato(&d));
    let primeiro = d.documento().partes()[0].compassos[0].notas[0].id;
    d.aplicar(ComandoDeEdicao::RemoverNota { nota: primeiro })
        .expect("2");
    retratos.push(retrato(&d));
    d.aplicar(inserir(
        &d,
        Fracao::inteiro(2),
        alt('G', 4),
        Figura::Seminima,
        0,
    ))
    .expect("3");
    retratos.push(retrato(&d));
    assert_eq!(d.entradas_de_desfazer(), 3);
    // A conferência completa vale no estado final.
    d.conferir_exportacao(&d.exportar().expect("exporta"))
        .expect("confere");
    // Desfaz tudo, um a um, voltando a cada retrato anterior.
    for esperado in retratos.iter().rev().skip(1) {
        d.desfazer().expect("desfaz");
        assert!(&retrato(&d) == esperado, "retrato diferente ao desfazer");
    }
    assert_eq!(d.exportar().expect("exporta"), texto);
    assert!(!d.pode_desfazer());
    // Refaz tudo, de volta ao último.
    for esperado in retratos.iter().skip(1) {
        d.refazer().expect("refaz");
        assert!(&retrato(&d) == esperado, "retrato diferente ao refazer");
    }
}

// --- identidades, varredura do corpus ----------------------------------------------------------

#[test]
fn identidades_sobrevivem_a_insercao_e_remocao_pelo_arquivo_lateral() {
    let texto = re_pausa_re();
    let mut d = abrir(&texto);
    d.aplicar(inserir(
        &d,
        Fracao::inteiro(1),
        alt('E', 4),
        Figura::Seminima,
        0,
    ))
    .expect("insere");
    let alvo = d.documento().partes()[0].compassos[0].notas[0].id;
    d.aplicar(ComandoDeEdicao::RemoverNota { nota: alvo })
        .expect("remove");
    let saida = d.exportar().expect("exporta");
    let lateral = TabelaIds::de_texto(&d.tabela_ids_para(&saida).para_texto()).expect("lê");
    let r = DocumentoAberto::abrir_texto_com_ids("t", &saida, Some(&lateral)).expect("reabre");
    assert!(
        d.documento()
            .notas()
            .map(|n| n.id)
            .eq(r.documento().notas().map(|n| n.id))
    );
    d.conferir_exportacao(&saida).expect("confere");
}

fn amostra_de_pausas(d: &DocumentoAberto, max: usize) -> Vec<(usize, usize, Fracao, Fracao)> {
    // (parte do documento, compasso, posição, duração) das pausas do primeiro nível.
    let mut v = Vec::new();
    for (pi, parte) in d.partitura().partes.iter().enumerate() {
        for (ci, c) in parte.compassos.iter().enumerate() {
            for n in &c.notas {
                if n.e_pausa() && !n.acorde && !n.adorno && n.duracao > Fracao::ZERO {
                    v.push((pi, ci, n.posicao, n.duracao));
                }
            }
        }
    }
    let passo = v.len().div_ceil(max).max(1);
    v.into_iter().step_by(passo).collect()
}

fn varre_insercao(fx: &Fixture, max: usize) -> (usize, usize) {
    let mut d = DocumentoAberto::abrir_texto(fx.id, &fx.conteudo).expect("abre");
    let somas = somas_por_voz(d.partitura());
    let (mut aceitas, mut recusadas) = (0, 0);
    for (pi, ci, posicao, _) in amostra_de_pausas(&d, max) {
        let comp = PosCompasso {
            parte: d.documento().partes()[pi].id.clone(),
            indice: ci,
            numero: String::new(),
        };
        let (voz_base, pauta_base) = {
            let n = d.partitura().partes[pi].compassos[ci]
                .notas
                .iter()
                .find(|n| n.e_pausa() && n.posicao == posicao)
                .expect("pausa");
            (n.voz.to_string(), n.pauta.to_string())
        };
        for figura in [Figura::Seminima, Figura::Minima, Figura::Semibreve] {
            let cmd = ComandoDeEdicao::InserirNota {
                compasso: comp.clone(),
                voz: voz_base.clone(),
                pauta: pauta_base.clone(),
                posicao,
                altura: alt('E', 4),
                acidente: None,
                figura,
                pontos: 0,
            };
            let antes = retrato(&d);
            let entradas = d.entradas_de_desfazer();
            match d.aplicar(cmd.clone()) {
                Err(e) => {
                    recusadas += 1;
                    assert!(!e.to_string().is_empty());
                    assert_eq!(retrato(&d), antes, "{}: {cmd:?}: recusa mexeu", fx.id);
                    assert_eq!(d.entradas_de_desfazer(), entradas, "{}", fx.id);
                }
                Ok(r) => {
                    aceitas += 1;
                    assert!(r.alterou, "{}", fx.id);
                    assert_eq!(d.entradas_de_desfazer(), entradas + 1, "{}", fx.id);
                    assert_eq!(
                        somas_por_voz(d.partitura()),
                        somas,
                        "{}: {cmd:?}: soma mudou",
                        fx.id
                    );
                    let saida = d.exportar().expect("exporta");
                    let rel = comparar_bytes(&fx.conteudo, &saida).expect("compara");
                    assert_eq!(rel.compassos_diferentes, r.compassos_afetados, "{}", fx.id);
                    assert!(!rel.fora_dos_compassos, "{}", fx.id);
                    verificar_golden(&fx.conteudo, &saida, &r.compassos_afetados, None)
                        .unwrap_or_else(|v| panic!("{}: golden: {v:?}", fx.id));
                    let reaberto = DocumentoAberto::abrir_texto(fx.id, &saida).expect("reabre");
                    assert_eq!(reaberto.partitura(), d.partitura(), "{}: {cmd:?}", fx.id);
                    d.conferir_exportacao(&saida)
                        .unwrap_or_else(|e| panic!("{}: conferência: {e}", fx.id));
                    // A5: desfazer volta ao estado de antes, byte a byte.
                    d.desfazer().expect("desfaz");
                    assert_eq!(retrato(&d), antes, "{}: {cmd:?}: desfazer", fx.id);
                    assert_eq!(d.exportar().expect("exporta"), fx.conteudo, "{}", fx.id);
                }
            }
        }
    }
    (aceitas, recusadas)
}

#[test]
fn corpus_inteiro_inserir_nota_em_pausas_fecha_o_compasso_e_desfaz_byte_a_byte() {
    let (mut aceitas, mut recusadas) = (0, 0);
    for fx in corpus() {
        let (a, r) = varre_insercao(&fx, 12);
        aceitas += a;
        recusadas += r;
    }
    assert!(
        aceitas >= 10,
        "a varredura precisa aceitar casos reais: {aceitas}"
    );
    assert!(recusadas > 0, "e recusar alguns: {recusadas}");
}

fn varre_remocao(fx: &Fixture, max: usize) -> usize {
    let mut d = DocumentoAberto::abrir_texto(fx.id, &fx.conteudo).expect("abre");
    let somas = somas_por_voz(d.partitura());
    let todas: Vec<IdNota> = d
        .documento()
        .notas()
        .filter(|n| n.altura.is_some())
        .map(|n| n.id)
        .collect();
    let passo = todas.len().div_ceil(max).max(1);
    let mut feitas = 0;
    for id in todas.into_iter().step_by(passo) {
        let antes = retrato(&d);
        let entradas = d.entradas_de_desfazer();
        let local = d.documento().posicao_da_nota(id).expect("local");
        match d.aplicar(ComandoDeEdicao::RemoverNota { nota: id }) {
            Err(e) => panic!("{}: remover {id:?}: {e}", fx.id),
            Ok(r) => {
                feitas += 1;
                assert!(r.alterou);
                assert_eq!(d.entradas_de_desfazer(), entradas + 1);
                assert_eq!(somas_por_voz(d.partitura()), somas, "{}: soma", fx.id);
                assert_eq!(r.compassos_afetados, vec![local.clone()], "{}", fx.id);
                // Propriedade: só o compasso editado muda; nele, as outras notas ficam onde
                // estavam (posição, duração, voz, pauta, altura).
                let p0 = &antes.partitura;
                let p1 = d.partitura();
                for (pi, parte) in p0.partes.iter().enumerate() {
                    for (ci, c) in parte.compassos.iter().enumerate() {
                        let c1 = &p1.partes[pi].compassos[ci];
                        let alvo =
                            d.documento().partes()[pi].id == local.parte && ci == local.indice;
                        if !alvo {
                            assert_eq!(c, c1, "{}: compasso {ci} mudou", fx.id);
                            continue;
                        }
                        for (k, n0) in c.notas.iter().enumerate() {
                            let ident = antes.ids[ids_ate(p0, pi, ci) + k];
                            if ident == Some(id) {
                                continue;
                            }
                            // Pode ter sumido um membro de acorde: procura pela identidade.
                            let pos1 =
                                (0..c1.notas.len()).find(|j| d.ids().id(pi, ci, *j) == ident);
                            if let Some(j) = pos1 {
                                let n1 = &c1.notas[j];
                                assert_eq!(
                                    (n1.posicao, n1.duracao, n1.voz, n1.pauta, n1.altura),
                                    (n0.posicao, n0.duracao, n0.voz, n0.pauta, n0.altura),
                                    "{}: nota vizinha se moveu",
                                    fx.id
                                );
                            }
                        }
                    }
                }
                let saida = d.exportar().expect("exporta");
                let rel = comparar_bytes(&fx.conteudo, &saida).expect("compara");
                assert_eq!(rel.compassos_diferentes, r.compassos_afetados, "{}", fx.id);
                verificar_golden(&fx.conteudo, &saida, &r.compassos_afetados, None)
                    .unwrap_or_else(|v| panic!("{}: golden: {v:?}", fx.id));
                assert_eq!(
                    DocumentoAberto::abrir_texto(fx.id, &saida)
                        .expect("reabre")
                        .partitura(),
                    d.partitura(),
                    "{}",
                    fx.id
                );
                d.conferir_exportacao(&saida)
                    .unwrap_or_else(|e| panic!("{}: conferência: {e}", fx.id));
                d.desfazer().expect("desfaz");
                assert_eq!(retrato(&d), antes, "{}: desfazer", fx.id);
                assert_eq!(d.exportar().expect("exporta"), fx.conteudo, "{}", fx.id);
                d.refazer().expect("refaz");
                d.desfazer().expect("desfaz");
            }
        }
    }
    feitas
}

/// Quantas notas (identidades) vêm antes do compasso `ci` da parte `pi` no retrato.
fn ids_ate(p: &Partitura, pi: usize, ci: usize) -> usize {
    let mut n = 0;
    for (i, parte) in p.partes.iter().enumerate() {
        for (j, c) in parte.compassos.iter().enumerate() {
            if (i, j) == (pi, ci) {
                return n;
            }
            n += c.notas.len();
        }
    }
    n
}

#[test]
fn corpus_inteiro_remover_nota_mantem_a_soma_move_so_o_compasso_e_desfaz_byte_a_byte() {
    let mut total = 0;
    for fx in corpus() {
        total += varre_remocao(&fx, 12);
    }
    assert!(total > 30, "{total}");
}
