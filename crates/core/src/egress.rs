//! 出站包与调用审计（09 号票 + ADR 0002 §5）：
//!
//! - [`EgressBundle`] 是本地不可变快照，只包含可安全出站的数据：当前草稿段落/选区、
//!   标题/大纲、已引用素材、至多 3 条检索命中片段。本机路径、完整文件、敏感全文
//!   永不出站。
//! - [`SyncBundle`] 是第二类出站包（同步载荷）。它与 AI 出站同等对待：
//!   **发送前**自带「每张表几条、带哪些字段」的清单，并写入同一份 `ai_call_audit`
//!   （`kind = 'sync'`）。铁律 3 说「同步载荷永不含路径」，靠 [`SYNCED_ASSETS`]
//!   白名单 + [`DEVICE_LOCAL_COLUMNS`] 黑名单成立，不靠调用点自觉。
//! - [`AuditLogger`] 记录每次出站：模型/类型、时间、数量与 ID、状态；
//!   不重复保存敏感全文。

use std::sync::Arc;

use serde::Serialize;
use serde_json::{Map, Value};
use thiserror::Error;

use crate::clock::now_iso;
use crate::document_store::DocumentStore;
use crate::drafts::DraftsService;
use crate::identity::new_id;
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
    /// `ai` 走 `model`（真实模型名）；`sync` 时 `model` 固定为 `"sync"`。
    pub kind: String,
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
    #[error("sync whitelist references device-local column {0}")]
    DeviceLocalColumn(String),
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
        self.insert("ai", model, fragment_count, fragment_ids, status)
    }

    /// 记录一次同步出站（ADR 0002 §5）：与 AI 调用同一份审计，靠 `kind` 分开。
    /// 没有模型名可记，`model` 固定写 `"sync"`，`fragment_ids` 记的是被送出去的
    /// 逻辑身份（草稿文件名等），这样用户能查「我的哪份稿子离开过本机」。
    pub fn log_sync(
        &self,
        record_count: i64,
        record_ids: &[String],
        status: &str,
    ) -> Result<AuditRecord, EgressError> {
        self.insert("sync", "sync", record_count, record_ids, status)
    }

    fn insert(
        &self,
        kind: &str,
        model: &str,
        count: i64,
        ids: &[String],
        status: &str,
    ) -> Result<AuditRecord, EgressError> {
        let record = AuditRecord {
            id: new_id(),
            kind: kind.to_string(),
            model: model.to_string(),
            created_at: now_iso(),
            fragment_count: count,
            fragment_ids: ids.to_vec(),
            status: status.to_string(),
        };

        let record_for_write = record.clone();
        self.repo.write(move |conn| {
            conn.execute(
                "INSERT INTO ai_call_audit
                     (id, kind, model, created_at, fragment_count, fragment_ids, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    record_for_write.id,
                    record_for_write.kind,
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
                "SELECT id, kind, model, created_at, fragment_count, fragment_ids, status
                 FROM ai_call_audit
                 ORDER BY created_at DESC
                 LIMIT ?1",
            )?;
            let mut rows = stmt.query(rusqlite::params![limit])?;
            let mut records = Vec::new();
            while let Some(row) = rows.next()? {
                let fragment_ids_str: String = row.get(5)?;
                let fragment_ids: Vec<String> =
                    serde_json::from_str(&fragment_ids_str).unwrap_or_default();
                records.push(AuditRecord {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    model: row.get(2)?,
                    created_at: row.get(3)?,
                    fragment_count: row.get(4)?,
                    fragment_ids,
                    status: row.get(6)?,
                });
            }
            Ok(records)
        })?;
        Ok(records)
    }
}

// --- 第二类出站包：同步载荷（ADR 0002 §5） ---

/// 设备本地列：绝对路径、授权根目录、以及只在单机有意义的保存基线。
/// 它们**永不进白名单**；[`SyncBundle::build`] 里再挡一道，
/// 任何一处失守都会立刻在 `tests/sync_bundle_contract.rs` 里红。
pub const DEVICE_LOCAL_COLUMNS: &[&str] = &[
    "path",
    "root_path",
    "abs_path",
    "file_identity",
    "baseline_hash",
    "baseline_mtime_ms",
    "baseline_size",
];

/// 参与同步的作者资产与每张表允许离开本机的列（ADR 0002 §2）。
///
/// 不含派生数据（fragment / FTS / 扫描游标）与本机身份；`rel_path` 是知识库内的
/// **相对**路径，本就是引用含义的一部分（§3），所以保留。
pub const SYNCED_ASSETS: &[(&str, &[&str])] = &[
    ("draft", &["id", "filename", "created_at", "updated_at"]),
    (
        "draft_reference",
        &[
            "id",
            "draft_id",
            "fragment_id",
            "source_id",
            "source_name",
            "rel_path",
            "heading_path",
            "snippet",
            "full_text",
            "start_line",
            "end_line",
            "adopted",
            "source_deleted",
            "created_at",
        ],
    ),
    (
        "thought_card",
        &[
            "id",
            "idea",
            "source_id",
            "source_name",
            "rel_path",
            "heading_path",
            "draft_id",
            "draft_filename",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "coach_session",
        &[
            "id",
            "draft_id",
            "state",
            "current_round",
            "max_rounds",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "coach_turn",
        &[
            "id",
            "session_id",
            "draft_id",
            "role",
            "content",
            "action",
            "round_number",
            "helpful",
            "created_at",
        ],
    ),
    (
        "ai_call_audit",
        &[
            "id",
            "kind",
            "model",
            "created_at",
            "fragment_count",
            "fragment_ids",
            "status",
        ],
    ),
];

/// 修订号与来源设备每条记录都要带，但不算业务字段，单独放在 [`SyncRecord`] 上。
const STRUCTURAL_COLUMNS: &[&str] = &["rev", "origin_device"];

/// 一张表申报：发几条、带哪些字段。发送前给用户看的就是这份清单。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeclaredTable {
    pub table: &'static str,
    pub record_count: usize,
    pub fields: Vec<&'static str>,
}

/// 载荷里的一条作者资产。身份是逻辑 ID，修订与来源设备随行。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRecord {
    pub table: &'static str,
    pub id: String,
    pub rev: i64,
    pub origin_device: Option<String>,
    pub fields: Map<String, Value>,
}

