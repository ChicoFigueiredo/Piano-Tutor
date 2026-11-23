//! Acordes e escalas (F04 aplicado; base do exercício de F05): dicionário com teclas-guia,
//! escalas, campo harmônico e o desafio "toque o acorde" (também de ouvido).

use crate::treino_andamento::{ItemTreino, TreinoAndamento};
use piano_tutor_core::NotaMidi;
use piano_tutor_core::teoria::{
    Acorde, ClasseAltura, DirecaoArpejo, GRAUS_ROMANOS, Gesto, JanelasGesto, Modo as ModoTonal,
    NotaTocada, Qualidade, TipoEscala, Tonalidade, campo_harmonico_maior, classificar_gesto,
    descrever_intervalo,
};

/// Ataque lembrado e o último instante em que a tecla foi vista ligada.
#[derive(Clone, Copy, Debug)]
struct Recente {
    ataque: NotaTocada,
    visto_ns: u64,
}

/// Quanto tempo um ataque continua contando para "o que estou tocando" depois de a tecla ser
/// solta (enquanto segurada, conta sempre).
pub const RETENCAO_NS: u64 = 4_000_000_000;
/// Máximo de ataques lembrados.
const MAX_RECENTES: usize = 16;
/// Máximo de notas citadas no texto de uma sequência.
const MAX_NOTAS_NO_TEXTO: usize = 8;

/// Aba da tela.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Modo {
    Dicionario,
    Escalas,
    Desafio,
    /// Sequência de acordes no tempo do metrônomo (Play).
    Treino,
}

/// Progressões do treino com andamento, em graus do campo harmônico maior (0 = I).
pub const PROGRESSOES: [(&str, &[usize]); 6] = [
    ("I-IV-V-I", &[0, 3, 4, 0]),
    ("I-V-vi-IV", &[0, 4, 5, 3]),
    ("ii-V-I", &[1, 4, 0]),
    ("I-vi-IV-V", &[0, 5, 3, 4]),
    ("I-vi-ii-V", &[0, 5, 1, 4]),
    ("Campo harmônico (I a vii°)", &[0, 1, 2, 3, 4, 5, 6]),
];

/// De onde vêm os acordes do treino com andamento.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FonteTreino {
    /// Índice em [`PROGRESSOES`], na tônica escolhida.
    Progressao(usize),
    /// Um conjunto do desafio (maiores e menores, sétimas...).
    Conjunto(Conjunto),
}

impl FonteTreino {
    pub fn todas() -> Vec<FonteTreino> {
        (0..PROGRESSOES.len())
            .map(FonteTreino::Progressao)
            .chain(Conjunto::TODOS.into_iter().map(FonteTreino::Conjunto))
            .collect()
    }

    pub fn texto(self) -> String {
        match self {
            FonteTreino::Progressao(i) => {
                format!("Progressão {}", PROGRESSOES.get(i).map_or("?", |p| p.0))
            }
            FonteTreino::Conjunto(c) => format!("Acordes: {}", c.texto()),
        }
    }

    /// Usa a tônica escolhida?
    pub fn usa_tonica(self) -> bool {
        matches!(
            self,
            FonteTreino::Progressao(_) | FonteTreino::Conjunto(Conjunto::CampoHarmonico)
        )
    }
}

/// Que acordes o desafio sorteia.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Conjunto {
    /// Tríades maiores e menores nas 12 tonalidades.
    MaioresEMenores,
    /// Campo harmônico maior da tonalidade escolhida.
    CampoHarmonico,
    /// Acordes com sétima (7, 7M, m7).
    ComSetima,
    /// Todas as qualidades.
    Todos,
}

impl Conjunto {
    pub const TODOS: [Conjunto; 4] = [
        Conjunto::MaioresEMenores,
        Conjunto::CampoHarmonico,
        Conjunto::ComSetima,
        Conjunto::Todos,
    ];

    pub const fn texto(self) -> &'static str {
        match self {
            Conjunto::MaioresEMenores => "maiores e menores",
            Conjunto::CampoHarmonico => "campo harmônico da tonalidade",
            Conjunto::ComSetima => "acordes com sétima",
            Conjunto::Todos => "todos os tipos",
        }
    }
}

/// Resultado do último acorde do desafio.
#[derive(Clone, Debug, PartialEq)]
pub struct Resultado {
    pub alvo: Acorde,
    pub segundos: f64,
}

/// Estado da tela de acordes.
#[derive(Clone, Debug)]
pub struct EstadoAcordes {
    pub modo: Modo,
    pub fundamental: ClasseAltura,
    pub qualidade: Qualidade,
    pub inversao: usize,
    /// Nota-base da oitava onde o acorde/escala é mostrado.
    pub base: u8,
    pub escala: TipoEscala,
    pub conjunto: Conjunto,
    pub de_ouvido: bool,
    pub mostrar_dica: bool,
    pub alvo: Option<Acorde>,
    pub alvo_desde_ns: u64,
    pub acertos: u32,
    pub pulados: u32,
    pub ultimo: Option<Resultado>,
    /// Depois de um acerto, espera soltar todas as teclas para sortear o próximo.
    pub aguardando_soltar: bool,
    /// Tonalidade declarada (grafia dos nomes e armadura). `None`: grafia padrão, sem armadura.
    pub tonalidade: Option<Tonalidade>,
    /// Treino com andamento (Play).
    pub treino: TreinoAndamento,
    pub fonte_treino: FonteTreino,
    /// Ataques recentes, vistos quadro a quadro (ver [`EstadoAcordes::observar`]).
    recentes: Vec<Recente>,
    /// Teclas ligadas no quadro anterior.
    ligadas_antes: Vec<NotaMidi>,
    semente: u64,
}

