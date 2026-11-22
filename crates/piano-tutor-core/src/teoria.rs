//! Teoria musical (F04): classes de altura, intervalos, acordes, inversões e escalas.
//!
//! Código puro, sem GUI nem hardware. A identificação trabalha sobre o **conjunto de notas
//! seguradas ao mesmo tempo** (simultaneidade); sequências (arpejos) são tratadas por quem
//! acumula as notas no tempo. Quando o conjunto não bate com nenhum modelo, nada é rotulado
//! (sem impor nome em caso ambíguo).

use crate::NotaMidi;

/// Classe de altura 0–11 (0 = Dó).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ClasseAltura(u8);

impl ClasseAltura {
    pub const fn nova(valor: u8) -> Self {
        Self(valor % 12)
    }

    pub const fn de_nota(nota: NotaMidi) -> Self {
        Self(nota.numero() % 12)
    }

    pub const fn valor(self) -> u8 {
        self.0
    }

    /// Soma `semitons` (pode ser negativo), sempre módulo 12.
    pub const fn transpor(self, semitons: i32) -> Self {
        Self((((self.0 as i32 + semitons) % 12 + 12) % 12) as u8)
    }

    /// Distância ascendente de `self` até `outra` (0–11).
    pub const fn ate(self, outra: ClasseAltura) -> u8 {
        (outra.0 + 12 - self.0) % 12
    }

    /// Cifra com a grafia preferida para fundamentais: sustenidos em Dó♯, Fá♯ e Sol♯; bemóis
    /// em Mi♭, Lá♭ e Si♭ (como nos cancioneiros). Usa `#` e `b` (a fonte da tela não tem ♯/♭).
    pub const fn cifra(self) -> &'static str {
        const C: [&str; 12] = [
            "C", "C#", "D", "Eb", "E", "F", "F#", "G", "G#", "A", "Bb", "B",
        ];
        C[self.0 as usize]
    }

    /// Nome em português com a mesma grafia de [`ClasseAltura::cifra`].
    pub const fn nome_pt(self) -> &'static str {
        const N: [&str; 12] = [
            "Dó", "Dó#", "Ré", "Mib", "Mi", "Fá", "Fá#", "Sol", "Sol#", "Lá", "Sib", "Si",
        ];
        N[self.0 as usize]
    }
}

/// Nome de um intervalo simples (0–11 semitons) em português.
pub const fn nome_intervalo(semitons: u8) -> &'static str {
    const N: [&str; 12] = [
        "uníssono",
        "segunda menor",
        "segunda maior",
        "terça menor",
        "terça maior",
        "quarta justa",
        "trítono",
        "quinta justa",
        "sexta menor",
        "sexta maior",
        "sétima menor",
        "sétima maior",
    ];
    N[(semitons % 12) as usize]
}

/// Nome do intervalo entre duas notas, com "oitava" e compostos ("décima..." fica como
/// "terça maior + 1 oitava").
pub fn descrever_intervalo(a: NotaMidi, b: NotaMidi) -> String {
    let (grave, aguda) = if a.numero() <= b.numero() {
        (a, b)
    } else {
        (b, a)
    };
    let d = aguda.numero() - grave.numero();
    let oitavas = d / 12;
    let resto = d % 12;
    match (resto, oitavas) {
        (0, 0) => "uníssono".to_owned(),
        (0, 1) => "oitava".to_owned(),
        (0, n) => format!("{n} oitavas"),
        (r, 0) => nome_intervalo(r).to_owned(),
        (r, 1) => format!("{} + 1 oitava", nome_intervalo(r)),
        (r, n) => format!("{} + {n} oitavas", nome_intervalo(r)),
    }
}

/// Qualidade de acorde.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Qualidade {
    Maior,
    Menor,
    Diminuto,
    Aumentado,
    Sus2,
    Sus4,
    Sexta,
    MenorSexta,
    SetimaDominante,
    SetimaMaior,
    MenorSetima,
    MenorSetimaMaior,
    MeioDiminuto,
    Diminuto7,
    Nona,
    Add9,
    Quinta,
}

