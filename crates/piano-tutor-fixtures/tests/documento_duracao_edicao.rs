//! Ticket 16 (E01): mudar a duração de uma nota com validação do compasso, no serviço de
//! documento (sem janela e sem teclado). Cobre A4 (compasso que estoura é recusado, nada muda,
//! o undo não ganha entrada), A5 para o comando novo (desfazer volta byte a byte), A6 (anacruse,
//! voz secundária, tercina e quiáltera de cinco com `Fracao` exata), a pausa de preenchimento com
//! aviso, o alongamento sobre pausa, as vigas refeitas e o golden de exportação, em todo o corpus.

use piano_tutor_core::documento::{
    AvisoDeEdicao, ComandoDeEdicao, DocumentoAberto, ErroDeEdicao, IdNota, Nota, figura_da_tecla,
};
use piano_tutor_core::partitura::{Figura, Fracao, Partitura};
use piano_tutor_fixtures::corpus::{Fixture, corpus, fixture};
use piano_tutor_fixtures::harness::{comparar_bytes, comparar_semantica, verificar_golden};
use std::collections::BTreeMap;

const FIGURAS: [Figura; 8] = [
    Figura::Semifusa,
    Figura::Fusa,
    Figura::Semicolcheia,
    Figura::Colcheia,
    Figura::Seminima,
    Figura::Minima,
    Figura::Semibreve,
    Figura::Breve,
];

fn definir(nota: IdNota, figura: Figura) -> ComandoDeEdicao {
    ComandoDeEdicao::MudarDuracao { nota, figura }
}

fn ponto(nota: IdNota) -> ComandoDeEdicao {
    ComandoDeEdicao::AlternarPonto { nota }
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

/// Soma das durações de cada voz (sem notas de acorde nem de adorno) por parte e compasso, com
/// `Fracao` exata: é o que "o compasso fecha" quer dizer.
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

/// As vigas de cada voz do compasso formam grupos fechados (begin...end), só em nota com
/// bandeirola e nunca em pausa. Devolve o motivo se não.
fn vigas_consistentes(p: &Partitura, parte: usize, compasso: usize) -> Result<(), String> {
    use piano_tutor_core::partitura::EstadoViga::{Continua, Fim, Inicio};
    let c = &p.partes[parte].compassos[compasso];
    let mut chaves: Vec<(u8, u8)> = c.notas.iter().map(|n| (n.pauta, n.voz)).collect();
    chaves.sort_unstable();
    chaves.dedup();
    for (pauta, voz) in chaves {
        let notas: Vec<_> = c
            .notas
            .iter()
            .filter(|n| n.pauta == pauta && n.voz == voz && !n.acorde && !n.adorno)
            .collect();
        for nivel in 0..5usize {
            let mut aberto = false;
            for n in &notas {
                if !n.vigas.is_empty() && n.e_pausa() {
                    return Err(format!("pausa com viga (voz {voz})"));
                }
                if n.vigas.len() > usize::from(n.figura.bandeirolas()) {
                    return Err(format!("viga demais para {:?} (voz {voz})", n.figura));
                }
                match n.vigas.get(nivel) {
                    Some(Inicio) if aberto => return Err(format!("begin dentro de grupo {nivel}")),
                    Some(Inicio) => aberto = true,
                    Some(Continua) if !aberto => return Err(format!("continue solto {nivel}")),
                    Some(Fim) if !aberto => return Err(format!("end solto {nivel}")),
                    Some(Fim) => aberto = false,
                    Some(_) => {}
                    None if aberto => return Err(format!("grupo {nivel} quebrado sem end")),
                    None => {}
                }
            }
            if aberto {
                return Err(format!("grupo {nivel} sem end (voz {voz})"));
            }
        }
    }
    Ok(())
}

fn nota(d: &DocumentoAberto, compasso: usize, ordinal: usize) -> IdNota {
    d.documento().partes()[0].compassos[compasso].notas[ordinal].id
}

fn pausas_criadas(avisos: &[AvisoDeEdicao]) -> Vec<(Figura, u8)> {
    avisos
        .iter()
        .filter_map(|a| match a {
            AvisoDeEdicao::PausaCriada { figura, pontos, .. } => Some((*figura, *pontos)),
            _ => None,
        })
        .collect()
}

fn abrir(f: &Fixture) -> DocumentoAberto {
    DocumentoAberto::abrir_texto(f.id, &f.conteudo).expect("abre")
}

/// MusicXML mínimo de uma parte com os compassos dados (4/4, divisions `div`).
fn partitura_de(div: u32, compassos: &[&str]) -> String {
    partitura_de_com(div, compassos, false)
}

/// Como `partitura_de`; com `anacruse`, o primeiro compasso é o `0` implícito (incompleto).
fn partitura_de_com(div: u32, compassos: &[&str], anacruse: bool) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<score-partwise version=\"4.0\">\n  \
         <part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list>\n  \
         <part id=\"P1\">\n",
    );
    for (i, c) in compassos.iter().enumerate() {
        let atributos = if i == 0 {
            format!(
                "<attributes><divisions>{div}</divisions><key><fifths>0</fifths></key>\
                 <time><beats>4</beats><beat-type>4</beat-type></time></attributes>"
            )
        } else {
            String::new()
        };
        let marca = if anacruse && i == 0 {
            "number=\"0\" implicit=\"yes\"".to_owned()
        } else {
            format!("number=\"{}\"", i + usize::from(!anacruse))
        };
        s.push_str(&format!(
            "    <measure {marca}>{atributos}\n{c}\n    </measure>\n"
        ));
    }
    s.push_str("  </part>\n</score-partwise>\n");
    s
}

