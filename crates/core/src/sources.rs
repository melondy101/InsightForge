//! Sources 命令 seam（ARCHITECTURE §4：`sources.add / list / pause / remove / sync`）。
//!
//! 命令函数只做校验、落库与作业投递；扫描/解析/hash/写入全部在专门
//! executor 上执行，并带显式取消令牌（暂停/移除触发取消）。索引进度
//! 经事件 sink 推送（Tauri 壳把 sink 接到事件通道）。

use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::executor::{CancelToken, Executor};
use crate::indexer;
use crate::repository::Repository;
use crate::scan::{normalize_root, IgnoreMatcher, DEFAULT_IGNORE_GLOBS};
use crate::store::{self, StoredSource};

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

/// 正在进行的补扫作业：取消令牌 + 完成通知。
struct RunningJob {
    cancel: CancelToken,
    done: mpsc::Receiver<()>,
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
            id: Uuid::new_v4().to_string(),
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
        let source = self
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
            let _ = job.done.recv(); // 等待补扫真正停止后再清理
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

    fn spawn_backfill(&self, source: &StoredSource) -> Result<(), SourcesError> {
        let (done_tx, done) = mpsc::channel();
        let cancel = CancelToken::new();
        {
            let mut running = self.running.lock().expect("running lock");
            if running.contains_key(&source.id) {
                return Err(SourcesError::AlreadyIndexing(source.id.clone()));
            }
            running.insert(
                source.id.clone(),
                RunningJob {
                    cancel: cancel.clone(),
                    done,
                },
            );
        }

        let job_repo = Arc::clone(&self.repo);
        let job_sink = Arc::clone(&self.sink);
        let job_running = Arc::clone(&self.running);
        let job_source = source.clone();
        let job_id = source.id.clone();
        let spawn_result = self.executor.spawn(Box::new(move || {
            indexer::run_backfill(&job_repo, &job_source, &job_sink, &cancel);
            job_running.lock().expect("running lock").remove(&job_id);
            let _ = done_tx.send(());
        }));
        if spawn_result.is_err() {
            // executor 已关闭（正在退出）：立刻回滚登记。
            self.running
                .lock()
                .expect("running lock")
                .remove(&source.id);
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