/// 同步出站包：一个设备一次上报的全部作者资产快照。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncBundle {
    pub device_id: String,
    pub generated_at: String,
    pub record_count: usize,
    pub records: Vec<SyncRecord>,
}

impl SyncBundle {
    /// 从当前库构造同步载荷。草稿正文不在 `draft` 表里——真相在磁盘 `.md`
    /// （铁律 4），所以按文件名经 [`DocumentStore`] 读出后附上；读不到就带 `null`，
    /// 「文件被用户删了」也是要让远端知道的事实。
    pub fn build(
        repo: &Repository,
        drafts: &DraftsService,
        device_id: &str,
    ) -> Result<Self, EgressError> {
        let store = DocumentStore::new(drafts.dir());
        let mut records: Vec<SyncRecord> = Vec::new();

        for (table, columns) in SYNCED_ASSETS {
            for column in *columns {
                if DEVICE_LOCAL_COLUMNS.contains(column) {
                    return Err(EgressError::DeviceLocalColumn((*column).to_string()));
                }
            }

            let mut select: Vec<&str> = columns.to_vec();
            select.extend_from_slice(STRUCTURAL_COLUMNS);
            let sql = format!(
                "SELECT {} FROM \"{table}\" ORDER BY rowid",
                select.join(", ")
            );
            let rows = repo.read(move |conn| {
                let mut stmt = conn.prepare(&sql)?;
                let names: Vec<String> = stmt
                    .column_names()
                    .iter()
                    .map(|name| (*name).to_string())
                    .collect();
                let mut out: Vec<(i64, Option<String>, Map<String, Value>)> = Vec::new();
                let mut rows = stmt.query([])?;
                while let Some(row) = rows.next()? {
                    let mut fields = Map::new();
                    let mut rev = 0_i64;
                    let mut origin_device = None;
                    for (index, name) in names.iter().enumerate() {
                        let value = match row.get_ref(index)? {
                            rusqlite::types::ValueRef::Null => Value::Null,
                            rusqlite::types::ValueRef::Integer(i) => Value::from(i),
                            rusqlite::types::ValueRef::Real(r) => Value::from(r),
                            rusqlite::types::ValueRef::Text(t) => {
                                Value::from(String::from_utf8_lossy(t).into_owned())
                            }
                            rusqlite::types::ValueRef::Blob(b) => {
                                Value::from(String::from_utf8_lossy(b).into_owned())
                            }
                        };
                        match name.as_str() {
                            "rev" => rev = value.as_i64().unwrap_or(0),
                            "origin_device" => {
                                origin_device = value.as_str().map(|s| s.to_string())
                            }
                            _ => {
                                fields.insert(name.clone(), value);
                            }
                        }
                    }
                    out.push((rev, origin_device, fields));
                }
                Ok(out)
            })?;

            for (rev, origin_device, mut fields) in rows {
                if *table == "draft" {
                    let filename = fields
                        .get("filename")
                        .and_then(|value| value.as_str())
                        .map(|name| name.to_string());
                    let body = filename
                        .as_deref()
                        .and_then(|name| store.read_to_string(name).ok());
                    fields.insert(
                        DRAFT_BODY_FIELD.to_string(),
                        body.map(Value::from).unwrap_or(Value::Null),
                    );
                }
                let id = fields
                    .get("id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string();
                records.push(SyncRecord {
                    table,
                    id,
                    rev,
                    origin_device,
                    fields,
                });
            }
        }

        Ok(Self {
            device_id: device_id.to_string(),
            generated_at: now_iso(),
            record_count: records.len(),
            records,
        })
    }

    /// 「生成即明示」：每张表的条数与字段，含结构列。UI 在按同步之前必须先摆出这份。
    pub fn declared(&self) -> Vec<DeclaredTable> {
        SYNCED_ASSETS
            .iter()
            .map(|(table, columns)| {
                let mut fields: Vec<&'static str> = columns.to_vec();
                if *table == "draft" {
                    fields.push(DRAFT_BODY_FIELD);
                }
                fields.extend_from_slice(STRUCTURAL_COLUMNS);
                DeclaredTable {
                    table,
                    record_count: self
                        .records
                        .iter()
                        .filter(|record| record.table == *table)
                        .count(),
                    fields,
                }
            })
            .collect()
    }

    /// 被送出去的逻辑身份清单，写进审计（`kind = 'sync'`）用。
    pub fn record_ids(&self) -> Vec<String> {
        self.records
            .iter()
            .map(|record| record.id.clone())
            .collect()
    }
}

/// 草稿正文不在数据库里，是同步时额外附上的一列。
const DRAFT_BODY_FIELD: &str = "content";

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
