//! Dedilhado: a nomenclatura dos dedos (números ou letras) e a leitura do campo `dedilhado`
//! das Lições para saber qual dedo toca cada nota do gabarito.
//!
//! Dedo 1 é sempre o polegar e 5 o mínimo, nas duas mãos (convenção do piano). A nomenclatura
//! só muda o rótulo desenhado dentro da bolinha do teclado.

use crate::exercicios::Passo;
use piano_tutor_core::NotaMidi;

/// Como o dedo aparece na bolinha do teclado.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NomenclaturaDedos {
    /// 1 a 5, do polegar ao mínimo (padrão do mercado: partituras, Synthesia, Simply Piano).
    #[default]
    Numeros,
    /// Iniciais em português: P (polegar), I (indicador), M (médio), A (anelar), N (mínimo;
    /// "M" já é o médio e "Mi" confundiria com a nota). Convenção do app, sem tradição impressa.
    LetrasPt,
    /// "Dedilhado inglês" do séc. XIX: + no polegar e 1 a 4 do indicador ao mínimo
    /// (partituras britânicas antigas).
    InglesAntigo,
}

/// Todas, na ordem do seletor.
pub const NOMENCLATURAS: [NomenclaturaDedos; 3] = [
    NomenclaturaDedos::Numeros,
    NomenclaturaDedos::LetrasPt,
    NomenclaturaDedos::InglesAntigo,
];

impl NomenclaturaDedos {
    /// Chave estável (arquivo de preferências e id da ação do Ribbon).
    pub fn chave(self) -> &'static str {
        match self {
            Self::Numeros => "numeros",
            Self::LetrasPt => "letras",
            Self::InglesAntigo => "ingles-antigo",
        }
    }

    pub fn de_chave(s: &str) -> Option<Self> {
        NOMENCLATURAS.into_iter().find(|n| n.chave() == s)
    }

    /// Nome no seletor do Ribbon.
    pub fn nome(self) -> &'static str {
        match self {
            Self::Numeros => "Dedos: números 1 2 3 4 5",
            Self::LetrasPt => "Dedos: letras P I M A N",
            Self::InglesAntigo => "Dedos: inglês antigo + 1 2 3 4",
        }
    }

    /// Rótulo curto (seletor do Ribbon): os cinco símbolos, do polegar ao mínimo.
    pub fn curto(self) -> &'static str {
        match self {
            Self::Numeros => "1 2 3 4 5",
            Self::LetrasPt => "P I M A N",
            Self::InglesAntigo => "+ 1 2 3 4",
        }
    }

    /// Dica do seletor.
    pub fn dica(self) -> &'static str {
        match self {
            Self::Numeros => {
                "Números de 1 a 5: 1 polegar, 2 indicador, 3 médio, 4 anelar, 5 mínimo \
                 (o padrão das partituras)"
            }
            Self::LetrasPt => {
                "Letras: P polegar, I indicador, M médio, A anelar, N mínimo (mindinho)"
            }
            Self::InglesAntigo => {
                "Dedilhado inglês antigo (partituras britânicas do séc. XIX): + polegar, \
                 1 indicador, 2 médio, 3 anelar, 4 mínimo"
            }
        }
    }

    /// Rótulo do dedo `1..=5`; `None` fora dessa faixa.
    pub fn rotulo(self, dedo: u8) -> Option<&'static str> {
        let i = usize::from(dedo.checked_sub(1)?);
        let tabela: [&str; 5] = match self {
            Self::Numeros => ["1", "2", "3", "4", "5"],
            Self::LetrasPt => ["P", "I", "M", "A", "N"],
            Self::InglesAntigo => ["+", "1", "2", "3", "4"],
        };
        tabela.get(i).copied()
    }
}

/// Dedos citados no texto livre do dedilhado, na ordem ("Mão direita: 1-2-3-5; descendo:
/// 5-3-2-1" dá 1 2 3 5 5 3 2 1). Só conta algarismo solto de 1 a 5 (ignora "12", "0", "6").
pub fn dedos_do_texto(texto: &str) -> Vec<u8> {
    let b = texto.as_bytes();
    let mut v = Vec::new();
    for (i, c) in b.iter().enumerate() {
        let solto = |j: Option<usize>| j.and_then(|j| b.get(j)).is_none_or(|x| !x.is_ascii_digit());
        if (b'1'..=b'5').contains(c) && solto(i.checked_sub(1)) && solto(Some(i + 1)) {
            v.push(c - b'0');
        }
    }
    v
}

/// Texto do dedilhado para a ficha: igual ao da Lição em números; nas outras nomenclaturas,
/// acrescenta a tradução ("1-3-5 (P-M-N)") para bater com o que aparece nas bolinhas.
pub fn dedilhado_na_nomenclatura(texto: &str, n: NomenclaturaDedos) -> String {
    let traduzido: Vec<&str> = dedos_do_texto(texto)
        .into_iter()
        .filter_map(|d| n.rotulo(d))
        .collect();
    if n == NomenclaturaDedos::Numeros || traduzido.is_empty() {
        texto.to_owned()
    } else {
        format!("{texto}  ({})", traduzido.join("-"))
    }
}

