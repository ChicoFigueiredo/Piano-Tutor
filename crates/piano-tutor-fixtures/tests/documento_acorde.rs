//! Ticket 18 (E01): adicionar e retirar alturas de um acorde no serviço de documento (sem janela
//! e sem teclado). Cobre o `<chord/>` na exportação (comparação semântica e byte a byte fora do
//! compasso editado), a soma do compasso (A4), desfazer e refazer byte a byte (A5), a recusa sem
//! rastro (altura repetida, pausa, limites), as identidades, ligadura e dedilhado por altura e a
//! varredura do corpus inteiro.

use piano_tutor_core::documento::{
    Altura, AvisoDeEdicao, ComandoDeEdicao, DocumentoAberto, ErroDeEdicao, IdNota, Nota,
    PosCompasso,
};
use piano_tutor_core::documento::{SentidoDaAltura, UnidadeDeAltura};
use piano_tutor_core::partitura::{Figura, Fracao, Partitura};
use piano_tutor_fixtures::corpus::{Fixture, corpus};
use piano_tutor_fixtures::harness::{comparar_bytes, verificar_golden};
use std::collections::BTreeMap;

fn abrir(texto: &str) -> DocumentoAberto {
    DocumentoAberto::abrir_texto("t", texto).expect("abre")
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

fn peca_com(fifths: i8, medida: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<score-partwise version=\"4.0\">\n  \
         <part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list>\n  \
         <part id=\"P1\">\n    <measure number=\"1\"><attributes><divisions>1</divisions>\
         <key><fifths>{fifths}</fifths></key><time><beats>4</beats><beat-type>4</beat-type>\
         </time></attributes>\n{medida}    </measure>\n    <measure number=\"2\">\n      \
         <note><pitch><step>A</step><octave>4</octave></pitch><duration>4</duration>\
         <voice>1</voice><type>whole</type></note>\n    </measure>\n  </part>\n</score-partwise>\n"
    )
}

fn peca(medida: &str) -> String {
    peca_com(0, medida)
}

fn nota_xml(passo: char, oitava: i8, duracao: u32, tipo: &str, extra: &str) -> String {
    format!(
        "      <note><pitch><step>{passo}</step><octave>{oitava}</octave></pitch>\
         <duration>{duracao}</duration><voice>1</voice><type>{tipo}</type>{extra}</note>\n"
    )
}

fn pausa_xml(duracao: u32, tipo: &str) -> String {
    format!(
        "      <note><rest/><duration>{duracao}</duration><voice>1</voice><type>{tipo}</type>\
         </note>\n"
    )
}

/// `[Dó4 q][Ré4 q][Mi4 h]`.
fn do_re_mi() -> String {
    peca(&format!(
        "{}{}{}",
        nota_xml('C', 4, 1, "quarter", ""),
        nota_xml('D', 4, 1, "quarter", ""),
        nota_xml('E', 4, 2, "half", "")
    ))
}

/// Acorde `[Dó4 Mi4 Sol4]` de semibreve.
fn acorde_de_tres() -> String {
    peca(
        "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration>\
         <voice>1</voice><type>whole</type></note>\n      <note><chord/><pitch><step>E</step>\
         <octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note>\n      \
         <note><chord/><pitch><step>G</step><octave>4</octave></pitch><duration>4</duration>\
         <voice>1</voice><type>whole</type></note>\n",
    )
}

fn pos(d: &DocumentoAberto) -> PosCompasso {
    PosCompasso {
        parte: d.documento().partes()[0].id.clone(),
        indice: 0,
        numero: "1".into(),
    }
}

fn id_da(d: &DocumentoAberto, ordinal: usize) -> IdNota {
    d.documento().partes()[0].compassos[0].notas[ordinal].id
}

/// (posição, duração, voz, pauta, é acorde, nome do passo+alter+oitava) do compasso 0.
fn linha(d: &DocumentoAberto) -> Vec<(Fracao, Fracao, u8, u8, bool, String)> {
    d.partitura().partes[0].compassos[0]
        .notas
        .iter()
        .map(|n| {
            (
                n.posicao,
                n.duracao,
                n.voz,
                n.pauta,
                n.acorde,
                n.midi.map_or_else(|| "pausa".to_owned(), |m| m.to_string()),
            )
        })
        .collect()
}

fn adicionar(nota: IdNota, altura: Altura) -> ComandoDeEdicao {
    ComandoDeEdicao::AdicionarAltura {
        nota,
        altura,
        acidente: None,
    }
}

// --- acrescentar ------------------------------------------------------------------------------

