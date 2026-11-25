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

// ---------------------------------------------------------------------------------------------
// Provedor de dedilhado do componente de teclado
// ---------------------------------------------------------------------------------------------

/// Dedilhados de escala de uma oitava (8 notas, da tônica à oitava), mão direita e esquerda,
/// por classe de altura da tônica (0 = Dó). Tabela tradicional (métodos de escalas, como os do
/// ABRSM); a esquerda é lida subindo, do grave ao agudo.
const ESCALA_MAIOR: [([u8; 8], [u8; 8]); 12] = [
    ([1, 2, 3, 1, 2, 3, 4, 5], [5, 4, 3, 2, 1, 3, 2, 1]), // Dó
    ([2, 3, 1, 2, 3, 4, 1, 2], [3, 2, 1, 4, 3, 2, 1, 3]), // Réb
    ([1, 2, 3, 1, 2, 3, 4, 5], [5, 4, 3, 2, 1, 3, 2, 1]), // Ré
    ([3, 1, 2, 3, 4, 1, 2, 3], [3, 2, 1, 4, 3, 2, 1, 3]), // Mib
    ([1, 2, 3, 1, 2, 3, 4, 5], [5, 4, 3, 2, 1, 3, 2, 1]), // Mi
    ([1, 2, 3, 4, 1, 2, 3, 4], [5, 4, 3, 2, 1, 3, 2, 1]), // Fá
    ([2, 3, 4, 1, 2, 3, 1, 2], [4, 3, 2, 1, 3, 2, 1, 4]), // Fá#/Solb
    ([1, 2, 3, 1, 2, 3, 4, 5], [5, 4, 3, 2, 1, 3, 2, 1]), // Sol
    ([3, 4, 1, 2, 3, 1, 2, 3], [3, 2, 1, 4, 3, 2, 1, 3]), // Láb
    ([1, 2, 3, 1, 2, 3, 4, 5], [5, 4, 3, 2, 1, 3, 2, 1]), // Lá
    ([4, 1, 2, 3, 1, 2, 3, 4], [3, 2, 1, 4, 3, 2, 1, 3]), // Sib
    ([1, 2, 3, 1, 2, 3, 4, 5], [4, 3, 2, 1, 4, 3, 2, 1]), // Si
];

/// Menores (natural e harmônica usam o mesmo dedilhado de uma oitava).
const ESCALA_MENOR: [([u8; 8], [u8; 8]); 12] = [
    ([1, 2, 3, 1, 2, 3, 4, 5], [5, 4, 3, 2, 1, 3, 2, 1]), // Dó
    ([3, 4, 1, 2, 3, 1, 2, 3], [3, 2, 1, 4, 3, 2, 1, 3]), // Dó#
    ([1, 2, 3, 1, 2, 3, 4, 5], [5, 4, 3, 2, 1, 3, 2, 1]), // Ré
    ([3, 1, 2, 3, 4, 1, 2, 3], [2, 1, 4, 3, 2, 1, 3, 2]), // Mib
    ([1, 2, 3, 1, 2, 3, 4, 5], [5, 4, 3, 2, 1, 3, 2, 1]), // Mi
    ([1, 2, 3, 4, 1, 2, 3, 4], [5, 4, 3, 2, 1, 3, 2, 1]), // Fá
    ([3, 4, 1, 2, 3, 1, 2, 3], [4, 3, 2, 1, 3, 2, 1, 4]), // Fá#
    ([1, 2, 3, 1, 2, 3, 4, 5], [5, 4, 3, 2, 1, 3, 2, 1]), // Sol
    ([3, 4, 1, 2, 3, 1, 2, 3], [3, 2, 1, 3, 2, 1, 4, 3]), // Sol#
    ([1, 2, 3, 1, 2, 3, 4, 5], [5, 4, 3, 2, 1, 3, 2, 1]), // Lá
    ([2, 1, 2, 3, 1, 2, 3, 4], [2, 1, 3, 2, 1, 4, 3, 2]), // Sib
    ([1, 2, 3, 1, 2, 3, 4, 5], [4, 3, 2, 1, 4, 3, 2, 1]), // Si
];

/// Semitons entre notas vizinhas.
fn saltos(notas: &[NotaMidi]) -> Vec<u8> {
    notas
        .windows(2)
        .map(|w| w[1].numero().saturating_sub(w[0].numero()))
        .collect()
}

