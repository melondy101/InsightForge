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
    /// 间隔校验的周期（分钟）。它是设置而非索引事实，所以存在 `app_setting`，
    /// 读回时缺省用 [`DEFAULT_SYNC_INTERVAL_MINUTES`]。
    #[serde(default = "default_sync_interval")]
    pub sync_interval_minutes: u32,
}

fn default_sync_interval() -> u32 {
    crate::sources::DEFAULT_SYNC_INTERVAL_MINUTES
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
        sync_interval_minutes: read_sync_interval(row)?,
    })
}

/// 间隔存在 KV 里，是文本；没写过、或写着不能用的值，都退回领域默认值。
fn read_sync_interval(row: &rusqlite::Row<'_>) -> rusqlite::Result<u32> {
    let stored: Option<String> = row.get("sync_interval_minutes")?;
    Ok(stored
        .and_then(|text| text.trim().parse::<u32>().ok())
        .filter(|minutes| crate::sources::SYNC_INTERVAL_CHOICES.contains(minutes))
        .unwrap_or(crate::sources::DEFAULT_SYNC_INTERVAL_MINUTES))
}

/// 来源行的统一读法：同步间隔与来源本体分处两表，一次 JOIN 读回，
/// 免得「哪些来源到点了」这类判断在调用点上各读各的。
const SOURCE_COLUMNS: &str =
    "k.id, k.name, k.root_path, k.paused, k.ignore_globs, k.last_synced_at,
         kv.value AS sync_interval_minutes";
const SOURCE_FROM: &str = "FROM knowledge_source k
         LEFT JOIN app_setting kv ON kv.key = 'sources.sync_interval.' || k.id";
pub fn get_source(conn: &Connection, id: &str) -> rusqlite::Result<Option<StoredSource>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SOURCE_COLUMNS} {SOURCE_FROM} WHERE k.id = ?1"
    ))?;
    let mut rows = stmt.query(rusqlite::params![id])?;
    match rows.next()? {
        Some(row) => Ok(Some(row_to_source(row)?)),
        None => Ok(None),
    }
}

/// 全部来源 + 各自已索引文件数（用于来源列表展示）。
pub fn list_sources(conn: &Connection) -> rusqlite::Result<Vec<(StoredSource, i64)>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SOURCE_COLUMNS},
                (SELECT COUNT(*) FROM source_file sf WHERE sf.source_id = k.id) AS file_count
         {SOURCE_FROM} ORDER BY k.created_at, k.id"
    ))?;
    let rows = stmt.query_map([], |row| Ok((row_to_source(row)?, row.get("file_count")?)))?;
    rows.collect()
}

pub fn set_paused(conn: &Connection, id: &str, paused: bool) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE knowledge_source SET paused = ?2 WHERE id = ?1",
        rusqlite::params![id, i64::from(paused)],
    )
}

/// 记录「这次同步落库的时刻」。时间戳由调用方用项目统一时钟给出：
/// 用 SQLite 的 `strftime` 会写成 3 位毫秒，与 `now_iso()` 的 6 位微秒混在
/// 一列里，按字符串比先后就会判错到点没有（见 [`crate::clock`]）。
pub fn touch_last_synced(conn: &Connection, id: &str, synced_at: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE knowledge_source SET last_synced_at = ?2 WHERE id = ?1",
        rusqlite::params![id, synced_at],
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

/// 含 file_identity 的文件记录（供 04 号票重命名/删除收敛使用）。
#[derive(Debug, Clone)]
pub struct StoredFileWithIdentity {
    pub rel_path: String,
    pub content_hash: String,
    pub size: i64,
    pub mtime_ms: i64,
    pub file_identity: String,
}

pub fn files_of_source_with_identity(
    conn: &Connection,
    source_id: &str,
) -> rusqlite::Result<Vec<StoredFileWithIdentity>> {
    let mut stmt = conn.prepare(
        "SELECT rel_path, content_hash, size, mtime_ms, file_identity
         FROM source_file WHERE source_id = ?1",
    )?;
    let rows = stmt.query_map(rusqlite::params![source_id], |row| {
        Ok(StoredFileWithIdentity {
            rel_path: row.get("rel_path")?,
            content_hash: row.get("content_hash")?,
            size: row.get("size")?,
            mtime_ms: row.get("mtime_ms")?,
            file_identity: row.get("file_identity")?,
        })
    })?;
    rows.collect()
}

/// 将不在 keep_rel_paths 中的文件标记为 missing（删除收敛）。
pub fn mark_files_missing(
    conn: &Connection,
    source_id: &str,
    keep_rel_paths: &[String],
) -> rusqlite::Result<usize> {
    if keep_rel_paths.is_empty() {
        conn.execute(
            "UPDATE source_file SET status = 'missing' WHERE source_id = ?1",
            rusqlite::params![source_id],
        )
    } else {
        let placeholders = keep_rel_paths
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "UPDATE source_file SET status = 'missing' WHERE source_id = ?1 AND rel_path NOT IN ({})",
            placeholders
        );
        let mut params: Vec<rusqlite::types::Value> = vec![source_id.to_string().into()];
        for path in keep_rel_paths {
            params.push(path.to_string().into());
        }
        conn.execute(&sql, rusqlite::params_from_iter(params))
    }
}

/// 重建索引：删除来源的全部 source_file 记录（随后全量补扫会重建）。
pub fn delete_source_files(conn: &Connection, source_id: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM source_file WHERE source_id = ?1",
        rusqlite::params![source_id],
    )
}

/// 重命名：更新文件的 rel_path（保持 source_id + file_identity 不变）。
pub fn rename_file(
    conn: &Connection,
    source_id: &str,
    old_rel_path: &str,
    new_rel_path: &str,
) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE source_file SET rel_path = ?3 WHERE source_id = ?1 AND rel_path = ?2",
        rusqlite::params![source_id, old_rel_path, new_rel_path],
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 同步间隔的读侧 JOIN 与写侧键名必须拼出同一个键，否则设置会静默失效
    /// ——它已经以「只写不读」的形态存在过一轮了。
    #[test]
    fn the_sync_interval_join_uses_the_writers_key_prefix() {
        assert!(SOURCE_FROM.contains(crate::sources::SYNC_INTERVAL_KEY_PREFIX));
    }
}
