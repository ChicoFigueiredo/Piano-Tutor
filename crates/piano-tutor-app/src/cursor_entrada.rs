//! Cursor de entrada de notas da Oficina (ticket 17b, E01, seam S1): estado e regras **sem
//! janela e sem egui**. A tela só traduz clique e teclas em chamadas deste módulo e do serviço de
//! documento (`InserirNota`, `InserirPausa`).
//!
//! # Contrato em linguagem de domínio
//!
//! - **A posição é a coluna do layout, a voz e a pauta, nunca um pixel.** O cursor guarda
//!   ([`PosicaoDeEntrada`]) a parte, a pauta e a voz (a [`Faixa`]), o compasso e a **coluna** do
//!   compasso: o índice na lista [`colunas_do_compasso`], que é exatamente a lista de colunas do
//!   layout (`CompassoLay::colunas`, uma por início de evento do compasso, em todas as pautas).
//!   O tempo da coluna é a `Fracao` exata em semínimas que o comando de inserir recebe.
//! - **Mover.** [`CursorDeEntrada::seguinte`] e [`CursorDeEntrada::anterior`] andam pelos inícios
//!   de evento da própria voz (notas **e pausas**), atravessando compassos; nos limites o cursor
//!   fica e há um [`AvisoDeEntrada`]. [`CursorDeEntrada::faixa_abaixo`] e
//!   [`CursorDeEntrada::faixa_acima`] trocam de voz ou pauta, inclusive voz só de pausas (que a
//!   navegação por notas nunca alcança), mantendo compasso e coluna. [`CursorDeEntrada::posicionar`]
//!   vai direto a uma posição e [`CursorDeEntrada::selecionar_nota`] segue a nota selecionada.
//! - **Selecionar pausa.** O que está sob o cursor é um [`Conteudo`]: nota, pausa (inclusive o
//!   meio de uma pausa longa), meio de uma nota que dura, ou nada. A pausa sob o cursor é a
//!   "pausa selecionada": a descrição em texto a nomeia e é nela que a nota ou a pausa entram.
//! - **Altura pela letra.** [`altura_da_letra`] dá a altura da letra `A` a `G` como o MuseScore:
//!   a oitava mais próxima da última nota da voz antes do cursor (sem nota antes, da linha do
//!   meio da clave) e a alteração que a armadura do compasso pede.
//! - **Duração de entrada.** As teclas `1` a `8` guardam a duração das próximas inserções
//!   ([`DuracaoDeEntrada`], padrão semínima) e o ponto a alterna.
//! - Nada aqui edita o documento nem o marca como sujo.

use crate::geometria::IdNota;
use piano_tutor_core::documento::{Altura as AlturaEscrita, IdsPorNota};
use piano_tutor_core::partitura::{Armadura, Fracao, Nota, Partitura, SinalClave};

/// Parte, pauta e voz: o fio do compasso em que se escreve.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Faixa {
    pub parte: usize,
    pub pauta: u8,
    pub voz: u8,
}

/// Onde o cursor está: a faixa, o compasso (índice a partir de 0) e a coluna do layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PosicaoDeEntrada {
    pub faixa: Faixa,
    pub compasso: usize,
    pub coluna: usize,
}

/// O que há sob o cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conteudo {
    /// Uma nota começa aqui (a de baixo, se for acorde).
    Nota(IdNota),
    /// Uma pausa ocupa o lugar (começando aqui ou já em andamento): é a pausa selecionada.
    Pausa(IdNota),
    /// O meio de uma nota que dura: nada se insere aqui sem apagá-la antes.
    MeioDeNota(IdNota),
    /// A voz não tem nada neste ponto.
    Nada,
}

/// Aviso discreto de limite ou de situação do cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AvisoDeEntrada {
    SemPosicao,
    FimDaVoz,
    InicioDaVoz,
    PrimeiraFaixa,
    UltimaFaixa,
    PosicaoInvalida,
    SemNotas,
}

