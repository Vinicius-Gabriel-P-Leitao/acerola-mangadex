//! Escolha do título local do mangá.
//!
//! O MangaDex retorna títulos por idioma. A aplicação escolhe uma opção
//! preferencial para exibir no terminal e criar a pasta local.

use crate::data::MangaAttributes;

/// Escolhe o melhor título disponível nos atributos do mangá.
///
/// A ordem prioriza inglês, português do Brasil e títulos romanizados. Se
/// nenhuma dessas chaves existir, a função usa o primeiro título não vazio e,
/// por último, um fallback explícito.
pub fn choose_manga_title(attributes: &MangaAttributes) -> String {
    for language in ["en", "pt-br", "ja-ro", "ja"] {
        let title = attributes
            .title
            .get(language)
            .map(|title| title.trim())
            .filter(|title| !title.is_empty());

        if let Some(title) = title {
            return title.to_string();
        }
    }

    attributes
        .title
        .values()
        .find(|title| !title.trim().is_empty())
        .map(|title| title.trim().to_string())
        .unwrap_or_else(|| "Unknown Manga".to_string())
}
