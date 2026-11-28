//! Segunda revisão independente da E01 (`docs/evolucao-musical/e01/REVISAO-CODIGO.md`), achados
//! N5 a N8 e a classificação dos inversos que não restauram a partitura (sonda `fuzz_inverso`).
//!
//! - N7: nenhum comando consegue pôr marcação no XML (a API será exposta por MCP): acidente,
//!   grafia, voz e pauta passam por validação estrita; um teste de injeção por comando.
//! - N6: `>` (e outros caracteres válidos) em atributo do `<accidental>` não faz `MudarAltura`
//!   nem o inverso recusarem, e o inverso preserva os bytes da marca de abertura.
//! - N5: o inverso de remover membro de acorde com altura repetida se aplica.
//! - N8: `DefinirAltura` que não muda nada devolve `alterou = false` e não cria histórico.
//! - G2: nos comandos de inserir, de ponto e de duração, o inverso aplicado como comando sempre é
//!   aceito e restaura a partitura **exceto** o agrupamento das pausas livres e as vigas.

#![allow(clippy::unwrap_used)] // teste: o `unwrap` é a asserção

use piano_tutor_core::documento::{
    Altura, ComandoDeEdicao, DocumentoAberto, IdNota, Nota, PosCompasso, SentidoDaAltura,
    UnidadeDeAltura, figura_da_tecla,
};
use piano_tutor_core::partitura::Fracao;
use piano_tutor_fixtures::corpus::corpus;

fn peca(notas: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<score-partwise version=\"4.0\">\
         <part-list><score-part id=\"P1\"><part-name>P</part-name></score-part></part-list>\
         <part id=\"P1\"><measure number=\"1\"><attributes><divisions>4</divisions>\
         <time><beats>4</beats><beat-type>4</beat-type></time></attributes>{notas}</measure>\
         </part></score-partwise>\n"
    )
}

fn nota(step: char, alter: Option<i8>, acc: Option<&str>) -> String {
    let a = alter
        .map(|a| format!("<alter>{a}</alter>"))
        .unwrap_or_default();
    let ac = acc.unwrap_or("");
    format!(
        "<note><pitch><step>{step}</step>{a}<octave>4</octave></pitch><duration>4</duration>\
         <voice>1</voice><type>quarter</type>{ac}</note>"
    )
}

fn quatro_notas() -> String {
    peca(&format!(
        "{}{}{}{}",
        nota('C', None, None),
        nota('D', None, None),
        nota('E', None, None),
        nota('F', None, None)
    ))
}

fn pausa_de_compasso() -> String {
    peca("<note><rest/><duration>16</duration><voice>1</voice><type>whole</type></note>")
}

fn alt(passo: char, alter: i8, oitava: i8) -> Altura {
    Altura {
        passo,
        alter,
        oitava,
    }
}

const HOSTIS: &[&str] = &[
    "sharp</accidental><injetado/><accidental>sharp",
    "sharp a=\"1\"><injetado/",
    "sharp onclick=\"x\"",
    "sharp cautionary=\"yes\"><injetado/><x y=\"",
    "sharp cautionary=\"a<b\"",
    "sharp cautionary=\"a&b\"",
    "sharp cautionary=\"yes\" cautionary=\"no\"",
    "sharp cautionary=yes",
    "sharp cautionary='yes\"",
    "sharp\u{1F}<injetado/>",
    "<injetado/>",
    "sharp&amp;",
    "bacura",
    "",
    " ",
    "sharp\u{0}",
];

const VOZES_HOSTIS: &[&str] = &[
    "1</voice><injetado/><voice>1",
    "<injetado/>",
    "1 2",
    "",
    "1&amp;",
];

fn estado(d: &DocumentoAberto) -> (String, usize) {
    (d.exportar().unwrap(), d.entradas_de_desfazer())
}