impl Qualidade {
    /// Todas, na ordem de preferência para identificação (as mais comuns primeiro).
    pub const TODAS: [Qualidade; 17] = [
        Qualidade::Maior,
        Qualidade::Menor,
        Qualidade::SetimaDominante,
        Qualidade::SetimaMaior,
        Qualidade::MenorSetima,
        Qualidade::Diminuto,
        Qualidade::Aumentado,
        Qualidade::Sus4,
        Qualidade::Sus2,
        Qualidade::Sexta,
        Qualidade::MenorSexta,
        Qualidade::MeioDiminuto,
        Qualidade::Diminuto7,
        Qualidade::MenorSetimaMaior,
        Qualidade::Nona,
        Qualidade::Add9,
        Qualidade::Quinta,
    ];

    /// Intervalos a partir da fundamental, em semitons (ordem de empilhamento).
    pub const fn intervalos(self) -> &'static [u8] {
        match self {
            Qualidade::Maior => &[0, 4, 7],
            Qualidade::Menor => &[0, 3, 7],
            Qualidade::Diminuto => &[0, 3, 6],
            Qualidade::Aumentado => &[0, 4, 8],
            Qualidade::Sus2 => &[0, 2, 7],
            Qualidade::Sus4 => &[0, 5, 7],
            Qualidade::Sexta => &[0, 4, 7, 9],
            Qualidade::MenorSexta => &[0, 3, 7, 9],
            Qualidade::SetimaDominante => &[0, 4, 7, 10],
            Qualidade::SetimaMaior => &[0, 4, 7, 11],
            Qualidade::MenorSetima => &[0, 3, 7, 10],
            Qualidade::MenorSetimaMaior => &[0, 3, 7, 11],
            Qualidade::MeioDiminuto => &[0, 3, 6, 10],
            Qualidade::Diminuto7 => &[0, 3, 6, 9],
            Qualidade::Nona => &[0, 4, 7, 10, 14],
            Qualidade::Add9 => &[0, 4, 7, 14],
            Qualidade::Quinta => &[0, 7],
        }
    }

    /// Sufixo da cifra (ex.: "m7", "maj7", "°").
    pub const fn sufixo(self) -> &'static str {
        match self {
            Qualidade::Maior => "",
            Qualidade::Menor => "m",
            Qualidade::Diminuto => "dim",
            Qualidade::Aumentado => "aug",
            Qualidade::Sus2 => "sus2",
            Qualidade::Sus4 => "sus4",
            Qualidade::Sexta => "6",
            Qualidade::MenorSexta => "m6",
            Qualidade::SetimaDominante => "7",
            Qualidade::SetimaMaior => "7M",
            Qualidade::MenorSetima => "m7",
            Qualidade::MenorSetimaMaior => "m(7M)",
            Qualidade::MeioDiminuto => "m7(b5)",
            Qualidade::Diminuto7 => "dim7",
            Qualidade::Nona => "9",
            Qualidade::Add9 => "add9",
            Qualidade::Quinta => "5",
        }
    }

    /// Nome por extenso em português.
    pub const fn nome(self) -> &'static str {
        match self {
            Qualidade::Maior => "maior",
            Qualidade::Menor => "menor",
            Qualidade::Diminuto => "diminuto",
            Qualidade::Aumentado => "aumentado",
            Qualidade::Sus2 => "com segunda suspensa",
            Qualidade::Sus4 => "com quarta suspensa",
            Qualidade::Sexta => "maior com sexta",
            Qualidade::MenorSexta => "menor com sexta",
            Qualidade::SetimaDominante => "com sétima (dominante)",
            Qualidade::SetimaMaior => "com sétima maior",
            Qualidade::MenorSetima => "menor com sétima",
            Qualidade::MenorSetimaMaior => "menor com sétima maior",
            Qualidade::MeioDiminuto => "meio-diminuto",
            Qualidade::Diminuto7 => "diminuto com sétima",
            Qualidade::Nona => "com nona",
            Qualidade::Add9 => "com nona adicionada",
            Qualidade::Quinta => "power chord (quinta)",
        }
    }

    /// Conjunto de classes de altura relativo à fundamental (bitmask de 12 bits).
    pub const fn mascara(self) -> u16 {
        let iv = self.intervalos();
        let mut m = 0u16;
        let mut i = 0;
        while i < iv.len() {
            m |= 1 << (iv[i] % 12);
            i += 1;
        }
        m
    }
}

