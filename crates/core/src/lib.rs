//! InsightForge 桌面应用核心：SQLite repository、版本化迁移与领域服务。
//!
//! 01 号票交付 repository 地基（WAL + 单一写入队列 + 版本化迁移框架）；
//! 02 号票交付来源管理与首次补扫索引（分块纯函数、扫描边界、专门
//! executor、Sources 命令 seam）。后续票据继续垂直切片。

pub mod chunking;
pub mod clock;
pub mod coach;
pub mod diagnostics;
pub mod drafts;
pub mod egress;
pub mod executor;
pub mod http_client;
mod indexer;
pub mod references;
pub mod repository;
pub mod scan;
pub mod search;
pub mod secrets;
pub mod settings;
pub mod sources;
mod store;
pub mod thoughts;

pub use chunking::{chunk_markdown, chunk_plain, Chunk, ChunkConfig, DEFAULT_MAX_CHUNK_CHARS};
pub use clock::{format_iso, now_iso};
pub use coach::{
    CoachAction, CoachError, CoachService, CoachSession, CoachSessionState, CoachTurn,
};
pub use diagnostics::{
    build_snapshot, now_rfc3339, platform_label, DatabaseDiagnostics, DiagnosticSnapshot, EventLog,
    LogEntry, LogLevel, MUST_NOT_LOG_KEYS,
};
pub use drafts::{Conflict, DraftFile, DraftsError, DraftsService};
pub use egress::{
    AuditLogger, AuditRecord, EgressBundle, EgressBundleBuilder, EgressError, EgressFragment,
};
pub use executor::{CancelToken, Executor, ExecutorError};
pub use http_client::{AiHttpClient, HttpClientError, HttpRequest, HttpResponse, UreqAiHttpClient};
pub use references::{Reference, ReferencesError, ReferencesService};
pub use repository::{Migration, Migrator, Repository, RepositoryError};
pub use scan::{
    canonical_path, file_identity, normalize_root, open_read_only, stays_within_root,
    validate_rel_path, IgnoreMatcher, ScanError, ALLOWED_EXTENSIONS, DEFAULT_IGNORE_GLOBS,
};
pub use search::{EvidenceCard, FragmentContext, SearchError, SearchService};
pub use secrets::{
    KeyringStore, MemorySecretStore, SecretStore, SecretsError, API_KEY_ACCOUNT, SERVICE_NAME,
};
pub use settings::{
    AiAvailability, AiSampleReply, AiSettings, AiUnavailableReason, Settings, SettingsError,
    TestConnectionReport,
};
pub use sources::{
    EventSink, IndexStatus, SourceEvent, SourceStatus, SourceSummary, Sources, SourcesError,
};
pub use store::{
    files_of_source_with_identity, mark_files_missing, rename_file, StoredFileWithIdentity,
};
pub use thoughts::{ThoughtCard, ThoughtsError, ThoughtsService, UpsertThoughtCard};
