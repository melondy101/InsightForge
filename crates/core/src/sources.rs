//! Sources 命令 seam（ARCHITECTURE §4：`sources.add / list / pause / remove / sync`）。
//!
//! 命令函数只做校验、落库与作业投递；扫描/解析/hash/写入全部在专门
//! executor 上执行，并带显式取消令牌（暂停/移除触发取消）。索引进度
//! 经事件 sink 推送（Tauri 壳把 sink 接到事件通道）。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde::Serialize;
use thiserror::Error;
use walkdir::WalkDir;

use crate::executor::{CancelToken, Executor};
use crate::identity::new_id;
use crate::indexer;
use crate::repository::Repository;
use crate::scan::{
    canonical_path, file_identity, normalize_root, stays_within_root, validate_rel_path,
    IgnoreMatcher, ALLOWED_EXTENSIONS, DEFAULT_IGNORE_GLOBS,
};
use crate::store::{self, StoredFileWithIdentity, StoredSource};

/// 进度事件 sink：Tauri 壳接事件通道，测试接收集器。
pub type EventSink = Arc<dyn Fn(SourceEvent) + Send + Sync>;

/// 来源当前状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStatus {
    /// 没有进行中的补扫。
    Idle,
    /// 补扫进行中（进度见事件通道）。
    Indexing,
}

/// 推送给前端的事件（经事件通道）。
#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum SourceEvent {
    BackfillStarted {
        source_id: String,
        total_files: u64,
    },
    FileIndexed {
        source_id: String,
        rel_path: String,
        processed: u64,
        total_files: u64,
        fragment_count: u64,
    },
    BackfillCompleted {
        source_id: String,
        indexed_files: u64,
        total_files: u64,
    },
    BackfillCancelled {
        source_id: String,
        indexed_files: u64,
    },
    BackfillFailed {
        source_id: String,
        message: String,
    },
}

/// 来源列表条目（前端薄投影）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSummary {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub paused: bool,
    pub status: SourceStatus,
    pub file_count: u64,
    pub ignore_globs: Vec<String>,
    pub last_synced_at: Option<String>,
}

/// 来源索引状态（await_index_committed 返回）：前端在允许 search 前必查。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    pub source_id: String,
    pub file_count: u64,
    pub fragment_count: u64,
    /// true 表示该来源已索引的片段与 FTS row 已 commit，
    /// `SearchService::query(source_id, …)` 必能命中已索引片段。
    pub searchable: bool,
}

#[derive(Debug, Error)]
pub enum SourcesError {
    #[error("source name must not be blank")]
    InvalidName,
    #[error("this root is already added as a source: {0}")]
    DuplicateRoot(String),
    #[error("source not found: {0}")]
    SourceNotFound(String),
    #[error("source is paused, resume it before syncing: {0}")]
    SourcePaused(String),
    #[error("a backfill is already running for source: {0}")]
    AlreadyIndexing(String),
    #[error("invalid relative path (rejected): {0}")]
    InvalidRelPath(String),
    #[error(transparent)]
    Scan(#[from] crate::scan::ScanError),
    #[error(transparent)]
    Repository(#[from] crate::repository::RepositoryError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("operation was cancelled")]
    Cancelled,
}

impl SourcesError {
    /// 稳定错误码，供前端按码做 i18n 映射。
    pub fn code(&self) -> &'static str {
        match self {
            SourcesError::InvalidName => "invalid_name",
            SourcesError::DuplicateRoot(_) => "duplicate_root",
            SourcesError::SourceNotFound(_) => "source_not_found",
            SourcesError::SourcePaused(_) => "source_paused",
            SourcesError::AlreadyIndexing(_) => "already_indexing",
            SourcesError::InvalidRelPath(_) => "invalid_rel_path",
            SourcesError::Scan(scan_error) => match scan_error {
                crate::scan::ScanError::InvalidRoot(_) => "invalid_root",
                crate::scan::ScanError::RootNotADirectory(_) => "root_not_a_directory",
                crate::scan::ScanError::InvalidRelPath(_) => "invalid_rel_path",
                crate::scan::ScanError::InvalidIgnoreGlob(_) => "invalid_ignore_glob",
            },
            SourcesError::Repository(_) => "repository_error",
            SourcesError::Io(_) => "io_error",
            SourcesError::Cancelled => "cancelled",
        }
    }
}

/// 正在进行的补扫作业：取消令牌 + 完成通知（可被多观察者阻塞等待）。
struct RunningJob {
    cancel: CancelToken,
    /// 共享的 "done" 信号：predicate + Condvar。
    /// 完成时把 `done` 置为 true 并 notify_all；多个 await_index_committed / remove 路径
    /// 可同时阻塞等待 commit 完成，避免各自 sleep 或 poll。
    done_signal: Arc<JobDone>,
}

/// 共享的"补扫已完成"信号：predicate 由 condvar 守护，0 写入开销。
struct JobDone {
    done: Mutex<bool>,
    cvar: Condvar,
}

impl JobDone {
    fn new() -> Self {
        Self {
            done: Mutex::new(false),
            cvar: Condvar::new(),
        }
    }

