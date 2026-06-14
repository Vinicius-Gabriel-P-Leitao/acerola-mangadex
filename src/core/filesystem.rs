//! Utilitários de caminho e nomes de pasta.
//!
//! A aplicação cria uma pasta por mangá dentro do diretório escolhido pelo
//! usuário. O nome precisa ser previsível e válido nos sistemas suportados.

/// Tamanho máximo do nome de pasta gerado para o mangá.
const MAX_FOLDER_NAME_CHARS: usize = 120;

/// Sanitiza o nome do mangá para uso como pasta.
///
/// Caracteres inválidos são substituídos por `_`, espaços repetidos são
/// colapsados e nomes reservados do Windows recebem um sufixo seguro.
pub fn sanitize_folder_name(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|character| match is_invalid_path_character(character) {
            true => '_',
            false => character,
        })
        .collect();

    let collapsed = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_matches([' ', '.']).trim().to_string();

    let candidate = match trimmed.is_empty() {
        true => "manga".to_string(),
        false => trimmed,
    };

    let reserved_safe = match is_reserved_windows_name(&candidate) {
        true => format!("{candidate}_"),
        false => candidate,
    };

    reserved_safe.chars().take(MAX_FOLDER_NAME_CHARS).collect()
}

/// Indica se um caractere é inválido para caminhos nos sistemas suportados.
fn is_invalid_path_character(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
        )
}

/// Protege nomes reservados do Windows mesmo quando há extensão.
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
    use super::sanitize_folder_name;

    #[test]
    fn replaces_cross_platform_invalid_characters() {
        assert_eq!(
            sanitize_folder_name("Berserk: Deluxe/Edition?"),
            "Berserk_ Deluxe_Edition_"
        );
    }

    #[test]
    fn protects_reserved_windows_names() {
        assert_eq!(sanitize_folder_name("CON"), "CON_");
    }

    #[test]
    fn falls_back_when_name_is_empty() {
        assert_eq!(sanitize_folder_name("..."), "manga");
    }
}
