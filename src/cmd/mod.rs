//! Camada de comando da aplicação.
//!
//! Este módulo é responsável por transformar entrada de terminal em chamadas
//! para o domínio da aplicação. Ele usa:
//!
//! - [`clap`] para declarar e ler argumentos de linha de comando a partir da
//!   struct `Cli`.
//! - [`dialoguer`] para perguntar interativamente os valores que não vieram por
//!   argumento.
//! - [`tracing_subscriber`] para configurar logs no início da execução.
//!
//! A regra de negócio não fica aqui. O módulo apenas coleta dados, monta as
//! dependências e chama `MangaDownloadService`.

use std::path::{Path, PathBuf};

use clap::Parser;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Select};
use tracing_subscriber::EnvFilter;

use crate::core::archive::{parse_chapter_file_name_format, supported_chapter_file_name_formats};
use crate::core::downloader::CoverStatus;
use crate::core::language::{parse_language, supported_languages};
use crate::core::selection::{parse_chapter_number_selection, parse_selection};
use crate::core::{ChapterFileNameFormat, ChapterSelection, Language, MangaDownloadService};
use crate::data::MangaDexClient;
use crate::infra::{AppConfig, AppError, Result};

impl From<dialoguer::Error> for AppError {
    /// Converte erros do `dialoguer` para o tipo de erro único da aplicação.
    ///
    /// Isso permite usar `?` em prompts interativos sem espalhar tipos de erro
    /// da biblioteca de CLI fora da camada `cmd`.
    fn from(error: dialoguer::Error) -> Self {
        Self::Prompt(error.to_string())
    }
}

/// Argumentos aceitos pela CLI.
///
/// A derive [`Parser`] vem do `clap`. Ela lê os atributos `#[command(...)]` e
/// `#[arg(...)]` para gerar:
///
/// - parsing de argumentos;
/// - `--help`;
/// - `--version`;
/// - validação simples, como conflitos entre flags.
///
/// O atributo `#[command(author, version, about = "...")]` configura metadados
/// do comando principal. `author` e `version` são puxados do `Cargo.toml`; o
/// `about` é o texto curto exibido no help.
#[derive(Debug, Parser)]
#[command(author, version, about = "Download MangaDex chapters as CBZ files")]
struct Cli {
    /// Link do título no MangaDex ou UUID cru do mangá.
    ///
    /// Como não tem `short` nem `long`, este campo é um argumento posicional.
    /// Se não for informado, a CLI pergunta interativamente.
    #[arg(value_name = "MANGADEX_TITLE_URL")]
    link: Option<String>,
    /// Diretório base onde a pasta do mangá será criada.
    ///
    /// `short, long` faz o `clap` aceitar `-o` e `--output`.
    #[arg(short, long, value_name = "DIR")]
    output: Option<PathBuf>,
    /// Seleção por índice da lista carregada.
    ///
    /// Exemplos: `all`, `7`, `100-200`.
    ///
    /// Este modo conflita com `chapter`, porque uma seleção como `133` seria
    /// ambígua se índice e número real do capítulo fossem aceitos juntos.
    #[arg(
        short,
        long,
        value_name = "all|INDEX|START-END",
        conflicts_with = "chapter"
    )]
    selection: Option<String>,
    /// Seleção pelo número real do capítulo no MangaDex.
    ///
    /// Exemplos: `133`, `0.01`, `100-200`.
    ///
    /// Este modo usa `attributes.chapter`, não a posição na lista.
    #[arg(
        short = 'c',
        long,
        value_name = "CHAPTER|START-END",
        conflicts_with = "selection"
    )]
    chapter: Option<String>,
    /// Idioma dos capítulos a buscar no feed do MangaDex.
    ///
    /// Hoje a aplicação aceita apenas `pt-br`. Se não for informado, a CLI
    /// mostra um menu interativo.
    #[arg(short, long, value_name = "LANGUAGE")]
    language: Option<String>,
    /// Formato do nome dos arquivos CBZ.
    ///
    /// Valores aceitos:
    ///
    /// - `chapter-title`: `Ch. 163 - A Sombra de uma Ideia (1).cbz`
    /// - `number`: `163.cbz`
    ///
    /// Se não for informado, a CLI mostra um menu interativo.
    #[arg(long, value_name = "chapter-title|number")]
    name_format: Option<String>,
}

