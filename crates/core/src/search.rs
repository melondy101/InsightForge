//! 本地关键词检索：FTS5 trigram + bm25() 排序 + snippet() 上下文。
//!
//! 公共接口：
//! - `SearchService::query(source_id, query, limit)` → `Vec<EvidenceCard>`
//! - `SearchService::open_context(source_id, fragment_id)` → `Option<FragmentContext>`

use std::sync::Arc;

use serde::Serialize;

use crate::repository::Repository;
use crate::store;

/// 证据卡：前端薄投影，不含本机路径。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceCard {
    pub fragment_id: i64,
    pub source_id: String,
    pub source_name: String,
    pub rel_path: String,
    pub heading_path: String,
    pub snippet: String,
    pub bm25: f64,
    pub start_line: i64,
    pub end_line: i64,
}

/// 片段上下文（打开证据卡时展示更大段文本）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FragmentContext {
    pub fragment_id: i64,
    pub source_id: String,
    pub source_name: String,
    pub rel_path: String,
    pub heading_path: String,
    pub text: String,
    pub start_line: i64,
    pub end_line: i64,
}

/// 检索错误。
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("repository error: {0}")]
    Repository(#[from] crate::repository::RepositoryError),
    #[error("source not found: {0}")]
    SourceNotFound(String),
    #[error("invalid query: {0}")]
    InvalidQuery(String),
}

impl SearchError {
    /// 稳定错误码，供前端按码做 i18n 映射。
    pub fn code(&self) -> &'static str {
        match self {
            SearchError::Repository(_) => "repository_error",
            SearchError::SourceNotFound(_) => "source_not_found",
            SearchError::InvalidQuery(_) => "invalid_query",
        }
    }
}

/// 搜索服务：只读检索，不修改数据。
pub struct SearchService {
    repo: Arc<Repository>,
}

