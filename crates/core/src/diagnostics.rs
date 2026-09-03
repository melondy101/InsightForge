//! 诊断快照：用户可随时触发、脱敏、可复制导出的结构化状态概览。
//!
//! 设计红线（与「绝不上传用户内容」一致）：快照只含版本、平台、SQLite 版本，
//! 以及数据库的脱敏聚合（journal 模式、已应用迁移、完整性结论、各表行数）。
//! 绝不出现：API Key、草稿/来源正文、用户文件路径、源文件内容。

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use serde::Serialize;

use crate::repository::Repository;

/// 结构化事件日志级别（稳定枚举，序列化用小写，便于前端/导出统一）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// 脱敏红线：这些字段名（大小写不敏感）一旦进入日志即被替换为 `[redacted]`，
/// 绝不记录密钥、令牌、正文、密码等。任何新增日志点都受此约束，无需逐处审查。
pub const MUST_NOT_LOG_KEYS: &[&str] = &[
    "apikey",
    "api_key",
    "token",
    "password",
    "secret",
    "authorization",
    "content",
    "body",
    "key",
    "cookie",
    "credential",
];

/// 一条结构化日志：稳定事件名 + 级别 + 已脱敏字段。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub ts: String,
    pub level: LogLevel,
    pub name: String,
    pub fields: HashMap<String, String>,
}

type Clock = Arc<dyn Fn() -> String + Send + Sync>;

/// 应用内结构化事件日志：定长环形缓冲（写满丢弃最旧），用于诊断导出。
///
/// 与 stdout/stderr 不同，它可被用户随时取出、复制、求助；且强制脱敏，
/// 任何含敏感字段名的条目都会被静默替换为 `[redacted]`。
pub struct EventLog {
    cap: usize,
    entries: VecDeque<LogEntry>,
    clock: Clock,
}

impl EventLog {
    pub fn new(cap: usize) -> Self {
        Self::with_clock(cap, Arc::new(now_rfc3339))
    }

    pub fn with_clock(cap: usize, clock: Clock) -> Self {
        Self {
            cap,
            entries: VecDeque::new(),
            clock,
        }
    }

    /// 写入一条事件。字段先经脱敏过滤，再入环形缓冲（超容丢弃最旧）。
    pub fn push(&mut self, level: LogLevel, name: &str, fields: &[(&str, &str)]) {
        let mut redacted = HashMap::new();
        for (k, v) in fields {
            let key = (*k).to_lowercase();
            if MUST_NOT_LOG_KEYS.contains(&key.as_str()) {
                redacted.insert((*k).to_string(), "[redacted]".to_string());
            } else {
                redacted.insert((*k).to_string(), (*v).to_string());
            }
        }
        self.entries.push_back(LogEntry {
            ts: (self.clock)(),
            level,
            name: name.to_string(),
            fields: redacted,
        });
        while self.entries.len() > self.cap {
            self.entries.pop_front();
        }
    }

    /// 最近 `limit` 条（新→旧），用于快照/导出。
    pub fn recent(&self, limit: usize) -> Vec<LogEntry> {
        self.entries.iter().rev().take(limit).cloned().collect()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.recent(self.entries.len())).unwrap_or_else(|_| "[]".to_string())
    }

    /// 仅序列化最近 `limit` 条（新→旧），用于命令导出。
    pub fn to_json_of_newest(&self, limit: usize) -> String {
        serde_json::to_string(&self.recent(limit)).unwrap_or_else(|_| "[]".to_string())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// 当前平台标签（如 `windows x86_64`），用于区分环境相关的故障。
pub fn platform_label() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

/// 当前时间（RFC3339），用于快照时间戳。集中在 core 以避免 tauri 壳再拉一个 chrono 依赖。
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// 数据库脱敏诊断：仅聚合指标，绝不携带数据内容。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseDiagnostics {
    pub journal_mode: String,
    pub applied_migrations: Vec<i64>,
    pub integrity: String,
    pub table_counts: HashMap<String, i64>,
}

/// 一份完整的脱敏诊断快照。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticSnapshot {
    pub version: String,
    pub generated_at: String,
    pub platform: String,
    pub sqlite_version: String,
    pub database: DatabaseDiagnostics,
}