/// `Ok` se o comando foi recusado sem mexer em nada; `Err(motivo)` se foi aceito ou se mexeu.
fn recusa_sem_rastro(
    d: &mut DocumentoAberto,
    comando: ComandoDeEdicao,
    rotulo: &str,
) -> Result<(), String> {
    let antes = estado(d);
    match d.aplicar(comando) {
        Ok(_) => {
            let t = d.exportar().unwrap();
            if t.contains("<injetado") || t.contains("onclick") {
                return Err(format!("{rotulo}: ACEITO e a marcação entrou no XML"));
            }
            Err(format!("{rotulo}: aceito (deveria ser recusado)"))
        }
        Err(_) => {
            if estado(d) != antes {
                return Err(format!("{rotulo}: recusou mas mexeu no documento"));
            }
            Ok(())
        }
    }
}

#[test]
fn n7_definir_altura_recusa_acidente_hostil() {
    let mut d = DocumentoAberto::abrir_texto("x", &quatro_notas()).unwrap();
    let id = d.ids().id(0, 0, 1).unwrap();
    let mut falhas = Vec::new();
    for h in HOSTIS {
        let c = ComandoDeEdicao::DefinirAltura {
            nota: id,
            altura: alt('D', 1, 4),
            acidente: Some((*h).into()),
        };
        if let Err(e) = recusa_sem_rastro(&mut d, c, &format!("DefinirAltura {h:?}")) {
            falhas.push(e);
        }
    }
    assert!(falhas.is_empty(), "{falhas:#?}");
}

#[test]
fn n7_adicionar_altura_recusa_acidente_hostil() {
    let mut d = DocumentoAberto::abrir_texto("x", &quatro_notas()).unwrap();
    let id = d.ids().id(0, 0, 0).unwrap();
    let mut falhas = Vec::new();
    for h in HOSTIS {
        let c = ComandoDeEdicao::AdicionarAltura {
            nota: id,
            altura: alt('E', 1, 4),
            acidente: Some((*h).into()),
        };
        if let Err(e) = recusa_sem_rastro(&mut d, c, &format!("AdicionarAltura {h:?}")) {
            falhas.push(e);
        }
    }
    assert!(falhas.is_empty(), "{falhas:#?}");
}

#[test]
fn n7_inserir_nota_recusa_acidente_voz_e_pauta_hostis() {
    let mut d = DocumentoAberto::abrir_texto("x", &pausa_de_compasso()).unwrap();
    let c = PosCompasso::nova("P1", 0);
    let mut falhas = Vec::new();
    let monta = |voz: &str, pauta: &str, acidente: Option<&str>| ComandoDeEdicao::InserirNota {
        compasso: c.clone(),
        voz: voz.into(),
        pauta: pauta.into(),
        posicao: Fracao::nova(0, 1),
        altura: alt('G', 1, 4),
        acidente: acidente.map(Into::into),
        figura: figura_da_tecla(5).unwrap(),
        pontos: 0,
    };
    for h in HOSTIS {
        let r = recusa_sem_rastro(&mut d, monta("1", "1", Some(h)), &format!("acidente {h:?}"));
        if let Err(e) = r {
            falhas.push(e);
        }
    }
    for h in VOZES_HOSTIS {
        if let Err(e) = recusa_sem_rastro(&mut d, monta(h, "1", None), &format!("voz {h:?}")) {
            falhas.push(e);
        }
        if let Err(e) = recusa_sem_rastro(&mut d, monta("1", h, None), &format!("pauta {h:?}")) {
            falhas.push(e);
        }
    }
    assert!(falhas.is_empty(), "{falhas:#?}");
}

#[test]
fn n7_inserir_pausa_recusa_voz_e_pauta_hostis() {
    let mut d = DocumentoAberto::abrir_texto("x", &pausa_de_compasso()).unwrap();
    let c = PosCompasso::nova("P1", 0);
    let mut falhas = Vec::new();
    for h in VOZES_HOSTIS {
        for (voz, pauta) in [(*h, "1"), ("1", *h)] {
            let cmd = ComandoDeEdicao::InserirPausa {
                compasso: c.clone(),
                voz: voz.into(),
                pauta: pauta.into(),
                posicao: Fracao::nova(1, 1),
                figura: figura_da_tecla(5).unwrap(),
                pontos: 0,
            };
            let r = recusa_sem_rastro(&mut d, cmd, &format!("voz {voz:?} pauta {pauta:?}"));
            if let Err(e) = r {
                falhas.push(e);
            }
        }
    }
    assert!(falhas.is_empty(), "{falhas:#?}");
}

