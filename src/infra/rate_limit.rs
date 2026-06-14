//! Limitador simples de intervalo mínimo.
//!
//! O MangaDex possui limites de requisição. Este utilitário serializa o acesso
//! por recurso compartilhado e garante uma distância mínima entre chamadas.

use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tokio::time::sleep;

/// Limitador assíncrono baseado em um próximo instante disponível.
#[derive(Debug)]
pub struct RateLimiter {
    /// Distância mínima entre duas liberações.
    min_interval: Duration,
    /// Próximo instante em que uma chamada pode prosseguir.
    next_available: Mutex<Instant>,
}

impl RateLimiter {
    /// Cria um limitador com o intervalo informado.
    pub fn new(min_interval: Duration) -> Self {
        Self {
            min_interval,
            next_available: Mutex::new(Instant::now()),
        }
    }

    /// Aguarda até a próxima janela disponível.
    ///
    /// A função segura o mutex durante a espera para serializar concorrência e
    /// impedir que várias tarefas passem juntas após o mesmo atraso.
    pub async fn wait(&self) {
        let mut next_available = self.next_available.lock().await;
        let now = Instant::now();
        let delay = next_available
            .checked_duration_since(now)
            .unwrap_or(Duration::ZERO);

        if !delay.is_zero() {
            sleep(delay).await;
        }

        *next_available = Instant::now() + self.min_interval;
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::RateLimiter;

    #[tokio::test]
    async fn waits_between_two_consecutive_slots() {
        let limiter = RateLimiter::new(Duration::from_millis(25));

        limiter.wait().await;
        let started = Instant::now();
        limiter.wait().await;

        assert!(started.elapsed() >= Duration::from_millis(20));
    }
}
