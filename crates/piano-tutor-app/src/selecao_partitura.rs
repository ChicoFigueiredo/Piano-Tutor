//! Seleção de nota e navegação pela partitura (ticket 12, E01, seam S1): estado e regras **sem
//! janela e sem egui**. A tela só traduz clique e teclas em chamadas deste módulo e pinta o
//! resultado.
//!
//! # Contrato em linguagem de domínio
//!
//! - **Identidade, não posição.** A nota selecionada é guardada pela [`IdNota`] que o serviço de
//!   documento atribuiu. Se o documento mudar (edição do ticket 14), [`SelecaoPartitura::reancorar`]
//!   procura a mesma identidade no índice novo: ela continua selecionada onde estiver; só se a
//!   nota deixou de existir a seleção é limpa, com aviso.
//! - **Trilha** é um fio de leitura: a mesma parte, pauta e voz ([`Trilha`]). Só entram trilhas
//!   que têm ao menos uma nota com altura: voz só de pausas **não existe** para a navegação (as
//!   pausas não têm cabeça de nota para selecionar).
//! - **Ordem de leitura** dentro da trilha: compasso, depois posição no compasso; num acorde, da
//!   nota mais grave para a mais aguda. `←` e `→` andam por essa ordem, atravessando acordes e
//!   barras de compasso. Nos limites o cursor **fica** onde está e há um [`Aviso`] discreto.
//! - **Troca de trilha** (`Tab` e `Shift+Tab`): vai para a trilha seguinte (ou anterior) que
//!   existe, em ciclo, e cai na nota dela **mais próxima no tempo** da nota atual (em empate,
//!   a de altura mais próxima, depois a anterior). Com uma trilha só, fica e avisa.
//! - **Clique** ([`SelecaoPartitura::clicar`]): em nota, seleciona a nota; fora de nota, devolve o
//!   compasso para o comportamento de sempre (compasso de partida) **sem perder** a nota
//!   selecionada; fora de tudo, não muda nada.
//! - Nada aqui edita o documento nem o marca como sujo: a seleção vive fora dele.
//!
//! # Contraste
//!
//! A cor da seleção é conferida por [`razao_de_contraste`] (razão de luminância relativa do
//! WCAG 2.x) contra o papel: no mínimo [`CONTRASTE_MINIMO`] (3:1, critério 1.4.11 de componente
//! não textual) e, no modo de alto contraste, [`CONTRASTE_MINIMO_ALTO`] (7:1, nível AAA do 1.4.6).
//! A seleção nunca depende só da cor: há anel em volta da nota e a descrição em texto. Leitor de
//! tela real é NÃO VERIFICADO.

use crate::geometria::IdNota;
use crate::partitura_visual::Acerto;
use piano_tutor_core::documento::IdsPorNota;
use piano_tutor_core::partitura::{Altura, Fracao, Partitura};
use std::collections::{BTreeMap, HashMap};

/// Razão mínima de contraste da seleção contra o papel (WCAG 2.2, critério 1.4.11).
pub const CONTRASTE_MINIMO: f32 = 3.0;
/// Razão mínima no modo de alto contraste (WCAG, nível AAA do critério 1.4.6).
pub const CONTRASTE_MINIMO_ALTO: f32 = 7.0;

