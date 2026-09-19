//! 文档落盘的唯一出口（铁律 1 + 铁律 4）。
//!
//! - **铁律 1（知识库只读）**：草稿只能写在授权目录里。文件名来自前端，
//!   所以「一个名字就能跳出目录」必须是结构性不可能，而不是每个调用点各自小心。
//! - **铁律 4（无损 Markdown）**：真实内容永远在 `.md`，写一半不算写过——
//!   唯一写法是唯一临时文件 → flush/fsync → 平台原子替换。
//!
//! 这里只有文件与目录；数据库基线、冲突判定、草稿登记仍归 `drafts`。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;

use thiserror::Error;

/// 本进程临时文件序号（与 pid 一起保证并发保存互不覆盖）。
static TEMP_COUNTER: LazyLock<AtomicU64> = LazyLock::new(AtomicU64::default);

#[derive(Debug, Error)]
pub enum DocumentError {
    #[error("invalid draft name: {0}")]
    InvalidName(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl DocumentError {
    /// 稳定错误码，供前端按码做 i18n 映射。
    pub fn code(&self) -> &'static str {
        match self {
            DocumentError::InvalidName(_) => "invalid_name",
            DocumentError::Io(_) => "io_error",
        }
    }
}

/// 一个授权目录里的文档存储。构造即声明：能写的位置只有 `root`。
#[derive(Debug, Clone)]
pub struct DocumentStore {
    root: PathBuf,
}

impl DocumentStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 建目录（幂等）。
    pub fn ensure_root(&self) -> Result<(), DocumentError> {
        std::fs::create_dir_all(&self.root)?;
        Ok(())
    }

    /// 把调用方给的文件名解析成目录内的绝对路径；任何越界企图都是 `InvalidName`。
    pub fn resolve(&self, filename: &str) -> Result<PathBuf, DocumentError> {
        validate_filename(filename)?;
        let path = self.root.join(filename);
        // 防御性复检：单段名字本不可能逃逸，逃逸说明上面的规则被改坏了。
        if path.parent() != Some(self.root.as_path()) {
            return Err(DocumentError::InvalidName(format!(
                "{filename} escapes the drafts directory"
            )));
        }
        Ok(path)
    }

    pub fn path_of(&self, filename: &str) -> Result<PathBuf, DocumentError> {
        self.resolve(filename)
    }

    pub fn read_to_string(&self, filename: &str) -> Result<String, DocumentError> {
        let path = self.resolve(filename)?;
        Ok(std::fs::read_to_string(path)?)
    }

    pub fn metadata(&self, filename: &str) -> Result<std::fs::Metadata, DocumentError> {
        let path = self.resolve(filename)?;
        Ok(std::fs::metadata(path)?)
    }

    /// 新建空文档；已存在时报 `InvalidName`（同目录内不允许两个同名草稿）。
    pub fn create(&self, filename: &str) -> Result<PathBuf, DocumentError> {
        let path = self.resolve(filename)?;
        if path.exists() {
            return Err(DocumentError::InvalidName(format!(
                "file already exists: {filename}"
            )));
        }
        std::fs::write(&path, "")?;
        Ok(path)
    }

