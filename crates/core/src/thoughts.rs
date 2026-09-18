//! 思考卡（11 号票）：用户把「想法 ↔ 素材 ↔ 草稿位置」的关系确认为思考卡，
//! 在 UI 中查看、跳转；重建索引后思考卡、引用快照与保存基线依然存在——它们是
//! 作者资产，不是派生缓存。

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::clock::now_iso;
use crate::drafts::DraftsService;
use crate::identity::new_id;
use crate::repository::Repository;

/// 思考卡：连接想法、素材与草稿位置。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThoughtCard {
    pub id: String,
    pub idea: String,
    pub source_id: String,
    pub source_name: String,
    pub rel_path: String,
    pub heading_path: String,
    pub draft_id: String,
    pub draft_filename: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 创建/更新思考卡的输入。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertThoughtCard {
    pub id: Option<String>,
    pub idea: String,
    pub source_id: String,
    pub source_name: String,
    pub rel_path: String,
    pub heading_path: String,
    pub draft_id: String,
    pub draft_filename: String,
}

/// 思考卡错误。
#[derive(Debug, Error)]
pub enum ThoughtsError {
    #[error("repository error: {0}")]
    Repository(#[from] crate::repository::RepositoryError),
    #[error("drafts error: {0}")]
    Drafts(#[from] crate::drafts::DraftsError),
    #[error("invalid thought card: {0}")]
    InvalidThoughtCard(String),
}

/// 思考卡服务：CRUD + 按草稿/来源查询。
pub struct ThoughtsService {
    repo: Arc<Repository>,
    drafts: Arc<DraftsService>,
}

impl ThoughtsService {
    pub fn new(repo: Arc<Repository>, drafts: Arc<DraftsService>) -> Self {
        Self { repo, drafts }
    }

    /// 创建或更新思考卡。
    pub fn upsert(&self, input: &UpsertThoughtCard) -> Result<ThoughtCard, ThoughtsError> {
        let now = now_iso();
        let id = input.id.clone().unwrap_or_else(new_id);

        // 验证草稿是否存在。
        let _draft = self.drafts.find_draft(&input.draft_filename)?;

        let record = ThoughtCard {
            id: id.clone(),
            idea: input.idea.clone(),
            source_id: input.source_id.clone(),
            source_name: input.source_name.clone(),
            rel_path: input.rel_path.clone(),
            heading_path: input.heading_path.clone(),
            draft_id: input.draft_id.clone(),
            draft_filename: input.draft_filename.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        let record_for_write = record.clone();
        self.repo.write(move |conn| {
            conn.execute(
                "INSERT INTO thought_card
                     (id, idea, source_id, source_name, rel_path, heading_path,
                      draft_id, draft_filename, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT (id) DO UPDATE SET
                     idea = excluded.idea,
                     source_id = excluded.source_id,
                     source_name = excluded.source_name,
                     rel_path = excluded.rel_path,
                     heading_path = excluded.heading_path,
                     draft_id = excluded.draft_id,
                     draft_filename = excluded.draft_filename,
                     updated_at = excluded.updated_at",
                rusqlite::params![
                    record_for_write.id,
                    record_for_write.idea,
                    record_for_write.source_id,
                    record_for_write.source_name,
                    record_for_write.rel_path,
                    record_for_write.heading_path,
                    record_for_write.draft_id,
                    record_for_write.draft_filename,
                    record_for_write.created_at,
                    record_for_write.updated_at,
                ],
            )?;
            Ok(())
        })?;

        Ok(record)
    }

    /// 列出全部思考卡。
    pub fn list(&self) -> Result<Vec<ThoughtCard>, ThoughtsError> {
        let cards = self.repo.read(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, idea, source_id, source_name, rel_path, heading_path,
                        draft_id, draft_filename, created_at, updated_at
                 FROM thought_card
                 ORDER BY updated_at DESC",
            )?;
            let mut rows = stmt.query(rusqlite::params![])?;
            let mut cards = Vec::new();
            while let Some(row) = rows.next()? {
                cards.push(ThoughtCard {
                    id: row.get(0)?,
                    idea: row.get(1)?,
                    source_id: row.get(2)?,
                    source_name: row.get(3)?,
                    rel_path: row.get(4)?,
                    heading_path: row.get(5)?,
                    draft_id: row.get(6)?,
                    draft_filename: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                });
            }
            Ok(cards)
        })?;
        Ok(cards)
    }

    /// 按草稿 ID 列出思考卡。
    pub fn list_by_draft(&self, draft_id: &str) -> Result<Vec<ThoughtCard>, ThoughtsError> {
        let draft_id = draft_id.to_string();
        let cards = self.repo.read(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, idea, source_id, source_name, rel_path, heading_path,
                        draft_id, draft_filename, created_at, updated_at
                 FROM thought_card
                 WHERE draft_id = ?1
                 ORDER BY updated_at DESC",
            )?;
            let mut rows = stmt.query(rusqlite::params![draft_id])?;
            let mut cards = Vec::new();
            while let Some(row) = rows.next()? {
                cards.push(ThoughtCard {
                    id: row.get(0)?,
                    idea: row.get(1)?,
                    source_id: row.get(2)?,
                    source_name: row.get(3)?,
                    rel_path: row.get(4)?,
                    heading_path: row.get(5)?,
                    draft_id: row.get(6)?,
                    draft_filename: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                });
            }
            Ok(cards)
        })?;
        Ok(cards)
    }

    /// 按来源 ID 列出思考卡。
    pub fn list_by_source(&self, source_id: &str) -> Result<Vec<ThoughtCard>, ThoughtsError> {
        let source_id = source_id.to_string();
        let cards = self.repo.read(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, idea, source_id, source_name, rel_path, heading_path,
                        draft_id, draft_filename, created_at, updated_at
                 FROM thought_card
                 WHERE source_id = ?1
                 ORDER BY updated_at DESC",
            )?;
            let mut rows = stmt.query(rusqlite::params![source_id])?;
            let mut cards = Vec::new();
            while let Some(row) = rows.next()? {
                cards.push(ThoughtCard {
                    id: row.get(0)?,
                    idea: row.get(1)?,
                    source_id: row.get(2)?,
                    source_name: row.get(3)?,
                    rel_path: row.get(4)?,
                    heading_path: row.get(5)?,
                    draft_id: row.get(6)?,
                    draft_filename: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                });
            }
            Ok(cards)
        })?;
        Ok(cards)
    }

    /// 删除思考卡。
    pub fn delete(&self, id: &str) -> Result<(), ThoughtsError> {
        let id = id.to_string();
        self.repo.write(move |conn| {
            conn.execute(
                "DELETE FROM thought_card WHERE id = ?1",
                rusqlite::params![id],
            )?;
            Ok(())
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn upsert_then_list_thought_card() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("insightforge.sqlite");
        let repo = Arc::new(Repository::open(&db_path).unwrap());
        let drafts_dir = dir.path().join("drafts");
        std::fs::create_dir_all(&drafts_dir).unwrap();
        let drafts = Arc::new(DraftsService::new(Arc::clone(&repo), drafts_dir));
        let service = ThoughtsService::new(repo, drafts);

        let card = service
            .upsert(&UpsertThoughtCard {
                id: None,
                idea: "Test idea".to_string(),
                source_id: "s1".to_string(),
                source_name: "Source 1".to_string(),
                rel_path: "a.md".to_string(),
                heading_path: "Heading".to_string(),
                draft_id: "d1".to_string(),
                draft_filename: "draft.md".to_string(),
            })
            .unwrap();

        assert!(!card.id.is_empty());
        let list = service.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].idea, "Test idea");
    }
}