fn n(passo: char, duracao: u32, tipo: &str, extra: &str) -> String {
    format!(
        "      <note><pitch><step>{passo}</step><octave>5</octave></pitch><duration>{duracao}\
         </duration><voice>1</voice><type>{tipo}</type>{extra}</note>\n"
    )
}

// --- A4 ------------------------------------------------------------------------------------

#[test]
fn a4_compasso_44_completo_que_estoura_e_recusado_sem_mudar_nada() {
    let f = fixture("anacruse");
    let mut d = abrir(&f);
    // Compasso 1: Dó (mínima), Mi (semínima), Ré (semínima) = 4/4 completo.
    let mi = nota(&d, 1, 1);
    let antes = retrato(&d);
    let e = d.aplicar(definir(mi, Figura::Minima)).expect_err("estoura");
    assert!(matches!(e, ErroDeEdicao::CompassoNaoFecha { .. }), "{e:?}");
    let msg = e.to_string();
    assert!(msg.contains("compasso"), "{msg}");
    assert!(msg.contains("5"), "mostra o quanto ficaria: {msg}");
    assert!(msg.contains("4"), "mostra o quanto cabe: {msg}");
    assert!(
        msg.contains("não foi alterad") || msg.contains("nada foi alterado"),
        "{msg}"
    );
    // Modelo, texto e identidades inalterados; o undo não ganhou entrada.
    assert_eq!(retrato(&d), antes);
    assert_eq!(d.entradas_de_desfazer(), 0);
    assert!(!d.pode_desfazer());
    assert!(!d.editado());
    assert!(!d.modificado());
    assert_eq!(d.exportar().expect("exporta"), f.conteudo);
    // O ponto também estoura (semínima pontuada).
    let e = d.aplicar(ponto(mi)).expect_err("ponto estoura");
    assert!(matches!(e, ErroDeEdicao::CompassoNaoFecha { .. }), "{e:?}");
    assert_eq!(retrato(&d), antes);
    assert_eq!(d.entradas_de_desfazer(), 0);
}

#[test]
fn a4_recusa_no_meio_do_historico_nao_cria_entrada_nem_descarta_o_refazer() {
    let f = fixture("anacruse");
    let mut d = abrir(&f);
    let (do5, mi) = (nota(&d, 1, 0), nota(&d, 1, 1));
    d.aplicar(definir(do5, Figura::Seminima)).expect("encurta");
    d.desfazer().expect("desfaz");
    let (desf, ref_) = (d.entradas_de_desfazer(), d.entradas_de_refazer());
    assert_eq!((desf, ref_), (0, 1));
    let antes = retrato(&d);
    d.aplicar(definir(mi, Figura::Semibreve))
        .expect_err("estoura");
    assert_eq!(
        (d.entradas_de_desfazer(), d.entradas_de_refazer()),
        (desf, ref_)
    );
    assert_eq!(retrato(&d), antes);
}

#[test]
fn a4_voz_secundaria_estourando_cita_a_voz() {
    let f = fixture("duas-vozes");
    let mut d = abrir(&f);
    // Compasso 1, voz 1: Mi e Ré (mínimas); alongar o Ré para semibreve estoura a voz 1.
    let re = nota(&d, 0, 1);
    let antes = retrato(&d);
    let e = d
        .aplicar(definir(re, Figura::Semibreve))
        .expect_err("estoura");
    match &e {
        ErroDeEdicao::CompassoNaoFecha {
            voz,
            ficaria,
            esperado,
            ..
        } => {
            assert_eq!(voz, "1");
            assert_eq!(*ficaria, Fracao::inteiro(6));
            assert_eq!(*esperado, Fracao::inteiro(4));
        }
        outro => panic!("{outro:?}"),
    }
    assert!(e.to_string().contains("voz 1"), "{e}");
    assert_eq!(retrato(&d), antes);
}

// --- Encurtar: pausa exata com aviso ----------------------------------------------------------

#[test]
fn encurtar_preenche_com_pausa_da_duracao_exata_e_avisa() {
    let f = fixture("anacruse");
    let mut d = abrir(&f);
    let do5 = nota(&d, 1, 0);
    let antes = retrato(&d);
    let somas = somas_por_voz(d.partitura());
    let r = d.aplicar(definir(do5, Figura::Seminima)).expect("encurta");
    assert!(r.alterou);
    assert_eq!(r.compassos_afetados.len(), 1);
    assert_eq!(pausas_criadas(&r.avisos), vec![(Figura::Seminima, 0)]);
    let m = r.duracao.as_ref().expect("resultado de duração");
    assert_eq!(m.antes.figura, Figura::Minima);
    assert_eq!(m.depois.figura, Figura::Seminima);
    assert_eq!(m.depois.duracao, Fracao::inteiro(1));
    assert_eq!(m.pausas_criadas.len(), 1);
    // O compasso continua fechando; as notas seguintes ficam onde estavam.
    assert_eq!(somas_por_voz(d.partitura()), somas);
    let c = &d.partitura().partes[0].compassos[1];
    assert_eq!(c.notas.len(), 4);
    assert!(c.notas[1].e_pausa());
    assert_eq!(c.notas[1].duracao, Fracao::inteiro(1));
    assert_eq!(c.notas[1].posicao, Fracao::inteiro(1));
    assert_eq!(c.notas[2].posicao, Fracao::inteiro(2));
    assert_eq!(c.notas[3].posicao, Fracao::inteiro(3));
    // Identidades: as notas de antes mantêm o id; a pausa nova é id novo e nunca reutilizado.
    assert_eq!(d.ids().id(0, 1, 0), Some(do5));
    assert_eq!(d.ids().id(0, 1, 2), Some(IdNota(3)));
    assert_eq!(d.ids().id(0, 1, 3), Some(IdNota(4)));
    assert_eq!(d.ids().id(0, 1, 1), Some(m.pausas_criadas[0]));
    assert!(m.pausas_criadas[0].0 > 5, "id novo: {:?}", m.pausas_criadas);
    assert_eq!(d.ids().total(), antes.ids.len() + 1);
    // Só o compasso editado mudou, byte a byte, e o texto reabre como o mesmo modelo.
    let saida = d.exportar().expect("exporta");
    let rel = comparar_bytes(&f.conteudo, &saida).expect("compara");
    assert_eq!(rel.compassos_diferentes, r.compassos_afetados);
    verificar_golden(&f.conteudo, &saida, &r.compassos_afetados, None).expect("golden");
    let reaberto = DocumentoAberto::abrir_texto("r", &saida).expect("reabre");
    assert_eq!(reaberto.partitura(), d.partitura());
    d.conferir_exportacao(&saida)
        .expect("a conferência aceita a pausa nova");
    // A5: desfazer volta ao arquivo original byte a byte; refazer volta ao editado.
    let editado = retrato(&d);
    d.desfazer().expect("desfaz");
    assert_eq!(retrato(&d), antes);
    assert_eq!(d.exportar().expect("exporta"), f.conteudo);
    d.refazer().expect("refaz");
    assert_eq!(retrato(&d), editado);
}

