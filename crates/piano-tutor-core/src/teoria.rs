//! Teoria musical (F04): classes de altura, intervalos, acordes, inversões e escalas.
//!
//! Código puro, sem GUI nem hardware. A identificação trabalha sobre o **conjunto de notas
//! seguradas ao mesmo tempo** (simultaneidade); sequências (arpejos) são tratadas por quem
//! acumula as notas no tempo. Quando o conjunto não bate com nenhum modelo, nada é rotulado
//! (sem impor nome em caso ambíguo). Também modela tonalidades (armadura, relativas, grafia
//! enarmônica por contexto) e classifica gestos (nota isolada, acorde, arpejo).

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

/// Identifica o acorde de um conjunto de notas simultâneas. Exige ao menos três classes de
/// altura distintas: duas notas são um intervalo, não um acorde (nem "quinta" tipo C5). Prefere a leitura cuja fundamental é o baixo (estado fundamental); senão,
/// a primeira qualidade de [`Qualidade::TODAS`] que casa exatamente. Sem casamento exato,
/// devolve `None` (não impõe rótulo).
pub fn identificar_acorde(notas: &[NotaMidi]) -> Option<Acorde> {
    let mascara = mascara_de(notas);
    if mascara.count_ones() < 3 {
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

// --- Tonalidades, armadura e grafia por contexto (F04) --------------------------------------

/// Modo de uma tonalidade.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Modo {
    Maior,
    Menor,
}

/// Letras na **linha das quintas** a partir de Fá (posição -1): F C G D A E B.
const LETRAS_DAS_QUINTAS: [char; 7] = ['F', 'C', 'G', 'D', 'A', 'E', 'B'];

/// Menor e maior posição da linha das quintas com no máximo um acidente (Fb = -8, B# = 12).
const POSICAO_MIN: i32 = -8;
const POSICAO_MAX: i32 = 12;

/// Nome da nota na posição `pos` da linha das quintas (Dó = 0, Sol = 1, Fá = -1, ...), com
/// `#` ou `b` repetidos conforme a posição.
fn nome_na_linha_das_quintas(pos: i32) -> String {
    let i = pos + 1;
    let letra = LETRAS_DAS_QUINTAS[i.rem_euclid(7) as usize];
    let acidentes = i.div_euclid(7);
    let mut nome = String::from(letra);
    for _ in 0..acidentes.unsigned_abs() {
        nome.push(if acidentes > 0 { '#' } else { 'b' });
    }
    nome
}

/// Classe de altura da posição `pos` na linha das quintas.
fn classe_na_linha_das_quintas(pos: i32) -> u8 {
    (pos * 7).rem_euclid(12) as u8
}

/// Tonalidade declarada pela **armadura** (número de quintas de -7 a +7; negativo = bemóis) e
/// pelo modo. É a armadura que fixa a grafia: Fá♯ maior (6 sustenidos) e Sol♭ maior
/// (6 bemóis) soam iguais, mas são tonalidades diferentes. Nenhuma grafia é imposta fora do
/// contexto declarado: veja [`Tonalidade::candidatas`] e [`Tonalidade::grafia`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Tonalidade {
    quintas: i8,
    modo: Modo,
}

impl Tonalidade {
    /// Tonalidade da armadura com `quintas` acidentes (-7 a +7); `None` fora disso.
    pub const fn da_armadura(quintas: i8, modo: Modo) -> Option<Self> {
        if quintas >= -7 && quintas <= 7 {
            Some(Self { quintas, modo })
        } else {
            None
        }
    }

    /// Todas as tonalidades (de 7 bemóis a 7 sustenidos) com essa tônica e modo, em ordem
    /// crescente de quintas. Duas quando a grafia é enarmônica (Sol♭ e Fá♯ maior), senão uma.
    pub fn candidatas(tonica: ClasseAltura, modo: Modo) -> Vec<Self> {
        (-7..=7i8)
            .filter_map(|q| Self::da_armadura(q, modo))
            .filter(|t| t.tonica() == tonica)
            .collect()
    }

    pub const fn modo(self) -> Modo {
        self.modo
    }

    /// Quintas da armadura: negativo = bemóis, positivo = sustenidos.
    pub const fn armadura_quintas(self) -> i8 {
        self.quintas
    }

    pub const fn numero_de_acidentes(self) -> u8 {
        self.quintas.unsigned_abs()
    }

    /// Posição da tônica na linha das quintas (a menor está três quintas acima da relativa
    /// maior).
    const fn posicao_da_tonica(self) -> i32 {
        self.quintas as i32
            + match self.modo {
                Modo::Maior => 0,
                Modo::Menor => 3,
            }
    }

    pub fn tonica(self) -> ClasseAltura {
        ClasseAltura::nova(classe_na_linha_das_quintas(self.posicao_da_tonica()))
    }

    /// Nome da tônica na grafia da própria tonalidade ("F#", "Gb", "A").
    pub fn nome_tonica(self) -> String {
        nome_na_linha_das_quintas(self.posicao_da_tonica())
    }