impl Default for EstadoAcordes {
    fn default() -> Self {
        Self {
            modo: Modo::Dicionario,
            fundamental: ClasseAltura::nova(0),
            qualidade: Qualidade::Maior,
            inversao: 0,
            base: 60,
            escala: TipoEscala::Maior,
            conjunto: Conjunto::MaioresEMenores,
            de_ouvido: false,
            mostrar_dica: false,
            alvo: None,
            alvo_desde_ns: 0,
            acertos: 0,
            pulados: 0,
            ultimo: None,
            aguardando_soltar: false,
            tonalidade: None,
            treino: TreinoAndamento::default(),
            fonte_treino: FonteTreino::Progressao(0),
            recentes: Vec::new(),
            ligadas_antes: Vec::new(),
            semente: 0x9E37_79B9_7F4A_7C15,
        }
    }
}

/// As 30 tonalidades (15 maiores, de 7 bemóis a 7 sustenidos; depois as 15 menores).
pub fn tonalidades_disponiveis() -> Vec<Tonalidade> {
    [ModoTonal::Maior, ModoTonal::Menor]
        .into_iter()
        .flat_map(|m| (-7..=7i8).filter_map(move |q| Tonalidade::da_armadura(q, m)))
        .collect()
}

/// Rótulo de uma tonalidade para o seletor: "Sol maior (1 sustenido)".
pub fn rotulo_da_tonalidade(t: Tonalidade) -> String {
    let modo = match t.modo() {
        ModoTonal::Maior => "maior",
        ModoTonal::Menor => "menor",
    };
    format!(
        "{} {modo} ({})",
        nome_pt_da_grafia(&t.nome_tonica()),
        quantidade_de_acidentes(t)
    )
}

fn quantidade_de_acidentes(t: Tonalidade) -> String {
    let n = t.numero_de_acidentes();
    match (n, t.armadura_quintas() > 0) {
        (0, _) => "sem acidentes".to_owned(),
        (1, true) => "1 sustenido".to_owned(),
        (_, true) => format!("{n} sustenidos"),
        (1, false) => "1 bemol".to_owned(),
        (_, false) => format!("{n} bemóis"),
    }
}

/// Texto da armadura: "Armadura de Ré maior: 2 sustenidos (F# C#)".
pub fn texto_da_armadura(t: Tonalidade) -> String {
    let modo = match t.modo() {
        ModoTonal::Maior => "maior",
        ModoTonal::Menor => "menor",
    };
    let base = format!(
        "Armadura de {} {modo}: {}",
        nome_pt_da_grafia(&t.nome_tonica()),
        quantidade_de_acidentes(t)
    );
    let acidentes = t.acidentes_da_armadura();
    if acidentes.is_empty() {
        base
    } else {
        format!("{base} ({})", acidentes.join(" "))
    }
}

/// "F#" -> "Fá#", "Bb" -> "Sib", "Cb" -> "Dób" (mesma grafia de [`ClasseAltura::nome_pt`]).
pub fn nome_pt_da_grafia(nome: &str) -> String {
    let mut cs = nome.chars();
    let letra = match cs.next() {
        Some('C') => "Dó",
        Some('D') => "Ré",
        Some('E') => "Mi",
        Some('F') => "Fá",
        Some('G') => "Sol",
        Some('A') => "Lá",
        Some('B') => "Si",
        _ => return nome.to_owned(),
    };
    format!("{letra}{}", cs.as_str())
}

fn mascara(notas: impl IntoIterator<Item = NotaMidi>) -> u16 {
    notas
        .into_iter()
        .fold(0u16, |m, n| m | 1 << (n.numero() % 12))
}

impl EstadoAcordes {
    /// Acorde selecionado no dicionário.
    pub fn acorde(&self) -> Acorde {
        Acorde::novo(self.fundamental, self.qualidade)
    }

    fn nota_base(&self) -> NotaMidi {
        NotaMidi::nova(self.base.min(115)).unwrap_or(NotaMidi::nova(60).expect("60"))
    }

    /// Notas do acorde selecionado (com a inversão).
    pub fn notas_do_acorde(&self) -> Vec<NotaMidi> {
        self.acorde().notas(self.nota_base(), self.inversao)
    }

    /// Notas da escala selecionada (uma oitava a partir da tônica).
    pub fn notas_da_escala(&self) -> Vec<NotaMidi> {
        let b = self.nota_base().numero() + ClasseAltura::nova(self.base).ate(self.fundamental);
        NotaMidi::nova(b)
            .map(|n| self.escala.notas(n))
            .unwrap_or_default()
    }

