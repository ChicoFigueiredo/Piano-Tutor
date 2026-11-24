//! Fixtures sintéticas compartilhadas pelos testes de integração.
//!
//! Nada aqui vem de curso real: o "Curso Sintético de Teste" tem 1 Seção, 2 Lições,
//! 2 Exercícios (MusicXML mínimo de 3 notas), 1 Alinhamento e 1 Roteiro.
#![allow(dead_code)]

use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use piano_tutor_curso::ChavePrivada;

pub const MANIFESTO: &str = r#"formato_versao = 1
id = "teste.sintetico"
titulo = "Curso Sintético de Teste"
versao = "0.1.0"
gerado_em = "2026-01-01T00:00:00Z"
idioma = "pt-BR"
convencao_oitava = "do3-central"

[autor]
nome = "Autor Sintético"

[licenciado]
nome = "Aluno de Teste"
licenca = "Somente para testes automatizados."

[[secoes]]
id = "s01"
numero = 1
titulo = "Seção de teste"

[[secoes.licoes]]
id = "s01-a01"
numero = 1
titulo = "Três notas"
tipo = "exercicio"

[secoes.licoes.aula]
nome_arquivo = "01 - Três notas.mp4"
duracao_s = 60.0

[[secoes.licoes.exercicios]]
id = "tres-notas"
titulo = "Três notas"
modo = "praticar"
mao = "direita"
estado = "aprovada"
partitura = "partituras/tres-notas.musicxml"
audio = "audio/tres-notas.opus"
alinhamento = "alinhamentos/tres-notas.json"
bpm = 80

[secoes.licoes.exercicios.trecho_aula]
inicio_s = 5.0
fim_s = 15.5

[[secoes.licoes]]
id = "s01-a02"
numero = 2
titulo = "Três notas, de volta"
tipo = "exercicio"

[[secoes.licoes.exercicios]]
id = "tres-notas-volta"
titulo = "Três notas, de volta"
modo = "chamada_e_resposta"
mao = "ambas"
estado = "hipotese"
partitura = "partituras/tres-notas-volta.musicxml"
"#;

pub const MUSICXML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes><divisions>1</divisions><time><beats>3</beats><beat-type>4</beat-type></time></attributes>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note>
      <note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note>
      <note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note>
    </measure>
  </part>
</score-partwise>
"#;

pub const ALINHAMENTO: &str = r#"{
  "formato_versao": 1,
  "exercicio": "tres-notas",
  "audio": "audio/tres-notas.opus",
  "estado": "hipotese",
  "metodo": "dtw-croma",
  "offset_aula_s": 5.0,
  "notas": [
    { "indice": 0, "compasso": 1, "midi": 60, "t_partitura_s": 0.0, "t_audio_s": 0.42, "confianca": 0.81 },
    { "indice": 1, "compasso": 1, "midi": 62, "t_partitura_s": 0.5, "t_audio_s": 0.95, "confianca": 0.77 },
    { "indice": 2, "compasso": 1, "midi": 64, "t_partitura_s": 1.0, "t_audio_s": 1.48, "confianca": 0.79 }
  ]
}
"#;

pub const ROTEIRO: &str = r#"formato_versao = 1

[[falas]]
id = "s01-a01-intro"
licao = "s01-a01"
exercicio = "tres-notas"
momento = "antes"
texto = "Vamos tocar três notas com a mão direita."
estado = "aprovada"

[[falas]]
id = "s01-a01-passo1"
licao = "s01-a01"
exercicio = "tres-notas"
momento = "passo"
passo = 1
texto = "Agora o Ré."
estado = "hipotese"
origem = { aula = 1, inicio_s = 12.3, fim_s = 18.0 }
"#;

