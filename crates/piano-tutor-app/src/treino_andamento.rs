//! Treino com andamento: uma lista de acordes (ou notas) que avança com o metrônomo, um item
//! a cada N compassos. O app pode tocar cada item no tempo ou só mostrar; o aluno toca e o
//! treino confere pelas teclas seguradas (MIDI ou simulação), aceitando qualquer inversão e
//! dobra de oitava. Serve à tela Acordes e escalas e aos exercícios com `andamento` nos
//! arquivos de Lição.
//!
//! O tempo vem do [`EstadoRitmo`] compartilhado (metrônomo e ritmos de fábrica): o item da vez
//! é calculado do compasso em que o ritmo está, então mudar o BPM no meio não desalinha nada.

use crate::ritmo::EstadoRitmo;
use crate::som::ServicoSom;
use piano_tutor_core::NotaMidi;
use std::time::Duration;

/// Um item do treino (um acorde, um intervalo ou uma nota).
#[derive(Clone, Debug, PartialEq)]
pub struct ItemTreino {
    /// Notas para tocar e mostrar no teclado.
    pub notas: Vec<NotaMidi>,
    /// Texto grande (cifra ou legenda).
    pub rotulo: String,
    /// Texto menor (nome por extenso, grau).
    pub detalhe: String,
}

impl ItemTreino {
    pub fn novo(
        notas: Vec<NotaMidi>,
        rotulo: impl Into<String>,
        detalhe: impl Into<String>,
    ) -> Self {
        Self {
            notas,
            rotulo: rotulo.into(),
            detalhe: detalhe.into(),
        }
    }

    fn mascara(&self) -> u16 {
        mascara(&self.notas)
    }
}

fn mascara(notas: &[NotaMidi]) -> u16 {
    notas.iter().fold(0u16, |m, n| m | (1 << (n.numero() % 12)))
}

/// Ordem dos itens.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ordem {
    Sequencial,
    Aleatoria,
}

impl Ordem {
    pub const fn texto(self) -> &'static str {
        match self {
            Ordem::Sequencial => "em sequência",
            Ordem::Aleatoria => "aleatória",
        }
    }
}

/// Ajustes do treino.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConfigTreino {
    pub ordem: Ordem,
    /// O app toca cada item no tempo (senão só mostra).
    pub app_toca: bool,
    /// Compassos que cada item dura (1 a 4).
    pub compassos_por_item: u8,
    /// Compassos de preparação antes do primeiro item.
    pub contagem: u8,
    /// Voltas pela lista; `None` = sem fim (até Parar).
    pub voltas: Option<u32>,
    /// Mostrar as teclas do item no teclado.
    pub mostrar_teclas: bool,
    /// Atraso máximo (ms) para contar "no tempo".
    pub tolerancia_ms: u32,
}

impl Default for ConfigTreino {
    fn default() -> Self {
        Self {
            ordem: Ordem::Sequencial,
            app_toca: true,
            compassos_por_item: 1,
            contagem: 1,
            voltas: None,
            mostrar_teclas: true,
            tolerancia_ms: 250,
        }
    }
}

/// Resultado de um item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Julgamento {
    /// Tocado; atraso em ms do início do item (negativo = antecipou).
    Acertou { atraso_ms: i64 },
    /// O tempo do item acabou sem o acorde certo.
    Errou,
}

/// Contagem do treino.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Placar {
    pub no_tempo: u32,
    pub atrasados: u32,
    pub errados: u32,
}

impl Placar {
    pub fn total(&self) -> u32 {
        self.no_tempo + self.atrasados + self.errados
    }

    /// Aproveitamento (acertos no tempo ou atrasados) em %.
    pub fn aproveitamento(&self) -> Option<u32> {
        let t = self.total();
        (t > 0).then(|| (self.no_tempo + self.atrasados) * 100 / t)
    }
}

/// Onde o treino está agora.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Momento {
    Parado,
    /// Preparação: pulso (1-based) e total de pulsos que faltam.
    Contagem {
        pulso: u64,
        pulsos: u64,
    },
    /// Item `vaga` da execução (0-based).
    Item {
        vaga: usize,
    },
    Terminou,
}

/// Tempo do ritmo, visto pelo treino (separado para testar sem áudio).
pub trait Tempo {
    /// Compasso e pulso atuais desde o início do ritmo; `None` se parado ou antes do 1º tique.
    fn posicao(&self, agora_ns: u64) -> Option<(u64, u64)>;
    fn instante_do_compasso(&self, c: u64) -> Option<u64>;
    fn pulso_ns(&self) -> Option<u64>;
    fn pulsos_por_compasso(&self) -> u64;
}