/// Acorde: fundamental + qualidade (+ baixo, quando invertido).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Acorde {
    pub fundamental: ClasseAltura,
    pub qualidade: Qualidade,
    /// Nota mais grave tocada. Igual à fundamental no estado fundamental.
    pub baixo: ClasseAltura,
}

impl Acorde {
    pub const fn novo(fundamental: ClasseAltura, qualidade: Qualidade) -> Self {
        Self {
            fundamental,
            qualidade,
            baixo: fundamental,
        }
    }

    /// Cifra, por exemplo "Am7" ou "C/E".
    pub fn cifra(&self) -> String {
        let base = format!("{}{}", self.fundamental.cifra(), self.qualidade.sufixo());
        if self.baixo == self.fundamental {
            base
        } else {
            format!("{base}/{}", self.baixo.cifra())
        }
    }

    /// Nome por extenso, por exemplo "Lá menor com sétima".
    pub fn nome(&self) -> String {
        format!("{} {}", self.fundamental.nome_pt(), self.qualidade.nome())
    }

    /// 0 = estado fundamental, 1 = primeira inversão (terça no baixo), 2 = segunda (quinta no
    /// baixo), 3 = terceira (sétima no baixo). `None` se o baixo não pertence ao acorde.
    pub fn inversao(&self) -> Option<usize> {
        let alvo = self.fundamental.ate(self.baixo);
        self.qualidade
            .intervalos()
            .iter()
            .position(|i| i % 12 == alvo)
    }

    /// Texto da inversão em português.
    pub fn texto_inversao(&self) -> &'static str {
        match self.inversao() {
            Some(0) => "estado fundamental",
            Some(1) => "1ª inversão",
            Some(2) => "2ª inversão",
            Some(3) => "3ª inversão",
            Some(_) => "inversão com extensão no baixo",
            None => "baixo fora do acorde",
        }
    }

    /// Notas MIDI do acorde a partir de `base` (fundamental na oitava de `base` ou acima),
    /// já na inversão pedida: a inversão `k` sobe as `k` notas mais graves uma oitava.
    pub fn notas(&self, base: NotaMidi, inversao: usize) -> Vec<NotaMidi> {
        let raiz = base.numero() + ClasseAltura::de_nota(base).ate(self.fundamental);
        let mut v: Vec<u16> = self
            .qualidade
            .intervalos()
            .iter()
            .map(|i| u16::from(raiz) + u16::from(*i))
            .collect();
        let k = inversao.min(v.len().saturating_sub(1));
        for n in v.iter_mut().take(k) {
            *n += 12;
        }
        v.sort_unstable();
        v.into_iter()
            .filter_map(|n| u8::try_from(n).ok().and_then(NotaMidi::nova))
            .collect()
    }
}

fn mascara_de(notas: &[NotaMidi]) -> u16 {
    notas.iter().fold(0u16, |m, n| m | 1 << (n.numero() % 12))
}

fn rotacionar(mascara: u16, raiz: u8) -> u16 {
    // Classe `raiz` vai para o bit 0.
    let r = u32::from(raiz % 12);
    let m = u32::from(mascara);
    (((m >> r) | (m << (12 - r))) & 0xFFF) as u16
}

/// Identifica o acorde de um conjunto de notas simultâneas. Exige ao menos duas classes de
/// altura distintas. Prefere a leitura cuja fundamental é o baixo (estado fundamental); senão,
/// a primeira qualidade de [`Qualidade::TODAS`] que casa exatamente. Sem casamento exato,
/// devolve `None` (não impõe rótulo).
pub fn identificar_acorde(notas: &[NotaMidi]) -> Option<Acorde> {
    let mascara = mascara_de(notas);
    if mascara.count_ones() < 2 {
        return None;
    }
    let baixo = ClasseAltura::de_nota(*notas.iter().min_by_key(|n| n.numero())?);
    let mut melhor: Option<Acorde> = None;
    for q in Qualidade::TODAS {
        for raiz in 0..12u8 {
            if mascara & (1 << raiz) == 0 {
                continue;
            }
            if rotacionar(mascara, raiz) == q.mascara() {
                let a = Acorde {
                    fundamental: ClasseAltura::nova(raiz),
                    qualidade: q,
                    baixo,
                };
                if a.fundamental == baixo {
                    return Some(a);
                }
                if melhor.is_none() {
                    melhor = Some(a);
                }
            }
        }
    }
    melhor
}

