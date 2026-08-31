//! 版本化迁移框架。
//!
//! - 领域表迁移按票据预分配编号：02 号票占用 `0001–0009`（已交付
//!   `0001 knowledge_source`、`0002 source_file`、`0003 fragment + fragment_fts`），
//!   08 号票经主控授权使用 `0004`（`app_setting`，非敏感设置 KV；其余编号留空），
//!   06 号票占用 `0010–0019`，09 号票占用 `0020–0029`。
//! - 迁移版本记录表 [`SCHEMA_MIGRATIONS_SQL`] 是框架簿记，**不是**编号迁移，
//!   不占用任何领域迁移编号。

use rusqlite::Connection;

use super::RepositoryError;

/// 迁移簿记表的建表 SQL：幂等，不属于任何编号迁移。
pub const SCHEMA_MIGRATIONS_SQL: &str = "CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
)";

/// 一个版本化迁移：版本号严格递增，SQL 在单个事务内执行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    /// 迁移版本号（领域表迁移从 0001 起，见模块文档的编号预分配）。
    pub version: i64,
    /// 人类可读的迁移名（如 `create_knowledge_source`）。
    pub name: &'static str,
    /// 迁移 SQL（可包含多条以分号分隔的语句）。
    pub sql: &'static str,
}

/// 应用当前的全部领域迁移列表。
///
/// 编号预分配：02 号票 `0001–0009`（已用 `0001–0003`），08 号票经主控授权
/// 使用 `0004`，06 号票 `0010–0019`，09 号票 `0020–0029`；未用完的编号留空。
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "create_knowledge_source",
        sql: "CREATE TABLE knowledge_source (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL UNIQUE,
    paused INTEGER NOT NULL DEFAULT 0 CHECK (paused IN (0, 1)),
    ignore_globs TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_synced_at TEXT
)",
    },
    Migration {
        version: 2,
        name: "create_source_file",
        sql: "CREATE TABLE source_file (
    id INTEGER PRIMARY KEY,
    source_id TEXT NOT NULL REFERENCES knowledge_source(id),
    rel_path TEXT NOT NULL,
    file_identity TEXT,
    content_hash TEXT NOT NULL,
    size INTEGER NOT NULL,
    mtime_ms INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'missing')),
    UNIQUE (source_id, rel_path)
);
CREATE INDEX idx_source_file_source ON source_file(source_id)",
    },
    Migration {
        version: 3,
        name: "create_fragment_and_fts",
        sql: "CREATE TABLE fragment (
    id INTEGER PRIMARY KEY,
    source_file_id INTEGER NOT NULL REFERENCES source_file(id),
    ordinal INTEGER NOT NULL,
    heading_path TEXT NOT NULL DEFAULT '',
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    text TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    UNIQUE (source_file_id, ordinal)
);
CREATE INDEX idx_fragment_file ON fragment(source_file_id);
CREATE VIRTUAL TABLE fragment_fts USING fts5(
    fragment_id UNINDEXED,
    text,
    tokenize = 'trigram'
)",
    },
    Migration {
        version: 4,
        name: "create_app_setting",
        // 08 号票：非敏感设置的 KV 表（AI base URL / model / 总开关）。
        // API Key 是敏感凭据，只进 OS 凭据库（secrets 模块），绝不进本表。
        sql: "CREATE TABLE app_setting (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
)",
    },
    // 06 号票：草稿元数据表（正文永远在磁盘，数据库只存基线）。
    Migration {
        version: 10,
        name: "create_draft",
        sql: "CREATE TABLE draft (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    filename TEXT NOT NULL,
    baseline_hash TEXT NOT NULL,
    baseline_mtime_ms INTEGER NOT NULL,
    baseline_size INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
)",
    },
    // 07 号票：草稿引用快照表（不可变引用，保留来源删除后的证据）。
    Migration {
        version: 11,
        name: "create_draft_reference",
        sql: "CREATE TABLE draft_reference (
    id TEXT PRIMARY KEY,
    draft_id TEXT NOT NULL REFERENCES draft(id),
    fragment_id INTEGER NOT NULL,
    source_id TEXT NOT NULL,
    source_name TEXT NOT NULL,
    rel_path TEXT NOT NULL,
    heading_path TEXT NOT NULL,
    snippet TEXT NOT NULL,
    full_text TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    adopted INTEGER NOT NULL DEFAULT 0,
    source_deleted INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
)",
    },
];

/// 版本化迁移执行器：校验编号后把未应用的迁移按序应用到数据库。
#[derive(Debug, Clone)]
pub struct Migrator {
    migrations: Vec<Migration>,
}

impl Migrator {
    /// 用给定的迁移列表构造执行器。
    ///
    /// 校验：版本号必须严格递增且不重复，否则拒绝执行。
    pub fn new(migrations: &[Migration]) -> Result<Self, RepositoryError> {
        for pair in migrations.windows(2) {
            if pair[0].version >= pair[1].version {
                return Err(RepositoryError::InvalidMigrationSequence(format!(
                    "migration versions must strictly increase: {} then {}",
                    pair[0].version, pair[1].version
                )));
            }
        }
        Ok(Self {
            migrations: migrations.to_vec(),
        })
    }

    /// 已在数据库中记录的迁移版本（升序）。
    pub fn applied_versions(&self, conn: &Connection) -> Result<Vec<i64>, RepositoryError> {
        let mut stmt = conn.prepare("SELECT version FROM schema_migrations ORDER BY version")?;
        let versions = stmt
            .query_map([], |row| row.get(0))?
            .map_while(Result::ok)
            .collect();
        Ok(versions)
    }

    /// 建立簿记表并把所有未应用的迁移按序应用（每个迁移一个事务）。
    ///
    /// 幂等：重复执行只会应用缺失的版本，已应用的版本原样保留。
    pub fn run(&self, conn: &Connection) -> Result<(), RepositoryError> {
        conn.execute_batch(SCHEMA_MIGRATIONS_SQL)?;
        let current = self.applied_versions(conn)?.pop().unwrap_or(0);

        for migration in self.migrations.iter().filter(|m| m.version > current) {
            let tx = conn.unchecked_transaction()?;
            conn.execute_batch(migration.sql)?;
            conn.execute(
                "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
                rusqlite::params![migration.version, migration.name],
            )?;
            tx.commit()?;
        }
        Ok(())
    }
}
