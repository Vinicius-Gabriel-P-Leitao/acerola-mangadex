//! Indexação e disponibilidade de capítulos.
//!
//! O feed do MangaDex pode conter múltiplas entradas para o mesmo número de
//! capítulo, incluindo traduções, grupos diferentes e capítulos externos. Este
//! módulo agrupa essas entradas em capítulos indexados e escolhe a melhor opção
//! baixável para o idioma pedido.

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::data::ChapterResource;

/// Entrada individual de capítulo vinda do MangaDex já convertida para o core.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChapterEntry {
    /// UUID do capítulo usado no endpoint AtHome.
    pub id: String,
    /// Número real do capítulo (`attributes.chapter`).
    pub chapter_number: Option<String>,
    /// Título opcional do capítulo.
    pub title: Option<String>,
    /// Idioma da tradução.
    pub translated_language: String,
    /// Quantidade de páginas informada pela API.
    pub pages: u32,
    /// URL externa quando o capítulo não é hospedado no MangaDex.
    pub external_url: Option<String>,
}

/// Grupo de entradas que representam o mesmo número de capítulo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedChapter {
    /// Posição 1-based exibida para seleção por índice.
    pub index: usize,
    /// Chave de agrupamento, normalmente o número real do capítulo.
    pub key: String,
    /// Texto legível usado em logs e mensagens de erro.
    pub display_label: String,
    /// Entradas candidatas para esse capítulo.
    pub chapters: Vec<ChapterEntry>,
}

/// Resultado da escolha de um capítulo para determinado idioma.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChapterAvailability {
    /// Há uma entrada hospedada no MangaDex que pode ser baixada.
    Available(ChapterEntry),
    /// O idioma existe, mas todas as entradas apontam para fonte externa.
    ExternalOnly,
    /// Não há entrada no idioma solicitado.
    MissingLanguage,
}

impl IndexedChapter {
    /// Escolhe a entrada baixável para um idioma.
    ///
    /// Quando há mais de uma entrada no idioma, a função escolhe a que possui
    /// mais páginas, que costuma representar a versão mais completa.
    pub fn downloadable_for_language(&self, language: &str) -> ChapterAvailability {
        let language_matches = self
            .chapters
            .iter()
            .filter(|chapter| chapter.translated_language == language)
            .collect::<Vec<_>>();

        if language_matches.is_empty() {
            return ChapterAvailability::MissingLanguage;
        }

        let downloadable = language_matches
            .into_iter()
            .filter(|chapter| chapter.external_url.is_none())
            .max_by_key(|chapter| chapter.pages)
            .cloned();

        match downloadable {
            Some(chapter) => ChapterAvailability::Available(chapter),
            None => ChapterAvailability::ExternalOnly,
        }
    }
}

/// Agrupa recursos do feed em capítulos indexados.
///
/// A ordenação usa comparação numérica quando a chave parece um número de
/// capítulo, preservando casos decimais como `0.01`.
pub fn index_chapters(resources: Vec<ChapterResource>) -> Vec<IndexedChapter> {
    let mut grouped = HashMap::<String, Vec<ChapterEntry>>::new();

    for resource in resources {
        let key = chapter_key(&resource);
        grouped.entry(key).or_default().push(ChapterEntry {
            id: resource.id,
            chapter_number: resource.attributes.chapter,
            title: resource.attributes.title,
            translated_language: resource.attributes.translated_language,
            pages: resource.attributes.pages.unwrap_or_default(),
            external_url: resource.attributes.external_url,
        });
    }

    let mut groups = grouped.into_iter().collect::<Vec<_>>();
    groups.sort_by(|left, right| compare_chapter_keys(&left.0, &right.0));

    groups
        .into_iter()
        .enumerate()
        .map(|(position, (key, chapters))| IndexedChapter {
            index: position + 1,
            display_label: display_label(&chapters),
            key,
            chapters,
        })
        .collect()
}

/// Define a chave de agrupamento para um recurso do feed.
///
/// Capítulos sem número usam uma chave baseada no ID para não colidir com
/// outros recursos sem `attributes.chapter`.
fn chapter_key(resource: &ChapterResource) -> String {
    resource
        .attributes
        .chapter
        .clone()
        .unwrap_or_else(|| format!("id:{}", resource.id))
}

/// Compara chaves de capítulo, usando número quando possível.
fn compare_chapter_keys(left: &str, right: &str) -> Ordering {
    match (parse_chapter_number(left), parse_chapter_number(right)) {
        (Some(left), Some(right)) => left.total_cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => left.cmp(right),
    }
}

/// Tenta interpretar uma chave como número de capítulo.
fn parse_chapter_number(value: &str) -> Option<f64> {
    value.parse::<f64>().ok()
}

/// Monta o texto mostrado em logs e falhas.
fn display_label(chapters: &[ChapterEntry]) -> String {
    let first = chapters.first();
    let chapter_number = first.and_then(|chapter| chapter.chapter_number.as_deref());
    let title = first.and_then(|chapter| chapter.title.as_deref());

    match (chapter_number, title) {
        (Some(number), Some(title)) if !title.trim().is_empty() => {
            format!("Chapter {number} - {}", title.trim())
        }
        (Some(number), _) => format!("Chapter {number}"),
        (_, Some(title)) if !title.trim().is_empty() => title.trim().to_string(),
        _ => "Chapter without number".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use crate::data::{ChapterAttributes, ChapterResource};

    use super::{ChapterAvailability, ChapterEntry, index_chapters};

    fn chapter(id: &str, number: &str, language: &str, pages: u32) -> ChapterResource {
        ChapterResource {
            id: id.to_string(),
            resource_type: "chapter".to_string(),
            attributes: ChapterAttributes {
                chapter: Some(number.to_string()),
                title: None,
                translated_language: language.to_string(),
                pages: Some(pages),
                external_url: None,
            },
            relationships: Vec::new(),
        }
    }

    #[test]
    fn groups_translations_by_chapter_number_and_selects_language() {
        let indexed = index_chapters(vec![
            chapter("en-1", "1", "en", 12),
            chapter("pt-1", "1", "pt-br", 14),
            chapter("pt-2", "2", "pt-br", 20),
        ]);

        assert_eq!(indexed.len(), 2);
        assert_eq!(indexed[0].index, 1);
        assert_eq!(
            indexed[0].downloadable_for_language("pt-br"),
            ChapterAvailability::Available(ChapterEntry {
                id: "pt-1".to_string(),
                chapter_number: Some("1".to_string()),
                title: None,
                translated_language: "pt-br".to_string(),
                pages: 14,
                external_url: None,
            })
        );
    }

    #[test]
    fn reports_missing_language() {
        let indexed = index_chapters(vec![chapter("en-1", "1", "en", 12)]);

        assert_eq!(
            indexed[0].downloadable_for_language("pt-br"),
            ChapterAvailability::MissingLanguage
        );
    }
}