#[test]
fn encurtar_com_resto_que_nao_e_uma_figura_so_vira_varias_pausas_exatas() {
    // divisions 2: semibreve (8 unidades) -> semínima pontuada (3 unidades); sobram 5 unidades
    // (5/2 de semínima) = mínima + colcheia: duas pausas exatas, nunca uma pausa arredondada.
    let xml = partitura_de(2, &[&n('C', 8, "whole", "")]);
    let mut d = DocumentoAberto::abrir_texto("resto", &xml).expect("abre");
    let alvo = nota(&d, 0, 0);
    let r = d
        .aplicar(ComandoDeEdicao::DefinirDuracao {
            nota: alvo,
            figura: Figura::Seminima,
            pontos: 1,
        })
        .expect("encurta");
    assert_eq!(
        pausas_criadas(&r.avisos),
        vec![(Figura::Minima, 0), (Figura::Colcheia, 0)]
    );
    let c = &d.partitura().partes[0].compassos[0];
    assert_eq!(c.notas.len(), 3);
    assert_eq!(c.notas[0].duracao, Fracao::nova(3, 2));
    assert_eq!(c.notas[1].posicao, Fracao::nova(3, 2));
    assert_eq!(c.notas[2].posicao, Fracao::nova(7, 2));
    assert_eq!(
        c.notas.iter().fold(Fracao::ZERO, |s, n| s + n.duracao),
        Fracao::inteiro(4)
    );
    // O inverso resolvido devolve a nota ao que era (a pausa é consumida).
    let r2 = d.aplicar(r.inverso.clone()).expect("inverso");
    assert!(r2.alterou);
    assert_eq!(d.exportar().expect("exporta"), xml);
    assert_eq!(r2.inverso, r.aplicado);
}

// --- A6: anacruse, voz secundária, tercina e quiáltera --------------------------------------

#[test]
fn a6_anacruse_continua_valida_e_nao_cresce() {
    let f = fixture("anacruse");
    let mut d = abrir(&f);
    assert!(d.partitura().compassos[0].incompleto);
    let sol = nota(&d, 0, 0);
    // Alongar a nota da anacruse estouraria o compasso incompleto sinalizado: recusado.
    let antes = retrato(&d);
    let e = d.aplicar(definir(sol, Figura::Minima)).expect_err("cresce");
    assert!(matches!(e, ErroDeEdicao::CompassoNaoFecha { .. }), "{e:?}");
    assert_eq!(retrato(&d), antes);
    // Aqui `divisions` é 1: a colcheia (1/2) não cabe, e isso é erro explícito.
    let e = d
        .aplicar(definir(sol, Figura::Colcheia))
        .expect_err("divisions");
    assert!(
        matches!(e, ErroDeEdicao::DuracaoExigeMudarDivisions { .. }),
        "{e:?}"
    );
    assert_eq!(retrato(&d), antes);

    // Com `divisions = 2` a anacruse (uma semínima) encurta para colcheia com pausa exata.
    let xml = partitura_de_com(
        2,
        &[&n('G', 2, "quarter", ""), &n('C', 8, "whole", "")],
        true,
    );
    let mut d = DocumentoAberto::abrir_texto("anacruse2", &xml).expect("abre");
    assert!(d.partitura().compassos[0].incompleto);
    let inicios = d.partitura().inicios_dos_compassos();
    let antes = retrato(&d);
    let sol = nota(&d, 0, 0);
    let r = d.aplicar(definir(sol, Figura::Colcheia)).expect("encurta");
    assert_eq!(pausas_criadas(&r.avisos), vec![(Figura::Colcheia, 0)]);
    assert!(d.partitura().compassos[0].incompleto, "segue sinalizado");
    assert_eq!(d.partitura().compassos[0].duracao, Fracao::inteiro(1));
    assert_eq!(d.partitura().inicios_dos_compassos(), inicios);
    let c = &d.partitura().partes[0].compassos[0];
    assert_eq!(c.notas[0].duracao, Fracao::nova(1, 2));
    assert_eq!(c.notas[1].duracao, Fracao::nova(1, 2));
    assert!(c.notas[1].e_pausa());
    // Alongar de volta consome a pausa; mais que isso estoura o compasso incompleto.
    d.aplicar(definir(sol, Figura::Seminima)).expect("alonga");
    assert_eq!(d.exportar().expect("exporta"), xml);
    let e = d
        .aplicar(definir(sol, Figura::Minima))
        .expect_err("estoura");
    assert!(matches!(e, ErroDeEdicao::CompassoNaoFecha { .. }), "{e:?}");
    while d.desfazer().is_some() {}
    assert_eq!(retrato(&d), antes);
}

