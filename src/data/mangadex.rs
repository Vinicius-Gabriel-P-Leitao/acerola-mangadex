//! Cliente HTTP para a API do MangaDex.
//!
//! Este módulo centraliza:
//!
//! - URL base e `User-Agent`;
//! - timeouts;
//! - paginação do feed;
//! - limitação de taxa por tipo de endpoint;
//! - tratamento de `429`, `Retry-After` e erros transitórios.
//!
//! O cliente força HTTP/1.1 porque essa foi a forma estável para evitar
//! respostas HTML inesperadas do gateway em algumas chamadas.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER, USER_AGENT};
use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;
use tokio::time::sleep;
use tracing::warn;

use crate::data::models::{
    AtHomeResponse, ChapterResource, CollectionResponse, CoverResource, MangaResource,
    SingleResponse,
};
use crate::infra::{AppError, MangaDexConfig, RateLimiter, Result};

/// Cliente MangaDex usado pela camada de domínio.
#[derive(Clone)]
pub struct MangaDexClient {
    /// Cliente para endpoints JSON da API.
    api_client: Client,
    /// Cliente para download de imagens do host de uploads/AtHome.
    image_client: Client,
    /// Configuração de URLs, timeouts, paginação e retries.
    config: MangaDexConfig,
    /// Limitador global para chamadas da API.
    api_limiter: Arc<RateLimiter>,
    /// Limitador adicional para o endpoint AtHome.
    at_home_limiter: Arc<RateLimiter>,
    /// Limitador para downloads de imagens.
    image_limiter: Arc<RateLimiter>,
}

