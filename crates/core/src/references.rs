//! 草稿引用快照：用户在草稿中引用证据卡时，写入不可变快照。
//!
//! 关键设计：
//! - `draft_reference` 只存片段级快照（snippet + full_text），不依赖源文件后续存在。
//! - 来源文件删除后，引用快照保留，仅标记 `source_deleted = 1`，草稿正文不受影响。
//! - 重建索引不触碰 `draft_reference`，满足「非派生数据」验收。

use std::sync::Arc;

use chrono::Utc;
use thiserror::Error;

use crate::repository::Repository;

#[derive(Debug, Error)]
pub enum ReferencesError {
    #[error("repository error: {0}")]
    Repository(#[from] crate::repository::RepositoryError),

    #[error("draft not found: {0}")]
    DraftNotFound(String),

    #[error("reference not found: {0}")]
    ReferenceNotFound(String),

    #[error("invalid reference: {0}")]
    InvalidReference(String),
}

impl ReferencesError {
    pub fn code(&self) -> &'static str {
        match self {
            ReferencesError::Repository(_) => "repository_error",
            ReferencesError::DraftNotFound(_) => "draft_not_found",
            ReferencesError::ReferenceNotFound(_) => "reference_not_found",
            ReferencesError::InvalidReference(_) => "invalid_reference",
        }
    }
}

/// 草稿引用快照（不可变）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub id: String,
    pub draft_id: String,
    pub fragment_id: i64,
    pub source_id: String,
    pub source_name: String,
    pub rel_path: String,
    pub heading_path: String,
    pub snippet: String,
    pub full_text: String,
    pub start_line: i64,
    pub end_line: i64,
    pub adopted: bool,
    pub source_deleted: bool,
    pub created_at: String,
}

/// 创建引用时的输入（前端只传片段 ID 与草稿位置，服务端负责快照）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReference {
    pub draft_id: String,
    pub fragment_id: i64,
}

/// 引用服务。
pub struct ReferencesService {
    repo: Arc<Repository>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct FragmentSnapshot {
    pub fragment_id: i64,
    pub source_file_id: String,
    pub heading_path: String,
    pub start_line: i64,
    pub end_line: i64,
    pub text: String,
    pub rel_path: String,
    pub source_name: String,
    pub source_id: String,
}

impl ReferencesService {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self { repo }
    }

    /// 为草稿创建引用快照。
    pub fn create(&self, input: &CreateReference) -> Result<Reference, ReferencesError> {
        let fragment_id = input.fragment_id;
        let snapshot_result = self.repo.read(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT f.id, f.source_file_id, f.heading_path, f.start_line, f.end_line, f.text,
                        sf.rel_path, ks.name, ks.id
                 FROM fragment f
                 JOIN source_file sf ON sf.id = f.source_file_id
                 JOIN knowledge_source ks ON ks.id = sf.source_id
                 WHERE f.id = ?1",
            )?;
            let result = stmt.query_row(rusqlite::params![fragment_id], |row| {
                Ok(FragmentSnapshot {
                    fragment_id: row.get(0)?,
                    source_file_id: row.get(1)?,
                    heading_path: row.get(2)?,
                    start_line: row.get(3)?,
                    end_line: row.get(4)?,
                    text: row.get(5)?,
                    rel_path: row.get(6)?,
                    source_name: row.get(7)?,
                    source_id: row.get(8)?,
                })
            });
            Ok(result)
        });

        let snapshot = match snapshot_result {
            Ok(Ok(s)) => s,
            Ok(Err(rusqlite::Error::QueryReturnedNoRows)) => {
                return Err(ReferencesError::InvalidReference(
                    "fragment not found".to_string(),
                ));
            }
            Ok(Err(err)) => return Err(ReferencesError::Repository(err.into())),
            Err(err) => return Err(ReferencesError::Repository(err)),
        };

        let now = now_iso();
        let reference = Reference {
            id: generate_id(),
            draft_id: input.draft_id.clone(),
            fragment_id: input.fragment_id,
            source_id: snapshot.source_id,
            source_name: snapshot.source_name,
            rel_path: snapshot.rel_path,
            heading_path: snapshot.heading_path,
            snippet: extract_snippet(&snapshot.text, 200),
            full_text: snapshot.text,
            start_line: snapshot.start_line,
            end_line: snapshot.end_line,
            adopted: false,
            source_deleted: false,
            created_at: now,
        };

        let ref_for_write = reference.clone();
        self.repo.write(move |conn| {
            conn.execute(
                "INSERT INTO draft_reference
                     (id, draft_id, fragment_id, source_id, source_name, rel_path, heading_path,
                      snippet, full_text, start_line, end_line, adopted, source_deleted, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                rusqlite::params![
                    ref_for_write.id,
                    ref_for_write.draft_id,
                    ref_for_write.fragment_id,
                    ref_for_write.source_id,
                    ref_for_write.source_name,
                    ref_for_write.rel_path,
                    ref_for_write.heading_path,
                    ref_for_write.snippet,
                    ref_for_write.full_text,
                    ref_for_write.start_line,
                    ref_for_write.end_line,
                    i64::from(ref_for_write.adopted),
                    i64::from(ref_for_write.source_deleted),
                    ref_for_write.created_at,
                ],
            )?;
            Ok(())
        })?;

        Ok(reference)
    }

    /// 列出草稿的全部引用。
    pub fn list(&self, draft_id: &str) -> Result<Vec<Reference>, ReferencesError> {
        let draft_id = draft_id.to_string();
        let rows = self.repo.read(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, draft_id, fragment_id, source_id, source_name, rel_path, heading_path,
                        snippet, full_text, start_line, end_line, adopted, source_deleted, created_at
                 FROM draft_reference
                 WHERE draft_id = ?1
                 ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map(rusqlite::params![draft_id], |row| {
                Ok(Reference {
                    id: row.get(0)?,
                    draft_id: row.get(1)?,
                    fragment_id: row.get(2)?,
                    source_id: row.get(3)?,
                    source_name: row.get(4)?,
                    rel_path: row.get(5)?,
                    heading_path: row.get(6)?,
                    snippet: row.get(7)?,
                    full_text: row.get(8)?,
                    start_line: row.get(9)?,
                    end_line: row.get(10)?,
                    adopted: row.get::<_, i64>(11)? != 0,
                    source_deleted: row.get::<_, i64>(12)? != 0,
                    created_at: row.get(13)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
        })?;
        Ok(rows)
    }

    /// 标记引用为已采用（用户确认引用）。
    pub fn adopt(&self, id: &str) -> Result<(), ReferencesError> {
        let id = id.to_string();
        self.repo.write(move |conn| {
            conn.execute(
                "UPDATE draft_reference SET adopted = 1 WHERE id = ?1",
                rusqlite::params![id],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    /// 标记来源已删除（触发于 04 号票的删除收敛）。
    pub fn mark_source_deleted(&self, source_id: &str) -> Result<(), ReferencesError> {
        let source_id = source_id.to_string();
        self.repo.write(move |conn| {
            conn.execute(
                "UPDATE draft_reference SET source_deleted = 1 WHERE source_id = ?1",
                rusqlite::params![source_id],
            )?;
            Ok(())
        })?;
        Ok(())
    }
}

fn extract_snippet(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut s = text.chars().take(max_chars).collect::<String>();
    s.push('…');
    s
}

fn now_iso() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%fZ").to_string()
}

fn generate_id() -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or(0)
        .hash(&mut hasher);
    format!("{:x}", hasher.finish())
}