/// Tipos de escala.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TipoEscala {
    Maior,
    MenorNatural,
    MenorHarmonica,
    PentatonicaMaior,
    PentatonicaMenor,
    Blues,
    Cromatica,
}

impl TipoEscala {
    pub const TODAS: [TipoEscala; 7] = [
        TipoEscala::Maior,
        TipoEscala::MenorNatural,
        TipoEscala::MenorHarmonica,
        TipoEscala::PentatonicaMaior,
        TipoEscala::PentatonicaMenor,
        TipoEscala::Blues,
        TipoEscala::Cromatica,
    ];

    pub const fn intervalos(self) -> &'static [u8] {
        match self {
            TipoEscala::Maior => &[0, 2, 4, 5, 7, 9, 11],
            TipoEscala::MenorNatural => &[0, 2, 3, 5, 7, 8, 10],
            TipoEscala::MenorHarmonica => &[0, 2, 3, 5, 7, 8, 11],
            TipoEscala::PentatonicaMaior => &[0, 2, 4, 7, 9],
            TipoEscala::PentatonicaMenor => &[0, 3, 5, 7, 10],
            TipoEscala::Blues => &[0, 3, 5, 6, 7, 10],
            TipoEscala::Cromatica => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        }
    }

    pub const fn nome(self) -> &'static str {
        match self {
            TipoEscala::Maior => "maior",
            TipoEscala::MenorNatural => "menor natural",
            TipoEscala::MenorHarmonica => "menor harmônica",
            TipoEscala::PentatonicaMaior => "pentatônica maior",
            TipoEscala::PentatonicaMenor => "pentatônica menor",
            TipoEscala::Blues => "blues",
            TipoEscala::Cromatica => "cromática",
        }
    }

    /// Notas de uma oitava subindo a partir de `base` (inclui a oitava de chegada).
    pub fn notas(self, base: NotaMidi) -> Vec<NotaMidi> {
        self.intervalos()
            .iter()
            .chain(std::iter::once(&12))
            .filter_map(|i| base.numero().checked_add(*i).and_then(NotaMidi::nova))
            .collect()
    }
}

/// Campo harmônico maior: os sete acordes (tríades) da tonalidade maior de `tonica`.
pub fn campo_harmonico_maior(tonica: ClasseAltura) -> [Acorde; 7] {
    const GRAUS: [(u8, Qualidade); 7] = [
        (0, Qualidade::Maior),
        (2, Qualidade::Menor),
        (4, Qualidade::Menor),
        (5, Qualidade::Maior),
        (7, Qualidade::Maior),
        (9, Qualidade::Menor),
        (11, Qualidade::Diminuto),
    ];
    GRAUS.map(|(s, q)| Acorde::novo(tonica.transpor(i32::from(s)), q))
}

/// Algarismos romanos dos graus do campo harmônico maior.
pub const GRAUS_ROMANOS: [&str; 7] = ["I", "ii", "iii", "IV", "V", "vi", "vii°"];

#[cfg(test)]
mod testes {
    use super::*;

    fn n(x: u8) -> NotaMidi {
        NotaMidi::nova(x).expect("nota válida")
    }

    fn ns(v: &[u8]) -> Vec<NotaMidi> {
        v.iter().map(|x| n(*x)).collect()
    }

    #[test]
    fn classes_de_altura_para_os_128_numeros_midi() {
        for x in 0..=127u8 {
            let c = ClasseAltura::de_nota(n(x));
            assert_eq!(c.valor(), x % 12);
            assert_eq!(c.transpor(12), c);
            assert_eq!(c.transpor(-12), c);
        }
        assert_eq!(ClasseAltura::nova(11).transpor(1).valor(), 0);
        assert_eq!(ClasseAltura::nova(0).transpor(-1).valor(), 11);
        assert_eq!(ClasseAltura::nova(9).ate(ClasseAltura::nova(0)), 3);
    }

    #[test]
    fn intervalos_basicos() {
        assert_eq!(descrever_intervalo(n(60), n(64)), "terça maior");
        assert_eq!(descrever_intervalo(n(64), n(60)), "terça maior");
        assert_eq!(descrever_intervalo(n(60), n(67)), "quinta justa");
        assert_eq!(descrever_intervalo(n(60), n(72)), "oitava");
        assert_eq!(descrever_intervalo(n(60), n(60)), "uníssono");
        assert_eq!(descrever_intervalo(n(60), n(76)), "terça maior + 1 oitava");
        assert_eq!(descrever_intervalo(n(60), n(66)), "trítono");
    }