impl MangaDexClient {
    /// Cria o cliente HTTP com headers, timeouts e limitadores.
    ///
    /// O `User-Agent` é obrigatório para uso correto da API pública. As
    /// instâncias de `reqwest::Client` são clonáveis e compartilham pool.
    pub fn new(config: MangaDexConfig) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_str(&config.user_agent)?);

        let api_client = Client::builder()
            .default_headers(headers.clone())
            .http1_only()
            .timeout(config.request_timeout)
            .build()?;

        let image_client = Client::builder()
            .default_headers(headers)
            .http1_only()
            .timeout(config.image_request_timeout)
            .build()?;

        Ok(Self {
            api_client,
            image_client,
            api_limiter: Arc::new(RateLimiter::new(config.api_min_interval)),
            at_home_limiter: Arc::new(RateLimiter::new(config.at_home_min_interval)),
            image_limiter: Arc::new(RateLimiter::new(config.image_min_interval)),
            config,
        })
    }

    /// Busca os metadados do mangá incluindo relacionamento de cover.
    pub async fn get_manga(&self, manga_id: &str) -> Result<MangaResource> {
        let path = format!("/manga/{manga_id}");
        let query = vec![("includes[]", "cover_art".to_string())];
        let response: SingleResponse<MangaResource> =
            self.get_api_json(&path, &query, None).await?;
        ensure_success_result(&response.result)?;
        Ok(response.data)
    }

    /// Busca o recurso de cover para obter o nome do arquivo.
    pub async fn get_cover(&self, cover_id: &str) -> Result<CoverResource> {
        let path = format!("/cover/{cover_id}");
        let response: SingleResponse<CoverResource> = self.get_api_json(&path, &[], None).await?;
        ensure_success_result(&response.result)?;
        Ok(response.data)
    }

    /// Monta a URL pública da imagem de cover.
    pub fn cover_image_url(&self, manga_id: &str, file_name: &str) -> String {
        format!(
            "{}/covers/{}/{}",
            self.config.upload_base_url.trim_end_matches('/'),
            manga_id,
            file_name
        )
    }

    /// Busca todos os capítulos do feed para um idioma.
    ///
    /// A consulta já envia `translatedLanguage[]`, então a aplicação não baixa
    /// metadados de outros idiomas. A paginação respeita o limite de offset da
    /// API e falha cedo quando a consulta ultrapassaria esse limite.
    pub async fn get_all_chapters(
        &self,
        manga_id: &str,
        translated_language: &str,
    ) -> Result<Vec<ChapterResource>> {
        let mut chapters = Vec::new();
        let mut offset = 0;

        loop {
            if offset >= self.config.collection_hard_offset_limit {
                return Err(AppError::Api(format!(
                    "Manga feed cannot be paginated safely past offset {} because MangaDex rejects offset + limit above {}",
                    offset, self.config.collection_hard_offset_limit
                )));
            }

            let page = self
                .get_manga_feed_page(manga_id, translated_language, offset)
                .await?;
            ensure_success_result(&page.result)?;

            if page.total > self.config.collection_hard_offset_limit {
                return Err(AppError::Api(format!(
                    "Manga feed contains {} entries, which exceeds MangaDex collection pagination limit {}",
                    page.total, self.config.collection_hard_offset_limit
                )));
            }

            let page_len = page.data.len() as u32;
            chapters.extend(page.data);

            if page_len == 0 {
                return Err(AppError::Api(
                    "MangaDex returned an empty chapter page before the feed was complete"
                        .to_string(),
                ));
            }

            offset += page_len;

            if offset >= page.total {
                return Ok(chapters);
            }
        }
    }

    /// Busca o servidor AtHome temporário para um capítulo.
    ///
    /// O endpoint tem limitador próprio além do limitador global da API.
    pub async fn get_at_home_server(&self, chapter_id: &str) -> Result<AtHomeResponse> {
        let path = format!("/at-home/server/{chapter_id}");
        let response: AtHomeResponse = self
            .get_api_json(&path, &[], Some(self.at_home_limiter.as_ref()))
            .await?;
        ensure_success_result(&response.result)?;
        Ok(response)
    }

    /// Baixa uma imagem respeitando limiter, timeout e política de retry.
    ///
    /// `429` usa headers de retry quando disponíveis. `403` vira erro de
    /// proteção para evitar insistência contra bloqueio do host.
    pub async fn download_image(&self, url: &str) -> Result<Vec<u8>> {
        for attempt in 0..=self.config.max_retries {
            self.image_limiter.wait().await;

            let response = self.image_client.get(url).send().await;
            match response {
                Ok(response) => {
                    let status = response.status();

                    if status.is_success() {
                        return Ok(response.bytes().await?.to_vec());
                    }

                    if status == StatusCode::TOO_MANY_REQUESTS {
                        let delay = retry_delay(&response, attempt);
                        warn!(
                            url,
                            retry_after_ms = delay.as_millis(),
                            "Image request hit a rate limit"
                        );
                        sleep(delay).await;
                        continue;
                    }

                    if status == StatusCode::FORBIDDEN {
                        return Err(AppError::RateLimit(
                            "image host returned HTTP 403; stop sending requests before retrying later"
                                .to_string(),
                        ));
                    }

                    if status.is_server_error() && attempt < self.config.max_retries {
                        let delay = exponential_backoff(attempt);
                        warn!(
                            url,
                            status = status.as_u16(),
                            retry_after_ms = delay.as_millis(),
                            "Image host returned a server error"
                        );
                        sleep(delay).await;
                        continue;
                    }

                    let body = response.text().await?;
                    return Err(AppError::Api(format!(
                        "Image request failed with HTTP {} at {}: {}",
                        status,
                        url,
                        summarize_body(&body)
                    )));
                }
                Err(error) => {
                    if attempt >= self.config.max_retries {
                        return Err(AppError::from(error));
                    }

                    let delay = exponential_backoff(attempt);
                    warn!(
                        url,
                        %error,
                        retry_after_ms = delay.as_millis(),
                        "Image request failed"
                    );
                    sleep(delay).await;
                }
            }
        }

        Err(AppError::RateLimit(
            "image request retry budget was exhausted".to_string(),
        ))
    }

    /// Busca uma página do feed de capítulos já filtrada por idioma.
    async fn get_manga_feed_page(
        &self,
        manga_id: &str,
        translated_language: &str,
        offset: u32,
    ) -> Result<CollectionResponse<ChapterResource>> {
        let path = format!("/manga/{manga_id}/feed");

        let limit = self
            .config
            .feed_page_limit
            .min(self.config.collection_hard_offset_limit - offset);

        let query = vec![
            ("limit", limit.to_string()),
            ("offset", offset.to_string()),
            ("translatedLanguage[]", translated_language.to_string()),
            ("includes[]", "scanlation_group".to_string()),
            ("order[volume]", "asc".to_string()),
            ("order[chapter]", "asc".to_string()),
        ];

        self.get_api_json(&path, &query, None).await
    }

    /// Executa uma requisição GET JSON para a API com retries controlados.
    ///
    /// O limitador específico do endpoint roda antes do limitador global quando
    /// fornecido. Isso permite, por exemplo, proteger AtHome com intervalo maior
    /// sem duplicar lógica de HTTP.
    async fn get_api_json<T>(
        &self,
        path: &str,
        query: &[(&str, String)],
        endpoint_limiter: Option<&RateLimiter>,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let url = format!("{}{}", self.config.api_base_url.trim_end_matches('/'), path);

        for attempt in 0..=self.config.max_retries {
            if let Some(limiter) = endpoint_limiter {
                limiter.wait().await;
            }

            self.api_limiter.wait().await;

            let request = self.api_client.get(&url).query(query);
            let request_url = request
                .try_clone()
                .and_then(|builder| builder.build().ok())
                .map(|request| request.url().to_string())
                .unwrap_or_else(|| url.clone());
            let response = request.send().await;
            match response {
                Ok(response) => {
                    let status = response.status();

                    if status.is_success() {
                        return Ok(response.json::<T>().await?);
                    }

                    if status == StatusCode::TOO_MANY_REQUESTS {
                        let delay = retry_delay(&response, attempt);
                        warn!(
                            url,
                            retry_after_ms = delay.as_millis(),
                            "MangaDex API request hit a rate limit"
                        );
                        sleep(delay).await;
                        continue;
                    }

                    if status == StatusCode::FORBIDDEN {
                        return Err(AppError::RateLimit(
                            "MangaDex returned HTTP 403; stop sending requests before retrying later"
                                .to_string(),
                        ));
                    }

                    if status.is_server_error() && attempt < self.config.max_retries {
                        let delay = exponential_backoff(attempt);
                        warn!(
                            url,
                            status = status.as_u16(),
                            retry_after_ms = delay.as_millis(),
                            "MangaDex API returned a server error"
                        );
                        sleep(delay).await;
                        continue;
                    }

                    let body = response.text().await?;
                    return Err(AppError::Api(format!(
                        "MangaDex API request failed with HTTP {} at {}: {}",
                        status,
                        request_url,
                        summarize_body(&body)
                    )));
                }
                Err(error) => {
                    if attempt >= self.config.max_retries {
                        return Err(AppError::from(error));
                    }

                    let delay = exponential_backoff(attempt);
                    warn!(
                        url,
                        %error,
                        retry_after_ms = delay.as_millis(),
                        "MangaDex API request failed"
                    );
                    sleep(delay).await;
                }
            }
        }

        Err(AppError::RateLimit(
            "MangaDex API request retry budget was exhausted".to_string(),
        ))
    }
}