    /// 原子写入：唯一临时文件 → flush/fsync → 平台替换（Windows ReplaceFileW / Unix rename）。
    pub fn write_atomic(&self, filename: &str, content: &str) -> Result<(), DocumentError> {
        let path = self.resolve(filename)?;
        let temp_path = self.unique_temp_for(&path);
        std::fs::write(&temp_path, content)?;
        // flush + fsync，确保断电/崩溃前已落盘
        if let Ok(mut file) = std::fs::File::options().write(true).open(&temp_path) {
            let _ = file.flush();
            let _ = file.sync_all();
        }

        #[cfg(windows)]
        unsafe {
            use std::os::windows::prelude::*;
            // encode_wide() 不含 NUL 终结符；W-API（ReplaceFileW）需要以 0 结尾的宽字符串，
            // 否则会读到越界垃圾并返回 ERROR_INVALID_NAME。
            let mut path_w: Vec<u16> = path.as_os_str().encode_wide().collect();
            path_w.push(0);
            let mut temp_w: Vec<u16> = temp_path.as_os_str().encode_wide().collect();
            temp_w.push(0);
            // ReplaceFileW 要求被替换文件已存在；新文件走 rename。
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
                match std::fs::rename(&temp_path, &path) {
                    Ok(()) => 1,
                    Err(err) => {
                        let _ = std::fs::remove_file(&temp_path);
                        return Err(DocumentError::Io(err));
                    }
                }
            };
            if result == 0 {
                let err = std::io::Error::last_os_error();
                let _ = std::fs::remove_file(&temp_path);
                return Err(DocumentError::Io(err));
            }
        }
        #[cfg(not(windows))]
        {
            std::fs::rename(&temp_path, &path)?;
        }
        Ok(())
    }

    pub fn remove(&self, filename: &str) -> Result<(), DocumentError> {
        let path = self.resolve(filename)?;
        std::fs::remove_file(path)?;
        Ok(())
    }

    /// 目录内的 `.md` 文档名（含孤儿文件：用户用外部编辑器直接落地的）。
    pub fn documents(&self) -> Result<Vec<String>, DocumentError> {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let path = entry?.path();
            if path.is_file() && extension_of(&path).as_deref() == Some("md") {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    names.push(name.to_string());
                }
            }
        }
        Ok(names)
    }

    /// 为冲突另存副本生成不重名的新文件名：`note.conflict.<nanos>.md`。
    pub fn unique_copy_name(&self, filename: &str) -> Result<String, DocumentError> {
        validate_filename(filename)?;
        let stem = Path::new(filename)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "draft".to_string());
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        Ok(format!("{stem}.conflict.{nanos}.md"))
    }

    /// 清理崩溃/中断保存遗留的临时文件。真实内容永远在 `.md`，遗留 `.tmp` 不是真相；
    /// 只删符合本存储命名模式的文件，避免误删用户或其他工具的临时文件。
    pub fn cleanup_stale_temp_files(&self) -> Result<usize, DocumentError> {
        let mut removed = 0;
        for entry in std::fs::read_dir(&self.root)? {
            let path = entry?.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if is_our_temp(name) && std::fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// 为 `path` 生成进程内唯一的临时文件名（并发保存互不覆盖，且便于崩溃后清理）。
    fn unique_temp_for(&self, path: &Path) -> PathBuf {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "draft.md".to_string());
        let n = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        path.with_file_name(format!("{name}.{pid}-{n}.tmp"))
    }
}

fn extension_of(path: &Path) -> Option<String> {
    path.extension().map(|e| e.to_string_lossy().to_string())
}

/// 文件名字符集校验：只允许「目录内的单个 `.md` 文件名」。
fn validate_filename(filename: &str) -> Result<(), DocumentError> {
    let reject = |reason: String| {
        Err(DocumentError::InvalidName(format!(
            "\"{filename}\": {reason}"
        )))
    };

    if filename.is_empty() {
        return reject("must not be empty".to_string());
    }
    if filename.len() > 255 {
        return reject("must be at most 255 bytes".to_string());
    }
    if filename.contains('/') || filename.contains('\\') {
        return reject("must not contain a path separator".to_string());
    }
    if filename.contains(':') {
        return reject("must not contain a drive or alternate data stream".to_string());
    }
    if filename
        .chars()
        .any(|c| matches!(c, '\0' | '\u{1}'..='\u{1f}'))
    {
        return reject("must not contain control characters".to_string());
    }
    let path = Path::new(filename);
    if path.is_absolute() || path.parent().is_some_and(|p| !p.as_os_str().is_empty()) {
        return reject("must not be an absolute path".to_string());
    }
    // 最后兜底：只有自身作为单段文件名的写法才合法（挡掉 "."、".."、保留名等）。
    if path.file_name().and_then(|n| n.to_str()) != Some(filename) {
        return reject("must be a single file name inside the drafts directory".to_string());
    }
    if extension_of(path).as_deref() != Some("md") {
        return reject("must end with .md".to_string());
    }
    Ok(())
}