#[test]
fn acrescentar_altura_a_uma_nota_forma_acorde_com_mesma_posicao_duracao_voz_e_pauta() {
    let texto = do_re_mi();
    let mut d = abrir(&texto);
    let somas = somas_por_voz(d.partitura());
    let antes = retrato(&d);
    let re = id_da(&d, 1);
    let r = d.aplicar(adicionar(re, alt('A', 4))).expect("acrescenta");
    assert!(r.alterou);
    assert_eq!(r.nota, re, "a seleção e o foco ficam na nota selecionada");
    let est = r.estrutura.as_ref().expect("estrutura");
    assert!(est.alvo_soa);
    assert_ne!(est.alvo, re);
    assert_eq!(r.midi_depois(), Some(69));
    assert_eq!(r.compassos_afetados, vec![pos(&d)]);
    // Ré4 (q) + Lá4 (acorde): mesma posição e duração; o resto do compasso não se mexe.
    let l = linha(&d);
    assert_eq!(l.len(), 4);
    assert_eq!(l[1].0, l[2].0);
    assert_eq!(l[1].1, l[2].1);
    assert_eq!((l[2].2, l[2].3, l[2].4), (l[1].2, l[1].3, true));
    assert_eq!(l[2].5, "69");
    assert_eq!(somas_por_voz(d.partitura()), somas);
    // Identidades: as antigas ficam, a nova é nova.
    assert_eq!(
        d.documento().nota(re).expect("Ré").altura,
        Some(alt('D', 4))
    );
    assert_eq!(d.ids().id(0, 0, 2), Some(est.alvo));
    let saida = d.exportar().expect("exporta");
    assert!(saida.contains("<chord/>"), "{saida}");
    // O compasso 2 e tudo fora do compasso editado saem byte a byte iguais.
    let rel = comparar_bytes(&texto, &saida).expect("compara");
    assert_eq!(rel.compassos_diferentes, r.compassos_afetados);
    assert!(!rel.fora_dos_compassos);
    // Comparação semântica contra o resultado esperado, escrito à mão.
    let esperado = peca(&format!(
        "{}{}{}{}",
        nota_xml('C', 4, 1, "quarter", ""),
        nota_xml('D', 4, 1, "quarter", ""),
        "      <note><chord/><pitch><step>A</step><octave>4</octave></pitch><duration>1</duration>\
         <voice>1</voice><type>quarter</type></note>\n",
        nota_xml('E', 4, 2, "half", "")
    ));
    verificar_golden(&texto, &saida, &r.compassos_afetados, Some(&esperado)).expect("golden");
    assert_eq!(abrir(&saida).partitura(), d.partitura());
    d.conferir_exportacao(&saida).expect("confere");
    // A5.
    let editado = retrato(&d);
    d.desfazer().expect("desfaz");
    assert_eq!(retrato(&d), antes);
    assert_eq!(d.exportar().expect("exporta"), texto);
    d.refazer().expect("refaz");
    assert_eq!(retrato(&d), editado);
    // O inverso comandável tira a altura nova.
    let mut d2 = abrir(&texto);
    let re2 = id_da(&d2, 1);
    let r2 = d2.aplicar(adicionar(re2, alt('A', 4))).expect("acrescenta");
    d2.aplicar(r2.inverso).expect("retira");
    assert_eq!(linha(&d2), linha(&abrir(&texto)));
    assert_eq!(somas_por_voz(d2.partitura()), somas);
}

#[test]
fn acrescentar_a_um_acorde_qualquer_membro_pode_ser_a_selecionada_e_a_ordem_e_crescente() {
    let texto = acorde_de_tres();
    let mut d = abrir(&texto);
    let (c, g) = (id_da(&d, 0), id_da(&d, 2));
    // Sol4 selecionado, acrescenta Ré4: fica entre Dó e Mi? Não: vai depois das menores em ordem.
    d.aplicar(adicionar(g, alt('D', 4))).expect("acrescenta");
    let midis: Vec<String> = linha(&d).into_iter().map(|l| l.5).collect();
    assert_eq!(midis, ["60", "62", "64", "67"]);
    // A principal continua sendo Dó, a única sem `<chord/>`.
    assert_eq!(d.documento().nota(c).map(|n| n.acorde), Some(false));
    assert!(linha(&d).iter().skip(1).all(|l| l.4));
    // Acima da última.
    d.aplicar(adicionar(c, alt('B', 4))).expect("acrescenta");
    let midis: Vec<String> = linha(&d).into_iter().map(|l| l.5).collect();
    assert_eq!(midis, ["60", "62", "64", "67", "71"]);
    // Abaixo da principal: a principal não troca de lugar.
    d.aplicar(adicionar(c, alt('G', 3))).expect("acrescenta");
    let midis: Vec<String> = linha(&d).into_iter().map(|l| l.5).collect();
    assert_eq!(midis[0], "60");
    assert!(midis.contains(&"55".to_owned()));
    assert_eq!(d.documento().nota(c).map(|n| n.acorde), Some(false));
    let saida = d.exportar().expect("exporta");
    d.conferir_exportacao(&saida).expect("confere");
    assert_eq!(saida.matches("<chord/>").count(), 5);
}

#[test]
fn a_tecla_shift_letra_resolve_na_oitava_mais_proxima_e_e_igual_ao_comando_resolvido() {
    // Dó4 selecionado: Sol fica abaixo (Sol3, 5 semitons) e Mi, acima (Mi4, 4).
    let casos = [
        ('G', alt('G', 3)),
        ('E', alt('E', 4)),
        ('F', alt('F', 4)),
        ('A', alt('A', 3)),
        ('B', alt('B', 3)),
        // A própria nota: oitava acima.
        ('C', alt('C', 5)),
    ];
    for (letra, esperada) in casos {
        let texto = do_re_mi();
        let mut por_tecla = abrir(&texto);
        let mut por_comando = abrir(&texto);
        let do_ = id_da(&por_tecla, 0);
        let r = por_tecla
            .aplicar(ComandoDeEdicao::AcrescentarLetra { nota: do_, letra })
            .expect("tecla");
        let ComandoDeEdicao::AdicionarAltura { altura, .. } = r.aplicado.clone() else {
            panic!("o aplicado deve ser a forma resolvida: {:?}", r.aplicado);
        };
        assert_eq!(altura, esperada, "Shift+{letra}");
        por_comando
            .aplicar(r.aplicado.clone())
            .expect("comando resolvido");
        assert_eq!(
            retrato(&por_tecla),
            retrato(&por_comando),
            "Shift+{letra} difere do comando equivalente"
        );
    }
    // Empate (Si3 para Fá: Fá3 e Fá4 a 6 semitons): a de cima.
    let texto = peca(&nota_xml('B', 3, 4, "whole", ""));
    let mut d = abrir(&texto);
    let b = id_da(&d, 0);
    let r = d
        .aplicar(ComandoDeEdicao::AcrescentarLetra {
            nota: b,
            letra: 'F',
        })
        .expect("acrescenta");
    assert_eq!(r.depois, alt('F', 4));
}

