//! Núcleo da aplicação.
//!
//! Esta camada concentra regras que não dependem de terminal nem de detalhes
//! específicos do cliente HTTP. Ela decide como capítulos são agrupados,
//! selecionados, nomeados e gravados em CBZ.

/// Montagem de nomes de arquivos e entradas do arquivo CBZ.
pub mod archive;
/// Agrupamento e disponibilidade de capítulos.
pub mod chapter;
/// Serviço que coordena catálogo, cover e download dos capítulos.
pub mod downloader;
/// Sanitização de nomes de pastas para Windows e Linux.
pub mod filesystem;
/// Idiomas suportados pela aplicação.
pub mod language;
/// Extração e validação de IDs a partir de links do MangaDex.
pub mod manga_link;
/// Parsing e resolução da seleção de capítulos.
pub mod selection;
/// Escolha do título principal do mangá.
pub mod title;

/// Formatos públicos de nome de arquivo CBZ.
pub use archive::ChapterFileNameFormat;
/// Tipos principais usados pela camada de comando.
pub use downloader::{DownloadCatalog, DownloadReport, MangaDownloadService};
/// Idioma selecionável na CLI.
pub use language::Language;
/// Seleção de capítulos por índice, intervalo ou número real.
pub use selection::ChapterSelection;
