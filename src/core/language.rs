//! Idiomas aceitos pelo downloader.
//!
//! Hoje a CLI expõe apenas `pt-br`, mas o domínio usa um enum para que novos
//! idiomas possam ser adicionados sem trocar strings soltas pelo código.

use crate::infra::{AppError, Result};

/// Idioma de tradução solicitado para o feed de capítulos.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    /// Português do Brasil, código MangaDex `pt-br`.
    PortugueseBrazil,
}

impl Language {
    /// Código esperado pelo parâmetro `translatedLanguage[]` da API.
    pub fn code(self) -> &'static str {
        match self {
            Self::PortugueseBrazil => "pt-br",
        }
    }

    /// Nome legível exibido nos prompts da CLI.
    pub fn label(self) -> &'static str {
        match self {
            Self::PortugueseBrazil => "Portuguese (Brazil)",
        }
    }
}

/// Lista de idiomas disponíveis para prompt e validação.
pub fn supported_languages() -> &'static [Language] {
    &[Language::PortugueseBrazil]
}

/// Converte entrada textual em um idioma suportado.
///
/// Mantém aliases simples para facilitar uso manual, mas rejeita qualquer
/// idioma ainda não implementado.
pub fn parse_language(input: &str) -> Result<Language> {
    let normalized = input.trim().to_ascii_lowercase();

    match normalized.as_str() {
        "pt-br" | "ptbr" | "portuguese (brazil)" | "portuguese-brazil" => {
            Ok(Language::PortugueseBrazil)
        }
        _ => Err(AppError::InvalidInput(format!(
            "unsupported language '{input}'. Currently supported: pt-br"
        ))),
    }
}