#[test]
fn a_letra_leva_a_alteracao_da_armadura_sem_acidente_impresso() {
    // Sol maior: Fá é sustenido pela armadura.
    let texto = peca_com(1, &nota_xml('D', 4, 4, "whole", ""));
    let mut d = abrir(&texto);
    let re = id_da(&d, 0);
    let r = d
        .aplicar(ComandoDeEdicao::AcrescentarLetra {
            nota: re,
            letra: 'F',
        })
        .expect("acrescenta");
    assert_eq!(
        r.depois,
        Altura {
            passo: 'F',
            alter: 1,
            oitava: 4
        }
    );
    let saida = d.exportar().expect("exporta");
    assert!(!saida.contains("<accidental>"), "{saida}");
    // Altura alterada contra a armadura leva o acidente devido.
    let mut d2 = abrir(&peca(&nota_xml('D', 4, 4, "whole", "")));
    let re = id_da(&d2, 0);
    d2.aplicar(ComandoDeEdicao::AdicionarAltura {
        nota: re,
        altura: Altura {
            passo: 'F',
            alter: 1,
            oitava: 4,
        },
        acidente: None,
    })
    .expect("acrescenta");
    assert!(
        d2.exportar()
            .expect("exporta")
            .contains("<accidental>sharp")
    );
    d2.conferir_exportacao(&d2.exportar().expect("exporta"))
        .expect("confere");
}

#[test]
fn intervalo_acima_segue_a_escala_da_armadura() {
    let casos: [(i8, char, u8, Altura); 4] = [
        (0, 'C', 3, alt('E', 4)),
        (0, 'C', 8, alt('C', 5)),
        (0, 'C', 9, alt('D', 5)),
        // Sol maior: a terça acima de Ré é Fá sustenido.
        (
            1,
            'D',
            3,
            Altura {
                passo: 'F',
                alter: 1,
                oitava: 4,
            },
        ),
    ];
    for (fifths, passo, graus, esperada) in casos {
        let mut d = abrir(&peca_com(fifths, &nota_xml(passo, 4, 4, "whole", "")));
        let n = id_da(&d, 0);
        let r = d
            .aplicar(ComandoDeEdicao::AcrescentarIntervalo { nota: n, graus })
            .expect("intervalo");
        assert_eq!(r.depois, esperada, "intervalo {graus} sobre {passo}4");
        let saida = d.exportar().expect("exporta");
        d.conferir_exportacao(&saida).expect("confere");
    }
    // Fora de 2 a 9: recusa com mensagem.
    let mut d = abrir(&do_re_mi());
    let antes = retrato(&d);
    for graus in [0u8, 1, 10] {
        let e = d
            .aplicar(ComandoDeEdicao::AcrescentarIntervalo {
                nota: id_da(&d, 0),
                graus,
            })
            .expect_err("recusa");
        assert_eq!(e, ErroDeEdicao::IntervaloInvalido(graus));
        assert!(!e.to_string().is_empty());
    }
    assert_eq!(retrato(&d), antes);
    assert_eq!(d.entradas_de_desfazer(), 0);
}

// --- recusas ----------------------------------------------------------------------------------

