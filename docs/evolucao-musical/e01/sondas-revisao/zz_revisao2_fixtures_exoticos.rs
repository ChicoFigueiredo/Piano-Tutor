//! SONDAS TEMPORARIAS da segunda revisao independente da E01 (G1, G2, R4, R6, injecao de marcacao).
use piano_tutor_core::documento::{
    Altura, ComandoDeEdicao, DocumentoAberto, PosCompasso, SentidoDaAltura, TETO_DO_CONTADOR_DE_IDS,
    TabelaIds, UnidadeDeAltura, figura_da_tecla,
};
use piano_tutor_core::partitura::Fracao;

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
    let a = alter.map(|a| format!("<alter>{a}</alter>")).unwrap_or_default();
    let ac = acc.unwrap_or("");
    format!(
        "<note><pitch><step>{step}</step>{a}<octave>4</octave></pitch><duration>4</duration>\
         <voice>1</voice><type>quarter</type>{ac}</note>"
    )
}

#[test]
fn zz_g1_acidentes_com_atributos_exoticos() {
    let casos = [
        ("cautionary", "<accidental cautionary=\"yes\" parentheses=\"no\">natural</accidental>"),
        ("aspas simples", "<accidental color='#FF0000' smufl='accidentalNatural'>natural</accidental>"),
        ("espacos duplos", "<accidental   cautionary=\"yes\"    editorial=\"no\">natural</accidental>"),
        ("quebra de linha", "<accidental\n  cautionary=\"yes\"\n>natural</accidental>"),
        ("maior-que no valor", "<accidental font-family=\"a>b\">natural</accidental>"),
        ("aspas dentro de aspas", "<accidental font-family='a\"b'>natural</accidental>"),
        ("ref de entidade no valor", "<accidental font-family=\"a&amp;b\">natural</accidental>"),
        ("conteudo com espacos", "<accidental cautionary=\"yes\"> natural </accidental>"),
        ("sharp com attr", "<accidental parentheses=\"yes\">sharp</accidental>"),
    ];
    for (nome, acc) in casos {
        let alter = if nome == "sharp com attr" { Some(1) } else { Some(0) };
        let texto = peca(&format!(
            "{}{}{}{}",
            nota('C', None, None),
            nota('F', alter.filter(|a| *a != 0), Some(acc)),
            nota('E', None, None),
            nota('G', None, None)
        ));
        let mut d = match DocumentoAberto::abrir_texto("x", &texto) {
            Ok(d) => d,
            Err(e) => {
                println!("SONDA g1 [{nome}] nem abre: {e}");
                continue;
            }
        };
        let id = d.ids().id(0, 0, 1).expect("id");
        let mut exato = 0;
        let mut recusado = 0;
        let mut difere = 0;
        for (u, s) in [
            (UnidadeDeAltura::Semitom, SentidoDaAltura::Acima),
            (UnidadeDeAltura::Semitom, SentidoDaAltura::Abaixo),
            (UnidadeDeAltura::GrauDaEscala, SentidoDaAltura::Acima),
            (UnidadeDeAltura::GrauDaEscala, SentidoDaAltura::Abaixo),
            (UnidadeDeAltura::Oitava, SentidoDaAltura::Acima),
        ] {
            let antes = d.exportar().unwrap();
            let r = match d.aplicar(ComandoDeEdicao::MudarAltura { nota: id, unidade: u, sentido: s }) {
                Ok(r) => r,
                Err(e) => {
                    println!("SONDA g1 [{nome}] {u:?} {s:?} recusado: {e}");
                    continue;
                }
            };
            let meio = d.exportar().unwrap();
            match d.aplicar(r.inverso.clone()) {
                Ok(_) => {
                    let depois = d.exportar().unwrap();
                    if depois == antes {
                        exato += 1;
                    } else {
                        difere += 1;
                        println!("SONDA g1 [{nome}] {u:?} {s:?} inverso difere:\n  antes ={}\n  meio  ={}\n  depois={}",
                            trecho(&antes), trecho(&meio), trecho(&depois));
                    }
                }
                Err(e) => {
                    recusado += 1;
                    println!("SONDA g1 [{nome}] {u:?} {s:?} inverso RECUSADO: {e} inv={:?}", r.inverso);
                }
            }
            // volta ao estado inicial pelo historico para a proxima combinacao
            d.desfazer();
            d.desfazer();
            assert_eq!(d.exportar().unwrap(), antes, "[{nome}] historico");
        }
        println!("SONDA g1 [{nome}] exato={exato} recusado={recusado} difere={difere}");
    }
}

