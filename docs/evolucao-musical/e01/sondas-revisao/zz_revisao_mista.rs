//! Sonda temporária da revisão independente: edições aleatórias de TODOS os tipos de comando
//! sobre todo o corpus, com as invariantes de exportação, atomicidade, desfazer e refazer.
use piano_tutor_core::documento::{
    ComandoDeEdicao, DocumentoAberto, IdNota, Nota, PosCompasso, SentidoDaAltura, UnidadeDeAltura,
    figura_da_tecla,
};
use piano_tutor_core::partitura::{Fracao, Partitura};
use piano_tutor_fixtures::corpus::corpus;

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

#[derive(Debug, PartialEq)]
struct Retrato {
    texto: String,
    notas: Vec<Nota>,
    partitura: Partitura,
    ids: Vec<Option<IdNota>>,
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
        partitura: p.clone(),
        ids,
    }
}

fn comando(d: &DocumentoAberto, s: &mut Sorteio) -> Option<ComandoDeEdicao> {
    let notas: Vec<&Nota> = d.documento().notas().collect();
    if notas.is_empty() {
        return None;
    }
    let n = notas[s.ate(notas.len())];
    let id = n.id;
    Some(match s.ate(12) {
        0 | 1 => ComandoDeEdicao::MudarAltura {
            nota: id,
            unidade: [
                UnidadeDeAltura::Semitom,
                UnidadeDeAltura::GrauDaEscala,
                UnidadeDeAltura::Oitava,
            ][s.ate(3)],
            sentido: [SentidoDaAltura::Acima, SentidoDaAltura::Abaixo][s.ate(2)],
        },
        2 | 3 => ComandoDeEdicao::MudarDuracao {
            nota: id,
            figura: figura_da_tecla(1 + s.ate(8) as u8).expect("figura"),
        },
        4 => ComandoDeEdicao::AlternarPonto { nota: id },
        5 | 6 => ComandoDeEdicao::RemoverNota { nota: id },
        7 => ComandoDeEdicao::RetirarAltura { nota: id },
        8 => ComandoDeEdicao::AcrescentarLetra {
            nota: id,
            letra: ['A', 'B', 'C', 'D', 'E', 'F', 'G'][s.ate(7)],
        },
        9 => ComandoDeEdicao::AcrescentarIntervalo {
            nota: id,
            graus: 2 + s.ate(8) as u8,
        },
        _ => {
            // Inserção numa posição do compasso da nota sorteada.
            let pos = d.documento().posicao_da_nota(id)?;
            let c = PosCompasso::nova(&pos.parte, pos.indice);
            let posicao = Fracao::nova(s.ate(16) as i64, 4);
            let figura = figura_da_tecla(2 + s.ate(6) as u8).expect("figura");
            if s.ate(2) == 0 {
                ComandoDeEdicao::InserirPausa {
                    compasso: c,
                    voz: n.voz.clone(),
                    pauta: n.pauta.clone(),
                    posicao,
                    figura,
                    pontos: s.ate(2) as u8,
                }
            } else {
                ComandoDeEdicao::InserirNota {
                    compasso: c,
                    voz: n.voz.clone(),
                    pauta: n.pauta.clone(),
                    posicao,
                    altura: n.altura.unwrap_or(piano_tutor_core::documento::Altura {
                        passo: 'E',
                        alter: 0,
                        oitava: 4,
                    }),
                    acidente: None,
                    figura,
                    pontos: s.ate(2) as u8,
                }
            }
        }
    })
}

#[test]
fn mista_em_todo_o_corpus() {
    let mut total_ok = 0usize;
    let mut total_recusas = 0usize;
    for (semente, f) in corpus().into_iter().enumerate() {
        let mut d = DocumentoAberto::abrir_texto(f.id, &f.conteudo).expect("abre");
        let original = retrato(&d);
        let mut s = Sorteio(0xDEAD_BEEF_1234_5678 ^ (semente as u64 + 7));
        let mut estados: Vec<Retrato> = Vec::new();
        for passo in 0..400 {
            if estados.len() >= 40 {
                break;
            }
            let Some(cmd) = comando(&d, &mut s) else {
                break;
            };
            let antes = retrato(&d);
            let desfazer_antes = d.entradas_de_desfazer();
            let rotulo = format!("{cmd:?}");
            match d.aplicar(cmd) {
                Ok(r) if r.alterou => {
                    total_ok += 1;
                    let depois = retrato(&d);
                    assert_eq!(d.entradas_de_desfazer(), desfazer_antes + 1, "{} {rotulo}", f.id);
                    // A exportação reabre e confere (modelo e identidades).
                    let reaberto = DocumentoAberto::abrir_texto(f.id, &depois.texto);
                    assert!(
                        reaberto.is_ok(),
                        "{} passo {passo}: exportação não reabre ({:?}) após {rotulo}",
                        f.id,
                        reaberto.err().map(|e| e.to_string())
                    );
                    if let Err(e) = d.conferir_exportacao(&depois.texto) {
                        panic!("{} passo {passo}: conferir falhou: {e} após {rotulo}", f.id);
                    }
                    estados.push(depois);
                }
                Ok(_) => {
                    assert_eq!(retrato(&d), antes, "{} {rotulo}: sem mudança mexeu", f.id);
                    assert_eq!(d.entradas_de_desfazer(), desfazer_antes);
                }
                Err(_) => {
                    total_recusas += 1;
                    assert_eq!(retrato(&d), antes, "{} {rotulo}: recusa mexeu no documento", f.id);
                    assert_eq!(d.entradas_de_desfazer(), desfazer_antes, "{} {rotulo}", f.id);
                }
            }
        }
        let n = estados.len();
        let final_ = retrato(&d);
        for i in (0..n).rev() {
            d.desfazer().unwrap_or_else(|| panic!("{} desfazer {i}", f.id));
            let esperado = if i == 0 { &original } else { &estados[i - 1] };
            assert_eq!(&retrato(&d), esperado, "{}: depois de desfazer {i}", f.id);
        }
        assert_eq!(d.exportar().expect("x"), f.conteudo, "{}", f.id);
        for (i, esperado) in estados.iter().enumerate() {
            d.refazer().unwrap_or_else(|| panic!("{} refazer {i}", f.id));
            assert_eq!(&retrato(&d), esperado, "{}: depois de refazer {i}", f.id);
        }
        assert_eq!(retrato(&d), final_, "{}", f.id);
        eprintln!("{}: {} edicoes ok", f.id, n);
    }
    eprintln!("TOTAL ok={total_ok} recusas={total_recusas}");
}