    /// Teclas-guia a mostrar no teclado agora.
    pub fn guias(&self) -> Vec<NotaMidi> {
        match self.modo {
            Modo::Dicionario => self.notas_do_acorde(),
            Modo::Escalas => self.notas_da_escala(),
            Modo::Desafio => match self.alvo {
                Some(a) if self.mostrar_dica => a.notas(self.nota_base(), 0),
                _ => Vec::new(),
            },
            Modo::Treino => self.treino.guias(),
        }
    }

    /// Acordes do treino com andamento, na ordem, conforme a fonte escolhida.
    pub fn acordes_do_treino(&self) -> Vec<Acorde> {
        match self.fonte_treino {
            FonteTreino::Progressao(i) => {
                let campo = campo_harmonico_maior(self.fundamental);
                PROGRESSOES
                    .get(i)
                    .map(|p| p.1.iter().map(|g| campo[*g]).collect())
                    .unwrap_or_default()
            }
            FonteTreino::Conjunto(c) => self.candidatos_de(c),
        }
    }

    /// Itens do treino: cada acorde na inversão mais próxima do anterior (mão quase parada).
    pub fn itens_do_treino(&self) -> Vec<ItemTreino> {
        let base = self.nota_base();
        let mut anterior: Option<Vec<NotaMidi>> = None;
        let campo = campo_harmonico_maior(self.fundamental);
        self.acordes_do_treino()
            .into_iter()
            .map(|ac| {
                let notas = voz_proxima(&ac, base, anterior.as_deref());
                anterior = Some(notas.clone());
                let grau = match self.fonte_treino {
                    FonteTreino::Progressao(_)
                    | FonteTreino::Conjunto(Conjunto::CampoHarmonico) => campo
                        .iter()
                        .position(|c| *c == ac)
                        .map(|g| format!("{} · ", GRAUS_ROMANOS[g]))
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                ItemTreino::novo(
                    notas,
                    self.cifra_do_acorde(&ac),
                    format!("{grau}{}", self.nome_do_acorde(&ac)),
                )
            })
            .collect()
    }

    fn aleatorio(&mut self) -> u64 {
        // xorshift64*: suficiente para sortear exercícios.
        let mut x = self.semente;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.semente = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Mistura a semente (chamar com o relógio ao abrir a tela).
    pub fn semear(&mut self, s: u64) {
        self.semente ^= s | 1;
    }

    fn candidatos(&self) -> Vec<Acorde> {
        self.candidatos_de(self.conjunto)
    }

    fn candidatos_de(&self, conjunto: Conjunto) -> Vec<Acorde> {
        let todas = |qs: &[Qualidade]| -> Vec<Acorde> {
            (0..12u8)
                .flat_map(|r| {
                    qs.iter()
                        .map(move |q| Acorde::novo(ClasseAltura::nova(r), *q))
                })
                .collect()
        };
        match conjunto {
            Conjunto::MaioresEMenores => todas(&[Qualidade::Maior, Qualidade::Menor]),
            Conjunto::CampoHarmonico => campo_harmonico_maior(self.fundamental).to_vec(),
            Conjunto::ComSetima => todas(&[
                Qualidade::SetimaDominante,
                Qualidade::SetimaMaior,
                Qualidade::MenorSetima,
            ]),
            Conjunto::Todos => todas(&Qualidade::TODAS),
        }
    }

    /// Sorteia o próximo acorde (diferente do atual quando possível).
    pub fn sortear(&mut self, agora_ns: u64) -> Acorde {
        let c = self.candidatos();
        let mut a = c[(self.aleatorio() % c.len() as u64) as usize];
        if c.len() > 1 && Some(a) == self.alvo {
            a = c[(self.aleatorio() % c.len() as u64) as usize];
        }
        self.alvo = Some(a);
        self.alvo_desde_ns = agora_ns;
        self.aguardando_soltar = false;
        a
    }

    /// Notas para tocar o alvo de exemplo (ouvir).
    pub fn notas_do_alvo(&self) -> Vec<NotaMidi> {
        self.alvo
            .map(|a| a.notas(self.nota_base(), 0))
            .unwrap_or_default()
    }

    /// Confere as teclas seguradas contra o alvo. Aceita qualquer inversão e dobras de
    /// oitava; exige todas as classes do acorde e nenhuma a mais. Devolve `true` no acerto.
    pub fn conferir(&mut self, seguradas: &[NotaMidi], agora_ns: u64) -> bool {
        if self.modo != Modo::Desafio {
            return false;
        }
        if self.aguardando_soltar {
            if seguradas.is_empty() {
                self.sortear(agora_ns);
            }
            return false;
        }
        let Some(alvo) = self.alvo else {
            return false;
        };
        let m_alvo = mascara(alvo.notas(self.nota_base(), 0));
        if seguradas.len() >= 2 && mascara(seguradas.iter().copied()) == m_alvo {
            self.acertos += 1;
            self.ultimo = Some(Resultado {
                alvo,
                segundos: agora_ns.saturating_sub(self.alvo_desde_ns) as f64 / 1e9,
            });
            self.aguardando_soltar = true;
            return true;
        }
        false
    }

    /// Pula o alvo atual.
    pub fn pular(&mut self, agora_ns: u64) -> Acorde {
        self.pulados += 1;
        self.sortear(agora_ns)
    }

    // --- Grafia e armadura (tonalidade declarada) ---------------------------------------

    /// Nome de uma classe de altura na grafia da tonalidade declarada ("F#" em Sol maior,
    /// "Gb" em Ré♭ maior); sem tonalidade, a grafia padrão de [`ClasseAltura::cifra`].
    pub fn nome_da_classe(&self, c: ClasseAltura) -> String {
        match self.tonalidade {
            Some(t) => t.grafia(c),
            None => c.cifra().to_owned(),
        }
    }

    /// Nome em português de uma classe de altura na mesma grafia ("Fá#", "Solb").
    pub fn nome_pt_da_classe(&self, c: ClasseAltura) -> String {
        match self.tonalidade {
            Some(t) => nome_pt_da_grafia(&t.grafia(c)),
            None => c.nome_pt().to_owned(),
        }
    }

    /// Cifra do acorde na grafia da tonalidade ("Gb", "C/E", "Bb/D").
    pub fn cifra_do_acorde(&self, a: &Acorde) -> String {
        let base = format!(
            "{}{}",
            self.nome_da_classe(a.fundamental),
            a.qualidade.sufixo()
        );
        if a.baixo == a.fundamental {
            base
        } else {
            format!("{base}/{}", self.nome_da_classe(a.baixo))
        }
    }

    /// Nome por extenso do acorde na grafia da tonalidade ("Solb maior").
    pub fn nome_do_acorde(&self, a: &Acorde) -> String {
        format!(
            "{} {}",
            self.nome_pt_da_classe(a.fundamental),
            a.qualidade.nome()
        )
    }

    /// Nome de uma nota com oitava na grafia da tonalidade ("Fá#4"). Dó♭ e Si♯ trocam de
    /// oitava em relação ao número MIDI (Dó♭4 = MIDI 59, Si♯3 = MIDI 60).
    pub fn nome_da_nota(&self, nota: NotaMidi) -> String {
        let nome = self.nome_da_classe(ClasseAltura::de_nota(nota));
        let ajuste = if nome.starts_with("Cb") {
            1
        } else if nome.starts_with("B#") {
            -1
        } else {
            0
        };
        format!(
            "{}{}",
            nome_pt_da_grafia(&nome),
            i32::from(nota.oitava()) + ajuste
        )
    }

    /// Notas separadas por espaço, na grafia da tonalidade.
    pub fn nomes_das_notas(&self, notas: &[NotaMidi]) -> String {
        notas
            .iter()
            .map(|n| self.nome_da_nota(*n))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Armadura da tonalidade declarada, em texto.
    pub fn armadura(&self) -> Option<String> {
        self.tonalidade.map(texto_da_armadura)
    }

    // --- "O que estou tocando": gesto sobre os ataques recentes -----------------------------

    /// Atualiza os ataques recentes com as teclas ligadas **neste quadro**: cada tecla que não
    /// estava ligada no quadro anterior vira um ataque em `agora_ns`. A resolução do tempo é a
    /// do quadro da interface (não há carimbo do teclado aqui), então ataques no mesmo quadro
    /// contam como simultâneos.
    pub fn observar(&mut self, ligadas: &[NotaMidi], agora_ns: u64) {
        for r in &mut self.recentes {
            if ligadas.contains(&r.ataque.nota) {
                r.visto_ns = agora_ns;
            }
        }
        for n in ligadas {
            if !self.ligadas_antes.contains(n) {
                self.recentes.push(Recente {
                    ataque: NotaTocada {
                        nota: *n,
                        inicio_ns: agora_ns,
                    },
                    visto_ns: agora_ns,
                });
            }
        }
        self.recentes
            .retain(|r| agora_ns.saturating_sub(r.visto_ns) <= RETENCAO_NS);
        if self.recentes.len() > MAX_RECENTES {
            let sobra = self.recentes.len() - MAX_RECENTES;
            self.recentes.drain(..sobra);
        }
        self.ligadas_antes = ligadas.to_vec();
    }

    /// Ataques do gesto mais recente: do último ataque para trás, até uma pausa maior que a
    /// janela de sequência (depois disso são notas de outro gesto).
    fn ultimo_gesto(&self) -> Vec<NotaTocada> {
        let mut v: Vec<NotaTocada> = self.recentes.iter().map(|r| r.ataque).collect();
        v.sort_by_key(|e| e.inicio_ns);
        let max = JanelasGesto::PADRAO.sequencia_max_ns;
        let mut inicio = v.len();
        while inicio > 0 {
            if inicio < v.len() && v[inicio].inicio_ns - v[inicio - 1].inicio_ns > max {
                break;
            }
            inicio -= 1;
        }
        v.split_off(inicio)
    }

    /// Classificação do gesto mais recente (ver [`classificar_gesto`]).
    pub fn gesto(&self) -> Gesto {
        classificar_gesto(&self.ultimo_gesto(), JanelasGesto::PADRAO)
    }

    /// Texto de "o que estou tocando". `None` quando não há nada tocado ou quando o gesto é
    /// ambíguo (nenhum rótulo é imposto). Um par de notas nunca é chamado de acorde.
    pub fn texto_do_gesto(&self) -> Option<String> {
        match self.gesto() {
            Gesto::Vazio | Gesto::Ambiguo | Gesto::NotasSeparadas { .. } => None,
            Gesto::NotaIsolada(n) => Some(format!("Nota isolada: {}", self.nome_da_nota(n))),
            Gesto::Simultaneo { notas, acorde } => Some(match (acorde, notas.len()) {
                (Some(a), _) => format!(
                    "Acorde: {} ({}, {})",
                    self.cifra_do_acorde(&a),
                    self.nome_do_acorde(&a),
                    a.texto_inversao()
                ),
                (None, 2) => format!(
                    "Duas notas juntas: {}",
                    descrever_intervalo(notas[0], notas[1])
                ),
                (None, _) => "Notas juntas sem nome de acorde reconhecido".to_owned(),
            }),
            Gesto::Arpejo {
                direcao, acorde, ..
            } => {
                let sentido = match direcao {
                    DirecaoArpejo::Ascendente => " ascendente",
                    DirecaoArpejo::Descendente => " descendente",
                    DirecaoArpejo::Misto => " (ordem mista)",
                };
                Some(format!(
                    "Arpejo{sentido} de {} ({})",
                    self.cifra_do_acorde(&acorde),
                    self.nome_do_acorde(&acorde)
                ))
            }
            Gesto::Sequencia { notas } => {
                let mais = if notas.len() > MAX_NOTAS_NO_TEXTO {
                    " ..."
                } else {
                    ""
                };
                let ate = &notas[..notas.len().min(MAX_NOTAS_NO_TEXTO)];
                Some(format!(
                    "Notas em sequência: {}{mais}",
                    self.nomes_das_notas(ate)
                ))
            }
        }
    }
}

/// Inversão do acorde mais perto da voz anterior (menor soma de distâncias entre as notas,
/// em ordem); sem anterior, estado fundamental a partir de `base`. Fica entre C3 e C6.
pub fn voz_proxima(ac: &Acorde, base: NotaMidi, anterior: Option<&[NotaMidi]>) -> Vec<NotaMidi> {
    let Some(ant) = anterior.filter(|a| !a.is_empty()) else {
        return ac.notas(base, 0);
    };
    let n = ac.qualidade.intervalos().len();
    let mut melhor: Option<(u32, Vec<NotaMidi>)> = None;
    for desloc in [-12i16, 0, 12] {
        let Some(b) = u8::try_from(i16::from(base.numero()) + desloc)
            .ok()
            .and_then(NotaMidi::nova)
        else {
            continue;
        };
        for inv in 0..n {
            let v = ac.notas(b, inv);
            if v.len() != n
                || v.first().is_some_and(|x| x.numero() < 48)
                || v.last().is_some_and(|x| x.numero() > 84)
            {
                continue;
            }
            let custo: u32 = (0..n.max(ant.len()))
                .map(|i| {
                    let a = ant[i.min(ant.len() - 1)].numero();
                    let x = v[i.min(n - 1)].numero();
                    u32::from(a.abs_diff(x))
                })
                .sum();
            if melhor.as_ref().is_none_or(|(c, _)| custo < *c) {
                melhor = Some((custo, v));
            }
        }
    }
    melhor.map_or_else(|| ac.notas(base, 0), |(_, v)| v)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn n(x: u8) -> NotaMidi {
        NotaMidi::nova(x).expect("nota")
    }

    #[test]
    fn dicionario_da_as_guias_do_acorde_com_inversao() {
        let mut e = EstadoAcordes {
            fundamental: ClasseAltura::nova(9),
            qualidade: Qualidade::Menor,
            base: 60,
            ..EstadoAcordes::default()
        };
        assert_eq!(
            e.guias().iter().map(|x| x.numero()).collect::<Vec<_>>(),
            vec![69, 72, 76]
        );
        e.inversao = 1;
        assert_eq!(
            e.guias().iter().map(|x| x.numero()).collect::<Vec<_>>(),
            vec![72, 76, 81]
        );
    }

    #[test]
    fn escala_comeca_na_tonica_escolhida() {
        let e = EstadoAcordes {
            modo: Modo::Escalas,
            fundamental: ClasseAltura::nova(7),
            base: 60,
            ..EstadoAcordes::default()
        };
        assert_eq!(
            e.guias().iter().map(|x| x.numero()).collect::<Vec<_>>(),
            vec![67, 69, 71, 72, 74, 76, 78, 79]
        );
    }

    #[test]
    fn desafio_aceita_qualquer_inversao_e_espera_soltar() {
        let mut e = EstadoAcordes {
            modo: Modo::Desafio,
            ..EstadoAcordes::default()
        };
        e.alvo = Some(Acorde::novo(ClasseAltura::nova(0), Qualidade::Maior));
        assert!(!e.conferir(&[n(60), n(64)], 0), "incompleto");
        assert!(!e.conferir(&[n(60), n(63), n(67)], 0), "menor não vale");
        assert!(!e.conferir(&[n(60), n(64), n(67), n(70)], 0), "nota a mais");
        assert!(
            e.conferir(&[n(52), n(55), n(60)], 2_000_000_000),
            "C/E vale"
        );
        assert_eq!(e.acertos, 1);
        assert_eq!(e.ultimo.as_ref().map(|r| r.segundos), Some(2.0));
        assert!(!e.conferir(&[n(52), n(55), n(60)], 0), "esperando soltar");
        assert!(!e.conferir(&[], 0));
        assert!(!e.aguardando_soltar);
        assert!(e.alvo.is_some());
    }

    #[test]
    fn sorteio_respeita_o_conjunto() {
        let mut e = EstadoAcordes {
            modo: Modo::Desafio,
            conjunto: Conjunto::CampoHarmonico,
            fundamental: ClasseAltura::nova(2),
            ..EstadoAcordes::default()
        };
        let campo = campo_harmonico_maior(ClasseAltura::nova(2));
        for i in 0..50 {
            let a = e.sortear(i);
            assert!(campo.contains(&a), "{}", a.cifra());
        }
        e.conjunto = Conjunto::ComSetima;
        for i in 0..50 {
            let a = e.sortear(i);
            assert!(a.qualidade.intervalos().len() == 4);
        }
    }

    // --- Gesto, tonalidade e grafia (F04 na interface) -------------------------------------

    const MS: u64 = 1_000_000;

    /// Toca `quadros`: lista de (instante em ms, teclas ligadas naquele quadro).
    fn tocar(e: &mut EstadoAcordes, quadros: &[(u64, &[u8])]) {
        for (ms, teclas) in quadros {
            let v: Vec<NotaMidi> = teclas.iter().map(|x| n(*x)).collect();
            e.observar(&v, ms * MS);
        }
    }

    fn em(t: Option<(i8, ModoTonal)>) -> EstadoAcordes {
        EstadoAcordes {
            tonalidade: t.and_then(|(q, m)| Tonalidade::da_armadura(q, m)),
            ..EstadoAcordes::default()
        }
    }

    #[test]
    fn observar_registra_so_ataques_novos() {
        let mut e = EstadoAcordes::default();
        tocar(
            &mut e,
            &[(0, &[60]), (16, &[60]), (100, &[60, 64]), (116, &[60, 64])],
        );
        assert_eq!(e.recentes.len(), 2);
        assert_eq!(e.recentes[0].ataque.inicio_ns, 0);
        assert_eq!(e.recentes[1].ataque.inicio_ns, 100 * MS);
        // Soltar e tocar de novo é outro ataque.
        tocar(&mut e, &[(200, &[]), (300, &[60])]);
        assert_eq!(e.recentes.len(), 3);
    }

    #[test]
    fn sem_teclas_nao_ha_texto() {
        let mut e = EstadoAcordes::default();
        assert_eq!(e.texto_do_gesto(), None);
        tocar(&mut e, &[(0, &[])]);
        assert_eq!(e.texto_do_gesto(), None);
    }

    #[test]
    fn nota_isolada() {
        let mut e = EstadoAcordes::default();
        tocar(&mut e, &[(0, &[60])]);
        assert_eq!(e.texto_do_gesto().as_deref(), Some("Nota isolada: Dó4"));
    }

    #[test]
    fn acorde_simultaneo_com_inversao() {
        let mut e = EstadoAcordes::default();
        tocar(&mut e, &[(0, &[60, 64, 67])]);
        assert_eq!(
            e.texto_do_gesto().as_deref(),
            Some("Acorde: C (Dó maior, estado fundamental)")
        );
        let mut e = EstadoAcordes::default();
        tocar(&mut e, &[(0, &[64]), (16, &[64, 67, 72])]);
        assert_eq!(
            e.texto_do_gesto().as_deref(),
            Some("Acorde: C/E (Dó maior, 1ª inversão)")
        );
    }

    #[test]
    fn par_de_notas_nunca_e_chamado_de_acorde() {
        for par in [[60u8, 64], [48, 55], [60, 67], [60, 72]] {
            let mut e = EstadoAcordes::default();
            tocar(&mut e, &[(0, &par)]);
            let t = e.texto_do_gesto().expect("par descrito");
            assert!(t.starts_with("Duas notas juntas: "), "{t}");
            assert!(!t.contains("Acorde"), "{t}");
            assert!(!t.contains("C5"), "{t}");
        }
    }

    #[test]
    fn arpejo_ainda_com_as_teclas_soltas() {
        let mut e = EstadoAcordes::default();
        // Cada tecla é solta antes da próxima, como num arpejo sem pedal.
        tocar(
            &mut e,
            &[
                (0, &[60]),
                (150, &[]),
                (300, &[64]),
                (450, &[]),
                (600, &[67]),
                (750, &[]),
            ],
        );
        assert_eq!(
            e.texto_do_gesto().as_deref(),
            Some("Arpejo ascendente de C (Dó maior)")
        );
        let mut e = EstadoAcordes::default();
        tocar(
            &mut e,
            &[(0, &[72]), (300, &[67]), (600, &[64]), (900, &[60])],
        );
        assert_eq!(
            e.texto_do_gesto().as_deref(),
            Some("Arpejo descendente de C (Dó maior)")
        );
    }

    #[test]
    fn sequencia_sem_acorde_e_so_sequencia() {
        let mut e = EstadoAcordes::default();
        tocar(&mut e, &[(0, &[60]), (300, &[62]), (600, &[64])]);
        assert_eq!(
            e.texto_do_gesto().as_deref(),
            Some("Notas em sequência: Dó4 Ré4 Mi4")
        );
        // Duas notas em sequência: sequência, nunca arpejo nem acorde.
        let mut e = EstadoAcordes::default();
        tocar(&mut e, &[(0, &[60]), (400, &[64])]);
        assert_eq!(
            e.texto_do_gesto().as_deref(),
            Some("Notas em sequência: Dó4 Mi4")
        );
    }

    #[test]
    fn gesto_ambiguo_nao_mostra_nada() {
        let mut e = EstadoAcordes::default();
        // 80 ms entre as notas: nem juntas nem em sequência.
        tocar(&mut e, &[(0, &[60]), (80, &[60, 64]), (160, &[60, 64, 67])]);
        assert_eq!(e.gesto(), Gesto::Ambiguo);
        assert_eq!(e.texto_do_gesto(), None);
    }

    #[test]
    fn notas_antigas_somem_mas_teclas_seguradas_ficam() {
        let mut e = EstadoAcordes::default();
        tocar(&mut e, &[(0, &[60, 64, 67])]);
        // Segurando por 10 s: continua mostrando o acorde.
        tocar(&mut e, &[(10_000, &[60, 64, 67])]);
        assert!(e.texto_do_gesto().is_some());
        // Soltou: some depois da retenção.
        tocar(&mut e, &[(10_100, &[])]);
        assert!(e.texto_do_gesto().is_some());
        tocar(&mut e, &[(14_000, &[])]);
        assert!(e.texto_do_gesto().is_some(), "ainda dentro da retenção");
        tocar(&mut e, &[(14_200, &[])]);
        assert_eq!(e.texto_do_gesto(), None);
    }

    #[test]
    fn pausa_longa_separa_os_gestos() {
        let mut e = EstadoAcordes::default();
        tocar(
            &mut e,
            &[(0, &[60, 64, 67]), (100, &[]), (3_000, &[62, 65, 69])],
        );
        assert_eq!(
            e.texto_do_gesto().as_deref(),
            Some("Acorde: Dm (Ré menor, estado fundamental)")
        );
    }

    #[test]
    fn historico_e_limitado() {
        let mut e = EstadoAcordes::default();
        for i in 0..100u64 {
            tocar(&mut e, &[(i * 10, &[60]), (i * 10 + 5, &[])]);
        }
        assert!(e.recentes.len() <= MAX_RECENTES);
    }

    #[test]
    fn tonalidades_disponiveis_sao_as_15_maiores_e_as_15_menores() {
        let v = tonalidades_disponiveis();
        assert_eq!(v.len(), 30);
        assert!(v[..15].iter().all(|t| t.modo() == ModoTonal::Maior));
        assert!(v[15..].iter().all(|t| t.modo() == ModoTonal::Menor));
        let mut cifras: Vec<String> = v.iter().map(|t| t.cifra()).collect();
        cifras.sort();
        cifras.dedup();
        assert_eq!(cifras.len(), 30, "sem repetidas");
    }

    #[test]
    fn grafia_dos_acordes_muda_com_a_tonalidade() {
        let fa_sus = Acorde::novo(ClasseAltura::nova(6), Qualidade::Maior);
        assert_eq!(em(None).cifra_do_acorde(&fa_sus), "F#");
        assert_eq!(
            em(Some((1, ModoTonal::Maior))).cifra_do_acorde(&fa_sus),
            "F#"
        );
        assert_eq!(
            em(Some((-5, ModoTonal::Maior))).cifra_do_acorde(&fa_sus),
            "Gb"
        );
        assert_eq!(
            em(Some((-5, ModoTonal::Maior))).nome_do_acorde(&fa_sus),
            "Solb maior"
        );
        assert_eq!(
            em(Some((1, ModoTonal::Maior))).nome_do_acorde(&fa_sus),
            "Fá# maior"
        );
        // Baixo (inversão) segue a mesma grafia: Gb/Bb em Ré♭ maior, F#/A# em Si maior.
        let inv = Acorde {
            baixo: ClasseAltura::nova(10),
            ..fa_sus
        };
        assert_eq!(
            em(Some((-5, ModoTonal::Maior))).cifra_do_acorde(&inv),
            "Gb/Bb"
        );
        assert_eq!(
            em(Some((5, ModoTonal::Maior))).cifra_do_acorde(&inv),
            "F#/A#"
        );
    }

    #[test]
    fn gesto_usa_a_grafia_da_tonalidade() {
        let mut bemol = em(Some((-5, ModoTonal::Maior)));
        tocar(&mut bemol, &[(0, &[66])]);
        assert_eq!(
            bemol.texto_do_gesto().as_deref(),
            Some("Nota isolada: Solb4")
        );
        let mut sustenido = em(Some((2, ModoTonal::Maior)));
        tocar(&mut sustenido, &[(0, &[66])]);
        assert_eq!(
            sustenido.texto_do_gesto().as_deref(),
            Some("Nota isolada: Fá#4")
        );
        // Acorde de Sol♭ maior (Solb-Sib-Reb) em Ré♭ maior.
        let mut e = em(Some((-5, ModoTonal::Maior)));
        tocar(&mut e, &[(0, &[66, 70, 73])]);
        assert_eq!(
            e.texto_do_gesto().as_deref(),
            Some("Acorde: Gb (Solb maior, estado fundamental)")
        );
    }

    #[test]
    fn oitava_de_do_bemol_e_si_sustenido_segue_a_grafia() {
        // Dó♭4 = MIDI 59 (soa como Si3); Si♯3 = MIDI 60 (soa como Dó4).
        assert_eq!(em(Some((-6, ModoTonal::Maior))).nome_da_nota(n(59)), "Dób4");
        assert_eq!(em(Some((7, ModoTonal::Maior))).nome_da_nota(n(60)), "Si#3");
        assert_eq!(em(None).nome_da_nota(n(59)), "Si3");
        assert_eq!(em(None).nome_da_nota(n(60)), "Dó4");
    }

    #[test]
    fn armadura_em_texto() {
        let t = |q, m| Tonalidade::da_armadura(q, m).expect("válida");
        assert_eq!(
            texto_da_armadura(t(0, ModoTonal::Maior)),
            "Armadura de Dó maior: sem acidentes"
        );
        assert_eq!(
            texto_da_armadura(t(0, ModoTonal::Menor)),
            "Armadura de Lá menor: sem acidentes"
        );
        assert_eq!(
            texto_da_armadura(t(1, ModoTonal::Maior)),
            "Armadura de Sol maior: 1 sustenido (F#)"
        );
        assert_eq!(
            texto_da_armadura(t(-1, ModoTonal::Maior)),
            "Armadura de Fá maior: 1 bemol (Bb)"
        );
        assert_eq!(
            texto_da_armadura(t(-5, ModoTonal::Maior)),
            "Armadura de Réb maior: 5 bemóis (Bb Eb Ab Db Gb)"
        );
        assert_eq!(
            texto_da_armadura(t(3, ModoTonal::Menor)),
            "Armadura de Fá# menor: 3 sustenidos (F# C# G#)"
        );
        assert_eq!(em(None).armadura(), None);
        assert_eq!(
            em(Some((2, ModoTonal::Maior))).armadura().as_deref(),
            Some("Armadura de Ré maior: 2 sustenidos (F# C#)")
        );
    }

    #[test]
    fn rotulos_do_seletor_cobrem_as_30_tonalidades() {
        let rotulos: Vec<String> = tonalidades_disponiveis()
            .into_iter()
            .map(rotulo_da_tonalidade)
            .collect();
        assert_eq!(rotulos[0], "Dób maior (7 bemóis)");
        assert_eq!(rotulos[7], "Dó maior (sem acidentes)");
        assert_eq!(rotulos[14], "Dó# maior (7 sustenidos)");
        assert_eq!(rotulos[22], "Lá menor (sem acidentes)");
        assert_eq!(rotulos[16], "Mib menor (6 bemóis)");
    }

    #[test]
    fn treino_i_iv_v_i_em_do_com_vozes_proximas() {
        let a = EstadoAcordes::default();
        let itens = a.itens_do_treino();
        let rotulos: Vec<&str> = itens.iter().map(|i| i.rotulo.as_str()).collect();
        assert_eq!(rotulos, ["C", "F", "G", "C"]);
        assert!(itens[1].detalhe.starts_with("IV · "));
        // Fá e Sol em inversão perto do Dó: a mão anda no máximo uma terça por nota.
        for par in itens.windows(2) {
            for (x, y) in par[0].notas.iter().zip(&par[1].notas) {
                assert!(x.numero().abs_diff(y.numero()) <= 4, "{par:?}");
            }
        }
    }

    #[test]
    fn treino_de_conjunto_lista_todos_os_candidatos() {
        let a = EstadoAcordes {
            fonte_treino: FonteTreino::Conjunto(Conjunto::MaioresEMenores),
            ..Default::default()
        };
        assert_eq!(a.itens_do_treino().len(), 24);
        for f in FonteTreino::todas() {
            let a = EstadoAcordes {
                fonte_treino: f,
                ..Default::default()
            };
            assert!(!a.itens_do_treino().is_empty(), "{f:?}");
        }
    }
}
