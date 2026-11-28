//! Acrescentar e retirar alturas de um acorde (ticket 18, E01, seam S1), sem janela e sem
//! teclado. A tela só traduz Shift+letra, Alt+número e a tecla de retirar em
//! [`ComandoDeEdicao::AcrescentarLetra`], [`ComandoDeEdicao::AcrescentarIntervalo`] e
//! [`ComandoDeEdicao::RetirarAltura`] e chama [`DocumentoAberto::aplicar`].
//!
//! # Contrato em linguagem de domínio
//!
//! - **Dois níveis**, como nos tickets 14 e 16: [`ComandoDeEdicao::AcrescentarLetra`] (a letra
//!   `A` a `G` na oitava **mais próxima** da nota selecionada; a alteração é a da armadura; se a
//!   mais próxima for a própria nota, vale a oitava acima, como no MuseScore) e
//!   [`ComandoDeEdicao::AcrescentarIntervalo`] (2 = segunda ... 9 = nona **acima** da nota
//!   selecionada, na escala da armadura) são **intenções**; o serviço as resolve em
//!   [`ComandoDeEdicao::AdicionarAltura`] (altura e acidente exatos), que é o `aplicado`.
//! - **A nova altura é uma nota de acorde** (`<chord/>`) com a **mesma posição, duração (figura,
//!   pontos, quiáltera), voz e pauta** do acorde, e soa junto com ele. A nota **principal** do
//!   acorde (a que não tem `<chord/>`) nunca muda de lugar: é ela que carrega letra, vigas,
//!   marcas de tupla e ligaduras, e é por ela que a identidade de exibição (letra, dedilhado)
//!   continua certa. A nota nova entra depois dela, em ordem crescente de altura em relação às
//!   que já estavam em ordem. Qualquer nota do acorde pode ser a selecionada, mas não pausa nem
//!   nota de adorno ([`ErroDeEdicao::AcordeSoEmNota`]).
//! - **Sem altura repetida.** Altura que já soa no acorde (mesmo número MIDI, mesmo grafada
//!   de outro jeito) é recusada com [`ErroDeEdicao::AlturaJaNoAcorde`]; nada muda, o histórico
//!   não ganha entrada. Os limites de altura valem como em todo comando
//!   ([`super::LimitesDeAltura`]: piano de 88 teclas, Lá0 a Dó8, por padrão).
//! - **Retirar altura** ([`ComandoDeEdicao::RetirarAltura`]) tira a nota do acorde e mantém as
//!   demais, com a mesma duração e posição (se era a principal, a seguinte passa a ser a
//!   principal, como na remoção do ticket 17: o caminho é o mesmo e não está duplicado). Quando a
//!   nota **não tem acorde** (uma só altura), o comando equivale à remoção do ticket 17: a nota
//!   vira pausa e o resultado é o de [`ComandoDeEdicao::RemoverNota`].
//! - **Nada se perde em silêncio.** Letra, dedilhado, ornamento, articulação e ligadura da nota
//!   retirada geram avisos (os do ticket 17). Ao **acrescentar** a um acorde que estava ligado à
//!   seguinte, o aviso [`AvisoDeEdicao::AlturaNovaSemLigadura`] diz que a altura nova entra sem
//!   ligadura; a ligadura e o dedilhado das alturas que já estavam ficam intactos (cada nota
//!   guarda os seus).
//! - **Identidade.** A altura nova ganha `IdNota` novo (monotônico, nunca reutilizado); as
//!   demais mantêm o seu. No resultado de acrescentar, `nota` é a nota **selecionada** (a seleção
//!   e o foco do desfazer ficam onde estavam) e [`MudancaDeEstrutura::alvo`] é a altura nova.
//! - **Atômico, um compasso, A5.** Tudo é calculado numa cópia do compasso; só o compasso da
//!   nota é sujo; o histórico guarda o compasso inteiro e desfazer volta ao original byte a byte.
//!   As vigas e os acidentes são conferidos como nos tickets 16 e 17 (avisos
//!   [`AvisoDeEdicao::VigasRefeitas`] e [`AvisoDeEdicao::AcidenteDeNotaSeguinteMudou`]).
//! - **Inverso.** O `inverso` de acrescentar é [`ComandoDeEdicao::RetirarAltura`] da nota nova.
//!   O de retirar de um acorde é [`ComandoDeEdicao::AdicionarAltura`] sobre outra nota do acorde
//!   (mesmo som; a ordem das alturas no arquivo pode diferir: quem devolve os bytes é o
//!   histórico).
//!
//! # Fora de escopo
//!
//! Acrescentar ao acorde de nota de adorno, intervalo **abaixo** e acrescentar várias alturas de
//! uma vez.