/// Razão de contraste WCAG entre duas cores sRGB opacas (de 1,0 a 21,0).
pub fn razao_de_contraste(a: [u8; 3], b: [u8; 3]) -> f32 {
    let (la, lb) = (luminancia(a), luminancia(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Luminância relativa (WCAG) de uma cor sRGB.
fn luminancia(c: [u8; 3]) -> f32 {
    let lin = |v: u8| {
        let s = f32::from(v) / 255.0;
        if s <= 0.039_28 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// Uma nota (com altura) na ordem de leitura.
#[derive(Clone, Debug, PartialEq)]
pub struct EntradaNota {
    pub id: IdNota,
    pub parte: usize,
    pub compasso: usize,
    pub pauta: u8,
    pub voz: u8,
    /// Início desde o começo da peça, em semínimas (ordem escrita).
    pub inicio_q: f64,
    pub altura: Altura,
}

/// Fio de leitura: parte, pauta e voz que têm ao menos uma nota com altura.
#[derive(Clone, Debug, PartialEq)]
pub struct Trilha {
    pub parte: usize,
    pub pauta: u8,
    pub voz: u8,
    pub notas: Vec<EntradaNota>,
}

/// Notas que soam juntas (um acorde é um grupo) e a posição delas no compasso.
type Grupo = (Fracao, Vec<EntradaNota>);

/// Índice de leitura derivado da partitura e das identidades de nota. Recalcule ao trocar ou
/// editar o documento.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IndiceLeitura {
    trilhas: Vec<Trilha>,
    por_id: HashMap<IdNota, (usize, usize)>,
}

/// Sentido de um passo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Passo {
    Anterior,
    Seguinte,
}

impl IndiceLeitura {
    /// Monta o índice. Notas sem identidade ou sem altura (pausas) não entram.
    pub fn de(p: &Partitura, ids: &IdsPorNota) -> Self {
        let mut mapa: BTreeMap<(usize, u8, u8), Vec<EntradaNota>> = BTreeMap::new();
        for (pi, parte) in p.partes.iter().enumerate() {
            for (ci, comp) in parte.compassos.iter().enumerate() {
                let base = p.compassos.get(ci).map_or(0.0, |i| i.inicio.como_f64());
                // Por trilha, grupos de notas que soam juntas (um acorde é um grupo).
                let mut grupos: BTreeMap<(u8, u8), Vec<Grupo>> = BTreeMap::new();
                for (oi, n) in comp.notas.iter().enumerate() {
                    let (Some(altura), Some(id)) = (n.altura, ids.id(pi, ci, oi)) else {
                        continue;
                    };
                    let e = EntradaNota {
                        id,
                        parte: pi,
                        compasso: ci,
                        pauta: n.pauta,
                        voz: n.voz,
                        inicio_q: base + n.posicao.como_f64(),
                        altura,
                    };
                    let g = grupos.entry((n.pauta, n.voz)).or_default();
                    match g.last_mut() {
                        Some((_, notas)) if n.acorde => notas.push(e),
                        _ => g.push((n.posicao, vec![e])),
                    }
                }
                for ((pauta, voz), mut gs) in grupos {
                    // Estável: notas de adorno (mesma posição) ficam antes da principal.
                    gs.sort_by_key(|(pos, _)| *pos);
                    let destino = mapa.entry((pi, pauta, voz)).or_default();
                    for (_, mut notas) in gs {
                        notas.sort_by_key(|n| (n.altura.diatonica(), n.altura.alteracao));
                        destino.extend(notas);
                    }
                }
            }
        }
        Self::de_trilhas(
            mapa.into_iter()
                .map(|((parte, pauta, voz), notas)| Trilha {
                    parte,
                    pauta,
                    voz,
                    notas,
                })
                .collect(),
        )
    }

    /// Índice a partir de trilhas já montadas (para testes e para o editor).
    pub fn de_trilhas(trilhas: Vec<Trilha>) -> Self {
        let trilhas: Vec<Trilha> = trilhas
            .into_iter()
            .filter(|t| !t.notas.is_empty())
            .collect();
        let mut por_id = HashMap::new();
        for (t, tr) in trilhas.iter().enumerate() {
            for (i, n) in tr.notas.iter().enumerate() {
                por_id.insert(n.id, (t, i));
            }
        }
        Self { trilhas, por_id }
    }

    pub fn trilhas(&self) -> &[Trilha] {
        &self.trilhas
    }

    pub fn total(&self) -> usize {
        self.por_id.len()
    }

    pub fn contem(&self, id: IdNota) -> bool {
        self.por_id.contains_key(&id)
    }

    pub fn entrada(&self, id: IdNota) -> Option<&EntradaNota> {
        let &(t, i) = self.por_id.get(&id)?;
        self.trilhas.get(t)?.notas.get(i)
    }

    /// A primeira nota da primeira trilha.
    pub fn primeira(&self) -> Option<IdNota> {
        self.trilhas.first()?.notas.first().map(|n| n.id)
    }

    /// A nota anterior ou seguinte na mesma trilha; `None` no limite (ou id desconhecido).
    pub fn vizinha(&self, id: IdNota, passo: Passo) -> Option<IdNota> {
        let &(t, i) = self.por_id.get(&id)?;
        let notas = &self.trilhas.get(t)?.notas;
        let j = match passo {
            Passo::Anterior => i.checked_sub(1)?,
            Passo::Seguinte => i + 1,
        };
        notas.get(j).map(|n| n.id)
    }

    /// A nota mais próxima no tempo na trilha seguinte ou anterior (em ciclo). `None` com uma
    /// trilha só ou id desconhecido.
    pub fn outra_trilha(&self, id: IdNota, passo: Passo) -> Option<IdNota> {
        let n = self.trilhas.len();
        let &(t, _) = self.por_id.get(&id)?;
        if n < 2 {
            return None;
        }
        let atual = self.entrada(id)?;
        let alvo = match passo {
            Passo::Seguinte => (t + 1) % n,
            Passo::Anterior => (t + n - 1) % n,
        };
        // Distância no tempo (em milionésimos de semínima, para empates exatos), depois na
        // altura (graus diatônicos), depois a mais cedo.
        let chave = |e: &&EntradaNota| {
            (
                ((e.inicio_q - atual.inicio_q).abs() * 1e6).round() as i64,
                (e.altura.diatonica() - atual.altura.diatonica()).abs(),
                (e.inicio_q * 1e6).round() as i64,
            )
        };
        self.trilhas[alvo]
            .notas
            .iter()
            .min_by_key(chave)
            .map(|e| e.id)
    }
}

/// Comando de navegação (a tabela de teclas, que depois será remapeável, fica na tela).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comando {
    Anterior,
    Seguinte,
    TrilhaAnterior,
    TrilhaSeguinte,
}

/// Aviso discreto de limite ou de situação.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aviso {
    PrimeiraNota,
    UltimaNota,
    UmaSoVoz,
    SemNotas,
    NotaSumiu,
}

impl Aviso {
    pub fn texto(self) -> &'static str {
        match self {
            Aviso::PrimeiraNota => "Esta é a primeira nota desta voz.",
            Aviso::UltimaNota => "Esta é a última nota desta voz.",
            Aviso::UmaSoVoz => "Só há uma voz ou pauta com notas.",
            Aviso::SemNotas => "A partitura não tem notas para selecionar.",
            Aviso::NotaSumiu => "A nota selecionada deixou de existir.",
        }
    }
}