#[test]
fn altura_repetida_pausa_adorno_e_limites_sao_recusados_sem_rastro() {
    let texto = acorde_de_tres();
    let mut d = abrir(&texto);
    d.aplicar(adicionar(id_da(&d, 0), alt('A', 4))).expect("ok");
    d.desfazer().expect("desfaz");
    d.aplicar(adicionar(id_da(&d, 0), alt('A', 4))).expect("ok");
    let antes = retrato(&d);
    let c = id_da(&d, 0);
    // Altura repetida (Mi4 já soa), também grafada de outro jeito (Fá bemol 4 = Mi4).
    let e = d.aplicar(adicionar(c, alt('E', 4))).expect_err("repetida");
    assert!(matches!(e, ErroDeEdicao::AlturaJaNoAcorde { .. }), "{e:?}");
    assert!(e.to_string().contains("já está no acorde"), "{e}");
    let enarmonica = Altura {
        passo: 'F',
        alter: -1,
        oitava: 4,
    };
    assert!(matches!(
        d.aplicar(adicionar(c, enarmonica)),
        Err(ErroDeEdicao::AlturaJaNoAcorde { .. })
    ));
    // A mesma altura pedida pela tecla (Shift+Dó sobre a própria Dó dá Dó5; repetido depois).
    d.aplicar(ComandoDeEdicao::AcrescentarLetra {
        nota: c,
        letra: 'C',
    })
    .expect("Dó5");
    let antes2 = retrato(&d);
    let e = d
        .aplicar(ComandoDeEdicao::AcrescentarLetra {
            nota: c,
            letra: 'C',
        })
        .expect_err("Dó5 de novo");
    assert!(matches!(e, ErroDeEdicao::AlturaJaNoAcorde { .. }), "{e:?}");
    assert_eq!(retrato(&d), antes2);
    d.desfazer().expect("desfaz o Dó5");
    assert_eq!(retrato(&d), antes);
    // Limites do piano de 88 teclas: Dó8 (108) vale; Dó♯8 (109) e Sol♯0 (20) não.
    d.aplicar(adicionar(c, alt('C', 8))).expect("Dó8 vale");
    d.desfazer().expect("desfaz");
    d.aplicar(adicionar(c, alt('A', 0))).expect("Lá0 vale");
    d.desfazer().expect("desfaz");
    let (entradas, refazer) = (d.entradas_de_desfazer(), d.entradas_de_refazer());
    assert_eq!(
        refazer, 1,
        "o ramo de refazer existe e a recusa não o descarta"
    );
    for fora in [
        Altura {
            passo: 'C',
            alter: 1,
            oitava: 8,
        },
        Altura {
            passo: 'G',
            alter: 1,
            oitava: 0,
        },
    ] {
        let e = d.aplicar(adicionar(c, fora)).expect_err("fora dos limites");
        assert!(matches!(e, ErroDeEdicao::ForaDosLimites { .. }), "{e:?}");
    }
    assert!(matches!(
        d.aplicar(adicionar(
            c,
            Altura {
                passo: 'H',
                alter: 0,
                oitava: 4
            }
        )),
        Err(ErroDeEdicao::Documento(_))
    ));
    // Letra que não existe.
    assert!(
        d.aplicar(ComandoDeEdicao::AcrescentarLetra {
            nota: c,
            letra: 'X'
        })
        .is_err()
    );
    // Nota que não existe.
    assert!(matches!(
        d.aplicar(adicionar(IdNota(9999), alt('A', 4))),
        Err(ErroDeEdicao::NotaInexistente(_))
    ));
    assert_eq!(retrato(&d), antes);
    assert_eq!(d.entradas_de_desfazer(), entradas);
    assert_eq!(d.entradas_de_refazer(), refazer);

    // Pausa e nota de adorno não formam acorde.
    let com_pausa = peca(&format!(
        "{}{}",
        pausa_xml(2, "half"),
        nota_xml('C', 4, 2, "half", "")
    ));
    let mut p = abrir(&com_pausa);
    let pausa = id_da(&p, 0);
    let a0 = retrato(&p);
    let e = p.aplicar(adicionar(pausa, alt('A', 4))).expect_err("pausa");
    assert_eq!(e, ErroDeEdicao::AcordeSoEmNota(pausa));
    assert!(e.to_string().contains("Só uma nota"), "{e}");
    assert!(matches!(
        p.aplicar(ComandoDeEdicao::AcrescentarLetra {
            nota: pausa,
            letra: 'C'
        }),
        Err(ErroDeEdicao::AcordeSoEmNota(_))
    ));
    assert_eq!(retrato(&p), a0);
    let com_adorno = peca(&format!(
        "      <note><grace/><pitch><step>B</step><octave>3</octave></pitch><voice>1</voice>\
         <type>eighth</type></note>\n{}",
        nota_xml('C', 4, 4, "whole", "")
    ));
    let mut g = abrir(&com_adorno);
    let adorno = id_da(&g, 0);
    assert_eq!(
        g.aplicar(adicionar(adorno, alt('D', 4))),
        Err(ErroDeEdicao::AcordeSoEmNota(adorno))
    );
}

// --- retirar ----------------------------------------------------------------------------------

#[test]
fn retirar_altura_de_acorde_mantem_as_demais_e_a_soma_e_desfaz_byte_a_byte() {
    let texto = acorde_de_tres();
    let mut d = abrir(&texto);
    let somas = somas_por_voz(d.partitura());
    let (c, e, g) = (id_da(&d, 0), id_da(&d, 1), id_da(&d, 2));
    let r = d
        .aplicar(ComandoDeEdicao::RetirarAltura { nota: e })
        .expect("retira Mi");
    assert_eq!(r.aplicado, ComandoDeEdicao::RetirarAltura { nota: e });
    assert!(matches!(
        r.inverso,
        ComandoDeEdicao::AdicionarAltura { altura, .. } if altura == alt('E', 4)
    ));
    assert_eq!(r.estrutura.as_ref().expect("estrutura").removidas, vec![e]);
    assert_eq!(linha(&d).len(), 2);
    assert_eq!(d.ids().id(0, 0, 1), Some(g));
    assert_eq!(somas_por_voz(d.partitura()), somas);
    let saida = d.exportar().expect("exporta");
    d.conferir_exportacao(&saida).expect("confere");
    // Retira a principal: Sol passa a ser a principal.
    d.aplicar(ComandoDeEdicao::RetirarAltura { nota: c })
        .expect("retira Dó");
    let l = linha(&d);
    assert_eq!(l.len(), 1);
    assert!(!l[0].4);
    assert_eq!(somas_por_voz(d.partitura()), somas);
    // A última altura: equivale à remoção do ticket 17 (vira pausa).
    let r = d
        .aplicar(ComandoDeEdicao::RetirarAltura { nota: g })
        .expect("retira Sol");
    assert_eq!(r.aplicado, ComandoDeEdicao::RemoverNota { nota: g });
    assert!(d.partitura().partes[0].compassos[0].notas[0].e_pausa());
    assert_eq!(somas_por_voz(d.partitura()), somas);
    d.desfazer().expect("1");
    d.desfazer().expect("2");
    d.desfazer().expect("3");
    assert_eq!(d.exportar().expect("exporta"), texto);
    assert!(!d.modificado());
}

