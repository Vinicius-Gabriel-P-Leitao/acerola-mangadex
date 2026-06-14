//! Biblioteca da CLI `acerola-mangadex`.
//!
//! A biblioteca expõe as camadas principais da aplicação para que o binário
//! fique pequeno e a lógica possa ser testada sem passar pelo executável.
//!
//! A organização segue quatro responsabilidades:
//!
//! - `cmd`: entrada de terminal, prompts e relatório para o usuário.
//! - `core`: regras de negócio, seleção de capítulos e geração de nomes.
//! - `data`: acesso à API do MangaDex e modelos de resposta.
//! - `infra`: configuração, erros e utilitários compartilhados.

/// Camada de comando da CLI.
pub mod cmd;
/// Regras de negócio e serviços da aplicação.
pub mod core;
/// Cliente MangaDex e modelos de dados externos.
pub mod data;
/// Configuração, erros e mecanismos de infraestrutura.
pub mod infra;
