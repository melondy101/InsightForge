//! InsightForge 桌面应用核心：SQLite repository、版本化迁移与领域服务。
//!
//! 01 号票仅交付 repository 地基（WAL + 单一写入队列 + 版本化迁移框架），
//! 领域能力（来源、索引、检索等）由后续票据在此 crate 内垂直切片交付。

pub mod repository;

pub use repository::{Migration, Migrator, Repository, RepositoryError};
