//! 草稿管理：正文永远在磁盘，数据库只存元数据与保存基线。
//!
//! 落盘方式（唯一临时文件 → fsync → 平台原子替换）与「一个名字能不能逃出目录」都归
//! [`DocumentStore`]（铁律 1 + 铁律 4）；本模块只管登记、基线比对与冲突三选一。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::clock::now_iso;
use crate::document_store::{DocumentError, DocumentStore};
use crate::identity::new_id;
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

impl From<DocumentError> for DraftsError {
    fn from(error: DocumentError) -> Self {
        match error {
            DocumentError::InvalidName(name) => DraftsError::InvalidName(name),
            DocumentError::Io(err) => DraftsError::Io(err),
        }
    }
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
    store: DocumentStore,
}

impl DraftsService {
    pub fn new(repo: Arc<Repository>, drafts_dir: impl Into<PathBuf>) -> Self {
        Self {
            repo,
            store: DocumentStore::new(drafts_dir),
        }
    }

    /// 授权目录本身（列表、诊断与测试用）。
    pub fn dir(&self) -> &Path {
        self.store.root()
    }

    /// 确保草稿目录存在且可写。
    pub fn ensure_directory(&self) -> Result<(), DraftsError> {
        self.store.ensure_root()?;
        Ok(())
    }

    /// 列出草稿目录下的 .md 文件。
    ///
    /// 磁盘上存在、但数据库无登记的 .md（例如用户用外部编辑器直接落地的文件）
    /// 会被自动登记，绝不静默丢弃——否则用户会看到「文件明明在磁盘，列表里却没有」。
    pub fn list(&self) -> Result<Vec<DraftFile>, DraftsError> {
        let mut drafts = Vec::new();
        for name in self.store.documents()? {
            // 目录里可能有名字根本没法寻址的东西（例如别处搬来的 `a:b.md`）：
            // 读不到也写不回，硬登记进列表反而更糟，故跳过而不是让整个列表失败。
            let Ok(path) = self.store.path_of(&name) else {
                continue;
            };
            let draft = match self.load_draft(&path)? {
                Some(d) => d,
                None => self.register_orphan(&path)?,
            };
            drafts.push(draft);
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
            id: new_id(),
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
        let path = self.store.create(filename)?;
        let baseline = self.read_baseline(&path)?;
        let now = now_iso();
        let draft = DraftFile {
            id: new_id(),
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
        let path = self.store.path_of(filename)?;
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
            .unwrap_or_else(|| (new_id(), now.clone()));
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
        let path = self.store.path_of(filename)?;
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
                let external_content = self.store.read_to_string(filename)?;
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

        self.store.write_atomic(filename, content)?;

        let new_baseline = self.read_baseline(&path)?;
        let now = now_iso();
        let draft = DraftFile {
            id: baseline.map(|b| b.id.clone()).unwrap_or_else(new_id),
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
        let path = self.store.path_of(filename)?;
        if !path.exists() {
            return Err(DraftsError::NotFound(filename.to_string()));
        }
        match resolution {
            ConflictResolution::KeepLocal => {
                self.store.write_atomic(filename, local_content)?;
                self.open(filename)
            }
            ConflictResolution::AdoptExternal => {
                // 外部文件即真相，仅以当前文件刷新基线
                self.open(filename)
            }
            ConflictResolution::SaveAsCopy => {
                let copy_name = self.store.unique_copy_name(filename)?;
                self.store.write_atomic(&copy_name, local_content)?;
                self.open(&copy_name)
            }
        }
    }

    /// 删除草稿文件。
    pub fn delete(&self, filename: &str) -> Result<(), DraftsError> {
        let path = self.store.path_of(filename)?;
        if !path.exists() {
            return Err(DraftsError::NotFound(filename.to_string()));
        }
        self.store.remove(filename)?;
        self.remove_draft(filename)?;
        Ok(())
    }

    /// 清理崩溃/中断保存遗留的临时文件（命名形如 `<name>.<pid>-<n>.tmp`）。
    ///
    /// 真实内容永远在 `.md`（原子替换保证 `.md` 是已提交版本），遗留的 `.tmp`
    /// 永远不是真相，可安全删除。启动时调用一次即可；调用时无在途保存，不会误删进行中的临时文件。
    /// 仅清理符合本服务命名模式的 `.tmp`，避免误删用户或其他工具的临时文件。
    pub fn cleanup_stale_temp_files(&self) -> Result<usize, DraftsError> {
        Ok(self.store.cleanup_stale_temp_files()?)
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
        let path = service.dir().join("note.md");
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

        let path = service.dir().join("note.md");
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

        let path = service.dir().join("note.md");
        std::fs::write(&path, "EXTERNAL-CHANGES").unwrap();

        let copy = service
            .resolve_conflict("note.md", ConflictResolution::SaveAsCopy, "v2-local")
            .expect("save_as_copy should succeed");
        assert_ne!(copy.filename, "note.md");
        assert!(copy.filename.contains("conflict"));
        // 原文件保持外部内容
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "EXTERNAL-CHANGES");
        // 副本内容为本地编辑
        let copy_path = service.dir().join(&copy.filename);
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

    /// 崩溃/中断保存会遗留 .tmp 临时文件；cleanup_stale_temp_files 必须清掉它们，
    /// 且不能误删用户 .md 或合法的 .conflict. 副本（真实内容永远在 .md）。
    #[test]
    fn cleanup_removes_orphaned_temp_files_only() {
        let (_repo, service, dir) = setup_service();
        let drafts_dir = dir.path().join("drafts");
        std::fs::create_dir_all(&drafts_dir).unwrap();

        // 模拟崩溃遗留的临时文件（符合命名模式 <name>.<pid>-<n>.tmp）
        let temp1 = drafts_dir.join("note.md.1234-0.tmp");
        std::fs::write(&temp1, "partial").unwrap();
        let temp2 = drafts_dir.join("note.md.5678-1.tmp");
        std::fs::write(&temp2, "partial").unwrap();
        // 真实草稿，必须保留
        let real = drafts_dir.join("note.md");
        std::fs::write(&real, "real content").unwrap();
        // 合法的冲突副本，必须保留
        let other = drafts_dir.join("note.md.conflict.999.md");
        std::fs::write(&other, "copy").unwrap();

        let removed = service
            .cleanup_stale_temp_files()
            .expect("cleanup should succeed");
        assert_eq!(
            removed, 2,
            "exactly the two orphaned temps should be removed"
        );
        assert!(!temp1.exists(), "temp1 removed");
        assert!(!temp2.exists(), "temp2 removed");
        assert!(real.exists(), "real .md kept");
        assert!(other.exists(), "conflict copy kept");
    }
}
