//! 扫描边界纯逻辑（02 号票 · CONTEXT.md 铁律 1「知识库只读」）：
//! 根目录规范化、相对路径校验（防 path traversal）、ignore glob 匹配、
//! Windows file identity、以及知识库文件的唯一打开方式——只读打开。

use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobSet};
use thiserror::Error;

/// 首版默认忽略规则（spec：默认忽略规则，可配置）。
pub const DEFAULT_IGNORE_GLOBS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    ".obsidian",
    ".DS_Store",
    "Thumbs.db",
    "desktop.ini",
    "~$*",
    "*.tmp",
    "*.swp",
    "*.bak",
];

/// 允许索引的扩展名（spec：首版输入格式 `.md` / `.txt`）。
pub const ALLOWED_EXTENSIONS: &[&str] = &["md", "txt"];

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("invalid knowledge source root: {0}")]
    InvalidRoot(String),
    #[error("knowledge source root is not a directory: {0}")]
    RootNotADirectory(String),
    #[error("invalid relative path (rejected): {0}")]
    InvalidRelPath(String),
    #[error("invalid ignore glob (rejected): {0}")]
    InvalidIgnoreGlob(String),
}

/// 规范化并验证授权根目录：绝对路径、存在且是目录。
/// Windows 下去除 canonicalize 产生的 verbatim（`\\?\` / `\\?\UNC\`）前缀，
/// 便于展示与一致比较。
pub fn normalize_root(root: &Path) -> Result<PathBuf, ScanError> {
    let canonical = canonical_path(root)
        .map_err(|err| ScanError::InvalidRoot(format!("{}: {err}", root.display())))?;
    if !canonical.is_dir() {
        return Err(ScanError::RootNotADirectory(root.display().to_string()));
    }
    Ok(canonical)
}

/// canonical 化并去除 Windows verbatim 前缀（与 normalize_root 口径一致，
/// 供 symlink 逃逸比较使用）。
pub fn canonical_path(path: &Path) -> std::io::Result<PathBuf> {
    Ok(strip_verbatim(&path.canonicalize()?))
}

/// 去除 Windows verbatim 路径前缀（非 Windows 或非 verbatim 路径原样返回）。
fn strip_verbatim(path: &Path) -> PathBuf {
    let text = path.as_os_str().to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        PathBuf::from(text.into_owned())
    }
}

/// 校验相对路径（来自目录遍历或数据库记录）：拒绝空、绝对路径、盘符、
/// `.`/`..` 段与空段；统一以 `/` 分隔返回。任何 traversal 用例一律拒绝。
pub fn validate_rel_path(rel: &str) -> Result<String, ScanError> {
    let trimmed = rel.trim();
    if trimmed.is_empty() {
        return Err(ScanError::InvalidRelPath(rel.to_string()));
    }
    if trimmed.starts_with('/') || trimmed.starts_with('\\') {
        return Err(ScanError::InvalidRelPath(rel.to_string()));
    }
    if trimmed.contains(':') {
        return Err(ScanError::InvalidRelPath(rel.to_string()));
    }
    let segments: Vec<&str> = trimmed.split(['/', '\\']).collect();
    if segments
        .iter()
        .any(|seg| seg.is_empty() || *seg == "." || *seg == "..")
    {
        return Err(ScanError::InvalidRelPath(rel.to_string()));
    }
    Ok(segments.join("/"))
}

/// 判断 canonical 化后的候选路径是否仍在 canonical 根内（symlink 逃逸边界）。
pub fn stays_within_root(root: &Path, candidate: &Path) -> bool {
    candidate == root || candidate.starts_with(root)
}

/// ignore 匹配器：不含 `/` 的模式按路径任一组件匹配（目录/文件名皆可），
/// 含 `/` 的模式按相对路径的祖先前缀或整条路径匹配（`*` 不跨目录分隔符，
/// `**` 可跨）。
#[derive(Debug, Clone)]
pub struct IgnoreMatcher {
    anchored: GlobSet,
    bare: GlobSet,
}