#[test]
fn retirar_a_unica_altura_e_igual_a_remocao_do_ticket_17() {
    let texto = do_re_mi();
    let mut por_retirar = abrir(&texto);
    let mut por_remover = abrir(&texto);
    let re = id_da(&por_retirar, 1);
    let a = por_retirar
        .aplicar(ComandoDeEdicao::RetirarAltura { nota: re })
        .expect("retira");
    let b = por_remover
        .aplicar(ComandoDeEdicao::RemoverNota { nota: re })
        .expect("remove");
    assert_eq!(a, b);
    assert_eq!(retrato(&por_retirar), retrato(&por_remover));
    // Pausa: não há altura para retirar (mesma recusa do ticket 17).
    let pausa = re;
    assert_eq!(
        por_retirar.aplicar(ComandoDeEdicao::RetirarAltura { nota: pausa }),
        Err(ErroDeEdicao::PausaNaoRemovivel(pausa))
    );
}

// --- ligadura, dedilhado, letra, exibição ------------------------------------------------------

#[test]
fn ligadura_e_dedilhado_das_alturas_existentes_ficam_e_a_nova_entra_sem_ligadura_com_aviso() {
    let texto = peca(
        "      <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration>\
         <tie type=\"start\"/><voice>1</voice><type>half</type><notations><tied type=\"start\"/>\
         <technical><fingering>1</fingering></technical></notations>\
         <lyric number=\"1\"><syllabic>single</syllabic><text>la</text></lyric></note>\n      \
         <note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration>\
         <tie type=\"start\"/><voice>1</voice><type>half</type><notations><tied type=\"start\"/>\
         <technical><fingering>3</fingering></technical></notations></note>\n      \
         <note><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice>\
         <type>half</type></note>\n",
    );
    let mut d = abrir(&texto);
    let (c, e, re) = (id_da(&d, 0), id_da(&d, 1), id_da(&d, 2));
    let ex0 = d.documento().exibicao();
    let (letra_c, dedo_c, dedo_e) = (
        ex0.da_nota(c).cloned().expect("Dó"),
        ex0.da_nota(c).cloned(),
        ex0.da_nota(e).cloned().expect("Mi"),
    );
    assert_eq!(letra_c.silabas[0].texto, "la");
    assert_eq!(dedo_c.expect("dedo").dedos[0].texto, "1");
    assert_eq!(dedo_e.dedos[0].texto, "3");
    let r = d.aplicar(adicionar(e, alt('G', 4))).expect("acrescenta");
    let novo = r.estrutura.as_ref().expect("estrutura").alvo;
    assert!(
        r.avisos
            .contains(&AvisoDeEdicao::AlturaNovaSemLigadura { nota: novo }),
        "{:?}",
        r.avisos
    );
    assert!(!r.avisos[0].texto().is_empty());
    // Exibição por IdNota: letra e dedilhado das que já estavam seguem nelas; a nova não tem.
    let ex = d.documento().exibicao();
    assert_eq!(ex.da_nota(c), Some(&letra_c));
    assert_eq!(ex.da_nota(e), Some(&dedo_e));
    assert_eq!(ex.da_nota(novo), None);
    assert_eq!(ex.da_nota(re), None);
    // A ligadura das duas alturas que já estavam continua no texto.
    let saida = d.exportar().expect("exporta");
    assert_eq!(saida.matches("<tie type=\"start\"/>").count(), 2);
    assert_eq!(saida.matches("<tied type=\"start\"/>").count(), 2);
    d.conferir_exportacao(&saida).expect("confere");
    // Retirar uma altura com dedilhado e ligadura avisa; a dos outros fica.
    let r = d
        .aplicar(ComandoDeEdicao::RetirarAltura { nota: e })
        .expect("retira Mi");
    assert!(r.avisos.iter().any(
        |a| matches!(a, AvisoDeEdicao::DedilhadoRemovido { nota, texto } if *nota == e && texto == "3")
    ));
    assert!(
        r.avisos
            .iter()
            .any(|a| matches!(a, AvisoDeEdicao::LigaduraDeValorRemovida { nota } if *nota == e))
    );
    let ex = d.documento().exibicao();
    assert_eq!(ex.da_nota(c), Some(&letra_c));
    assert_eq!(ex.da_nota(e), None);
    // Retirar a principal leva a letra junto, com aviso (nunca em silêncio).
    let r = d
        .aplicar(ComandoDeEdicao::RetirarAltura { nota: c })
        .expect("retira Dó");
    assert!(
        r.avisos
            .iter()
            .any(|a| matches!(a, AvisoDeEdicao::LetraRemovida { nota, .. } if *nota == c))
    );
}

#[test]
fn identidades_do_acorde_sobrevivem_pelo_arquivo_lateral_e_o_diff_pareia_pelo_id() {
    let texto = acorde_de_tres();
    let mut d = abrir(&texto);
    let c = id_da(&d, 0);
    d.aplicar(adicionar(c, alt('B', 4))).expect("acrescenta");
    d.aplicar(ComandoDeEdicao::RetirarAltura { nota: id_da(&d, 1) })
        .expect("retira");
    let saida = d.exportar().expect("exporta");
    let lateral =
        piano_tutor_core::documento::TabelaIds::de_texto(&d.tabela_ids_para(&saida).para_texto())
            .expect("lê");
    let r = DocumentoAberto::abrir_texto_com_ids("t", &saida, Some(&lateral)).expect("reabre");
    assert!(
        d.documento()
            .notas()
            .map(|n| n.id)
            .eq(r.documento().notas().map(|n| n.id))
    );
    d.conferir_exportacao(&saida).expect("confere");
}

