//! Ticket 15 (E01): transação e pilha de desfazer/refazer do serviço de documento, sem janela e
//! sem teclado. Cobre A5 (N edições aleatórias desfeitas voltam ao original byte a byte; refeitas
//! chegam ao estado final) em todo o corpus, a recusa que não cria entrada (pré-requisito de A4),
//! o descarte do ramo de refazer, o limite de memória, o agrupamento e o indicador de modificado.

use piano_tutor_core::documento::{
    Altura, AvisoDeEdicao, ComandoDeEdicao, DocumentoAberto, ErroDeEdicao, IdNota, Nota,
    SentidoDaAltura, UnidadeDeAltura,
};
use piano_tutor_core::partitura::Partitura;
use piano_tutor_fixtures::corpus::{corpus, fixture};

const UNIDADES: [UnidadeDeAltura; 3] = [
    UnidadeDeAltura::Semitom,
    UnidadeDeAltura::GrauDaEscala,
    UnidadeDeAltura::Oitava,
];
const SENTIDOS: [SentidoDaAltura; 2] = [SentidoDaAltura::Acima, SentidoDaAltura::Abaixo];

/// Gerador pseudoaleatório determinístico (xorshift64*), para o teste ser reproduzível.
struct Sorteio(u64);

impl Sorteio {
    fn proximo(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn ate(&mut self, n: usize) -> usize {
        usize::try_from(self.proximo() >> 11).expect("cabe") % n.max(1)
    }
}

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

/// Tudo o que o documento sabe, para comparar dois estados por igualdade.
#[derive(Debug, PartialEq)]
struct Retrato {
    texto: String,
    notas: Vec<Nota>,
    sujos: Vec<String>,
    partitura: Partitura,
    ids_total: usize,
    ids: Vec<Option<IdNota>>,
    editado: bool,
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
        ids_total: d.ids().total(),
        ids,
        editado: d.editado(),
    }
}

/// Sorteia até `quantas` edições válidas (as recusadas e as que não mexem em nada não contam)
/// e devolve o retrato depois de cada edição que mudou algo.
fn editar_ao_acaso(d: &mut DocumentoAberto, sorteio: &mut Sorteio, quantas: usize) -> Vec<Retrato> {
    let mut estados = Vec::new();
    let notas = notas_afinadas(d);
    if notas.is_empty() {
        return estados;
    }
    for _ in 0..quantas * 6 {
        if estados.len() == quantas {
            break;
        }
        let nota = notas[sorteio.ate(notas.len())];
        let cmd = mudar(nota, UNIDADES[sorteio.ate(3)], SENTIDOS[sorteio.ate(2)]);
        let entradas = d.entradas_de_desfazer();
        match d.aplicar(cmd) {
            Ok(r) if r.alterou => {
                assert_eq!(d.entradas_de_desfazer(), entradas + 1);
                estados.push(retrato(d));
            }
            Ok(_) | Err(ErroDeEdicao::ForaDosLimites { .. }) => {
                assert_eq!(
                    d.entradas_de_desfazer(),
                    entradas,
                    "sem mudança, sem entrada"
                );
            }
            Err(e) => panic!("recusa inesperada: {e}"),
        }
    }
    estados
}