/// Ponto de entrada da camada de comando.
///
/// Este método não é "só configuração", mas ele faz a orquestração da CLI:
///
/// 1. inicializa logs;
/// 2. lê argumentos com `Cli::parse()`;
/// 3. pergunta valores ausentes com `dialoguer`;
/// 4. cria config, cliente MangaDex e serviço de download;
/// 5. carrega o catálogo;
/// 6. executa o download;
/// 7. imprime o relatório final.
///
/// Ele fica separado do `main` para manter o binário pequeno e deixar a lógica
/// testável/importável pela biblioteca.
pub async fn run() -> Result<()> {
    init_tracing();

    let cli = Cli::parse();
    let theme = ColorfulTheme::default();
    let output_dir = resolve_output_dir(cli.output, &theme)?;
    let link = resolve_link(cli.link, &theme)?;
    let language = resolve_language(cli.language, &theme)?;
    let file_name_format = resolve_file_name_format(cli.name_format, &theme)?;

    let config = AppConfig::default();
    let client = MangaDexClient::new(config.mangadex)?;
    let service = MangaDownloadService::new(client);

    println!("Loading MangaDex catalog...");
    let catalog = service.load_catalog(&link, language).await?;
    println!("Manga: {}", catalog.title);
    println!("Manga ID: {}", catalog.manga_id);
    println!(
        "Indexed {} chapters: {}",
        catalog.language.code(),
        catalog.chapters.len()
    );
    println!(
        "Downloadable {} chapters: {}",
        catalog.language.code(),
        catalog.available_count()
    );

    let selection = resolve_selection(cli.selection, cli.chapter, catalog.chapters.len(), &theme)?;
    let report = service
        .download_selection(&catalog, &output_dir, &selection, file_name_format)
        .await?;
    print_report(&catalog.folder_name, &output_dir, &report);

    Ok(())
}

/// Inicializa o sistema de logs da aplicação.
///
/// A variável de ambiente `RUST_LOG` pode sobrescrever o filtro padrão. Sem
/// essa variável, a aplicação usa nível `info`.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    match tracing_subscriber::fmt()
        .with_env_filter(filter)
        .without_time()
        .try_init()
    {
        Ok(()) => {}
        Err(error) => eprintln!("Logger initialization skipped: {error}"),
    }
}

/// Resolve o diretório de saída.
///
/// Se `--output` foi passado, usa o argumento. Caso contrário, pergunta no
/// terminal.
fn resolve_output_dir(output: Option<PathBuf>, theme: &ColorfulTheme) -> Result<PathBuf> {
    match output {
        Some(path) => Ok(path),
        None => {
            let input = Input::<String>::with_theme(theme)
                .with_prompt("Output directory")
                .interact_text()?;
            Ok(PathBuf::from(input))
        }
    }
}

/// Resolve o link/UUID do mangá.
///
/// Se o argumento posicional foi passado, usa o argumento. Caso contrário,
/// pergunta no terminal.
fn resolve_link(link: Option<String>, theme: &ColorfulTheme) -> Result<String> {
    match link {
        Some(link) => Ok(link),
        None => Ok(Input::<String>::with_theme(theme)
            .with_prompt("MangaDex title URL")
            .interact_text()?),
    }
}

/// Resolve o idioma dos capítulos.
///
/// Argumento explícito tem prioridade sobre prompt interativo.
fn resolve_language(language: Option<String>, theme: &ColorfulTheme) -> Result<Language> {
    match language {
        Some(language) => parse_language(&language),
        None => prompt_language(theme),
    }
}

/// Exibe o menu de idioma suportado.
///
/// Hoje existe só `pt-br`, mas o fluxo já está preparado para listar novos
/// idiomas se forem adicionados no core.
fn prompt_language(theme: &ColorfulTheme) -> Result<Language> {
    let languages = supported_languages();
    let labels = languages
        .iter()
        .map(|language| format!("{} ({})", language.label(), language.code()))
        .collect::<Vec<_>>();
    let selected = Select::with_theme(theme)
        .with_prompt("Language")
        .items(&labels)
        .default(0)
        .interact()?;
    let selected_language = languages
        .get(selected)
        .ok_or_else(|| AppError::InvalidInput(format!("invalid language menu index {selected}")))?;

    Ok(*selected_language)
}

/// Resolve o formato do nome dos arquivos CBZ.
///
/// Argumento explícito tem prioridade sobre prompt interativo.
fn resolve_file_name_format(
    file_name_format: Option<String>,
    theme: &ColorfulTheme,
) -> Result<ChapterFileNameFormat> {
    match file_name_format {
        Some(file_name_format) => parse_chapter_file_name_format(&file_name_format),
        None => prompt_file_name_format(theme),
    }
}

/// Exibe o menu de formato de nome do arquivo CBZ.
///
/// O menu mostra exemplos reais para deixar claro o efeito de cada opção.
fn prompt_file_name_format(theme: &ColorfulTheme) -> Result<ChapterFileNameFormat> {
    let formats = supported_chapter_file_name_formats();
    let labels = formats
        .iter()
        .map(|format| format!("{} ({})", format.label(), format.code()))
        .collect::<Vec<_>>();
    let selected = Select::with_theme(theme)
        .with_prompt("Chapter file name format")
        .items(&labels)
        .default(0)
        .interact()?;
    let selected_format = formats.get(selected).ok_or_else(|| {
        AppError::InvalidInput(format!("invalid file name format menu index {selected}"))
    })?;

    Ok(*selected_format)
}

