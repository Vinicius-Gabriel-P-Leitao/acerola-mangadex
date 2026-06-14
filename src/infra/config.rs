//! Configuração padrão da aplicação.
//!
//! Os valores ficam centralizados para que URLs, timeouts, intervalos mínimos e
//! limites de paginação não sejam repetidos em camadas diferentes.

use std::time::Duration;

/// Configuração raiz da aplicação.
#[derive(Clone, Debug, Default)]
pub struct AppConfig {
    /// Configuração específica do MangaDex.
    pub mangadex: MangaDexConfig,
}

/// Configuração do cliente MangaDex.
#[derive(Clone, Debug)]
pub struct MangaDexConfig {
    /// URL base da API JSON.
    pub api_base_url: String,
    /// URL base do host de uploads.
    pub upload_base_url: String,
    /// `User-Agent` enviado em todas as requisições.
    pub user_agent: String,
    /// Timeout para endpoints JSON.
    pub request_timeout: Duration,
    /// Timeout para downloads de imagem.
    pub image_request_timeout: Duration,
    /// Intervalo mínimo global entre chamadas da API.
    pub api_min_interval: Duration,
    /// Intervalo mínimo adicional para o endpoint AtHome.
    pub at_home_min_interval: Duration,
    /// Intervalo mínimo entre downloads de imagem.
    pub image_min_interval: Duration,
    /// Quantidade máxima de novas tentativas por requisição.
    pub max_retries: usize,
    /// Tamanho de página usado no feed de capítulos.
    pub feed_page_limit: u32,
    /// Limite duro de offset aceito com segurança pela API de collections.
    pub collection_hard_offset_limit: u32,
}

impl Default for MangaDexConfig {
    /// Cria a configuração conservadora para uso da API pública do MangaDex.
    fn default() -> Self {
        Self {
            api_base_url: "https://api.mangadex.org".to_string(),
            upload_base_url: "https://uploads.mangadex.org".to_string(),
            user_agent: format!(
                "acerola-mangadex/{} (Rust CLI downloader)",
                env!("CARGO_PKG_VERSION")
            ),
            request_timeout: Duration::from_secs(30),
            image_request_timeout: Duration::from_secs(60),
            api_min_interval: Duration::from_millis(250),
            at_home_min_interval: Duration::from_millis(1_600),
            image_min_interval: Duration::from_millis(100),
            max_retries: 5,
            feed_page_limit: 100,
            collection_hard_offset_limit: 10_000,
        }
    }
}