/// Escreve a pasta de origem do curso sintético (sem `[[arquivos]]` nem assinatura).
pub fn escrever_pasta_origem(pasta: &Path) {
    let musicxml = MUSICXML;
    for (caminho, conteudo) in [
        ("manifesto.toml", MANIFESTO.as_bytes().to_vec()),
        (
            "partituras/tres-notas.musicxml",
            musicxml.as_bytes().to_vec(),
        ),
        (
            "partituras/tres-notas-volta.musicxml",
            musicxml.as_bytes().to_vec(),
        ),
        ("audio/tres-notas.opus", b"OggS-sintetico".to_vec()),
        (
            "alinhamentos/tres-notas.json",
            ALINHAMENTO.as_bytes().to_vec(),
        ),
        ("roteiro.toml", ROTEIRO.as_bytes().to_vec()),
    ] {
        let destino = pasta.join(caminho);
        fs::create_dir_all(destino.parent().unwrap()).unwrap();
        fs::write(destino, conteudo).unwrap();
    }
}

/// Empacota o curso sintético com a chave dada.
pub fn pacote_sintetico(chave: &ChavePrivada) -> Vec<u8> {
    let pasta = tempfile::tempdir().unwrap();
    escrever_pasta_origem(pasta.path());
    piano_tutor_curso::empacotar(pasta.path(), chave).unwrap()
}

/// Abre um zip e devolve as entradas `(nome, bytes)` na ordem.
pub fn entradas(zip_bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut zip = zip::ZipArchive::new(Cursor::new(zip_bytes)).unwrap();
    let mut saida = Vec::new();
    for i in 0..zip.len() {
        let mut arquivo = zip.by_index(i).unwrap();
        let mut dados = Vec::new();
        arquivo.read_to_end(&mut dados).unwrap();
        saida.push((arquivo.name().unwrap().to_string(), dados));
    }
    saida
}

/// Monta um zip a partir de entradas, sem nenhuma verificação (para fabricar pacotes inválidos).
pub fn montar(entradas: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opcoes = zip::write::SimpleFileOptions::default();
    for (nome, dados) in entradas {
        zip.start_file(nome.as_str(), opcoes).unwrap();
        zip.write_all(dados).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

pub fn trocar(entradas: &mut [(String, Vec<u8>)], nome: &str, novo: Vec<u8>) {
    let entrada = entradas.iter_mut().find(|(n, _)| n == nome).unwrap();
    entrada.1 = novo;
}

/// Pacote com manifesto e arquivos arbitrários, assinado corretamente com `chave`:
/// `texto_manifesto` é gravado como está (o teste responde pelo conteúdo).
pub fn montar_assinado(
    chave: &ChavePrivada,
    texto_manifesto: &str,
    outros: &[(&str, &[u8])],
) -> Vec<u8> {
    let mut todas = vec![
        (
            "manifesto.toml".to_string(),
            texto_manifesto.as_bytes().to_vec(),
        ),
        (
            "assinatura.ed25519".to_string(),
            chave.assinar(texto_manifesto.as_bytes()).to_vec(),
        ),
    ];
    for (nome, dados) in outros {
        todas.push((nome.to_string(), dados.to_vec()));
    }
    montar(&todas)
}

pub fn sha256_hex(dados: &[u8]) -> String {
    piano_tutor_curso::sha256_hex(dados)
}

/// Manifesto mínimo (1 Seção, 1 Lição `assistir`) com a chave pública e a versão de formato dadas.
pub fn manifesto_minimo(chave: &ChavePrivada, formato_versao: u32, arquivos: &str) -> String {
    format!(
        r#"formato_versao = {formato_versao}
id = "minimo"
titulo = "Mínimo"
versao = "1.0.0"
gerado_em = "2026-01-01T00:00:00Z"
idioma = "pt-BR"
convencao_oitava = "do3-central"

[autor]
nome = "Autor"

[licenciado]
nome = "Aluno"
licenca = "Teste"

[assinatura]
chave_publica = "{}"

[[secoes]]
id = "s01"
numero = 1
titulo = "Seção"

[[secoes.licoes]]
id = "s01-a01"
numero = 1
titulo = "Assistir"
tipo = "assistir"

{arquivos}
"#,
        chave.publica().para_hex()
    )
}
