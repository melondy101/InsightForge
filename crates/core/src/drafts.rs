//! 草稿管理：正文永远在磁盘，数据库只存元数据与保存基线。
//!
//! 保存路径：
//! - 同目录写入临时文件 → flush → fsync → 平台原子替换（Windows ReplaceFileW，
//!   Unix rename）→ 成功后更新基线。
//! - 外部编辑器改过文件时，保存前比对基线，返回结构化 Conflict，前端三选一。

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

/// 临时文件命名用的进程内计数器（与 pid 组合保证唯一，避免并发保存互相覆盖）。
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

use chrono::Utc;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::repository::Repository;

#[derive(Debug, Error)]
pub enum DraftsError {
    #[error("repository error: {0}")]
    Repository(#[from] crate::repository::RepositoryError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("draft not found: {0}")]
    NotFound(String),

    #[error("invalid draft name: {0}")]
    InvalidName(String),

    #[error("conflict: {0}")]
    Conflict(String),
}

impl DraftsError {
    pub fn code(&self) -> &'static str {
        match self {
            DraftsError::Repository(_) => "repository_error",
            DraftsError::Io(_) => "io_error",
            DraftsError::NotFound(_) => "not_found",
            DraftsError::InvalidName(_) => "invalid_name",
            DraftsError::Conflict(_) => "conflict",
        }
    }
}

/// 草稿元数据（正文永远在磁盘）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftFile {
    pub id: String,
    pub path: String,
    pub filename: String,
    pub baseline_hash: String,
    pub baseline_mtime_ms: i64,
    pub baseline_size: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// 保存冲突：外部修改 vs 本地修改。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub message: String,
    pub baseline_content: String,
    pub local_content: String,
    pub external_content: String,
}

/// 冲突三选一的用户决策（绝不静默覆盖）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictResolution {
    /// 保留本地编辑：用内存中的本地内容覆盖外部改动。
    KeepLocal,
    /// 采用外部版本：丢弃本地编辑，以外部文件为准并更新基线。
    AdoptExternal,
    /// 另存副本：本地内容写入新文件，原外部文件不动。
    SaveAsCopy,
}

impl std::str::FromStr for ConflictResolution {
    type Err = DraftsError;

