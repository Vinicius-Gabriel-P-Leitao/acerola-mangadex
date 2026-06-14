//! Serviço de download do mangá.
//!
//! Este módulo coordena o fluxo principal da aplicação:
//!
//! 1. resolve o ID do mangá;
//! 2. carrega metadados, cover e capítulos filtrados por idioma;
//! 3. cria a pasta local;
//! 4. baixa a cover;
//! 5. resolve a seleção do usuário;
//! 6. baixa páginas e grava cada capítulo em CBZ.
//!
//! A escrita usa arquivos `.part` para reduzir a chance de deixar arquivos CBZ
//! corrompidos quando uma falha acontece no meio do download.

use std::io::Write;
use std::path::{Path, PathBuf};

use tokio::fs;
use tracing::{error, info, warn};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::core::archive::{
    ChapterFileNameFormat, chapter_cbz_file_name, cover_file_name, page_entry_name,
};
use crate::core::chapter::{ChapterAvailability, IndexedChapter, index_chapters};
use crate::core::filesystem::sanitize_folder_name;
use crate::core::language::Language;
use crate::core::manga_link::extract_manga_id;
use crate::core::selection::ChapterSelection;
use crate::core::title::choose_manga_title;
use crate::data::{AtHomeResponse, MangaDexClient};
use crate::infra::{AppError, Result};

/// Serviço de alto nível que executa downloads usando o cliente MangaDex.
#[derive(Clone)]
pub struct MangaDownloadService {
    /// Cliente HTTP e regras de acesso à API.
    client: MangaDexClient,
}

impl MangaDownloadService {
    /// Cria um serviço com o cliente já configurado.
    pub fn new(client: MangaDexClient) -> Self {
        Self { client }
    }

    /// Carrega o catálogo necessário antes de perguntar ou executar a seleção.
    ///
    /// O catálogo já vem filtrado pelo idioma na chamada ao feed do MangaDex.
    /// Isso evita baixar páginas de metadados desnecessárias para outros
    /// idiomas.
    pub async fn load_catalog(
        &self,
        manga_link: &str,
        language: Language,
    ) -> Result<DownloadCatalog> {
        let manga_id = extract_manga_id(manga_link)?;
        let manga = self.client.get_manga(&manga_id).await?;
        let title = choose_manga_title(&manga.attributes);
        let cover = self.resolve_cover(&manga_id, &manga.relationships).await?;
        let chapters = index_chapters(
            self.client
                .get_all_chapters(&manga_id, language.code())
                .await?,
        );

        Ok(DownloadCatalog {
            manga_id,
            cover,
            folder_name: sanitize_folder_name(&title),
            title,
            language,
            chapters,
        })
    }