/// Escala de uma oitava reconhecida pelos saltos: maior, menor (natural ou harmônica) ou
/// cromática. Pentatônicas e blues não têm dedilhado padrão: ficam sem dedo.
fn dedos_de_escala(notas: &[NotaMidi], esquerda: bool) -> Option<Vec<u8>> {
    let s = saltos(notas);
    let tonica = usize::from(notas.first()?.numero() % 12);
    let tabela = match s.as_slice() {
        [2, 2, 1, 2, 2, 2, 1] => &ESCALA_MAIOR,
        [2, 1, 2, 2, 1, 2, 2] | [2, 1, 2, 2, 1, 3, 1] => &ESCALA_MENOR,
        _ if s.len() == 12 && s.iter().all(|x| *x == 1) => {
            // Cromática: 3 nas pretas, 1 nas brancas; 2 no Fá e no Dó (direita) ou no Mi e no
            // Si (esquerda), onde duas brancas se encostam.
            // A primeira nota (branca) é sempre o polegar.
            let dedo = |(i, n): (usize, &NotaMidi)| match (n.numero() % 12, esquerda) {
                (1 | 3 | 6 | 8 | 10, _) => 3,
                (5 | 0, false) | (4 | 11, true) if i > 0 => 2,
                _ => 1,
            };
            return Some(notas.iter().enumerate().map(dedo).collect());
        }
        _ => return None,
    };
    let (direita, esq) = tabela[tonica];
    Some(if esquerda { esq } else { direita }.to_vec())
}

/// Acorde de 2 a 5 notas dentro de uma oitava (do grave ao agudo). Tríades seguem a inversão:
/// direita 1-3-5 (fundamental e 2ª inversão) e 1-2-5 (1ª); esquerda 5-3-1 e 5-2-1 (2ª).
fn dedos_de_acorde(notas: &[NotaMidi], esquerda: bool) -> Option<Vec<u8>> {
    let s = saltos(notas);
    let v: &[u8] = match (notas.len(), esquerda) {
        (2, false) => &[1, 5],
        (2, true) => &[5, 1],
        (3, false) if s[1] == 5 && s[0] != 2 => &[1, 2, 5],
        (3, false) => &[1, 3, 5],
        (3, true) if s[0] == 5 && s[1] != 2 => &[5, 2, 1],
        (3, true) => &[5, 3, 1],
        (4, false) => &[1, 2, 3, 5],
        (4, true) => &[5, 3, 2, 1],
        (5, false) => &[1, 2, 3, 4, 5],
        (5, true) => &[5, 4, 3, 2, 1],
        _ => return None,
    };
    Some(v.to_vec())
}

/// Intervalos (semitons a partir da tônica) dos 7 graus da escala maior.
const GRAUS_MAIOR: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];

/// Nomes das 12 tônicas maiores, por classe de altura (0 = Dó), para o seletor do mapa.
pub const TONICAS_MAPA: [&str; 12] = [
    "Dó", "Ré♭", "Ré", "Mi♭", "Mi", "Fá", "Fá♯", "Sol", "Lá♭", "Lá", "Si♭", "Si",
];

/// Mapa de dedos para decorar: o dedilhado da escala maior de `tonica` (classe de altura,
/// 0 = Dó) em todas as teclas de `notas` que pertencem à escala. Abaixo de `divisao` vale a
/// mão esquerda, a partir dela a direita, com o dedilhado contínuo de várias oitavas da tabela
/// tradicional (na esquerda, a tônica mais grave usa o dedo de início da escala). Teclas fora
/// da escala ficam sem dedo.
pub fn mapa_de_escala(
    tonica: u8,
    notas: impl IntoIterator<Item = NotaMidi>,
    divisao: u8,
) -> Vec<(NotaMidi, u8)> {
    let t = usize::from(tonica % 12);
    let (direita, esquerda) = ESCALA_MAIOR[t];
    let mut notas: Vec<NotaMidi> = notas.into_iter().collect();
    notas.sort_by_key(|n| n.numero());
    let primeira_tonica_esquerda = notas
        .iter()
        .find(|n| n.numero() < divisao && usize::from(n.numero() % 12) == t)
        .copied();
    notas
        .into_iter()
        .filter_map(|n| {
            let rel = (n.numero() + 12 - tonica % 12) % 12;
            let grau = GRAUS_MAIOR.iter().position(|&g| g == rel)?;
            let dedo = if n.numero() >= divisao {
                direita[grau]
            } else if grau == 0 && Some(n) != primeira_tonica_esquerda {
                esquerda[7]
            } else {
                esquerda[grau]
            };
            Some((n, dedo))
        })
        .collect()
}