/// O que um comando fez.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resultado {
    /// A seleção está agora nesta nota (pode ser a mesma, se o comando a selecionou de início).
    Moveu(IdNota),
    /// A seleção não mudou; o aviso explica.
    Ficou(Aviso),
}

/// O que um clique na partitura fez.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EfeitoDoClique {
    NotaSelecionada(IdNota),
    /// Clique fora de nota, dentro de um compasso: o chamador trata o compasso (compasso de
    /// partida); a nota selecionada não muda.
    Compasso(usize),
    Nenhum,
}

/// Estado da seleção: a nota (por identidade), o último aviso e o pedido de rolagem.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SelecaoPartitura {
    nota: Option<IdNota>,
    aviso: Option<Aviso>,
    rolar: bool,
}

impl SelecaoPartitura {
    pub fn nota(&self) -> Option<IdNota> {
        self.nota
    }

    pub fn aviso(&self) -> Option<Aviso> {
        self.aviso
    }

    /// Seleciona a nota `id` se ela existir no índice.
    pub fn selecionar(&mut self, idx: &IndiceLeitura, id: IdNota) -> bool {
        if !idx.contem(id) {
            return false;
        }
        self.nota = Some(id);
        self.aviso = None;
        true
    }

    pub fn limpar(&mut self) {
        self.nota = None;
        self.aviso = None;
        self.rolar = false;
    }

    /// Executa um comando de navegação. Sem seleção, qualquer comando seleciona a primeira nota.
    pub fn executar(&mut self, idx: &IndiceLeitura, cmd: Comando) -> Resultado {
        let Some(atual) = self.nota.filter(|id| idx.contem(*id)) else {
            // Sem seleção (ou com uma identidade que o índice não conhece): começa pela primeira.
            return match idx.primeira() {
                Some(id) => self.mover_para(id),
                None => {
                    self.nota = None;
                    self.aviso = Some(Aviso::SemNotas);
                    Resultado::Ficou(Aviso::SemNotas)
                }
            };
        };
        let alvo = match cmd {
            Comando::Anterior => idx
                .vizinha(atual, Passo::Anterior)
                .ok_or(Aviso::PrimeiraNota),
            Comando::Seguinte => idx.vizinha(atual, Passo::Seguinte).ok_or(Aviso::UltimaNota),
            Comando::TrilhaAnterior => idx
                .outra_trilha(atual, Passo::Anterior)
                .ok_or(Aviso::UmaSoVoz),
            Comando::TrilhaSeguinte => idx
                .outra_trilha(atual, Passo::Seguinte)
                .ok_or(Aviso::UmaSoVoz),
        };
        match alvo {
            Ok(id) => self.mover_para(id),
            Err(aviso) => {
                self.aviso = Some(aviso);
                Resultado::Ficou(aviso)
            }
        }
    }