    /// Baixa os capítulos selecionados para o diretório informado.
    ///
    /// O diretório final sempre é `output_dir/folder_name`. Capítulos existentes
    /// são pulados, capítulos sem idioma/sem páginas/externos viram contadores
    /// no relatório e erros individuais não interrompem o restante da seleção.
    pub async fn download_selection(
        &self,
        catalog: &DownloadCatalog,
        output_dir: &Path,
        selection: &ChapterSelection,
        file_name_format: ChapterFileNameFormat,
    ) -> Result<DownloadReport> {
        let selected_indices = selection.resolve(&catalog.chapters)?;
        let manga_dir = output_dir.join(&catalog.folder_name);
        fs::create_dir_all(&manga_dir).await?;

        let mut report = DownloadReport {
            cover: self.download_cover(catalog, &manga_dir).await,
            ..DownloadReport::default()
        };

        for index in selected_indices {
            let chapter = catalog.chapters.get(index - 1).ok_or_else(|| {
                AppError::InvalidInput(format!("chapter index {index} is not in the catalog"))
            })?;

            match self
                .download_chapter(catalog, chapter, &manga_dir, file_name_format)
                .await
            {
                Ok(DownloadOutcome::Downloaded(path)) => {
                    info!(path = %path.display(), "Downloaded chapter");
                    report.downloaded += 1;
                }
                Ok(DownloadOutcome::SkippedExisting(path)) => {
                    info!(path = %path.display(), "Skipping existing CBZ");
                    report.skipped_existing += 1;
                }
                Ok(DownloadOutcome::SkippedMissingLanguage) => {
                    info!(
                        index = chapter.index,
                        label = chapter.display_label,
                        language = catalog.language.code(),
                        "Skipping chapter because the requested language is not available"
                    );
                    report.skipped_missing_language += 1;
                }
                Ok(DownloadOutcome::SkippedExternalOnly) => {
                    info!(
                        index = chapter.index,
                        label = chapter.display_label,
                        language = catalog.language.code(),
                        "Skipping chapter because the requested language points to an external source"
                    );
                    report.skipped_external += 1;
                }
                Ok(DownloadOutcome::SkippedNoPages) => {
                    info!(
                        index = chapter.index,
                        label = chapter.display_label,
                        language = catalog.language.code(),
                        "Skipping chapter because MangaDex returned no page files"
                    );
                    report.skipped_no_pages += 1;
                }
                Ok(DownloadOutcome::SkippedMissingChapterNumber) => {
                    info!(
                        index = chapter.index,
                        label = chapter.display_label,
                        "Skipping chapter because MangaDex did not provide a chapter number"
                    );
                    report.skipped_missing_chapter_number += 1;
                }
                Err(error) => {
                    error!(
                        index = chapter.index,
                        label = chapter.display_label,
                        %error,
                        "Chapter download failed"
                    );
                    report.failed.push(DownloadFailure {
                        index: chapter.index,
                        label: chapter.display_label.clone(),
                        message: error.to_string(),
                    });
                }
            }
        }

        Ok(report)
    }

    /// Resolve a cover vinculada ao mangá.
    ///
    /// O endpoint de mangá retorna apenas o relacionamento. A aplicação consulta
    /// o recurso de cover para obter o `fileName` e montar a URL de upload.
    async fn resolve_cover(
        &self,
        manga_id: &str,
        relationships: &[crate::data::Relationship],
    ) -> Result<Option<CoverImage>> {
        let cover_id = relationships
            .iter()
            .find(|relationship| relationship.resource_type == "cover_art")
            .map(|relationship| relationship.id.as_str());
        let Some(cover_id) = cover_id else {
            return Ok(None);
        };

        let cover = self.client.get_cover(cover_id).await?;
        let url = self
            .client
            .cover_image_url(manga_id, &cover.attributes.file_name);

        Ok(Some(CoverImage {
            file_name: cover.attributes.file_name,
            url,
        }))
    }

    /// Baixa a cover quando ela está disponível e ainda não existe localmente.
    ///
    /// Falhas de cover são registradas no relatório, mas não impedem o download
    /// dos capítulos.
    async fn download_cover(&self, catalog: &DownloadCatalog, manga_dir: &Path) -> CoverStatus {
        let Some(cover) = &catalog.cover else {
            return CoverStatus::Unavailable;
        };

        let final_path = manga_dir.join(cover_file_name(&cover.file_name));
        match fs::try_exists(&final_path).await {
            Ok(true) => return CoverStatus::SkippedExisting(final_path),
            Ok(false) => {}
            Err(error) => {
                error!(
                    path = %final_path.display(),
                    %error,
                    "Failed to inspect existing cover"
                );
                return CoverStatus::Failed(error.to_string());
            }
        }

        let result = self.download_cover_file(&cover.url, &final_path).await;
        match result {
            Ok(()) => CoverStatus::Downloaded(final_path),
            Err(error) => {
                error!(
                    url = cover.url,
                    path = %final_path.display(),
                    %error,
                    "Cover download failed"
                );
                CoverStatus::Failed(error.to_string())
            }
        }
    }

    /// Baixa a cover usando escrita temporária com `.part`.
    async fn download_cover_file(&self, url: &str, final_path: &Path) -> Result<()> {
        let part_path = part_path(final_path);
        let result = self
            .download_cover_file_part(url, final_path, &part_path)
            .await;

        match result {
            Ok(()) => Ok(()),
            Err(error) => {
                cleanup_partial_file(&part_path).await;
                Err(error)
            }
        }
    }