/// Como o mapa para decorar distribui os dedos.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModoMapa {
    /// Escala contínua em todas as oitavas (o 5 só aparece no fim de uma escala, então nas
    /// oitavas do meio ele não surge); as notas fora da escala ganham o dedo da escala
    /// cromática.
    #[default]
    EscalaContinua,
    /// Uma oitava de cada mão a partir da divisão (direita subindo, esquerda logo abaixo),
    /// com o 5 na ponta, como se estuda a escala de uma oitava.
    UmaOitava,
    /// Posição fixa de cinco dedos na tônica: um dedo por tecla, do 1 ao 5 (esquerda do 5 ao 1).
    CincoDedos,
}

pub const MODOS_MAPA: [ModoMapa; 3] = [
    ModoMapa::EscalaContinua,
    ModoMapa::UmaOitava,
    ModoMapa::CincoDedos,
];

impl ModoMapa {
    pub fn chave(self) -> &'static str {
        match self {
            Self::EscalaContinua => "escala",
            Self::UmaOitava => "uma-oitava",
            Self::CincoDedos => "cinco-dedos",
        }
    }

    pub fn de_chave(s: &str) -> Option<Self> {
        MODOS_MAPA.into_iter().find(|m| m.chave() == s)
    }

    pub fn nome(self) -> &'static str {
        match self {
            Self::EscalaContinua => "Escala em todas as oitavas",
            Self::UmaOitava => "Escala de uma oitava (com o 5)",
            Self::CincoDedos => "Posição de cinco dedos",
        }
    }
}

/// Um dedo do mapa: `cromatico` quando a tecla está fora da escala e o dedo vem da escala
/// cromática (o teclado o desenha mais discreto).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DedoMapa {
    pub nota: NotaMidi,
    pub dedo: u8,
    pub cromatico: bool,
}

/// Dedo da escala cromática (tabela tradicional): pretas sempre com o 3; nas brancas, polegar,
/// salvo onde duas brancas se seguem (Mi–Fá, Si–Dó), em que a direita põe o 2 no Fá e no Dó e a
/// esquerda o 2 no Mi e no Si.
pub fn dedo_cromatico(nota: NotaMidi, esquerda: bool) -> u8 {
    match (nota.numero() % 12, esquerda) {
        (1 | 3 | 6 | 8 | 10, _) => 3,
        (5 | 0, false) | (4 | 11, true) => 2,
        _ => 1,
    }
}

/// Mapa de dedos para decorar, conforme o [`ModoMapa`] (ver cada variante).
pub fn mapa_de_dedos(
    modo: ModoMapa,
    tonica: u8,
    notas: impl IntoIterator<Item = NotaMidi>,
    divisao: u8,
) -> Vec<DedoMapa> {
    let t = tonica % 12;
    let (direita, esquerda) = ESCALA_MAIOR[usize::from(t)];
    let notas: Vec<NotaMidi> = notas.into_iter().collect();
    let tonica_direita = notas
        .iter()
        .map(|n| n.numero())
        .filter(|n| *n >= divisao && n % 12 == t)
        .min();
    let grau_de = |n: u8| {
        let rel = (n + 12 - t) % 12;
        GRAUS_MAIOR.iter().position(|&g| g == rel)
    };
    match modo {
        ModoMapa::EscalaContinua => {
            let escala = mapa_de_escala(t, notas.iter().copied(), divisao);
            notas
                .iter()
                .map(|&n| match escala.iter().find(|(x, _)| *x == n) {
                    Some((_, d)) => DedoMapa {
                        nota: n,
                        dedo: *d,
                        cromatico: false,
                    },
                    None => DedoMapa {
                        nota: n,
                        dedo: dedo_cromatico(n, n.numero() < divisao),
                        cromatico: true,
                    },
                })
                .collect()
        }
        ModoMapa::UmaOitava | ModoMapa::CincoDedos => {
            let Some(td) = tonica_direita.or_else(|| {
                notas
                    .iter()
                    .map(|n| n.numero())
                    .filter(|n| n % 12 == t)
                    .max()
            }) else {
                return Vec::new();
            };
            let te = td.saturating_sub(12);
            let (graus, dd, de): (usize, [u8; 8], [u8; 8]) = if modo == ModoMapa::UmaOitava {
                (8, direita, esquerda)
            } else {
                (5, [1, 2, 3, 4, 5, 0, 0, 0], [5, 4, 3, 2, 1, 0, 0, 0])
            };
            let mut v = Vec::new();
            for &n in &notas {
                let x = n.numero();
                let posicao = |base: u8| -> Option<usize> {
                    if x < base || x > base + 12 {
                        return None;
                    }
                    let g = if x == base + 12 { Some(7) } else { grau_de(x) }?;
                    (g < graus).then_some(g)
                };
                if let Some(g) = posicao(td) {
                    v.push(DedoMapa {
                        nota: n,
                        dedo: dd[g],
                        cromatico: false,
                    });
                } else if x < td
                    && let Some(g) = posicao(te)
                {
                    v.push(DedoMapa {
                        nota: n,
                        dedo: de[g],
                        cromatico: false,
                    });
                }
            }
            v
        }
    }
}

