//! Modelos desserializados das respostas do MangaDex.
//!
//! Estes tipos representam apenas os campos usados pela aplicação. Campos
//! extras retornados pela API são ignorados pelo `serde`.

use std::collections::HashMap;

use serde::Deserialize;

/// Envelope de resposta para endpoints que retornam um único recurso.
#[derive(Clone, Debug, Deserialize)]
pub struct SingleResponse<T> {
    /// Resultado textual retornado pela API, normalmente `ok`.
    pub result: String,
    /// Recurso retornado pelo endpoint.
    pub data: T,
}

/// Envelope de resposta para endpoints paginados.
#[derive(Clone, Debug, Deserialize)]
pub struct CollectionResponse<T> {
    /// Resultado textual retornado pela API, normalmente `ok`.
    pub result: String,
    /// Tipo de resposta informado pela API quando presente.
    pub response: Option<String>,
    /// Página atual de recursos.
    pub data: Vec<T>,
    /// Limite usado na página.
    pub limit: u32,
    /// Offset usado na página.
    pub offset: u32,
    /// Total de recursos disponíveis para a consulta.
    pub total: u32,
}

/// Formato genérico de recurso MangaDex.
#[derive(Clone, Debug, Deserialize)]
pub struct Resource<T> {
    /// UUID do recurso.
    pub id: String,
    /// Tipo do recurso, vindo do campo JSON `type`.
    #[serde(rename = "type")]
    pub resource_type: String,
    /// Atributos específicos do recurso.
    pub attributes: T,
    /// Relacionamentos incluídos ou referenciados pela resposta.
    #[serde(default)]
    pub relationships: Vec<Relationship>,
}

/// Recurso de mangá com atributos de mangá.
pub type MangaResource = Resource<MangaAttributes>;
/// Recurso de capítulo com atributos de capítulo.
pub type ChapterResource = Resource<ChapterAttributes>;
/// Recurso de cover com atributos de cover.
pub type CoverResource = Resource<CoverAttributes>;

/// Relacionamento entre recursos do MangaDex.
#[derive(Clone, Debug, Deserialize)]
pub struct Relationship {
    /// UUID do recurso relacionado.
    pub id: String,
    /// Tipo do recurso relacionado, vindo do campo JSON `type`.
    #[serde(rename = "type")]
    pub resource_type: String,
}

/// Atributos usados do recurso de mangá.
#[derive(Clone, Debug, Deserialize)]
pub struct MangaAttributes {
    /// Títulos por código de idioma.
    pub title: HashMap<String, String>,
}

/// Atributos usados do recurso de cover.
#[derive(Clone, Debug, Deserialize)]
pub struct CoverAttributes {
    /// Nome do arquivo de cover no MangaDex.
    #[serde(rename = "fileName")]
    pub file_name: String,
}

/// Atributos usados do recurso de capítulo.
#[derive(Clone, Debug, Deserialize)]
pub struct ChapterAttributes {
    /// Número real do capítulo, como `133` ou `0.01`.
    pub chapter: Option<String>,
    /// Título opcional do capítulo.
    pub title: Option<String>,
    /// Idioma da tradução.
    #[serde(rename = "translatedLanguage")]
    pub translated_language: String,
    /// Quantidade de páginas informada pela API.
    pub pages: Option<u32>,
    /// URL externa quando o capítulo não está hospedado no MangaDex.
    #[serde(rename = "externalUrl")]
    pub external_url: Option<String>,
}

/// Resposta do endpoint AtHome.
#[derive(Clone, Debug, Deserialize)]
pub struct AtHomeResponse {
    /// Resultado textual retornado pela API, normalmente `ok`.
    pub result: String,
    /// URL base temporária para baixar imagens do capítulo.
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    /// Dados de página e hash do capítulo.
    pub chapter: AtHomeChapter,
}

/// Dados de imagem retornados pelo AtHome.
#[derive(Clone, Debug, Deserialize)]
pub struct AtHomeChapter {
    /// Hash usado no caminho das imagens.
    pub hash: String,
    /// Arquivos de imagem em qualidade completa.
    pub data: Vec<String>,
    /// Arquivos de imagem em modo econômico.
    #[serde(rename = "dataSaver")]
    pub data_saver: Vec<String>,
}