#[test]
fn a5_desfazer_n_vezes_volta_ao_original_e_refazer_n_vezes_ao_final_em_todo_o_corpus() {
    for (semente, f) in corpus().into_iter().enumerate() {
        let mut d = DocumentoAberto::abrir_texto(f.id, &f.conteudo).expect("abre");
        let original = retrato(&d);
        assert_eq!(original.texto, f.conteudo, "{}", f.id);
        assert!(!d.modificado(), "{}", f.id);
        let mut sorteio = Sorteio(0x9E37_79B9_7F4A_7C15 ^ (semente as u64 + 1));
        let estados = editar_ao_acaso(&mut d, &mut sorteio, 14);
        assert!(
            !estados.is_empty(),
            "{}: nenhuma edição válida sorteada",
            f.id
        );
        let n = estados.len();
        assert_eq!(d.entradas_de_desfazer(), n, "{}", f.id);
        assert!(d.modificado(), "{}", f.id);
        let final_ = retrato(&d);

        // Desfaz passo a passo: cada passo volta exatamente ao estado de antes da edição.
        for i in (0..n).rev() {
            let r = d
                .desfazer()
                .unwrap_or_else(|| panic!("{}: desfazer {i}", f.id));
            assert!(!r.compassos_afetados.is_empty(), "{}", f.id);
            let esperado = if i == 0 { &original } else { &estados[i - 1] };
            assert_eq!(&retrato(&d), esperado, "{}: depois de desfazer {i}", f.id);
        }
        // A5: N desfeitas = o arquivo original, byte a byte.
        assert_eq!(
            d.exportar().expect("exporta").as_bytes(),
            f.conteudo.as_bytes()
        );
        assert!(!d.editado(), "{}: nenhum compasso sujo", f.id);
        assert!(!d.modificado(), "{}: voltou ao estado salvo", f.id);
        assert!(!d.pode_desfazer());
        assert_eq!(d.entradas_de_refazer(), n);
        assert!(d.desfazer().is_none(), "{}: nada mais a desfazer", f.id);
        d.conferir_exportacao(&f.conteudo).expect("confere");

        // Refaz passo a passo até o estado final.
        for (i, esperado) in estados.iter().enumerate() {
            d.refazer()
                .unwrap_or_else(|| panic!("{}: refazer {i}", f.id));
            assert_eq!(&retrato(&d), esperado, "{}: depois de refazer {i}", f.id);
        }
        assert_eq!(retrato(&d), final_, "{}", f.id);
        assert_eq!(
            d.exportar().expect("exporta").as_bytes(),
            final_.texto.as_bytes(),
            "{}: refeitas = estado final byte a byte",
            f.id
        );
        assert!(d.modificado());
        assert!(!d.pode_refazer());
        assert!(d.refazer().is_none());
        // O que foi refeito reabre como o mesmo documento.
        d.conferir_exportacao(&final_.texto)
            .expect("confere o final");
    }
}

fn ode() -> (DocumentoAberto, String) {
    let f = fixture("beethoven-ode");
    (
        DocumentoAberto::abrir_texto(f.id, &f.conteudo).expect("abre"),
        f.conteudo,
    )
}

#[test]
fn comando_recusado_nao_cria_entrada_nem_mexe_no_historico() {
    let (mut d, _) = ode();
    let notas = notas_afinadas(&d);
    d.aplicar(mudar(
        notas[0],
        UnidadeDeAltura::Semitom,
        SentidoDaAltura::Acima,
    ))
    .expect("edita");
    d.desfazer().expect("desfaz");
    let antes = (
        d.entradas_de_desfazer(),
        d.entradas_de_refazer(),
        retrato(&d),
    );
    assert_eq!(antes.0, 0);
    assert_eq!(antes.1, 1);
    // Nota inexistente, nota sem altura e altura fora dos limites: três recusas distintas.
    let sem_altura = d
        .documento()
        .notas()
        .find(|n| n.altura.is_none())
        .map(|n| n.id);
    let mut recusas = vec![
        d.aplicar(mudar(
            IdNota(999_999),
            UnidadeDeAltura::Oitava,
            SentidoDaAltura::Acima,
        )),
        d.aplicar(ComandoDeEdicao::DefinirAltura {
            nota: notas[0],
            altura: Altura {
                passo: 'C',
                alter: 0,
                oitava: 9,
            },
            acidente: None,
        }),
    ];
    if let Some(p) = sem_altura {
        recusas.push(d.aplicar(mudar(p, UnidadeDeAltura::Semitom, SentidoDaAltura::Acima)));
    }
    assert!(recusas.iter().all(Result::is_err), "{recusas:?}");
    assert_eq!(
        d.entradas_de_desfazer(),
        antes.0,
        "a pilha de desfazer não mudou"
    );
    assert_eq!(
        d.entradas_de_refazer(),
        antes.1,
        "o ramo de refazer não foi descartado"
    );
    assert_eq!(retrato(&d), antes.2);
    // Pôr a nota onde ela já está também não cria entrada.
    let atual = d
        .documento()
        .nota(notas[0])
        .and_then(|n| n.altura)
        .expect("altura");
    let acidente = d.documento().acidente_da_nota(notas[0]).expect("acidente");
    let r = d
        .aplicar(ComandoDeEdicao::DefinirAltura {
            nota: notas[0],
            altura: atual,
            acidente,
        })
        .expect("aceita");
    assert!(!r.alterou);
    assert_eq!(d.entradas_de_desfazer(), 0);
    assert_eq!(d.entradas_de_refazer(), 1);
}