/// 构建脱敏快照。
///
/// `generated_at` 由调用方注入（生产用当前时间，测试用固定值），以便测试可重复。
/// 任何单项读取失败都不应让整个快照失败——降级为 `"unknown"` 或空聚合，
/// 因为诊断本身常常是在「系统已经不太正常」时才被触发。
pub fn build_snapshot(
    repo: &Repository,
    version: &str,
    generated_at: &str,
) -> Result<DiagnosticSnapshot, crate::repository::RepositoryError> {
    let journal_mode = repo
        .journal_mode()
        .unwrap_or_else(|_| "unknown".to_string());
    let applied_migrations = repo.applied_migrations().unwrap_or_default();
    let integrity = repo
        .integrity_report()
        .unwrap_or_else(|_| "unknown".to_string());
    let table_counts = repo.table_counts().unwrap_or_default();

    Ok(DiagnosticSnapshot {
        version: version.to_string(),
        generated_at: generated_at.to_string(),
        platform: platform_label(),
        sqlite_version: Repository::sqlite_libversion().to_string(),
        database: DatabaseDiagnostics {
            journal_mode,
            applied_migrations,
            integrity,
            table_counts,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drafts::DraftsService;
    use std::sync::Arc;

    fn setup() -> (Arc<Repository>, DraftsService, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("insightforge.sqlite");
        let repo = Arc::new(Repository::open(&db_path).unwrap());
        let drafts = DraftsService::new(Arc::clone(&repo), dir.path().join("drafts"));
        (repo, drafts, dir)
    }

    /// 快照必须包含版本/平台/sqlite 版本等结构性字段，且不因单项读取失败而崩溃。
    #[test]
    fn snapshot_contains_structural_fields() {
        let (repo, _drafts, _dir) = setup();
        let snap = build_snapshot(&repo, "9.9.9", "2026-01-01T00:00:00Z").unwrap();
        assert_eq!(snap.version, "9.9.9");
        assert_eq!(snap.generated_at, "2026-01-01T00:00:00Z");
        assert!(!snap.platform.is_empty());
        assert!(!snap.sqlite_version.is_empty());
        assert_eq!(snap.database.integrity, "ok");
        // schema_migrations 等表应被计数
        assert!(snap.database.table_counts.contains_key("schema_migrations"));
    }

    /// 脱敏红线：即使用户数据是带秘密正文的草稿，快照序列化结果也不得包含该正文，
    /// 且不得出现 API Key 常见前缀——从结构上杜绝内容外泄。
    #[test]
    fn snapshot_is_desensitized_no_content_leak() {
        let (repo, drafts, _dir) = setup();
        // 写入一份「含敏感正文」的草稿
        drafts.create("secret.md").unwrap();
        drafts
            .save("secret.md", "TOPSECRET_DRAFT_BODY_SHOULD_NOT_LEAK")
            .unwrap();

        let snap = build_snapshot(&repo, "1.0.0", "2026-01-01T00:00:00Z").unwrap();
        let json = serde_json::to_string(&snap).unwrap();

        assert!(
            !json.contains("TOPSECRET_DRAFT_BODY_SHOULD_NOT_LEAK"),
            "draft body must never appear in the diagnostic snapshot"
        );
        assert!(
            !json.contains("sk-"),
            "api key prefix must never appear in the diagnostic snapshot"
        );
        // 表计数里应有 draft 表且行数 >= 1，证明我们统计的是结构而非内容
        let draft_count = snap
            .database
            .table_counts
            .get("draft")
            .copied()
            .unwrap_or(0);
        assert!(draft_count >= 1, "draft table should be counted");
    }

    /// 环形缓冲：写满后丢弃最旧，长度不超过容量，且保留最新。
    #[test]
    fn event_log_rotates_when_full() {
        let clock: Arc<dyn Fn() -> String + Send + Sync> =
            Arc::new(|| "2026-01-01T00:00:00Z".to_string());
        let mut log = EventLog::with_clock(3, clock);
        for i in 0..5 {
            log.push(LogLevel::Info, "tick", &[("n", &i.to_string())]);
        }
        assert_eq!(log.len(), 3, "log must not exceed capacity");
        let recent = log.recent(10);
        assert_eq!(recent.len(), 3);
        // 最新一条应是 n=4（最旧 0、1 被丢弃）
        assert_eq!(recent[0].fields.get("n").map(|s| s.as_str()), Some("4"));
        // n=0、n=1 不应残留
        assert!(!recent
            .iter()
            .any(|e| e.fields.get("n") == Some(&"0".to_string())));
    }

    /// 脱敏红线：包含敏感字段名的条目一律替换为 `[redacted]`，且序列化结果不含原值。
    #[test]
    fn event_log_redacts_sensitive_fields() {
        let clock: Arc<dyn Fn() -> String + Send + Sync> =
            Arc::new(|| "2026-01-01T00:00:00Z".to_string());
        let mut log = EventLog::with_clock(10, clock);
        log.push(
            LogLevel::Info,
            "draft.save",
            &[
                ("filename", "note.md"),
                ("apiKey", "sk-TOPSECRETKEY"),
                ("content", "user private body"),
            ],
        );
        let entry = &log.recent(1)[0];
        assert_eq!(entry.fields.get("filename"), Some(&"note.md".to_string()));
        assert_eq!(
            entry.fields.get("apiKey").map(|s| s.as_str()),
            Some("[redacted]")
        );
        assert_eq!(
            entry.fields.get("content").map(|s| s.as_str()),
            Some("[redacted]")
        );
        let json = log.to_json();
        assert!(
            !json.contains("sk-TOPSECRETKEY"),
            "api key must never be logged"
        );
        assert!(
            !json.contains("user private body"),
            "draft content must never be logged"
        );
    }
}
