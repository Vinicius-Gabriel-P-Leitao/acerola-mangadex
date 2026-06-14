//! Regras de nomeação de arquivos do download.
//!
//! O MangaDex fornece nomes de páginas e metadados de capítulo, mas esses
//! valores não são seguros para uso direto como caminhos. Este módulo centraliza
//! a conversão para nomes estáveis, compatíveis com Windows e Linux.

use crate::infra::{AppError, Result};

/// Formato escolhido para o nome final de cada arquivo CBZ.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChapterFileNameFormat {
    /// Usa número e título do capítulo quando o título existir.
    ///
    /// Exemplo: `Ch. 163 - A Sombra de uma Ideia (1).cbz`.
    ChapterTitle,
    /// Usa apenas o número real do capítulo retornado pela API.
    ///
    /// Exemplo: `163.cbz` ou `0.01.cbz`.
    NumberOnly,
}

impl ChapterFileNameFormat {
    /// Código usado em argumentos de CLI e parsing textual.
    pub fn code(self) -> &'static str {
        match self {
            Self::ChapterTitle => "chapter-title",
            Self::NumberOnly => "number",
        }
    }

    /// Texto exibido no menu interativo.
    ///
    /// O valor é um exemplo de arquivo para que o usuário veja o efeito da
    /// opção antes de iniciar o download.
    pub fn label(self) -> &'static str {
        match self {
            Self::ChapterTitle => "Ch. 163 - A Sombra de uma Ideia (1).cbz",
            Self::NumberOnly => "163.cbz",
        }
    }
}

/// Lista de formatos aceitos pela aplicação.
pub fn supported_chapter_file_name_formats() -> &'static [ChapterFileNameFormat] {
    &[
        ChapterFileNameFormat::ChapterTitle,
        ChapterFileNameFormat::NumberOnly,
    ]
}

/// Converte entrada textual para um formato de nome de capítulo.
///
/// A função aceita alguns aliases para facilitar uso via terminal, mas sempre
/// retorna um enum fechado para o restante do domínio.
pub fn parse_chapter_file_name_format(input: &str) -> Result<ChapterFileNameFormat> {
    let normalized = input.trim().to_ascii_lowercase();

    match normalized.as_str() {
        "chapter-title" | "chapter_title" | "title" | "full" | "ch-title" | "ch" => {
            Ok(ChapterFileNameFormat::ChapterTitle)
        }
        "number" | "number-only" | "number_only" | "chapter-number" | "163" | "163.cbz" => {
            Ok(ChapterFileNameFormat::NumberOnly)
        }
        _ => Err(AppError::InvalidInput(format!(
            "unsupported chapter file name format '{input}'. Supported formats: chapter-title, number"
        ))),
    }
}

/// Monta o nome final do arquivo CBZ para um capítulo.
///
/// A função retorna `None` quando a API não traz número de capítulo. Isso é
/// intencional: a aplicação não usa mais índice como fallback para evitar nomes
/// instáveis quando a lista muda.
pub fn chapter_cbz_file_name(
    format: ChapterFileNameFormat,
    chapter_number: Option<&str>,
    chapter_title: Option<&str>,
) -> Option<String> {
    let chapter_number = chapter_number
        .map(str::trim)
        .filter(|chapter_number| !chapter_number.is_empty())?;
    let raw_stem = match format {
        ChapterFileNameFormat::ChapterTitle => chapter_title_stem(chapter_number, chapter_title),
        ChapterFileNameFormat::NumberOnly => chapter_number.to_string(),
    };
    let stem = sanitize_file_stem(&raw_stem);

    match stem.is_empty() {
        true => None,
        false => Some(format!("{stem}.cbz")),
    }
}

/// Monta o nome de uma página dentro do CBZ.
///
/// O nome recebe padding baseado no total de páginas para manter a ordenação
/// lexicográfica correta em leitores de CBZ.
pub fn page_entry_name(position: usize, total: usize, source_name: &str) -> String {
    let width = total.to_string().len().max(1);
    let extension = safe_extension(source_name);

    format!("{position:0width$}.{extension}")
}

