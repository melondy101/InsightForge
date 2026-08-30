//! 领域 SQL 访问层：所有语句都在 repository 单写队列的连接上执行
//! （写操作自动包在 `BEGIN IMMEDIATE` 事务里，见 01 号票 repository）。

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::chunking::Chunk;

/// knowledge_source 的存储行。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSource {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub paused: bool,
    #[serde(default)]
    pub ignore_globs: Vec<String>,
    #[serde(default)]
    pub last_synced_at: Option<String>,
}

/// source_file 的存储行（本票消费 hash/size/mtime 做增量判断；
/// file identity 的读回归属 04 号票的重命名解析）。
#[derive(Debug, Clone)]
pub struct StoredFile {
    pub rel_path: String,
    pub content_hash: String,
    pub size: i64,
    pub mtime_ms: i64,
}

/// 一个待写入的片段（来自分块纯函数 + 内容 hash）。
pub struct FragmentUpsert<'a> {
    pub ordinal: usize,
    pub chunk: &'a Chunk,
    pub content_hash: &'a str,
}

fn parse_globs(text: &str) -> Vec<String> {
    serde_json::from_str(text).unwrap_or_default()
}

pub fn insert_source(conn: &Connection, source: &StoredSource) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO knowledge_source (id, name, root_path, paused, ignore_globs)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            source.id,
            source.name,
            source.root_path,
            i64::from(source.paused),
            serde_json::to_string(&source.ignore_globs).expect("serialize ignore globs"),
        ],
    )?;
    Ok(())
}

pub fn find_id_by_root(conn: &Connection, root_path: &str) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT id FROM knowledge_source WHERE root_path = ?1")?;
    let mut rows = stmt.query(rusqlite::params![root_path])?;
    match rows.next()? {
        Some(row) => Ok(Some(row.get(0)?)),
        None => Ok(None),
    }
}

fn row_to_source(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredSource> {
    let globs_text: String = row.get("ignore_globs")?;
    Ok(StoredSource {
        id: row.get("id")?,
        name: row.get("name")?,
        root_path: row.get("root_path")?,
        paused: row.get::<_, i64>("paused")? != 0,
        ignore_globs: parse_globs(&globs_text),
        last_synced_at: row.get("last_synced_at")?,
    })
}

const SOURCE_COLUMNS: &str = "id, name, root_path, paused, ignore_globs, last_synced_at";

pub fn get_source(conn: &Connection, id: &str) -> rusqlite::Result<Option<StoredSource>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SOURCE_COLUMNS} FROM knowledge_source WHERE id = ?1"
    ))?;
    let mut rows = stmt.query(rusqlite::params![id])?;
    match rows.next()? {
        Some(row) => Ok(Some(row_to_source(row)?)),
        None => Ok(None),
    }
}

/// 全部来源 + 各自已索引文件数（用于来源列表展示）。
pub fn list_sources(conn: &Connection) -> rusqlite::Result<Vec<(StoredSource, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT k.id, k.name, k.root_path, k.paused, k.ignore_globs, k.last_synced_at,
                (SELECT COUNT(*) FROM source_file sf WHERE sf.source_id = k.id) AS file_count
         FROM knowledge_source k ORDER BY k.created_at, k.id",
    )?;
    let rows = stmt.query_map([], |row| {
        let globs_text: String = row.get("ignore_globs")?;
        Ok((
            StoredSource {
                id: row.get("id")?,
                name: row.get("name")?,
                root_path: row.get("root_path")?,
                paused: row.get::<_, i64>("paused")? != 0,
                ignore_globs: parse_globs(&globs_text),
                last_synced_at: row.get("last_synced_at")?,
            },
            row.get("file_count")?,
        ))
    })?;
    rows.collect()
}

pub fn set_paused(conn: &Connection, id: &str, paused: bool) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE knowledge_source SET paused = ?2 WHERE id = ?1",
        rusqlite::params![id, i64::from(paused)],
    )
}

pub fn touch_last_synced(conn: &Connection, id: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE knowledge_source
         SET last_synced_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1",
        rusqlite::params![id],
    )
}