#[test]
fn n7_comandos_sem_texto_livre_nao_deixam_entrar_marcacao() {
    // Os demais comandos só carregam números, enums e `char`: a letra hostil é recusada.
    let mut d = DocumentoAberto::abrir_texto("x", &quatro_notas()).unwrap();
    let id = d.ids().id(0, 0, 0).unwrap();
    for letra in ['<', '>', '&', '"', '\'', '\0', '/', 'H'] {
        let c = ComandoDeEdicao::AcrescentarLetra { nota: id, letra };
        recusa_sem_rastro(&mut d, c, &format!("AcrescentarLetra {letra:?}")).unwrap();
    }
    let hostil = alt('<', 0, 4);
    for c in [
        ComandoDeEdicao::DefinirAltura {
            nota: id,
            altura: hostil,
            acidente: None,
        },
        ComandoDeEdicao::AdicionarAltura {
            nota: id,
            altura: hostil,
            acidente: None,
        },
        ComandoDeEdicao::InserirNota {
            compasso: PosCompasso::nova("P1", 0),
            voz: "1".into(),
            pauta: "1".into(),
            posicao: Fracao::nova(0, 1),
            altura: hostil,
            acidente: None,
            figura: figura_da_tecla(5).unwrap(),
            pontos: 0,
        },
    ] {
        recusa_sem_rastro(&mut d, c, "passo hostil").unwrap();
    }
}

#[test]
fn n7_grafias_legitimas_continuam_aceitas() {
    let mut d = DocumentoAberto::abrir_texto("x", &quatro_notas()).unwrap();
    let id = d.ids().id(0, 0, 1).unwrap();
    for g in [
        "sharp",
        "sharp cautionary=\"yes\"",
        "sharp cautionary='yes' parentheses=\"no\"",
        "sharp font-family=\"a>b\"",
        "sharp font-family=\"a&amp;b\"",
        "sharp font-family='a\"b'",
        "sharp color=\"#FF0000\" smufl=\"accidentalSharp\"",
    ] {
        let r = d.aplicar(ComandoDeEdicao::DefinirAltura {
            nota: id,
            altura: alt('D', 1, 4),
            acidente: Some(g.into()),
        });
        assert!(r.is_ok(), "{g}: {:?}", r.err());
        let t = d.exportar().unwrap();
        DocumentoAberto::abrir_texto("y", &t).expect("reabre");
        d.desfazer().unwrap();
    }
}

fn trecho(t: &str) -> String {
    let i = t.find("<accidental").unwrap_or(0);
    let fim = t[i..].find("</note>").map_or(t.len(), |f| i + f);
    t[i..fim].replace('\n', "\\n")
}

