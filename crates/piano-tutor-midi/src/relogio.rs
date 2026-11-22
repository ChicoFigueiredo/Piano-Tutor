//! Relógio monotônico injetável.
//!
//! Todo timestamp do pipeline MIDI é "nanossegundos desde a base do relógio".
//! A implementação real usa um [`Instant`] fixado na criação; a manual é para
//! testes (avança só quando o teste manda, sem `sleep`).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Fonte de tempo monotônico em nanossegundos desde uma base fixa.
pub trait Relogio: Send + Sync {
    fn agora_ns(&self) -> u64;
}

/// Relógio real: `Instant` base capturado em [`RelogioMonotonico::novo`].
#[derive(Clone, Copy, Debug)]
pub struct RelogioMonotonico {
    base: Instant,
}

impl RelogioMonotonico {
    #[must_use]
    pub fn novo() -> Self {
        Self {
            base: Instant::now(),
        }
    }
}

impl Default for RelogioMonotonico {
    fn default() -> Self {
        Self::novo()
    }
}

impl Relogio for RelogioMonotonico {
    fn agora_ns(&self) -> u64 {
        // Satura em ~584 anos de execução contínua.
        u64::try_from(self.base.elapsed().as_nanos()).unwrap_or(u64::MAX)
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
}