/// 判断是否本存储遗留的临时文件：`<name>.<pid>-<n>.tmp`（末两段都是数字）。
fn is_our_temp(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".tmp") else {
        return false;
    };
    let Some((_, tail)) = stem.rsplit_once('.') else {
        return false;
    };
    match tail.split_once('-') {
        Some((pid, counter)) => {
            !pid.is_empty()
                && !counter.is_empty()
                && pid.chars().all(|c| c.is_ascii_digit())
                && counter.chars().all(|c| c.is_ascii_digit())
                && stem.len() > tail.len() + 1
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_in(dir: &Path) -> DocumentStore {
        let store = DocumentStore::new(dir.join("drafts"));
        store.ensure_root().expect("ensure root");
        store
    }

    #[test]
    fn a_single_markdown_name_resolves_inside_the_root() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = store_in(dir.path());

        let path = store.resolve("note.md").expect("resolve");
        assert_eq!(path, dir.path().join("drafts").join("note.md"));
    }

    #[test]
    fn every_escaping_or_non_markdown_name_is_rejected() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = store_in(dir.path());

        for bad in [
            "",
            "../x.md",
            "..\\x.md",
            "a/b.md",
            "a\\b.md",
            "./x.md",
            "/etc/x.md",
            "C:/Windows/x.md",
            "C:\\Windows\\x.md",
            "..",
            ".",
            "note.txt",
            "note",
            "note.md.exe",
            "note.md::$DATA",
            "nul:.md",
            "a\u{0}b.md",
        ] {
            let error = store.resolve(bad).expect_err("\"{bad}\" must be rejected");
            assert!(
                matches!(error, DocumentError::InvalidName(_)),
                "\"{bad}\" returned {error:?}"
            );
        }
    }

    #[test]
    fn a_rejected_write_creates_nothing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = store_in(dir.path());

        assert!(store.write_atomic("../escape.md", "内容").is_err());
        assert!(store.write_atomic("C:/escape.md", "内容").is_err());
        assert!(!dir.path().join("escape.md").exists());
        assert!(std::fs::read_dir(dir.path().join("drafts"))
            .expect("read drafts")
            .next()
            .is_none());
    }

    #[test]
    fn an_atomic_write_leaves_no_temp_file_behind() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = store_in(dir.path());
        store.write_atomic("note.md", "初稿").expect("first write");

        store.write_atomic("note.md", "定稿").expect("second write");

        assert_eq!(
            store.read_to_string("note.md").expect("read back"),
            "定稿",
            "the document is the committed version"
        );
        let leftovers: Vec<String> = store
            .documents()
            .expect("documents")
            .into_iter()
            .filter(|name| name != "note.md")
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp files must be renamed away: {leftovers:?}"
        );
    }

    #[test]
    fn cleanup_only_removes_our_own_temp_pattern() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = store_in(dir.path());
        let root = dir.path().join("drafts");
        std::fs::write(root.join("note.md.1234-0.tmp"), "stale").expect("our temp");
        std::fs::write(root.join("keep.md"), "document").expect("document");
        std::fs::write(root.join("someone-else.tmp"), "not ours").expect("foreign temp");
        std::fs::write(root.join("note.md.1234-9.md"), "odd but ours?").expect("not temp ext");

        let removed = store.cleanup_stale_temp_files().expect("cleanup");

        assert_eq!(removed, 1, "only the stale temp of a committed document");
        assert!(root.join("keep.md").exists());
        assert!(root.join("someone-else.tmp").exists());
        assert!(root.join("note.md.1234-9.md").exists());
    }
}