impl Tempo for EstadoRitmo {
    fn posicao(&self, agora_ns: u64) -> Option<(u64, u64)> {
        EstadoRitmo::posicao(self, agora_ns).map(|p| (p.compasso, p.pulso))
    }
    fn instante_do_compasso(&self, c: u64) -> Option<u64> {
        EstadoRitmo::instante_do_compasso(self, c)
    }
    fn pulso_ns(&self) -> Option<u64> {
        EstadoRitmo::pulso_ns(self)
    }
    fn pulsos_por_compasso(&self) -> u64 {
        self.padrao.formula.pulsos()
    }
}

/// Uma demonstração a agendar: notas, início e duração.
#[derive(Clone, Debug, PartialEq)]
pub struct Demo {
    pub notas: Vec<NotaMidi>,
    pub inicio_ns: u64,
    pub duracao: Duration,
}

/// O treino em si.
#[derive(Clone, Debug)]
pub struct TreinoAndamento {
    pub config: ConfigTreino,
    itens: Vec<ItemTreino>,
    /// Índices em `itens`, na ordem da execução (cresce sob demanda na aleatória).
    fila: Vec<usize>,
    /// Resultado de cada vaga da fila.
    resultados: Vec<Option<Julgamento>>,
    /// Compasso do ritmo em que a execução começou (início da contagem).
    inicio_compasso: Option<u64>,
    /// Vagas cuja demonstração já foi agendada.
    demos_ate: usize,
    momento: Momento,
    /// O treino ligou o ritmo (então desliga ao terminar).
    ligou_ritmo: bool,
    pub placar: Placar,
    /// Último julgamento, para o retorno na tela.
    pub ultimo: Option<(usize, Julgamento)>,
    semente: u64,
}

impl Default for TreinoAndamento {
    fn default() -> Self {
        Self::novo(Vec::new(), ConfigTreino::default())
    }
}

/// Antecedência com que a demonstração de um item é agendada.
const ANTECEDENCIA_DEMO_NS: u64 = 250_000_000;

impl TreinoAndamento {
    pub fn novo(itens: Vec<ItemTreino>, config: ConfigTreino) -> Self {
        Self {
            config,
            itens,
            fila: Vec::new(),
            resultados: Vec::new(),
            inicio_compasso: None,
            demos_ate: 0,
            momento: Momento::Parado,
            ligou_ritmo: false,
            placar: Placar::default(),
            ultimo: None,
            semente: 0x2545_F491_4F6C_DD1D,
        }
    }

    pub fn itens(&self) -> &[ItemTreino] {
        &self.itens
    }

    /// Troca a lista (só parado; tocando, vale na próxima execução).
    pub fn definir_itens(&mut self, itens: Vec<ItemTreino>) {
        if !self.ativo() {
            self.itens = itens;
        }
    }

    pub fn semear(&mut self, s: u64) {
        self.semente ^= s | 1;
    }

    pub fn ativo(&self) -> bool {
        matches!(
            self.momento,
            Momento::Contagem { .. } | Momento::Item { .. }
        )
    }

    pub fn momento(&self) -> Momento {
        self.momento
    }

    /// Total de vagas da execução (`None` = sem fim).
    pub fn total_de_vagas(&self) -> Option<usize> {
        self.config
            .voltas
            .map(|v| self.itens.len() * v.max(1) as usize)
    }

