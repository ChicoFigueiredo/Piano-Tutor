//! Testes de integração do Pacote de Curso, só com fixtures sintéticas.
#![allow(clippy::unwrap_used)] // testes: falhar com panic é o comportamento desejado

mod comum;

use std::fs;

use comum::*;
use piano_tutor_curso::{
    ChavePrivada, Confianca, ErroPacote, EstadoConteudo, Manifesto, Momento, TipoLicao,
    cursos_instalados, empacotar, instalar, ler_pacote, ler_pacote_de_arquivo,
};

fn chave() -> ChavePrivada {
    ChavePrivada::gerar()
}

// ---------- ida e volta ----------

#[test]
fn empacotar_e_ler_devolve_o_mesmo_curso() {
    let chave = chave();
    let bytes = pacote_sintetico(&chave);
    let pacote = ler_pacote(&bytes).unwrap();

    let m = &pacote.manifesto;
    assert_eq!(m.id, "teste.sintetico");
    assert_eq!(m.titulo, "Curso Sintético de Teste");
    assert_eq!(m.autor.nome, "Autor Sintético");
    assert_eq!(m.licenciado.nome, "Aluno de Teste");
    assert_eq!(m.assinatura.chave_publica, chave.publica().para_hex());
    assert_eq!(pacote.chave_publica, chave.publica());
    assert_eq!(m.secoes.len(), 1);
    assert_eq!(m.secoes[0].licoes.len(), 2);
    assert_eq!(m.secoes[0].licoes[0].tipo, TipoLicao::Exercicio);
    assert_eq!(m.exercicios().count(), 2);

    // arquivos: 2 partituras + áudio + alinhamento + roteiro
    assert_eq!(m.arquivos.len(), 5);
    assert!(pacote.arquivo("partituras/tres-notas.musicxml").is_some());
    assert!(
        pacote.arquivo("manifesto.toml").is_none(),
        "manifesto não é arquivo listado"
    );

    let alinhamento = pacote.alinhamento("tres-notas").unwrap();
    assert_eq!(alinhamento.notas.len(), 3);
    assert_eq!(alinhamento.notas[1].midi, 62);

    let roteiro = pacote.roteiro.as_ref().unwrap();
    assert_eq!(roteiro.falas.len(), 2);
    assert_eq!(roteiro.falas[1].momento, Momento::Passo);
    assert_eq!(roteiro.falas_aprovadas().count(), 1);
}

#[test]
fn ler_de_arquivo_funciona_e_empacotar_e_deterministico() {
    let chave = chave();
    let bytes = pacote_sintetico(&chave);
    assert_eq!(
        bytes,
        pacote_sintetico(&chave),
        "mesma origem e chave dão os mesmos bytes"
    );
    let pasta = tempfile::tempdir().unwrap();
    let caminho = pasta.path().join("c.ptcurso");
    fs::write(&caminho, &bytes).unwrap();
    assert!(ler_pacote_de_arquivo(&caminho).is_ok());
    assert!(matches!(
        ler_pacote_de_arquivo(&pasta.path().join("nao-existe.ptcurso")),
        Err(ErroPacote::Io(_))
    ));
}

#[test]
fn empacotar_ignora_assinatura_e_arquivos_ocultos_da_origem() {
    let chave = chave();
    let pasta = tempfile::tempdir().unwrap();
    escrever_pasta_origem(pasta.path());
    fs::write(pasta.path().join("assinatura.ed25519"), b"velha").unwrap();
    fs::write(pasta.path().join(".rascunho"), b"x").unwrap();
    let pacote = ler_pacote(&empacotar(pasta.path(), &chave).unwrap()).unwrap();
    assert_eq!(pacote.manifesto.arquivos.len(), 5);
}

// ---------- adulteração ----------

#[test]
fn assinatura_adulterada_e_rejeitada() {
    let mut e = entradas(&pacote_sintetico(&chave()));
    let mut assinatura = e
        .iter()
        .find(|(n, _)| n == "assinatura.ed25519")
        .unwrap()
        .1
        .clone();
    assinatura[0] ^= 0xff;
    trocar(&mut e, "assinatura.ed25519", assinatura);
    assert!(matches!(
        ler_pacote(&montar(&e)),
        Err(ErroPacote::AssinaturaInvalida)
    ));
}