use super::comando::acidente_devido;
use super::comando::{AvisoDeEdicao, ComandoDeEdicao, ErroDeEdicao, ResultadoDaEdicao};
use super::edicao::{leaf, validar};
use super::editar_duracao::{
    Aplicacao, Planejado, derivar_e_vigar, grupo_do_acorde, inconsistente, inserir_nota_apos,
    montar_no, no_da_nota,
};
use super::erro::ErroDocumento;
use super::grafia::{alter_da_armadura, alter_do_acidente, indice_da_letra, midi, por_grau};
use super::inserir_remover::{
    alvo_por_indice, avisos_de_acidente, nova_nota, planejar_remocao, resultado_base,
};
use super::servico::DocumentoAberto;
use super::texto::{No, Peca, Texto};
use super::{Altura, Documento, IdNota};
use crate::partitura::Partitura;
use roxmltree::Node;

impl DocumentoAberto {
    /// Aplica um comando de acorde (sem registrar no histórico). Ver o contrato do módulo.
    pub(super) fn aplicar_acorde(
        &mut self,
        comando: &ComandoDeEdicao,
    ) -> Result<ResultadoDaEdicao, ErroDeEdicao> {
        let planejado = match comando {
            ComandoDeEdicao::AdicionarAltura {
                nota,
                altura,
                acidente,
            } => {
                self.conferir_altura_nova(*altura, acidente.as_deref())?;
                planejar_adicao(
                    &self.documento,
                    &self.partitura,
                    *nota,
                    *altura,
                    acidente.as_deref(),
                )?
            }
            ComandoDeEdicao::AcrescentarLetra { nota, letra } => {
                let altura = self.resolver_intencao(*nota, |base, quintas| {
                    por_letra_mais_proxima(base, *letra, quintas)
                })?;
                self.conferir_altura_nova(altura, None)?;
                planejar_adicao(&self.documento, &self.partitura, *nota, altura, None)?
            }
            ComandoDeEdicao::AcrescentarIntervalo { nota, graus } => {
                if !(2..=9).contains(graus) {
                    return Err(ErroDeEdicao::IntervaloInvalido(*graus));
                }
                let altura = self.resolver_intencao(*nota, |base, quintas| {
                    por_intervalo_acima(base, *graus, quintas)
                })?;
                self.conferir_altura_nova(altura, None)?;
                planejar_adicao(&self.documento, &self.partitura, *nota, altura, None)?
            }
            ComandoDeEdicao::RetirarAltura { nota } => {
                planejar_retirada(&self.documento, &self.partitura, *nota)?
            }
            _ => return Err(inconsistente("comando que não é de acorde")),
        };
        match planejado {
            Planejado::SemMudanca(r) => Ok(*r),
            Planejado::Muda(ap) => Ok(self.trocar_compasso(*ap)),
        }
    }

    /// Altura e acidente pedidos valem: letra `A` a `G`, oitava e alteração do MusicXML, limites
    /// de altura e acidente coerente com a alteração.
    fn conferir_altura_nova(
        &self,
        altura: Altura,
        acidente: Option<&str>,
    ) -> Result<(), ErroDeEdicao> {
        validar(altura)?;
        self.conferir_limites(altura)?;
        if let Some(t) = acidente
            && alter_do_acidente(t) != Some(altura.alter)
        {
            return Err(ErroDeEdicao::Documento(ErroDocumento::AlturaInvalida(
                format!(
                    "o acidente «{t}» não corresponde à alteração {}",
                    altura.alter
                ),
            )));
        }
        Ok(())
    }