// --- varredura do corpus -----------------------------------------------------------------------

fn varre_acrescentar(fx: &Fixture, max: usize) -> (usize, usize) {
    let mut d = DocumentoAberto::abrir_texto(fx.id, &fx.conteudo).expect("abre");
    let somas = somas_por_voz(d.partitura());
    let candidatas: Vec<IdNota> = d
        .documento()
        .notas()
        .filter(|n| n.altura.is_some() && !n.graca && !n.pausa)
        .map(|n| n.id)
        .collect();
    let passo = candidatas.len().div_ceil(max).max(1);
    let (mut aceitas, mut recusadas) = (0, 0);
    for id in candidatas.into_iter().step_by(passo) {
        let local = d.documento().posicao_da_nota(id).expect("local");
        for letra in ['E', 'G'] {
            let cmd = ComandoDeEdicao::AcrescentarLetra { nota: id, letra };
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
                    assert_eq!(r.compassos_afetados, vec![local.clone()], "{}", fx.id);
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
                    assert_eq!(
                        DocumentoAberto::abrir_texto(fx.id, &saida)
                            .expect("reabre")
                            .partitura(),
                        d.partitura(),
                        "{}: {cmd:?}",
                        fx.id
                    );
                    d.conferir_exportacao(&saida)
                        .unwrap_or_else(|e| panic!("{}: conferência: {e}", fx.id));
                    // A nova nota é acorde, na posição e duração da selecionada.
                    let novo = r.estrutura.as_ref().expect("estrutura").alvo;
                    let nn = d.documento().nota(novo).expect("nova");
                    let sel = d.documento().nota(id).expect("selecionada");
                    assert!(nn.acorde);
                    assert_eq!(
                        (nn.duracao, &nn.voz, &nn.pauta),
                        (sel.duracao, &sel.voz, &sel.pauta),
                        "{}",
                        fx.id
                    );
                    // Retirar a altura nova devolve o mesmo modelo de tocar.
                    let editado = retrato(&d);
                    d.aplicar(r.inverso.clone()).expect("inverso");
                    assert_eq!(
                        d.partitura().partes,
                        antes.partitura.partes,
                        "{}: inverso",
                        fx.id
                    );
                    d.desfazer().expect("desfaz o inverso");
                    assert_eq!(retrato(&d), editado, "{}", fx.id);
                    // A5: desfazer volta ao estado de antes, byte a byte.
                    d.desfazer().expect("desfaz");
                    assert_eq!(retrato(&d), antes, "{}: {cmd:?}: desfazer", fx.id);
                    assert_eq!(d.exportar().expect("exporta"), fx.conteudo, "{}", fx.id);
                    d.refazer().expect("refaz");
                    d.desfazer().expect("desfaz");
                }
            }
        }
    }
    (aceitas, recusadas)
}

#[test]
fn corpus_inteiro_acrescentar_altura_fecha_o_compasso_confere_a_exportacao_e_desfaz_byte_a_byte() {
    let (mut aceitas, mut recusadas) = (0, 0);
    for fx in corpus() {
        let (a, r) = varre_acrescentar(&fx, 10);
        aceitas += a;
        recusadas += r;
    }
    assert!(
        aceitas >= 30,
        "a varredura precisa de casos reais: {aceitas}"
    );
    assert!(
        recusadas > 0,
        "e recusar alguns (repetida, limites): {recusadas}"
    );
}

fn varre_retirar(fx: &Fixture, max: usize) -> usize {
    let mut d = DocumentoAberto::abrir_texto(fx.id, &fx.conteudo).expect("abre");
    let somas = somas_por_voz(d.partitura());
    let membros: Vec<IdNota> = d
        .documento()
        .partes()
        .iter()
        .flat_map(|p| p.compassos.iter())
        .flat_map(|c| {
            c.notas.iter().enumerate().filter_map(|(i, n)| {
                let em_acorde = n.acorde || c.notas.get(i + 1).is_some_and(|s| s.acorde);
                (em_acorde && !n.graca && !n.pausa).then_some(n.id)
            })
        })
        .collect();
    let passo = membros.len().div_ceil(max).max(1);
    let mut feitas = 0;
    for id in membros.into_iter().step_by(passo) {
        let antes = retrato(&d);
        let local = d.documento().posicao_da_nota(id).expect("local");
        let r = d
            .aplicar(ComandoDeEdicao::RetirarAltura { nota: id })
            .unwrap_or_else(|e| panic!("{}: retirar {id:?}: {e}", fx.id));
        feitas += 1;
        assert_eq!(r.aplicado, ComandoDeEdicao::RetirarAltura { nota: id });
        assert_eq!(r.compassos_afetados, vec![local], "{}", fx.id);
        assert_eq!(somas_por_voz(d.partitura()), somas, "{}: soma", fx.id);
        let saida = d.exportar().expect("exporta");
        let rel = comparar_bytes(&fx.conteudo, &saida).expect("compara");
        assert_eq!(rel.compassos_diferentes, r.compassos_afetados, "{}", fx.id);
        verificar_golden(&fx.conteudo, &saida, &r.compassos_afetados, None)
            .unwrap_or_else(|v| panic!("{}: golden: {v:?}", fx.id));
        d.conferir_exportacao(&saida)
            .unwrap_or_else(|e| panic!("{}: conferência: {e}", fx.id));
        d.desfazer().expect("desfaz");
        assert_eq!(retrato(&d), antes, "{}: desfazer", fx.id);
        assert_eq!(d.exportar().expect("exporta"), fx.conteudo, "{}", fx.id);
    }
    feitas
}