#[test]
fn editar_depois_de_desfazer_descarta_o_ramo_de_refazer() {
    let (mut d, original) = ode();
    let notas = notas_afinadas(&d);
    for n in &notas[..3] {
        d.aplicar(mudar(*n, UnidadeDeAltura::Semitom, SentidoDaAltura::Acima))
            .expect("edita");
    }
    d.desfazer().expect("desfaz");
    d.desfazer().expect("desfaz");
    assert_eq!((d.entradas_de_desfazer(), d.entradas_de_refazer()), (1, 2));
    d.aplicar(mudar(
        notas[5],
        UnidadeDeAltura::Oitava,
        SentidoDaAltura::Abaixo,
    ))
    .expect("edita de novo");
    assert_eq!((d.entradas_de_desfazer(), d.entradas_de_refazer()), (2, 0));
    assert!(!d.pode_refazer());
    assert!(d.refazer().is_none());
    // O histórico novo ainda volta ao original.
    d.desfazer().expect("desfaz");
    d.desfazer().expect("desfaz");
    assert_eq!(d.exportar().expect("exporta"), original);
    assert!(!d.modificado());
}

fn descartou(avisos: &[AvisoDeEdicao]) -> usize {
    avisos
        .iter()
        .filter(|a| matches!(a, AvisoDeEdicao::EdicaoMaisAntigaDescartada { .. }))
        .count()
}

#[test]
fn limite_de_memoria_descarta_a_edicao_mais_antiga_com_aviso_e_sem_corromper() {
    let (mut d, original) = ode();
    let notas = notas_afinadas(&d);
    assert!(d.limite_do_historico() > 0, "o limite padrão é documentado");
    // Um limite minúsculo: só cabe a edição mais recente (nunca zero: a última edição sempre
    // pode ser desfeita).
    d.definir_limite_do_historico(1);
    let mut avisos = 0;
    for n in &notas[..5] {
        let r = d
            .aplicar(mudar(*n, UnidadeDeAltura::Semitom, SentidoDaAltura::Acima))
            .expect("edita");
        avisos += descartou(&r.avisos);
    }
    assert_eq!(d.entradas_de_desfazer(), 1);
    assert_eq!(avisos, 4, "cada edição que empurra outra para fora avisa");
    assert_eq!(d.edicoes_descartadas(), 4);
    let depois_de_cinco = d.exportar().expect("exporta");
    // Desfazer a que sobrou restaura só o que ela mudou; nada é corrompido e o documento ainda
    // é o de uma edição a menos, reabrível.
    d.desfazer().expect("desfaz a última");
    assert!(!d.pode_desfazer());
    let t = d.exportar().expect("exporta");
    assert_ne!(t, depois_de_cinco);
    assert_ne!(
        t, original,
        "as quatro edições mais antigas não podem mais ser desfeitas"
    );
    assert!(
        d.modificado(),
        "ainda há edição que o histórico não alcança"
    );
    DocumentoAberto::abrir_texto("t", &t).expect("o texto continua válido");
    d.refazer().expect("refaz");
    assert_eq!(d.exportar().expect("exporta"), depois_de_cinco);

    // Limite folgado: nada é descartado.
    let (mut d, _) = ode();
    for n in &notas[..5] {
        let r = d
            .aplicar(mudar(*n, UnidadeDeAltura::Semitom, SentidoDaAltura::Acima))
            .expect("edita");
        assert_eq!(descartou(&r.avisos), 0);
    }
    assert_eq!(d.entradas_de_desfazer(), 5);
    assert_eq!(d.edicoes_descartadas(), 0);
    assert!(d.bytes_do_historico() > 0);
    assert!(d.bytes_do_historico() <= d.limite_do_historico());
}

#[test]
fn reduzir_o_limite_descarta_as_mais_antigas_na_hora() {
    let (mut d, _) = ode();
    let notas = notas_afinadas(&d);
    for n in &notas[..4] {
        d.aplicar(mudar(*n, UnidadeDeAltura::Semitom, SentidoDaAltura::Acima))
            .expect("edita");
    }
    assert_eq!(d.entradas_de_desfazer(), 4);
    d.definir_limite_do_historico(1);
    assert_eq!(
        d.entradas_de_desfazer(),
        1,
        "reduzir o limite aplica na hora"
    );
    assert_eq!(d.edicoes_descartadas(), 3);
}