/// Dedo de cada tecla-guia, resolvido pelo próprio teclado quando a tela não informa: escala de
/// uma oitava (8 notas, ou 13 na cromática) pela tabela tradicional, ou acorde de até 5 notas
/// numa oitava pela inversão. Nota sozinha ou desenho desconhecido fica sem dedo.
pub fn inferir(guias: &[NotaMidi], esquerda: bool) -> Vec<(NotaMidi, u8)> {
    let mut notas = guias.to_vec();
    notas.sort_by_key(|n| n.numero());
    notas.dedup();
    let (Some(a), Some(b)) = (notas.first(), notas.last()) else {
        return Vec::new();
    };
    let span = b.numero() - a.numero();
    let dedos = match notas.len() {
        8 | 13 if span == 12 => dedos_de_escala(&notas, esquerda),
        2..=5 if span <= 12 => dedos_de_acorde(&notas, esquerda),
        _ => None,
    };
    dedos.map_or_else(Vec::new, |d| notas.into_iter().zip(d).collect())
}

#[cfg(test)]
mod testes_provedor {

    fn mapa(modo: ModoMapa, tonica: u8) -> Vec<DedoMapa> {
        let notas: Vec<NotaMidi> = (36..=96).filter_map(NotaMidi::nova).collect();
        mapa_de_dedos(modo, tonica, notas, 60)
    }

    fn dedo(m: &[DedoMapa], x: u8) -> Option<u8> {
        m.iter().find(|d| d.nota.numero() == x).map(|d| d.dedo)
    }

    #[test]
    fn escala_continua_cobre_todas_as_teclas_com_as_pretas_no_cromatico() {
        let m = mapa(ModoMapa::EscalaContinua, 0);
        assert_eq!(m.len(), 61, "toda tecla tem dedo");
        assert_eq!(dedo(&m, 61), Some(3), "Dó♯ cromático");
        assert!(
            m.iter()
                .find(|d| d.nota.numero() == 61)
                .is_some_and(|d| d.cromatico)
        );
        assert_eq!(dedo(&m, 60), Some(1));
    }

    #[test]
    fn uma_oitava_tem_o_cinco_na_ponta_das_duas_maos() {
        let m = mapa(ModoMapa::UmaOitava, 0);
        let d: Vec<_> = [60, 62, 64, 65, 67, 69, 71, 72]
            .map(|x| dedo(&m, x))
            .to_vec();
        assert_eq!(d, [1, 2, 3, 1, 2, 3, 4, 5].map(Some).to_vec());
        let e: Vec<_> = [48, 50, 52, 53, 55, 57, 59].map(|x| dedo(&m, x)).to_vec();
        assert_eq!(e, [5, 4, 3, 2, 1, 3, 2].map(Some).to_vec());
        assert_eq!(dedo(&m, 74), None, "fora da oitava");
    }

    #[test]
    fn cinco_dedos_um_por_tecla() {
        let m = mapa(ModoMapa::CincoDedos, 7);
        let d: Vec<_> = [67, 69, 71, 72, 74].map(|x| dedo(&m, x)).to_vec();
        assert_eq!(d, [1, 2, 3, 4, 5].map(Some).to_vec(), "Sol maior, direita");
        let e: Vec<_> = [55, 57, 59, 60, 62].map(|x| dedo(&m, x)).to_vec();
        assert_eq!(
            e,
            [5, 4, 3, 2, 1].map(Some).to_vec(),
            "esquerda uma oitava abaixo"
        );
        assert_eq!(m.len(), 10);
    }