/// Garante que o campo `result` do MangaDex indica sucesso.
fn ensure_success_result(result: &str) -> Result<()> {
    if result == "ok" {
        return Ok(());
    }

    Err(AppError::Api(format!(
        "MangaDex returned result '{result}'"
    )))
}

/// Calcula o atraso para uma nova tentativa.
///
/// Headers de rate limit têm prioridade sobre backoff exponencial.
fn retry_delay(response: &Response, attempt: usize) -> Duration {
    rate_limit_header_delay(response).unwrap_or_else(|| exponential_backoff(attempt))
}

/// Lê headers conhecidos de rate limit e adiciona margem de segurança.
fn rate_limit_header_delay(response: &Response) -> Option<Duration> {
    if let Some(delay) = x_rate_limit_retry_after_delay(response) {
        return Some(delay + Duration::from_secs(1));
    }

    if let Some(delay) = retry_after_delay(response) {
        return Some(delay + Duration::from_secs(1));
    }

    None
}

/// Interpreta `X-RateLimit-Retry-After` como timestamp Unix.
fn x_rate_limit_retry_after_delay(response: &Response) -> Option<Duration> {
    let value = response.headers().get("X-RateLimit-Retry-After")?;
    let value = value.to_str().ok()?;
    let timestamp = value.parse::<u64>().ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();

    Some(Duration::from_secs(timestamp.saturating_sub(now)))
}

/// Interpreta `Retry-After` como quantidade de segundos.
fn retry_after_delay(response: &Response) -> Option<Duration> {
    let value = response.headers().get(RETRY_AFTER)?;
    let value = value.to_str().ok()?;
    let seconds = value.parse::<u64>().ok()?;

    Some(Duration::from_secs(seconds))
}

/// Backoff exponencial com teto de expoente para evitar atrasos enormes.
fn exponential_backoff(attempt: usize) -> Duration {
    let capped_attempt = attempt.min(5) as u32;
    Duration::from_millis(500 * 2_u64.pow(capped_attempt))
}

/// Reduz corpos de erro para mensagens legíveis no terminal.
fn summarize_body(body: &str) -> String {
    let normalized = body.split_whitespace().collect::<Vec<_>>().join(" ");

    if normalized.chars().count() <= 300 {
        return normalized;
    }

    let preview = normalized.chars().take(300).collect::<String>();
    format!("{preview}...")
}