    /// Cifra da tonalidade: "G" (Sol maior), "Em" (Mi menor).
    pub fn cifra(self) -> String {
        match self.modo {
            Modo::Maior => self.nome_tonica(),
            Modo::Menor => format!("{}m", self.nome_tonica()),
        }
    }

    /// Relativa: mesma armadura, outro modo (Dó maior ↔ Lá menor).
    pub const fn relativa(self) -> Self {
        Self {
            quintas: self.quintas,
            modo: match self.modo {
                Modo::Maior => Modo::Menor,
                Modo::Menor => Modo::Maior,
            },
        }
    }

    /// Paralela: mesma tônica (e mesma letra), outro modo (Dó maior ↔ Dó menor). `None` quando
    /// a armadura da paralela passaria de 7 acidentes (por exemplo Ré♯ maior).
    pub fn paralela(self) -> Option<Self> {
        let (quintas, modo) = match self.modo {
            Modo::Maior => (i32::from(self.quintas) - 3, Modo::Menor),
            Modo::Menor => (i32::from(self.quintas) + 3, Modo::Maior),
        };
        i8::try_from(quintas)
            .ok()
            .and_then(|q| Self::da_armadura(q, modo))
    }

    /// Posições, na linha das quintas, dos sete graus da escala (maior ou menor natural).
    fn posicoes_da_escala(self) -> [i32; 7] {
        let t = self.posicao_da_tonica();
        match self.modo {
            Modo::Maior => [t, t + 2, t + 4, t - 1, t + 1, t + 3, t + 5],
            Modo::Menor => [t, t + 2, t - 3, t - 1, t + 1, t - 4, t - 2],
        }
    }

    /// Os sete graus da escala (maior, ou menor natural) com a grafia da tonalidade.
    pub fn escala_nomes(self) -> [String; 7] {
        self.posicoes_da_escala().map(nome_na_linha_das_quintas)
    }

    /// Classes de altura da escala diatônica da tonalidade.
    pub fn classes(self) -> std::collections::BTreeSet<u8> {
        self.posicoes_da_escala()
            .into_iter()
            .map(classe_na_linha_das_quintas)
            .collect()
    }

    /// Acidentes escritos na armadura, na ordem tradicional (F# C# G# D# A# E# B# ou
    /// Bb Eb Ab Db Gb Cb Fb).
    pub fn acidentes_da_armadura(self) -> Vec<String> {
        let n = usize::from(self.numero_de_acidentes());
        let letras: Vec<char> = if self.quintas >= 0 {
            LETRAS_DAS_QUINTAS.to_vec()
        } else {
            LETRAS_DAS_QUINTAS.iter().rev().copied().collect()
        };
        let sinal = if self.quintas >= 0 { '#' } else { 'b' };
        letras
            .into_iter()
            .take(n)
            .map(|l| format!("{l}{sinal}"))
            .collect()
    }

    /// Grafia de `classe` no contexto desta tonalidade: as notas da escala mantêm o nome da
    /// armadura (Fá♯ em Sol maior, Sol♭ em Ré♭ maior, Dó♭ em Sol♭ maior); as demais usam a
    /// grafia mais próxima do centro da tonalidade (sustenidos em armaduras de sustenido,
    /// bemóis em armaduras de bemol). Sempre com no máximo um acidente.
    pub fn grafia(self, classe: ClasseAltura) -> String {
        // Centro da tonalidade na linha das quintas, em meias posições. A menor puxa meia
        // posição para cima, onde mora a sensível alterada (Sol♯ em Lá menor).
        let centro2 = 2 * i32::from(self.quintas)
            + match self.modo {
                Modo::Maior => 4,
                Modo::Menor => 5,
            };
        let base = i32::from(classe.valor()) * 7 % 12;
        let melhor = (POSICAO_MIN..=POSICAO_MAX)
            .filter(|p| (p - base).rem_euclid(12) == 0)
            .min_by_key(|p| ((2 * p - centro2).abs(), *p))
            .unwrap_or(base);
        nome_na_linha_das_quintas(melhor)
    }
}

// --- Simultaneidade × sequência × nota isolada (F04) ----------------------------------------

/// Nota tocada com o instante de ataque (nanossegundos, mesma base de [`crate::tempo`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NotaTocada {
    pub nota: NotaMidi,
    pub inicio_ns: u64,
}

/// Limites de tempo entre ataques consecutivos para classificar um gesto.
///
/// - intervalo `<= simultanea_ns`: mesma simultaneidade (os dedos nunca descem juntos);
/// - `simultanea_ns < intervalo < sequencia_min_ns`: zona cinzenta, **sem rótulo**;
/// - `sequencia_min_ns <= intervalo <= sequencia_max_ns`: notas em sequência (arpejo);
/// - acima de `sequencia_max_ns`: notas separadas (frases independentes).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JanelasGesto {
    pub simultanea_ns: u64,
    pub sequencia_min_ns: u64,
    pub sequencia_max_ns: u64,
}

