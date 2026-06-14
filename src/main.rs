//! Binário da aplicação.
//!
//! O executável apenas inicializa o runtime assíncrono e delega a execução para
//! a camada `cmd`. Isso evita duplicar lógica entre binário e biblioteca.

use std::process::ExitCode;

/// Ponto de entrada do processo.
///
/// O macro `tokio::main` cria o runtime assíncrono necessário para chamadas
/// HTTP e operações de arquivo. O retorno usa `ExitCode` para sinalizar sucesso
/// ou falha ao shell de forma explícita.
#[tokio::main]
async fn main() -> ExitCode {
    match acerola_mangadex::cmd::run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "Application failed");
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}
