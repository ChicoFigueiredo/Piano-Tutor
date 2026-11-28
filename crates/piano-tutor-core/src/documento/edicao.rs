//! Edições do spike: altura, acidente e duração. Cada edição trabalha sobre uma cópia do
//! elemento da nota e só a grava se tudo deu certo (rejeita sem alterar nada); liga o bit sujo do
//! compasso e atualiza o modelo semântico.

use super::duracao::{mmc, mmc_dos_denominadores, para_divisions};
use super::erro::ErroDocumento;
use super::grafia::separar_acidente;
use super::texto::{Abertura, No, Peca, fim_da_abertura};
use super::{Altura, Documento, IdNota};
use crate::partitura::Fracao;

/// Posto de cada filho de `<pitch>` no esquema MusicXML.
fn posto_pitch(nome: &str) -> Option<u32> {
    match nome {
        "step" => Some(0),
        "alter" => Some(1),
        "octave" => Some(2),
        _ => None,
    }
}

/// Posto dos filhos de `<note>` (sequência do esquema MusicXML 4.0; filhos fora da lista são
/// "desconhecidos" e ficam onde estão).
pub(super) fn posto_nota(nome: &str) -> Option<u32> {
    Some(match nome {
        "grace" | "cue" => 0,
        "chord" => 1,
        "pitch" | "unpitched" | "rest" => 2,
        "duration" => 3,
        "tie" => 4,
        "instrument" => 5,
        "footnote" => 6,
        "level" => 7,
        "voice" => 8,
        "type" => 9,
        "dot" => 10,
        "accidental" => 11,
        "time-modification" => 12,
        "stem" => 13,
        "notehead" => 14,
        "notehead-text" => 15,
        "staff" => 16,
        "beam" => 17,
        "notations" => 18,
        "lyric" => 19,
        "play" => 20,
        "listen" => 21,
        _ => return None,
    })
}

/// Acidente impresso que corresponde a uma alteração inteira.
fn acidente_de(alter: i8) -> &'static str {
    match alter {
        1 => "sharp",
        -1 => "flat",
        2 => "double-sharp",
        -2 => "flat-flat",
        _ => "natural",
    }
}

pub(super) fn validar(a: Altura) -> Result<(), ErroDocumento> {
    if !('A'..='G').contains(&a.passo) {
        return Err(ErroDocumento::AlturaInvalida(format!(
            "passo '{}'",
            a.passo
        )));
    }
    if !(0..=9).contains(&a.oitava) {
        return Err(ErroDocumento::AlturaInvalida(format!(
            "oitava {}",
            a.oitava
        )));
    }
    if !(-2..=2).contains(&a.alter) {
        return Err(ErroDocumento::AlturaInvalida(format!("alter {}", a.alter)));
    }
    Ok(())
}

pub(super) fn leaf(nome: &str, conteudo: &str) -> No {
    No::novo(nome, format!("<{nome}>{conteudo}</{nome}>"))
}

/// `<accidental>` novo a partir da grafia completa (`nome` ou `nome atributos`).
pub(super) fn leaf_acidente(grafia: &str) -> No {
    let (nome, atributos) = separar_acidente(grafia);
    if atributos.is_empty() {
        leaf("accidental", nome)
    } else {
        No::novo(
            "accidental",
            format!("<accidental {atributos}>{nome}</accidental>"),
        )
    }
}

/// Grafia completa de um `<accidental>` existente: o conteúdo e, se houver, os atributos.
fn grafia_do_acidente(a: &No, base: &str) -> Option<String> {
    let texto = a.texto(base);
    let nome = a.conteudo_folha(base)?;
    let fim = match fim_da_abertura(&texto, 0)? {
        Abertura::Aberta(i) => i - 1,
        Abertura::AutoFechada => return None,
    };
    let atributos = texto["<accidental".len()..fim].trim();
    Some(if atributos.is_empty() {
        nome.trim().to_owned()
    } else {
        format!("{} {atributos}", nome.trim())
    })
}

/// A grafia é aceitável? Nome não vazio e sem marcação; atributos sem `<` nem `>`.
fn grafia_valida(grafia: &str) -> bool {
    let (nome, atributos) = separar_acidente(grafia);
    !nome.is_empty() && !nome.contains(['<', '>', '&']) && !atributos.contains(['<', '>'])
}

/// Põe o `<accidental>` na grafia `t`. Grafia só com o nome troca o conteúdo e mantém os
/// atributos que o elemento já tinha; com atributos, o elemento vira exatamente o pedido.
fn troca_acidente(no: &mut No, base: &str, t: &str) -> Result<(), ErroDocumento> {
    let (nome, atributos) = separar_acidente(t);
    match no.filho_mut("accidental") {
        Some(a) if atributos.is_empty() => a.definir_conteudo(base, nome),
        Some(a) => {
            if grafia_do_acidente(a, base).as_deref() != Some(t.trim()) {
                *a = leaf_acidente(t);
            }
            Ok(())
        }
        None => no.inserir_em_ordem(leaf_acidente(t), &posto_nota),
    }
}