    /// 由前端传来的稳定字符串解析；非法值显式报错，不静默回退。
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "keep_local" => Ok(ConflictResolution::KeepLocal),
            "adopt_external" => Ok(ConflictResolution::AdoptExternal),
            "save_as_copy" => Ok(ConflictResolution::SaveAsCopy),
            other => Err(DraftsError::InvalidName(format!(
                "unknown conflict resolution: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
struct FileBaseline {
    mtime_ms: i64,
    size: i64,
    hash: String,
}

/// 草稿服务：只关心「授权目录下的 .md 文件」。
pub struct DraftsService {
    repo: Arc<Repository>,
    drafts_dir: PathBuf,
}

impl DraftsService {
    pub fn new(repo: Arc<Repository>, drafts_dir: impl Into<PathBuf>) -> Self {
        Self {
            repo,
            drafts_dir: drafts_dir.into(),
        }
    }

    /// 确保草稿目录存在且可写。
    pub fn ensure_directory(&self) -> Result<(), DraftsError> {
        std::fs::create_dir_all(&self.drafts_dir)?;
        Ok(())
    }

    /// 列出草稿目录下的 .md 文件。
    ///
    /// 磁盘上存在、但数据库无登记的 .md（例如用户用外部编辑器直接落地的文件）
    /// 会被自动登记，绝不静默丢弃——否则用户会看到「文件明明在磁盘，列表里却没有」。
    pub fn list(&self) -> Result<Vec<DraftFile>, DraftsError> {
        let entries = std::fs::read_dir(&self.drafts_dir)?;
        let mut drafts = Vec::new();
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                let draft = match self.load_draft(&path)? {
                    Some(d) => d,
                    None => self.register_orphan(&path)?,
                };
                drafts.push(draft);
            }
        }
        drafts.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(drafts)
    }

    /// 自动登记磁盘上未经 create() 登记过的 .md 文件：读基线、生成 id、入库，
    /// 使其出现在列表中并可被 open()/save() 正常处理。
    fn register_orphan(&self, path: &Path) -> Result<DraftFile, DraftsError> {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| DraftsError::InvalidName("invalid filename".to_string()))?
            .to_string();
        let baseline = self.read_baseline(path)?;
        // 用文件 mtime 作为创建/更新时间，使列表排序反映真实文件新旧而非登记时刻。
        let ts = mtime_to_iso(baseline.mtime_ms);
        let draft = DraftFile {
            id: generate_id(&filename),
            path: path.to_string_lossy().to_string(),
            filename,
            baseline_hash: baseline.hash,
            baseline_mtime_ms: baseline.mtime_ms,
            baseline_size: baseline.size,
            created_at: ts.clone(),
            updated_at: ts,
        };
        self.upsert_draft(&draft)?;
        Ok(draft)
    }

    /// 新建草稿文件（正文空，仅元数据入库）。
    pub fn create(&self, filename: &str) -> Result<DraftFile, DraftsError> {
        self.ensure_directory()?;
        if !filename.ends_with(".md") {
            return Err(DraftsError::InvalidName(
                "filename must end with .md".to_string(),
            ));
        }
        let path = self.drafts_dir.join(filename);
        if path.exists() {
            return Err(DraftsError::InvalidName(format!(
                "file already exists: {filename}"
            )));
        }
        std::fs::write(&path, "")?;
        let baseline = self.read_baseline(&path)?;
        let now = now_iso();
        let draft = DraftFile {
            id: generate_id(filename),
            path: path.to_string_lossy().to_string(),
            filename: filename.to_string(),
            baseline_hash: baseline.hash,
            baseline_mtime_ms: baseline.mtime_ms,
            baseline_size: baseline.size,
            created_at: now.clone(),
            updated_at: now,
        };
        self.upsert_draft(&draft)?;
        Ok(draft)
    }

    /// 打开草稿：记录当前保存基线。
    /// 若草稿已存在（同路径），复用已有 ID 与 created_at，仅更新 baseline 与 updated_at。
    pub fn open(&self, filename: &str) -> Result<DraftFile, DraftsError> {
        self.ensure_directory()?;
        let path = self.drafts_dir.join(filename);
        if !path.exists() {
            return Err(DraftsError::NotFound(filename.to_string()));
        }
        let baseline = self.read_baseline(&path)?;
        let now = now_iso();
        let existing = self.find_draft(filename)?;
        let (id, created_at) = existing
            .as_ref()
            .filter(|d| d.path == path.to_string_lossy())
            .map(|d| (d.id.clone(), d.created_at.clone()))
            .unwrap_or_else(|| (generate_id(filename), now.clone()));
        let draft = DraftFile {
            id,
            path: path.to_string_lossy().to_string(),
            filename: filename.to_string(),
            baseline_hash: baseline.hash,
            baseline_mtime_ms: baseline.mtime_ms,
            baseline_size: baseline.size,
            created_at,
            updated_at: now,
        };
        self.upsert_draft(&draft)?;
        Ok(draft)
    }

    /// 原子保存：检测外部修改，有冲突则返回 Conflict。
    pub fn save(&self, filename: &str, content: &str) -> Result<DraftFile, DraftsError> {
        self.ensure_directory()?;
        let path = self.drafts_dir.join(filename);
        if !path.exists() {
            return Err(DraftsError::NotFound(filename.to_string()));
        }

        let current_baseline = self.read_baseline(&path)?;

        // 尝试从数据库读取已知基线
        let known = self.find_draft(filename)?;
        let baseline = known.as_ref().filter(|d| d.path == path.to_string_lossy());

        // 如果数据库有基线，比对当前文件是否被外部修改
        if let Some(bl) = baseline {
            if current_baseline.mtime_ms != bl.baseline_mtime_ms
                || current_baseline.size != bl.baseline_size
                || current_baseline.hash != bl.baseline_hash
            {
                let external_content = std::fs::read_to_string(&path)?;
                return Err(DraftsError::Conflict(
                    serde_json::to_string(&Conflict {
                        message: "file modified externally since last open/save".to_string(),
                        baseline_content: bl.baseline_hash.clone(),
                        local_content: content.to_string(),
                        external_content,
                    })
                    .unwrap(),
                ));
            }
        }

        self.atomic_write(&path, content)?;

        let new_baseline = self.read_baseline(&path)?;
        let now = now_iso();
        let draft = DraftFile {
            id: baseline
                .map(|b| b.id.clone())
                .unwrap_or_else(|| generate_id(filename)),
            path: path.to_string_lossy().to_string(),
            filename: filename.to_string(),
            baseline_hash: new_baseline.hash,
            baseline_mtime_ms: new_baseline.mtime_ms,
            baseline_size: new_baseline.size,
            created_at: baseline
                .map(|b| b.created_at.clone())
                .unwrap_or_else(|| now.clone()),
            updated_at: now,
        };
        self.upsert_draft(&draft)?;
        Ok(draft)
    }

    /// 冲突三选一：绝不静默覆盖，按用户决策显式改写或保留。
    ///
    /// - `KeepLocal`：用内存中的本地内容覆盖外部改动，并更新基线。
    /// - `AdoptExternal`：丢弃本地编辑，以当前外部文件为准更新基线（文件不动）。
    /// - `SaveAsCopy`：本地内容写入新文件，原外部文件不动；返回新草稿。
    pub fn resolve_conflict(
        &self,
        filename: &str,
        resolution: ConflictResolution,
        local_content: &str,
    ) -> Result<DraftFile, DraftsError> {
        self.ensure_directory()?;
        let path = self.drafts_dir.join(filename);
        if !path.exists() {
            return Err(DraftsError::NotFound(filename.to_string()));
        }
        match resolution {
            ConflictResolution::KeepLocal => {
                self.atomic_write(&path, local_content)?;
                self.open(filename)
            }
            ConflictResolution::AdoptExternal => {
                // 外部文件即真相，仅以当前文件刷新基线
                self.open(filename)
            }
            ConflictResolution::SaveAsCopy => {
                let copy_name = self.unique_copy_name(filename)?;
                let copy_path = self.drafts_dir.join(&copy_name);
                self.atomic_write(&copy_path, local_content)?;
                self.open(&copy_name)
            }
        }
    }

    /// 原子写入：唯一临时文件 → flush/fsync → 平台替换（Windows ReplaceFileW / Unix rename）。
    /// 不更新数据库基线；调用方负责刷新（`open` 会重读基线）。
    fn atomic_write(&self, path: &Path, content: &str) -> Result<(), DraftsError> {
        let temp_path = self.unique_temp_for(path)?;
        std::fs::write(&temp_path, content)?;
        // flush + fsync，确保断电/崩溃前已落盘
        if let Ok(f) = std::fs::File::options().write(true).open(&temp_path) {
            let _ = f.sync_all();
        }

        #[cfg(windows)]
        unsafe {
            use std::os::windows::prelude::*;
            // encode_wide() 不含 NUL 终结符；W-API（ReplaceFileW）需要以 0 结尾的宽字符串，
            // 否则会读到越界垃圾并返回 ERROR_INVALID_NAME。这是 save() 既有实现中未被 Windows 测试暴露的缺陷。
            let mut path_w: Vec<u16> = path.as_os_str().encode_wide().collect();
            path_w.push(0);
            let mut temp_w: Vec<u16> = temp_path.as_os_str().encode_wide().collect();
            temp_w.push(0);
            // ReplaceFileW 要求被替换文件已存在；新文件（如冲突另存的副本）走 rename。
            let result = if path.exists() {
                windows_sys::Win32::Storage::FileSystem::ReplaceFileW(
                    path_w.as_ptr(),
                    temp_w.as_ptr(),
                    std::ptr::null(),
                    0,
                    std::ptr::null(),
                    std::ptr::null(),
                )
            } else {
                // 回退到 rename：新文件不存在时 ReplaceFileW 会报 ERROR_FILE_NOT_FOUND。
                match std::fs::rename(&temp_path, path) {
                    Ok(()) => 1,
                    Err(e) => {
                        let _ = std::fs::remove_file(&temp_path);
                        return Err(DraftsError::Io(e));
                    }
                }
            };
            if result == 0 {
                let err = std::io::Error::last_os_error();
                let _ = std::fs::remove_file(&temp_path);
                return Err(DraftsError::Io(err));
            }
        }
        #[cfg(not(windows))]
        {
            std::fs::rename(&temp_path, path)?;
        }
        Ok(())
    }

    /// 为 `path` 生成一个进程内唯一的临时文件名（避免并发保存互相覆盖，
    /// 也便于崩溃后清理残留）。
    fn unique_temp_for(&self, path: &Path) -> Result<PathBuf, DraftsError> {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "draft.md".to_string());
        let n = TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let pid = std::process::id();
        Ok(path.with_file_name(format!("{name}.{pid}-{n}.tmp")))
    }

