//! 诊断快照：用户可随时触发、脱敏、可复制导出的结构化状态概览。
//!
//! 设计红线（与「绝不上传用户内容」一致）：快照只含版本、平台、SQLite 版本，
//! 以及数据库的脱敏聚合（journal 模式、已应用迁移、完整性结论、各表行数）。
//! 绝不出现：API Key、草稿/来源正文、用户文件路径、源文件内容。

use std::collections::HashMap;

use serde::Serialize;

use crate::repository::Repository;

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
}
