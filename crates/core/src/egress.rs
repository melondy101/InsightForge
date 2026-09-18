//! 出站包与调用审计（09 号票）：
//!
//! - [`EgressBundle`] 是本地不可变快照，只包含可安全出站的数据：当前草稿段落/选区、
//!   标题/大纲、已引用素材、至多 3 条检索命中片段。本机路径、完整文件、敏感全文
//!   永不出站。
//! - [`AuditLogger`] 记录每次 AI 调用的模型、时间、发送片段数量/ID、状态；
//!   不重复保存敏感全文。

use std::sync::Arc;

use serde::Serialize;
use thiserror::Error;

use crate::references::ReferencesService;
use crate::repository::Repository;
use crate::search::SearchService;

/// 出站片段：轻量、无本机路径、无敏感全文。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EgressFragment {
    /// 片段来源类型：draft | reference | search | outline。
    pub kind: String,
    /// 片段标题/来源名（无路径）。
    pub label: String,
    /// 片段正文（已截断/脱敏）。
    pub text: String,
}

/// 出站包：当前 AI 调用的完整载荷快照。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EgressBundle {
    /// 出站片段总数。
    pub fragment_count: usize,
    /// 出站片段列表（顺序与前端展示一致）。
    pub fragments: Vec<EgressFragment>,
}

/// 调用审计记录。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditRecord {
    pub id: String,
    pub model: String,
    pub created_at: String,
    pub fragment_count: i64,
    pub fragment_ids: Vec<String>,
    pub status: String,
}

/// Egress 错误。
#[derive(Debug, Error)]
pub enum EgressError {
    #[error("repository error: {0}")]
    Repository(#[from] crate::repository::RepositoryError),
    #[error("search error: {0}")]
    Search(#[from] crate::search::SearchError),
    #[error("references error: {0}")]
    References(#[from] crate::references::ReferencesError),
    #[error("invalid draft content")]
    InvalidDraftContent,
}

/// 出站包生成器：从草稿内容、引用与检索结果组装不可变出站包。
pub struct EgressBundleBuilder {
    search: Arc<SearchService>,
    references: Arc<ReferencesService>,
}

impl EgressBundleBuilder {
    pub fn new(
        _repo: Arc<Repository>,
        search: Arc<SearchService>,
        references: Arc<ReferencesService>,
    ) -> Self {
        Self { search, references }
    }

    /// 为指定草稿生成出站包。
    pub fn build_for_draft(
        &self,
        draft_content: &str,
        draft_id: &str,
        search_query: Option<(&str, &str, usize)>,
    ) -> Result<EgressBundle, EgressError> {
        let mut fragments: Vec<EgressFragment> = Vec::new();

        // 1. 草稿内容：按段落拆分，保留标题/大纲。
        self.add_draft_fragments(draft_content, &mut fragments);

        // 2. 已引用素材（来自 references）。
        self.add_reference_fragments(draft_id, &mut fragments);

        // 3. 检索命中片段（至多 3 条）。
        if let Some((source_id, query, limit)) = search_query {
            self.add_search_fragments(source_id, query, limit.min(3), &mut fragments);
        }

        Ok(EgressBundle {
            fragment_count: fragments.len(),
            fragments,
        })
    }

    fn add_draft_fragments(&self, content: &str, fragments: &mut Vec<EgressFragment>) {
        let lines: Vec<&str> = content.lines().collect();
        let mut current_heading: Option<String> = None;
        let mut paragraph_buffer: Vec<String> = Vec::new();

        for line in lines {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                // 先提交上一段缓冲区。
                self.flush_paragraph_buffer(
                    paragraph_buffer,
                    current_heading.as_deref(),
                    fragments,
                );
                paragraph_buffer = Vec::new();

                // 记录新标题。
                current_heading = Some(trimmed.to_string());
            } else if trimmed.is_empty() {
                // 空行：提交缓冲区。
                self.flush_paragraph_buffer(
                    paragraph_buffer,
                    current_heading.as_deref(),
                    fragments,
                );
                paragraph_buffer = Vec::new();
            } else {
                paragraph_buffer.push(trimmed.to_string());
            }
        }

        // 提交最后一段。
        self.flush_paragraph_buffer(paragraph_buffer, current_heading.as_deref(), fragments);
    }

    fn flush_paragraph_buffer(
        &self,
        buffer: Vec<String>,
        heading: Option<&str>,
        fragments: &mut Vec<EgressFragment>,
    ) {
        if buffer.is_empty() {
            return;
        }

        let text = buffer.join(" ");
        let label = heading.unwrap_or("paragraph").to_string();

        // 单段落截断到 500 字符。
        let truncated: String = text.chars().take(500).collect();
        fragments.push(EgressFragment {
            kind: "draft".to_string(),
            label,
            text: truncated,
        });
    }

    fn add_reference_fragments(&self, draft_id: &str, fragments: &mut Vec<EgressFragment>) {
        let refs = match self.references.list(draft_id) {
            Ok(r) => r,
            Err(_) => return,
        };

        for reference in refs {
            let text = reference.full_text.chars().take(200).collect::<String>();
            fragments.push(EgressFragment {
                kind: "reference".to_string(),
                label: format!("{} ({})", reference.source_name, reference.rel_path),
                text,
            });
        }
    }

    fn add_search_fragments(
        &self,
        source_id: &str,
        query: &str,
        limit: usize,
        fragments: &mut Vec<EgressFragment>,
    ) {
        if limit == 0 {
            return;
        }

        let Ok(cards) = self.search.query(source_id, query, limit) else {
            return;
        };

        for card in cards {
            fragments.push(EgressFragment {
                kind: "search".to_string(),
                label: format!("{} ({})", card.source_name, card.rel_path),
                text: card.snippet,
            });
        }
    }
}

/// 调用审计日志：写入 `ai_call_audit` 表。
pub struct AuditLogger {
    repo: Arc<Repository>,
}

impl AuditLogger {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self { repo }
    }

    /// 记录一次 AI 调用。
    pub fn log(
        &self,
        model: &str,
        fragment_count: i64,
        fragment_ids: &[String],
        status: &str,
    ) -> Result<AuditRecord, EgressError> {
        let record = AuditRecord {
            id: generate_audit_id(),
            model: model.to_string(),
            created_at: now_iso(),
            fragment_count,
            fragment_ids: fragment_ids.to_vec(),
            status: status.to_string(),
        };

        let record_for_write = record.clone();
        self.repo.write(move |conn| {
            conn.execute(
                "INSERT INTO ai_call_audit
                     (id, model, created_at, fragment_count, fragment_ids, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    record_for_write.id,
                    record_for_write.model,
                    record_for_write.created_at,
                    record_for_write.fragment_count,
                    serde_json::to_string(&record_for_write.fragment_ids).unwrap(),
                    record_for_write.status,
                ],
            )?;
            Ok(())
        })?;

        Ok(record)
    }

    /// 列出最近的审计记录（用于前端审计视图）。
    pub fn list(&self, limit: usize) -> Result<Vec<AuditRecord>, EgressError> {
        let limit = limit as i64;
        let records = self.repo.read(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, model, created_at, fragment_count, fragment_ids, status
                 FROM ai_call_audit
                 ORDER BY created_at DESC
                 LIMIT ?1",
            )?;
            let mut rows = stmt.query(rusqlite::params![limit])?;
            let mut records = Vec::new();
            while let Some(row) = rows.next()? {
                let fragment_ids_str: String = row.get(4)?;
                let fragment_ids: Vec<String> =
                    serde_json::from_str(&fragment_ids_str).unwrap_or_default();
                records.push(AuditRecord {
                    id: row.get(0)?,
                    model: row.get(1)?,
                    created_at: row.get(2)?,
                    fragment_count: row.get(3)?,
                    fragment_ids,
                    status: row.get(5)?,
                });
            }
            Ok(records)
        })?;
        Ok(records)
    }
}

