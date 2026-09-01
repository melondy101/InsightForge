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
    ///
    /// 采用字符级迭代避免多字节 UTF-8 字节切片 panic。
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

        /// True if `ch` is a word boundary separator.
        fn is_separator(ch: char) -> bool {
            matches!(
                ch,
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
                    | '、'
                    | '！'
                    | '？'
                    | '「'
                    | '」'
                    | '『'
                    | '』'
                    | '（'
                    | '）'
                    | '【'
                    | '】'
            )
        }

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

            let q_chars: Vec<char> = query_owned.to_lowercase().chars().collect();
            let q_len = q_chars.len();
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

                let text_chars: Vec<char> = text.chars().collect();
                let text_lower: Vec<char> =
                    text_chars.iter().map(|c| c.to_ascii_lowercase()).collect();

                // Iterate over each character position as a potential match start
                let mut i = 0;
                while i + q_len <= text_lower.len() {
                    // Check if query matches at position i
                    let matched = (0..q_len).all(|j| text_lower[i + j] == q_chars[j]);
                    if matched {
                        // Check word boundary before
                        let before_ok = i == 0 || is_separator(text_lower[i - 1]);
                        // Check word boundary after
                        let after_ok =
                            i + q_len >= text_lower.len() || is_separator(text_lower[i + q_len]);
                        if before_ok && after_ok {
                            // Compute char index for snippet extraction
                            let snippet = Self::extract_snippet(&text, i, q_len);
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
                            // Skip past this match
                            i += q_len;
                            continue;
                        }
                    }
                    i += 1;
                }
            }
            Ok(results)
        })?;

        Ok(results)
    }

    fn extract_snippet(text: &str, center_char_idx: usize, width_chars: usize) -> String {
        let char_count = text.chars().count();
        let start_char = center_char_idx.saturating_sub(width_chars * 4);
        let end_char = (center_char_idx + width_chars * 4).min(char_count);

        let start_byte = text
            .char_indices()
            .nth(start_char)
            .map(|(i, _)| i)
            .unwrap_or(0);
        let end_byte = text
            .char_indices()
            .nth(end_char)
            .map(|(i, _)| i)
            .unwrap_or(text.len());

        let mut s = text[start_byte..end_byte].to_string();
        if start_char > 0 {
            s.insert(0, '…');
        }
        if end_char < char_count {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Seed a source + source_file + fragment + FTS row directly into the repo.
    /// Follows references.rs pattern: &Repository, move closure, all owned Strings.
    fn seed_with_fts(repo: &Repository, source_id: &str, frag_id: i64, frag_text: &str) -> String {
        let source_id_owned = source_id.to_string();
        let text_owned = frag_text.to_string();
        let sf_id_val = frag_id + 1000; // source_file.id offset
        let fts_text = text_owned.clone();
        let return_sid = source_id_owned.clone();
        let root_path = format!("/test/{}", source_id_owned);
        repo.write(move |conn| {
            conn.execute(
                "INSERT INTO knowledge_source (id, name, root_path) VALUES (?1, ?2, ?3)",
                rusqlite::params![source_id_owned, "Test Source", root_path],
            )?;
            conn.execute(
                "INSERT INTO source_file (id, source_id, rel_path, content_hash, size, mtime_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![sf_id_val, source_id_owned, "doc.md", "h1", 100, 0],
            )?;
            conn.execute(
                "INSERT INTO fragment (id, source_file_id, ordinal, heading_path, start_line, end_line, text, content_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![frag_id, sf_id_val, 0, "H", 1, 5, text_owned, "h2"],
            )?;
            // Also insert into FTS5 virtual table
            conn.execute(
                "INSERT INTO fragment_fts (fragment_id, text) VALUES (?1, ?2)",
                rusqlite::params![frag_id, fts_text],
            )?;
            Ok(())
        })
        .expect("seed_with_fts failed");
        return_sid
    }

    fn setup_service() -> (Arc<Repository>, SearchService, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("insightforge.sqlite");
        let repo = Arc::new(Repository::open(&db_path).unwrap());
        let service = SearchService::new(Arc::clone(&repo));
        (repo, service, dir)
    }

    /// 二字词回退：片段包含"知识"（前后有空格分隔），查询"知识"应命中。
    #[test]
    fn fallback_linear_scan_hits_two_char_query() {
        let (repo, service, _dir) = setup_service();
        seed_with_fts(&repo, "src-1", 1, "知识 管理是核心流程");

        let results = service
            .query("src-1", "知识", 10)
            .expect("query should succeed");
        assert_eq!(results.len(), 1, "expected 1 result, got {}", results.len());
        assert!(results[0].snippet.contains("知识"));
    }

    /// 二字词回退：片段不含查询词时返回空。
    #[test]
    fn fallback_linear_scan_returns_empty_when_no_match() {
        let (repo, service, _dir) = setup_service();
        seed_with_fts(&repo, "src-2", 1, "人工智能改变世界");

        let results = service
            .query("src-2", "知识", 10)
            .expect("query should succeed");
        assert!(results.is_empty());
    }

    /// 二字词回退：边界匹配（中文标点后的词应匹配）。
    #[test]
    fn fallback_linear_scan_respects_word_boundaries() {
        let (repo, service, _dir) = setup_service();
        seed_with_fts(&repo, "src-3", 1, "，知识。获取与存储");

        let results = service
            .query("src-3", "知识", 10)
            .expect("query should succeed");
        assert_eq!(
            results.len(),
            1,
            "boundary match failed: got {} results",
            results.len()
        );
    }

    /// 二字词回退：limit 正确裁剪结果数。
    #[test]
    fn fallback_linear_scan_respects_limit() {
        let (repo, service, _dir) = setup_service();
        // Source A: "知识" appears as standalone word (with trailing space)
        seed_with_fts(&repo, "src-4a", 1, "人工智能 知识 管理很重要");
        // Source B: not queried, just verifies multiple sources work
        seed_with_fts(&repo, "src-4b", 2, "知识 就是力量");

        let results = service
            .query("src-4a", "知识", 1)
            .expect("query should succeed");
        assert_eq!(
            results.len(),
            1,
            "limit=1 should return 1 result, got {} results",
            results.len()
        );
    }

    /// 三字及以上走 FTS5 主路径（非回退）。
    /// 验证：查询通过 FTS5 JOIN 路径命中；fallback 路径从不被调用（<=2 chars 才触发）。
    #[test]
    fn query_three_chars_uses_fts_not_fallback() {
        let (repo, service, _dir) = setup_service();
        seed_with_fts(&repo, "src-5", 1, "知识管理就是核心能力的核心体现");

        let results = service
            .query("src-5", "知识管理", 10)
            .expect("query should succeed");
        assert!(
            !results.is_empty(),
            "FTS5 query should find match for 4-char query"
        );
    }
}