    /// 阻塞直到 done=true；带可选超时用于可取消的等待路径。
    fn wait(&self, timeout: Option<Duration>) -> bool {
        // 无超时路径：阻塞直到被 notify。
        if timeout.is_none() {
            let done = self.done.lock().unwrap_or_else(|e| e.into_inner());
            if *done {
                return true;
            }
            let guard = self
                .cvar
                .wait_while(done, |d| !*d)
                .unwrap_or_else(|e| e.into_inner());
            return *guard;
        }
        // 有超时路径：循环 relock + wait_timeout，直到 done 或 deadline。
        let deadline = std::time::Instant::now() + timeout.unwrap();
        loop {
            let mut done = self.done.lock().unwrap_or_else(|e| e.into_inner());
            if *done {
                return true;
            }
            if std::time::Instant::now() >= deadline {
                return false;
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let guard = self
                .cvar
                .wait_timeout(done, remaining)
                .unwrap_or_else(|e| e.into_inner());
            done = guard.0;
            if *done {
                return true;
            }
        }
    }

    /// 标记完成并唤醒全部等待者（idempotent）。
    fn mark_done(&self) {
        let mut done = self.done.lock().unwrap_or_else(|e| e.into_inner());
        if !*done {
            *done = true;
            self.cvar.notify_all();
        }
    }
}

/// 来源管理服务：克隆/共享经 `Arc`，全部方法 `&self`。
pub struct Sources {
    repo: Arc<Repository>,
    sink: EventSink,
    executor: Executor,
    running: Arc<Mutex<HashMap<String, RunningJob>>>,
}

impl Sources {
    /// 构造服务并启动专门 executor（2 个工作线程）。
    pub fn new(repo: Arc<Repository>, sink: EventSink) -> Self {
        Self {
            repo,
            sink,
            executor: Executor::new(2),
            running: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 授权目录并命名（默认忽略规则可被显式覆盖），随后立即触发首次补扫。
    pub fn add(
        &self,
        root: &Path,
        name: &str,
        ignore_globs: Option<Vec<String>>,
    ) -> Result<SourceSummary, SourcesError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(SourcesError::InvalidName);
        }
        let ignore_globs = ignore_globs.unwrap_or_else(|| {
            DEFAULT_IGNORE_GLOBS
                .iter()
                .map(|glob| glob.to_string())
                .collect()
        });
        IgnoreMatcher::new(&ignore_globs)?;

        let root_path = normalize_root(root)?;
        let root_text = root_path.to_string_lossy().into_owned();
        let probe = root_text.clone();
        if let Some(existing) = self
            .repo
            .read(move |conn| store::find_id_by_root(conn, &probe))?
        {
            return Err(SourcesError::DuplicateRoot(existing));
        }

        let source = StoredSource {
            id: new_id(),
            name: name.to_string(),
            root_path: root_text,
            paused: false,
            ignore_globs,
            last_synced_at: None,
        };
        let row = source.clone();
        self.repo
            .write(move |conn| store::insert_source(conn, &row))?;
        self.spawn_backfill(&source)?;
        self.summary_of(&source)
    }

    /// 来源列表（名称/路径/状态/文件数/忽略规则/最近同步时间）。
    pub fn list(&self) -> Result<Vec<SourceSummary>, SourcesError> {
        let rows = self.repo.read(store::list_sources)?;
        let running = self.running.lock().expect("running lock");
        Ok(rows
            .into_iter()
            .map(|(source, file_count)| SourceSummary {
                status: if running.contains_key(&source.id) {
                    SourceStatus::Indexing
                } else {
                    SourceStatus::Idle
                },
                id: source.id,
                name: source.name,
                root_path: source.root_path,
                paused: source.paused,
                file_count: u64::try_from(file_count).unwrap_or(0),
                ignore_globs: source.ignore_globs,
                last_synced_at: source.last_synced_at,
            })
            .collect())
    }