#[test]
fn n6_atributos_exoticos_nao_recusam_e_o_inverso_preserva_os_bytes() {
    let casos = [
        (
            "cautionary",
            "<accidental cautionary=\"yes\" parentheses=\"no\">natural</accidental>",
        ),
        (
            "aspas simples",
            "<accidental color='#FF0000' smufl='accidentalNatural'>natural</accidental>",
        ),
        (
            "espacos duplos",
            "<accidental   cautionary=\"yes\"    editorial=\"no\">natural</accidental>",
        ),
        (
            "quebra de linha",
            "<accidental\n  cautionary=\"yes\"\n>natural</accidental>",
        ),
        (
            "maior-que no valor",
            "<accidental font-family=\"a>b\">natural</accidental>",
        ),
        (
            "aspas dentro de aspas",
            "<accidental font-family='a\"b'>natural</accidental>",
        ),
        (
            "ref de entidade no valor",
            "<accidental font-family=\"a&amp;b\">natural</accidental>",
        ),
        (
            "conteudo com espacos",
            "<accidental cautionary=\"yes\"> natural </accidental>",
        ),
        (
            "espaco antes do fecho",
            "<accidental cautionary=\"yes\" >natural</accidental>",
        ),
        (
            "sem atributo e com espacos",
            "<accidental> natural </accidental>",
        ),
        (
            "sharp com attr",
            "<accidental parentheses=\"yes\">sharp</accidental>",
        ),
    ];
    for (nome, acc) in casos {
        let alter = (nome == "sharp com attr").then_some(1);
        let texto = peca(&format!(
            "{}{}{}{}",
            nota('C', None, None),
            nota('F', alter, Some(acc)),
            nota('E', None, None),
            nota('G', None, None)
        ));
        let mut d = DocumentoAberto::abrir_texto("x", &texto).expect(nome);
        let id = d.ids().id(0, 0, 1).unwrap();
        for (u, s) in [
            (UnidadeDeAltura::Semitom, SentidoDaAltura::Acima),
            (UnidadeDeAltura::Semitom, SentidoDaAltura::Abaixo),
            (UnidadeDeAltura::GrauDaEscala, SentidoDaAltura::Acima),
            (UnidadeDeAltura::GrauDaEscala, SentidoDaAltura::Abaixo),
            (UnidadeDeAltura::Oitava, SentidoDaAltura::Acima),
        ] {
            let antes = d.exportar().unwrap();
            let r = d
                .aplicar(ComandoDeEdicao::MudarAltura {
                    nota: id,
                    unidade: u,
                    sentido: s,
                })
                .unwrap_or_else(|e| panic!("[{nome}] {u:?} {s:?} recusado: {e}"));
            d.aplicar(r.inverso.clone())
                .unwrap_or_else(|e| panic!("[{nome}] {u:?} {s:?} inverso recusado: {e}"));
            let depois = d.exportar().unwrap();
            assert_eq!(
                depois,
                antes,
                "[{nome}] {u:?} {s:?}: inverso não restaura os bytes\n antes ={}\n depois={}",
                trecho(&antes),
                trecho(&depois)
            );
            d.desfazer().unwrap();
            d.desfazer().unwrap();
            assert_eq!(d.exportar().unwrap(), antes, "[{nome}] histórico");
        }
    }
}

fn membro(step: char, alter: Option<i8>, oitava: u8, acorde: bool) -> String {
    let a = alter
        .map(|a| format!("<alter>{a}</alter>"))
        .unwrap_or_default();
    let c = if acorde { "<chord/>" } else { "" };
    format!(
        "<note>{c}<pitch><step>{step}</step>{a}<octave>{oitava}</octave></pitch>\
         <duration>16</duration><voice>1</voice><type>whole</type></note>"
    )
}

fn sons(d: &DocumentoAberto) -> Vec<i32> {
    let mut v: Vec<i32> = d
        .documento()
        .notas()
        .filter_map(|n: &Nota| n.altura.map(semitons))
        .collect();
    v.sort();
    v
}

fn semitons(a: Altura) -> i32 {
    let base = match a.passo {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        _ => 11,
    };
    i32::from(a.oitava) * 12 + base + i32::from(a.alter)
}

#[test]
fn n5_inverso_de_remover_nota_de_acorde_com_altura_repetida_se_aplica() {
    let casos = [
        (
            "unissono",
            format!(
                "{}{}",
                membro('C', None, 4, false),
                membro('C', None, 4, true)
            ),
        ),
        (
            "enarmonico",
            format!(
                "{}{}",
                membro('C', Some(1), 4, false),
                membro('D', Some(-1), 4, true)
            ),
        ),
        (
            "unissono triplo",
            format!(
                "{}{}{}",
                membro('C', None, 4, false),
                membro('C', None, 4, true),
                membro('C', None, 4, true)
            ),
        ),
    ];
    for (nome, notas) in casos {
        let texto = peca(&notas);
        let total = DocumentoAberto::abrir_texto("x", &texto)
            .unwrap()
            .documento()
            .notas()
            .count();
        for ordinal in 0..total {
            for retirar in [false, true] {
                let mut d = DocumentoAberto::abrir_texto("x", &texto).unwrap();
                let antes = sons(&d);
                let id = d.ids().id(0, 0, ordinal).unwrap();
                let c = if retirar {
                    ComandoDeEdicao::RetirarAltura { nota: id }
                } else {
                    ComandoDeEdicao::RemoverNota { nota: id }
                };
                let r = d
                    .aplicar(c)
                    .unwrap_or_else(|e| panic!("[{nome}] {ordinal}: {e}"));
                d.aplicar(r.inverso.clone()).unwrap_or_else(|e| {
                    panic!(
                        "[{nome}] ord {ordinal} retirar={retirar}: inverso recusado: {e} ({:?})",
                        r.inverso
                    )
                });
                assert_eq!(
                    sons(&d),
                    antes,
                    "[{nome}] ord {ordinal}: alturas não voltaram"
                );
                DocumentoAberto::abrir_texto("y", &d.exportar().unwrap()).expect("reabre");
            }
        }
    }
}