    /// Grava o arquivo temporário da cover e renomeia para o destino final.
    async fn download_cover_file_part(
        &self,
        url: &str,
        final_path: &Path,
        part_path: &Path,
    ) -> Result<()> {
        cleanup_partial_file(part_path).await;

        let bytes = self.client.download_image(url).await?;
        fs::write(&part_path, bytes).await?;
        fs::rename(&part_path, final_path).await?;
        Ok(())
    }

    /// Baixa um capítulo indexado quando há uma entrada válida para o idioma.
    ///
    /// A função decide todos os casos de skip antes de chamar o AtHome: idioma
    /// ausente, capítulo externo, número ausente e CBZ já existente.
    async fn download_chapter(
        &self,
        catalog: &DownloadCatalog,
        indexed: &IndexedChapter,
        manga_dir: &Path,
        file_name_format: ChapterFileNameFormat,
    ) -> Result<DownloadOutcome> {
        let chapter = match indexed.downloadable_for_language(catalog.language.code()) {
            ChapterAvailability::Available(chapter) => chapter,
            ChapterAvailability::ExternalOnly => return Ok(DownloadOutcome::SkippedExternalOnly),
            ChapterAvailability::MissingLanguage => {
                return Ok(DownloadOutcome::SkippedMissingLanguage);
            }
        };
        let Some(cbz_file_name) = chapter_cbz_file_name(
            file_name_format,
            chapter.chapter_number.as_deref(),
            chapter.title.as_deref(),
        ) else {
            return Ok(DownloadOutcome::SkippedMissingChapterNumber);
        };
        let cbz_path = manga_dir.join(cbz_file_name);

        if fs::try_exists(&cbz_path).await? {
            return Ok(DownloadOutcome::SkippedExisting(cbz_path));
        }

        let at_home = self.client.get_at_home_server(&chapter.id).await?;
        if at_home.chapter.data.is_empty() {
            return Ok(DownloadOutcome::SkippedNoPages);
        }

        self.write_cbz(&at_home, &cbz_path).await?;
        Ok(DownloadOutcome::Downloaded(cbz_path))
    }

    /// Escreve um CBZ final usando arquivo `.part`.
    ///
    /// O arquivo temporário só é renomeado após o ZIP terminar com sucesso.
    async fn write_cbz(&self, at_home: &AtHomeResponse, final_path: &Path) -> Result<()> {
        let part_path = part_path(final_path);
        let write_result = self.write_cbz_part(at_home, &part_path).await;

        match write_result {
            Ok(()) => {
                fs::rename(&part_path, final_path).await?;
                Ok(())
            }
            Err(error) => {
                cleanup_partial_file(&part_path).await;
                Err(error)
            }
        }
    }

    /// Baixa as páginas e grava cada uma como entrada do arquivo ZIP.
    ///
    /// O CBZ é um ZIP com extensão `.cbz`. A compressão é `Stored` porque as
    /// imagens já chegam comprimidas e recomprimir só aumenta custo de CPU.
    async fn write_cbz_part(&self, at_home: &AtHomeResponse, part_path: &Path) -> Result<()> {
        cleanup_partial_file(part_path).await;

        let file = std::fs::File::create(part_path)?;
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        let total_pages = at_home.chapter.data.len();

        for (position, source_name) in at_home.chapter.data.iter().enumerate() {
            let url = page_url(at_home, source_name);
            let bytes = self.client.download_image(&url).await?;
            let entry_name = page_entry_name(position + 1, total_pages, source_name);
            zip.start_file(entry_name, options)?;
            zip.write_all(&bytes)?;
        }

        zip.finish()?;
        Ok(())
    }
}

/// Catálogo carregado para um mangá e idioma.
#[derive(Clone, Debug)]
pub struct DownloadCatalog {
    /// UUID do mangá no MangaDex.
    pub manga_id: String,
    /// Cover resolvida, quando o mangá possui relacionamento de cover.
    pub cover: Option<CoverImage>,
    /// Título escolhido para exibição.
    pub title: String,
    /// Nome sanitizado da pasta local do mangá.
    pub folder_name: String,
    /// Idioma usado na busca de capítulos.
    pub language: Language,
    /// Capítulos agrupados e ordenados para seleção.
    pub chapters: Vec<IndexedChapter>,
}