    fn mover_para(&mut self, id: IdNota) -> Resultado {
        self.nota = Some(id);
        self.aviso = None;
        self.rolar = true;
        Resultado::Moveu(id)
    }

    /// Traduz o acerto de um clique. Ver o contrato do módulo.
    pub fn clicar(&mut self, idx: &IndiceLeitura, acerto: Option<Acerto>) -> EfeitoDoClique {
        match acerto {
            Some(Acerto::Nota(id)) if self.selecionar(idx, id) => {
                EfeitoDoClique::NotaSelecionada(id)
            }
            Some(Acerto::Compasso(c)) => EfeitoDoClique::Compasso(c),
            _ => EfeitoDoClique::Nenhum,
        }
    }

    /// Depois de uma mudança no documento: a mesma identidade continua selecionada onde estiver
    /// no índice novo; se sumiu, a seleção é limpa com [`Aviso::NotaSumiu`].
    pub fn reancorar(&mut self, idx: &IndiceLeitura) {
        if let Some(id) = self.nota
            && !idx.contem(id)
        {
            self.nota = None;
            self.rolar = false;
            self.aviso = Some(Aviso::NotaSumiu);
        }
    }

    /// A nota a levar para a vista, uma única vez (depois de navegar pelo teclado).
    pub fn tomar_rolagem(&mut self) -> Option<IdNota> {
        if std::mem::take(&mut self.rolar) {
            self.nota
        } else {
            None
        }
    }
}

const NOMES_DOS_PASSOS: [&str; 7] = ["Dó", "Ré", "Mi", "Fá", "Sol", "Lá", "Si"];

/// Nome da altura em português, sem símbolos fora da fonte da janela: "Fá sustenido 4".
pub fn nome_da_altura(a: Altura) -> String {
    let nome = NOMES_DOS_PASSOS[usize::from(a.passo % 7)];
    let alteracao = match a.alteracao {
        0 => String::new(),
        1 => " sustenido".to_owned(),
        -1 => " bemol".to_owned(),
        2 => " dobrado sustenido".to_owned(),
        -2 => " dobrado bemol".to_owned(),
        n => format!(" ({n:+} semitons)"),
    };
    format!("{nome}{alteracao} {}", a.oitava)
}