fn trecho(t: &str) -> String {
    let i = t.find("<accidental").map_or(0, |i| i.saturating_sub(0));
    let fim = t[i..].find("</note>").map_or(t.len(), |f| i + f);
    t[i..fim].replace('\n', "\\n")
}

#[test]
fn zz_g1_definir_altura_igual_com_nome_so_conta_como_mudanca() {
    let acc = "<accidental cautionary=\"yes\">natural</accidental>";
    let texto = peca(&format!(
        "{}{}{}{}",
        nota('C', None, None),
        nota('F', None, Some(acc)),
        nota('E', None, None),
        nota('G', None, None)
    ));
    let mut d = DocumentoAberto::abrir_texto("x", &texto).unwrap();
    let id = d.ids().id(0, 0, 1).unwrap();
    let entradas = d.entradas_de_desfazer();
    let r = d
        .aplicar(ComandoDeEdicao::DefinirAltura {
            nota: id,
            altura: Altura { passo: 'F', alter: 0, oitava: 4 },
            acidente: Some("natural".into()),
        })
        .unwrap();
    println!(
        "SONDA g1 DefinirAltura igual com nome so: alterou={} texto_igual={} entradas {}->{}",
        r.alterou,
        d.exportar().unwrap() == texto,
        entradas,
        d.entradas_de_desfazer()
    );
}

#[test]
fn zz_g2_acorde_com_unissono_e_enarmonico() {
    // Acorde C4 + C4 (unissono) e C#4 + Db4 (enarmonico)
    let casos = [
        ("unissono", "<note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note><note><chord/><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note>"),
        ("enarmonico", "<note><pitch><step>C</step><alter>1</alter><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note><note><chord/><pitch><step>D</step><alter>-1</alter><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note>"),
    ];
    for (nome, notas) in casos {
        let texto = peca(notas);
        let mut d = DocumentoAberto::abrir_texto("x", &texto).unwrap();
        for ordinal in [1usize, 0] {
            let mut d = d.clone_para_sonda(&texto);
            let id = d.ids().id(0, 0, ordinal).unwrap();
            let antes = d.exportar().unwrap();
            match d.aplicar(ComandoDeEdicao::RemoverNota { nota: id }) {
                Err(e) => println!("SONDA g2 [{nome}] remover ord {ordinal} recusado: {e}"),
                Ok(r) => match d.aplicar(r.inverso.clone()) {
                    Ok(_) => println!(
                        "SONDA g2 [{nome}] ord {ordinal}: inverso aplicado, texto_igual={}",
                        d.exportar().unwrap() == antes
                    ),
                    Err(e) => println!(
                        "SONDA g2 [{nome}] ord {ordinal}: inverso RECUSADO: {e} inv={:?}",
                        r.inverso
                    ),
                },
            }
        }
        let _ = &mut d;
    }
}

trait ClonePraSonda {
    fn clone_para_sonda(&self, texto: &str) -> DocumentoAberto;
}
impl ClonePraSonda for DocumentoAberto {
    fn clone_para_sonda(&self, texto: &str) -> DocumentoAberto {
        DocumentoAberto::abrir_texto("x", texto).unwrap()
    }
}