impl AvisoDeEntrada {
    pub fn texto(self) -> &'static str {
        match self {
            Self::SemPosicao => {
                "Escolha a posição de entrada: clique num compasso, use Ctrl+setas ou selecione uma nota."
            }
            Self::FimDaVoz => "Esta é a última posição desta voz.",
            Self::InicioDaVoz => "Esta é a primeira posição desta voz.",
            Self::PrimeiraFaixa => "Não há voz ou pauta acima desta.",
            Self::UltimaFaixa => "Não há voz ou pauta abaixo desta.",
            Self::PosicaoInvalida => "Essa posição não existe na partitura.",
            Self::SemNotas => "A partitura não tem compassos para escrever.",
        }
    }
}

/// Duração das próximas inserções: a tecla `1` a `8` (como no MuseScore) e o ponto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DuracaoDeEntrada {
    pub tecla: u8,
    pub pontos: u8,
}

impl Default for DuracaoDeEntrada {
    /// Semínima, sem ponto.
    fn default() -> Self {
        Self {
            tecla: 5,
            pontos: 0,
        }
    }
}

/// O cursor de entrada e a duração atual de entrada.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CursorDeEntrada {
    posicao: Option<PosicaoDeEntrada>,
    duracao: DuracaoDeEntrada,
}

/// As colunas do compasso `compasso`: os inícios de evento de todas as pautas, em ordem, sem
/// repetição. É a mesma lista que o layout guarda em `CompassoLay::colunas`.
pub fn colunas_do_compasso(p: &Partitura, compasso: usize) -> Vec<Fracao> {
    let mut v: Vec<Fracao> = p
        .partes
        .iter()
        .filter_map(|parte| parte.compassos.get(compasso))
        .flat_map(|c| c.notas.iter().map(|n| n.posicao))
        .collect();
    v.sort();
    v.dedup();
    if v.is_empty() {
        v.push(Fracao::ZERO);
    }
    v
}

/// Todas as faixas (parte, pauta, voz) que têm ao menos um evento, em ordem de leitura de
/// cima para baixo. Inclui a voz só de pausas.
pub fn faixas(p: &Partitura) -> Vec<Faixa> {
    let mut v: Vec<Faixa> = Vec::new();
    for (pi, parte) in p.partes.iter().enumerate() {
        for c in &parte.compassos {
            for n in &c.notas {
                v.push(Faixa {
                    parte: pi,
                    pauta: n.pauta,
                    voz: n.voz,
                });
            }
        }
    }
    v.sort();
    v.dedup();
    v
}

fn da_faixa(n: &Nota, f: Faixa) -> bool {
    n.pauta == f.pauta && n.voz == f.voz
}

