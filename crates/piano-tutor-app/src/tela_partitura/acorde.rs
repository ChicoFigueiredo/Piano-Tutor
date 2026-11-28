//! Acrescentar e retirar alturas de um acorde na tela Partitura (ticket 18, E01). A tecla só
//! traduz o atalho em [`PedidoDeAcorde`]; quem edita é o comando tipado do serviço de documento
//! (`piano_tutor_core::documento`, módulo `acorde`), e o resultado é o do comando chamado direto
//! (critério A12). Fica num módulo próprio para não disputar linhas com o resto da tela.

use super::*;
use crate::atalhos_partitura::PedidoDeAcorde;

impl EstadoPartitura {
    /// Acrescenta uma altura ao acorde da nota selecionada, ou a retira (ticket 18). Recusa, com
    /// mensagem, sem nota selecionada, em modo leitura e durante a reprodução ou a prática. A
    /// seleção segue na mesma nota (ao retirar a selecionada, passa para outra do acorde).
    pub fn editar_acorde(&mut self, pedido: PedidoDeAcorde) -> Option<ResultadoDaEdicao> {
        let r = self.aplicar_pedido_de_acorde(pedido);
        self.conferir_originais();
        r
    }

    fn aplicar_pedido_de_acorde(&mut self, pedido: PedidoDeAcorde) -> Option<ResultadoDaEdicao> {
        self.documento.as_ref()?;
        if self.recusar_em_leitura() {
            return None;
        }
        let Some(nota) = self.selecao.nota() else {
            self.ultima_edicao = Some(
                "Selecione uma nota (clique nela ou use as setas) antes de mexer no acorde.".into(),
            );
            return None;
        };
        if self.tocando() || self.praticando() {
            self.ultima_edicao =
                Some("Pare a reprodução ou a prática antes de editar a partitura.".into());
            return None;
        }
        let comando = match pedido {
            PedidoDeAcorde::Letra(letra) => ComandoDeEdicao::AcrescentarLetra { nota, letra },
            PedidoDeAcorde::Intervalo(graus) => {
                ComandoDeEdicao::AcrescentarIntervalo { nota, graus }
            }
            PedidoDeAcorde::Retirar => ComandoDeEdicao::RetirarAltura { nota },
        };
        let d = self.documento.as_mut()?;
        match d.aplicar(comando) {
            Ok(r) => {
                let mut texto = match &r.aplicado {
                    ComandoDeEdicao::AdicionarAltura { altura, .. } => {
                        format!("Acorde: acrescentada a altura {}.", nome_da_altura(*altura))
                    }
                    ComandoDeEdicao::RetirarAltura { .. } => {
                        "Acorde: altura retirada; as demais continuam soando.".to_owned()
                    }
                    _ => "Nota removida: virou pausa.".to_owned(),
                };
                if r.alterou {
                    // Ao retirar a nota selecionada, a seleção passa para outra do acorde (o
                    // inverso diz qual); senão, `reancorar` a solta com o aviso de sempre.
                    if let ComandoDeEdicao::AdicionarAltura { nota: outra, .. } = &r.inverso
                        && matches!(r.aplicado, ComandoDeEdicao::RetirarAltura { .. })
                    {
                        self.selecao.selecionar(&d.leitura, *outra);
                    }
                    self.selecao.reancorar(&d.leitura);
                    self.selecao.pedir_rolagem();
                    if let Some(c) = self
                        .selecao
                        .nota()
                        .and_then(|n| d.leitura.entrada(n))
                        .map(|e| e.compasso)
                    {
                        self.compasso_em_foco = Some(c);
                    }
                    // O que a prática e o desenho guardam é da música antiga.
                    self.pratica = None;
                    self.visual.invalidar();
                    self.audicao_pendente = r.midi_depois();
                }
                for a in &r.avisos {
                    texto.push(' ');
                    texto.push_str(&a.texto());
                }
                self.ultima_edicao = Some(texto);
                Some(r)
            }
            Err(e) => {
                self.ultima_edicao = Some(e.to_string());
                None
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use piano_tutor_core::documento::DocumentoAberto;

    /// Dó4 com letra e dedilhado, e logo depois um acorde Ré4-Fá4 com dedilhado.
    const COM_ACORDE: &str = r#"<score-partwise><part-list><score-part id="P1"><part-name>x</part-name></score-part></part-list>
<part id="P1"><measure number="1"><attributes><divisions>1</divisions><key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time></attributes>
<note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type><notations><technical><fingering>1</fingering></technical></notations><lyric number="1"><syllabic>single</syllabic><text>la</text></lyric></note>
<note><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type><notations><technical><fingering>2</fingering></technical></notations></note>
<note><chord/><pitch><step>F</step><octave>4</octave></pitch><duration>2</duration><voice>1</voice><type>half</type><notations><technical><fingering>4</fingering></technical></notations></note>
</measure></part></score-partwise>"#;

    fn estado() -> EstadoPartitura {
        let mut e = EstadoPartitura::novo(None);
        e.carregar_texto("acorde".into(), COM_ACORDE.into())
            .expect("abre");
        e
    }

    fn texto(e: &EstadoPartitura) -> String {
        e.documento
            .as_ref()
            .expect("doc")
            .exportado()
            .expect("exporta")
    }

    #[test]
    fn shift_letra_acrescenta_o_mesmo_que_o_comando_do_servico_e_preserva_a_exibicao() {
        let mut e = estado();
        let antes = e.exibicao().cloned().expect("exibição");
        e.navegar(Comando::Seguinte);
        let c = e.nota_selecionada().expect("selecionada");
        let destino = e.atalhos.rotear(Key::E, Modifiers::SHIFT, true);
        let Some(Destino::Acorde(pedido)) = destino else {
            panic!("Shift+E deve ser de acorde: {destino:?}");
        };
        let r = e.editar_acorde(pedido).expect("acrescenta");
        assert!(r.alterou);
        assert!(
            e.ultima_edicao().expect("msg").contains("Mi 4"),
            "{:?}",
            e.ultima_edicao()
        );
        assert_eq!(e.nota_selecionada(), Some(c), "a seleção fica na nota");
        assert_eq!(e.audicao_pendente, Some(64));
        // A tecla é o comando do serviço chamado direto.
        let mut direto = DocumentoAberto::abrir_texto("acorde", COM_ACORDE).expect("abre");
        direto
            .aplicar(ComandoDeEdicao::AcrescentarLetra {
                nota: c,
                letra: 'E',
            })
            .expect("direto");
        assert_eq!(texto(&e), direto.exportar().expect("exporta"));
        // Letra e dedilhado seguem nas mesmas notas (por IdNota); a altura nova não tem.
        let ex = e.exibicao().expect("exibição");
        for (id, elementos) in antes.notas() {
            assert_eq!(ex.da_nota(id), Some(elementos));
        }
        let novo = r.estrutura.as_ref().expect("estrutura").alvo;
        assert!(ex.da_nota(novo).is_none());
        assert!(e.documento.as_ref().expect("doc").editado());
        // Desfazer devolve o arquivo original, byte a byte.
        assert!(e.desfazer());
        assert_eq!(texto(&e), COM_ACORDE);
    }

    #[test]
    fn alt_numero_acrescenta_o_intervalo_e_altura_repetida_e_recusada_com_mensagem() {
        let mut e = estado();
        e.navegar(Comando::Seguinte);
        let alt = Modifiers {
            alt: true,
            ..Modifiers::NONE
        };
        let Some(Destino::Acorde(p)) = e.atalhos.rotear(Key::Num3, alt, true) else {
            panic!("Alt+3 deve ser de acorde");
        };
        let r = e.editar_acorde(p).expect("terça acima de Dó");
        assert_eq!(r.depois.passo, 'E');
        let antes = texto(&e);
        // De novo: Mi4 já está no acorde.
        assert!(e.editar_acorde(p).is_none());
        assert!(
            e.ultima_edicao()
                .expect("msg")
                .contains("já está no acorde")
        );
        assert_eq!(texto(&e), antes);
    }

    #[test]
    fn retirar_a_altura_passa_a_selecao_para_outra_do_acorde_e_a_ultima_vira_pausa() {
        let mut e = estado();
        e.navegar(Comando::Seguinte);
        e.navegar(Comando::Seguinte);
        let re = e.nota_selecionada().expect("Ré");
        let r = e.editar_acorde(PedidoDeAcorde::Retirar).expect("retira");
        assert_eq!(r.aplicado, ComandoDeEdicao::RetirarAltura { nota: re });
        assert!(e.ultima_edicao().expect("msg").contains("altura retirada"));
        let fa = e.nota_selecionada().expect("a seleção passou para o Fá");
        assert_ne!(fa, re);
        // A última altura: a nota vira pausa, como no Delete.
        let r = e
            .editar_acorde(PedidoDeAcorde::Retirar)
            .expect("retira a última");
        assert_eq!(r.aplicado, ComandoDeEdicao::RemoverNota { nota: fa });
        assert!(e.ultima_edicao().expect("msg").contains("virou pausa"));
        assert!(e.desfazer());
        assert!(e.desfazer());
        assert_eq!(texto(&e), COM_ACORDE);
    }

    #[test]
    fn sem_selecao_recusa_com_mensagem_e_nada_muda() {
        let mut e = estado();
        assert!(e.editar_acorde(PedidoDeAcorde::Letra('E')).is_none());
        assert!(
            e.ultima_edicao()
                .expect("msg")
                .contains("Selecione uma nota")
        );
        assert_eq!(texto(&e), COM_ACORDE);
        assert!(!e.documento.as_ref().expect("doc").editado());
    }
}