impl JanelasGesto {
    /// 50 ms, 120 ms e 1,5 s: valores iniciais de projeto, **não calibrados** com mãos reais
    /// nem com o teclado do projeto (NÃO VERIFICADO fora de teste unitário).
    pub const PADRAO: JanelasGesto = JanelasGesto {
        simultanea_ns: 50_000_000,
        sequencia_min_ns: 120_000_000,
        sequencia_max_ns: 1_500_000_000,
    };
}

/// Direção de um arpejo.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirecaoArpejo {
    Ascendente,
    Descendente,
    Misto,
}

/// Resultado de [`classificar_gesto`]. Só há rótulo de acorde quando existem ao menos três
/// classes de altura distintas e o conjunto casa com um modelo; pares nunca são rotulados.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Gesto {
    Vazio,
    NotaIsolada(NotaMidi),
    /// Ataques praticamente juntos (do mais grave ao mais agudo).
    Simultaneo {
        notas: Vec<NotaMidi>,
        acorde: Option<Acorde>,
    },
    /// Três ou mais notas em sequência cujo conjunto forma um acorde reconhecido.
    Arpejo {
        notas: Vec<NotaMidi>,
        direcao: DirecaoArpejo,
        acorde: Acorde,
    },
    /// Notas em sequência sem formar acorde (melodia, escala, par de notas).
    Sequencia {
        notas: Vec<NotaMidi>,
    },
    /// Notas tão espaçadas que não formam um único gesto.
    NotasSeparadas {
        notas: Vec<NotaMidi>,
    },
    /// Tempos na zona cinzenta ou mistura de juntas e sequenciais: nenhum rótulo é imposto.
    Ambiguo,
}

fn acorde_se_ha_tres_classes(notas: &[NotaMidi]) -> Option<Acorde> {
    if mascara_de(notas).count_ones() >= 3 {
        identificar_acorde(notas)
    } else {
        None
    }
}