type Local = (usize, usize, usize);

impl Documento {
    fn local(&self, id: IdNota) -> Result<Local, ErroDocumento> {
        self.indice
            .get(&id)
            .copied()
            .ok_or(ErroDocumento::NotaInexistente(id))
    }

    /// Cópia do elemento `<note>` para editar sem tocar no modelo até o fim.
    fn copia_da_nota(&self, (p, c, n): Local) -> Result<No, ErroDocumento> {
        let comp = &self.partes[p].compassos[c];
        match &comp.no.pecas[comp.notas[n].idx] {
            Peca::Elem(e) => Ok(e.clone()),
            Peca::Txt(_) => Err(ErroDocumento::FaixaInconsistente("peça da nota".into())),
        }
    }

    fn gravar_nota(&mut self, (p, c, n): Local, no: No) {
        let comp = &mut self.partes[p].compassos[c];
        let idx = comp.notas[n].idx;
        comp.no.pecas[idx] = Peca::Elem(no);
        comp.sujo = true;
    }

    /// Troca a altura da nota (passo, alteração e oitava). Se a nota tem `<accidental>`, ele
    /// acompanha a nova alteração; se não tem, nenhum é criado (a armadura pode já explicá-lo:
    /// quem quiser um acidente explícito usa [`Documento::definir_acidente`]).
    pub fn definir_altura(&mut self, id: IdNota, nova: Altura) -> Result<(), ErroDocumento> {
        validar(nova)?;
        let loc = self.local(id)?;
        let atual = self.partes[loc.0].compassos[loc.1].notas[loc.2]
            .altura
            .ok_or(ErroDocumento::NotaSemAltura(id))?;
        if atual == nova {
            return Ok(());
        }
        let mut no = self.copia_da_nota(loc)?;
        let base = &self.original;
        let pitch = no
            .filho_mut("pitch")
            .ok_or(ErroDocumento::NotaSemAltura(id))?;
        if nova.passo != atual.passo {
            pitch
                .filho_mut("step")
                .ok_or(ErroDocumento::NotaSemAltura(id))?
                .definir_conteudo(base, &nova.passo.to_string())?;
        }
        if nova.oitava != atual.oitava {
            pitch
                .filho_mut("octave")
                .ok_or(ErroDocumento::NotaSemAltura(id))?
                .definir_conteudo(base, &nova.oitava.to_string())?;
        }
        if nova.alter != atual.alter {
            let texto = nova.alter.to_string();
            match pitch.filho_mut("alter") {
                Some(a) => a.definir_conteudo(base, &texto)?,
                None => pitch.inserir_em_ordem(leaf("alter", &texto), &posto_pitch)?,
            }
            if let Some(ac) = no.filho_mut("accidental") {
                ac.definir_conteudo(base, acidente_de(nova.alter))?;
            }
        }
        self.gravar_nota(loc, no);
        self.partes[loc.0].compassos[loc.1].notas[loc.2].altura = Some(nova);
        Ok(())
    }

    /// Texto do `<accidental>` da nota, se tiver.
    pub fn acidente_da_nota(&self, id: IdNota) -> Result<Option<String>, ErroDocumento> {
        let loc = self.local(id)?;
        let no = self.copia_da_nota(loc)?;
        Ok(no
            .filho("accidental")
            .and_then(|a| grafia_do_acidente(a, &self.original)))
    }

    /// A nota participa de ligadura de prolongamento (`<tie>` ou `<tied>`)?
    pub fn tem_ligadura_de_valor(&self, id: IdNota) -> Result<bool, ErroDocumento> {
        let loc = self.local(id)?;
        let no = self.copia_da_nota(loc)?;
        Ok(no.filho("tie").is_some() || no.texto(&self.original).contains("<tied"))
    }