/// `true` quando o texto é só um padrão de dedos ("1-3-5", "Mão direita: 1-2-3-4-5-4-3-2"),
/// sem explicação: aí o padrão pode se repetir a cada grupo (Hanon, acordes seguidos).
fn padrao_puro(texto: &str) -> bool {
    let corpo = texto.rsplit_once(':').map_or(texto, |(antes, depois)| {
        if antes.trim_start().starts_with("Mão") {
            depois
        } else {
            texto
        }
    });
    let corpo = corpo.trim();
    !corpo.is_empty() && corpo.chars().all(|c| matches!(c, '1'..='5' | '-' | ' '))
}

/// Dedo de cada nota de cada passo, quando o dedilhado cobre exatamente todas as notas do
/// gabarito (num acorde, da nota mais grave para a mais aguda), ou quando é um padrão puro que se
/// repete um número inteiro de vezes. Se a contagem não bate (texto
/// explicativo, vários dedilhados alternativos), devolve vazio: melhor sem dedo que dedo errado.
pub fn dedos_por_passo(dedilhado: &str, passos: &[Passo]) -> Vec<Vec<(NotaMidi, u8)>> {
    let dedos = dedos_do_texto(dedilhado);
    let total: usize = passos.iter().map(|p| p.notas.len()).sum();
    let ciclo = padrao_puro(dedilhado) && !dedos.is_empty() && total.is_multiple_of(dedos.len());
    if dedos.is_empty() || (dedos.len() != total && !ciclo) {
        return Vec::new();
    }
    let mut resto = dedos.into_iter().cycle();
    passos
        .iter()
        .map(|p| {
            let mut notas = p.notas.clone();
            notas.sort_by_key(|n| n.numero());
            notas.into_iter().zip(resto.by_ref()).collect()
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn n(x: u8) -> NotaMidi {
        NotaMidi::nova(x).expect("nota")
    }

    fn passo(notas: &[u8]) -> Passo {
        Passo {
            notas: notas.iter().copied().map(n).collect(),
            por_classe: false,
            legenda: String::new(),
        }
    }

    #[test]
    fn rotulos_das_tres_nomenclaturas() {
        let todos =
            |nm: NomenclaturaDedos| (1..=5).filter_map(|d| nm.rotulo(d)).collect::<String>();
        assert_eq!(todos(NomenclaturaDedos::Numeros), "12345");
        assert_eq!(todos(NomenclaturaDedos::LetrasPt), "PIMAN");
        assert_eq!(todos(NomenclaturaDedos::InglesAntigo), "+1234");
        assert_eq!(NomenclaturaDedos::Numeros.rotulo(0), None);
        assert_eq!(NomenclaturaDedos::Numeros.rotulo(6), None);
    }

    #[test]
    fn ficha_mostra_a_traducao_do_dedilhado() {
        assert_eq!(
            dedilhado_na_nomenclatura("1-3-5", NomenclaturaDedos::Numeros),
            "1-3-5"
        );
        assert_eq!(
            dedilhado_na_nomenclatura("1-3-5", NomenclaturaDedos::LetrasPt),
            "1-3-5  (P-M-N)"
        );
        assert_eq!(
            dedilhado_na_nomenclatura("1-2-5", NomenclaturaDedos::InglesAntigo),
            "1-2-5  (+-1-4)"
        );
    }

    #[test]
    fn chave_ida_e_volta() {
        for nm in NOMENCLATURAS {
            assert_eq!(NomenclaturaDedos::de_chave(nm.chave()), Some(nm));
        }
        assert_eq!(NomenclaturaDedos::de_chave("x"), None);
    }

    #[test]
    fn le_dedos_do_texto_livre() {
        assert_eq!(
            dedos_do_texto("1-2-3-1-2-3-4-5"),
            vec![1, 2, 3, 1, 2, 3, 4, 5]
        );
        assert_eq!(
            dedos_do_texto("Mão direita subindo: 1-2-3-5; descendo: 5-3-2-1"),
            vec![1, 2, 3, 5, 5, 3, 2, 1]
        );
        assert_eq!(dedos_do_texto("compasso 12, dedo 6 e 0"), Vec::<u8>::new());
    }

    #[test]
    fn distribui_um_dedo_por_nota_e_acorde_do_grave_ao_agudo() {
        let passos = [passo(&[60]), passo(&[67, 64, 60])];
        let d = dedos_por_passo("2 / 5-3-1", &passos);
        assert_eq!(d[0], vec![(n(60), 2)]);
        assert_eq!(d[1], vec![(n(60), 5), (n(64), 3), (n(67), 1)]);
    }

    #[test]
    fn padrao_puro_se_repete_por_grupo() {
        let passos: Vec<Passo> = [60, 64, 67, 62, 65, 69]
            .iter()
            .map(|x| passo(&[*x]))
            .collect();
        let d = dedos_por_passo("Mão direita: 1-3-5", &passos);
        let so_dedos: Vec<u8> = d.iter().map(|p| p[0].1).collect();
        assert_eq!(so_dedos, vec![1, 3, 5, 1, 3, 5]);
        // Texto explicativo não se repete: contagem diferente fica sem dedo.
        assert!(dedos_por_passo("Mão direita: 1-3 para terças", &passos[..4]).is_empty());
    }

    #[test]
    fn contagem_diferente_nao_inventa_dedo() {
        let passos = [passo(&[60]), passo(&[62])];
        assert!(dedos_por_passo("1-2-3", &passos).is_empty());
        assert!(dedos_por_passo("", &passos).is_empty());
    }
}