    fn aleatorio(&mut self) -> u64 {
        let mut x = self.semente;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.semente = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Garante a fila até a vaga `v` (inclusive).
    fn garantir(&mut self, v: usize) {
        let n = self.itens.len();
        if n == 0 {
            return;
        }
        while self.fila.len() <= v {
            let i = match self.config.ordem {
                Ordem::Sequencial => self.fila.len() % n,
                Ordem::Aleatoria => {
                    let mut i = (self.aleatorio() % n as u64) as usize;
                    if n > 1 && self.fila.last() == Some(&i) {
                        i = (i + 1 + (self.aleatorio() % (n as u64 - 1)) as usize) % n;
                    }
                    i
                }
            };
            self.fila.push(i);
            self.resultados.push(None);
        }
    }

    /// Item da vaga `v` (gera a fila se preciso).
    pub fn item_da_vaga(&mut self, v: usize) -> Option<&ItemTreino> {
        if self.total_de_vagas().is_some_and(|t| v >= t) {
            return None;
        }
        self.garantir(v);
        self.fila.get(v).and_then(|i| self.itens.get(*i))
    }

    /// Item da vez (durante a contagem, o primeiro que vem).
    pub fn atual(&self) -> Option<&ItemTreino> {
        match self.momento {
            Momento::Item { vaga } => self.fila.get(vaga).and_then(|i| self.itens.get(*i)),
            Momento::Contagem { .. } => self.fila.first().and_then(|i| self.itens.get(*i)),
            _ => None,
        }
    }

    /// Próximo item (depois do atual).
    pub fn proximo(&self) -> Option<&ItemTreino> {
        let v = match self.momento {
            Momento::Item { vaga } => vaga + 1,
            Momento::Contagem { .. } => 1,
            _ => return None,
        };
        if self.total_de_vagas().is_some_and(|t| v >= t) {
            return None;
        }
        self.fila.get(v).and_then(|i| self.itens.get(*i))
    }

    pub fn resultado(&self, vaga: usize) -> Option<Julgamento> {
        self.resultados.get(vaga).copied().flatten()
    }

    /// Teclas-guia agora.
    pub fn guias(&self) -> Vec<NotaMidi> {
        if !self.config.mostrar_teclas {
            return Vec::new();
        }
        self.atual().map(|i| i.notas.clone()).unwrap_or_default()
    }

    /// Começa a execução no compasso `compasso` do ritmo (a contagem começa nele).
    pub fn comecar_no_compasso(&mut self, compasso: u64) {
        self.fila.clear();
        self.resultados.clear();
        self.demos_ate = 0;
        self.placar = Placar::default();
        self.ultimo = None;
        self.inicio_compasso = Some(compasso);
        self.garantir(1);
        self.momento = if self.itens.is_empty() {
            Momento::Terminou
        } else {
            Momento::Contagem {
                pulso: 0,
                pulsos: 0,
            }
        };
    }

    /// Play: liga o ritmo se estiver parado (e começa no compasso 0), ou entra no próximo
    /// compasso do ritmo que já toca.
    pub fn tocar(&mut self, ritmo: &mut EstadoRitmo, som: &mut ServicoSom) {
        if ritmo.tocando() {
            let c = Tempo::posicao(ritmo, som.agora_ns()).map_or(0, |(c, _)| c + 1);
            self.ligou_ritmo = false;
            self.comecar_no_compasso(c);
        } else {
            ritmo.tocar(som);
            self.ligou_ritmo = true;
            self.comecar_no_compasso(0);
        }
    }

    /// Stop: encerra e, se foi o treino que ligou o ritmo, para o ritmo.
    pub fn parar(&mut self, ritmo: &mut EstadoRitmo, som: &mut ServicoSom) {
        let ligou = std::mem::take(&mut self.ligou_ritmo);
        if self.ativo() {
            self.momento = Momento::Parado;
        }
        if ligou && ritmo.tocando() {
            ritmo.parar_e_silenciar(som);
        }
    }

    /// Chamar a cada quadro (tick).
    pub fn atualizar(
        &mut self,
        ritmo: &mut EstadoRitmo,
        som: &mut ServicoSom,
        seguradas: &[NotaMidi],
    ) {
        if !self.ativo() {
            return;
        }
        if !ritmo.tocando() {
            // Parado pela tela Ritmos ou outro lugar.
            self.momento = Momento::Parado;
            self.ligou_ritmo = false;
            return;
        }
        let demos = self.avancar(&*ritmo, som.agora_ns(), seguradas);
        for d in demos {
            som.demonstrar(&d.notas, d.inicio_ns, d.duracao);
        }
        if self.momento == Momento::Terminou && std::mem::take(&mut self.ligou_ritmo) {
            ritmo.parar_e_silenciar(som);
        }
    }

    /// Lógica pura de um quadro: atualiza momento e resultados; devolve demonstrações novas.
    pub fn avancar(
        &mut self,
        tempo: &dyn Tempo,
        agora_ns: u64,
        seguradas: &[NotaMidi],
    ) -> Vec<Demo> {
        let mut demos = Vec::new();
        let Some(c0) = self.inicio_compasso else {
            return demos;
        };
        if !self.ativo() {
            return demos;
        }
        let ppc = tempo.pulsos_por_compasso().max(1);
        let cpi = u64::from(self.config.compassos_por_item.max(1));
        let contagem = u64::from(self.config.contagem);
        let primeiro = c0 + contagem;
        let pulso_ns = tempo.pulso_ns().unwrap_or(500_000_000);
        let total = self.total_de_vagas();

        // Demonstrações agendadas um pouco antes de cada item (inclusive o primeiro, ainda
        // na contagem).
        if self.config.app_toca {
            loop {
                let v = self.demos_ate;
                if total.is_some_and(|t| v >= t) {
                    break;
                }
                let Some(ini) = tempo.instante_do_compasso(primeiro + v as u64 * cpi) else {
                    break;
                };
                if ini > agora_ns.saturating_add(ANTECEDENCIA_DEMO_NS) {
                    break;
                }
                let Some(fim) = tempo.instante_do_compasso(primeiro + (v as u64 + 1) * cpi) else {
                    break;
                };
                if let Some(item) = self.item_da_vaga(v).cloned() {
                    let dur = fim
                        .saturating_sub(ini)
                        .saturating_sub(60_000_000)
                        .max(80_000_000);
                    demos.push(Demo {
                        notas: item.notas,
                        inicio_ns: ini,
                        duracao: Duration::from_nanos(dur),
                    });
                }
                self.demos_ate += 1;
            }
        }

        let Some((compasso, pulso)) = tempo.posicao(agora_ns) else {
            return demos;
        };
        if compasso < c0 {
            return demos;
        }
        if compasso < primeiro {
            let feitos = (compasso - c0) * ppc + pulso;
            self.momento = Momento::Contagem {
                pulso: feitos + 1,
                pulsos: contagem * ppc,
            };
            self.tentar_antecipar(0, tempo, primeiro, cpi, pulso_ns, agora_ns, seguradas);
            return demos;
        }
        let vaga = ((compasso - primeiro) / cpi) as usize;
        // Fecha vagas anteriores sem acerto (mesmo as que passaram sem quadro).
        let fechar_ate = total.map_or(vaga, |t| vaga.min(t));
        if fechar_ate > 0 {
            self.garantir(fechar_ate - 1);
        }
        for v in 0..fechar_ate {
            if self.resultados[v].is_none() {
                self.resultados[v] = Some(Julgamento::Errou);
                self.placar.errados += 1;
                self.ultimo = Some((v, Julgamento::Errou));
            }
        }
        if total.is_some_and(|t| vaga >= t) {
            self.momento = Momento::Terminou;
            return demos;
        }
        self.garantir(vaga + 1);
        self.momento = Momento::Item { vaga };
        // Confere a vaga atual.
        if self.resultados[vaga].is_none()
            && let Some(item) = self.itens.get(self.fila[vaga])
            && confere(item, seguradas)
        {
            let ini = tempo
                .instante_do_compasso(primeiro + vaga as u64 * cpi)
                .unwrap_or(agora_ns);
            let atraso_ms = (agora_ns as i64 - ini as i64) / 1_000_000;
            self.julgar(vaga, atraso_ms);
        }
        // Antecipação da próxima: na última metade de pulso do item.
        let fim_item = tempo.instante_do_compasso(primeiro + (vaga as u64 + 1) * cpi);
        if fim_item.is_some_and(|f| agora_ns + pulso_ns / 2 >= f) {
            self.tentar_antecipar(
                vaga + 1,
                tempo,
                primeiro,
                cpi,
                pulso_ns,
                agora_ns,
                seguradas,
            );
        }
        demos
    }

    #[allow(clippy::too_many_arguments)]
    fn tentar_antecipar(
        &mut self,
        vaga: usize,
        tempo: &dyn Tempo,
        primeiro: u64,
        cpi: u64,
        pulso_ns: u64,
        agora_ns: u64,
        seguradas: &[NotaMidi],
    ) {
        if self.total_de_vagas().is_some_and(|t| vaga >= t) {
            return;
        }
        let Some(ini) = tempo.instante_do_compasso(primeiro + vaga as u64 * cpi) else {
            return;
        };
        if agora_ns + pulso_ns / 2 < ini {
            return;
        }
        // Na vaga anterior, o mesmo acorde segurado não vale como antecipação.
        let anterior = vaga
            .checked_sub(1)
            .and_then(|a| self.fila.get(a))
            .and_then(|i| self.itens.get(*i))
            .map(ItemTreino::mascara);
        self.garantir(vaga);
        if self.resultados[vaga].is_some() {
            return;
        }
        let Some(item) = self.itens.get(self.fila[vaga]) else {
            return;
        };
        if anterior == Some(item.mascara()) {
            return;
        }
        if confere(item, seguradas) {
            let atraso_ms = (agora_ns as i64 - ini as i64) / 1_000_000;
            self.julgar(vaga, atraso_ms);
        }
    }

    fn julgar(&mut self, vaga: usize, atraso_ms: i64) {
        let j = Julgamento::Acertou { atraso_ms };
        self.resultados[vaga] = Some(j);
        if atraso_ms <= i64::from(self.config.tolerancia_ms) {
            self.placar.no_tempo += 1;
        } else {
            self.placar.atrasados += 1;
        }
        self.ultimo = Some((vaga, j));
    }

    /// Texto do último julgamento.
    pub fn texto_ultimo(&self) -> Option<String> {
        let (v, j) = self.ultimo?;
        let nome = self
            .fila
            .get(v)
            .and_then(|i| self.itens.get(*i))
            .map(|i| i.rotulo.clone())
            .unwrap_or_default();
        Some(match j {
            Julgamento::Acertou { atraso_ms } if atraso_ms < -20 => {
                format!("{nome}: certo, adiantado {} ms", -atraso_ms)
            }
            Julgamento::Acertou { atraso_ms }
                if atraso_ms <= i64::from(self.config.tolerancia_ms) =>
            {
                format!("{nome}: certo, no tempo")
            }
            Julgamento::Acertou { atraso_ms } => format!("{nome}: certo, atrasado {atraso_ms} ms"),
            Julgamento::Errou => format!("{nome}: não veio a tempo"),
        })
    }
}

/// O acorde segurado é o do item? Todas as classes, nenhuma a mais; aceita inversão e dobra.
/// Item de uma nota só aceita aquela nota sozinha (em qualquer oitava).
fn confere(item: &ItemTreino, seguradas: &[NotaMidi]) -> bool {
    if seguradas.is_empty() || item.notas.is_empty() {
        return false;
    }
    let m = item.mascara();
    if m.count_ones() >= 2 && seguradas.len() < 2 {
        return false;
    }
    mascara(seguradas) == m
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Ritmo falso: 4/4 a 60 BPM (1 s por pulso), começando em t = 0.
    struct Fixo {
        ppc: u64,
        pulso: u64,
    }

    impl Tempo for Fixo {
        fn posicao(&self, t: u64) -> Option<(u64, u64)> {
            let p = t / self.pulso;
            Some((p / self.ppc, p % self.ppc))
        }
        fn instante_do_compasso(&self, c: u64) -> Option<u64> {
            Some(c * self.ppc * self.pulso)
        }
        fn pulso_ns(&self) -> Option<u64> {
            Some(self.pulso)
        }
        fn pulsos_por_compasso(&self) -> u64 {
            self.ppc
        }
    }

    const S: u64 = 1_000_000_000;

    fn n(x: u8) -> NotaMidi {
        NotaMidi::nova(x).expect("nota")
    }

    fn itens() -> Vec<ItemTreino> {
        vec![
            ItemTreino::novo(vec![n(60), n(64), n(67)], "C", "Dó maior"),
            ItemTreino::novo(vec![n(60), n(65), n(69)], "F", "Fá maior"),
            ItemTreino::novo(vec![n(59), n(62), n(67)], "G", "Sol maior"),
        ]
    }

    fn treino(cfg: ConfigTreino) -> TreinoAndamento {
        let mut t = TreinoAndamento::novo(itens(), cfg);
        t.comecar_no_compasso(0);
        t
    }

    const FIXO: Fixo = Fixo { ppc: 4, pulso: S };

    #[test]
    fn contagem_depois_itens_em_ordem_e_fim() {
        let mut t = treino(ConfigTreino {
            voltas: Some(1),
            app_toca: false,
            ..Default::default()
        });
        t.avancar(&FIXO, S / 2, &[]);
        assert_eq!(
            t.momento(),
            Momento::Contagem {
                pulso: 1,
                pulsos: 4
            }
        );
        assert_eq!(t.atual().map(|i| i.rotulo.as_str()), Some("C"));
        t.avancar(&FIXO, 4 * S + 1, &[]);
        assert_eq!(t.momento(), Momento::Item { vaga: 0 });
        assert_eq!(t.proximo().map(|i| i.rotulo.as_str()), Some("F"));
        t.avancar(&FIXO, 8 * S + 1, &[]);
        assert_eq!(t.atual().map(|i| i.rotulo.as_str()), Some("F"));
        assert_eq!(t.placar.errados, 1);
        t.avancar(&FIXO, 16 * S + 1, &[]);
        assert_eq!(t.momento(), Momento::Terminou);
        assert_eq!(t.placar.errados, 3);
    }

    #[test]
    fn acerto_no_tempo_inversao_e_atraso() {
        let mut t = treino(ConfigTreino {
            voltas: Some(1),
            ..Default::default()
        });
        // Dó maior em 1ª inversão, 100 ms depois do 1.
        t.avancar(&FIXO, 4 * S + 100_000_000, &[n(64), n(67), n(72)]);
        assert_eq!(t.resultado(0), Some(Julgamento::Acertou { atraso_ms: 100 }));
        assert_eq!(t.placar.no_tempo, 1);
        // Fá com 900 ms de atraso.
        t.avancar(&FIXO, 8 * S + 900_000_000, &[n(65), n(69), n(72)]);
        assert_eq!(t.placar.atrasados, 1);
    }

    #[test]
    fn nota_a_mais_nao_vale() {
        let mut t = treino(ConfigTreino::default());
        t.avancar(&FIXO, 4 * S + 1, &[n(60), n(64), n(67), n(71)]);
        assert_eq!(t.resultado(0), None);
    }

    #[test]
    fn antecipar_meio_pulso_vale_para_o_proximo() {
        let mut t = treino(ConfigTreino::default());
        t.avancar(&FIXO, 4 * S + 1, &[n(60), n(64), n(67)]);
        // 200 ms antes do compasso 2, já com o Fá.
        t.avancar(&FIXO, 8 * S - 200_000_000, &[n(60), n(65), n(69)]);
        assert_eq!(
            t.resultado(1),
            Some(Julgamento::Acertou { atraso_ms: -200 })
        );
        t.avancar(&FIXO, 8 * S + 1, &[n(60), n(65), n(69)]);
        assert_eq!(t.placar.no_tempo, 2);
    }

    #[test]
    fn dois_compassos_por_item() {
        let mut t = treino(ConfigTreino {
            compassos_por_item: 2,
            ..Default::default()
        });
        t.avancar(&FIXO, 11 * S, &[]);
        assert_eq!(t.momento(), Momento::Item { vaga: 0 });
        t.avancar(&FIXO, 12 * S + 1, &[]);
        assert_eq!(t.momento(), Momento::Item { vaga: 1 });
    }

    #[test]
    fn app_toca_agenda_cada_item_uma_vez_no_inicio_do_compasso() {
        let mut t = treino(ConfigTreino {
            voltas: Some(1),
            ..Default::default()
        });
        let d = t.avancar(&FIXO, 4 * S - 100_000_000, &[]);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].inicio_ns, 4 * S);
        assert_eq!(d[0].notas, itens()[0].notas);
        assert!(t.avancar(&FIXO, 4 * S, &[]).is_empty());
        let d = t.avancar(&FIXO, 20 * S, &[]);
        assert_eq!(d.len(), 2, "as duas restantes, nenhuma além do fim");
    }

    #[test]
    fn aleatoria_nao_repete_em_seguida_e_respeita_voltas() {
        let mut t = TreinoAndamento::novo(
            itens(),
            ConfigTreino {
                ordem: Ordem::Aleatoria,
                voltas: Some(10),
                ..Default::default()
            },
        );
        t.semear(42);
        t.comecar_no_compasso(0);
        for v in 0..30 {
            assert!(t.item_da_vaga(v).is_some());
        }
        assert!(t.item_da_vaga(30).is_none());
        for w in t.fila.windows(2) {
            assert_ne!(w[0], w[1]);
        }
    }

    #[test]
    fn comecar_num_compasso_adiante_quando_o_ritmo_ja_toca() {
        let mut t = TreinoAndamento::novo(itens(), ConfigTreino::default());
        t.comecar_no_compasso(3);
        t.avancar(&FIXO, 12 * S + 1, &[]);
        assert_eq!(
            t.momento(),
            Momento::Contagem {
                pulso: 1,
                pulsos: 4
            }
        );
        t.avancar(&FIXO, 16 * S + 1, &[]);
        assert_eq!(t.momento(), Momento::Item { vaga: 0 });
    }

    #[test]
    fn nota_unica_aceita_qualquer_oitava() {
        let mut t = TreinoAndamento::novo(
            vec![ItemTreino::novo(vec![n(60)], "C", "")],
            ConfigTreino::default(),
        );
        t.comecar_no_compasso(0);
        t.avancar(&FIXO, 4 * S + 1, &[n(72)]);
        assert!(matches!(t.resultado(0), Some(Julgamento::Acertou { .. })));
    }
}