    #[test]
    fn cromatico_tradicional() {
        let n = |x| NotaMidi::nova(x).expect("nota");
        let d: Vec<u8> = (60..=72).map(|x| dedo_cromatico(n(x), false)).collect();
        assert_eq!(d, vec![2, 3, 1, 3, 1, 2, 3, 1, 3, 1, 3, 1, 2]);
    }

    #[test]
    fn mapa_de_do_maior_nas_duas_maos() {
        let notas: Vec<NotaMidi> = (48..=72).filter_map(NotaMidi::nova).collect();
        let m = mapa_de_escala(0, notas, 60);
        let dedo = |x: u8| m.iter().find(|(n, _)| n.numero() == x).map(|(_, d)| *d);
        // Direita a partir do Dó central: 1 2 3 1 2 3 4, e o Dó seguinte de novo 1.
        let d: Vec<_> = [60, 62, 64, 65, 67, 69, 71, 72].map(dedo).to_vec();
        assert_eq!(d, [1, 2, 3, 1, 2, 3, 4, 1].map(Some).to_vec());
        // Esquerda: Dó3 é o começo (5), depois 4 3 2 1 3 2.
        let e: Vec<_> = [48, 50, 52, 53, 55, 57, 59].map(dedo).to_vec();
        assert_eq!(e, [5, 4, 3, 2, 1, 3, 2].map(Some).to_vec());
        assert_eq!(dedo(61), None, "Dó♯ não é de Dó maior");
    }

    #[test]
    fn mapa_de_fa_maior_usa_o_quarto_dedo_no_si_bemol() {
        let notas: Vec<NotaMidi> = (60..=72).filter_map(NotaMidi::nova).collect();
        let m = mapa_de_escala(5, notas, 0);
        assert!(m.contains(&(NotaMidi::nova(70).expect("nota"), 4)));
        assert!(m.iter().all(|(n, _)| n.numero() != 71), "Si natural fora");
    }

    use super::*;

    fn ns(v: &[u8]) -> Vec<NotaMidi> {
        v.iter()
            .map(|x| NotaMidi::nova(*x).expect("nota"))
            .collect()
    }

    fn so_dedos(v: &[u8], esquerda: bool) -> Vec<u8> {
        inferir(&ns(v), esquerda)
            .into_iter()
            .map(|(_, d)| d)
            .collect()
    }

    #[test]
    fn triade_pela_inversao_nas_duas_maos() {
        assert_eq!(so_dedos(&[60, 64, 67], false), vec![1, 3, 5]);
        assert_eq!(so_dedos(&[64, 67, 72], false), vec![1, 2, 5]);
        assert_eq!(so_dedos(&[67, 72, 76], false), vec![1, 3, 5]);
        assert_eq!(so_dedos(&[48, 52, 55], true), vec![5, 3, 1]);
        assert_eq!(so_dedos(&[52, 55, 60], true), vec![5, 3, 1]);
        assert_eq!(so_dedos(&[55, 60, 64], true), vec![5, 2, 1]);
        assert_eq!(so_dedos(&[60, 64, 67, 70], false), vec![1, 2, 3, 5]);
    }

    #[test]
    fn escalas_pela_tabela() {
        assert_eq!(
            so_dedos(&[60, 62, 64, 65, 67, 69, 71, 72], false),
            vec![1, 2, 3, 1, 2, 3, 4, 5]
        );
        assert_eq!(
            so_dedos(&[60, 62, 64, 65, 67, 69, 71, 72], true),
            vec![5, 4, 3, 2, 1, 3, 2, 1]
        );
        // Fá maior: polegar não cai no Sib.
        assert_eq!(
            so_dedos(&[65, 67, 69, 70, 72, 74, 76, 77], false),
            vec![1, 2, 3, 4, 1, 2, 3, 4]
        );
        // Lá menor harmônica.
        assert_eq!(
            so_dedos(&[57, 59, 60, 62, 64, 65, 68, 69], false),
            vec![1, 2, 3, 1, 2, 3, 4, 5]
        );
        let cromatica: Vec<u8> = (60..=72).collect();
        assert_eq!(so_dedos(&cromatica, false)[..6], [1, 3, 1, 3, 1, 2]);
    }

    #[test]
    fn nota_sozinha_e_pentatonica_ficam_sem_dedo() {
        assert!(inferir(&ns(&[60]), false).is_empty());
        assert!(inferir(&ns(&[60, 62, 64, 67, 69, 72]), false).is_empty());
        assert!(inferir(&ns(&[48, 72]), false).is_empty());
    }
}