/// 删除一个来源的全部索引记录（片段 FTS → 片段 → 源文件 → 来源），
/// 绝不触碰源文件本身。
pub fn delete_source_tree(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM fragment_fts WHERE fragment_id IN (
            SELECT f.id FROM fragment f JOIN source_file sf ON f.source_file_id = sf.id
            WHERE sf.source_id = ?1
        )",
        rusqlite::params![id],
    )?;
    conn.execute(
        "DELETE FROM fragment WHERE source_file_id IN (
            SELECT id FROM source_file WHERE source_id = ?1
        )",
        rusqlite::params![id],
    )?;
    conn.execute(
        "DELETE FROM source_file WHERE source_id = ?1",
        rusqlite::params![id],
    )?;
    conn.execute(
        "DELETE FROM knowledge_source WHERE id = ?1",
        rusqlite::params![id],
    )?;
    Ok(())
}

pub fn file_count(conn: &Connection, source_id: &str) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM source_file WHERE source_id = ?1",
        rusqlite::params![source_id],
        |row| row.get(0),
    )
}

pub fn files_of_source(conn: &Connection, source_id: &str) -> rusqlite::Result<Vec<StoredFile>> {
    let mut stmt = conn.prepare(
        "SELECT rel_path, content_hash, size, mtime_ms
         FROM source_file WHERE source_id = ?1",
    )?;
    let rows = stmt.query_map(rusqlite::params![source_id], |row| {
        Ok(StoredFile {
            rel_path: row.get("rel_path")?,
            content_hash: row.get("content_hash")?,
            size: row.get("size")?,
            mtime_ms: row.get("mtime_ms")?,
        })
    })?;
    rows.collect()
}

/// 新增或更新一个源文件记录（不替换片段），返回其行 id。
pub fn upsert_file(
    conn: &Connection,
    source_id: &str,
    rel_path: &str,
    content_hash: &str,
    size: i64,
    mtime_ms: i64,
    file_identity: &str,
) -> rusqlite::Result<i64> {
    conn.query_row(
        "INSERT INTO source_file
             (source_id, rel_path, file_identity, content_hash, size, mtime_ms, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'active')
         ON CONFLICT (source_id, rel_path) DO UPDATE SET
             file_identity = excluded.file_identity,
             content_hash = excluded.content_hash,
             size = excluded.size,
             mtime_ms = excluded.mtime_ms,
             status = 'active'
         RETURNING id",
        rusqlite::params![
            source_id,
            rel_path,
            file_identity,
            content_hash,
            size,
            mtime_ms
        ],
        |row| row.get(0),
    )
}

/// 在当前事务内整体替换一个文件的片段与 FTS 行：
/// 先删旧 FTS/片段，再插入新片段并同步 FTS，返回写入的片段数。
pub fn replace_fragments(
    conn: &Connection,
    file_id: i64,
    fragments: &[FragmentUpsert<'_>],
) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM fragment_fts WHERE fragment_id IN (
            SELECT id FROM fragment WHERE source_file_id = ?1
        )",
        rusqlite::params![file_id],
    )?;
    conn.execute(
        "DELETE FROM fragment WHERE source_file_id = ?1",
        rusqlite::params![file_id],
    )?;
    for fragment in fragments {
        conn.execute(
            "INSERT INTO fragment
                 (source_file_id, ordinal, heading_path, start_line, end_line, text, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                file_id,
                i64::try_from(fragment.ordinal).expect("ordinal fits in i64"),
                fragment.chunk.heading_path,
                i64::try_from(fragment.chunk.start_line).expect("line fits in i64"),
                i64::try_from(fragment.chunk.end_line).expect("line fits in i64"),
                fragment.chunk.text,
                fragment.content_hash,
            ],
        )?;
        let fragment_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO fragment_fts (fragment_id, text) VALUES (?1, ?2)",
            rusqlite::params![fragment_id, fragment.chunk.text],
        )?;
    }
    Ok(fragments.len())
}
