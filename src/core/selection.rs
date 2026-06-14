//! Parsing e resolução de seleção de capítulos.
//!
//! A CLI aceita seleção por índice da lista filtrada ou pelo número real do
//! capítulo informado pelo MangaDex. Este módulo transforma essas entradas em
//! índices válidos para o catálogo carregado.

use crate::core::chapter::IndexedChapter;
use crate::infra::{AppError, Result};

/// Seleção de capítulos solicitada pelo usuário.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChapterSelection {
    /// Baixa todos os capítulos do catálogo atual.
    All,
    /// Baixa um capítulo pela posição exibida na lista.
    Index(usize),
    /// Baixa um intervalo fechado de posições da lista.
    IndexRange { start: usize, end: usize },
    /// Baixa capítulos que tenham exatamente o número informado.
    Number(String),
    /// Baixa capítulos cujo número real esteja dentro do intervalo fechado.
    NumberRange { start: String, end: String },
}

impl ChapterSelection {
    /// Resolve a seleção para índices 1-based do catálogo.
    ///
    /// A resolução por número consulta os metadados já carregados e retorna os
    /// índices atuais correspondentes. Isso mantém a etapa de download usando
    /// uma única representação interna.
    pub fn resolve(&self, chapters: &[IndexedChapter]) -> Result<Vec<usize>> {
        if chapters.is_empty() {
            return Err(AppError::InvalidInput(
                "cannot select chapters from an empty catalog".to_string(),
            ));
        }

        let total = chapters.len();
        match self {
            Self::All => Ok((1..=total).collect()),
            Self::Index(index) => {
                validate_index(*index, total)?;
                Ok(vec![*index])
            }
            Self::IndexRange { start, end } => {
                validate_range(*start, *end, total)?;
                Ok((*start..=*end).collect())
            }
            Self::Number(number) => resolve_chapter_number(chapters, number),
            Self::NumberRange { start, end } => resolve_chapter_number_range(chapters, start, end),
        }
    }
}

/// Interpreta uma seleção por índice.
///
/// Aceita `all`, um índice único ou um intervalo como `100-200`. A validação
/// usa o total do catálogo para impedir seleções fora dos limites.
pub fn parse_selection(input: &str, total: usize) -> Result<ChapterSelection> {
    let trimmed = input.trim();

    if trimmed.eq_ignore_ascii_case("all") {
        return Ok(ChapterSelection::All);
    }

    if let Some((start, end)) = trimmed.split_once('-') {
        let start = start.trim().parse::<usize>()?;
        let end = end.trim().parse::<usize>()?;
        validate_range(start, end, total)?;
        return Ok(ChapterSelection::IndexRange { start, end });
    }

    let index = trimmed.parse::<usize>()?;
    validate_index(index, total)?;
    Ok(ChapterSelection::Index(index))
}

/// Interpreta uma seleção pelo número real do capítulo.
///
/// Ao contrário da seleção por índice, esta função não recebe o total do
/// catálogo. A existência dos números só é verificada depois, em `resolve`.
pub fn parse_chapter_number_selection(input: &str) -> Result<ChapterSelection> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(AppError::InvalidInput(
            "chapter number selection cannot be empty".to_string(),
        ));
    }

    if let Some((start, end)) = trimmed.split_once('-') {
        let start = validate_chapter_number_input(start)?;
        let end = validate_chapter_number_input(end)?;
        validate_chapter_number_order(&start, &end)?;
        return Ok(ChapterSelection::NumberRange { start, end });
    }

    Ok(ChapterSelection::Number(validate_chapter_number_input(
        trimmed,
    )?))
}

/// Encontra os índices correspondentes a um número de capítulo.
fn resolve_chapter_number(chapters: &[IndexedChapter], number: &str) -> Result<Vec<usize>> {
    let normalized = normalize_chapter_number(number);
    let matches = chapters
        .iter()
        .filter(|chapter| normalize_chapter_number(&chapter.key) == normalized)
        .map(|chapter| chapter.index)
        .collect::<Vec<_>>();

    if matches.is_empty() {
        return Err(AppError::InvalidInput(format!(
            "chapter number {number} was not found in the selected language catalog"
        )));
    }

    Ok(matches)
}

/// Encontra os índices correspondentes a um intervalo de números de capítulo.
///
/// O intervalo exige números parseáveis como `f64`, permitindo valores decimais
/// como `0.01`.
fn resolve_chapter_number_range(
    chapters: &[IndexedChapter],
    start: &str,
    end: &str,
) -> Result<Vec<usize>> {
    let start_number = parse_numeric_chapter_number(start)?;
    let end_number = parse_numeric_chapter_number(end)?;
    let matches = chapters
        .iter()
        .filter(|chapter| {
            parse_numeric_chapter_number(&chapter.key)
                .map(|number| number >= start_number && number <= end_number)
                .unwrap_or(false)
        })
        .map(|chapter| chapter.index)
        .collect::<Vec<_>>();

    if matches.is_empty() {
        return Err(AppError::InvalidInput(format!(
            "chapter number range {start}-{end} did not match any chapters in the selected language catalog"
        )));
    }

    Ok(matches)
}