    /// 暂停或恢复来源。暂停会取消该来源进行中的补扫（尽快停止）；
    /// 暂停期间 `sync` 被拒绝。
    pub fn pause(&self, id: &str, paused: bool) -> Result<SourceSummary, SourcesError> {
        let mut source = self
            .repo
            .read({
                let id = id.to_string();
                move |conn| store::get_source(conn, &id)
            })?
            .ok_or_else(|| SourcesError::SourceNotFound(id.to_string()))?;
        if paused {
            if let Some(job) = self.running.lock().expect("running lock").get(id) {
                job.cancel.cancel();
            }
        }
        let paused_id = id.to_string();
        self.repo
            .write(move |conn| store::set_paused(conn, &paused_id, paused))?;
        source.paused = paused;
        self.summary_of(&source)
    }

    /// 手动触发一次补扫（同步）。
    pub fn sync(&self, id: &str) -> Result<(), SourcesError> {
        let source = self
            .repo
            .read({
                let id = id.to_string();
                move |conn| store::get_source(conn, &id)
            })?
            .ok_or_else(|| SourcesError::SourceNotFound(id.to_string()))?;
        if source.paused {
            return Err(SourcesError::SourcePaused(id.to_string()));
        }
        self.spawn_backfill(&source)
    }

    /// 变更收敛：扫描当前目录，比对数据库记录，处理重命名/删除/新增/修改。
    pub fn reconcile(&self, id: &str) -> Result<(), SourcesError> {
        let source = self
            .repo
            .read({
                let id = id.to_string();
                move |conn| store::get_source(conn, &id)
            })?
            .ok_or_else(|| SourcesError::SourceNotFound(id.to_string()))?;
        if source.paused {
            return Err(SourcesError::SourcePaused(id.to_string()));
        }

        let root = normalize_root(Path::new(&source.root_path))?;
        let matcher = IgnoreMatcher::new(&source.ignore_globs)?;
        let current_files = collect_current_files(&root, &matcher)?;

        let source_id = source.id.clone();
        let db_files: Vec<StoredFileWithIdentity> = self
            .repo
            .read(move |conn| store::files_of_source_with_identity(conn, &source_id))?
            .into_iter()
            .filter(|f| f.file_identity != "missing")
            .collect();

        let mut db_by_identity: HashMap<String, StoredFileWithIdentity> = db_files
            .iter()
            .filter(|f| !f.file_identity.is_empty())
            .map(|f| (f.file_identity.clone(), f.clone()))
            .collect();

        let db_by_rel: HashMap<String, StoredFileWithIdentity> = db_files
            .iter()
            .map(|f| (f.rel_path.clone(), f.clone()))
            .collect();

        let mut current_rel_paths = Vec::new();
        let mut renamed_from: HashMap<String, String> = HashMap::new();

        for (rel_path, identity) in current_files {
            current_rel_paths.push(rel_path.clone());
            if let Some(db_file) = db_by_rel.get(&rel_path) {
                if db_file.file_identity == identity {
                    continue;
                }
            } else if let Some((_, db_file)) = db_by_identity.remove_entry(&identity) {
                renamed_from.insert(rel_path.clone(), db_file.rel_path.clone());
                continue;
            }
        }

        let source_id_for_rename = source.id.clone();
        self.repo.write(move |conn| {
            for (new_rel, old_rel) in renamed_from {
                store::rename_file(conn, &source_id_for_rename, &old_rel, &new_rel)?;
            }
            Ok(())
        })?;

        let keep_rel_paths: Vec<String> = current_rel_paths.clone();
        let source_id_for_mark = source.id.clone();
        let paths_for_mark = keep_rel_paths.clone();
        self.repo.write(move |conn| {
            store::mark_files_missing(conn, &source_id_for_mark, &paths_for_mark)
        })?;

        self.spawn_backfill(&source)
    }

    /// 重建索引：清空片段与 source_file，随后全量补扫。
    pub fn rebuild_index(&self, id: &str) -> Result<(), SourcesError> {
        let source = self
            .repo
            .read({
                let id = id.to_string();
                move |conn| store::get_source(conn, &id)
            })?
            .ok_or_else(|| SourcesError::SourceNotFound(id.to_string()))?;
        if source.paused {
            return Err(SourcesError::SourcePaused(id.to_string()));
        }

        let source_id_for_delete = source.id.clone();
        self.repo
            .write(move |conn| store::delete_source_files(conn, &source_id_for_delete))?;

        self.spawn_backfill(&source)
    }