#[test]
fn assinatura_com_tamanho_errado_e_rejeitada() {
    let mut e = entradas(&pacote_sintetico(&chave()));
    trocar(&mut e, "assinatura.ed25519", vec![0u8; 10]);
    assert!(matches!(
        ler_pacote(&montar(&e)),
        Err(ErroPacote::AssinaturaMalformada(_))
    ));
}

#[test]
fn manifesto_alterado_depois_de_assinar_e_rejeitado() {
    let mut e = entradas(&pacote_sintetico(&chave()));
    let manifesto = e
        .iter()
        .find(|(n, _)| n == "manifesto.toml")
        .unwrap()
        .1
        .clone();
    let alterado = String::from_utf8(manifesto).unwrap().replace(
        "Curso Sintético de Teste",
        "Curso Sintético de Teste (alterado)",
    );
    trocar(&mut e, "manifesto.toml", alterado.into_bytes());
    assert!(matches!(
        ler_pacote(&montar(&e)),
        Err(ErroPacote::AssinaturaInvalida)
    ));
}

#[test]
fn assinatura_de_outra_chave_e_rejeitada() {
    // Manifesto declara a chave A, mas foi assinado pela chave B.
    let a = chave();
    let b = chave();
    let texto = manifesto_minimo(&a, 1, "");
    let bytes = montar_assinado(&b, &texto, &[]);
    assert!(matches!(
        ler_pacote(&bytes),
        Err(ErroPacote::AssinaturaInvalida)
    ));
}

#[test]
fn arquivo_alterado_quebra_o_hash() {
    // Mesmo tamanho, conteúdo diferente: só o SHA-256 pega.
    let mut e = entradas(&pacote_sintetico(&chave()));
    let mut dados = e
        .iter()
        .find(|(n, _)| n == "partituras/tres-notas.musicxml")
        .unwrap()
        .1
        .clone();
    dados[10] ^= 0x01;
    trocar(&mut e, "partituras/tres-notas.musicxml", dados);
    match ler_pacote(&montar(&e)) {
        Err(ErroPacote::HashDivergente { caminho }) => {
            assert_eq!(caminho, "partituras/tres-notas.musicxml")
        }
        outro => panic!("esperava HashDivergente, veio {outro:?}"),
    }
}

#[test]
fn arquivo_com_tamanho_diferente_e_rejeitado() {
    let mut e = entradas(&pacote_sintetico(&chave()));
    trocar(
        &mut e,
        "partituras/tres-notas.musicxml",
        b"<score-partwise/>".to_vec(),
    );
    assert!(matches!(
        ler_pacote(&montar(&e)),
        Err(ErroPacote::TamanhoDivergente { .. })
    ));
}

#[test]
fn arquivo_extra_no_zip_invalida_o_pacote() {
    let mut e = entradas(&pacote_sintetico(&chave()));
    e.push(("extra/malicioso.bin".to_string(), b"x".to_vec()));
    match ler_pacote(&montar(&e)) {
        Err(ErroPacote::ArquivoNaoListado { caminho }) => {
            assert_eq!(caminho, "extra/malicioso.bin")
        }
        outro => panic!("esperava ArquivoNaoListado, veio {outro:?}"),
    }
}

#[test]
fn arquivo_listado_e_ausente_invalida_o_pacote() {
    let mut e = entradas(&pacote_sintetico(&chave()));
    e.retain(|(n, _)| n != "audio/tres-notas.opus");
    assert!(matches!(
        ler_pacote(&montar(&e)),
        Err(ErroPacote::ArquivoAusente { .. })
    ));
}

#[test]
fn bytes_que_nao_sao_zip_dao_erro_claro() {
    let erro = ler_pacote(b"isto nao e um zip").unwrap_err();
    assert!(matches!(erro, ErroPacote::ZipInvalido(_)));
    assert!(erro.to_string().contains("zip"), "{erro}");
}