impl IgnoreMatcher {
    pub fn new(patterns: &[String]) -> Result<Self, ScanError> {
        let mut anchored = Vec::new();
        let mut bare = Vec::new();
        for pattern in patterns {
            let pattern = pattern.trim().trim_end_matches('/');
            if pattern.is_empty() {
                return Err(ScanError::InvalidIgnoreGlob(pattern.to_string()));
            }
            let glob = GlobBuilder::new(pattern)
                .literal_separator(true)
                .build()
                .map_err(|_| ScanError::InvalidIgnoreGlob(pattern.to_string()))?;
            if pattern.contains('/') {
                anchored.push(glob);
            } else {
                bare.push(glob);
            }
        }
        Ok(Self {
            anchored: GlobSet::new(anchored)
                .map_err(|err| ScanError::InvalidIgnoreGlob(format!("globs: {err}")))?,
            bare: GlobSet::new(bare)
                .map_err(|err| ScanError::InvalidIgnoreGlob(format!("globs: {err}")))?,
        })
    }

    /// 相对路径（以 `/` 分隔）是否被忽略；对目录传入其相对路径即可剪枝。
    pub fn is_ignored(&self, rel: &str) -> bool {
        let mut prefix = String::new();
        for segment in rel.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(segment);
            if self.anchored.is_match(&prefix) {
                return true;
            }
            if self.bare.is_match(segment) {
                return true;
            }
        }
        false
    }
}

/// 稳定文件标识（spec：Windows file identity = volume serial + file index）。
///
/// 通过已打开的只读句柄查询；拿不到（非 Windows / 查询失败）时回退
/// `size + mtime(ns)` 组合（记录在案：回退标识在 mtime 精度内稳定）。
pub fn file_identity(handle: &std::fs::File, meta: &std::fs::Metadata) -> String {
    #[cfg(windows)]
    if let Some((volume, index)) = windows_file_identity(handle) {
        return format!("win:{volume:08x}:{index:016x}");
    }
    let mtime_ns = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("fallback:{}:{mtime_ns}", meta.len())
}

/// 经 Win32 `GetFileInformationByHandle` 读取 (volume serial, file index)。
#[cfg(windows)]
fn windows_file_identity(file: &std::fs::File) -> Option<(u32, u64)> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    let handle = file.as_raw_handle();
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    let ok = unsafe { GetFileInformationByHandle(handle, &mut info) };
    if ok == 0 {
        return None;
    }
    let index = ((info.nFileIndexHigh as u64) << 32) | (info.nFileIndexLow as u64);
    Some((info.dwVolumeSerialNumber, index))
}