#[test]
fn a6_voz_secundaria_encurta_e_alonga_sem_mexer_na_outra_voz() {
    let f = fixture("duas-vozes");
    let mut d = abrir(&f);
    let antes = retrato(&d);
    let somas = somas_por_voz(d.partitura());
    // Compasso 1, voz 2: Dó3 semibreve -> mínima, com pausa de mínima; a voz 1 fica intacta.
    let baixo = nota(&d, 0, 2);
    let r = d.aplicar(definir(baixo, Figura::Minima)).expect("encurta");
    assert_eq!(pausas_criadas(&r.avisos), vec![(Figura::Minima, 0)]);
    assert_eq!(somas_por_voz(d.partitura()), somas);
    let saida = d.exportar().expect("exporta");
    assert!(saida.contains("<backup><duration>4</duration></backup>") || saida.contains("backup"));
    let ant = Partitura::de_musicxml(&f.conteudo).expect("modelo");
    let nova = Partitura::de_musicxml(&saida).expect("modelo");
    let (v1_ant, v1_nova) = (
        ant.partes[0].compassos[0]
            .notas
            .iter()
            .filter(|n| n.voz == 1)
            .cloned()
            .collect::<Vec<_>>(),
        nova.partes[0].compassos[0]
            .notas
            .iter()
            .filter(|n| n.voz == 1)
            .cloned()
            .collect::<Vec<_>>(),
    );
    assert_eq!(v1_ant, v1_nova, "a voz 1 não mudou");
    // Alonga de volta: a pausa criada é consumida (com aviso) e o texto original volta.
    let r = d
        .aplicar(definir(baixo, Figura::Semibreve))
        .expect("alonga");
    assert!(
        r.avisos
            .iter()
            .any(|a| matches!(a, AvisoDeEdicao::PausasConsumidas { .. })),
        "{:?}",
        r.avisos
    );
    assert_eq!(somas_por_voz(d.partitura()), somas);
    assert_eq!(d.exportar().expect("exporta"), f.conteudo);
    assert_eq!(d.partitura(), &antes.partitura);
}

const TRIO: &str = "<time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification>";

#[test]
fn a6_tercina_exige_fracao_exata_com_denominador_3() {
    // divisions 3: três semínimas de tercina (2/3 cada, 2 unidades) e uma mínima (6 unidades).
    let compasso = [
        n('C', 2, "quarter", TRIO),
        n('D', 2, "quarter", TRIO),
        n('E', 2, "quarter", TRIO),
        n('F', 6, "half", ""),
    ]
    .concat();
    let xml = partitura_de(3, &[&compasso]);
    let mut d = DocumentoAberto::abrir_texto("tercina", &xml).expect("abre");
    let antes = retrato(&d);
    let somas = somas_por_voz(d.partitura());
    assert_eq!(somas[&(0, 0, 1, 1)], Fracao::inteiro(4));
    // Semínima de tercina (2/3) -> colcheia de tercina (1/3): sobra 1/3, uma pausa de colcheia
    // de tercina (também 1/3), com a mesma quiáltera; denominador 3 sem arredondar.
    let do5 = nota(&d, 0, 0);
    let r = d.aplicar(definir(do5, Figura::Colcheia)).expect("encurta");
    assert_eq!(pausas_criadas(&r.avisos), vec![(Figura::Colcheia, 0)]);
    assert!(
        r.avisos
            .iter()
            .any(|a| matches!(a, AvisoDeEdicao::TuplaPodeTerFicadoIncompleta { .. })),
        "a tupla perdeu a nota inteira: {:?}",
        r.avisos
    );
    let c = &d.partitura().partes[0].compassos[0];
    assert_eq!(c.notas[0].duracao, Fracao::nova(1, 3));
    assert_eq!(c.notas[1].duracao, Fracao::nova(1, 3));
    assert!(c.notas[1].e_pausa());
    assert_eq!(c.notas[1].tuplo, Some((3, 2)));
    assert_eq!(somas_por_voz(d.partitura()), somas);
    let saida = d.exportar().expect("exporta");
    assert_eq!(
        DocumentoAberto::abrir_texto("r", &saida)
            .expect("reabre")
            .partitura(),
        d.partitura()
    );
    // Semicolcheia de tercina = 1/6 de semínima = meia unidade: não cabe em divisions = 3.
    let re5 = nota(&d, 0, 2);
    let e = d
        .aplicar(definir(re5, Figura::Semicolcheia))
        .expect_err("não cabe");
    assert!(
        matches!(
            e,
            ErroDeEdicao::DuracaoExigeMudarDivisions {
                divisions: 3,
                necessarias: 6,
                ..
            }
        ),
        "{e:?}"
    );
    // Alongar de volta consome a pausa e devolve o texto original.
    d.aplicar(definir(do5, Figura::Seminima)).expect("alonga");
    assert_eq!(d.exportar().expect("exporta"), xml);
    assert_eq!(retrato(&d).partitura, antes.partitura);
}

#[test]
fn a6_tercina_valor_que_nao_cabe_em_divisions_e_erro_explicito_nunca_arredondamento() {
    let f = fixture("tercina");
    let mut d = abrir(&f);
    let antes = retrato(&d);
    // Semínima (3 unidades, divisions = 3) -> colcheia = 1/2 semínima: 3/2 unidade, não inteiro.
    let fa = nota(&d, 0, 3);
    let e = d
        .aplicar(definir(fa, Figura::Colcheia))
        .expect_err("não cabe");
    match &e {
        ErroDeEdicao::DuracaoExigeMudarDivisions {
            divisions,
            necessarias,
            valor,
        } => {
            assert_eq!(*divisions, 3);
            assert_eq!(*necessarias, 6);
            assert_eq!(*valor, Fracao::nova(1, 2));
        }
        outro => panic!("{outro:?}"),
    }
    let msg = e.to_string();
    assert!(msg.contains("divisions"), "{msg}");
    // `divisions` continua o que era e nada mudou.
    assert_eq!(d.documento().divisions_da_parte("P1"), Some(3));
    assert_eq!(retrato(&d), antes);
    assert_eq!(d.entradas_de_desfazer(), 0);
}