#[test]
fn manifesto_ou_assinatura_ausentes() {
    let e = entradas(&pacote_sintetico(&chave()));
    let sem_manifesto: Vec<_> = e
        .iter()
        .filter(|(n, _)| n != "manifesto.toml")
        .cloned()
        .collect();
    assert!(matches!(
        ler_pacote(&montar(&sem_manifesto)),
        Err(ErroPacote::ManifestoAusente)
    ));
    let sem_assinatura: Vec<_> = e
        .iter()
        .filter(|(n, _)| n != "assinatura.ed25519")
        .cloned()
        .collect();
    assert!(matches!(
        ler_pacote(&montar(&sem_assinatura)),
        Err(ErroPacote::AssinaturaAusente)
    ));
}

// ---------- caminhos seguros ----------

#[test]
fn caminho_com_ponto_ponto_no_manifesto_assinado_e_rejeitado() {
    let chave = chave();
    let conteudo = b"fora";
    let arquivos = format!(
        "[[arquivos]]\ncaminho = \"partituras/../fora.bin\"\nsha256 = \"{}\"\nbytes = {}\n",
        sha256_hex(conteudo),
        conteudo.len()
    );
    let texto = manifesto_minimo(&chave, 1, &arquivos);
    let bytes = montar_assinado(&chave, &texto, &[("partituras/../fora.bin", conteudo)]);
    assert!(matches!(
        ler_pacote(&bytes),
        Err(ErroPacote::CaminhoInseguro(_))
    ));
}

#[test]
fn entrada_do_zip_com_ponto_ponto_ou_absoluta_e_rejeitada() {
    let chave = chave();
    let texto = manifesto_minimo(&chave, 1, "");
    for nome in ["../fora.bin", "/etc/passwd", "a\\b.bin", "C:/x.bin"] {
        let bytes = montar_assinado(&chave, &texto, &[(nome, b"x")]);
        assert!(
            matches!(ler_pacote(&bytes), Err(ErroPacote::CaminhoInseguro(_))),
            "{nome} deveria ser inseguro"
        );
    }
}

#[test]
fn empacotar_recusa_caminho_inseguro_no_manifesto() {
    let pasta = tempfile::tempdir().unwrap();
    escrever_pasta_origem(pasta.path());
    let texto = MANIFESTO.replace("partituras/tres-notas.musicxml", "../tres-notas.musicxml");
    fs::write(pasta.path().join("manifesto.toml"), texto).unwrap();
    assert!(matches!(
        empacotar(pasta.path(), &chave()),
        Err(ErroPacote::CaminhoInseguro(_))
    ));
}

#[test]
fn empacotar_recusa_partitura_inexistente_na_pasta() {
    let pasta = tempfile::tempdir().unwrap();
    escrever_pasta_origem(pasta.path());
    fs::remove_file(pasta.path().join("partituras/tres-notas.musicxml")).unwrap();
    assert!(matches!(
        empacotar(pasta.path(), &chave()),
        Err(ErroPacote::ArquivoReferenciadoNaoListado { .. })
    ));
}

// ---------- versão do formato ----------

#[test]
fn formato_versao_futuro_da_erro_claro() {
    let chave = chave();
    let texto = manifesto_minimo(&chave, 2, "");
    let bytes = montar_assinado(&chave, &texto, &[]);
    let erro = ler_pacote(&bytes).unwrap_err();
    assert!(matches!(
        erro,
        ErroPacote::FormatoMaisNovo {
            encontrado: 2,
            suportado: 1
        }
    ));
    assert!(
        erro.to_string().contains("versão mais nova do app"),
        "{erro}"
    );
}

#[test]
fn formato_versao_zero_e_invalido() {
    let chave = chave();
    let texto = manifesto_minimo(&chave, 0, "");
    assert!(matches!(
        ler_pacote(&montar_assinado(&chave, &texto, &[])),
        Err(ErroPacote::ManifestoInvalido(_))
    ));
}

// ---------- validação estrutural ----------

