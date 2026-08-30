//! 版本化迁移框架。
//!
//! - 领域表迁移按票据预分配编号：02 号票占用 `0001–0009`，06 号票占用
//!   `0010–0019`，09 号票占用 `0020–0029`。
//! - 01 号票只交付框架本身：[`MIGRATIONS`] 列表为空，不写任何领域表迁移。
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
/// 01 号票不交付任何领域迁移；`0001–0029` 已预留给 02/06/09 号票。
pub const MIGRATIONS: &[Migration] = &[];

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