    /// 为冲突另存副本生成不重名的新文件名：`note.conflict.<nanos>.md`。
    fn unique_copy_name(&self, filename: &str) -> Result<String, DraftsError> {
        let path = Path::new(filename);
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "draft".to_string());
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "md".to_string());
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        Ok(format!("{stem}.conflict.{nanos}.{ext}"))
    }

    /// 删除草稿文件。
    pub fn delete(&self, filename: &str) -> Result<(), DraftsError> {
        let path = self.drafts_dir.join(filename);
        if !path.exists() {
            return Err(DraftsError::NotFound(filename.to_string()));
        }
        std::fs::remove_file(&path)?;
        self.remove_draft(filename)?;
        Ok(())
    }

    // --- 内部：SQLite 薄投影 ---

    fn load_draft(&self, path: &Path) -> Result<Option<DraftFile>, DraftsError> {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| DraftsError::InvalidName("invalid filename".to_string()))?;
        self.find_draft(filename)
    }

    pub fn find_draft(&self, filename: &str) -> Result<Option<DraftFile>, DraftsError> {
        let filename = filename.to_string();
        Ok(self.repo.read(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, path, filename, baseline_hash, baseline_mtime_ms, baseline_size,
                        created_at, updated_at
                 FROM draft
                 WHERE filename = ?1",
            )?;
            let mut rows = stmt.query(rusqlite::params![filename])?;
            match rows.next()? {
                Some(row) => Ok(Some(DraftFile {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    filename: row.get(2)?,
                    baseline_hash: row.get(3)?,
                    baseline_mtime_ms: row.get(4)?,
                    baseline_size: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })),
                None => Ok(None),
            }
        })?)
    }

    fn upsert_draft(&self, draft: &DraftFile) -> Result<(), DraftsError> {
        let draft = draft.clone();
        self.repo.write(move |conn| {
            conn.execute(
                "INSERT INTO draft
                     (id, path, filename, baseline_hash, baseline_mtime_ms, baseline_size,
                      created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                     path = excluded.path,
                     baseline_hash = excluded.baseline_hash,
                     baseline_mtime_ms = excluded.baseline_mtime_ms,
                     baseline_size = excluded.baseline_size,
                     updated_at = excluded.updated_at",
                rusqlite::params![
                    draft.id,
                    draft.path,
                    draft.filename,
                    draft.baseline_hash,
                    draft.baseline_mtime_ms,
                    draft.baseline_size,
                    draft.created_at,
                    draft.updated_at,
                ],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    fn remove_draft(&self, filename: &str) -> Result<(), DraftsError> {
        let filename = filename.to_string();
        self.repo.write(move |conn| {
            conn.execute(
                "DELETE FROM draft WHERE filename = ?1",
                rusqlite::params![filename],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    /// 读取文件保存基线（hash + mtime + size）。
    fn read_baseline(&self, path: &Path) -> Result<FileBaseline, DraftsError> {
        let metadata = std::fs::metadata(path)?;
        let size = metadata.len();
        let mtime_ms = metadata
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let content = std::fs::read_to_string(path)?;
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        Ok(FileBaseline {
            mtime_ms,
            size: size as i64,
            hash,
        })
    }
}

fn now_iso() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%S.%6fZ").to_string()
}

/// 将文件 mtime（毫秒）转 ISO-8601，便于把磁盘文件的新旧如实反映到列表排序。
/// 解析失败时回退到当前时间，绝不因此让 list() 失败。
fn mtime_to_iso(mtime_ms: i64) -> String {
    let secs = mtime_ms / 1000;
    let nanos = (mtime_ms % 1000) * 1_000_000;
    match chrono::DateTime::from_timestamp(secs, nanos as u32) {
        Some(dt) => dt.format("%Y-%m-%dT%H:%M:%S.%6fZ").to_string(),
        None => now_iso(),
    }
}

fn generate_id(filename: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    filename.hash(&mut hasher);
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

    fn setup_service() -> (Arc<Repository>, DraftsService, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("insightforge.sqlite");
        let repo = Arc::new(Repository::open(&db_path).unwrap());
        let service = DraftsService::new(Arc::clone(&repo), dir.path().join("drafts"));
        (repo, service, dir)
    }

    /// 验证 create → open → open 路径：同路径草稿可被多次 open() 而不触发 UNIQUE 约束。
    #[test]
    fn open_existing_draft_reuses_id_and_created_at() {
        let (_repo, service, _dir) = setup_service();

        // Step 1: create a new draft
        let d1 = service.create("note.md").expect("create should succeed");
        assert!(!d1.id.is_empty());
        let first_id = d1.id.clone();
        let first_created_at = d1.created_at.clone();

        // Step 2: open the same draft — should reuse id and created_at
        let d2 = service
            .open("note.md")
            .expect("first reopen should succeed");
        assert_eq!(d2.id, first_id, "open() must reuse the existing draft id");
        assert_eq!(
            d2.created_at, first_created_at,
            "created_at must be preserved"
        );
        assert_eq!(d2.path, d1.path);

        // Step 3: open again — still the same id (regression guard)
        let d3 = service
            .open("note.md")
            .expect("second reopen should succeed");
        assert_eq!(
            d3.id, first_id,
            "second open() must still reuse the same id"
        );
        assert_eq!(d3.created_at, first_created_at);
    }

    /// 验证新文件第一次 open() 生成新 draft（无已有记录时走 create 路径）。
    #[test]
    fn open_creates_new_draft_when_no_prior_record() {
        let (_repo, service, dir) = setup_service();

        // Write a file directly into the drafts directory (no prior create() call)
        let draft_path = dir.path().join("drafts").join("fresh.md");
        if let Some(parent) = draft_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&draft_path, "hello").unwrap();

        let d = service
            .open("fresh.md")
            .expect("open on new file should succeed");
        assert!(!d.id.is_empty());
        assert_eq!(d.filename, "fresh.md");
        assert!(d.path.ends_with("fresh.md"));
    }

    /// 模拟外部编辑后，resolve_conflict(KeepLocal) 必须用本地内容覆盖外部改动。
    #[test]
    fn resolve_keep_local_overwrites_external_changes() {
        let (_repo, service, _dir) = setup_service();
        service.create("note.md").unwrap();
        service.save("note.md", "v1-local").unwrap();

        // 外部进程改动了文件
        let path = service.drafts_dir.join("note.md");
        std::fs::write(&path, "EXTERNAL-CHANGES").unwrap();

        let resolved = service
            .resolve_conflict("note.md", ConflictResolution::KeepLocal, "v2-local")
            .expect("keep_local should succeed");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "v2-local");
        // 基线已更新为本地版本
        let baseline = service.find_draft("note.md").unwrap().unwrap();
        assert_eq!(resolved.baseline_hash, baseline.baseline_hash);
    }

    /// resolve_conflict(AdoptExternal) 保留外部文件，仅刷新基线（丢弃本地编辑）。
    #[test]
    fn resolve_adopt_external_keeps_external_file() {
        let (_repo, service, _dir) = setup_service();
        service.create("note.md").unwrap();
        service.save("note.md", "v1-local").unwrap();

        let path = service.drafts_dir.join("note.md");
        std::fs::write(&path, "EXTERNAL-CHANGES").unwrap();

        service
            .resolve_conflict("note.md", ConflictResolution::AdoptExternal, "v2-local")
            .expect("adopt_external should succeed");
        // 文件保持外部内容，未写入本地
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "EXTERNAL-CHANGES");
        let baseline = service.find_draft("note.md").unwrap().unwrap();
        assert_eq!(baseline.baseline_size, "EXTERNAL-CHANGES".len() as i64);
    }

    /// resolve_conflict(SaveAsCopy) 另存本地内容为副本，原外部文件不动。
    #[test]
    fn resolve_save_as_copy_creates_new_draft_keeps_original() {
        let (_repo, service, _dir) = setup_service();
        service.create("note.md").unwrap();
        service.save("note.md", "v1-local").unwrap();

        let path = service.drafts_dir.join("note.md");
        std::fs::write(&path, "EXTERNAL-CHANGES").unwrap();

        let copy = service
            .resolve_conflict("note.md", ConflictResolution::SaveAsCopy, "v2-local")
            .expect("save_as_copy should succeed");
        assert_ne!(copy.filename, "note.md");
        assert!(copy.filename.contains("conflict"));
        // 原文件保持外部内容
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "EXTERNAL-CHANGES");
        // 副本内容为本地编辑
        let copy_path = service.drafts_dir.join(&copy.filename);
        assert_eq!(std::fs::read_to_string(&copy_path).unwrap(), "v2-local");
        // 副本出现在列表中
        let names: Vec<String> = service
            .list()
            .unwrap()
            .into_iter()
            .map(|d| d.filename)
            .collect();
        assert!(names.contains(&copy.filename));
    }

    /// 非法决策字符串必须显式报错，不得静默回退到某个默认。
    #[test]
    fn conflict_resolution_rejects_unknown_choice() {
        assert!("nonsense".parse::<ConflictResolution>().is_err());
        assert!("keep_local".parse::<ConflictResolution>().is_ok());
    }

    /// 草稿目录里未经 create()、直接落到磁盘的 .md 文件，list() 必须自动登记并出现，
    /// 绝不能静默丢弃（这正是此前「磁盘有文件却不在列表里」的根因）。
    #[test]
    fn list_auto_registers_orphaned_disk_draft() {
        let (_repo, service, dir) = setup_service();

        // 外部直接写入一个 .md 文件，完全不经过 create()
        let draft_path = dir.path().join("drafts").join("orphan.md");
        if let Some(parent) = draft_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&draft_path, "orphaned content").unwrap();

        let drafts = service.list().expect("list should succeed");
        let names: Vec<String> = drafts.iter().map(|d| d.filename.clone()).collect();
        assert!(
            names.contains(&"orphan.md".to_string()),
            "orphaned disk draft must appear in list, got: {names:?}"
        );

        // 自动登记后应有 DB 记录，open() 不应再报 NotFound
        let registered = service.find_draft("orphan.md").unwrap();
        assert!(
            registered.is_some(),
            "orphaned draft must be registered in DB"
        );

        // 再次 list 不应重复登记（id 稳定）
        let drafts2 = service.list().unwrap();
        let orphan = drafts2.iter().find(|d| d.filename == "orphan.md").unwrap();
        assert_eq!(
            orphan.id,
            registered.unwrap().id,
            "id must be stable across list calls"
        );
    }
}