fn pacote_com_manifesto_alterado(de: &str, para: &str) -> Result<(), ErroPacote> {
    // Empacota uma origem cujo manifesto foi alterado (o empacotador valida antes de assinar).
    let pasta = tempfile::tempdir().unwrap();
    escrever_pasta_origem(pasta.path());
    fs::write(
        pasta.path().join("manifesto.toml"),
        MANIFESTO.replace(de, para),
    )
    .unwrap();
    empacotar(pasta.path(), &chave()).map(|_| ())
}

#[test]
fn ids_invalidos_e_duplicados_sao_rejeitados() {
    // curso id perigoso para nome de pasta
    assert!(matches!(
        pacote_com_manifesto_alterado("id = \"teste.sintetico\"", "id = \"..\""),
        Err(ErroPacote::IdInvalido { .. })
    ));
    assert!(matches!(
        pacote_com_manifesto_alterado("id = \"teste.sintetico\"", "id = \"Maiuscula\""),
        Err(ErroPacote::IdInvalido { .. })
    ));
    // pastas reservadas dentro de cursos/
    assert!(matches!(
        pacote_com_manifesto_alterado("id = \"teste.sintetico\"", "id = \"progresso\""),
        Err(ErroPacote::IdInvalido { .. })
    ));
    // lição duplicada
    assert!(matches!(
        pacote_com_manifesto_alterado("id = \"s01-a02\"", "id = \"s01-a01\""),
        Err(ErroPacote::IdDuplicado { .. })
    ));
    // exercício duplicado
    assert!(matches!(
        pacote_com_manifesto_alterado("id = \"tres-notas-volta\"", "id = \"tres-notas\""),
        Err(ErroPacote::IdDuplicado { .. })
    ));
}

#[test]
fn licao_assistir_nao_pode_ter_exercicios() {
    assert!(matches!(
        pacote_com_manifesto_alterado(
            "tipo = \"exercicio\"\n\n[secoes.licoes.aula]",
            "tipo = \"assistir\"\n\n[secoes.licoes.aula]"
        ),
        Err(ErroPacote::Estrutura(_))
    ));
}

#[test]
fn roteiro_apontando_para_licao_inexistente_e_rejeitado() {
    let pasta = tempfile::tempdir().unwrap();
    escrever_pasta_origem(pasta.path());
    fs::write(
        pasta.path().join("roteiro.toml"),
        ROTEIRO.replace("licao = \"s01-a01\"", "licao = \"nao-existe\""),
    )
    .unwrap();
    assert!(matches!(
        empacotar(pasta.path(), &chave()),
        Err(ErroPacote::Estrutura(_))
    ));
}

#[test]
fn alinhamento_de_outro_exercicio_e_rejeitado() {
    let pasta = tempfile::tempdir().unwrap();
    escrever_pasta_origem(pasta.path());
    fs::write(
        pasta.path().join("alinhamentos/tres-notas.json"),
        ALINHAMENTO.replace("\"exercicio\": \"tres-notas\"", "\"exercicio\": \"outro\""),
    )
    .unwrap();
    assert!(matches!(
        empacotar(pasta.path(), &chave()),
        Err(ErroPacote::Estrutura(_))
    ));
}

// ---------- confiança ----------

#[test]
fn chave_confiavel_versus_desconhecida() {
    let autor = chave();
    let outra = chave();
    let pacote = ler_pacote(&pacote_sintetico(&autor)).unwrap();
    assert_eq!(pacote.confianca(&[]), Confianca::Desconhecida);
    assert_eq!(
        pacote.confianca(&[outra.publica()]),
        Confianca::Desconhecida
    );
    assert_eq!(
        pacote.confianca(&[outra.publica(), autor.publica()]),
        Confianca::Confiavel
    );
}

#[test]
fn chaves_confiaveis_da_pasta_dados() {
    let autor = chave();
    let dados = tempfile::tempdir().unwrap();
    let pasta = dados.path().join("cursos/chaves-confiaveis");
    fs::create_dir_all(&pasta).unwrap();
    autor.publica().gravar(&pasta.join("autor.pub")).unwrap();
    fs::write(pasta.join("lixo.pub"), "isto não é uma chave").unwrap();
    fs::write(pasta.join("ignorado.txt"), autor.publica().para_hex()).unwrap();
    let chaves = piano_tutor_curso::chaves_confiaveis(dados.path());
    assert_eq!(chaves, vec![autor.publica()]);
}

