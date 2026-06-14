//! Tipo de erro único da aplicação.
//!
//! O projeto usa `thiserror` para manter mensagens legíveis e conversões `From`
//! explícitas. Assim as camadas podem propagar falhas com `?` sem perder o
//! contexto principal.

use thiserror::Error;

/// Erros que podem ocorrer durante execução da CLI.
#[derive(Debug, Error)]
pub enum AppError {
    /// Erro retornado pela API ou por uma resposta inesperada.
    #[error("MangaDex API error: {0}")]
    Api(String),
    /// Erro do cliente HTTP.
    #[error("HTTP client error: {0}")]
    Http(#[from] reqwest::Error),
    /// Header HTTP inválido, normalmente no `User-Agent`.
    #[error("invalid HTTP header value: {0}")]
    InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),
    /// Entrada inválida fornecida pelo usuário ou por parsing da CLI.
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// Falha de sistema de arquivos.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Falha ao converter número inteiro.
    #[error("integer parse error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),
    /// Falha vinda de prompt interativo.
    #[error("CLI prompt error: {0}")]
    Prompt(String),
    /// Proteção contra limite de taxa interrompeu a operação.
    #[error("rate limit protection stopped the request: {0}")]
    RateLimit(String),
    /// URL inválida.
    #[error("URL parse error: {0}")]
    Url(#[from] url::ParseError),
    /// UUID inválido.
    #[error("UUID parse error: {0}")]
    Uuid(#[from] uuid::Error),
    /// Falha ao escrever arquivo ZIP/CBZ.
    #[error("ZIP archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// Alias de resultado usado em todas as camadas da aplicação.
pub type Result<T> = std::result::Result<T, AppError>;