    /// 设置同步间隔（分钟）。返回更新后的来源摘要。
    pub fn set_sync_interval(
        &self,
        id: &str,
        interval_minutes: u32,
    ) -> Result<SourceSummary, SourcesError> {
        let source = self
            .repo
            .read({
                let id = id.to_string();
                move |conn| store::get_source(conn, &id)
            })?
            .ok_or_else(|| SourcesError::SourceNotFound(id.to_string()))?;

        let key = format!("sources.sync_interval.{}", source.id);
        let value = interval_minutes.to_string();
        self.repo.write(move |conn| {
            crate::settings::set_setting(conn, &key, &value)?;
            Ok(())
        })?;

        self.summary_of(&source)
    }

    /// 移除来源：先让进行中的补扫停止，再清理索引记录。
    /// 只删索引记录，绝不动源文件。
    pub fn remove(&self, id: &str) -> Result<(), SourcesError> {
        self.repo
            .read({
                let id = id.to_string();
                move |conn| store::get_source(conn, &id)
            })?
            .ok_or_else(|| SourcesError::SourceNotFound(id.to_string()))?;

        let job = self.running.lock().expect("running lock").remove(id);
        if let Some(job) = job {
            job.cancel.cancel();
            // 等 backfill 真退出再清理索引记录（避免与运行中的写竞态）
            job.done_signal.wait(None);
        }
        let removed_id = id.to_string();
        self.repo
            .write(move |conn| store::delete_source_tree(conn, &removed_id))?;
        Ok(())
    }

    /// 请求停止全部进行中的补扫（应用退出路径）。
    pub fn shutdown(&self) {
        for job in self.running.lock().expect("running lock").values() {
            job.cancel.cancel();
        }
    }

    /// 阻塞等待指定来源的索引提交完成（fragment + FTS 已写入 SQLite），
    /// 然后返回可观察的索引状态。
    ///
    /// 契约：
    /// - 返回时该来源对应的所有已索引文件的 fragment + FTS row **必然** 已在 writer 线程 commit；
    /// - 因此 `searchable == true` 时，`SearchService::query(source_id, …)` 必能命中已索引片段。
    ///
    /// 实现：不使用 sleep / poll 掩盖竞态——若该来源仍在 running map 中，
    /// 则阻塞等待 Condvar 信号；信号由 backfill 完成后在 executor 线程内 notify。
    pub fn await_index_committed(&self, source_id: &str) -> Result<IndexStatus, SourcesError> {
        // 阶段 0：先验证 source 存在（避免对已删除 source 误报 searchable）。
        let exists = self.repo.read({
            let id = source_id.to_string();
            move |conn| {
                let n: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM knowledge_source WHERE id = ?1",
                    rusqlite::params![id],
                    |r| r.get(0),
                )?;
                Ok(n > 0)
            }
        })?;
        if !exists {
            return Err(SourcesError::SourceNotFound(source_id.to_string()));
        }

        // 阶段 1：若 backfill 仍在跑，阻塞等待 JobDone 信号（纯 Condvar，无 sleep）。
        let signal = {
            let running = self.running.lock().expect("running lock");
            running
                .get(source_id)
                .map(|job| Arc::clone(&job.done_signal))
        };
        if let Some(sig) = signal {
            sig.wait(None);
        }

        // 阶段 2：从数据库读最终 commit 状态。
        let sid = source_id.to_string();
        let (file_count, fragment_count) = self.repo.read(move |conn| {
            let fc: i64 = conn.query_row(
                "SELECT COUNT(*) FROM source_file WHERE source_id = ?1",
                rusqlite::params![sid.clone()],
                |r| r.get(0),
            )?;
            let fragc: i64 = conn.query_row(
                "SELECT COUNT(*) FROM fragment f JOIN source_file sf ON f.source_file_id = sf.id WHERE sf.source_id = ?1",
                rusqlite::params![sid],
                |r| r.get(0),
            )?;
            Ok((fc, fragc))
        })?;
        Ok(IndexStatus {
            source_id: source_id.to_string(),
            file_count: u64::try_from(file_count).expect("COUNT(*) must be non-negative"),
            fragment_count: u64::try_from(fragment_count).expect("COUNT(*) must be non-negative"),
            // 严格语义：searchable = fragment_count > 0。
            // 原因：FTS5 倒排索引在 fragment_fts INSERT 时同步建立；
            //       fragment_count == 0 表示没有可命中片段，search 必返空（语义正确，不算违约）。
            searchable: fragment_count > 0,
        })
    }