    /// Troca a altura **e** deixa o `<accidental>` exatamente como `acidente` (`Some` define ou
    /// troca o texto, `None` remove). Atômico: em erro o documento fica como estava. Devolve
    /// `false` (e não suja nada) se a nota já estava assim.
    ///
    /// `<alter>` criado nesta sessão some de novo quando a alteração volta a zero, de modo que
    /// o comando inverso restaura o texto exato.
    pub fn definir_altura_e_acidente(
        &mut self,
        id: IdNota,
        nova: Altura,
        acidente: Option<&str>,
    ) -> Result<bool, ErroDocumento> {
        validar(nova)?;
        if let Some(t) = acidente
            && !grafia_valida(t)
        {
            return Err(ErroDocumento::AlturaInvalida(format!("acidente '{t}'")));
        }
        let loc = self.local(id)?;
        let atual = self.partes[loc.0].compassos[loc.1].notas[loc.2]
            .altura
            .ok_or(ErroDocumento::NotaSemAltura(id))?;
        let mut no = self.copia_da_nota(loc)?;
        let base = &self.original;
        let acidente_atual = no
            .filho("accidental")
            .and_then(|a| grafia_do_acidente(a, base));
        if atual == nova && acidente_atual.as_deref() == acidente {
            return Ok(false);
        }
        let pitch = no
            .filho_mut("pitch")
            .ok_or(ErroDocumento::NotaSemAltura(id))?;
        if nova.passo != atual.passo {
            pitch
                .filho_mut("step")
                .ok_or(ErroDocumento::NotaSemAltura(id))?
                .definir_conteudo(base, &nova.passo.to_string())?;
        }
        if nova.oitava != atual.oitava {
            pitch
                .filho_mut("octave")
                .ok_or(ErroDocumento::NotaSemAltura(id))?
                .definir_conteudo(base, &nova.oitava.to_string())?;
        }
        if nova.alter != atual.alter {
            let texto = nova.alter.to_string();
            match pitch.filho_mut("alter") {
                Some(a) if nova.alter == 0 && a.criado_na_sessao() => {
                    pitch.remover_filho("alter");
                }
                Some(a) => a.definir_conteudo(base, &texto)?,
                None => pitch.inserir_em_ordem(leaf("alter", &texto), &posto_pitch)?,
            }
        }
        match acidente {
            Some(t) => troca_acidente(&mut no, base, t)?,
            None => {
                no.remover_filho("accidental");
            }
        }
        self.gravar_nota(loc, no);
        self.partes[loc.0].compassos[loc.1].notas[loc.2].altura = Some(nova);
        Ok(true)
    }

    /// Define (`Some`), troca ou remove (`None`) o `<accidental>` da nota, na ordem do esquema.
    pub fn definir_acidente(
        &mut self,
        id: IdNota,
        acidente: Option<&str>,
    ) -> Result<(), ErroDocumento> {
        let loc = self.local(id)?;
        let mut no = self.copia_da_nota(loc)?;
        let base = &self.original;
        match acidente {
            Some(t) => {
                if !grafia_valida(t) {
                    return Err(ErroDocumento::AlturaInvalida(format!("acidente '{t}'")));
                }
                troca_acidente(&mut no, base, t)?;
            }
            None => {
                if !no.remover_filho("accidental") {
                    return Ok(());
                }
            }
        }
        self.gravar_nota(loc, no);
        Ok(())
    }

    /// Define a duração (em semínimas) sem mexer em `divisions`. Valor que não seja múltiplo de
    /// `1/divisions` é erro explícito (`DuracaoNaoRepresentavel`), nunca arredondamento.
    pub fn definir_duracao(&mut self, id: IdNota, duracao: Fracao) -> Result<(), ErroDocumento> {
        let loc = self.local(id)?;
        let nota = &self.partes[loc.0].compassos[loc.1].notas[loc.2];
        let divisions = nota.divisions;
        if nota.duracao.is_none() {
            return Err(ErroDocumento::DuracaoInvalida(
                "nota de graça ou sem <duration> inteiro".into(),
            ));
        }
        let unidades = para_divisions(duracao, divisions)?;
        let mut no = self.copia_da_nota(loc)?;
        no.filho_mut("duration")
            .ok_or_else(|| ErroDocumento::DuracaoInvalida("sem <duration>".into()))?
            .definir_conteudo(&self.original, &unidades.to_string())?;
        self.gravar_nota(loc, no);
        self.partes[loc.0].compassos[loc.1].notas[loc.2].duracao =
            Some(Fracao::nova(unidades, divisions));
        Ok(())
    }

    /// Mudar `divisions` é **recusado por padrão** (decisão do supervisor): o caminho normal é
    /// [`Documento::definir_duracao`]. Esta variante só deve ser chamada depois de a interface
    /// propor a mudança e o usuário confirmar (ticket 16), porque suja a parte inteira.
    ///
    /// Como [`Documento::definir_duracao`], mas, se o valor não couber, eleva `divisions` da parte
    /// ao MMC dos denominadores de todas as durações (e reescala todos os compassos da parte, que
    /// ficam sujos). Atômico: em erro o documento fica como estava.
    pub fn definir_duracao_ajustando_divisions(
        &mut self,
        id: IdNota,
        duracao: Fracao,
    ) -> Result<(), ErroDocumento> {
        match self.definir_duracao(id, duracao) {
            Err(ErroDocumento::DuracaoNaoRepresentavel { .. }) => {}
            outro => return outro,
        }
        let loc = self.local(id)?;
        let mut copia = self.clone();
        let novo = copia.divisions_minimas(loc.0, duracao)?;
        copia.elevar_divisions(loc.0, novo)?;
        copia.definir_duracao(id, duracao)?;
        *self = copia;
        Ok(())
    }