#[test]
fn n5_adicionar_altura_repetida_continua_recusada_como_comando_publico() {
    let texto = peca(&format!(
        "{}{}",
        membro('C', None, 4, false),
        membro('E', None, 4, true)
    ));
    let mut d = DocumentoAberto::abrir_texto("x", &texto).unwrap();
    let id = d.ids().id(0, 0, 0).unwrap();
    let c = ComandoDeEdicao::AdicionarAltura {
        nota: id,
        altura: alt('E', 0, 4),
        acidente: None,
    };
    let antes = estado(&d);
    assert!(d.aplicar(c).is_err());
    assert_eq!(estado(&d), antes);
}

#[test]
fn n8_definir_altura_sem_mudanca_nao_altera_nem_cria_historico() {
    let casos = [
        (
            "cautionary",
            "<accidental cautionary=\"yes\">natural</accidental>",
            "natural",
            0,
        ),
        (
            "espacos",
            "<accidental   cautionary=\"yes\">natural</accidental>",
            "natural",
            0,
        ),
        (
            "conteudo com espacos",
            "<accidental> natural </accidental>",
            "natural",
            0,
        ),
        (
            "sharp com attr",
            "<accidental parentheses=\"yes\">sharp</accidental>",
            "sharp",
            1,
        ),
    ];
    for (nome, acc, grafia, alter) in casos {
        let a = (alter != 0).then_some(alter);
        let texto = peca(&format!(
            "{}{}{}{}",
            nota('C', None, None),
            nota('F', a, Some(acc)),
            nota('E', None, None),
            nota('G', None, None)
        ));
        let mut d = DocumentoAberto::abrir_texto("x", &texto).unwrap();
        let id = d.ids().id(0, 0, 1).unwrap();
        let entradas = d.entradas_de_desfazer();
        let r = d
            .aplicar(ComandoDeEdicao::DefinirAltura {
                nota: id,
                altura: alt('F', alter, 4),
                acidente: Some(grafia.into()),
            })
            .unwrap();
        assert!(!r.alterou, "[{nome}] alterou=true sem mudança");
        assert_eq!(d.exportar().unwrap(), texto, "[{nome}] texto");
        assert_eq!(d.entradas_de_desfazer(), entradas, "[{nome}] histórico");
        assert!(!d.editado(), "[{nome}] marcado como editado");
    }
}

// --- G2: classificação dos 19 casos --------------------------------------------------------------