/// Descrição em texto da nota (para a linha de status e para leitores de tela): altura,
/// compasso, parte, pauta e voz.
pub fn descrever(p: &Partitura, idx: &IndiceLeitura, id: IdNota) -> Option<String> {
    let e = idx.entrada(id)?;
    let numero = p
        .compassos
        .get(e.compasso)
        .map_or_else(|| (e.compasso + 1).to_string(), |i| i.numero.clone());
    let mut t = format!("{}, compasso {numero}", nome_da_altura(e.altura));
    if p.partes.len() > 1
        && let Some(parte) = p.partes.get(e.parte)
    {
        let nome = if parte.nome.trim().is_empty() {
            &parte.id
        } else {
            &parte.nome
        };
        t.push_str(&format!(", parte {nome}"));
    }
    t.push_str(&format!(", pauta {}, voz {}", e.pauta, e.voz));
    Some(t)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::partitura_visual::{TOLERANCIA_NOTA_SP, calcular_layout_com_ids};
    use piano_tutor_core::documento::DocumentoAberto;

    /// Um piano de duas pautas. Compasso 1: voz 1 (pauta 1) com Dó4 e um acorde Mi4+Sol4 escrito
    /// "de cabeça para baixo" (Sol antes de Mi), depois Ré4 e uma pausa; voz 5 (pauta 2) com Dó3.
    /// Compasso 2: voz 1 com Mi4; voz 2 (pauta 1) só de pausa; voz 5 de pausa.
    const PIANO: &str = "<?xml version=\"1.0\"?>\n<score-partwise version=\"4.0\">\
<part-list><score-part id=\"P1\"><part-name>Piano</part-name></score-part></part-list>\
<part id=\"P1\">\
<measure number=\"1\"><attributes><divisions>1</divisions><staves>2</staves>\
<clef number=\"1\"><sign>G</sign><line>2</line></clef>\
<clef number=\"2\"><sign>F</sign><line>4</line></clef></attributes>\
<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>1</staff></note>\
<note><pitch><step>G</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>1</staff></note>\
<note><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>1</staff></note>\
<note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>1</staff></note>\
<note><rest/><duration>1</duration><voice>1</voice><type>quarter</type><staff>1</staff></note>\
<backup><duration>4</duration></backup>\
<note><pitch><step>C</step><octave>3</octave></pitch><duration>4</duration><voice>5</voice><type>whole</type><staff>2</staff></note>\
</measure>\
<measure number=\"2\">\
<note><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type><staff>1</staff></note>\
<backup><duration>4</duration></backup>\
<note><rest/><duration>4</duration><voice>2</voice><type>whole</type><staff>1</staff></note>\
<backup><duration>4</duration></backup>\
<note><rest/><duration>4</duration><voice>5</voice><type>whole</type><staff>2</staff></note>\
</measure></part></score-partwise>";

    /// Uma voz só, três notas em dois compassos.
    const MELODIA: &str = "<?xml version=\"1.0\"?>\n<score-partwise version=\"4.0\">\
<part-list><score-part id=\"P1\"><part-name>M</part-name></score-part></part-list>\
<part id=\"P1\"><measure number=\"1\"><attributes><divisions>1</divisions></attributes>\
<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><type>half</type></note>\
<note><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><type>half</type></note></measure>\
<measure number=\"2\">\
<note><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note>\
</measure></part></score-partwise>";

    fn abrir(xml: &str) -> DocumentoAberto {
        DocumentoAberto::abrir_texto("t", xml).expect("abre")
    }

    fn indice(d: &DocumentoAberto) -> IndiceLeitura {
        IndiceLeitura::de(d.partitura(), d.ids())
    }

    /// Alturas da trilha `t`, na ordem do índice, como "C4".
    fn alturas(idx: &IndiceLeitura, t: usize) -> Vec<String> {
        idx.trilhas()[t]
            .notas
            .iter()
            .map(|n| {
                format!(
                    "{}{}",
                    ['C', 'D', 'E', 'F', 'G', 'A', 'B'][n.altura.passo as usize],
                    n.altura.oitava
                )
            })
            .collect()
    }

    #[test]
    fn indice_so_tem_trilhas_com_nota_e_ordena_o_acorde_do_grave_ao_agudo() {
        let d = abrir(PIANO);
        let idx = indice(&d);
        // Voz 2 (só pausas) e as pausas não entram: só voz 1 e voz 5 existem.
        let chaves: Vec<_> = idx
            .trilhas()
            .iter()
            .map(|t| (t.parte, t.pauta, t.voz))
            .collect();
        assert_eq!(chaves, vec![(0, 1, 1), (0, 2, 5)]);
        assert_eq!(alturas(&idx, 0), ["C4", "E4", "G4", "D4", "E4"]);
        assert_eq!(alturas(&idx, 1), ["C3"]);
        assert_eq!(idx.total(), 6);
        // As identidades são as do serviço de documento.
        let primeira = idx.trilhas()[0].notas[0].id;
        assert_eq!(Some(primeira), d.ids().id(0, 0, 0));
    }

    #[test]
    fn setas_atravessam_acorde_e_compasso_e_o_limite_fica_com_aviso() {
        let d = abrir(PIANO);
        let idx = indice(&d);
        let mut s = SelecaoPartitura::default();
        let mut visitadas = Vec::new();
        // Sem seleção, → escolhe a primeira nota.
        for _ in 0..5 {
            match s.executar(&idx, Comando::Seguinte) {
                Resultado::Moveu(id) => visitadas.push(id),
                outro => panic!("{outro:?}"),
            }
        }
        let esperadas: Vec<IdNota> = idx.trilhas()[0].notas.iter().map(|n| n.id).collect();
        assert_eq!(visitadas, esperadas);
        assert_eq!(s.nota(), esperadas.last().copied());
        assert_eq!(s.aviso(), None);
        // Limite final: fica, avisa.
        assert_eq!(
            s.executar(&idx, Comando::Seguinte),
            Resultado::Ficou(Aviso::UltimaNota)
        );
        assert_eq!(s.nota(), esperadas.last().copied());
        assert_eq!(s.aviso(), Some(Aviso::UltimaNota));
        // Voltar apaga o aviso e atravessa a barra de compasso e o acorde.
        assert_eq!(
            s.executar(&idx, Comando::Anterior),
            Resultado::Moveu(esperadas[3])
        );
        assert_eq!(s.aviso(), None);
        for k in (0..3).rev() {
            assert_eq!(
                s.executar(&idx, Comando::Anterior),
                Resultado::Moveu(esperadas[k])
            );
        }
        // Limite inicial.
        assert_eq!(
            s.executar(&idx, Comando::Anterior),
            Resultado::Ficou(Aviso::PrimeiraNota)
        );
        assert_eq!(s.nota(), Some(esperadas[0]));
    }

    #[test]
    fn tab_alterna_entre_trilhas_existentes_e_nunca_para_voz_inexistente() {
        let d = abrir(PIANO);
        let idx = indice(&d);
        let voz1: Vec<IdNota> = idx.trilhas()[0].notas.iter().map(|n| n.id).collect();
        let voz5 = idx.trilhas()[1].notas[0].id;
        let mut s = SelecaoPartitura::default();
        s.selecionar(&idx, voz1[0]);
        assert_eq!(
            s.executar(&idx, Comando::TrilhaSeguinte),
            Resultado::Moveu(voz5)
        );
        // Em ciclo: de volta à trilha 1, na nota mais próxima no tempo (início do compasso 1).
        assert_eq!(
            s.executar(&idx, Comando::TrilhaSeguinte),
            Resultado::Moveu(voz1[0])
        );
        assert_eq!(
            s.executar(&idx, Comando::TrilhaAnterior),
            Resultado::Moveu(voz5)
        );
        // Nunca para a voz 2 (só pausas): a partir de qualquer nota, o destino é voz 1 ou 5.
        for &id in &voz1 {
            s.selecionar(&idx, id);
            let Resultado::Moveu(alvo) = s.executar(&idx, Comando::TrilhaSeguinte) else {
                panic!("deveria trocar de trilha");
            };
            assert_eq!(alvo, voz5);
        }
    }

    #[test]
    fn tab_cai_na_nota_mais_proxima_no_tempo() {
        // A pauta de baixo tem Dó3 no início do compasso 1 (tempo 0) e Sol2 no do compasso 2
        // (tempo 4).
        let xml = PIANO.replace(
            "<note><rest/><duration>4</duration><voice>5</voice><type>whole</type><staff>2</staff></note>",
            "<note><pitch><step>G</step><octave>2</octave></pitch><duration>4</duration><voice>5</voice><type>whole</type><staff>2</staff></note>",
        );
        let d = abrir(&xml);
        let idx = indice(&d);
        let voz1 = &idx.trilhas()[0].notas;
        let voz5 = &idx.trilhas()[1].notas;
        assert_eq!(voz5.len(), 2);
        let mut s = SelecaoPartitura::default();
        // Mi4 do compasso 2 (tempo 4) cai no Sol2 (tempo 4), não no Dó3 (tempo 0).
        s.selecionar(&idx, voz1[4].id);
        assert_eq!(
            s.executar(&idx, Comando::TrilhaSeguinte),
            Resultado::Moveu(voz5[1].id)
        );
        // Sol4 (tempo 1): Dó3 está a 1 de distância, Sol2 a 3.
        s.selecionar(&idx, voz1[2].id);
        assert_eq!(
            s.executar(&idx, Comando::TrilhaSeguinte),
            Resultado::Moveu(voz5[0].id)
        );
        // Ré4 (tempo 2): empate no tempo (2 e 2); desempata pela altura mais próxima (Dó3).
        s.selecionar(&idx, voz1[3].id);
        assert_eq!(
            s.executar(&idx, Comando::TrilhaSeguinte),
            Resultado::Moveu(voz5[0].id)
        );
        // Do Dó4 (tempo 0) vai para o Dó3 (tempo 0).
        s.selecionar(&idx, voz1[0].id);
        assert_eq!(
            s.executar(&idx, Comando::TrilhaSeguinte),
            Resultado::Moveu(voz5[0].id)
        );
    }

    #[test]
    fn com_uma_trilha_so_tab_fica_e_avisa() {
        let d = abrir(MELODIA);
        let idx = indice(&d);
        assert_eq!(idx.trilhas().len(), 1);
        let mut s = SelecaoPartitura::default();
        let primeira = idx.primeira().expect("há nota");
        s.selecionar(&idx, primeira);
        for cmd in [Comando::TrilhaSeguinte, Comando::TrilhaAnterior] {
            assert_eq!(s.executar(&idx, cmd), Resultado::Ficou(Aviso::UmaSoVoz));
            assert_eq!(s.nota(), Some(primeira));
        }
    }

    #[test]
    fn partitura_sem_notas_avisa_e_nao_seleciona() {
        let idx = IndiceLeitura::default();
        let mut s = SelecaoPartitura::default();
        assert_eq!(
            s.executar(&idx, Comando::Seguinte),
            Resultado::Ficou(Aviso::SemNotas)
        );
        assert_eq!(s.nota(), None);
        assert!(!s.selecionar(&idx, IdNota(1)));
    }

    #[test]
    fn selecao_e_por_identidade_e_sobrevive_a_um_indice_novo() {
        let d = abrir(MELODIA);
        let idx = indice(&d);
        let notas: Vec<IdNota> = idx.trilhas()[0].notas.iter().map(|n| n.id).collect();
        let mut s = SelecaoPartitura::default();
        s.selecionar(&idx, notas[1]);
        // Uma "edição" que insere uma nota antes: a mesma identidade passa a ser a 3ª da trilha.
        let mut trilhas = idx.trilhas().to_vec();
        let mut nova = trilhas[0].notas[0].clone();
        nova.id = IdNota(999);
        trilhas[0].notas.insert(0, nova);
        let depois = IndiceLeitura::de_trilhas(trilhas);
        s.reancorar(&depois);
        assert_eq!(s.nota(), Some(notas[1]));
        assert_eq!(s.aviso(), None);
        // A vizinha seguinte é a mesma nota de antes (o Mi), apesar da posição ter mudado.
        assert_eq!(
            s.executar(&depois, Comando::Seguinte),
            Resultado::Moveu(notas[2])
        );
        // Se a nota deixa de existir, limpa e avisa.
        let sem = IndiceLeitura::de_trilhas(vec![Trilha {
            notas: vec![depois.trilhas()[0].notas[0].clone()],
            ..depois.trilhas()[0].clone()
        }]);
        s.reancorar(&sem);
        assert_eq!(s.nota(), None);
        assert_eq!(s.aviso(), Some(Aviso::NotaSumiu));
    }

    #[test]
    fn clique_em_nota_seleciona_fora_de_nota_devolve_o_compasso_sem_perder_a_nota() {
        let d = abrir(MELODIA);
        let idx = indice(&d);
        let notas: Vec<IdNota> = idx.trilhas()[0].notas.iter().map(|n| n.id).collect();
        let mut s = SelecaoPartitura::default();
        assert_eq!(
            s.clicar(&idx, Some(Acerto::Nota(notas[1]))),
            EfeitoDoClique::NotaSelecionada(notas[1])
        );
        assert_eq!(s.nota(), Some(notas[1]));
        // Fora de nota, dentro de um compasso: o compasso é do chamador; a nota fica.
        assert_eq!(
            s.clicar(&idx, Some(Acerto::Compasso(1))),
            EfeitoDoClique::Compasso(1)
        );
        assert_eq!(s.nota(), Some(notas[1]));
        // Fora de tudo: nada muda.
        assert_eq!(s.clicar(&idx, None), EfeitoDoClique::Nenhum);
        assert_eq!(s.nota(), Some(notas[1]));
        // Identidade desconhecida (layout velho): ignora, sem perder a seleção.
        assert_eq!(
            s.clicar(&idx, Some(Acerto::Nota(IdNota(12345)))),
            EfeitoDoClique::Nenhum
        );
        assert_eq!(s.nota(), Some(notas[1]));
    }

    #[test]
    fn clicar_numa_cabeca_do_layout_informa_a_mesma_id_do_hit_test() {
        let d = abrir(PIANO);
        let idx = indice(&d);
        let l = calcular_layout_com_ids(d.partitura(), d.ids(), 80.0);
        let mut conferidas = 0;
        for it in l.sistemas.iter().flat_map(|s| &s.itens) {
            let (Some(id), crate::partitura_visual::Forma::Cabeca { centro, .. }) =
                (it.nota, &it.forma)
            else {
                continue;
            };
            let mut s = SelecaoPartitura::default();
            let acerto = l.acertar(*centro, TOLERANCIA_NOTA_SP);
            // Cabeças vizinhas podem se sobrepor: o que importa é que a seleção seja exatamente a
            // identidade que o hit test informou.
            let esperado = l
                .nota_em(*centro, TOLERANCIA_NOTA_SP)
                .expect("sobre a cabeça");
            assert_eq!(acerto, Some(Acerto::Nota(esperado)));
            assert_eq!(
                s.clicar(&idx, acerto),
                EfeitoDoClique::NotaSelecionada(esperado)
            );
            assert_eq!(s.nota(), Some(esperado));
            if esperado == id {
                conferidas += 1;
            }
        }
        assert!(conferidas >= 4, "poucas cabeças conferidas: {conferidas}");
    }

    #[test]
    fn rolagem_e_pedida_uma_vez_depois_de_navegar() {
        let d = abrir(MELODIA);
        let idx = indice(&d);
        let mut s = SelecaoPartitura::default();
        assert_eq!(s.tomar_rolagem(), None);
        s.executar(&idx, Comando::Seguinte);
        assert_eq!(s.tomar_rolagem(), s.nota());
        assert_eq!(s.tomar_rolagem(), None);
    }

    #[test]
    fn navegar_nao_suja_o_documento_nem_muda_a_exportacao() {
        let d = abrir(PIANO);
        let idx = indice(&d);
        let mut s = SelecaoPartitura::default();
        for cmd in [
            Comando::Seguinte,
            Comando::Seguinte,
            Comando::TrilhaSeguinte,
            Comando::Anterior,
            Comando::TrilhaAnterior,
        ] {
            s.executar(&idx, cmd);
        }
        assert!(!d.editado());
        assert!(d.documento().compassos_sujos().is_empty());
        assert_eq!(d.exportar().expect("exporta"), PIANO);
    }

    #[test]
    fn nomes_e_descricao_em_portugues() {
        let alt = |passo, alteracao, oitava| Altura {
            passo,
            alteracao,
            oitava,
        };
        assert_eq!(nome_da_altura(alt(0, 0, 4)), "Dó 4");
        assert_eq!(nome_da_altura(alt(3, 1, 5)), "Fá sustenido 5");
        assert_eq!(nome_da_altura(alt(6, -1, 3)), "Si bemol 3");
        assert_eq!(nome_da_altura(alt(4, 2, 4)), "Sol dobrado sustenido 4");
        assert_eq!(nome_da_altura(alt(4, -2, 4)), "Sol dobrado bemol 4");
        let d = abrir(PIANO);
        let idx = indice(&d);
        let e = idx.trilhas()[0].notas[1].id;
        let t = descrever(d.partitura(), &idx, e).expect("descrição");
        assert!(t.contains("Mi 4"), "{t}");
        assert!(t.contains("compasso 1"), "{t}");
        assert!(t.contains("pauta 1"), "{t}");
        assert!(t.contains("voz 1"), "{t}");
        assert_eq!(descrever(d.partitura(), &idx, IdNota(777)), None);
    }

    #[test]
    fn razao_de_contraste_confere_com_valores_conhecidos_do_wcag() {
        let preto = [0, 0, 0];
        let branco = [255, 255, 255];
        assert!((razao_de_contraste(preto, branco) - 21.0).abs() < 0.01);
        assert!((razao_de_contraste(branco, preto) - 21.0).abs() < 0.01);
        assert!((razao_de_contraste(branco, branco) - 1.0).abs() < 1e-4);
        // #767676 sobre branco é o limite clássico de 4,5:1.
        let r = razao_de_contraste([0x76, 0x76, 0x76], branco);
        assert!((r - 4.54).abs() < 0.02, "{r}");
    }
}
