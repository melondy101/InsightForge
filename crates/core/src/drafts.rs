//! 草稿管理：正文永远在磁盘，数据库只存元数据与保存基线。
//!
//! 保存路径：
//! - 同目录写入临时文件 → flush → fsync → 平台原子替换（Windows ReplaceFileW，
//!   Unix rename）→ 成功后更新基线。
//! - 外部编辑器改过文件时，保存前比对基线，返回结构化 Conflict，前端三选一。

use std::path::{Path, PathBuf};
use std::sync::Arc;

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
    pub fn list(&self) -> Result<Vec<DraftFile>, DraftsError> {
        let entries = std::fs::read_dir(&self.drafts_dir)?;
        let mut drafts = Vec::new();
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                if let Some(draft) = self.load_draft(&path)? {
                    drafts.push(draft);
                }
            }
        }
        drafts.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(drafts)
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
    pub fn open(&self, filename: &str) -> Result<DraftFile, DraftsError> {
        self.ensure_directory()?;
        let path = self.drafts_dir.join(filename);
        if !path.exists() {
            return Err(DraftsError::NotFound(filename.to_string()));
        }
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

        // 原子替换
        let temp_path = path.with_extension("md.tmp");
        std::fs::write(&temp_path, content)?;
        // flush + fsync
        if let Ok(f) = std::fs::File::options().write(true).open(&temp_path) {
            let _ = f.sync_all();
        }

        #[cfg(windows)]
        unsafe {
            use std::os::windows::prelude::*;
            let path_w: Vec<u16> = path.as_os_str().encode_wide().collect();
            let temp_w: Vec<u16> = temp_path.as_os_str().encode_wide().collect();
            let result = windows_sys::Win32::Storage::FileSystem::ReplaceFileW(
                path_w.as_ptr(),
                temp_w.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null(),
            );
            if result == 0 {
                let err = std::io::Error::last_os_error();
                let _ = std::fs::remove_file(&temp_path);
                return Err(DraftsError::Io(err));
            }
        }
        #[cfg(not(windows))]
        {
            std::fs::rename(&temp_path, &path)?;
        }

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

    fn find_draft(&self, filename: &str) -> Result<Option<DraftFile>, DraftsError> {
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
    Utc::now().format("%Y-%m-%dT%H:%M:%fZ").to_string()
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