/// Classifica um trecho curto de notas (já recortado pelo chamador) como nota isolada, acorde
/// (simultaneidade), arpejo/sequência ou ambíguo. Trabalha só com os instantes de ataque; a
/// ordem de chegada dos eventos não importa. Puro e determinístico.
pub fn classificar_gesto(eventos: &[NotaTocada], janelas: JanelasGesto) -> Gesto {
    let mut v: Vec<NotaTocada> = eventos.to_vec();
    v.sort_by_key(|e| (e.inicio_ns, e.nota.numero()));
    match v.len() {
        0 => return Gesto::Vazio,
        1 => return Gesto::NotaIsolada(v[0].nota),
        _ => {}
    }
    let intervalos: Vec<u64> = v
        .windows(2)
        .map(|w| w[1].inicio_ns - w[0].inicio_ns)
        .collect();
    let juntos = |i: &u64| *i <= janelas.simultanea_ns;
    let sequenciais = |i: &u64| *i >= janelas.sequencia_min_ns && *i <= janelas.sequencia_max_ns;
    let distantes = |i: &u64| *i > janelas.sequencia_max_ns;
    let em_ordem: Vec<NotaMidi> = v.iter().map(|e| e.nota).collect();

    if intervalos.iter().all(juntos) {
        // Uma corrida rápida de notas não é um acorde: a mão inteira desce numa janela curta.
        let espalhamento = v[v.len() - 1].inicio_ns - v[0].inicio_ns;
        if espalhamento > janelas.simultanea_ns.saturating_mul(2) {
            return Gesto::Ambiguo;
        }
        let mut notas = em_ordem;
        notas.sort_by_key(|n| n.numero());
        let acorde = acorde_se_ha_tres_classes(&notas);
        return Gesto::Simultaneo { notas, acorde };
    }
    if intervalos.iter().all(distantes) {
        return Gesto::NotasSeparadas { notas: em_ordem };
    }
    if intervalos.iter().all(sequenciais) {
        return match acorde_se_ha_tres_classes(&em_ordem) {
            Some(acorde) => {
                let sobe = em_ordem.windows(2).all(|w| w[0].numero() < w[1].numero());
                let desce = em_ordem.windows(2).all(|w| w[0].numero() > w[1].numero());
                let direcao = if sobe {
                    DirecaoArpejo::Ascendente
                } else if desce {
                    DirecaoArpejo::Descendente
                } else {
                    DirecaoArpejo::Misto
                };
                Gesto::Arpejo {
                    notas: em_ordem,
                    direcao,
                    acorde,
                }
            }
            None => Gesto::Sequencia { notas: em_ordem },
        };
    }
    Gesto::Ambiguo
}

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
        let casos: [(&[u8], &str); 11] = [
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
        assert_eq!(identificar_acorde(&ns(&[48, 55])), None, "duas notas não são acorde (C5)");
        assert_eq!(identificar_acorde(&ns(&[60, 61, 62])), None);
        assert_eq!(identificar_acorde(&[]), None);
    }

    #[test]
    fn construir_acorde_e_inversoes_e_reidentificar() {
        for raiz in 0..12u8 {
            for q in Qualidade::TODAS {
                if q.intervalos().len() < 3 {
                    continue; // "5" (power chord) é intervalo: não é identificado como acorde.
                }
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

    // --- F04-1: escalas menores e todas as tonalidades -------------------------------------

    fn classes(v: &[NotaMidi]) -> std::collections::BTreeSet<u8> {
        v.iter().map(|x| x.numero() % 12).collect()
    }

    #[test]
    fn escalas_para_as_12_tonicas_seguem_a_formula_de_intervalos() {
        for tipo in TipoEscala::TODAS {
            for tonica in 48..60u8 {
                let notas = tipo.notas(n(tonica));
                assert_eq!(notas.len(), tipo.intervalos().len() + 1, "{}", tipo.nome());
                assert_eq!(notas[0].numero(), tonica);
                assert_eq!(notas.last().map(|x| x.numero()), Some(tonica + 12));
                for (nota, i) in notas.iter().zip(tipo.intervalos()) {
                    assert_eq!(nota.numero(), tonica + i, "{}", tipo.nome());
                }
                assert!(notas.windows(2).all(|w| w[0].numero() < w[1].numero()));
            }
        }
    }

    #[test]
    fn escalas_menores_de_la_e_de_do() {
        let la_menor: Vec<u8> = TipoEscala::MenorNatural
            .notas(n(57))
            .iter()
            .map(|x| x.numero())
            .collect();
        assert_eq!(la_menor, vec![57, 59, 60, 62, 64, 65, 67, 69]);
        let la_harmonica: Vec<u8> = TipoEscala::MenorHarmonica
            .notas(n(57))
            .iter()
            .map(|x| x.numero())
            .collect();
        assert_eq!(la_harmonica, vec![57, 59, 60, 62, 64, 65, 68, 69]);
        let do_menor: Vec<u8> = TipoEscala::MenorNatural
            .notas(n(60))
            .iter()
            .map(|x| x.numero())
            .collect();
        assert_eq!(do_menor, vec![60, 62, 63, 65, 67, 68, 70, 72]);
    }

    #[test]
    fn tonalidade_relativa_menor_tem_as_mesmas_notas_da_maior() {
        for tonica in 0..12u8 {
            let maior = TipoEscala::Maior.notas(n(48 + tonica));
            let relativa_menor = n(48 + (tonica + 9) % 12);
            let menor = TipoEscala::MenorNatural.notas(relativa_menor);
            assert_eq!(classes(&maior), classes(&menor), "tônica {tonica}");
        }
    }

    #[test]
    fn as_15_tonalidades_maiores_tem_nome_e_armadura_corretos() {
        // (quintas, nome da tônica, sete notas da escala na grafia da tonalidade)
        let casos: [(i8, &str, [&str; 7]); 15] = [
            (-7, "Cb", ["Cb", "Db", "Eb", "Fb", "Gb", "Ab", "Bb"]),
            (-6, "Gb", ["Gb", "Ab", "Bb", "Cb", "Db", "Eb", "F"]),
            (-5, "Db", ["Db", "Eb", "F", "Gb", "Ab", "Bb", "C"]),
            (-4, "Ab", ["Ab", "Bb", "C", "Db", "Eb", "F", "G"]),
            (-3, "Eb", ["Eb", "F", "G", "Ab", "Bb", "C", "D"]),
            (-2, "Bb", ["Bb", "C", "D", "Eb", "F", "G", "A"]),
            (-1, "F", ["F", "G", "A", "Bb", "C", "D", "E"]),
            (0, "C", ["C", "D", "E", "F", "G", "A", "B"]),
            (1, "G", ["G", "A", "B", "C", "D", "E", "F#"]),
            (2, "D", ["D", "E", "F#", "G", "A", "B", "C#"]),
            (3, "A", ["A", "B", "C#", "D", "E", "F#", "G#"]),
            (4, "E", ["E", "F#", "G#", "A", "B", "C#", "D#"]),
            (5, "B", ["B", "C#", "D#", "E", "F#", "G#", "A#"]),
            (6, "F#", ["F#", "G#", "A#", "B", "C#", "D#", "E#"]),
            (7, "C#", ["C#", "D#", "E#", "F#", "G#", "A#", "B#"]),
        ];
        for (quintas, nome, escala) in casos {
            let t = Tonalidade::da_armadura(quintas, Modo::Maior).expect("armadura válida");
            assert_eq!(t.nome_tonica(), nome, "{quintas} quintas");
            assert_eq!(t.cifra(), nome);
            assert_eq!(t.escala_nomes(), escala, "{nome} maior");
            assert_eq!(t.numero_de_acidentes(), quintas.unsigned_abs());
            assert_eq!(t.armadura_quintas(), quintas);
            // As notas escritas formam exatamente a escala maior da tônica.
            let base = n(60 + t.tonica().valor());
            assert_eq!(
                t.classes(),
                classes(&TipoEscala::Maior.notas(base)),
                "{nome} maior"
            );
        }
    }

    #[test]
    fn as_15_tonalidades_menores_tem_nome_e_armadura_corretos() {
        let casos: [(i8, &str, [&str; 7]); 15] = [
            (-7, "Ab", ["Ab", "Bb", "Cb", "Db", "Eb", "Fb", "Gb"]),
            (-6, "Eb", ["Eb", "F", "Gb", "Ab", "Bb", "Cb", "Db"]),
            (-5, "Bb", ["Bb", "C", "Db", "Eb", "F", "Gb", "Ab"]),
            (-4, "F", ["F", "G", "Ab", "Bb", "C", "Db", "Eb"]),
            (-3, "C", ["C", "D", "Eb", "F", "G", "Ab", "Bb"]),
            (-2, "G", ["G", "A", "Bb", "C", "D", "Eb", "F"]),
            (-1, "D", ["D", "E", "F", "G", "A", "Bb", "C"]),
            (0, "A", ["A", "B", "C", "D", "E", "F", "G"]),
            (1, "E", ["E", "F#", "G", "A", "B", "C", "D"]),
            (2, "B", ["B", "C#", "D", "E", "F#", "G", "A"]),
            (3, "F#", ["F#", "G#", "A", "B", "C#", "D", "E"]),
            (4, "C#", ["C#", "D#", "E", "F#", "G#", "A", "B"]),
            (5, "G#", ["G#", "A#", "B", "C#", "D#", "E", "F#"]),
            (6, "D#", ["D#", "E#", "F#", "G#", "A#", "B", "C#"]),
            (7, "A#", ["A#", "B#", "C#", "D#", "E#", "F#", "G#"]),
        ];
        for (quintas, nome, escala) in casos {
            let t = Tonalidade::da_armadura(quintas, Modo::Menor).expect("armadura válida");
            assert_eq!(t.nome_tonica(), nome, "{quintas} quintas");
            assert_eq!(t.cifra(), format!("{nome}m"));
            assert_eq!(t.escala_nomes(), escala, "{nome} menor");
            assert_eq!(t.numero_de_acidentes(), quintas.unsigned_abs());
            let base = n(60 + t.tonica().valor());
            assert_eq!(
                t.classes(),
                classes(&TipoEscala::MenorNatural.notas(base)),
                "{nome} menor"
            );
        }
    }

    #[test]
    fn relativas_e_paralelas() {
        for quintas in -7..=7i8 {
            let maior = Tonalidade::da_armadura(quintas, Modo::Maior).expect("válida");
            let menor = maior.relativa();
            assert_eq!(menor.modo(), Modo::Menor);
            assert_eq!(menor.armadura_quintas(), quintas, "mesma armadura");
            assert_eq!(menor.relativa(), maior);
            assert_eq!(menor.tonica(), maior.tonica().transpor(9));
            assert_eq!(maior.classes(), menor.classes());
        }
        // Dó maior ↔ Lá menor; Sol maior ↔ Mi menor; Mib maior ↔ Dó menor.
        let de = |q, m| Tonalidade::da_armadura(q, m).expect("válida");
        assert_eq!(de(0, Modo::Maior).relativa().cifra(), "Am");
        assert_eq!(de(1, Modo::Maior).relativa().cifra(), "Em");
        assert_eq!(de(-3, Modo::Maior).relativa().cifra(), "Cm");
        assert_eq!(de(-3, Modo::Menor).relativa().cifra(), "Eb");
        // Paralela: mesma tônica, modo trocado (Dó maior → Dó menor, três bemóis).
        let do_menor = de(0, Modo::Maior).paralela().expect("Dó menor existe");
        assert_eq!(do_menor.cifra(), "Cm");
        assert_eq!(do_menor.armadura_quintas(), -3);
        assert_eq!(do_menor.paralela().expect("Dó maior").cifra(), "C");
        // Lá maior ↔ Lá menor (três sustenidos → nenhum).
        assert_eq!(
            de(3, Modo::Maior).paralela().expect("Lá menor").cifra(),
            "Am"
        );
        // Fora do círculo de 7 acidentes: Ré♯ maior (9 sustenidos) e Dó♭ menor não existem.
        assert_eq!(de(6, Modo::Menor).paralela(), None);
        assert_eq!(de(-7, Modo::Maior).paralela(), None);
    }

    #[test]
    fn armadura_fora_de_7_bemois_ou_7_sustenidos_e_recusada() {
        assert_eq!(Tonalidade::da_armadura(8, Modo::Maior), None);
        assert_eq!(Tonalidade::da_armadura(-8, Modo::Menor), None);
        assert!(Tonalidade::da_armadura(7, Modo::Maior).is_some());
        assert!(Tonalidade::da_armadura(-7, Modo::Menor).is_some());
    }

    #[test]
    fn acidentes_da_armadura_seguem_a_ordem_tradicional() {
        let de = |q| Tonalidade::da_armadura(q, Modo::Maior).expect("válida");
        assert!(de(0).acidentes_da_armadura().is_empty());
        assert_eq!(de(1).acidentes_da_armadura(), ["F#"]);
        assert_eq!(de(2).acidentes_da_armadura(), ["F#", "C#"]);
        assert_eq!(
            de(7).acidentes_da_armadura(),
            ["F#", "C#", "G#", "D#", "A#", "E#", "B#"]
        );
        assert_eq!(de(-1).acidentes_da_armadura(), ["Bb"]);
        assert_eq!(de(-3).acidentes_da_armadura(), ["Bb", "Eb", "Ab"]);
        assert_eq!(
            de(-7).acidentes_da_armadura(),
            ["Bb", "Eb", "Ab", "Db", "Gb", "Cb", "Fb"]
        );
    }

    #[test]
    fn tonalidades_possiveis_para_uma_tonica_incluem_as_grafias_enarmonicas() {
        // Sem impor grafia: Fá♯/Sol♭ maior e Ré♯/Mi♭ menor têm duas armaduras possíveis.
        let fs = Tonalidade::candidatas(ClasseAltura::nova(6), Modo::Maior);
        let nomes: Vec<String> = fs.into_iter().map(Tonalidade::cifra).collect();
        assert_eq!(nomes, ["Gb", "F#"]);
        let ds = Tonalidade::candidatas(ClasseAltura::nova(3), Modo::Menor);
        let nomes: Vec<String> = ds.into_iter().map(Tonalidade::cifra).collect();
        assert_eq!(nomes, ["Ebm", "D#m"]);
        // Dó♯/Ré♭ e Si/Dó♭ maiores também têm duas; Sol maior só uma.
        assert_eq!(
            Tonalidade::candidatas(ClasseAltura::nova(1), Modo::Maior).len(),
            2
        );
        assert_eq!(
            Tonalidade::candidatas(ClasseAltura::nova(11), Modo::Maior).len(),
            2
        );
        assert_eq!(
            Tonalidade::candidatas(ClasseAltura::nova(7), Modo::Maior).len(),
            1
        );
        // Todas as 12 tônicas, nos dois modos, têm ao menos uma tonalidade.
        for t in 0..12u8 {
            for modo in [Modo::Maior, Modo::Menor] {
                let c = Tonalidade::candidatas(ClasseAltura::nova(t), modo);
                assert!(!c.is_empty(), "tônica {t} {modo:?}");
                assert!(
                    c.iter()
                        .all(|x| x.tonica().valor() == t && x.modo() == modo)
                );
            }
        }
    }

    // --- F04-3: enarmonia depende do contexto declarado -------------------------------------

    #[test]
    fn mesma_altura_tem_grafias_diferentes_conforme_a_tonalidade() {
        let fa_sus = ClasseAltura::nova(6);
        let de = |q, m| Tonalidade::da_armadura(q, m).expect("válida");
        // Fá♯ em Sol maior e em Ré maior; Sol♭ em Ré♭ maior e em Sol♭ maior.
        assert_eq!(de(1, Modo::Maior).grafia(fa_sus), "F#");
        assert_eq!(de(2, Modo::Maior).grafia(fa_sus), "F#");
        assert_eq!(de(-5, Modo::Maior).grafia(fa_sus), "Gb");
        assert_eq!(de(-6, Modo::Maior).grafia(fa_sus), "Gb");
        assert_eq!(de(6, Modo::Maior).grafia(fa_sus), "F#");
        // Si♭ × Lá♯ (classe 10).
        let si_bemol = ClasseAltura::nova(10);
        assert_eq!(de(-1, Modo::Maior).grafia(si_bemol), "Bb");
        assert_eq!(de(5, Modo::Maior).grafia(si_bemol), "A#");
        // Dó♭ × Si (classe 11): em Sol♭ maior escreve-se Dó♭; em Si maior, Si.
        let si = ClasseAltura::nova(11);
        assert_eq!(de(-6, Modo::Maior).grafia(si), "Cb");
        assert_eq!(de(5, Modo::Maior).grafia(si), "B");
        // Mi♯ × Fá (classe 5): Mi♯ só aparece em Fá♯ maior / Dó♯ maior.
        let fa = ClasseAltura::nova(5);
        assert_eq!(de(6, Modo::Maior).grafia(fa), "E#");
        assert_eq!(de(7, Modo::Maior).grafia(fa), "E#");
        assert_eq!(de(0, Modo::Maior).grafia(fa), "F");
        // Nas menores: Sol♯ (sensível) em Lá menor; Mi♭ em Dó menor.
        assert_eq!(de(0, Modo::Menor).grafia(ClasseAltura::nova(8)), "G#");
        assert_eq!(de(-3, Modo::Menor).grafia(ClasseAltura::nova(3)), "Eb");
    }

    #[test]
    fn grafia_sempre_representa_a_mesma_classe_de_altura() {
        // Para qualquer tonalidade e qualquer classe, o nome escolhido soa na classe pedida.
        for quintas in -7..=7i8 {
            for modo in [Modo::Maior, Modo::Menor] {
                let t = Tonalidade::da_armadura(quintas, modo).expect("válida");
                for c in 0..12u8 {
                    let nome = t.grafia(ClasseAltura::nova(c));
                    assert_eq!(classe_do_nome(&nome), c, "{} em {}: {nome}", c, t.cifra());
                }
                // As notas da própria escala mantêm a grafia da tonalidade.
                for nome in t.escala_nomes() {
                    let c = ClasseAltura::nova(classe_do_nome(&nome));
                    assert_eq!(t.grafia(c), nome, "{} {nome}", t.cifra());
                }
            }
        }
    }

    /// Interpreta "F#", "Bb", "E#", "Cb" etc. (testes apenas).
    fn classe_do_nome(nome: &str) -> u8 {
        let mut it = nome.chars();
        let base = match it.next().expect("nome não vazio") {
            'C' => 0i32,
            'D' => 2,
            'E' => 4,
            'F' => 5,
            'G' => 7,
            'A' => 9,
            'B' => 11,
            outro => panic!("letra {outro}"),
        };
        let alt: i32 = it.map(|c| if c == '#' { 1 } else { -1 }).sum();
        ((base + alt).rem_euclid(12)) as u8
    }

    // --- F04-2: simultaneidade × sequência × nota isolada -------------------------------------

    fn tocar(v: &[(u8, u64)]) -> Vec<NotaTocada> {
        v.iter()
            .map(|(x, ms)| NotaTocada {
                nota: n(*x),
                inicio_ns: ms * 1_000_000,
            })
            .collect()
    }

    #[test]
    fn nota_unica_e_nota_isolada_sem_acorde() {
        let g = classificar_gesto(&tocar(&[(60, 0)]), JanelasGesto::PADRAO);
        assert_eq!(g, Gesto::NotaIsolada(n(60)));
        assert_eq!(classificar_gesto(&[], JanelasGesto::PADRAO), Gesto::Vazio);
    }

    #[test]
    fn do_mi_sol_quase_juntos_e_acorde_simultaneo() {
        // Até 30 ms de diferença entre os dedos: ainda é um acorde (mão humana).
        let g = classificar_gesto(&tocar(&[(60, 0), (64, 12), (67, 30)]), JanelasGesto::PADRAO);
        match g {
            Gesto::Simultaneo { notas, acorde } => {
                assert_eq!(notas, ns(&[60, 64, 67]));
                assert_eq!(acorde.expect("C").cifra(), "C");
            }
            outro => panic!("esperava simultâneo, veio {outro:?}"),
        }
        // Inversão C/E.
        let g = classificar_gesto(&tocar(&[(64, 0), (67, 5), (72, 9)]), JanelasGesto::PADRAO);
        match g {
            Gesto::Simultaneo { acorde, .. } => {
                assert_eq!(acorde.expect("C/E").cifra(), "C/E");
            }
            outro => panic!("{outro:?}"),
        }
    }

    #[test]
    fn do_mi_sol_em_sequencia_e_arpejo_ascendente() {
        let g = classificar_gesto(
            &tocar(&[(60, 0), (64, 300), (67, 600)]),
            JanelasGesto::PADRAO,
        );
        match g {
            Gesto::Arpejo {
                notas,
                direcao,
                acorde,
            } => {
                assert_eq!(notas, ns(&[60, 64, 67]));
                assert_eq!(direcao, DirecaoArpejo::Ascendente);
                assert_eq!(acorde.cifra(), "C");
            }
            outro => panic!("esperava arpejo, veio {outro:?}"),
        }
    }

    #[test]
    fn arpejo_descendente_e_arpejo_de_inversao() {
        let g = classificar_gesto(
            &tocar(&[(72, 0), (67, 250), (64, 500), (60, 750)]),
            JanelasGesto::PADRAO,
        );
        match g {
            Gesto::Arpejo {
                direcao, acorde, ..
            } => {
                assert_eq!(direcao, DirecaoArpejo::Descendente);
                assert_eq!(acorde.cifra(), "C");
            }
            outro => panic!("{outro:?}"),
        }
        // Mi-Sol-Dó (C/E) em sequência: o baixo é a primeira nota tocada? Não: o baixo do
        // conjunto é a nota mais grave (Mi), então a cifra é C/E.
        let g = classificar_gesto(
            &tocar(&[(64, 0), (67, 300), (72, 600)]),
            JanelasGesto::PADRAO,
        );
        match g {
            Gesto::Arpejo { acorde, .. } => assert_eq!(acorde.cifra(), "C/E"),
            outro => panic!("{outro:?}"),
        }
    }

    #[test]
    fn arpejo_de_ordem_misturada_tem_direcao_mista() {
        let g = classificar_gesto(
            &tocar(&[(60, 0), (67, 300), (64, 600)]),
            JanelasGesto::PADRAO,
        );
        match g {
            Gesto::Arpejo { direcao, .. } => assert_eq!(direcao, DirecaoArpejo::Misto),
            outro => panic!("{outro:?}"),
        }
    }

    #[test]
    fn duas_notas_nunca_ganham_rotulo_de_acorde_nem_de_arpejo() {
        // Em par simultâneo: só o intervalo, sem acorde.
        let g = classificar_gesto(&tocar(&[(60, 0), (64, 10)]), JanelasGesto::PADRAO);
        assert_eq!(
            g,
            Gesto::Simultaneo {
                notas: ns(&[60, 64]),
                acorde: None
            }
        );
        // Quinta justa (C5): continua sem rótulo de acorde no gesto.
        let g = classificar_gesto(&tocar(&[(48, 0), (55, 0)]), JanelasGesto::PADRAO);
        assert_eq!(
            g,
            Gesto::Simultaneo {
                notas: ns(&[48, 55]),
                acorde: None
            }
        );
        // Em sequência: apenas notas em sequência, sem arpejo nem acorde.
        let g = classificar_gesto(&tocar(&[(60, 0), (64, 400)]), JanelasGesto::PADRAO);
        assert_eq!(
            g,
            Gesto::Sequencia {
                notas: ns(&[60, 64])
            }
        );
    }

    #[test]
    fn sequencia_que_nao_forma_acorde_nao_vira_arpejo() {
        // Dó-Ré-Mi (escala): três notas em sequência sem casamento de acorde.
        let g = classificar_gesto(
            &tocar(&[(60, 0), (62, 300), (64, 600)]),
            JanelasGesto::PADRAO,
        );
        assert_eq!(
            g,
            Gesto::Sequencia {
                notas: ns(&[60, 62, 64])
            }
        );
        // Acorde simultâneo sem casamento (cluster): simultâneo, sem rótulo.
        let g = classificar_gesto(&tocar(&[(60, 0), (61, 0), (62, 0)]), JanelasGesto::PADRAO);
        assert_eq!(
            g,
            Gesto::Simultaneo {
                notas: ns(&[60, 61, 62]),
                acorde: None
            }
        );
    }

    #[test]
    fn intervalo_entre_simultaneo_e_sequencia_e_ambiguo() {
        // 80 ms entre as notas: nem simultâneo (≤ 50 ms) nem sequência (≥ 120 ms).
        let g = classificar_gesto(
            &tocar(&[(60, 0), (64, 80), (67, 160)]),
            JanelasGesto::PADRAO,
        );
        assert_eq!(g, Gesto::Ambiguo);
        // Mistura: duas notas juntas e depois uma separada (acorde quebrado): ambíguo.
        let g = classificar_gesto(
            &tocar(&[(60, 0), (64, 10), (67, 500)]),
            JanelasGesto::PADRAO,
        );
        assert_eq!(g, Gesto::Ambiguo);
    }

    #[test]
    fn notas_muito_espacadas_sao_notas_separadas_e_nao_arpejo() {
        // 2 s entre as notas: frases independentes, não um arpejo.
        let g = classificar_gesto(
            &tocar(&[(60, 0), (64, 2_000), (67, 4_000)]),
            JanelasGesto::PADRAO,
        );
        assert_eq!(
            g,
            Gesto::NotasSeparadas {
                notas: ns(&[60, 64, 67])
            }
        );
    }

    #[test]
    fn classificacao_ignora_a_ordem_de_chegada_dos_eventos() {
        let embaralhado = tocar(&[(67, 600), (60, 0), (64, 300)]);
        let g = classificar_gesto(&embaralhado, JanelasGesto::PADRAO);
        match g {
            Gesto::Arpejo { notas, direcao, .. } => {
                assert_eq!(notas, ns(&[60, 64, 67]));
                assert_eq!(direcao, DirecaoArpejo::Ascendente);
            }
            outro => panic!("{outro:?}"),
        }
    }

    #[test]
    fn mesma_nota_repetida_em_sequencia_nao_e_arpejo() {
        let g = classificar_gesto(
            &tocar(&[(60, 0), (60, 300), (60, 600)]),
            JanelasGesto::PADRAO,
        );
        assert_eq!(
            g,
            Gesto::Sequencia {
                notas: ns(&[60, 60, 60])
            }
        );
    }

    #[test]
    fn janelas_personalizadas_mudam_o_limite() {
        let eventos = tocar(&[(60, 0), (64, 80), (67, 160)]);
        let larga = JanelasGesto {
            simultanea_ns: 100_000_000,
            sequencia_min_ns: 150_000_000,
            sequencia_max_ns: 1_500_000_000,
        };
        assert!(matches!(
            classificar_gesto(&eventos, larga),
            Gesto::Simultaneo { .. }
        ));
    }
}
