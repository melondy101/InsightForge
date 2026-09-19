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
/// 使用 `0004`，06 号票 `0010–0019`（`0010–0013` 已用），09 号票 `0020–0029`；
/// 10 号票（陪练）经主控授权使用 `0014–0015`；ADR 0002（互通契约）使用 `0016–0017`；
/// 未用完的编号留空。
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
    // 09 号票：AI 调用审计表（只记录元数据，不重复保存敏感全文）。
    Migration {
        version: 12,
        name: "create_ai_call_audit",
        sql: "CREATE TABLE ai_call_audit (
    id TEXT PRIMARY KEY,
    model TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    fragment_count INTEGER NOT NULL,
    fragment_ids TEXT NOT NULL,
    status TEXT NOT NULL
)",
    },
    // 11 号票：思考卡表（作者资产，不随索引重建而重建）。
    Migration {
        version: 13,
        name: "create_thought_card",
        sql: "CREATE TABLE thought_card (
    id TEXT PRIMARY KEY,
    idea TEXT NOT NULL,
    source_id TEXT NOT NULL,
    source_name TEXT NOT NULL,
    rel_path TEXT NOT NULL,
    heading_path TEXT NOT NULL,
    draft_id TEXT NOT NULL,
    draft_filename TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
)",
    },
    // 10 号票：陪练会话与轮次（三轮协议的状态载体）。
    Migration {
        version: 14,
        name: "create_coach_session",
        sql: "CREATE TABLE coach_session (
    id TEXT PRIMARY KEY,
    draft_id TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('active', 'waiting_for_coach', 'completed', 'archived')),
    current_round INTEGER NOT NULL,
    max_rounds INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_coach_session_draft ON coach_session(draft_id)",
    },
    Migration {
        version: 15,
        name: "create_coach_turn",
        sql: "CREATE TABLE coach_turn (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES coach_session(id),
    draft_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'coach')),
    content TEXT NOT NULL,
    action TEXT,
    round_number INTEGER NOT NULL,
    helpful INTEGER NOT NULL DEFAULT 0 CHECK (helpful IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_coach_turn_session ON coach_turn(session_id)",
    },
    // ADR 0002：互通契约的判序列。rev = 该逻辑行在本设备上的修订序号（单调递增），
    // origin_device = 最后一次写入它的设备身份。两者共同替代「本机路径 / 时间戳」
    // 作为跨设备比较依据；缺省值让既有行成为「同步基线上的第 0 版」。
    Migration {
        version: 16,
        name: "add_sync_columns_to_author_assets",
        sql: "ALTER TABLE draft ADD COLUMN rev INTEGER NOT NULL DEFAULT 0;
ALTER TABLE draft ADD COLUMN origin_device TEXT;
ALTER TABLE draft_reference ADD COLUMN rev INTEGER NOT NULL DEFAULT 0;
ALTER TABLE draft_reference ADD COLUMN origin_device TEXT;
ALTER TABLE thought_card ADD COLUMN rev INTEGER NOT NULL DEFAULT 0;
ALTER TABLE thought_card ADD COLUMN origin_device TEXT;
ALTER TABLE coach_session ADD COLUMN rev INTEGER NOT NULL DEFAULT 0;
ALTER TABLE coach_session ADD COLUMN origin_device TEXT;
ALTER TABLE coach_turn ADD COLUMN rev INTEGER NOT NULL DEFAULT 0;
ALTER TABLE coach_turn ADD COLUMN origin_device TEXT;
ALTER TABLE ai_call_audit ADD COLUMN rev INTEGER NOT NULL DEFAULT 0;
ALTER TABLE ai_call_audit ADD COLUMN origin_device TEXT",
    },
    // ADR 0002：修订号的打点由存储层统一负责——12 个领域服务写路径、未来的同步入站
    // 应用、以及任何一条直连 SQL 都共享同一规则，漏写一处就会让两台设备对「谁更新」
    // 产生分歧，而这正是同步最贵的故障。
    //
    // 规则：
    // - 本机写入（不显式声明 rev）→ rev 递增 1，origin_device 记为本机身份；
    //   `WHEN NEW.rev = OLD.rev` 既是「调用方没有自己动过 rev」的判据，
    //   也让触发器自身的第二次 UPDATE 不会再触发（与 recursive_triggers 取值无关）。
    // - 入站应用远端修订（显式 SET rev = 对方声明的值）→ NEW.rev ≠ OLD.rev，
    //   触发器不介入，原样落地。
    Migration {
        version: 17,
        name: "add_revision_triggers_to_author_assets",
        sql: "CREATE TRIGGER trg_draft_rev_on_insert AFTER INSERT ON draft WHEN NEW.rev = 0
BEGIN
    UPDATE draft SET rev = 1, origin_device = (SELECT value FROM app_setting WHERE key = 'device.id') WHERE id = NEW.id;
END;
CREATE TRIGGER trg_draft_rev_on_update AFTER UPDATE ON draft WHEN NEW.rev = OLD.rev
BEGIN
    UPDATE draft SET rev = OLD.rev + 1, origin_device = (SELECT value FROM app_setting WHERE key = 'device.id') WHERE id = OLD.id;
END;
CREATE TRIGGER trg_draft_reference_rev_on_insert AFTER INSERT ON draft_reference WHEN NEW.rev = 0
BEGIN
    UPDATE draft_reference SET rev = 1, origin_device = (SELECT value FROM app_setting WHERE key = 'device.id') WHERE id = NEW.id;
END;
CREATE TRIGGER trg_draft_reference_rev_on_update AFTER UPDATE ON draft_reference WHEN NEW.rev = OLD.rev
BEGIN
    UPDATE draft_reference SET rev = OLD.rev + 1, origin_device = (SELECT value FROM app_setting WHERE key = 'device.id') WHERE id = OLD.id;
END;
CREATE TRIGGER trg_thought_card_rev_on_insert AFTER INSERT ON thought_card WHEN NEW.rev = 0
BEGIN
    UPDATE thought_card SET rev = 1, origin_device = (SELECT value FROM app_setting WHERE key = 'device.id') WHERE id = NEW.id;
END;
CREATE TRIGGER trg_thought_card_rev_on_update AFTER UPDATE ON thought_card WHEN NEW.rev = OLD.rev
BEGIN
    UPDATE thought_card SET rev = OLD.rev + 1, origin_device = (SELECT value FROM app_setting WHERE key = 'device.id') WHERE id = OLD.id;
END;
CREATE TRIGGER trg_coach_session_rev_on_insert AFTER INSERT ON coach_session WHEN NEW.rev = 0
BEGIN
    UPDATE coach_session SET rev = 1, origin_device = (SELECT value FROM app_setting WHERE key = 'device.id') WHERE id = NEW.id;
END;
CREATE TRIGGER trg_coach_session_rev_on_update AFTER UPDATE ON coach_session WHEN NEW.rev = OLD.rev
BEGIN
    UPDATE coach_session SET rev = OLD.rev + 1, origin_device = (SELECT value FROM app_setting WHERE key = 'device.id') WHERE id = OLD.id;
END;
CREATE TRIGGER trg_coach_turn_rev_on_insert AFTER INSERT ON coach_turn WHEN NEW.rev = 0
BEGIN
    UPDATE coach_turn SET rev = 1, origin_device = (SELECT value FROM app_setting WHERE key = 'device.id') WHERE id = NEW.id;
END;
CREATE TRIGGER trg_coach_turn_rev_on_update AFTER UPDATE ON coach_turn WHEN NEW.rev = OLD.rev
BEGIN
    UPDATE coach_turn SET rev = OLD.rev + 1, origin_device = (SELECT value FROM app_setting WHERE key = 'device.id') WHERE id = OLD.id;
END;
CREATE TRIGGER trg_ai_call_audit_rev_on_insert AFTER INSERT ON ai_call_audit WHEN NEW.rev = 0
BEGIN
    UPDATE ai_call_audit SET rev = 1, origin_device = (SELECT value FROM app_setting WHERE key = 'device.id') WHERE id = NEW.id;
END;
CREATE TRIGGER trg_ai_call_audit_rev_on_update AFTER UPDATE ON ai_call_audit WHEN NEW.rev = OLD.rev
BEGIN
    UPDATE ai_call_audit SET rev = OLD.rev + 1, origin_device = (SELECT value FROM app_setting WHERE key = 'device.id') WHERE id = OLD.id;
END",
    },
    // ADR 0002 §5：同步是第二类出站包，审计必须能把「为 AI 发的」和「为同步发的」
    // 分开陈列——用户要能查清哪些内容因什么原因离开过本机。默认 'ai'，既有行语义不变。
    Migration {
        version: 18,
        name: "add_kind_to_ai_call_audit",
        sql: "ALTER TABLE ai_call_audit ADD COLUMN kind TEXT NOT NULL DEFAULT 'ai'",
    },
    // ADR 0002 §3：服务端只保管「每条逻辑身份所见过的最高修订」，而判定一条入站
    // 修订能不能落地，先要回答「这台设备还作数吗」。撤销必须不可逆——被盗用的设备
    // 身份不能靠重新注册复活，所以状态用 CHECK 锁死在 active/revoked 两值。
    Migration {
        version: 19,
        name: "add_sync_device_registry",
        sql: "CREATE TABLE sync_device (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'revoked')),
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    revoked_at TEXT
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