    /// Resolve uma intenção (letra ou intervalo) numa altura, a partir da altura da nota
    /// selecionada e da armadura do compasso dela.
    fn resolver_intencao(
        &self,
        id: IdNota,
        resolver: impl FnOnce(Altura, i8) -> Result<Altura, ErroDeEdicao>,
    ) -> Result<Altura, ErroDeEdicao> {
        let &(pd, ci, ord) = self
            .documento
            .indice
            .get(&id)
            .ok_or(ErroDeEdicao::NotaInexistente(id))?;
        let a = alvo_por_indice(&self.documento, &self.partitura, pd, ci)?;
        let sel = &a.comp0.notas[ord];
        if sel.pausa || sel.graca {
            return Err(ErroDeEdicao::AcordeSoEmNota(id));
        }
        let base = sel.altura.ok_or(ErroDeEdicao::NotaSemAltura(id))?;
        resolver(base, a.info.armadura.quintas)
    }
}

/// A letra na oitava mais próxima de `base` (empate: a de cima). Se a mais próxima é a própria
/// altura de `base`, a oitava acima. A alteração é a que a armadura dá à letra.
fn por_letra_mais_proxima(base: Altura, letra: char, quintas: i8) -> Result<Altura, ErroDeEdicao> {
    let letra = letra.to_ascii_uppercase();
    let invalida = || {
        ErroDeEdicao::Documento(ErroDocumento::AlturaInvalida(format!(
            "a letra '{letra}' não é de A a G"
        )))
    };
    indice_da_letra(letra).ok_or_else(invalida)?;
    let alter = alter_da_armadura(quintas, letra);
    let m0 = midi(base).ok_or_else(invalida)?;
    let com_oitava = |oitava: i8| Altura {
        passo: letra,
        alter,
        oitava,
    };
    let mut melhor: Option<(i32, i32, Altura)> = None; // (distância, -midi para preferir cima)
    for delta in [-1i8, 0, 1] {
        let Some(oitava) = base.oitava.checked_add(delta) else {
            continue;
        };
        let cand = com_oitava(oitava);
        let Some(m) = midi(cand) else { continue };
        let chave = ((m - m0).abs(), -m);
        if melhor.is_none_or(|(d, n, _)| chave < (d, n)) {
            melhor = Some((chave.0, chave.1, cand));
        }
    }
    let (_, _, escolhida) = melhor.ok_or_else(invalida)?;
    if midi(escolhida) == Some(m0) {
        let oitava = base.oitava.checked_add(1).ok_or_else(invalida)?;
        return Ok(com_oitava(oitava));
    }
    Ok(escolhida)
}

/// `graus` acima de `base` (2 = segunda ... 9 = nona), grau a grau na escala da armadura.
fn por_intervalo_acima(base: Altura, graus: u8, quintas: i8) -> Result<Altura, ErroDeEdicao> {
    let mut atual = base;
    for _ in 1..graus {
        atual = por_grau(atual, true, quintas).ok_or_else(|| {
            ErroDeEdicao::Documento(ErroDocumento::AlturaInvalida(format!(
                "passo '{}'",
                atual.passo
            )))
        })?;
    }
    Ok(atual)
}

/// A nota tem uma ligadura de prolongamento que **começa** nela (`<tie type="start">`)?
fn comeca_ligadura_de_valor(texto_da_nota: &str) -> Result<bool, ErroDeEdicao> {
    let doc = crate::xml_seguro::analisar(texto_da_nota)
        .map_err(|e| ErroDeEdicao::Documento(ErroDocumento::from(e)))?;
    let comeca = |n: Node<'_, '_>| n.attribute("type") == Some("start");
    Ok(doc
        .root_element()
        .children()
        .filter(Node::is_element)
        .any(|c| match c.tag_name().name() {
            "tie" => comeca(c),
            "notations" => c
                .children()
                .filter(Node::is_element)
                .any(|n| n.tag_name().name() == "tied" && comeca(n)),
            _ => false,
        }))
}

