//! Ticket 19 (E01, D13): letra, cifra e dedilhado só **mostrados e preservados**, mais a lista de
//! elementos que o editor não desenha. Cobre A3 (reforço: três estrofes no compasso editado),
//! A15 (lista de não suportados e golden byte a byte fora do compasso editado), a preservação de
//! dinâmica, ornamento e demais elementos reconhecidos, e T8 (elemento desconhecido nunca aborta
//! a abertura). A parte de desenho (A18) está nos testes do app.

use piano_tutor_core::documento::{
    ComandoDeEdicao, DocumentoAberto, Exibicao, IdNota, SentidoDaAltura, UnidadeDeAltura,
};
use piano_tutor_fixtures::corpus::{corpus, fixture};
use piano_tutor_fixtures::exibicao::COMBINADA;
use piano_tutor_fixtures::harness::comparar_bytes;

fn abrir(texto: &str) -> DocumentoAberto {
    DocumentoAberto::abrir_texto("t", texto).expect("abre")
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

fn contar(texto: &str, trecho: &str) -> usize {
    texto.matches(trecho).count()
}

fn quantidade(ex: &Exibicao, nome: &str) -> usize {
    ex.nao_suportados()
        .iter()
        .find(|e| e.nome == nome)
        .map_or(0, |e| e.quantidade)
}

#[test]
fn exibicao_combinada_tem_o_que_o_ticket_exige() {
    let d = abrir(COMBINADA);
    let ex = d.documento().exibicao();
    // Letra: 3 estrofes, 30 sílabas (4+3 notas de 3 versos nos compassos 1 e 2, 2 no 3, 1 no 4).
    assert_eq!(ex.estrofes(), ["1", "2", "3"]);
    assert_eq!(ex.total_de_silabas(), 30);
    // Dedilhado: 16 (inclusive a troca de dedo 1-2 do último compasso).
    assert_eq!(ex.total_de_dedos(), 16);
    // Cifra: 6, todas ligadas a uma nota.
    assert_eq!(ex.cifras().len(), 6);
    assert!(ex.cifras().iter().all(|c| c.nota.is_some()));
    let nomes: Vec<&str> = ex.cifras().iter().map(|c| c.texto.as_str()).collect();
    assert_eq!(nomes, ["C", "G7/B", "Am", "Bb9", "F", "C"]);
    // Não suportados: nome e quantidade.
    for (nome, n) in [
        ("dynamics", 1),
        ("trill-mark", 1),
        ("staccato", 1),
        ("fermata", 1),
        ("grouping", 1),
        ("x-extensao-futura", 1),
    ] {
        assert_eq!(
            quantidade(&ex, nome),
            n,
            "{nome}: {:?}",
            ex.nao_suportados()
        );
    }
    assert_eq!(ex.nao_suportados().len(), 6);
}

#[test]
fn cada_texto_esta_associado_a_uma_nota_que_existe() {
    let d = abrir(COMBINADA);
    let ex = d.documento().exibicao();
    for (id, e) in ex.notas() {
        assert!(d.documento().nota(id).is_some(), "{id:?}");
        assert!(!e.silabas.is_empty() || !e.dedos.is_empty());
    }
    for c in ex.cifras() {
        let id = c.nota.expect("âncora");
        let pos = d.documento().posicao_da_nota(id).expect("posição");
        assert_eq!(
            pos.indice, c.compasso,
            "a cifra {} está no compasso da nota",
            c.texto
        );
    }
}

#[test]
fn a3_editar_altura_em_compasso_com_tres_estrofes_preserva_silabas_cifra_e_dedilhado() {
    let base = abrir(COMBINADA);
    let antes = base.documento().exibicao();
    let ids = notas_afinadas(&base);
    assert!(ids.len() >= 20);
    for id in ids {
        for (u, s) in [
            (UnidadeDeAltura::Semitom, SentidoDaAltura::Acima),
            (UnidadeDeAltura::GrauDaEscala, SentidoDaAltura::Abaixo),
            (UnidadeDeAltura::Oitava, SentidoDaAltura::Acima),
        ] {
            let mut d = abrir(COMBINADA);
            if d.aplicar(mudar(id, u, s)).is_err() {
                continue;
            }
            let depois_no_modelo = d.documento().exibicao();
            assert_eq!(antes, depois_no_modelo, "no documento editado ({id:?})");
            // E no arquivo exportado e reaberto: mesmas sílabas, cifras e dedos, nas mesmas
            // notas (a identidade das notas é a da ordem do documento, que a edição de altura
            // não muda).
            let saida = d.exportar().expect("exporta");
            let reaberto = abrir(&saida).documento().exibicao();
            assert_eq!(antes, reaberto, "no arquivo exportado ({id:?})");
        }
    }
}

#[test]
fn a15_elementos_sem_suporte_sobrevivem_e_so_o_compasso_editado_muda() {
    let mut d = abrir(COMBINADA);
    let antes = d.documento().exibicao();
    // A nota do piano, compasso 3, que carrega o `x-extensao-futura`: a própria nota editada
    // preserva o filho desconhecido.
    let alvo = d
        .documento()
        .notas()
        .find(|n| COMBINADA[n.faixa.clone()].contains("x-extensao-futura"))
        .expect("nota com extensão")
        .id;
    d.aplicar(mudar(
        alvo,
        UnidadeDeAltura::Semitom,
        SentidoDaAltura::Acima,
    ))
    .expect("edita");
    let saida = d.exportar().expect("exporta");
    let r = comparar_bytes(COMBINADA, &saida).expect("compara");
    assert!(!r.identicos);
    assert!(!r.fora_dos_compassos, "o texto fora dos compassos mudou");
    assert_eq!(
        r.compassos_diferentes.len(),
        1,
        "{:?}",
        r.compassos_diferentes
    );
    assert_eq!(r.compassos_diferentes[0].parte, "P2");
    assert_eq!(r.compassos_diferentes[0].indice, 2);
    for trecho in [
        "<x-extensao-futura valor=\"1\"/>",
        "<grouping type=\"start\" number=\"1\"/>",
        "<staccato/>",
        "<dynamics><p/></dynamics>",
        "<trill-mark/>",
        "<fermata/>",
    ] {
        assert_eq!(
            contar(&saida, trecho),
            contar(COMBINADA, trecho),
            "{trecho}"
        );
    }
    assert_eq!(
        antes.nao_suportados(),
        abrir(&saida).documento().exibicao().nao_suportados()
    );
}

#[test]
fn dinamica_ornamento_e_desconhecidos_do_corpus_sobrevivem_a_edicao_de_altura() {
    for id in ["dinamica", "ornamento", "desconhecido"] {
        let f = fixture(id);
        let base = abrir(&f.conteudo);
        let antes = base.documento().exibicao();
        assert!(
            !antes.nao_suportados().is_empty(),
            "{id}: esperava elementos sem suporte"
        );
        let mut editado = false;
        for nota in notas_afinadas(&base) {
            let mut d = abrir(&f.conteudo);
            if d.aplicar(mudar(
                nota,
                UnidadeDeAltura::Semitom,
                SentidoDaAltura::Acima,
            ))
            .is_err()
            {
                continue;
            }
            editado = true;
            let saida = d.exportar().expect("exporta");
            let r = comparar_bytes(&f.conteudo, &saida).expect("compara");
            assert!(!r.fora_dos_compassos, "{id}");
            assert_eq!(r.compassos_diferentes.len(), 1, "{id}");
            let depois = abrir(&saida).documento().exibicao();
            assert_eq!(antes.nao_suportados(), depois.nao_suportados(), "{id}");
        }
        assert!(editado, "{id}: nenhuma edição aplicada");
    }
}

#[test]
fn t8_elemento_desconhecido_nunca_aborta_a_abertura_e_e_listado() {
    let f = fixture("desconhecido");
    let d = abrir(&f.conteudo);
    let ex = d.documento().exibicao();
    let desconhecidos: Vec<&str> = ex
        .nao_suportados()
        .iter()
        .filter(|e| !e.reconhecido)
        .map(|e| e.nome.as_str())
        .collect();
    assert!(
        desconhecidos.contains(&"x-extensao-futura"),
        "{desconhecidos:?}"
    );
    assert!(
        desconhecidos.contains(&"x-elemento-inexistente"),
        "{desconhecidos:?}"
    );
    // Sem edição, a exportação é idêntica ao original (A1), inclusive os desconhecidos.
    assert_eq!(d.exportar().expect("exporta"), f.conteudo);
}

#[test]
fn o_leitor_nao_perde_nenhum_dedilhado_cifra_ou_silaba_do_corpus() {
    for f in corpus() {
        let d = abrir(&f.conteudo);
        let ex = d.documento().exibicao();
        assert_eq!(
            ex.total_de_dedos(),
            contar(&f.conteudo, "<fingering"),
            "{}",
            f.id
        );
        assert_eq!(
            ex.cifras().len(),
            contar(&f.conteudo, "<harmony"),
            "{}",
            f.id
        );
        let com_texto = f
            .conteudo
            .split("<lyric")
            .skip(1)
            .filter(|l| {
                l.split("</lyric>")
                    .next()
                    .is_some_and(|c| c.contains("<text"))
            })
            .count();
        assert_eq!(ex.total_de_silabas(), com_texto, "{}", f.id);
    }
}
