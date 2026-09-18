//! 草稿引用快照：用户在草稿中引用证据卡时，写入不可变快照。
//!
//! 关键设计：
//! - `draft_reference` 只存片段级快照（snippet + full_text），不依赖源文件后续存在。
//! - 来源文件删除后，引用快照保留，仅标记 `source_deleted = 1`，草稿正文不受影响。
//! - 重建索引不触碰 `draft_reference`，满足「非派生数据」验收。

use std::sync::Arc;

use chrono::{DateTime, Utc};
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
    pub source_file_id: i64,
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
        self.create_with_time(input, Utc::now())
    }

    /// 为草稿创建引用快照（注入确定性时间，便于测试）。
    pub fn create_with_time(
        &self,
        input: &CreateReference,
        now: DateTime<Utc>,
    ) -> Result<Reference, ReferencesError> {
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

        let now = format_iso(now);
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
                 ORDER BY created_at DESC, rowid DESC",
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

use crate::clock::format_iso;

fn generate_id() -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or(0)
        .hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// 向测试库写入一条 knowledge_source + source_file + fragment 链路，
    /// 返回 (source_id, fragment_id)。
    fn seed_fragment(repo: &Repository, source_id: &str, fragment_text: &str) -> (String, i64) {
        let source_id_owned = source_id.to_string();
        let text_owned = fragment_text.to_string();
        repo.write(move |conn| {
            conn.execute(
                "INSERT INTO knowledge_source (id, name, root_path) VALUES (?1, ?2, ?3)",
                rusqlite::params![source_id_owned, "Test Source", "/test/root"],
            )?;
            conn.execute(
                "INSERT INTO source_file (id, source_id, rel_path, content_hash, size, mtime_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![1, source_id_owned, "doc.md", "hash1", 100, 0],
            )?;
            conn.execute(
                "INSERT INTO fragment (id, source_file_id, ordinal, heading_path, start_line, end_line, text, content_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![1, 1, 0, "Heading", 1, 10, text_owned, "hash2"],
            )?;
            Ok(())
        })
        .expect("seed_fragment: insert failed");
        (source_id.to_string(), 1)
    }

    /// 向测试库写入一条 draft 行（draft_reference.draft_id 的外键依赖）。
    fn seed_draft(repo: &Repository, draft_id: &str) {
        let draft_id_owned = draft_id.to_string();
        repo.write(move |conn| {
            conn.execute(
                "INSERT INTO draft (id, path, filename, baseline_hash, baseline_mtime_ms, baseline_size)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![draft_id_owned, "/drafts/test.md", "test.md", "hash", 0, 0],
            )?;
            Ok(())
        })
        .expect("seed_draft: insert failed");
    }

    fn setup_service() -> (Arc<Repository>, ReferencesService, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("insightforge.sqlite");
        let repo = Arc::new(Repository::open(&db_path).unwrap());
        // draft_reference.draft_id 有外键依赖，预先插入一条 draft
        seed_draft(&repo, "draft-1");
        let service = ReferencesService::new(Arc::clone(&repo));
        (repo, service, dir)
    }

    #[test]
    fn create_reference_snapshots_fragment_and_persists() {
        let (repo, service, _dir) = setup_service();
        let (source_id, fragment_id) =
            seed_fragment(&repo, "src-1", "这是一段测试文本，用于验证引用快照。");

        let reference = service
            .create(&CreateReference {
                draft_id: "draft-1".to_string(),
                fragment_id,
            })
            .expect("create reference should succeed");

        assert!(!reference.id.is_empty());
        assert_eq!(reference.draft_id, "draft-1");
        assert_eq!(reference.fragment_id, fragment_id);
        assert_eq!(reference.source_id, source_id);
        assert_eq!(reference.source_name, "Test Source");
        assert_eq!(reference.rel_path, "doc.md");
        assert_eq!(reference.heading_path, "Heading");
        assert_eq!(reference.full_text, "这是一段测试文本，用于验证引用快照。");
        assert!(!reference.snippet.is_empty());
        assert_eq!(reference.start_line, 1);
        assert_eq!(reference.end_line, 10);
        assert!(!reference.adopted);
        assert!(!reference.source_deleted);
        assert!(!reference.created_at.is_empty());
    }

    #[test]
    fn create_reference_with_nonexistent_fragment_returns_error() {
        let (_repo, service, _dir) = setup_service();

        let result = service.create(&CreateReference {
            draft_id: "draft-1".to_string(),
            fragment_id: 9999,
        });

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), "invalid_reference");
    }

    #[test]
    fn list_references_returns_created_snapshots_ordered_by_created_at_desc() {
        let (repo, service, _dir) = setup_service();
        seed_fragment(&repo, "src-1", "文本一");

        // Deterministic timestamps: r1 earlier (us=099999), r2 later (us=100000).
        // Under %f variable-width: r1="...05.99999Z", r2="...05.1Z",
        // lexicographic DESC wrongly puts r1 (earlier) first. %6f fixed-width makes lex=time.
        // Cross-second boundary: t1 at 5.9s, t2 at 6.1s.
        // Old format "%Y-%m-%dT%H:%M:%fZ" omits %S, so t1="...00:900000000Z" and
        // t2="...00:100000000Z"; lexicographic DESC puts t1 first — WRONG.
        // Fixed format "%Y-%m-%dT%H:%M:%S.%6fZ" restores seconds, making lex=time.
        let t1 = chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:05.900000Z")
            .unwrap()
            .with_timezone(&Utc);
        let t2 = chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:06.100000Z")
            .unwrap()
            .with_timezone(&Utc);

        let r1 = service
            .create_with_time(
                &CreateReference {
                    draft_id: "draft-1".to_string(),
                    fragment_id: 1,
                },
                t1,
            )
            .unwrap();
        let r2 = service
            .create_with_time(
                &CreateReference {
                    draft_id: "draft-1".to_string(),
                    fragment_id: 1,
                },
                t2,
            )
            .unwrap();

        let list = service.list("draft-1").unwrap();
        assert_eq!(list.len(), 2);
        // created_at DESC: r2 (newer) first
        assert_eq!(list[0].id, r2.id);
        assert_eq!(list[1].id, r1.id);
    }

    #[test]
    fn list_references_for_unknown_draft_returns_empty() {
        let (_repo, service, _dir) = setup_service();
        let list = service.list("nonexistent-draft").unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn adopt_marks_reference_as_adopted() {
        let (repo, service, _dir) = setup_service();
        seed_fragment(&repo, "src-1", "文本");

        let reference = service
            .create(&CreateReference {
                draft_id: "draft-1".to_string(),
                fragment_id: 1,
            })
            .unwrap();
        assert!(!reference.adopted);

        service.adopt(&reference.id).unwrap();

        let list = service.list("draft-1").unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].adopted);
    }

    #[test]
    fn mark_source_deleted_preserves_snapshot_and_flags_source_unavailable() {
        let (repo, service, _dir) = setup_service();
        let (source_id, _) = seed_fragment(&repo, "src-del", "不可变快照文本");

        let reference = service
            .create(&CreateReference {
                draft_id: "draft-1".to_string(),
                fragment_id: 1,
            })
            .unwrap();
        assert!(!reference.source_deleted);

        // 模拟来源删除：标记 source_deleted
        service.mark_source_deleted(&source_id).unwrap();

        // 引用快照仍然存在
        let list = service.list("draft-1").unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].source_deleted);
        // 快照内容不受影响
        assert_eq!(list[0].full_text, "不可变快照文本");
        assert_eq!(list[0].source_name, "Test Source");
        assert_eq!(list[0].rel_path, "doc.md");
    }

    #[test]
    fn mark_source_deleted_only_affects_matching_source() {
        let (repo, service, _dir) = setup_service();
        seed_fragment(&repo, "src-a", "来源A的文本");
        // 为第二个来源插入数据（source_file id=2, fragment id=2）
        repo.write(move |conn| {
            conn.execute(
                "INSERT INTO knowledge_source (id, name, root_path) VALUES (?1, ?2, ?3)",
                rusqlite::params!["src-b", "Source B", "/test/b"],
            )?;
            conn.execute(
                "INSERT INTO source_file (id, source_id, rel_path, content_hash, size, mtime_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![2, "src-b", "b.md", "h", 10, 0],
            )?;
            conn.execute(
                "INSERT INTO fragment (id, source_file_id, ordinal, heading_path, start_line, end_line, text, content_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![2, 2, 0, "", 1, 5, "来源B的文本", "h2"],
            )?;
            Ok(())
        })
        .unwrap();

        let ref_a = service
            .create(&CreateReference {
                draft_id: "draft-1".to_string(),
                fragment_id: 1,
            })
            .unwrap();
        let ref_b = service
            .create(&CreateReference {
                draft_id: "draft-1".to_string(),
                fragment_id: 2,
            })
            .unwrap();

        // 只标记 src-a 为已删除
        service.mark_source_deleted("src-a").unwrap();

        let list = service.list("draft-1").unwrap();
        let a = list.iter().find(|r| r.id == ref_a.id).unwrap();
        let b = list.iter().find(|r| r.id == ref_b.id).unwrap();
        assert!(a.source_deleted);
        assert!(!b.source_deleted);
    }

    #[test]
    fn long_text_snippet_is_truncated_with_ellipsis() {
        let (repo, service, _dir) = setup_service();
        let long_text = "一".repeat(250);
        seed_fragment(&repo, "src-1", &long_text);

        let reference = service
            .create(&CreateReference {
                draft_id: "draft-1".to_string(),
                fragment_id: 1,
            })
            .unwrap();

        // snippet 最多 200 字符 + 省略号
        assert!(reference.snippet.chars().count() <= 201);
        assert!(reference.snippet.ends_with('…'));
        // full_text 完整保留
        assert_eq!(reference.full_text.chars().count(), 250);
    }
}