/// `<note>` novo de acorde: `<chord/>`, `<pitch>`, e a duração, voz, figura, pontos, quiáltera,
/// haste e pauta copiados da nota principal, na ordem do esquema.
fn no_de_nota_de_acorde(principal: &No, base: &str, altura: Altura, acidente: Option<&str>) -> No {
    let mut pitch = vec![leaf("step", &altura.passo.to_string())];
    if altura.alter != 0 {
        pitch.push(leaf("alter", &altura.alter.to_string()));
    }
    pitch.push(leaf("octave", &altura.oitava.to_string()));
    let mut pecas = vec![Peca::Txt(Texto::Novo("<pitch>".to_owned()))];
    for f in pitch {
        pecas.push(Peca::Txt(Texto::Novo(String::new())));
        pecas.push(Peca::Elem(f));
    }
    pecas.push(Peca::Txt(Texto::Novo("</pitch>".to_owned())));
    let no_pitch = No {
        nome: "pitch".to_owned(),
        pecas,
    };

    let mut filhos: Vec<No> = Vec::new();
    if let Some(c) = principal.filho("cue") {
        filhos.push(c.clone());
    }
    filhos.push(No::novo("chord", "<chord/>".to_owned()));
    filhos.push(no_pitch);
    for nome in ["duration", "voice", "type"] {
        if let Some(n) = principal.filho(nome) {
            filhos.push(n.clone());
        }
    }
    for (_, n) in principal.filhos().filter(|(_, n)| n.nome == "dot") {
        filhos.push(n.clone());
    }
    if let Some(t) = acidente {
        filhos.push(leaf("accidental", t));
    }
    for nome in ["time-modification", "stem", "staff"] {
        if let Some(n) = principal.filho(nome) {
            filhos.push(n.clone());
        }
    }
    montar_no(principal, base, filhos)
}