#[test]
fn corpus_inteiro_retirar_altura_de_acorde_mantem_a_soma_e_desfaz_byte_a_byte() {
    let total: usize = corpus().iter().map(|fx| varre_retirar(fx, 10)).sum();
    assert!(total >= 10, "{total}");
}

// --- G2 (gate da E01): o `inverso` devolvido, aplicado como COMANDO, restaura o texto ----------

/// O `inverso` de cada resultado aceito, aplicado como comando num documento que acabou de aplicar
/// o `aplicado`, devolve o texto exportado byte a byte (como o desfazer por snapshot).
fn varre_inversos(fx: &Fixture, max: usize) -> (usize, Vec<String>) {
    let d0 = DocumentoAberto::abrir_texto(fx.id, &fx.conteudo).expect("abre");
    let amostrar = |v: Vec<IdNota>| -> Vec<IdNota> {
        let passo = v.len().div_ceil(max).max(1);
        v.into_iter().step_by(passo).collect()
    };
    let afinadas = amostrar(
        d0.documento()
            .notas()
            .filter(|n| n.altura.is_some() && !n.graca)
            .map(|n| n.id)
            .collect(),
    );
    let em_acorde = amostrar(
        d0.documento()
            .partes()
            .iter()
            .flat_map(|p| p.compassos.iter())
            .flat_map(|c| {
                c.notas.iter().enumerate().filter_map(|(i, n)| {
                    let acorde = n.acorde || c.notas.get(i + 1).is_some_and(|s| s.acorde);
                    (acorde && !n.graca && !n.pausa).then_some(n.id)
                })
            })
            .collect(),
    );
    let mut cmds: Vec<ComandoDeEdicao> = Vec::new();
    for id in afinadas.iter().chain(&em_acorde).copied() {
        let alt0 = d0.documento().nota(id).and_then(|n| n.altura);
        cmds.push(ComandoDeEdicao::MudarAltura {
            nota: id,
            unidade: UnidadeDeAltura::Semitom,
            sentido: SentidoDaAltura::Acima,
        });
        cmds.push(ComandoDeEdicao::MudarDuracao {
            nota: id,
            figura: Figura::Colcheia,
        });
        cmds.push(ComandoDeEdicao::AlternarPonto { nota: id });
        cmds.push(ComandoDeEdicao::RemoverNota { nota: id });
        cmds.push(ComandoDeEdicao::RetirarAltura { nota: id });
        if let Some(a) = alt0 {
            let passo = if a.passo == 'G' {
                'A'
            } else {
                (a.passo as u8 + 1) as char
            };
            cmds.push(adicionar(id, alt(passo, a.oitava)));
        }
    }
    // Pausas: inserir nota e inserir pausa no lugar de uma pausa.
    let mut pausas = Vec::new();
    for (pi, parte) in d0.partitura().partes.iter().enumerate() {
        for (ci, c) in parte.compassos.iter().enumerate() {
            for n in c.notas.iter().filter(|n| n.e_pausa()) {
                pausas.push((pi, ci, n.posicao, n.voz.to_string(), n.pauta.to_string()));
            }
        }
    }
    let passo = pausas.len().div_ceil(max).max(1);
    for (pi, ci, posicao, voz, pauta) in pausas.into_iter().step_by(passo) {
        let compasso = PosCompasso {
            parte: d0.documento().partes()[pi].id.clone(),
            indice: ci,
            numero: String::new(),
        };
        for figura in [Figura::Seminima, Figura::Colcheia] {
            cmds.push(ComandoDeEdicao::InserirNota {
                compasso: compasso.clone(),
                voz: voz.clone(),
                pauta: pauta.clone(),
                posicao,
                altura: alt('E', 4),
                acidente: None,
                figura,
                pontos: 0,
            });
            cmds.push(ComandoDeEdicao::InserirPausa {
                compasso: compasso.clone(),
                voz: voz.clone(),
                pauta: pauta.clone(),
                posicao,
                figura,
                pontos: 0,
            });
        }
    }
    let coerente = figuras_coerentes(d0.partitura());
    let (mut aceitos, mut falhas) = (0, Vec::new());
    for cmd in cmds {
        let mut d = d0.clone();
        let Ok(r) = d.aplicar(cmd.clone()) else {
            continue;
        };
        if !r.alterou {
            continue;
        }
        aceitos += 1;
        let mut v = d.clone();
        if let Err(e) = v.aplicar(r.inverso.clone()) {
            falhas.push(format!(
                "{}: {cmd:?}: inverso {:?} recusado: {e}",
                fx.id, r.inverso
            ));
            continue;
        }
        // Garantia geral: as notas do modelo de tocar voltam (como conjunto por compasso: a
        // ordem das alturas de um acorde cuja principal foi removida pode mudar).
        let so_altura = matches!(
            cmd,
            ComandoDeEdicao::MudarAltura { .. } | ComandoDeEdicao::AdicionarAltura { .. }
        );
        if (so_altura || coerente)
            && conjunto_de_notas(v.partitura()) != conjunto_de_notas(d0.partitura())
        {
            falhas.push(format!(
                "{}: {cmd:?}: o inverso não restaurou o modelo",
                fx.id
            ));
        }
        // Garantia exata: o texto volta byte a byte nos comandos que não descartam nada.
        let exato = matches!(
            cmd,
            ComandoDeEdicao::MudarAltura { .. } | ComandoDeEdicao::AdicionarAltura { .. }
        );
        if exato && v.exportar().expect("exporta") != fx.conteudo {
            falhas.push(format!(
                "{}: {cmd:?}: o inverso não restaurou o texto",
                fx.id
            ));
        }
    }
    (aceitos, falhas)
}

