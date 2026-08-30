//! InsightForge 桌面应用核心：SQLite repository、版本化迁移与领域服务。
//!
//! 01 号票交付 repository 地基（WAL + 单一写入队列 + 版本化迁移框架）；
//! 02 号票交付来源管理与首次补扫索引（分块纯函数、扫描边界、专门
//! executor、Sources 命令 seam）。后续票据继续垂直切片。

pub mod chunking;
pub mod executor;
mod indexer;
pub mod repository;
pub mod scan;
pub mod sources;
mod store;

pub use chunking::{chunk_markdown, chunk_plain, Chunk, ChunkConfig, DEFAULT_MAX_CHUNK_CHARS};
pub use executor::{CancelToken, Executor, ExecutorError};
pub use repository::{Migration, Migrator, Repository, RepositoryError};
pub use scan::{
    canonical_path, file_identity, normalize_root, open_read_only, stays_within_root,
    validate_rel_path, IgnoreMatcher, ScanError, ALLOWED_EXTENSIONS, DEFAULT_IGNORE_GLOBS,
};
pub use sources::{EventSink, SourceEvent, SourceStatus, SourceSummary, Sources, SourcesError};