const SEMENTE: u64 = 0xDEAD_BEEF_1234_5678;
struct Sorteio(u64);
impl Sorteio {
    fn proximo(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn ate(&mut self, n: usize) -> usize {
        (self.proximo() >> 11) as usize % n.max(1)
    }
}

/// O que o inverso **garante**: cada nota (posição, duração, altura, voz, acorde) e, por voz, a
/// duração total das pausas; as pausas livres podem estar agrupadas de outro jeito e as vigas
/// podem diferir.
fn projecao(d: &DocumentoAberto) -> Vec<String> {
    let mut v = Vec::new();
    for (pi, parte) in d.partitura().partes.iter().enumerate() {
        for (ci, c) in parte.compassos.iter().enumerate() {
            let mut pausas: std::collections::BTreeMap<String, Fracao> = Default::default();
            for n in &c.notas {
                if n.midi.is_none() && !n.adorno {
                    let e = pausas
                        .entry(format!("{}/{}", n.voz, n.pauta))
                        .or_insert(Fracao::ZERO);
                    *e = *e + n.duracao;
                } else {
                    v.push(format!(
                        "{pi}.{ci} {}+{} m{:?} v{} p{} a{} ad{} ac{:?} lig{}{}",
                        n.posicao,
                        n.duracao,
                        n.midi,
                        n.voz,
                        n.pauta,
                        n.acorde as u8,
                        n.adorno as u8,
                        n.acidente,
                        n.ligada_ao_proximo as u8,
                        n.ligada_ao_anterior as u8
                    ));
                }
            }
            for (k, f) in pausas {
                v.push(format!("{pi}.{ci} pausas {k} total {f}"));
            }
        }
    }
    v
}

#[test]
fn g2_inserir_ponto_e_duracao_o_inverso_e_aceito_e_restaura_salvo_pausas_e_vigas() {
    let (mut total, mut exatos, mut agrupamento) = (0, 0, 0);
    for (semente, f) in corpus().into_iter().enumerate() {
        let mut d = DocumentoAberto::abrir_texto(f.id, &f.conteudo).expect("abre");
        let mut s = Sorteio(SEMENTE ^ (semente as u64 + 7));
        let mut feitas = 0;
        for _ in 0..200 {
            if feitas >= 40 {
                break;
            }
            let notas: Vec<IdNota> = d.documento().notas().map(|n| n.id).collect();
            let n = d
                .documento()
                .nota(notas[s.ate(notas.len())])
                .unwrap()
                .clone();
            let id = n.id;
            let cmd = match s.ate(5) {
                0 => ComandoDeEdicao::MudarDuracao {
                    nota: id,
                    figura: figura_da_tecla(1 + s.ate(8) as u8).unwrap(),
                },
                1 => ComandoDeEdicao::AlternarPonto { nota: id },
                k => {
                    let Some(pos) = d.documento().posicao_da_nota(id) else {
                        continue;
                    };
                    let c = PosCompasso::nova(&pos.parte, pos.indice);
                    let posicao = Fracao::nova(s.ate(16) as i64, 4);
                    let figura = figura_da_tecla(2 + s.ate(6) as u8).unwrap();
                    let pontos = s.ate(2) as u8;
                    if k == 2 {
                        ComandoDeEdicao::InserirPausa {
                            compasso: c,
                            voz: n.voz.clone(),
                            pauta: n.pauta.clone(),
                            posicao,
                            figura,
                            pontos,
                        }
                    } else {
                        ComandoDeEdicao::InserirNota {
                            compasso: c,
                            voz: n.voz.clone(),
                            pauta: n.pauta.clone(),
                            posicao,
                            altura: n.altura.unwrap_or(alt('E', 0, 4)),
                            acidente: None,
                            figura,
                            pontos,
                        }
                    }
                }
            };
            let rotulo = format!("{cmd:?}");
            let antes_texto = d.exportar().unwrap();
            let antes = projecao(&d);
            let Ok(r) = d.aplicar(cmd) else { continue };
            if !r.alterou {
                continue;
            }
            feitas += 1;
            total += 1;
            let depois_txt = d.exportar().unwrap();
            let tabela = d.tabela_ids_para(&depois_txt);
            let mut d2 = DocumentoAberto::abrir_texto_com_ids(f.id, &depois_txt, Some(&tabela))
                .expect("reabre");
            d2.aplicar(r.inverso.clone()).unwrap_or_else(|e| {
                panic!("{} {rotulo}: inverso recusado: {e} ({:?})", f.id, r.inverso)
            });
            // Classe 3 (não exato por definição): na fixture `tercina` as notas têm `<type>`
            // incoerente com `<duration>`; editar a duração normaliza o `<type>`, e o inverso
            // restaura a figura, não a duração incoerente de antes.
            if f.id != "tercina" {
                assert_eq!(
                    projecao(&d2),
                    antes,
                    "{} {rotulo}: o inverso não restaurou notas nem a soma das pausas",
                    f.id
                );
            }
            if d2.exportar().unwrap() == antes_texto {
                exatos += 1;
            } else {
                agrupamento += 1;
            }
        }
    }
    eprintln!("G2 classes: total={total} byte_a_byte={exatos} so_pausas_ou_vigas={agrupamento}");
    assert!(total > 50, "o sorteio aplicou poucas edições: {total}");
}