/// O conteúdo musical por compasso: as notas com altura (posição, duração, altura, voz, pauta,
/// figura, pontos, quiáltera) como conjunto, e a soma das pausas por voz e pauta (o inverso não
/// reagrupa as pausas que uma inserção dividiu).
type ConteudoDoCompasso = (Vec<String>, BTreeMap<(u8, u8), Fracao>);

fn conjunto_de_notas(p: &Partitura) -> Vec<ConteudoDoCompasso> {
    p.partes
        .iter()
        .flat_map(|parte| parte.compassos.iter())
        .map(|c| {
            let mut notas: Vec<String> = Vec::new();
            let mut pausas: BTreeMap<(u8, u8), Fracao> = BTreeMap::new();
            for n in &c.notas {
                if n.e_pausa() {
                    let soma = pausas.entry((n.voz, n.pauta)).or_insert(Fracao::inteiro(0));
                    *soma = *soma + n.duracao;
                } else {
                    notas.push(format!(
                        "{:?}",
                        (
                            n.posicao, n.duracao, n.altura, n.midi, n.voz, n.pauta, n.adorno,
                            n.figura, n.pontos, n.tuplo
                        )
                    ));
                }
            }
            notas.sort();
            (notas, pausas)
        })
        .collect()
}

/// `true` se toda nota com altura tem a duração que a figura, os pontos e a quiáltera escritos
/// dizem (o fixture `tercina` escreve `<type>eighth</type>` com a duração de uma semínima de
/// tercina: o primeiro comando de duração normaliza isso, e o inverso não desfaz a normalização).
fn figuras_coerentes(p: &Partitura) -> bool {
    p.partes
        .iter()
        .flat_map(|x| x.compassos.iter())
        .flat_map(|c| c.notas.iter())
        .all(|n| {
            if n.e_pausa() || n.adorno {
                return true;
            }
            let f = n.figura;
            let pontos = f64::from(n.pontos);
            let nominal = f.duracao_nominal();
            let mut esperado = nominal.como_f64() * (2.0 - 0.5f64.powf(pontos));
            if let Some((a, b)) = n.tuplo {
                esperado *= f64::from(b) / f64::from(a);
            }
            (esperado - n.duracao.como_f64()).abs() < 1e-9
        })
}

#[test]
fn g2_corpus_o_inverso_aplicado_como_comando_restaura_o_texto_byte_a_byte() {
    let (mut total, mut falhas) = (0, Vec::new());
    for fx in corpus() {
        let (n, f) = varre_inversos(&fx, 6);
        total += n;
        falhas.extend(f);
    }
    assert!(total >= 100, "a varredura precisa de casos reais: {total}");
    assert!(
        falhas.is_empty(),
        "{} inversos falharam:\n{}",
        falhas.len(),
        falhas
            .iter()
            .take(400)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn g2_remover_nota_de_acorde_tem_inverso_que_se_aplica_como_comando() {
    let texto = acorde_de_tres();
    let original = abrir(&texto);
    let pitch_set = |d: &DocumentoAberto| {
        let mut v: Vec<i32> = d.partitura().partes[0].compassos[0]
            .notas
            .iter()
            .filter_map(|n| n.midi.map(i32::from))
            .collect();
        v.sort_unstable();
        v
    };
    // Membro que não é a principal: o inverso devolve o texto byte a byte.
    for ordinal in [1, 2] {
        for retira in [false, true] {
            let mut d = original.clone();
            let id = id_da(&d, ordinal);
            let cmd = if retira {
                ComandoDeEdicao::RetirarAltura { nota: id }
            } else {
                ComandoDeEdicao::RemoverNota { nota: id }
            };
            let r = d.aplicar(cmd).expect("remove");
            assert!(
                matches!(r.inverso, ComandoDeEdicao::AdicionarAltura { .. }),
                "{:?}",
                r.inverso
            );
            d.aplicar(r.inverso).expect("o inverso se aplica");
            assert_eq!(d.exportar().expect("exporta"), texto, "ordinal {ordinal}");
        }
    }
    // A principal: o inverso se aplica e devolve as alturas, a duração e a soma; a ordem das
    // alturas dentro do acorde pode mudar (a principal volta como membro).
    for retira in [false, true] {
        let mut d = original.clone();
        let id = id_da(&d, 0);
        let cmd = if retira {
            ComandoDeEdicao::RetirarAltura { nota: id }
        } else {
            ComandoDeEdicao::RemoverNota { nota: id }
        };
        let r = d.aplicar(cmd).expect("remove a principal");
        d.aplicar(r.inverso).expect("o inverso se aplica");
        assert_eq!(pitch_set(&d), pitch_set(&original));
        assert_eq!(
            somas_por_voz(d.partitura()),
            somas_por_voz(original.partitura())
        );
        d.conferir_exportacao(&d.exportar().expect("exporta"))
            .expect("confere");
    }
}