impl DownloadCatalog {
    /// Conta quantos capítulos têm arquivo baixável para o idioma do catálogo.
    pub fn available_count(&self) -> usize {
        self.chapters
            .iter()
            .filter(|chapter| {
                matches!(
                    chapter.downloadable_for_language(self.language.code()),
                    ChapterAvailability::Available(_)
                )
            })
            .count()
    }
}

/// Dados necessários para baixar a cover.
#[derive(Clone, Debug)]
pub struct CoverImage {
    /// Nome original do arquivo no MangaDex.
    pub file_name: String,
    /// URL final no host de uploads.
    pub url: String,
}

/// Resumo da execução de download.
#[derive(Clone, Debug, Default)]
pub struct DownloadReport {
    /// Resultado do download da cover.
    pub cover: CoverStatus,
    /// Quantidade de capítulos baixados nesta execução.
    pub downloaded: usize,
    /// Quantidade de capítulos pulados porque o CBZ já existia.
    pub skipped_existing: usize,
    /// Quantidade de capítulos sem tradução no idioma solicitado.
    pub skipped_missing_language: usize,
    /// Quantidade de capítulos sem número real para formar o nome do CBZ.
    pub skipped_missing_chapter_number: usize,
    /// Quantidade de capítulos que apontavam apenas para fonte externa.
    pub skipped_external: usize,
    /// Quantidade de capítulos sem páginas retornadas pelo AtHome.
    pub skipped_no_pages: usize,
    /// Falhas individuais de capítulo que não interromperam a seleção inteira.
    pub failed: Vec<DownloadFailure>,
}

/// Estado final da cover no relatório.
#[derive(Clone, Debug, Default)]
pub enum CoverStatus {
    /// Cover baixada nesta execução.
    Downloaded(PathBuf),
    /// Tentativa de cover falhou, mas os capítulos puderam continuar.
    Failed(String),
    /// Cover já existia no disco.
    SkippedExisting(PathBuf),
    /// O mangá não tinha cover resolvível.
    #[default]
    Unavailable,
}

/// Falha individual de capítulo.
#[derive(Clone, Debug)]
pub struct DownloadFailure {
    /// Índice 1-based do capítulo no catálogo.
    pub index: usize,
    /// Texto de identificação usado no terminal.
    pub label: String,
    /// Mensagem de erro capturada.
    pub message: String,
}

/// Resultado interno de uma tentativa de capítulo.
enum DownloadOutcome {
    /// Capítulo foi gravado no caminho final.
    Downloaded(PathBuf),
    /// O arquivo final já existia.
    SkippedExisting(PathBuf),
    /// O capítulo existe apenas fora do MangaDex.
    SkippedExternalOnly,
    /// Não há tradução no idioma solicitado.
    SkippedMissingLanguage,
    /// Não há número de capítulo para formar nome de arquivo.
    SkippedMissingChapterNumber,
    /// O AtHome retornou uma lista vazia de páginas.
    SkippedNoPages,
}

/// Calcula o caminho temporário ao lado do arquivo final.
fn part_path(final_path: &Path) -> PathBuf {
    let file_name = final_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("chapter.cbz");
    final_path.with_file_name(format!("{file_name}.part"))
}

/// Monta a URL de uma página a partir da resposta AtHome.
fn page_url(at_home: &AtHomeResponse, source_name: &str) -> String {
    format!(
        "{}/data/{}/{}",
        at_home.base_url.trim_end_matches('/'),
        at_home.chapter.hash,
        source_name
    )
}

/// Remove arquivo temporário, registrando erro sem derrubar o fluxo.
async fn cleanup_partial_file(path: &Path) {
    match fs::try_exists(path).await {
        Ok(true) => match fs::remove_file(path).await {
            Ok(()) => {}
            Err(error) => warn!(path = %path.display(), %error, "Failed to remove partial CBZ"),
        },
        Ok(false) => {}
        Err(error) => warn!(path = %path.display(), %error, "Failed to inspect partial CBZ"),
    }
}

#[cfg(test)]
mod tests {
    use super::part_path;
    use std::path::Path;

    #[test]
    fn builds_partial_path_next_to_final_file() {
        assert_eq!(
            part_path(Path::new("Berserk/001.cbz")),
            Path::new("Berserk/001.cbz.part")
        );
    }
}