#[test]
fn a6_quialtera_de_cinco_exige_fracao_exata_com_denominador_5() {
    // 4/4, divisions 5. Quiáltera 5:4 de semicolcheias (cada uma vale 1/5 de semínima = 1
    // unidade) ocupando uma semínima; depois uma mínima e uma semínima... (1 + 2 + 1 = 4).
    let q = "<time-modification><actual-notes>5</actual-notes><normal-notes>4</normal-notes></time-modification>";
    let quint = |passo: char, dur: u32, tipo: &str| n(passo, dur, tipo, q);
    let compasso = [
        quint('C', 2, "eighth"),
        quint('D', 1, "16th"),
        quint('E', 1, "16th"),
        quint('F', 1, "16th"),
        n('G', 10, "half", ""),
        n('A', 5, "quarter", ""),
    ]
    .concat();
    let xml = partitura_de(5, &[&compasso]);
    let mut d = DocumentoAberto::abrir_texto("quint", &xml).expect("abre");
    let antes = retrato(&d);
    let somas = somas_por_voz(d.partitura());
    assert_eq!(somas[&(0, 0, 1, 1)], Fracao::inteiro(4));
    // Colcheia de quiáltera (2/5) -> semicolcheia (1/5): sobra 1/5, uma pausa de semicolcheia
    // da mesma quiáltera, de uma unidade (denominador 5, sem arredondar).
    let do5 = nota(&d, 0, 0);
    let r = d
        .aplicar(definir(do5, Figura::Semicolcheia))
        .expect("encurta");
    assert_eq!(pausas_criadas(&r.avisos), vec![(Figura::Semicolcheia, 0)]);
    let c = &d.partitura().partes[0].compassos[0];
    assert_eq!(c.notas[0].duracao, Fracao::nova(1, 5));
    assert_eq!(c.notas[1].duracao, Fracao::nova(1, 5));
    assert!(c.notas[1].e_pausa());
    assert_eq!(c.notas[1].tuplo, Some((5, 4)));
    assert_eq!(somas_por_voz(d.partitura()), somas);
    // 32nd em quiáltera 5:4 = 1/10 de semínima = meia unidade: não cabe em divisions = 5.
    let re5 = nota(&d, 0, 2);
    let e = d.aplicar(definir(re5, Figura::Fusa)).expect_err("não cabe");
    assert!(
        matches!(
            e,
            ErroDeEdicao::DuracaoExigeMudarDivisions {
                divisions: 5,
                necessarias: 10,
                ..
            }
        ),
        "{e:?}"
    );
    // A5 vale também aqui.
    d.desfazer().expect("desfaz");
    assert_eq!(retrato(&d), antes);
    assert_eq!(d.exportar().expect("exporta"), xml);
}

// --- Alongar sobre pausa -----------------------------------------------------------------------

#[test]
fn alongar_so_cresce_sobre_pausa_e_o_ponto_alterna() {
    // divisions 2: mínima (4), semínima (2), semínima (2).
    let xml = partitura_de(
        2,
        &[&[
            n('C', 4, "half", ""),
            n('E', 2, "quarter", ""),
            n('D', 2, "quarter", ""),
        ]
        .concat()],
    );
    let mut d = DocumentoAberto::abrir_texto("alongar", &xml).expect("abre");
    let original = retrato(&d);
    let do5 = nota(&d, 0, 0);
    // Cria a pausa (mínima -> semínima) e usa o ponto: a semínima pontuada consome meia pausa.
    d.aplicar(definir(do5, Figura::Seminima)).expect("encurta");
    let r = d.aplicar(ponto(do5)).expect("ponto liga");
    let m = r.duracao.as_ref().expect("duração");
    assert_eq!((m.antes.pontos, m.depois.pontos), (0, 1));
    assert_eq!(m.depois.duracao, Fracao::nova(3, 2));
    assert!(
        r.avisos
            .iter()
            .any(|a| matches!(a, AvisoDeEdicao::PausasConsumidas { .. })),
        "{:?}",
        r.avisos
    );
    let c = &d.partitura().partes[0].compassos[0];
    assert_eq!(c.notas[0].pontos, 1);
    assert_eq!(c.notas[0].figura, Figura::Seminima);
    assert_eq!(c.notas[1].duracao, Fracao::nova(1, 2));
    assert!(c.notas[1].e_pausa());
    assert_eq!(c.notas[2].posicao, Fracao::inteiro(2), "Mi não se moveu");
    // O ponto de novo tira o ponto (alterna) e devolve a meia pausa.
    let r = d.aplicar(ponto(do5)).expect("ponto desliga");
    let m = r.duracao.as_ref().expect("duração");
    assert_eq!((m.antes.pontos, m.depois.pontos), (1, 0));
    assert!(!pausas_criadas(&r.avisos).is_empty());
    let soma = d.partitura().partes[0].compassos[0]
        .notas
        .iter()
        .fold(Fracao::ZERO, |s, n| s + n.duracao);
    assert_eq!(soma, Fracao::inteiro(4));
    // Desfazer tudo: o arquivo original, byte a byte.
    while d.desfazer().is_some() {}
    assert_eq!(retrato(&d), original);
    assert_eq!(d.exportar().expect("exporta"), xml);
}

