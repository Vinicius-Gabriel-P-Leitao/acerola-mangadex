//! Extração do identificador do mangá.
//!
//! A CLI aceita tanto a URL pública do MangaDex quanto o UUID cru. Este módulo
//! valida a origem do link e entrega sempre o UUID normalizado.

use url::Url;
use uuid::Uuid;

use crate::infra::{AppError, Result};

/// Extrai o UUID do mangá a partir de um link do MangaDex ou UUID cru.
///
/// URLs de outros domínios são rejeitadas para evitar downloads disparados a
/// partir de entradas ambíguas ou malformadas.
pub fn extract_manga_id(input: &str) -> Result<String> {
    let trimmed = input.trim();

    if let Ok(uuid) = Uuid::parse_str(trimmed) {
        return Ok(uuid.to_string());
    }

    let url = Url::parse(trimmed).map_err(|_| {
        AppError::InvalidInput("expected a MangaDex title URL or manga UUID".to_string())
    })?;

    let host = url.host_str().unwrap_or_default();
    let allowed_host = matches!(host, "mangadex.org" | "www.mangadex.org");
    if !allowed_host {
        return Err(AppError::InvalidInput(format!(
            "expected mangadex.org URL, got host '{host}'"
        )));
    }

    let segments: Vec<&str> = url
        .path_segments()
        .map(|segments| segments.collect())
        .unwrap_or_default();
    let title_position = segments.iter().position(|segment| *segment == "title");
    let manga_id = title_position
        .and_then(|position| segments.get(position + 1))
        .ok_or_else(|| {
            AppError::InvalidInput("expected URL path /title/{manga-id}/...".to_string())
        })?;

    Ok(Uuid::parse_str(manga_id)?.to_string())
}

#[cfg(test)]
mod tests {
    use super::extract_manga_id;

    #[test]
    fn extracts_manga_id_from_title_url() {
        let id = extract_manga_id(
            "https://mangadex.org/title/801513ba-a712-498c-8f57-cae55b38cc92/berserk",
        )
        .unwrap();

        assert_eq!(id, "801513ba-a712-498c-8f57-cae55b38cc92");
    }

    #[test]
    fn accepts_raw_uuid() {
        let id = extract_manga_id("801513ba-a712-498c-8f57-cae55b38cc92").unwrap();

        assert_eq!(id, "801513ba-a712-498c-8f57-cae55b38cc92");
    }

    #[test]
    fn rejects_other_hosts() {
        assert!(
            extract_manga_id("https://example.com/title/801513ba-a712-498c-8f57-cae55b38cc92")
                .is_err()
        );
    }
}