    #[test]
    fn do_mi_sol_e_suas_inversoes() {
        let c = identificar_acorde(&ns(&[60, 64, 67])).expect("C");
        assert_eq!(c.cifra(), "C");
        assert_eq!(c.inversao(), Some(0));
        assert_eq!(c.nome(), "Dó maior");

        let c1 = identificar_acorde(&ns(&[64, 67, 72])).expect("C/E");
        assert_eq!(c1.cifra(), "C/E");
        assert_eq!(c1.inversao(), Some(1));
        assert_eq!(c1.texto_inversao(), "1ª inversão");

        let c2 = identificar_acorde(&ns(&[55, 60, 64])).expect("C/G");
        assert_eq!(c2.cifra(), "C/G");
        assert_eq!(c2.inversao(), Some(2));
    }

    #[test]
    fn qualidades_comuns() {
        let casos: [(&[u8], &str); 12] = [
            (&[57, 60, 64], "Am"),
            (&[55, 59, 62, 65], "G7"),
            (&[60, 64, 67, 71], "C7M"),
            (&[62, 65, 69, 72], "Dm7"),
            (&[59, 62, 65], "Bdim"),
            (&[60, 64, 68], "Caug"),
            (&[60, 65, 67], "Csus4"),
            (&[60, 62, 67], "Csus2"),
            (&[59, 62, 65, 69], "Bm7(b5)"),
            (&[59, 62, 65, 68], "Bdim7"),
            (&[48, 55], "C5"),
            (&[58, 62, 65], "Bb"),
        ];
        for (notas, esperado) in casos {
            let a = identificar_acorde(&ns(notas)).unwrap_or_else(|| panic!("{esperado}"));
            assert_eq!(a.cifra(), esperado, "notas {notas:?}");
        }
    }

    #[test]
    fn notas_dobradas_em_oitavas_nao_mudam_o_acorde() {
        let a = identificar_acorde(&ns(&[48, 60, 64, 67, 72, 76])).expect("C");
        assert_eq!(a.cifra(), "C");
    }

    #[test]
    fn sem_rotulo_quando_nao_ha_casamento() {
        assert_eq!(identificar_acorde(&ns(&[60])), None);
        assert_eq!(identificar_acorde(&ns(&[60, 72])), None);
        assert_eq!(identificar_acorde(&ns(&[60, 61, 62])), None);
        assert_eq!(identificar_acorde(&[]), None);
    }

    #[test]
    fn construir_acorde_e_inversoes_e_reidentificar() {
        for raiz in 0..12u8 {
            for q in Qualidade::TODAS {
                let a = Acorde::novo(ClasseAltura::nova(raiz), q);
                let base = n(48);
                for inv in 0..q.intervalos().len() {
                    let notas = a.notas(base, inv);
                    assert_eq!(notas.len(), q.intervalos().len());
                    let id = identificar_acorde(&notas).expect("reidentifica");
                    // Conjuntos simétricos (aug, dim7) ou equivalentes (C6 = Am7) podem ter
                    // outra leitura; o conjunto de classes precisa ser o mesmo.
                    let m1 = rotacionar(mascara_de(&notas), 0);
                    let m2 = rotacionar(id.qualidade.mascara(), (12 - id.fundamental.valor()) % 12);
                    assert_eq!(m1, m2, "{} inv {inv}", a.cifra());
                }
            }
        }
    }

    #[test]
    fn escalas_e_campo_harmonico() {
        let do_maior = TipoEscala::Maior.notas(n(60));
        assert_eq!(
            do_maior.iter().map(|x| x.numero()).collect::<Vec<_>>(),
            vec![60, 62, 64, 65, 67, 69, 71, 72]
        );
        let campo = campo_harmonico_maior(ClasseAltura::nova(7));
        let cifras: Vec<String> = campo.iter().map(Acorde::cifra).collect();
        assert_eq!(cifras, ["G", "Am", "Bm", "C", "D", "Em", "F#dim"]);
    }
}