/// Resolve a seleção de capítulos.
///
/// Ordem de prioridade:
///
/// 1. `--selection`, para índice;
/// 2. `--chapter`, para número real do capítulo;
/// 3. prompts interativos.
fn resolve_selection(
    raw_selection: Option<String>,
    raw_chapter: Option<String>,
    total: usize,
    theme: &ColorfulTheme,
) -> Result<ChapterSelection> {
    match raw_selection {
        Some(selection) => parse_selection(&selection, total),
        None => match raw_chapter {
            Some(chapter) => parse_chapter_number_selection(&chapter),
            None => prompt_selection(total, theme),
        },
    }
}

/// Primeiro menu de seleção interativa.
///
/// `All chapters` aparece antes da escolha por índice ou número porque baixar
/// todos os capítulos não depende de nenhum desses modos.
fn prompt_selection(total: usize, theme: &ColorfulTheme) -> Result<ChapterSelection> {
    let modes = ["All chapters", "Index", "Chapter number"];
    let selected_mode = Select::with_theme(theme)
        .with_prompt("Download by")
        .items(&modes)
        .default(0)
        .interact()?;

    match selected_mode {
        0 => Ok(ChapterSelection::All),
        1 => prompt_index_selection(total, theme),
        2 => prompt_chapter_number_selection(theme),
        _ => Err(AppError::InvalidInput(format!(
            "invalid download mode menu index {selected_mode}"
        ))),
    }
}

/// Menu para seleção por índice.
///
/// Índice é a posição do capítulo na lista filtrada pelo idioma escolhido.
fn prompt_index_selection(total: usize, theme: &ColorfulTheme) -> Result<ChapterSelection> {
    let options = ["Single chapter index", "Chapter index range"];
    let selected = Select::with_theme(theme)
        .with_prompt("Index download mode")
        .items(&options)
        .default(0)
        .interact()?;

    match selected {
        0 => {
            let input = Input::<String>::with_theme(theme)
                .with_prompt(format!("Chapter index (1-{total})"))
                .interact_text()?;
            parse_selection(&input, total)
        }
        1 => {
            let input = Input::<String>::with_theme(theme)
                .with_prompt(format!("Chapter range (example: 100-{total})"))
                .interact_text()?;
            parse_selection(&input, total)
        }
        _ => Err(AppError::InvalidInput(format!(
            "invalid index download mode menu index {selected}"
        ))),
    }
}

/// Menu para seleção pelo número real do capítulo.
///
/// Número real é o valor de `attributes.chapter` retornado pela API do
/// MangaDex.
fn prompt_chapter_number_selection(theme: &ColorfulTheme) -> Result<ChapterSelection> {
    let options = ["Single chapter number", "Chapter number range"];
    let selected = Select::with_theme(theme)
        .with_prompt("Chapter number download mode")
        .items(&options)
        .default(0)
        .interact()?;

    match selected {
        0 => {
            let input = Input::<String>::with_theme(theme)
                .with_prompt("Chapter number (example: 133 or 0.01)")
                .interact_text()?;
            parse_chapter_number_selection(&input)
        }
        1 => {
            let input = Input::<String>::with_theme(theme)
                .with_prompt("Chapter number range (example: 100-200)")
                .interact_text()?;
            parse_chapter_number_selection(&input)
        }
        _ => Err(AppError::InvalidInput(format!(
            "invalid chapter number download mode menu index {selected}"
        ))),
    }
}

/// Imprime um resumo da execução.
///
/// O relatório mostra downloads, pulos e falhas de capítulo, além do status da
/// cover.
fn print_report(folder_name: &str, output_dir: &Path, report: &crate::core::DownloadReport) {
    println!();
    println!("Output: {}", output_dir.join(folder_name).display());
    print_cover_status(&report.cover);
    println!("Downloaded: {}", report.downloaded);
    println!("Skipped existing: {}", report.skipped_existing);
    println!(
        "Skipped missing language: {}",
        report.skipped_missing_language
    );
    println!(
        "Skipped missing chapter number: {}",
        report.skipped_missing_chapter_number
    );
    println!("Skipped external-only: {}", report.skipped_external);
    println!("Skipped no pages: {}", report.skipped_no_pages);
    println!("Failed: {}", report.failed.len());

    for failure in &report.failed {
        println!(
            "Failure index {} ({}): {}",
            failure.index, failure.label, failure.message
        );
    }
}

/// Imprime apenas o status da cover.
fn print_cover_status(status: &CoverStatus) {
    match status {
        CoverStatus::Downloaded(path) => println!("Cover: downloaded ({})", path.display()),
        CoverStatus::Failed(message) => println!("Cover: failed ({message})"),
        CoverStatus::SkippedExisting(path) => {
            println!("Cover: skipped existing ({})", path.display());
        }
        CoverStatus::Unavailable => println!("Cover: unavailable"),
    }
}