/// Valida se o índice 1-based existe no catálogo.
fn validate_index(index: usize, total: usize) -> Result<()> {
    if index == 0 {
        return Err(AppError::InvalidInput(
            "chapter index must start at 1".to_string(),
        ));
    }

    if index > total {
        return Err(AppError::InvalidInput(format!(
            "chapter index {index} is outside the catalog total {total}"
        )));
    }

    Ok(())
}

/// Valida uma entrada textual de número de capítulo.
///
/// Separadores de caminho são rejeitados porque o número pode ir para o nome do
/// arquivo final.
fn validate_chapter_number_input(input: &str) -> Result<String> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err(AppError::InvalidInput(
            "chapter number cannot be empty".to_string(),
        ));
    }

    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err(AppError::InvalidInput(format!(
            "chapter number '{trimmed}' contains invalid path separators"
        )));
    }

    Ok(trimmed.to_string())
}

/// Valida a ordem de um intervalo por número de capítulo.
fn validate_chapter_number_order(start: &str, end: &str) -> Result<()> {
    let start_number = parse_numeric_chapter_number(start)?;
    let end_number = parse_numeric_chapter_number(end)?;

    if start_number > end_number {
        return Err(AppError::InvalidInput(format!(
            "chapter number range start {start} cannot be greater than end {end}"
        )));
    }

    Ok(())
}

/// Converte um número de capítulo textual para comparação numérica.
fn parse_numeric_chapter_number(value: &str) -> Result<f64> {
    value.trim().parse::<f64>().map_err(|error| {
        AppError::InvalidInput(format!(
            "chapter number '{value}' must be numeric for range selection: {error}"
        ))
    })
}

/// Normaliza número de capítulo para comparação exata.
fn normalize_chapter_number(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

/// Valida um intervalo fechado de índices.
fn validate_range(start: usize, end: usize, total: usize) -> Result<()> {
    validate_index(start, total)?;
    validate_index(end, total)?;

    if start > end {
        return Err(AppError::InvalidInput(format!(
            "chapter range start {start} cannot be greater than end {end}"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::core::chapter::{ChapterEntry, IndexedChapter};

    use super::{ChapterSelection, parse_chapter_number_selection, parse_selection};

    #[test]
    fn parses_all() {
        assert_eq!(parse_selection("all", 10).unwrap(), ChapterSelection::All);
    }

    #[test]
    fn parses_single_index() {
        assert_eq!(
            parse_selection("7", 10).unwrap(),
            ChapterSelection::Index(7)
        );
    }

    #[test]
    fn parses_range() {
        assert_eq!(
            parse_selection("2-4", 10).unwrap(),
            ChapterSelection::IndexRange { start: 2, end: 4 }
        );
    }

    #[test]
    fn rejects_out_of_bounds_range() {
        assert!(parse_selection("2-11", 10).is_err());
    }

    #[test]
    fn parses_single_chapter_number() {
        assert_eq!(
            parse_chapter_number_selection("133").unwrap(),
            ChapterSelection::Number("133".to_string())
        );
    }

    #[test]
    fn parses_chapter_number_range() {
        assert_eq!(
            parse_chapter_number_selection("100-133").unwrap(),
            ChapterSelection::NumberRange {
                start: "100".to_string(),
                end: "133".to_string()
            }
        );
    }

    #[test]
    fn resolves_chapter_number_to_current_index() {
        let chapters = indexed_chapters();

        assert_eq!(
            ChapterSelection::Number("133".to_string())
                .resolve(&chapters)
                .unwrap(),
            vec![3]
        );
    }

    #[test]
    fn resolves_chapter_number_range_to_current_indexes() {
        let chapters = indexed_chapters();

        assert_eq!(
            ChapterSelection::NumberRange {
                start: "0.02".to_string(),
                end: "133".to_string()
            }
            .resolve(&chapters)
            .unwrap(),
            vec![2, 3]
        );
    }

    fn indexed_chapters() -> Vec<IndexedChapter> {
        vec![
            indexed_chapter(1, "0.01"),
            indexed_chapter(2, "0.02"),
            indexed_chapter(3, "133"),
        ]
    }

    fn indexed_chapter(index: usize, key: &str) -> IndexedChapter {
        IndexedChapter {
            index,
            key: key.to_string(),
            display_label: format!("Chapter {key}"),
            chapters: vec![ChapterEntry {
                id: format!("chapter-{key}"),
                chapter_number: Some(key.to_string()),
                title: None,
                translated_language: "pt-br".to_string(),
                pages: 1,
                external_url: None,
            }],
        }
    }
}