impl SearchService {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self { repo }
    }

    /// 在指定来源中检索关键词，返回按 bm25 排序的证据卡。
    /// 两字符查询回退到受限线性扫描（边界匹配）。
    pub fn query(
        &self,
        source_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<EvidenceCard>, SearchError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        if trimmed.chars().count() <= 2 {
            return self.fallback_linear_scan(source_id, trimmed, limit);
        }

        let source_id_owned = source_id.to_string();
        let trimmed_owned = trimmed.to_string();
        let limit_i64 = limit as i64;

        let cards = self.repo.read(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT f.id, f.text, f.heading_path, f.start_line, f.end_line,
                        ks.name, ks.id, sf.rel_path,
                        snippet(fragment_fts, 1, '<mark>', '</mark>', '...', 20) AS snippet,
                        bm25(fragment_fts) AS bm25
                 FROM fragment f
                 JOIN source_file sf ON sf.id = f.source_file_id
                 JOIN knowledge_source ks ON ks.id = sf.source_id
                 JOIN fragment_fts ON fragment_fts.fragment_id = f.id
                 WHERE sf.source_id = ?1 AND fragment_fts MATCH ?2
                 ORDER BY bm25(fragment_fts)
                 LIMIT ?3",
            )?;

            let mut cursor =
                stmt.query(rusqlite::params![source_id_owned, trimmed_owned, limit_i64])?;
            let mut cards = Vec::new();
            while let Some(row) = cursor.next()? {
                cards.push(EvidenceCard {
                    fragment_id: row.get(0)?,
                    source_id: row.get(6)?,
                    source_name: row.get(5)?,
                    rel_path: row.get(7)?,
                    heading_path: row.get(2)?,
                    snippet: row.get(8)?,
                    bm25: row.get(9)?,
                    start_line: row.get(3)?,
                    end_line: row.get(4)?,
                });
            }
            Ok(cards)
        })?;

        Ok(cards)
    }

    /// 两字符查询的回退方案：受限线性扫描。
    /// 限制：仅匹配片段边界（行首/行尾/标点后），避免全表 LIKE 爆炸。
    fn fallback_linear_scan(
        &self,
        source_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<EvidenceCard>, SearchError> {
        let source_id_owned = source_id.to_string();
        let query_owned = query.to_string();
        let source = self.repo.read({
            let source_id = source_id_owned.clone();
            move |conn| store::get_source(conn, &source_id)
        })?;
        let source = source.ok_or_else(|| SearchError::SourceNotFound(source_id_owned.clone()))?;

        let results: Vec<EvidenceCard> = self.repo.read(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT f.id, f.text, f.heading_path, f.start_line, f.end_line,
                        ks.name, ks.id, sf.rel_path
                 FROM fragment f
                 JOIN source_file sf ON sf.id = f.source_file_id
                 JOIN knowledge_source ks ON ks.id = sf.source_id
                 WHERE sf.source_id = ?1
                 ORDER BY f.source_file_id, f.ordinal",
            )?;

            let q = query_owned.to_lowercase();
            let mut results = Vec::new();
            let mut cursor = stmt.query([&source.id])?;
            while let Some(row) = cursor.next()? {
                let fid = row.get::<_, i64>(0)?;
                let text = row.get::<_, String>(1)?;
                let heading = row.get::<_, String>(2)?;
                let start = row.get::<_, i64>(3)?;
                let end = row.get::<_, i64>(4)?;
                let name = row.get::<_, String>(5)?;
                let sid = row.get::<_, String>(6)?;
                let rel_path = row.get::<_, String>(7)?;
                let text_lower = text.to_lowercase();

                {
                    let mut pos = 0;
                    while let Some(idx) = text_lower[pos..].find(&q) {
                        let abs_idx = pos + idx;
                        let before = if abs_idx == 0 {
                            true
                        } else {
                            let ch = text_lower.chars().nth(abs_idx - 1).unwrap();
                            ch == '\n'
                                || ch == ' '
                                || ch == '\t'
                                || ch == ','
                                || ch == '.'
                                || ch == ';'
                                || ch == ':'
                                || ch == '，'
                                || ch == '。'
                                || ch == '；'
                                || ch == '：'
                        };
                        let after = abs_idx + q.len();
                        let after_ok = after >= text_lower.len()
                            || matches!(
                                text_lower.chars().next(),
                                Some(
                                    '\n' | ' '
                                        | '\t'
                                        | ','
                                        | '.'
                                        | ';'
                                        | ':'
                                        | '，'
                                        | '。'
                                        | '；'
                                        | '：'
                                )
                            );
                        if before && after_ok {
                            let snippet = Self::extract_snippet(&text, abs_idx, q.len());
                            results.push(EvidenceCard {
                                fragment_id: fid,
                                source_id: sid.clone(),
                                source_name: name.clone(),
                                rel_path: rel_path.clone(),
                                heading_path: heading.clone(),
                                snippet,
                                bm25: 0.0,
                                start_line: start,
                                end_line: end,
                            });
                            if results.len() >= limit {
                                break;
                            }
                        }
                        pos = abs_idx + 1;
                    }
                }
            }
            Ok(results)
        })?;

        Ok(results)
    }

    fn extract_snippet(text: &str, center: usize, width: usize) -> String {
        let start = center.saturating_sub(width * 4);
        let end = (center + width * 4).min(text.len());
        let mut s = text[start..end].to_string();
        if start > 0 {
            s.insert(0, '…');
        }
        if end < text.len() {
            s.push('…');
        }
        s
    }

    /// 打开片段上下文：返回更大段文本。
    pub fn open_context(
        &self,
        source_id: &str,
        fragment_id: i64,
    ) -> Result<Option<FragmentContext>, SearchError> {
        let source_id_owned = source_id.to_string();
        let source = self.repo.read({
            let source_id = source_id_owned.clone();
            move |conn| store::get_source(conn, &source_id)
        })?;
        let source = match source {
            Some(s) => s,
            None => return Ok(None),
        };

        let ctx = self.repo.read(move |conn| {
            let _ = &source; // suppress unused warning

            let mut stmt = conn.prepare(
                "SELECT f.id, f.text, f.heading_path, f.start_line, f.end_line,
                        ks.name, ks.id, sf.rel_path
                 FROM fragment f
                 JOIN source_file sf ON sf.id = f.source_file_id
                 JOIN knowledge_source ks ON ks.id = sf.source_id
                 WHERE f.id = ?1 AND sf.source_id = ?2",
            )?;

            let result = stmt.query_row(rusqlite::params![fragment_id, source_id_owned], |row| {
                Ok(FragmentContext {
                    fragment_id: row.get(0)?,
                    source_id: row.get(6)?,
                    source_name: row.get(5)?,
                    rel_path: row.get(7)?,
                    heading_path: row.get(2)?,
                    text: row.get(1)?,
                    start_line: row.get(3)?,
                    end_line: row.get(4)?,
                })
            });

            Ok(result.ok())
        })?;

        Ok(ctx)
    }
}
