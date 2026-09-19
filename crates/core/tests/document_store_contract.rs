//! 文档落盘契约（候选 ③ / 铁律 1 + 4）：草稿只能写在授权目录里。
//!
//! 铁律 1 说知识库只读，铁律 4 说磁盘 `.md` 是渲染真相——两者都要求「写文件」这件事
//! 只有一个出口。此前 `drafts_dir.join(filename)` 直接吃调用方给的字符串，
//! 一个带 `..` 或绝对路径的文件名就能把文件写到目录外（包括写进只读知识库）。

use std::path::Path;
use std::sync::Arc;

use insightforge_core::{DraftsError, DraftsService, Repository};

const ESCAPES: &[&str] = &[
    "../escape.md",
    "..\\escape.md",
    "sub/dir.md",
    "sub\\dir.md",
    "./inside.md",
    "/tmp/absolute.md",
    "C:/Windows/temp.md",
    "C:\\Windows\\temp.md",
    "../outside.md",
];

fn services() -> (tempfile::TempDir, DraftsService) {
    let dir = tempfile::tempdir().expect("temp dir");
    let repo = Arc::new(
        Repository::open(&dir.path().join("insightforge.sqlite")).expect("open repository"),
    );
    let service = DraftsService::new(repo, dir.path().join("drafts"));
    service.ensure_directory().expect("ensure drafts dir");
    (dir, service)
}

#[test]
fn no_draft_command_accepts_a_name_that_escapes_the_directory() {
    for filename in ESCAPES {
        let (_dir, service) = services();

        let create = service.create(filename);
        let save = service.save(filename, "内容");
        let open = service.open(filename);
        let remove = service.delete(filename);

        for (verb, outcome) in [
            ("create", create.map(|_| ())),
            ("save", save.map(|_| ())),
            ("open", open.map(|_| ())),
            ("delete", remove),
        ] {
            let error = outcome.expect_err("{filename} must be rejected by {verb}");
            assert!(
                matches!(error, DraftsError::InvalidName(_)),
                "{verb}({filename}) returned {error:?}, not InvalidName"
            );
        }
    }
}

#[test]
fn a_rejected_name_touches_nothing_outside_the_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let drafts_dir = dir.path().join("drafts");
    let vault_dir = dir.path().join("vault");
    std::fs::create_dir_all(&drafts_dir).expect("drafts dir");
    std::fs::create_dir_all(&vault_dir).expect("vault dir");
    std::fs::write(vault_dir.join("keep.md"), "# 原文\n").expect("seed vault");

    let repo = Arc::new(
        Repository::open(&dir.path().join("insightforge.sqlite")).expect("open repository"),
    );
    let service = DraftsService::new(repo, drafts_dir.clone());

    // 目标：../../.. 相对 drafts_dir 出去的上一层，正好是 vault 所在目录。
    let evil = Path::new("../vault/keep.md");
    assert!(service.save(evil.to_str().unwrap(), "重写").is_err());
    assert!(service.create("../vault/created.md").is_err());

    assert_eq!(
        std::fs::read_to_string(vault_dir.join("keep.md")).expect("vault file intact"),
        "# 原文\n",
        "被拒绝的草稿名绝不该碰到授权目录之外"
    );
    assert!(
        !vault_dir.join("created.md").exists(),
        "被拒绝的 create 不该留下文件"
    );
    assert!(
        std::fs::read_dir(&drafts_dir)
            .expect("read drafts")
            .next()
            .is_none(),
        "被拒绝的写入也不该在草稿目录里留下东西"
    );
}

#[test]
fn an_ordinary_name_still_saves_and_reads_back() {
    let (_dir, service) = services();

    let draft = service.create("note.md").expect("create draft");
    let saved = service
        .save("note.md", "# 标题\n\n正文\n")
        .expect("save draft");

    assert_eq!(draft.filename, saved.filename);
    assert!(saved.baseline_size > 0);
    assert_eq!(service.open("note.md").expect("open draft").id, draft.id);
}