#[test]
fn alongar_sem_pausa_depois_da_nota_num_compasso_incompleto_diz_que_falta_espaco() {
    // Voz 2 só com uma mínima num 4/4 (a voz está incompleta): alongar sem pausa não pode, mas
    // não estoura o compasso; a mensagem é de falta de espaço, não de estouro.
    let c = [
        n('C', 4, "whole", ""),
        "      <backup><duration>4</duration></backup>\n".to_owned(),
        "      <note><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration>\
         <voice>2</voice><type>half</type></note>\n"
            .to_owned(),
    ]
    .concat();
    let xml = partitura_de(1, &[&c]);
    let mut d = DocumentoAberto::abrir_texto("v2", &xml).expect("abre");
    let antes = retrato(&d);
    let e = d
        .aplicar(definir(nota(&d, 0, 1), Figura::Semibreve))
        .expect_err("sem espaço");
    assert!(
        matches!(e, ErroDeEdicao::SemPausaParaAlongar { .. }),
        "{e:?}"
    );
    assert!(e.to_string().contains("pausa"), "{e}");
    assert_eq!(retrato(&d), antes);
}

// --- Vigas -------------------------------------------------------------------------------------

fn colcheia_viga(passo: char, estado: &str, duracao: u32) -> String {
    n(
        passo,
        duracao,
        "eighth",
        &format!("<beam number=\"1\">{estado}</beam>"),
    )
}

#[test]
fn encurtar_nota_no_meio_de_um_grupo_de_vigas_refaz_as_vigas_de_forma_consistente() {
    let grupo = |a: &str, b: &str, c: &str, e: &str| {
        [
            colcheia_viga('C', a, 2),
            colcheia_viga('D', b, 2),
            colcheia_viga('E', c, 2),
            colcheia_viga('F', e, 2),
        ]
        .concat()
    };
    let compasso = [
        grupo("begin", "continue", "continue", "end"),
        grupo("begin", "continue", "continue", "end"),
    ]
    .concat();
    let xml = partitura_de(4, &[&compasso]);
    let mut d = DocumentoAberto::abrir_texto("vigas", &xml).expect("abre");
    vigas_consistentes(d.partitura(), 0, 0).expect("o arquivo já era consistente");
    let antes = retrato(&d);
    // Segunda colcheia -> semicolcheia: a pausa de preenchimento cai no meio do grupo aberto.
    let r = d
        .aplicar(definir(nota(&d, 0, 1), Figura::Semicolcheia))
        .expect("encurta");
    assert!(
        r.avisos
            .iter()
            .any(|a| matches!(a, AvisoDeEdicao::VigasRefeitas { .. })),
        "{:?}",
        r.avisos
    );
    vigas_consistentes(d.partitura(), 0, 0).expect("modelo editado");
    // Semântica: o texto exportado, relido do zero, tem as mesmas vigas consistentes.
    let saida = d.exportar().expect("exporta");
    let reaberto = DocumentoAberto::abrir_texto("r", &saida).expect("reabre");
    assert_eq!(reaberto.partitura(), d.partitura());
    vigas_consistentes(reaberto.partitura(), 0, 0).expect("relido");
    // A pausa nunca tem viga; as colcheias do segundo grupo seguem vigadas entre si.
    let c = &reaberto.partitura().partes[0].compassos[0];
    assert!(
        c.notas
            .iter()
            .filter(|n| n.e_pausa())
            .all(|n| n.vigas.is_empty())
    );
    assert!(!c.notas[5].vigas.is_empty() && !c.notas[6].vigas.is_empty());
    // Desfazer devolve o texto original (e as vigas originais) byte a byte.
    d.desfazer().expect("desfaz");
    assert_eq!(retrato(&d), antes);
    assert_eq!(d.exportar().expect("exporta"), xml);
}

#[test]
fn nota_que_deixa_de_ter_bandeirola_perde_a_viga_sem_deixar_grupo_aberto() {
    // Duas colcheias vigadas; a primeira vira semínima... só encurtando é possível; usa-se o
    // caminho contrário: a semínima vira colcheia dentro de um compasso com vigas.
    let compasso = [
        colcheia_viga('C', "begin", 1),
        colcheia_viga('D', "end", 1),
        n('E', 2, "quarter", ""),
        n('F', 4, "half", ""),
    ]
    .concat();
    let xml = partitura_de(2, &[&compasso]);
    let mut d = DocumentoAberto::abrir_texto("v", &xml).expect("abre");
    let r = d
        .aplicar(definir(nota(&d, 0, 2), Figura::Colcheia))
        .expect("encurta");
    assert_eq!(pausas_criadas(&r.avisos), vec![(Figura::Colcheia, 0)]);
    vigas_consistentes(d.partitura(), 0, 0).expect("consistente");
    let c = &d.partitura().partes[0].compassos[0];
    assert_eq!(c.notas[2].duracao, Fracao::nova(1, 2));
    assert!(c.notas[3].e_pausa());
}

// --- Recusas ------------------------------------------------------------------------------------

#[test]
fn pausa_nota_de_graca_e_nota_inexistente_sao_recusadas_sem_mudar_nada() {
    let f = fixture("brahms-wiegenlied");
    let mut d = abrir(&f);
    let antes = retrato(&d);
    let pausa = d.documento().notas().find(|n| n.pausa).map(|n| n.id);
    let graca = d.documento().notas().find(|n| n.graca).map(|n| n.id);
    let mut recusas = vec![d.aplicar(definir(IdNota(9_999_999), Figura::Seminima))];
    if let Some(p) = pausa {
        recusas.push(d.aplicar(definir(p, Figura::Seminima)));
    }
    if let Some(g) = graca {
        recusas.push(d.aplicar(definir(g, Figura::Seminima)));
    }
    assert!(recusas.len() >= 2, "o corpus tem pausa ou graça");
    for r in &recusas {
        let e = r.as_ref().expect_err("recusa");
        assert!(!e.to_string().is_empty());
    }
    assert!(matches!(recusas[0], Err(ErroDeEdicao::NotaInexistente(_))));
    assert_eq!(retrato(&d), antes);
    assert_eq!(d.entradas_de_desfazer(), 0);
}

