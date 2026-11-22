//! Base de tempo **única** do Piano Tutor: nanossegundos monotônicos desde uma época fixa.
//!
//! Contrato (ADR-0005, relógio mestre): o timestamp que a entrada MIDI carimba em cada evento
//! (`timestamp_monotonico_ns`) e o instante que o callback de áudio usa para ancorar frames ↔
//! nanossegundos precisam vir **do mesmo relógio, com a mesma época**. Por isso o trait e as
//! implementações moram aqui, no núcleo, e as crates `piano-tutor-midi` e `piano-tutor-audio`
//! apenas os reexportam: há um único tipo [`RelogioMonotonico`] no workspace e o app cria **uma**
//! instância e a entrega às duas.
//!
//! Exceção consciente ao "núcleo sem SO": [`RelogioMonotonico`] lê `std::time::Instant`
//! (relógio monotônico da biblioteca padrão, sem hardware nem E/S). Os testes determinísticos
//! usam [`RelogioManual`].

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Fonte de tempo monotônico em nanossegundos desde uma base fixa.
///
/// Implementações devem ser baratas, sem alocação e sem lock bloqueante: o callback de áudio
/// chama [`Relogio::agora_ns`] a cada bloco.
pub trait Relogio: Send + Sync {
    fn agora_ns(&self) -> u64;
}

impl<T: Relogio + ?Sized> Relogio for Arc<T> {
    fn agora_ns(&self) -> u64 {
        (**self).agora_ns()
    }
}

impl<T: Relogio + ?Sized> Relogio for &T {
    fn agora_ns(&self) -> u64 {
        (**self).agora_ns()
    }
}

/// Relógio real: `Instant` base capturado em [`RelogioMonotonico::novo`]. Cópias compartilham
/// a mesma época (o `Instant` é copiado, não recapturado).
#[derive(Clone, Copy, Debug)]
pub struct RelogioMonotonico {
    epoca: Instant,
}

impl RelogioMonotonico {
    #[must_use]
    pub fn novo() -> Self {
        Self {
            epoca: Instant::now(),
        }
    }

    /// Nanossegundos desde a época. Sem alocação nem lock (seguro no callback de áudio).
    /// Satura em ~584 anos de execução contínua.
    pub fn agora_ns(&self) -> u64 {
        u64::try_from(self.epoca.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }
}

impl Default for RelogioMonotonico {
    fn default() -> Self {
        Self::novo()
    }
}

impl Relogio for RelogioMonotonico {
    fn agora_ns(&self) -> u64 {
        RelogioMonotonico::agora_ns(self)
    }
}

/// Relógio controlado manualmente (testes e simulação). Clones compartilham o tempo.
#[derive(Clone, Debug, Default)]
pub struct RelogioManual {
    ns: Arc<AtomicU64>,
}

impl RelogioManual {
    #[must_use]
    pub fn novo() -> Self {
        Self::default()
    }

    /// Avança o tempo (nunca retrocede: monotônico por construção).
    pub fn avancar(&self, passo: Duration) {
        let passo = u64::try_from(passo.as_nanos()).unwrap_or(u64::MAX);
        // `fetch_update` com closure que sempre retorna Some nunca falha.
        let _ = self
            .ns
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |atual| {
                Some(atual.saturating_add(passo))
            });
    }
}

impl Relogio for RelogioManual {
    fn agora_ns(&self) -> u64 {
        self.ns.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn relogio_manual_avanca_e_e_compartilhado() {
        let r = RelogioManual::novo();
        let copia = r.clone();
        assert_eq!(r.agora_ns(), 0);
        copia.avancar(Duration::from_millis(250));
        assert_eq!(r.agora_ns(), 250_000_000);
    }

    #[test]
    fn relogio_monotonico_nao_retrocede() {
        let r = RelogioMonotonico::novo();
        let a = r.agora_ns();
        let b = r.agora_ns();
        assert!(b >= a);
    }

    #[test]
    fn copia_do_relogio_monotonico_tem_a_mesma_epoca() {
        let r = RelogioMonotonico::novo();
        let copia = r;
        std::thread::sleep(Duration::from_millis(2));
        let a = r.agora_ns();
        let b = copia.agora_ns();
        let c = r.agora_ns();
        // Mesma época: a leitura da cópia fica entre duas leituras do original.
        assert!(a <= b && b <= c, "{a} {b} {c}");
        assert!(a >= 2_000_000);
    }

    #[test]
    fn arc_dyn_relogio_delega_ao_relogio_de_dentro() {
        let manual = RelogioManual::novo();
        let compartilhado: Arc<dyn Relogio> = Arc::new(manual.clone());
        manual.avancar(Duration::from_micros(7));
        assert_eq!(compartilhado.agora_ns(), 7_000);
        assert_eq!(Relogio::agora_ns(&compartilhado), 7_000);
    }
}