fn planejar_adicao(
    doc: &Documento,
    partitura: &Partitura,
    id: IdNota,
    altura: Altura,
    acidente_pedido: Option<&str>,
) -> Result<Planejado, ErroDeEdicao> {
    let &(pd, ci, ord) = doc
        .indice
        .get(&id)
        .ok_or(ErroDeEdicao::NotaInexistente(id))?;
    let a = alvo_por_indice(doc, partitura, pd, ci)?;
    let base = doc.original.as_str();
    let sel = &a.comp0.notas[ord];
    if sel.pausa || sel.graca {
        return Err(ErroDeEdicao::AcordeSoEmNota(id));
    }
    if sel.altura.is_none() {
        return Err(ErroDeEdicao::NotaSemAltura(id));
    }
    let (p0, q) = grupo_do_acorde(a.comp0, ord);
    let m_novo = midi(altura).ok_or_else(|| {
        ErroDeEdicao::Documento(ErroDocumento::AlturaInvalida(format!(
            "passo '{}'",
            altura.passo
        )))
    })?;
    for k in p0..=q {
        let n = &a.comp0.notas[k];
        if n.altura.and_then(midi) == Some(m_novo) {
            return Err(ErroDeEdicao::AlturaJaNoAcorde { nota: n.id, altura });
        }
    }

    let principal = &a.comp0.notas[p0];
    let no_principal = no_da_nota(a.comp0, p0)?;
    let tocar_principal = &a.tocar0.notas[p0];
    let pauta_n = tocar_principal.pauta;
    let acidente = match acidente_pedido {
        Some(t) => Some(t.to_owned()),
        None => acidente_devido(
            a.tocar0,
            a.info.armadura.quintas,
            altura,
            pauta_n,
            tocar_principal.posicao,
        ),
    };
    let ligado = {
        let mut ligado = false;
        for k in p0..=q {
            ligado |= comeca_ligadura_de_valor(&no_da_nota(a.comp0, k)?.texto(base))?;
        }
        ligado
    };

    // --- Edita uma cópia do compasso ---------------------------------------------------------
    let mut comp = a.comp0.clone();
    let id_novo = IdNota(doc.ids.primeiro_livre()?);
    // Depois da principal, e depois das notas do acorde que já estavam em ordem crescente.
    let mut ref_ord = p0;
    for k in p0 + 1..=q {
        match a.comp0.notas[k].altura.and_then(midi) {
            Some(m) if m <= m_novo => ref_ord = k,
            _ => break,
        }
    }
    let no_novo = no_de_nota_de_acorde(no_principal, base, altura, acidente.as_deref());
    let mut nota = nova_nota(
        id_novo,
        Some(altura),
        0,
        principal.divisions,
        (&principal.voz, &principal.pauta),
    );
    nota.acorde = true;
    nota.duracao = principal.duracao;
    inserir_nota_apos(&mut comp, ref_ord, no_novo, nota);
    comp.sujo = true;

    let (tocar, avisos_vigas) = derivar_e_vigar(
        &mut comp,
        base,
        principal.divisions,
        a.info,
        a.tocar0,
        (tocar_principal.pauta, tocar_principal.voz),
        &a.pos,
    )?;

    // --- Resultado ---------------------------------------------------------------------------
    let aplicado = ComandoDeEdicao::AdicionarAltura {
        nota: id,
        altura,
        acidente: acidente.clone(),
    };
    let mut r = resultado_base(aplicado, id_novo, altura, true);
    r.nota = id;
    r.inverso = ComandoDeEdicao::RetirarAltura { nota: id_novo };
    if ligado {
        r.avisos
            .push(AvisoDeEdicao::AlturaNovaSemLigadura { nota: id_novo });
    }
    r.avisos.extend(avisos_vigas);
    r.avisos
        .extend(avisos_de_acidente(&a, &comp, &tocar, Some(id_novo)));
    let ids = comp.notas.iter().map(|n| n.id).collect();
    Ok(Planejado::Muda(Box::new(Aplicacao {
        pd: a.pd,
        ci: a.ci,
        pt: a.pt,
        comp,
        tocar,
        ids,
        novos: 1,
        resultado: Box::new(r),
    })))
}

/// Retirar a altura de `id`: a mesma remoção do ticket 17 (nota de acorde sai; nota sozinha vira
/// pausa). Num acorde, o resultado diz `RetirarAltura` e o inverso devolve a altura.
fn planejar_retirada(
    doc: &Documento,
    partitura: &Partitura,
    id: IdNota,
) -> Result<Planejado, ErroDeEdicao> {
    let &(pd, ci, ord) = doc
        .indice
        .get(&id)
        .ok_or(ErroDeEdicao::NotaInexistente(id))?;
    let a = alvo_por_indice(doc, partitura, pd, ci)?;
    let sel = &a.comp0.notas[ord];
    let (p0, q) = grupo_do_acorde(a.comp0, ord);
    let em_acorde = !sel.pausa && !sel.graca && q > p0;
    let restante = if ord > p0 { p0 } else { ord + 1 };
    let (altura, restante_id) = (sel.altura, a.comp0.notas[restante.min(q)].id);
    let acidente = doc.acidente_da_nota(id).ok().flatten();
    let planejado = planejar_remocao(doc, partitura, id)?;
    match planejado {
        Planejado::Muda(mut ap) if em_acorde => {
            ap.resultado.aplicado = ComandoDeEdicao::RetirarAltura { nota: id };
            if let Some(altura) = altura {
                ap.resultado.inverso = ComandoDeEdicao::AdicionarAltura {
                    nota: restante_id,
                    altura,
                    acidente,
                };
            }
            Ok(Planejado::Muda(ap))
        }
        outro => Ok(outro),
    }
}