#[test]
fn as_teclas_um_a_oito_mapeiam_para_as_figuras_e_a_nove_e_recusada_com_motivo() {
    let esperado = [
        (1, Figura::Semifusa),
        (2, Figura::Fusa),
        (3, Figura::Semicolcheia),
        (4, Figura::Colcheia),
        (5, Figura::Seminima),
        (6, Figura::Minima),
        (7, Figura::Semibreve),
        (8, Figura::Breve),
    ];
    for (tecla, figura) in esperado {
        assert_eq!(figura_da_tecla(tecla), Ok(figura), "tecla {tecla}");
    }
    // A longa (tecla 9) não existe no modelo de partitura: recusa explícita, nunca figura errada.
    let e = figura_da_tecla(9).expect_err("longa");
    assert!(matches!(e, ErroDeEdicao::FiguraNaoSuportada(9)), "{e:?}");
    assert!(e.to_string().contains("longa"), "{e}");
    assert!(figura_da_tecla(0).is_err());
    assert!(figura_da_tecla(10).is_err());
}

// --- Corpus ------------------------------------------------------------------------------------

/// Notas afinadas, espalhadas pela peça (no máximo `max`).
fn amostra(d: &DocumentoAberto, max: usize) -> Vec<IdNota> {
    let todas: Vec<IdNota> = d
        .documento()
        .notas()
        .filter(|n| n.altura.is_some() && !n.graca)
        .map(|n| n.id)
        .collect();
    let passo = todas.len().div_ceil(max).max(1);
    todas.into_iter().step_by(passo).collect()
}

#[derive(Default)]
struct Contagem {
    aceitas: usize,
    estouro: usize,
    sem_pausa: usize,
    divisions: usize,
    com_pausa_criada: usize,
    com_pausa_consumida: usize,
    vigas_refeitas: usize,
}

fn varre(f: &Fixture, max_notas: usize, c: &mut Contagem) {
    let mut d = abrir(f);
    let original = retrato(&d);
    let somas = somas_por_voz(d.partitura());
    for id in amostra(&d, max_notas) {
        let mut comandos: Vec<ComandoDeEdicao> = FIGURAS.iter().map(|g| definir(id, *g)).collect();
        comandos.push(ponto(id));
        for cmd in comandos {
            let antes = retrato(&d);
            let entradas = d.entradas_de_desfazer();
            match d.aplicar(cmd.clone()) {
                Err(e) => {
                    match e {
                        ErroDeEdicao::CompassoNaoFecha { .. } => c.estouro += 1,
                        ErroDeEdicao::SemPausaParaAlongar { .. } => c.sem_pausa += 1,
                        ErroDeEdicao::DuracaoExigeMudarDivisions { .. } => c.divisions += 1,
                        ErroDeEdicao::PausaNaoEditavel(_)
                        | ErroDeEdicao::NotaDeGracaSemDuracao(_)
                        | ErroDeEdicao::ValorEscritoDesconhecido(_) => {}
                        outro => panic!("{}: {cmd:?}: recusa inesperada: {outro}", f.id),
                    }
                    assert!(!e_vazia(&e), "{}: mensagem vazia", f.id);
                    assert_eq!(
                        retrato(&d),
                        antes,
                        "{}: {cmd:?}: recusa mexeu em algo",
                        f.id
                    );
                    assert_eq!(d.entradas_de_desfazer(), entradas, "{}: sem entrada", f.id);
                }
                Ok(r) if !r.alterou => {
                    assert_eq!(retrato(&d), antes, "{}: {cmd:?}: sem mudança", f.id);
                    assert_eq!(d.entradas_de_desfazer(), entradas, "{}", f.id);
                }
                Ok(r) => {
                    c.aceitas += 1;
                    assert_eq!(d.entradas_de_desfazer(), entradas + 1, "{}", f.id);
                    assert_eq!(r.compassos_afetados.len(), 1, "{}", f.id);
                    // O compasso continua fechando, voz a voz, com Fracao exata.
                    assert_eq!(
                        somas_por_voz(d.partitura()),
                        somas,
                        "{}: {cmd:?}: a soma de alguma voz mudou",
                        f.id
                    );
                    // Golden: só o compasso editado difere do original, byte a byte.
                    let saida = d.exportar().expect("exporta");
                    let rel = comparar_bytes(&f.conteudo, &saida).expect("compara");
                    assert_eq!(rel.compassos_diferentes, r.compassos_afetados, "{}", f.id);
                    assert!(!rel.fora_dos_compassos, "{}", f.id);
                    verificar_golden(&f.conteudo, &saida, &r.compassos_afetados, None)
                        .unwrap_or_else(|v| panic!("{}: golden: {v:?}", f.id));
                    // O modelo de tocar já editado é o que sai de reabrir o texto exportado.
                    let reaberto = DocumentoAberto::abrir_texto(f.id, &saida).expect("reabre");
                    assert_eq!(
                        reaberto.partitura(),
                        d.partitura(),
                        "{}: {cmd:?}: modelo de tocar",
                        f.id
                    );
                    d.conferir_exportacao(&saida)
                        .unwrap_or_else(|e| panic!("{}: conferência: {e}", f.id));
                    assert_eq!(reaberto.ids().total(), d.ids().total(), "{}", f.id);
                    // Toda pausa criada ou consumida foi declarada.
                    let m = r.duracao.as_ref().expect("duração");
                    assert_eq!(
                        pausas_criadas(&r.avisos).len(),
                        m.pausas_criadas.len(),
                        "{}",
                        f.id
                    );
                    c.com_pausa_criada += usize::from(!m.pausas_criadas.is_empty());
                    let consumiu = r
                        .avisos
                        .iter()
                        .any(|a| matches!(a, AvisoDeEdicao::PausasConsumidas { .. }));
                    assert_eq!(consumiu, !m.pausas_removidas.is_empty(), "{}", f.id);
                    c.com_pausa_consumida += usize::from(consumiu);
                    if r.avisos
                        .iter()
                        .any(|a| matches!(a, AvisoDeEdicao::VigasRefeitas { .. }))
                    {
                        c.vigas_refeitas += 1;
                        let p = &r.compassos_afetados[0];
                        let parte = d
                            .documento()
                            .partes()
                            .iter()
                            .position(|x| x.id == p.parte)
                            .expect("parte");
                        vigas_consistentes(d.partitura(), parte, p.indice)
                            .unwrap_or_else(|m| panic!("{}: vigas: {m}", f.id));
                    }
                    // O inverso resolvido refaz a duração: a pausa criada é consumida (com aviso)
                    // e o compasso volta a ser musicalmente o de antes.
                    if !m.pausas_criadas.is_empty() {
                        let volta = d
                            .aplicar(r.inverso.clone())
                            .unwrap_or_else(|e| panic!("{}: {cmd:?}: inverso: {e}", f.id));
                        assert_eq!(somas_por_voz(d.partitura()), somas, "{}", f.id);
                        let devolta = d.exportar().expect("exporta");
                        let reaberto = DocumentoAberto::abrir_texto(f.id, &devolta).expect("abre");
                        assert_eq!(reaberto.partitura(), d.partitura(), "{}", f.id);
                        // Arquivo cujo `<type>` não bate com o `<duration>` (comum em OMR): o
                        // inverso põe a figura de volta, mas a duração é a da figura.
                        let restaurou = d.documento().nota(r.nota).and_then(|n| n.duracao)
                            == Some(m.antes.duracao);
                        if restaurou {
                            assert!(volta.alterou, "{}", f.id);
                            let sem =
                                comparar_semantica(&antes.texto, &devolta).expect("semântica");
                            assert!(sem.equivalentes(), "{}: {cmd:?}: {:?}", f.id, sem.detalhes);
                            c.com_pausa_consumida += usize::from(
                                volta
                                    .avisos
                                    .iter()
                                    .any(|a| matches!(a, AvisoDeEdicao::PausasConsumidas { .. })),
                            );
                        }
                        if volta.alterou {
                            d.desfazer().expect("desfaz o inverso");
                        }
                    }
                    // A5: desfazer volta ao estado anterior, byte a byte.
                    d.desfazer().expect("desfaz");
                    assert_eq!(retrato(&d), antes, "{}: {cmd:?}: desfazer", f.id);
                    assert_eq!(d.exportar().expect("exporta"), antes.texto);
                }
            }
        }
    }
    assert_eq!(retrato(&d), original, "{}: voltou ao original", f.id);
    assert_eq!(d.exportar().expect("exporta"), f.conteudo, "{}", f.id);
}