    /// `divisions` declarado no início da parte.
    pub fn divisions_da_parte(&self, parte: &str) -> Option<i64> {
        let p = self.partes.iter().find(|p| p.id == parte)?;
        p.compassos.iter().find_map(|c| {
            let a = c.no.filho("attributes")?;
            let d = a.filho("divisions")?;
            d.conteudo_folha(&self.original)?.trim().parse().ok()
        })
    }

    /// Durações (em semínimas) de notas, `backup` e `forward` da parte, mais `extra`.
    fn duracoes_da_parte(&self, p: usize, extra: Fracao) -> Result<Vec<Fracao>, ErroDocumento> {
        let parte = &self.partes[p];
        let div = self
            .divisions_da_parte(&parte.id)
            .ok_or_else(|| ErroDocumento::NaoSuportado("parte sem <divisions>".into()))?;
        let mut v = vec![extra];
        for c in &parte.compassos {
            for (_, e) in c.no.filhos() {
                if matches!(e.nome.as_str(), "note" | "backup" | "forward")
                    && let Some(d) = e.filho("duration")
                    && let Some(t) = d.conteudo_folha(&self.original)
                    && let Ok(x) = t.trim().parse::<i64>()
                {
                    v.push(Fracao::nova(x, div));
                }
            }
        }
        Ok(v)
    }

    fn divisions_minimas(&self, p: usize, extra: Fracao) -> Result<i64, ErroDocumento> {
        mmc_dos_denominadores(&self.duracoes_da_parte(p, extra)?)
    }

    /// Reescreve a parte inteira com `divisions = novo`. Só vale para parte com uma única
    /// declaração de `<divisions>` e sem elementos medidos em divisions fora de nota, `backup` e
    /// `forward` (`offset`, `figured-bass`): nesses casos é erro, não conserto às cegas.
    fn elevar_divisions(&mut self, p: usize, novo: i64) -> Result<(), ErroDocumento> {
        let parte = &self.partes[p];
        if parte.declaracoes_divisions != 1 {
            return Err(ErroDocumento::NaoSuportado(format!(
                "parte {} declara <divisions> {} vezes",
                parte.id, parte.declaracoes_divisions
            )));
        }
        let atual = self
            .divisions_da_parte(&parte.id)
            .ok_or_else(|| ErroDocumento::NaoSuportado("parte sem <divisions>".into()))?;
        let base = self.original.clone();
        let mut compassos = self.partes[p].compassos.clone();
        for c in &mut compassos {
            for i in 0..c.no.pecas.len() {
                let Peca::Elem(e) = &mut c.no.pecas[i] else {
                    continue;
                };
                match e.nome.as_str() {
                    "note" | "backup" | "forward" => {
                        let Some(d) = e.filho_mut("duration") else {
                            continue;
                        };
                        let texto = d.conteudo_folha(&base).unwrap_or_default();
                        let v: i64 = texto.trim().parse().map_err(|_| {
                            ErroDocumento::DuracaoInvalida(format!("<duration>{texto}</duration>"))
                        })?;
                        let escalado = i128::from(v) * i128::from(novo);
                        if escalado % i128::from(atual) != 0 {
                            return Err(ErroDocumento::DuracaoNaoRepresentavel {
                                valor: Fracao::nova(v, atual),
                                divisions: novo,
                                necessarias: mmc(novo, Fracao::nova(v, atual).denominador())?,
                            });
                        }
                        let v2 = i64::try_from(escalado / i128::from(atual))
                            .map_err(|_| ErroDocumento::Overflow(format!("{v} × {novo}")))?;
                        d.definir_conteudo(&base, &v2.to_string())?;
                    }
                    "attributes" => {
                        if let Some(d) = e.filho_mut("divisions") {
                            d.definir_conteudo(&base, &novo.to_string())?;
                        }
                    }
                    n => {
                        let t = e.texto(&base);
                        if n == "figured-bass" || t.contains("<offset") {
                            return Err(ErroDocumento::NaoSuportado(format!(
                                "<{n}> mede tempo em divisions: reescala não suportada"
                            )));
                        }
                    }
                }
            }
            c.sujo = true;
            for nota in &mut c.notas {
                nota.divisions = novo;
            }
        }
        self.partes[p].compassos = compassos;
        Ok(())
    }
}