    fn spawn_backfill(&self, source: &StoredSource) -> Result<(), SourcesError> {
        let cancel = CancelToken::new();
        let done_signal = Arc::new(JobDone::new());
        {
            let mut running = self.running.lock().expect("running lock");
            if running.contains_key(&source.id) {
                return Err(SourcesError::AlreadyIndexing(source.id.clone()));
            }
            running.insert(
                source.id.clone(),
                RunningJob {
                    cancel: cancel.clone(),
                    done_signal: Arc::clone(&done_signal),
                },
            );
        }

        let job_repo = Arc::clone(&self.repo);
        let job_sink = Arc::clone(&self.sink);
        let job_running = Arc::clone(&self.running);
        let job_source = source.clone();
        let job_id = source.id.clone();
        let job_signal = Arc::clone(&done_signal);
        let spawn_result = self.executor.spawn(Box::new(move || {
            indexer::run_backfill(&job_repo, &job_source, &job_sink, &cancel);
            job_running.lock().expect("running lock").remove(&job_id);
            // 在从 running map 移除后，标记 done 并唤醒所有等待者。
            // 这保证 await_index_committed 即使在 cancel 路径下也能可靠返回。
            job_signal.mark_done();
        }));
        if spawn_result.is_err() {
            // executor 已关闭（正在退出）：先回滚，再释放等待者。
            // 保证 await_index_committed 不会在清理完成前看到信号并继续。
            self.running
                .lock()
                .expect("running lock")
                .remove(&source.id);
            done_signal.mark_done();
            return Err(SourcesError::Cancelled);
        }
        Ok(())
    }

    fn summary_of(&self, source: &StoredSource) -> Result<SourceSummary, SourcesError> {
        let counted_id = source.id.clone();
        let file_count = self
            .repo
            .read(move |conn| store::file_count(conn, &counted_id))?;
        let status = if self
            .running
            .lock()
            .expect("running lock")
            .contains_key(&source.id)
        {
            SourceStatus::Indexing
        } else {
            SourceStatus::Idle
        };
        Ok(SourceSummary {
            id: source.id.clone(),
            name: source.name.clone(),
            root_path: source.root_path.clone(),
            paused: source.paused,
            status,
            file_count: u64::try_from(file_count).unwrap_or(0),
            ignore_globs: source.ignore_globs.clone(),
            last_synced_at: source.last_synced_at.clone(),
        })
    }
}

// ---------- 04 号票：变更收敛辅助函数 ----------

/// 收集当前目录下的所有候选文件（与 indexer 相同的过滤逻辑）。
fn collect_current_files(
    root: &Path,
    matcher: &IgnoreMatcher,
) -> Result<HashMap<String, String>, SourcesError> {
    let mut files = HashMap::new();
    let mut walker = WalkDir::new(root).follow_links(false).into_iter();
    while let Some(entry) = walker.next() {
        let entry =
            entry.map_err(|err| SourcesError::Io(std::io::Error::other(err.to_string())))?;
        if entry.depth() == 0 {
            continue;
        }
        let file_type = entry.file_type();
        let path = entry.path().to_path_buf();
        if file_type.is_dir() {
            let rel = rel_string(&path, root)?;
            if matcher.is_ignored(&rel) {
                walker.skip_current_dir();
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
            let canonical = canonical_path(&path)?;
            if !stays_within_root(root, &canonical) {
                continue;
            }
        }
        let identity = file_identity_of(&path)?;
        files.insert(rel, identity);
    }
    Ok(files)
}

fn rel_string(path: &Path, root: &Path) -> Result<String, SourcesError> {
    let rel = path
        .strip_prefix(root)
        .map_err(|_| SourcesError::InvalidRelPath(path.display().to_string()))?;
    Ok(validate_rel_path(&rel.to_string_lossy())?)
}

/// 计算文件的 identity（简化版：Windows 用 file index + volume，其他用 size+mtime）。
fn file_identity_of(path: &Path) -> Result<String, SourcesError> {
    let file = std::fs::File::open(path)?;
    let meta = file.metadata()?;
    Ok(file_identity(&file, &meta))
}

impl Drop for Sources {
    fn drop(&mut self) {
        // 先请求全部取消，让 executor 的 join 在文件边界尽快收尾。
        for job in self.running.lock().expect("running lock").values() {
            job.cancel.cancel();
        }
        // executor 随字段析构关闭队列并 join 工作线程。
        let _ = &self.executor;
    }
}