/// Inícios de evento da faixa no compasso, em ordem.
fn inicios_da_faixa(p: &Partitura, f: Faixa, compasso: usize) -> Vec<Fracao> {
    let mut v: Vec<Fracao> = p
        .partes
        .get(f.parte)
        .and_then(|parte| parte.compassos.get(compasso))
        .map(|c| {
            c.notas
                .iter()
                .filter(|n| da_faixa(n, f))
                .map(|n| n.posicao)
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v.dedup();
    v
}

impl CursorDeEntrada {
    pub fn posicao(&self) -> Option<PosicaoDeEntrada> {
        self.posicao
    }

    pub fn duracao(&self) -> DuracaoDeEntrada {
        self.duracao
    }

    /// A tecla `1` a `8` passa a ser a duração das próximas inserções (zera o ponto, como no
    /// MuseScore). Fora de `1..=8` não muda nada e devolve `false`.
    pub fn definir_duracao(&mut self, tecla: u8) -> bool {
        if !(1..=8).contains(&tecla) {
            return false;
        }
        self.duracao = DuracaoDeEntrada { tecla, pontos: 0 };
        true
    }

    /// Liga ou desliga o ponto da duração de entrada.
    pub fn alternar_ponto(&mut self) {
        self.duracao.pontos = u8::from(self.duracao.pontos == 0);
    }

    pub fn limpar(&mut self) {
        self.posicao = None;
    }

    /// Vai direto à posição dada (faixa, compasso e coluna do layout). Recusa o que não existe.
    pub fn posicionar(
        &mut self,
        p: &Partitura,
        faixa: Faixa,
        compasso: usize,
        coluna: usize,
    ) -> Result<(), AvisoDeEntrada> {
        if p.partes.get(faixa.parte).is_none() || compasso >= p.compassos.len() {
            return Err(AvisoDeEntrada::PosicaoInvalida);
        }
        if !faixas(p).contains(&faixa) || coluna >= colunas_do_compasso(p, compasso).len() {
            return Err(AvisoDeEntrada::PosicaoInvalida);
        }
        self.posicao = Some(PosicaoDeEntrada {
            faixa,
            compasso,
            coluna,
        });
        Ok(())
    }

    /// Vai ao compasso e coluna clicados, na faixa em que o cursor já estava (ou na primeira, se
    /// ainda não havia posição).
    pub fn posicionar_na_coluna(
        &mut self,
        p: &Partitura,
        compasso: usize,
        coluna: usize,
    ) -> Result<(), AvisoDeEntrada> {
        let faixa = match self.posicao {
            Some(pos) => pos.faixa,
            None => *faixas(p).first().ok_or(AvisoDeEntrada::SemNotas)?,
        };
        self.posicionar(p, faixa, compasso, coluna)
    }

    /// Segue a nota (ou pausa) `id`: a posição passa a ser a dela. `false` se a identidade não
    /// existe no modelo.
    pub fn selecionar_nota(&mut self, p: &Partitura, ids: &IdsPorNota, id: IdNota) -> bool {
        for (pi, parte) in p.partes.iter().enumerate() {
            for (ci, c) in parte.compassos.iter().enumerate() {
                for (oi, n) in c.notas.iter().enumerate() {
                    if ids.id(pi, ci, oi) != Some(id) {
                        continue;
                    }
                    let colunas = colunas_do_compasso(p, ci);
                    let Some(coluna) = colunas.iter().position(|q| *q == n.posicao) else {
                        return false;
                    };
                    self.posicao = Some(PosicaoDeEntrada {
                        faixa: Faixa {
                            parte: pi,
                            pauta: n.pauta,
                            voz: n.voz,
                        },
                        compasso: ci,
                        coluna,
                    });
                    return true;
                }
            }
        }
        false
    }

    /// Tempo do cursor no compasso (semínimas, exato), se há posição.
    pub fn tempo(&self, p: &Partitura) -> Option<Fracao> {
        let pos = self.posicao?;
        colunas_do_compasso(p, pos.compasso)
            .get(pos.coluna)
            .copied()
    }

    /// Próximo início de evento da voz (nota ou pausa), atravessando compassos.
    pub fn seguinte(&mut self, p: &Partitura) -> Result<PosicaoDeEntrada, AvisoDeEntrada> {
        let pos = self.posicao.ok_or(AvisoDeEntrada::SemPosicao)?;
        let tempo = self.tempo(p).ok_or(AvisoDeEntrada::PosicaoInvalida)?;
        for c in pos.compasso..p.compassos.len() {
            let inicios = inicios_da_faixa(p, pos.faixa, c);
            let proximo = inicios.into_iter().find(|q| c > pos.compasso || *q > tempo);
            if let Some(q) = proximo
                && let Some(coluna) = colunas_do_compasso(p, c).iter().position(|x| *x == q)
            {
                let nova = PosicaoDeEntrada {
                    faixa: pos.faixa,
                    compasso: c,
                    coluna,
                };
                self.posicao = Some(nova);
                return Ok(nova);
            }
        }
        Err(AvisoDeEntrada::FimDaVoz)
    }

    /// Início de evento anterior da voz (nota ou pausa), atravessando compassos.
    pub fn anterior(&mut self, p: &Partitura) -> Result<PosicaoDeEntrada, AvisoDeEntrada> {
        let pos = self.posicao.ok_or(AvisoDeEntrada::SemPosicao)?;
        let tempo = self.tempo(p).ok_or(AvisoDeEntrada::PosicaoInvalida)?;
        for c in (0..=pos.compasso).rev() {
            let inicios = inicios_da_faixa(p, pos.faixa, c);
            let anterior = inicios
                .into_iter()
                .rev()
                .find(|q| c < pos.compasso || *q < tempo);
            if let Some(q) = anterior
                && let Some(coluna) = colunas_do_compasso(p, c).iter().position(|x| *x == q)
            {
                let nova = PosicaoDeEntrada {
                    faixa: pos.faixa,
                    compasso: c,
                    coluna,
                };
                self.posicao = Some(nova);
                return Ok(nova);
            }
        }
        Err(AvisoDeEntrada::InicioDaVoz)
    }

    /// Passa para a voz ou pauta abaixo (a seguinte na lista de faixas), no mesmo compasso e
    /// coluna.
    pub fn faixa_abaixo(&mut self, p: &Partitura) -> Result<PosicaoDeEntrada, AvisoDeEntrada> {
        self.trocar_de_faixa(p, 1)
    }

    /// Passa para a voz ou pauta acima, no mesmo compasso e coluna.
    pub fn faixa_acima(&mut self, p: &Partitura) -> Result<PosicaoDeEntrada, AvisoDeEntrada> {
        self.trocar_de_faixa(p, -1)
    }

    fn trocar_de_faixa(
        &mut self,
        p: &Partitura,
        passo: isize,
    ) -> Result<PosicaoDeEntrada, AvisoDeEntrada> {
        let pos = self.posicao.ok_or(AvisoDeEntrada::SemPosicao)?;
        let todas = faixas(p);
        let i = todas
            .iter()
            .position(|f| *f == pos.faixa)
            .ok_or(AvisoDeEntrada::PosicaoInvalida)?;
        let j = i.checked_add_signed(passo).filter(|j| *j < todas.len());
        let Some(j) = j else {
            return Err(if passo < 0 {
                AvisoDeEntrada::PrimeiraFaixa
            } else {
                AvisoDeEntrada::UltimaFaixa
            });
        };
        let nova = PosicaoDeEntrada {
            faixa: todas[j],
            ..pos
        };
        self.posicao = Some(nova);
        Ok(nova)
    }

    /// O que há sob o cursor (ver [`Conteudo`]).
    pub fn conteudo(&self, p: &Partitura, ids: &IdsPorNota) -> Option<Conteudo> {
        let pos = self.posicao?;
        let tempo = self.tempo(p)?;
        let parte = p.partes.get(pos.faixa.parte)?;
        let c = parte.compassos.get(pos.compasso)?;
        let mut pausa = None;
        let mut meio = None;
        for (oi, n) in c.notas.iter().enumerate() {
            if !da_faixa(n, pos.faixa) || n.adorno {
                continue;
            }
            let id = ids.id(pos.faixa.parte, pos.compasso, oi)?;
            if n.posicao == tempo && !n.e_pausa() {
                return Some(Conteudo::Nota(id));
            }
            if n.posicao <= tempo && tempo < n.posicao + n.duracao {
                if n.e_pausa() {
                    pausa = pausa.or(Some(id));
                } else if !n.acorde {
                    meio = meio.or(Some(id));
                }
            }
        }
        Some(match (pausa, meio) {
            (Some(id), _) => Conteudo::Pausa(id),
            (None, Some(id)) => Conteudo::MeioDeNota(id),
            (None, None) => Conteudo::Nada,
        })
    }

    /// A pausa sob o cursor, se for isso que está lá ("pausa selecionada").
    pub fn pausa_selecionada(&self, p: &Partitura, ids: &IdsPorNota) -> Option<IdNota> {
        match self.conteudo(p, ids)? {
            Conteudo::Pausa(id) => Some(id),
            _ => None,
        }
    }

    /// Descrição em texto da posição (linha de estado e leitor de tela).
    pub fn descricao(&self, p: &Partitura, ids: &IdsPorNota) -> Option<String> {
        let pos = self.posicao?;
        let tempo = self.tempo(p)?;
        let numero = p
            .compassos
            .get(pos.compasso)
            .map_or_else(|| (pos.compasso + 1).to_string(), |i| i.numero.clone());
        // Tempo escrito em batidas de semínima a partir de 1 ("tempo 1", "tempo 2 e meio").
        let batida = tempo.como_f64() + 1.0;
        let batida = if (batida - batida.round()).abs() < 1e-9 {
            format!("{}", batida.round() as i64)
        } else {
            format!("{batida:.2}").replace('.', ",")
        };
        let o_que = match self.conteudo(p, ids) {
            Some(Conteudo::Nota(_)) => "nota",
            Some(Conteudo::Pausa(_)) => "pausa selecionada",
            Some(Conteudo::MeioDeNota(_)) => "meio de uma nota",
            _ => "sem nada na voz",
        };
        Some(format!(
            "compasso {numero}, tempo {batida} (coluna {}), pauta {}, voz {}: {o_que}",
            pos.coluna + 1,
            pos.faixa.pauta,
            pos.faixa.voz
        ))
    }
}

// --- Altura pela letra --------------------------------------------------------------------

/// Quantas alterações a armadura dá à letra (`C` = 0, ... `B` = 6): +1 sustenido, -1 bemol.
fn alteracao_da_armadura(a: Armadura, passo: u8) -> i8 {
    // Ordem dos sustenidos: Fá Dó Sol Ré Lá Mi Si; dos bemóis, a inversa.
    const SUSTENIDOS: [u8; 7] = [3, 0, 4, 1, 5, 2, 6];
    const BEMOIS: [u8; 7] = [6, 2, 5, 1, 4, 0, 3];
    let n = usize::from(a.quintas.unsigned_abs().min(7));
    if a.quintas > 0 && SUSTENIDOS[..n].contains(&passo) {
        1
    } else if a.quintas < 0 && BEMOIS[..n].contains(&passo) {
        -1
    } else {
        0
    }
}

/// Linha do meio da clave em vigor na pauta (posição diatônica absoluta, Dó4 = 28): Sol = Si4,
/// Fá = Ré3, Dó = Dó4; as demais usam Si4.
fn meio_da_clave(p: &Partitura, f: Faixa, compasso: usize, tempo: Fracao) -> i32 {
    let mut sinal = SinalClave::Sol;
    if let Some(parte) = p.partes.get(f.parte) {
        for (c, comp) in parte.compassos.iter().enumerate().take(compasso + 1) {
            for m in comp.claves.iter().filter(|m| m.pauta == f.pauta) {
                if c < compasso || m.posicao <= tempo {
                    sinal = m.clave.sinal;
                }
            }
        }
    }
    match sinal {
        SinalClave::Fa => 3 * 7 + 1,
        SinalClave::Do => 4 * 7,
        _ => 4 * 7 + 6,
    }
}

/// A altura da letra `A` a `G` na posição do cursor: a oitava mais próxima da última nota da voz
/// antes do cursor (sem nota antes, da linha do meio da clave) e a alteração da armadura do
/// compasso. `None` se a letra não é de `A` a `G` ou não há posição.
pub fn altura_da_letra(
    p: &Partitura,
    cursor: &CursorDeEntrada,
    letra: char,
) -> Option<AlturaEscrita> {
    let passo = match letra.to_ascii_uppercase() {
        'C' => 0u8,
        'D' => 1,
        'E' => 2,
        'F' => 3,
        'G' => 4,
        'A' => 5,
        'B' => 6,
        _ => return None,
    };
    let pos = cursor.posicao()?;
    let tempo = cursor.tempo(p)?;
    // Última nota com altura da voz estritamente antes do cursor.
    let mut referencia: Option<i32> = None;
    if let Some(parte) = p.partes.get(pos.faixa.parte) {
        'busca: for c in (0..=pos.compasso).rev() {
            let Some(comp) = parte.compassos.get(c) else {
                continue;
            };
            let mut melhor: Option<(Fracao, i32)> = None;
            for n in comp.notas.iter().filter(|n| da_faixa(n, pos.faixa)) {
                let Some(a) = n.altura else { continue };
                if c == pos.compasso && n.posicao >= tempo {
                    continue;
                }
                if melhor.is_none_or(|(q, _)| n.posicao >= q) {
                    melhor = Some((n.posicao, a.diatonica()));
                }
            }
            if let Some((_, d)) = melhor {
                referencia = Some(d);
                break 'busca;
            }
        }
    }
    let alvo = referencia.unwrap_or_else(|| meio_da_clave(p, pos.faixa, pos.compasso, tempo));
    let oitava = (alvo.div_euclid(7) - 1..=alvo.div_euclid(7) + 1)
        .min_by_key(|o| (o * 7 + i32::from(passo) - alvo).abs())?;
    let armadura = p.compassos.get(pos.compasso)?.armadura;
    Some(AlturaEscrita {
        passo: ['C', 'D', 'E', 'F', 'G', 'A', 'B'][usize::from(passo)],
        alter: alteracao_da_armadura(armadura, passo),
        oitava: i8::try_from(oitava).ok()?,
    })
}

#[cfg(test)]
mod testes {
    use super::*;
    use piano_tutor_core::documento::DocumentoAberto;

    /// Um piano de duas pautas, 4/4, divisions 1. Compasso 1: voz 1 (pauta 1) Dó4 (1), pausa (2),
    /// Ré4 (1); voz 2 (pauta 1) só pausa de semibreve; voz 5 (pauta 2) Dó3 (semibreve).
    /// Compasso 2: voz 1 pausa de semibreve; voz 5 Mi3 (2) e Sol3 (2).
    const PIANO: &str = "<?xml version=\"1.0\"?>\n<score-partwise version=\"4.0\">\
<part-list><score-part id=\"P1\"><part-name>Piano</part-name></score-part></part-list>\
<part id=\"P1\">\
<measure number=\"1\"><attributes><divisions>1</divisions><key><fifths>2</fifths></key>\
<time><beats>4</beats><beat-type>4</beat-type></time><staves>2</staves>\
<clef number=\"1\"><sign>G</sign><line>2</line></clef>\
<clef number=\"2\"><sign>F</sign><line>4</line></clef></attributes>\
<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>1</staff></note>\
<note><rest/><duration>2</duration><voice>1</voice><type>half</type><staff>1</staff></note>\
<note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>1</staff></note>\
<backup><duration>4</duration></backup>\
<note><rest/><duration>4</duration><voice>2</voice><type>whole</type><staff>1</staff></note>\
<backup><duration>4</duration></backup>\
<note><pitch><step>C</step><octave>3</octave></pitch><duration>4</duration><voice>5</voice><type>whole</type><staff>2</staff></note>\
</measure>\
<measure number=\"2\">\
<note><rest/><duration>4</duration><voice>1</voice><type>whole</type><staff>1</staff></note>\
<backup><duration>4</duration></backup>\
<note><rest/><duration>4</duration><voice>2</voice><type>whole</type><staff>1</staff></note>\
<backup><duration>4</duration></backup>\
<note><pitch><step>E</step><octave>3</octave></pitch><duration>2</duration><voice>5</voice><type>half</type><staff>2</staff></note>\
<note><pitch><step>G</step><octave>3</octave></pitch><duration>2</duration><voice>5</voice><type>half</type><staff>2</staff></note>\
</measure></part></score-partwise>";

    fn doc() -> DocumentoAberto {
        DocumentoAberto::abrir_texto("piano", PIANO).expect("abre")
    }

    const V1: Faixa = Faixa {
        parte: 0,
        pauta: 1,
        voz: 1,
    };
    const V2: Faixa = Faixa {
        parte: 0,
        pauta: 1,
        voz: 2,
    };
    const V5: Faixa = Faixa {
        parte: 0,
        pauta: 2,
        voz: 5,
    };

    fn q(n: i64) -> Fracao {
        Fracao::inteiro(n)
    }

    #[test]
    fn colunas_sao_os_inicios_de_evento_de_todas_as_pautas_sem_repeticao() {
        let d = doc();
        let p = d.partitura();
        assert_eq!(colunas_do_compasso(p, 0), vec![q(0), q(1), q(3)]);
        assert_eq!(colunas_do_compasso(p, 1), vec![q(0), q(2)]);
        assert_eq!(faixas(p), vec![V1, V2, V5], "inclui a voz só de pausas");
    }

    #[test]
    fn posicionar_por_coluna_voz_e_pauta_nunca_por_pixel_e_recusa_o_que_nao_existe() {
        let d = doc();
        let p = d.partitura();
        let mut c = CursorDeEntrada::default();
        assert_eq!(c.posicao(), None);
        assert_eq!(c.seguinte(p), Err(AvisoDeEntrada::SemPosicao));
        c.posicionar(p, V1, 0, 1).expect("posiciona");
        assert_eq!(c.tempo(p), Some(q(1)));
        assert_eq!(
            c.posicionar(p, V1, 0, 3),
            Err(AvisoDeEntrada::PosicaoInvalida)
        );
        assert_eq!(
            c.posicionar(p, V1, 9, 0),
            Err(AvisoDeEntrada::PosicaoInvalida)
        );
        let fantasma = Faixa { voz: 9, ..V1 };
        assert_eq!(
            c.posicionar(p, fantasma, 0, 0),
            Err(AvisoDeEntrada::PosicaoInvalida)
        );
        // A posição anterior não mudou com as recusas.
        assert_eq!(c.tempo(p), Some(q(1)));
    }

    #[test]
    fn o_que_ha_sob_o_cursor_nota_pausa_meio_de_pausa_e_meio_de_nota() {
        let d = doc();
        let (p, ids) = (d.partitura(), d.ids());
        let mut c = CursorDeEntrada::default();
        // Voz 1: Dó4 no tempo 0, pausa de mínima no tempo 1, Ré4 no tempo 3.
        c.posicionar(p, V1, 0, 0).expect("ok");
        assert!(matches!(c.conteudo(p, ids), Some(Conteudo::Nota(_))));
        c.posicionar(p, V1, 0, 1).expect("ok");
        let pausa = c.pausa_selecionada(p, ids).expect("pausa selecionada");
        assert_eq!(
            ids.id(0, 0, 1),
            Some(pausa),
            "a pausa de mínima do compasso 1"
        );
        // Voz 2 (só pausa): a pausa de semibreve cobre todas as colunas, inclusive o meio dela.
        c.posicionar(p, V2, 0, 2).expect("ok");
        assert!(c.pausa_selecionada(p, ids).is_some());
        // Voz 5: Dó3 dura o compasso todo; nas colunas 1 e 3 é "meio de nota".
        c.posicionar(p, V5, 0, 1).expect("ok");
        assert!(matches!(c.conteudo(p, ids), Some(Conteudo::MeioDeNota(_))));
        assert_eq!(c.pausa_selecionada(p, ids), None);
        // Descrição em texto, para a linha de estado.
        let t = c.descricao(p, ids).expect("descrição");
        assert!(
            t.contains("compasso 1") && t.contains("tempo 2") && t.contains("voz 5"),
            "{t}"
        );
        c.posicionar(p, V1, 0, 1).expect("ok");
        assert!(
            c.descricao(p, ids)
                .expect("d")
                .contains("pausa selecionada")
        );
    }

    #[test]
    fn seguinte_e_anterior_andam_pelos_inicios_da_voz_com_pausas_e_atravessam_compassos() {
        let d = doc();
        let p = d.partitura();
        let mut c = CursorDeEntrada::default();
        c.posicionar(p, V1, 0, 0).expect("ok");
        let passos: Vec<(usize, Fracao)> = std::iter::from_fn(|| {
            c.seguinte(p)
                .ok()
                .map(|pos| (pos.compasso, c.tempo(p).expect("tempo da posição")))
        })
        .collect();
        // Voz 1: 0, 1 (pausa), 3 no compasso 1 e a pausa de semibreve do compasso 2.
        assert_eq!(passos, vec![(0, q(1)), (0, q(3)), (1, q(0))]);
        assert_eq!(c.seguinte(p), Err(AvisoDeEntrada::FimDaVoz));
        assert_eq!(c.tempo(p), Some(q(0)), "no limite o cursor fica");
        let volta: Vec<(usize, Fracao)> = std::iter::from_fn(|| {
            c.anterior(p)
                .ok()
                .map(|pos| (pos.compasso, c.tempo(p).expect("tempo da posição")))
        })
        .collect();
        assert_eq!(volta, vec![(0, q(3)), (0, q(1)), (0, q(0))]);
        assert_eq!(c.anterior(p), Err(AvisoDeEntrada::InicioDaVoz));
        // Voz 5: pula as colunas onde ela não começa nada (1 e 3 do compasso 1).
        c.posicionar(p, V5, 0, 0).expect("ok");
        let pos = c.seguinte(p).expect("segue");
        assert_eq!((pos.compasso, c.tempo(p)), (1, Some(q(0))));
    }

    #[test]
    fn trocar_de_faixa_alcanca_a_voz_so_de_pausas_e_para_nos_limites() {
        let d = doc();
        let p = d.partitura();
        let mut c = CursorDeEntrada::default();
        c.posicionar(p, V1, 0, 1).expect("ok");
        assert_eq!(c.faixa_acima(p), Err(AvisoDeEntrada::PrimeiraFaixa));
        let pos = c.faixa_abaixo(p).expect("voz 2");
        assert_eq!((pos.faixa, pos.compasso, pos.coluna), (V2, 0, 1));
        assert_eq!(c.faixa_abaixo(p).expect("pauta 2").faixa, V5);
        assert_eq!(c.faixa_abaixo(p), Err(AvisoDeEntrada::UltimaFaixa));
        assert_eq!(c.faixa_acima(p).expect("volta").faixa, V2);
    }

    #[test]
    fn selecionar_nota_leva_o_cursor_para_a_posicao_dela() {
        let d = doc();
        let (p, ids) = (d.partitura(), d.ids());
        let mut c = CursorDeEntrada::default();
        let re = ids.id(0, 0, 2).expect("Ré4");
        assert!(c.selecionar_nota(p, ids, re));
        let pos = c.posicao().expect("posição");
        assert_eq!((pos.faixa, pos.compasso, c.tempo(p)), (V1, 0, Some(q(3))));
        assert!(!c.selecionar_nota(p, ids, IdNota(9999)));
        assert_eq!(c.tempo(p), Some(q(3)), "id desconhecido não move");
    }

    #[test]
    fn duracao_de_entrada_so_aceita_de_um_a_oito_e_o_ponto_alterna() {
        let mut c = CursorDeEntrada::default();
        assert_eq!(
            c.duracao(),
            DuracaoDeEntrada {
                tecla: 5,
                pontos: 0
            }
        );
        assert!(c.definir_duracao(4));
        c.alternar_ponto();
        assert_eq!(
            c.duracao(),
            DuracaoDeEntrada {
                tecla: 4,
                pontos: 1
            }
        );
        assert!(!c.definir_duracao(9), "a longa o modelo não tem");
        assert!(!c.definir_duracao(0));
        assert_eq!(c.duracao().tecla, 4);
        assert!(c.definir_duracao(6));
        assert_eq!(c.duracao().pontos, 0, "a tecla zera o ponto");
        c.alternar_ponto();
        c.alternar_ponto();
        assert_eq!(c.duracao().pontos, 0);
    }

    #[test]
    fn altura_da_letra_usa_a_oitava_mais_proxima_e_a_armadura() {
        let d = doc();
        let p = d.partitura();
        let mut c = CursorDeEntrada::default();
        // Voz 1, depois de Dó4 (armadura de Ré maior: Fá e Dó sustenidos): o Sol mais próximo
        // fica abaixo (uma quarta) e não acima (uma quinta).
        c.posicionar(p, V1, 0, 1).expect("ok");
        let g = altura_da_letra(p, &c, 'g').expect("sol");
        assert_eq!((g.passo, g.alter, g.oitava), ('G', 0, 3));
        let f = altura_da_letra(p, &c, 'F').expect("fá");
        assert_eq!(
            (f.passo, f.alter, f.oitava),
            ('F', 1, 4),
            "armadura: Fá sustenido"
        );
        let b = altura_da_letra(p, &c, 'B').expect("si");
        assert_eq!(
            (b.passo, b.alter, b.oitava),
            ('B', 0, 3),
            "o Si mais próximo de Dó4 é o grave"
        );
        assert_eq!(altura_da_letra(p, &c, 'H'), None);
        // Na pauta de fá, sem nota antes no começo: ao redor do Ré3 (meio da clave de fá).
        c.posicionar(p, V5, 0, 0).expect("ok");
        let a = altura_da_letra(p, &c, 'A').expect("lá");
        assert_eq!((a.passo, a.oitava), ('A', 2));
        // No compasso 2 da voz 5, a referência é a última nota do compasso 1 (Dó3).
        c.posicionar(p, V5, 1, 1).expect("ok");
        let e = altura_da_letra(p, &c, 'E').expect("mi");
        assert_eq!((e.passo, e.oitava), ('E', 3));
        // Sem posição: nada.
        assert_eq!(altura_da_letra(p, &CursorDeEntrada::default(), 'C'), None);
    }

    #[test]
    fn as_colunas_sao_as_do_layout() {
        use crate::partitura_visual::calcular_layout_com_ids;
        let d = doc();
        let (p, ids) = (d.partitura(), d.ids());
        let l = calcular_layout_com_ids(p, ids, 60.0);
        for c in 0..p.compassos.len() {
            let do_layout: Vec<f64> = l.compassos[c].colunas.iter().map(|(t, _)| *t).collect();
            let do_cursor: Vec<f64> = colunas_do_compasso(p, c)
                .iter()
                .map(|f| f.como_f64())
                .collect();
            assert_eq!(do_cursor, do_layout, "compasso {c}");
        }
    }
}