// ---------- instalação ----------

fn pacote_com_versao(chave: &ChavePrivada, versao: &str) -> Vec<u8> {
    let pasta = tempfile::tempdir().unwrap();
    escrever_pasta_origem(pasta.path());
    fs::write(
        pasta.path().join("manifesto.toml"),
        MANIFESTO.replace("versao = \"0.1.0\"", &format!("versao = \"{versao}\"")),
    )
    .unwrap();
    empacotar(pasta.path(), chave).unwrap()
}

#[test]
fn instalar_extrai_e_lista() {
    let chave = chave();
    let dados = tempfile::tempdir().unwrap();
    let pacote = ler_pacote(&pacote_sintetico(&chave)).unwrap();
    let destino = instalar(&pacote, dados.path()).unwrap();
    assert_eq!(destino, dados.path().join("cursos/teste.sintetico"));
    assert!(destino.join("manifesto.toml").is_file());
    assert!(destino.join("assinatura.ed25519").is_file());
    assert!(destino.join("partituras/tres-notas.musicxml").is_file());
    assert!(destino.join("roteiro.toml").is_file());

    let lista = cursos_instalados(dados.path()).unwrap();
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0].id, "teste.sintetico");
    assert_eq!(lista[0].versao, "0.1.0");
    assert_eq!(lista[0].autor, "Autor Sintético");
    assert!(lista[0].assinatura_valida);
    assert_eq!(lista[0].pasta, destino);

    // Sem temporários sobrando.
    let nomes: Vec<_> = fs::read_dir(dados.path().join("cursos"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(nomes, vec!["teste.sintetico".to_string()]);
}

#[test]
fn cursos_instalados_sem_pasta_devolve_vazio_e_ignora_reservadas() {
    let dados = tempfile::tempdir().unwrap();
    assert!(cursos_instalados(dados.path()).unwrap().is_empty());
    fs::create_dir_all(dados.path().join("cursos/chaves-confiaveis")).unwrap();
    fs::create_dir_all(dados.path().join("cursos/progresso")).unwrap();
    assert!(cursos_instalados(dados.path()).unwrap().is_empty());
}

#[test]
fn instalar_substitui_so_quando_a_nova_versao_verificou() {
    let chave = chave();
    let dados = tempfile::tempdir().unwrap();

    let v1 = ler_pacote(&pacote_com_versao(&chave, "0.1.0")).unwrap();
    instalar(&v1, dados.path()).unwrap();

    // Versão 2 válida: substitui.
    let bytes_v2 = pacote_com_versao(&chave, "0.2.0");
    instalar(&ler_pacote(&bytes_v2).unwrap(), dados.path()).unwrap();
    assert_eq!(cursos_instalados(dados.path()).unwrap()[0].versao, "0.2.0");

    // Versão 3 adulterada: nem chega a ser lida, então nada muda.
    let mut e = entradas(&pacote_com_versao(&chave, "0.3.0"));
    trocar(
        &mut e,
        "partituras/tres-notas.musicxml",
        b"<adulterado/>".to_vec(),
    );
    assert!(ler_pacote(&montar(&e)).is_err());
    let lista = cursos_instalados(dados.path()).unwrap();
    assert_eq!(lista[0].versao, "0.2.0");
    assert!(lista[0].assinatura_valida);
}

#[test]
fn instalar_nao_deixa_resto_antigo_na_substituicao() {
    let chave = chave();
    let dados = tempfile::tempdir().unwrap();
    let destino = instalar(
        &ler_pacote(&pacote_com_versao(&chave, "0.1.0")).unwrap(),
        dados.path(),
    )
    .unwrap();
    fs::write(destino.join("resto-da-versao-antiga.txt"), b"x").unwrap();
    instalar(
        &ler_pacote(&pacote_com_versao(&chave, "0.2.0")).unwrap(),
        dados.path(),
    )
    .unwrap();
    assert!(!destino.join("resto-da-versao-antiga.txt").exists());
}

#[test]
fn instalar_recusa_trocar_o_autor_do_curso_ja_instalado() {
    let dados = tempfile::tempdir().unwrap();
    let original = chave();
    let impostor = chave();
    let destino = instalar(
        &ler_pacote(&pacote_sintetico(&original)).unwrap(),
        dados.path(),
    )
    .unwrap();
    let pacote_impostor = ler_pacote(&pacote_sintetico(&impostor)).unwrap();
    assert!(matches!(
        instalar(&pacote_impostor, dados.path()),
        Err(ErroPacote::AutorDiferente { .. })
    ));
    // O curso original continua intacto e íntegro.
    let lista = cursos_instalados(dados.path()).unwrap();
    assert_eq!(lista.len(), 1);
    assert!(lista[0].assinatura_valida);
    assert_eq!(lista[0].chave_publica, original.publica());
    assert!(destino.join("manifesto.toml").is_file());
}

// ---------- Gabarito ----------

#[test]
fn so_aprovada_serve_de_gabarito() {
    let mut m: Manifesto = toml_do_modelo();
    let todos: Vec<_> = m.exercicios().map(|x| x.exercicio.estado).collect();
    assert_eq!(
        todos,
        vec![EstadoConteudo::Aprovada, EstadoConteudo::Hipotese]
    );

    // Um exercício em cada estado.
    let base = m.secoes[0].licoes[0].exercicios[0].clone();
    m.secoes[0].licoes[1].exercicios.clear();
    m.secoes[0].licoes[0].exercicios.clear();
    for (i, estado) in [
        EstadoConteudo::Hipotese,
        EstadoConteudo::EmRevisao,
        EstadoConteudo::Aprovada,
        EstadoConteudo::Rejeitada,
    ]
    .into_iter()
    .enumerate()
    {
        let mut ex = base.clone();
        ex.id = format!("ex-{i}");
        ex.estado = estado;
        assert_eq!(ex.serve_de_gabarito(), estado == EstadoConteudo::Aprovada);
        m.secoes[0].licoes[0].exercicios.push(ex);
    }
    let para_aluno: Vec<_> = m
        .exercicios_para_aluno()
        .map(|x| x.exercicio.id.as_str())
        .collect();
    assert_eq!(para_aluno, vec!["ex-2"]);
    assert_eq!(m.exercicios().count(), 4);
}

#[test]
fn contagem_por_estado() {
    let m = toml_do_modelo();
    let c = m.contagem();
    assert_eq!((c.secoes, c.licoes, c.exercicios), (1, 2, 2));
    assert_eq!(c.por_estado(EstadoConteudo::Aprovada), 1);
    assert_eq!(c.por_estado(EstadoConteudo::Hipotese), 1);
    assert_eq!(c.por_estado(EstadoConteudo::EmRevisao), 0);
    assert_eq!(c.por_estado(EstadoConteudo::Rejeitada), 0);
}

fn toml_do_modelo() -> Manifesto {
    Manifesto::de_toml(MANIFESTO).unwrap()
}

#[test]
fn manifesto_serializa_e_volta_igual() {
    let m = toml_do_modelo();
    let texto = m.para_toml().unwrap();
    assert_eq!(Manifesto::de_toml(&texto).unwrap(), m);
}

#[test]
fn arquivo_acima_do_limite_e_recusado_sem_ler_tudo() {
    // 51 MiB de zeros comprimem para poucos KB: só o limite declarado pode barrar.
    let chave = chave();
    let texto = manifesto_minimo(&chave, 1, "");
    let grande = vec![0u8; 51 * 1024 * 1024];
    let bytes = montar_assinado(&chave, &texto, &[("audio/grande.bin", &grande)]);
    assert!(
        bytes.len() < 1024 * 1024,
        "o zip de teste deveria ser pequeno"
    );
    assert!(matches!(
        ler_pacote(&bytes),
        Err(ErroPacote::LimiteExcedido(_))
    ));
}