/// Monta o nome local da cover usando uma extensão segura.
pub fn cover_file_name(source_name: &str) -> String {
    format!("cover.{}", safe_extension(source_name))
}

/// Extrai e normaliza uma extensão segura de arquivo.
///
/// Extensões vazias ou com caracteres inesperados caem para `jpg`, evitando
/// que nomes remotos criem caminhos inválidos.
pub fn safe_extension(source_name: &str) -> String {
    let extension = source_name
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .unwrap_or("jpg");
    
    let valid = !extension.is_empty()
        && extension
            .chars()
            .all(|character| character.is_ascii_alphanumeric());

    match valid {
        true => extension.to_ascii_lowercase(),
        false => "jpg".to_string(),
    }
}

/// Monta o stem antes da sanitização e da extensão `.cbz`.
fn chapter_title_stem(chapter_number: &str, chapter_title: Option<&str>) -> String {
    let title = chapter_title
        .map(str::trim)
        .filter(|title| !title.is_empty());

    match title {
        Some(title) => format!("Ch. {chapter_number} - {title}"),
        None => format!("Ch. {chapter_number}"),
    }
}

/// Remove caracteres inválidos e nomes reservados do stem do arquivo.
///
/// A regra cobre Windows e Linux, mas é mais restritiva por causa das regras de
/// nomes reservados do Windows.
fn sanitize_file_stem(value: &str) -> String {
    let replaced = value
        .trim()
        .chars()
        .map(
            |character| match is_invalid_file_name_character(character) {
                true => '_',
                false => character,
            },
        )
        .collect::<String>();
    let trimmed = replaced.trim_matches([' ', '.']).to_string();

    if trimmed.is_empty() {
        return String::new();
    }

    match is_reserved_windows_name(&trimmed) {
        true => format!("{trimmed}_"),
        false => trimmed,
    }
}

/// Indica se um caractere não pode aparecer em um nome de arquivo local.
fn is_invalid_file_name_character(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
        )
}

/// Protege contra nomes especiais do Windows, como `CON` e `NUL`.
fn is_reserved_windows_name(name: &str) -> bool {
    let base_name = name.split('.').next().unwrap_or(name).to_ascii_uppercase();

    matches!(
        base_name.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

#[cfg(test)]
mod tests {
    use super::{ChapterFileNameFormat, chapter_cbz_file_name, page_entry_name};

    #[test]
    fn uses_number_only_cbz_file_name() {
        assert_eq!(
            chapter_cbz_file_name(ChapterFileNameFormat::NumberOnly, Some("133"), None),
            Some("133.cbz".to_string())
        );
    }

    #[test]
    fn keeps_decimal_chapter_number_for_cbz_file_name() {
        assert_eq!(
            chapter_cbz_file_name(ChapterFileNameFormat::NumberOnly, Some("0.01"), None),
            Some("0.01.cbz".to_string())
        );
    }

    #[test]
    fn uses_chapter_title_cbz_file_name() {
        assert_eq!(
            chapter_cbz_file_name(
                ChapterFileNameFormat::ChapterTitle,
                Some("163"),
                Some("A Sombra de uma Ideia (1)")
            ),
            Some("Ch. 163 - A Sombra de uma Ideia (1).cbz".to_string())
        );
    }

    #[test]
    fn omits_title_separator_when_title_is_missing() {
        assert_eq!(
            chapter_cbz_file_name(ChapterFileNameFormat::ChapterTitle, Some("163"), None),
            Some("Ch. 163.cbz".to_string())
        );
    }

    #[test]
    fn does_not_fall_back_to_index_when_chapter_number_is_missing() {
        assert_eq!(
            chapter_cbz_file_name(ChapterFileNameFormat::NumberOnly, None, None),
            None
        );
    }

    #[test]
    fn keeps_safe_page_extensions() {
        assert_eq!(page_entry_name(2, 12, "abc.PNG"), "02.png");
    }

    #[test]
    fn falls_back_to_jpg_for_unsafe_extensions() {
        assert_eq!(page_entry_name(2, 12, "abc.bad/ext"), "02.jpg");
    }
}