fn e_vazia(e: &ErroDeEdicao) -> bool {
    e.to_string().trim().is_empty()
}

#[test]
fn corpus_inteiro_toda_edicao_aceita_fecha_o_compasso_e_toda_recusa_nao_muda_nada() {
    let mut c = Contagem::default();
    for f in corpus() {
        varre(&f, 6, &mut c);
    }
    assert!(c.aceitas >= 40, "amostra pequena: {}", c.aceitas);
    assert!(c.estouro >= 10, "A4 precisa de casos reais: {}", c.estouro);
    assert!(c.com_pausa_criada >= 10, "{}", c.com_pausa_criada);
    assert!(c.divisions >= 1, "{}", c.divisions);
    assert!(c.com_pausa_consumida >= 10, "{}", c.com_pausa_consumida);
}

/// Exaustivo (toda nota afinada, as oito figuras e o ponto): rode em release com `--ignored`.
#[test]
#[ignore = "exaustivo; rodar em release"]
fn corpus_exaustivo_toda_nota_afinada_com_todas_as_figuras() {
    let mut c = Contagem::default();
    for f in corpus() {
        varre(&f, usize::MAX, &mut c);
    }
    assert!(c.aceitas > 1000, "{}", c.aceitas);
}

#[test]
fn varias_edicoes_de_duracao_desfeitas_voltam_ao_original_byte_a_byte() {
    for f in corpus() {
        let mut d = abrir(&f);
        let original = retrato(&d);
        let mut feitas = 0;
        for id in amostra(&d, 10) {
            for g in [Figura::Colcheia, Figura::Minima, Figura::Seminima] {
                if d.aplicar(definir(id, g)).is_ok_and(|r| r.alterou) {
                    feitas += 1;
                }
            }
            if d.aplicar(ponto(id)).is_ok_and(|r| r.alterou) {
                feitas += 1;
            }
        }
        assert_eq!(d.entradas_de_desfazer(), feitas, "{}", f.id);
        let final_ = retrato(&d);
        for _ in 0..feitas {
            d.desfazer().expect("desfaz");
        }
        assert_eq!(retrato(&d), original, "{}", f.id);
        assert_eq!(d.exportar().expect("exporta"), f.conteudo, "{}", f.id);
        assert!(!d.modificado(), "{}", f.id);
        for _ in 0..feitas {
            d.refazer().expect("refaz");
        }
        assert_eq!(retrato(&d), final_, "{}", f.id);
    }
}