#[test]
fn grupo_de_edicoes_e_uma_unica_entrada_desfeita_de_uma_vez() {
    let (mut d, original) = ode();
    let notas = notas_afinadas(&d);
    let antes = retrato(&d);
    d.iniciar_grupo();
    // Seis notas espalhadas pela peça: compassos diferentes.
    for i in 0..6 {
        d.aplicar(mudar(
            notas[i * notas.len() / 6],
            UnidadeDeAltura::Semitom,
            SentidoDaAltura::Acima,
        ))
        .expect("edita");
    }
    // Duas edições na mesma nota dentro do grupo também cabem numa entrada só.
    d.aplicar(mudar(
        notas[0],
        UnidadeDeAltura::Oitava,
        SentidoDaAltura::Acima,
    ))
    .expect("edita");
    assert!(d.encerrar_grupo().is_empty());
    assert_eq!(d.entradas_de_desfazer(), 1);
    let meio = retrato(&d);
    let r = d.desfazer().expect("desfaz o gesto");
    assert!(r.compassos_afetados.len() > 1, "{:?}", r.compassos_afetados);
    assert_eq!(
        r.foco,
        Some(notas[0]),
        "a seleção volta à primeira nota do gesto"
    );
    assert_eq!(retrato(&d), antes);
    assert_eq!(d.exportar().expect("exporta"), original);
    d.refazer().expect("refaz o gesto");
    assert_eq!(retrato(&d), meio);

    // Grupo sem edição bem-sucedida não cria entrada (e recusa dentro do grupo também não).
    let (mut v, _) = ode();
    v.iniciar_grupo();
    assert!(
        v.aplicar(mudar(
            IdNota(999_999),
            UnidadeDeAltura::Semitom,
            SentidoDaAltura::Acima
        ))
        .is_err()
    );
    v.encerrar_grupo();
    assert_eq!(v.entradas_de_desfazer(), 0);

    // Dentro de um grupo aberto não se desfaz (o gesto ainda não terminou).
    let (mut g, _) = ode();
    g.aplicar(mudar(
        notas[0],
        UnidadeDeAltura::Semitom,
        SentidoDaAltura::Acima,
    ))
    .expect("edita");
    g.iniciar_grupo();
    assert!(g.desfazer().is_none());
    g.encerrar_grupo();
    assert!(g.desfazer().is_some());
}

#[test]
fn indicador_de_modificado_acompanha_o_estado_salvo() {
    let (mut d, _) = ode();
    let notas = notas_afinadas(&d);
    assert!(!d.modificado());
    d.aplicar(mudar(
        notas[0],
        UnidadeDeAltura::Semitom,
        SentidoDaAltura::Acima,
    ))
    .expect("edita");
    assert!(d.modificado());
    d.desfazer().expect("desfaz");
    assert!(!d.modificado(), "voltou ao estado aberto");
    d.refazer().expect("refaz");
    assert!(d.modificado());

    // Gravou neste ponto: limpo; editar suja; desfazer a edição volta a limpo (mesmo com o
    // compasso ainda marcado como sujo, porque o estado salvo já tinha a edição).
    d.marcar_salvo();
    assert!(!d.modificado());
    d.aplicar(mudar(
        notas[1],
        UnidadeDeAltura::Semitom,
        SentidoDaAltura::Abaixo,
    ))
    .expect("edita");
    assert!(d.modificado());
    d.desfazer().expect("desfaz");
    assert!(!d.modificado());
    assert!(
        d.editado(),
        "o compasso editado antes de gravar continua sujo"
    );
    // Desfazer além do salvo: modificado outra vez.
    d.desfazer().expect("desfaz");
    assert!(d.modificado());
    d.refazer().expect("refaz");
    assert!(!d.modificado());

    // Estado salvo fora da pilha (ramo descartado): nunca mais limpo por engano.
    d.aplicar(mudar(
        notas[2],
        UnidadeDeAltura::Oitava,
        SentidoDaAltura::Acima,
    ))
    .expect("edita");
    d.desfazer().expect("desfaz");
    assert!(!d.modificado());
    d.desfazer().expect("desfaz além do salvo");
    d.aplicar(mudar(
        notas[3],
        UnidadeDeAltura::Oitava,
        SentidoDaAltura::Acima,
    ))
    .expect("edita em outro ramo");
    d.desfazer().expect("desfaz");
    assert!(d.modificado(), "o estado de antes não é o salvo");
}

#[test]
fn a_identidade_das_notas_e_o_foco_da_selecao_sobrevivem_ao_desfazer() {
    let (mut d, _) = ode();
    let notas = notas_afinadas(&d);
    let ids_antes: Vec<IdNota> = d.documento().notas().map(|n| n.id).collect();
    let r = d
        .aplicar(mudar(
            notas[7],
            UnidadeDeAltura::Oitava,
            SentidoDaAltura::Acima,
        ))
        .expect("edita");
    let u = d.desfazer().expect("desfaz");
    assert_eq!(u.foco, Some(r.nota));
    assert_eq!(u.notas, vec![r.nota]);
    assert_eq!(u.compassos_afetados, r.compassos_afetados);
    let f = d.refazer().expect("refaz");
    assert_eq!(f.foco, Some(r.nota));
    let ids_depois: Vec<IdNota> = d.documento().notas().map(|n| n.id).collect();
    assert_eq!(ids_antes, ids_depois);
    assert_eq!(
        d.documento().nota(r.nota).and_then(|n| n.altura),
        Some(r.depois)
    );
}
