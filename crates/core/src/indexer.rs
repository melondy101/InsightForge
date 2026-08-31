//! 补扫索引器：验证授权根目录 → 遍历允许扩展名与 ignore globs →
//! 记录稳定文件标识与 hash → 仅解析新增或 hash 改变的文件 →
//! Markdown 优先按标题、其次按段落与长度窗口分块 →
//! 单个 SQLite 事务内替换该文件的片段与 FTS 行。
//!
//! 全部重活都运行在调用方指定的专门 executor 上，循环内检查显式取消令牌。

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::chunking::{chunk_markdown, chunk_plain, ChunkConfig};
use crate::executor::CancelToken;
use crate::repository::Repository;
use crate::scan::{
    canonical_path, file_identity, normalize_root, open_read_only, stays_within_root,
    validate_rel_path, IgnoreMatcher, ALLOWED_EXTENSIONS,
};
use crate::sources::{EventSink, SourceEvent, SourcesError};
use crate::store::{self, FragmentUpsert, StoredFile, StoredSource};

/// 一次补扫的可观察结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackfillOutcome {
    pub total_files: u64,
    pub indexed_files: u64,
    pub cancelled: bool,
}

/// 目录遍历产出的一个待处理文件。
struct Candidate {
    rel_path: String,
    abs_path: PathBuf,
}

/// 运行一次补扫并推送进度事件；错误不 panic，经 `BackfillFailed` 事件上浮。
pub fn run_backfill(
    repo: &Arc<Repository>,
    source: &StoredSource,
    sink: &EventSink,
    cancel: &CancelToken,
) -> BackfillOutcome {
    match run_backfill_inner(repo, source, sink, cancel) {
        Ok(outcome) => outcome,
        Err(error) => {
            sink(SourceEvent::BackfillFailed {
                source_id: source.id.clone(),
                message: error.to_string(),
            });
            BackfillOutcome {
                total_files: 0,
                indexed_files: 0,
                cancelled: false,
            }
        }
    }
}

fn run_backfill_inner(
    repo: &Arc<Repository>,
    source: &StoredSource,
    sink: &EventSink,
    cancel: &CancelToken,
) -> Result<BackfillOutcome, SourcesError> {
    let root = normalize_root(Path::new(&source.root_path))?;
    let matcher = IgnoreMatcher::new(&source.ignore_globs)?;
    let candidates = collect_candidates(&root, &matcher)?;
    let total_files = candidates.len() as u64;

    sink(SourceEvent::BackfillStarted {
        source_id: source.id.clone(),
        total_files,
    });

    let source_id = source.id.clone();
    let existing: HashMap<String, StoredFile> = repo
        .read(move |conn| store::files_of_source(conn, &source_id))?
        .into_iter()
        .map(|row| (row.rel_path.clone(), row))
        .collect();

    let mut indexed_files = 0u64;
    // 记录本轮实际重索引的文件，用于 emit 时获取准确的 fragment_count；
    // 未变文件 fragment_count 固定为 0，避免无谓的数据库查询。
    let mut reindexed: Vec<String> = Vec::new();
    for candidate in &candidates {
        if cancel.is_cancelled() {
            sink(SourceEvent::BackfillCancelled {
                source_id: source.id.clone(),
                indexed_files,
            });
            return Ok(BackfillOutcome {
                total_files,
                indexed_files,
                cancelled: true,
            });
        }
        if index_file(repo, &source.id, candidate, &existing)? {
            reindexed.push(candidate.rel_path.clone());
        }
        indexed_files += 1;
        let fragment_count = if reindexed.contains(&candidate.rel_path) {
            fragment_count_of(repo, &source.id, &candidate.rel_path)?
        } else {
            0
        };
        sink(SourceEvent::FileIndexed {
            source_id: source.id.clone(),
            rel_path: candidate.rel_path.clone(),
            processed: indexed_files,
            total_files,
            fragment_count,
        });
    }

    let synced_id = source.id.clone();
    repo.write(move |conn| store::touch_last_synced(conn, &synced_id))?;
    sink(SourceEvent::BackfillCompleted {
        source_id: source.id.clone(),
        indexed_files: total_files,
        total_files,
    });
    Ok(BackfillOutcome {
        total_files,
        indexed_files,
        cancelled: false,
    })
}

