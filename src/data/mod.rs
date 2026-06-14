//! Camada de dados.
//!
//! Esta camada conhece os endpoints e o formato das respostas do MangaDex. O
//! restante da aplicação consome tipos já desserializados e não precisa montar
//! URLs ou parâmetros de API diretamente.

/// Cliente HTTP específico para o MangaDex.
pub mod mangadex;
/// Modelos mínimos das respostas usadas pela aplicação.
pub mod models;

/// Cliente MangaDex usado pelos serviços do domínio.
pub use mangadex::MangaDexClient;
/// Reexporta os modelos externos usados fora desta camada.
pub use models::{
    AtHomeChapter, AtHomeResponse, ChapterAttributes, ChapterResource, CollectionResponse,
    CoverAttributes, CoverResource, MangaAttributes, MangaResource, Relationship, SingleResponse,
};