#[test]
fn zz_injecao_de_marcacao_pelo_acidente_em_inserir_nota_e_acorde() {
    let texto = peca(&format!(
        "{}{}{}{}",
        nota('C', None, None),
        nota('D', None, None),
        nota('E', None, None),
        nota('F', None, None)
    ));
    for hostil in [
        "sharp</accidental><injetado/><accidental>sharp",
        "sharp a=\"1\"><injetado/",
        "sharp onclick=\"x\"",
        "",
        "sharp ",
    ] {
        let mut d = DocumentoAberto::abrir_texto("x", &texto).unwrap();
        let c = PosCompasso::nova("P1", 0);
        let r = d.aplicar(ComandoDeEdicao::InserirNota {
            compasso: c.clone(),
            voz: "1".into(),
            pauta: "1".into(),
            posicao: Fracao::nova(0, 1),
            altura: Altura { passo: 'G', alter: 1, oitava: 4 },
            acidente: Some(hostil.into()),
            figura: figura_da_tecla(5).unwrap(),
            pontos: 0,
        });
        match r {
            Err(e) => println!("SONDA inj InserirNota {hostil:?}: recusado ({e})"),
            Ok(_) => {
                let t = d.exportar().unwrap();
                let reabre = DocumentoAberto::abrir_texto("x", &t);
                println!(
                    "SONDA inj InserirNota {hostil:?}: ACEITO; contem '<injetado' = {}; reabre={:?}; conferir={:?}",
                    t.contains("<injetado"),
                    reabre.as_ref().map(|_| ()).map_err(|e| e.to_string()),
                    d.conferir_exportacao(&t).map_err(|e| e.to_string())
                );
            }
        }
        // AdicionarAltura
        let mut d = DocumentoAberto::abrir_texto("x", &texto).unwrap();
        let id = d.ids().id(0, 0, 0).unwrap();
        let r = d.aplicar(ComandoDeEdicao::AdicionarAltura {
            nota: id,
            altura: Altura { passo: 'E', alter: 1, oitava: 4 },
            acidente: Some(hostil.into()),
        });
        match r {
            Err(e) => println!("SONDA inj AdicionarAltura {hostil:?}: recusado ({e})"),
            Ok(_) => {
                let t = d.exportar().unwrap();
                println!(
                    "SONDA inj AdicionarAltura {hostil:?}: ACEITO; contem '<injetado' = {}; reabre={:?}",
                    t.contains("<injetado"),
                    DocumentoAberto::abrir_texto("x", &t).map(|_| ()).map_err(|e| e.to_string())
                );
            }
        }
    }
}

#[test]
fn zz_r4_contador_perto_do_teto() {
    let texto = peca(&format!(
        "{}{}{}{}",
        nota('C', None, None),
        nota('D', None, None),
        nota('E', None, None),
        nota('F', None, None)
    ));
    for folga in [0u64, 1, 5, (1 << 20) - 1, 1 << 20, (1 << 20) + 1] {
        let proximo = TETO_DO_CONTADOR_DE_IDS - folga;
        let mut linhas = format!("piano-tutor-ids v1\nproximo={proximo}\n");
        // ids 0..3 nas 4 posicoes: pegar a tabela de um doc aberto e trocar o proximo
        let d0 = DocumentoAberto::abrir_texto("x", &texto).unwrap();
        let base = d0.tabela_ids().para_texto();
        for l in base.lines() {
            if !l.starts_with("proximo=") && !l.starts_with("piano-tutor-ids") {
                linhas.push_str(l);
                linhas.push('\n');
            }
        }
        let t = match TabelaIds::de_texto(&linhas) {
            Ok(t) => t,
            Err(e) => {
                println!("SONDA r4 folga={folga}: tabela invalida: {e}");
                continue;
            }
        };
        let mut d = match DocumentoAberto::abrir_texto_com_ids("x", &texto, Some(&t)) {
            Ok(d) => d,
            Err(e) => {
                println!("SONDA r4 folga={folga}: abrir com tabela falhou: {e}");
                continue;
            }
        };
        let id = d.ids().id(0, 0, 1).unwrap();
        let r_dur = d.aplicar(ComandoDeEdicao::MudarDuracao { nota: id, figura: figura_da_tecla(4).unwrap() });
        let r_ins = d.aplicar(ComandoDeEdicao::InserirPausa {
            compasso: PosCompasso::nova("P1", 0),
            voz: "1".into(),
            pauta: "1".into(),
            posicao: Fracao::nova(0, 1),
            figura: figura_da_tecla(5).unwrap(),
            pontos: 0,
        });
        let t2 = d.tabela_ids().para_texto();
        println!(
            "SONDA r4 folga={folga}: duracao={:?} inserir_pausa={:?} proximo_final_ok={}",
            r_dur.as_ref().map(|r| r.alterou).map_err(|e| e.to_string()),
            r_ins.as_ref().map(|r| r.alterou).map_err(|e| e.to_string()),
            TabelaIds::de_texto(&t2).is_ok()
        );
    }
}