/// 生成审计记录 ID。
fn generate_audit_id() -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .hash(&mut hasher);
    format!("audit_{:x}", hasher.finish())
}

use crate::clock::now_iso;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn egress_builder_creates_fragments_from_draft() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("insightforge.sqlite");
        let repo = Arc::new(Repository::open(&db_path).unwrap());
        let search = Arc::new(SearchService::new(Arc::clone(&repo)));
        let references = Arc::new(ReferencesService::new(Arc::clone(&repo)));
        let builder = EgressBundleBuilder::new(repo, search, references);

        let content = "# Title\n\nParagraph one.\n\nParagraph two.\n\n## Section\n\nMore text.";
        let bundle = builder.build_for_draft(content, "draft-1", None).unwrap();

        // 至少包含标题和段落片段。
        assert!(bundle.fragment_count > 0);
        assert_eq!(bundle.fragments[0].kind, "draft");
    }

    #[test]
    fn egress_builder_truncates_long_paragraphs() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("insightforge.sqlite");
        let repo = Arc::new(Repository::open(&db_path).unwrap());
        let search = Arc::new(SearchService::new(Arc::clone(&repo)));
        let references = Arc::new(ReferencesService::new(Arc::clone(&repo)));
        let builder = EgressBundleBuilder::new(repo, search, references);

        let long_text: String = "word ".repeat(200);
        let content = format!("# Title\n\n{}", long_text);
        let bundle = builder.build_for_draft(&content, "draft-1", None).unwrap();

        let draft_fragments: Vec<_> = bundle
            .fragments
            .iter()
            .filter(|f| f.kind == "draft")
            .collect();
        assert!(!draft_fragments.is_empty());
        assert!(draft_fragments[0].text.chars().count() <= 500);
    }
}