/// 遍历根目录收集候选文件：只取允许扩展名、未被 ignore 命中、
/// 且不逃出根目录（symlink 逃逸拒绝）的普通文件。
fn collect_candidates(
    root: &Path,
    matcher: &IgnoreMatcher,
) -> Result<Vec<Candidate>, SourcesError> {
    let mut candidates = Vec::new();
    let mut walker = WalkDir::new(root).follow_links(false).into_iter();
    while let Some(entry) = walker.next() {
        let entry =
            entry.map_err(|err| SourcesError::Io(std::io::Error::other(err.to_string())))?;
        if entry.depth() == 0 {
            continue; // 根自身不是候选
        }
        let file_type = entry.file_type();
        let path = entry.path().to_path_buf();

        if file_type.is_dir() {
            let rel = rel_string(&path, root)?;
            if matcher.is_ignored(&rel) {
                walker.skip_current_dir(); // 整目录剪枝
            }
            continue;
        }

        let rel = rel_string(&path, root)?;
        if matcher.is_ignored(&rel) {
            continue;
        }
        let extension_ok = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .is_some_and(|ext| ALLOWED_EXTENSIONS.contains(&ext.as_str()));
        if !extension_ok {
            continue;
        }
        if file_type.is_symlink() {
            // 默认不跟随逃出根目录的 symlink：逃逸与断链一律拒绝（跳过）。
            let canonical = canonical_path(&path)?;
            if !stays_within_root(root, &canonical) {
                continue;
            }
        }
        candidates.push(Candidate {
            rel_path: rel,
            abs_path: path,
        });
    }
    Ok(candidates)
}

fn rel_string(path: &Path, root: &Path) -> Result<String, SourcesError> {
    let rel = path
        .strip_prefix(root)
        .map_err(|_| SourcesError::InvalidRelPath(path.display().to_string()))?;
    Ok(validate_rel_path(&rel.to_string_lossy())?)
}

/// 索引单个文件。返回是否发生了重新索引（解析 + 事务替换）。
fn index_file(
    repo: &Arc<Repository>,
    source_id: &str,
    candidate: &Candidate,
    existing: &HashMap<String, StoredFile>,
) -> Result<bool, SourcesError> {
    let Ok(meta) = std::fs::metadata(&candidate.abs_path) else {
        return Ok(false); // 文件在遍历与读取之间消失：跳过，删除收敛归 04 号票
    };

    // 快路径：size + mtime 未变 → 不读不写（仅解析新增或 hash 改变的文件）。
    if let Some(row) = existing.get(&candidate.rel_path) {
        if row.size == size_of(&meta) && row.mtime_ms == mtime_ms_of(&meta) {
            return Ok(false);
        }
    }

    // 只读打开 + 一次读取同时得到内容 hash 与 file identity。
    let mut file = open_read_only(&candidate.abs_path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let identity = file_identity(&file, &meta);
    drop(file);
    let content_hash = sha256_hex(&bytes);

    // hash 未变（仅元数据抖动）→ 不重解析。
    if let Some(row) = existing.get(&candidate.rel_path) {
        if row.content_hash == content_hash {
            return Ok(false);
        }
    }

    let text = String::from_utf8_lossy(&bytes).into_owned();
    let is_markdown = candidate
        .rel_path
        .rsplit('.')
        .next()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"));
    let chunks = if is_markdown {
        chunk_markdown(&text, ChunkConfig::default())
    } else {
        chunk_plain(&text, ChunkConfig::default())
    };

    // 单个 SQLite 事务内替换该文件的片段与 FTS 行（经 01 单写队列）。
    let rel_path = candidate.rel_path.clone();
    let owned_source_id = source_id.to_string();
    let size = size_of(&meta);
    let mtime_ms = mtime_ms_of(&meta);
    repo.write(move |conn| {
        let file_id = store::upsert_file(
            conn,
            &owned_source_id,
            &rel_path,
            &content_hash,
            size,
            mtime_ms,
            &identity,
        )?;
        let hashes: Vec<String> = chunks
            .iter()
            .filter(|chunk| !chunk.text.trim().is_empty())
            .map(|chunk| sha256_hex(chunk.text.as_bytes()))
            .collect();
        let fragments: Vec<FragmentUpsert<'_>> = chunks
            .iter()
            .filter(|chunk| !chunk.text.trim().is_empty())
            .zip(hashes.iter())
            .enumerate()
            .map(|(ordinal, (chunk, hash))| FragmentUpsert {
                ordinal,
                chunk,
                content_hash: hash,
            })
            .collect();
        store::replace_fragments(conn, file_id, &fragments)?;
        Ok(())
    })?;
    Ok(true)
}

fn fragment_count_of(
    repo: &Arc<Repository>,
    source_id: &str,
    rel_path: &str,
) -> Result<u64, SourcesError> {
    let owned_source_id = source_id.to_string();
    let owned_rel = rel_path.to_string();
    let count = repo.read(move |conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM fragment f
             JOIN source_file sf ON f.source_file_id = sf.id
             WHERE sf.source_id = ?1 AND sf.rel_path = ?2",
            rusqlite::params![owned_source_id, owned_rel],
            |row| row.get::<_, i64>(0),
        )
    })?;
    Ok(u64::try_from(count).unwrap_or(0))
}

fn size_of(meta: &std::fs::Metadata) -> i64 {
    i64::try_from(meta.len()).unwrap_or(i64::MAX)
}

fn mtime_ms_of(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
