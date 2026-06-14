//! Infraestrutura compartilhada.
//!
//! Aqui ficam peças transversais que não pertencem a uma regra de negócio
//! específica: configuração da aplicação, tipo de erro comum e limitador de
//! taxa usado pelo cliente HTTP.

/// Configuração padrão e valores de proteção para chamadas externas.
pub mod config;
/// Tipo de erro único da aplicação.
pub mod error;
/// Limitador simples de intervalo mínimo entre operações.
pub mod rate_limit;

/// Configurações públicas usadas na montagem das dependências.
pub use config::{AppConfig, MangaDexConfig};
/// Tipo de erro e alias de resultado da aplicação.
pub use error::{AppError, Result};
/// Limitador de taxa compartilhado pelo cliente MangaDex.
pub use rate_limit::RateLimiter;