/// 以只读方式打开知识库文件——应用对知识库文件的唯一打开方式。
/// 返回的句柄没有写权限：任何经此路径的写入 API 都会被操作系统拒绝。
pub fn open_read_only(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new().read(true).open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rel_path_rejects_traversal_and_absolute_cases() {
        for bad in [
            "",
            "  ",
            "..",
            "../escape.md",
            "..\\escape.md",
            "a/../b.md",
            "a/./b.md",
            "a//b.md",
            "/absolute.md",
            "\\absolute.md",
            "C:/system32/x.md",
            "C:\\system32\\x.md",
            "notes/..",
        ] {
            assert!(
                validate_rel_path(bad).is_err(),
                "traversal/absolute 用例必须拒绝: {bad:?}"
            );
        }
    }

    #[test]
    fn validate_rel_path_accepts_and_normalizes_regular_paths() {
        assert_eq!(validate_rel_path("a/b.md").unwrap(), "a/b.md");
        assert_eq!(validate_rel_path("中文/文件.md").unwrap(), "中文/文件.md");
        assert_eq!(validate_rel_path(r"a\b.md").unwrap(), "a/b.md");
    }

    #[test]
    fn normalize_root_accepts_temp_dir_and_rejects_missing_or_file() {
        let dir = tempfile::tempdir().unwrap();
        let normalized = normalize_root(dir.path()).unwrap();
        assert!(normalized.is_absolute(), "规范化根必须是绝对路径");

        let missing = dir.path().join("does-not-exist");
        assert!(matches!(
            normalize_root(&missing),
            Err(ScanError::InvalidRoot(_))
        ));

        let file = dir.path().join("plain.md");
        std::fs::write(&file, "x").unwrap();
        assert!(matches!(
            normalize_root(&file),
            Err(ScanError::RootNotADirectory(_))
        ));
    }

    #[test]
    fn stays_within_root_rejects_paths_outside_the_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let inside = root.join("a.md");
        assert!(stays_within_root(&root, &inside));
        assert!(stays_within_root(&root, &root));

        let other = tempfile::tempdir().unwrap();
        let outside = other.path().canonicalize().unwrap();
        assert!(!stays_within_root(&root, &outside));
        assert!(!stays_within_root(&root, &root.parent().unwrap().join("x")));
    }

    fn matcher(patterns: &[&str]) -> IgnoreMatcher {
        let owned: Vec<String> = patterns.iter().map(|p| p.to_string()).collect();
        IgnoreMatcher::new(&owned).unwrap()
    }

    #[test]
    fn bare_globs_match_any_path_component() {
        let m = matcher(&["node_modules", "*.tmp"]);
        assert!(m.is_ignored("a/node_modules/b.md"));
        assert!(m.is_ignored("node_modules"));
        assert!(m.is_ignored("deep/nested/cache.tmp"));
        assert!(!m.is_ignored("a/nodem.md"));
        assert!(!m.is_ignored("notes/tmp.md.tmpx"));
    }

    #[test]
    fn anchored_globs_only_match_from_root_prefix() {
        let m = matcher(&["notes/private/**", "keep/dist"]);
        assert!(m.is_ignored("notes/private/x.md"));
        assert!(m.is_ignored("notes/private/sub/y.md"));
        assert!(
            !m.is_ignored("x/notes/private/y.md"),
            "锚定模式不从任意层匹配"
        );
        assert!(!m.is_ignored("dist/a.md"), "未命中锚定前缀不忽略");
        assert!(m.is_ignored("keep/dist/a.md"));
    }

    #[test]
    fn trailing_slash_and_empty_patterns_are_handled() {
        assert!(matcher(&["build/"]).is_ignored("sub/build/x.md"));
        assert!(matcher(&["build/"]).is_ignored("build"));
        let empty: Vec<String> = vec![String::new()];
        assert!(matches!(
            IgnoreMatcher::new(&empty),
            Err(ScanError::InvalidIgnoreGlob(_))
        ));
    }

    #[test]
    fn file_identity_is_stable_per_file_and_distinct_across_files() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.md");
        let b = dir.path().join("b.md");
        std::fs::write(&a, "alpha").unwrap();
        std::fs::write(&b, "beta").unwrap();

        let handle_a = open_read_only(&a).unwrap();
        let id_a1 = file_identity(&handle_a, &handle_a.metadata().unwrap());
        let id_a2 = file_identity(&handle_a, &handle_a.metadata().unwrap());
        let handle_b = open_read_only(&b).unwrap();
        let id_b = file_identity(&handle_b, &handle_b.metadata().unwrap());
        drop(handle_a);
        drop(handle_b);

        assert_eq!(id_a1, id_a2, "同一文件两次元数据的标识必须一致");
        assert_ne!(id_a1, id_b, "不同文件的标识必须不同");

        #[cfg(windows)]
        assert!(
            id_a1.starts_with("win:"),
            "Windows 上应记录 volume serial + file index，got {id_a1}"
        );
    }

    #[test]
    fn read_only_handle_rejects_writes_but_allows_reads() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.md");
        std::fs::write(&file, "原始内容").unwrap();

        let mut handle = open_read_only(&file).expect("read-only open must succeed");
        let mut content = String::new();
        use std::io::Read;
        handle.read_to_string(&mut content).expect("read must work");
        assert_eq!(content, "原始内容");

        let write_result = std::io::Write::write(&mut handle, b"tamper");
        assert!(
            write_result.is_err(),
            "知识库文件的写入路径必须在 Rust 侧被拒绝（句柄无写权限）"
        );
    }
}
