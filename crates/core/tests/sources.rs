//! Sources 公共命令 seam 集成测试（spec §Testing Decisions）：
//! 真实临时目录 + 真实 SQLite 文件，headless 直接调用命令级 API，
//! 只断言公共接口可观察的行为（事件、列表状态、文件系统与数据库最终状态）。

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use insightforge_core::{
    Repository, ScanError, SourceEvent, SourceStatus, Sources, SourcesError, DEFAULT_IGNORE_GLOBS,
};

/// 事件收集器：注入 EventSink，测试断言事件序列。
#[derive(Clone)]
struct EventLog(Arc<Mutex<Vec<SourceEvent>>>);

impl EventLog {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }

    fn sink(&self) -> insightforge_core::EventSink {
        let log = Arc::clone(&self.0);
        Arc::new(move |event| log.lock().unwrap().push(event))
    }

    fn snapshot(&self) -> Vec<SourceEvent> {
        self.0.lock().unwrap().clone()
    }

    fn clear(&self) {
        self.0.lock().unwrap().clear();
    }
}

struct TestHarness {
    db_dir: tempfile::TempDir,
    sources: Sources,
    events: EventLog,
}

fn harness() -> TestHarness {
    let db_dir = tempfile::tempdir().expect("temp dir");
    let repo = Arc::new(
        Repository::open(&db_dir.path().join("insightforge.sqlite")).expect("open repository"),
    );
    let events = EventLog::new();
    let sources = Sources::new(Arc::clone(&repo), events.sink());
    TestHarness {
        db_dir,
        sources,
        events,
    }
}

fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create dirs");
    std::fs::write(path, content).expect("write file");
}

/// 轮询直至该来源状态回到 idle（补扫结束），返回最终摘要。
fn wait_until_idle(sources: &Sources, id: &str) -> insightforge_core::SourceSummary {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let summary = sources
            .list()
            .expect("list should succeed")
            .into_iter()
            .find(|s| s.id == id)
            .expect("source should exist");
        if summary.status == SourceStatus::Idle {
            return summary;
        }
        assert!(
            Instant::now() < deadline,
            "backfill for {id} did not finish within 20s"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn event_source_id(event: &SourceEvent) -> &str {
    match event {
        SourceEvent::BackfillStarted { source_id, .. }
        | SourceEvent::FileIndexed { source_id, .. }
        | SourceEvent::BackfillCompleted { source_id, .. }
        | SourceEvent::BackfillCancelled { source_id, .. }
        | SourceEvent::BackfillFailed { source_id, .. } => source_id,
    }
}

#[test]
fn add_runs_first_backfill_landing_fragments_and_fts_with_progress_events() {
    let h = harness();
    let root = h.db_dir.path().join("kb");
    write_file(
        &root,
        "笔记.md",
        "# 架构\n\n本地优先的核心思想。\n\n## 边界\n\n知识库只读。\n",
    );
    write_file(
        &root,
        "plain.txt",
        "第一段，一些中文内容。\n\n第二段，更多内容。\n",
    );

    let summary = h
        .sources
        .add(&root, "我的笔记", None)
        .expect("add should succeed");
    assert_eq!(summary.status, SourceStatus::Indexing, "添加后立即进入索引");
    assert!(!summary.paused);
    assert_eq!(summary.name, "我的笔记");

    let idle = wait_until_idle(&h.sources, &summary.id);
    assert_eq!(idle.status, SourceStatus::Idle);
    assert_eq!(idle.file_count, 2, "补扫完成后来源的文件数必须是 2");
    assert_eq!(
        idle.ignore_globs,
        DEFAULT_IGNORE_GLOBS
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        "未显式传 ignore 时应落默认忽略规则"
    );

    // 数据库最终状态：片段与 FTS 行一一对应，标题分块生效。
    let inspector =
        rusqlite::Connection::open(h.db_dir.path().join("insightforge.sqlite")).unwrap();
    let mut stmt = inspector
        .prepare(
            "SELECT sf.rel_path, COUNT(f.id) FROM source_file sf
             LEFT JOIN fragment f ON f.source_file_id = sf.id
             GROUP BY sf.id ORDER BY sf.rel_path",
        )
        .unwrap();
    let per_file: Vec<(String, i64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map_while(Result::ok)
        .collect();
    assert_eq!(
        per_file,
        vec![("plain.txt".to_string(), 1), ("笔记.md".to_string(), 2),],
        "Markdown 按标题分两块，纯文本整文件一块"
    );

    let fts_total: i64 = inspector
        .query_row("SELECT COUNT(*) FROM fragment_fts", [], |row| row.get(0))
        .unwrap();
    assert_eq!(fts_total, 3, "FTS 行必须与片段一一对应");

    let mut stmt = inspector
        .prepare("SELECT DISTINCT heading_path FROM fragment WHERE heading_path != ''")
        .unwrap();
    let heading_paths: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .unwrap()
        .map_while(Result::ok)
        .collect();
    assert!(
        heading_paths.contains(&"架构".to_string())
            && heading_paths.contains(&"架构 > 边界".to_string()),
        "标题路径必须是祖先链，got {heading_paths:?}"
    );

    let trigram_hits: i64 = inspector
        .query_row(
            "SELECT COUNT(*) FROM fragment_fts WHERE fragment_fts MATCH '知识库'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(trigram_hits >= 1, "trigram FTS 必须能命中中文片段");

    // 进度事件：Started → FileIndexed×N → Completed，全部属于该来源。
    let events = h.events.snapshot();
    assert!(
        matches!(
            &events[0],
            SourceEvent::BackfillStarted { total_files: 2, .. }
        ),
        "第一个事件必须是 BackfillStarted，got {:?}",
        events[0]
    );
    assert!(
        matches!(
            events.last().unwrap(),
            SourceEvent::BackfillCompleted { total_files: 2, .. }
        ),
        "最后一个事件必须是 BackfillCompleted"
    );
    let indexed_count = events
        .iter()
        .filter(|e| matches!(e, SourceEvent::FileIndexed { .. }))
        .count();
    assert_eq!(indexed_count, 2, "两个文件各发一次 FileIndexed");
    assert!(
        events.iter().all(|e| event_source_id(e) == summary.id),
        "所有事件都必须携带该来源 id"
    );
}

#[test]
fn add_rejects_missing_root() {
    let h = harness();
    let missing = h.db_dir.path().join("does-not-exist");
    let error = h.sources.add(&missing, "x", None).unwrap_err();
    assert!(
        matches!(error, SourcesError::Scan(ScanError::InvalidRoot(_))),
        "got: {error}"
    );
}

#[test]
fn add_rejects_file_as_root() {
    let h = harness();
    let file = h.db_dir.path().join("plain.md");
    std::fs::write(&file, "not a dir").unwrap();
    let error = h.sources.add(&file, "x", None).unwrap_err();
    assert!(
        matches!(error, SourcesError::Scan(ScanError::RootNotADirectory(_))),
        "got: {error}"
    );
}

#[test]
fn add_rejects_blank_name() {
    let h = harness();
    let root = h.db_dir.path().join("kb");
    std::fs::create_dir_all(&root).unwrap();
    let error = h.sources.add(&root, "   ", None).unwrap_err();
    assert!(matches!(error, SourcesError::InvalidName), "got: {error}");
}

#[test]
fn add_rejects_duplicate_root() {
    let h = harness();
    let root = h.db_dir.path().join("kb");
    write_file(&root, "a.md", "# 一\n内容。");

    let first = h.sources.add(&root, "first", None).expect("first add");
    wait_until_idle(&h.sources, &first.id);

    let error = h.sources.add(&root, "second", None).unwrap_err();
    assert!(
        matches!(error, SourcesError::DuplicateRoot(_)),
        "同一根目录不允许重复授权，got: {error}"
    );
}

// ---------------------------------------------------------------------------
// 切片 5：增量语义——仅解析新增或 hash 改变的文件；重复 sync 无重复片段。
// ---------------------------------------------------------------------------

/// 建一个含两文件的来源并等首次补扫结束。
fn seeded_source() -> (TestHarness, std::path::PathBuf, String) {
    let h = harness();
    let root = h.db_dir.path().join("kb");
    write_file(
        &root,
        "笔记.md",
        "# 架构\n\n本地优先的核心思想。\n\n## 边界\n\n知识库只读。\n",
    );
    write_file(
        &root,
        "plain.txt",
        "第一段，一些中文内容。\n\n第二段，更多内容。\n",
    );
    let summary = h
        .sources
        .add(&root, "笔记", None)
        .expect("add should succeed");
    wait_until_idle(&h.sources, &summary.id);
    (h, root, summary.id)
}

fn fragment_totals(db_path: &Path) -> (i64, i64, i64) {
    let inspector = rusqlite::Connection::open(db_path).unwrap();
    let fragments: i64 = inspector
        .query_row("SELECT COUNT(*) FROM fragment", [], |row| row.get(0))
        .unwrap();
    let fts: i64 = inspector
        .query_row("SELECT COUNT(*) FROM fragment_fts", [], |row| row.get(0))
        .unwrap();
    let files: i64 = inspector
        .query_row("SELECT COUNT(*) FROM source_file", [], |row| row.get(0))
        .unwrap();
    (files, fragments, fts)
}

#[test]
fn sync_skips_unchanged_files_without_reindexing() {
    let (h, _root, id) = seeded_source();
    h.events.clear();

    h.sources.sync(&id).expect("manual sync should succeed");
    wait_until_idle(&h.sources, &id);

    let events = h.events.snapshot();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, SourceEvent::FileIndexed { .. })),
        "内容未变的文件绝不能重解析，got {events:?}"
    );
    assert!(
        matches!(
            events.last(),
            Some(SourceEvent::BackfillCompleted {
                indexed_files: 0,
                total_files: 2,
                ..
            })
        ),
        "补扫应完成且 0 个文件被重建索引，got {events:?}"
    );
    let (files, fragments, fts) = fragment_totals(&h.db_dir.path().join("insightforge.sqlite"));
    assert_eq!((files, fragments, fts), (2, 3, 3));
}

#[test]
fn sync_reindexes_only_the_changed_file() {
    let (h, root, id) = seeded_source();
    let original = std::fs::read_to_string(root.join("笔记.md")).unwrap();
    let updated = format!("{original}\n## 新边界\n\n补扫是最终一致的手段。\n");
    std::fs::write(root.join("笔记.md"), updated).unwrap();
    h.events.clear();

    h.sources.sync(&id).expect("manual sync should succeed");
    wait_until_idle(&h.sources, &id);

    let changed: Vec<String> = h
        .events
        .snapshot()
        .into_iter()
        .filter_map(|e| match e {
            SourceEvent::FileIndexed { rel_path, .. } => Some(rel_path),
            _ => None,
        })
        .collect();
    assert_eq!(
        changed,
        vec!["笔记.md".to_string()],
        "只有内容改变的文件被重建索引"
    );

    let inspector =
        rusqlite::Connection::open(h.db_dir.path().join("insightforge.sqlite")).unwrap();
    let note_fragments: i64 = inspector
        .query_row(
            "SELECT COUNT(*) FROM fragment f JOIN source_file sf ON f.source_file_id = sf.id
             WHERE sf.rel_path = '笔记.md'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        note_fragments, 3,
        "变更文件的单文件事务替换：三个小节 → 三个片段"
    );
    let old_heading_gone: i64 = inspector
        .query_row(
            "SELECT COUNT(*) FROM fragment WHERE heading_path = '架构 > 新边界'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(old_heading_gone, 1, "新标题路径应入库");
    let (files, fragments, fts) = fragment_totals(&h.db_dir.path().join("insightforge.sqlite"));
    assert_eq!((files, fragments, fts), (2, 4, 4), "FTS 与片段保持一一对应");
}

#[test]
fn repeated_sync_never_duplicates_fragments() {
    let (h, _root, id) = seeded_source();

    for round in 0..3 {
        h.events.clear();
        h.sources.sync(&id).expect("sync should succeed");
        wait_until_idle(&h.sources, &id);
        let (files, fragments, fts) = fragment_totals(&h.db_dir.path().join("insightforge.sqlite"));
        assert_eq!(
            (files, fragments, fts),
            (2, 3, 3),
            "第 {round} 轮重复 sync 不得产生重复片段/FTS 行"
        );
    }
}

// ---------------------------------------------------------------------------
// 切片 6：暂停 / 恢复 / 移除——取消令牌、索引清理、源文件原样保留。
// ---------------------------------------------------------------------------

use std::fs;

fn sha256_of_file(path: &Path) -> String {
    let bytes = fs::read(path).expect("read file for hashing");
    // 复用 core 的 hash 不属于公共 API：这里直接用双实现对比不划算，
    // 只需同一实现前后一致即可证明「内容未变」。
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(&bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn pause_rejects_sync_and_resume_restores_it() {
    let (h, _root, id) = seeded_source();

    let paused = h.sources.pause(&id, true).expect("pause should succeed");
    assert!(paused.paused, "列表状态必须反映暂停");

    let error = h.sources.sync(&id).unwrap_err();
    assert!(
        matches!(error, SourcesError::SourcePaused(_)),
        "暂停来源的 sync 必须被拒绝，got: {error}"
    );

    let resumed = h.sources.pause(&id, false).expect("resume should succeed");
    assert!(!resumed.paused);
    h.sources.sync(&id).expect("resume 后 sync 必须可用");
    wait_until_idle(&h.sources, &id);
}

#[test]
fn pause_stops_running_backfill_and_resume_sync_converges() {
    let h = harness();
    let root = h.db_dir.path().join("many");
    for i in 0..400 {
        write_file(
            &root,
            &format!("note-{i}.md"),
            &format!("# 标题{i}\n\n第{i}段内容，中文正文若干。\n"),
        );
    }
    let summary = h
        .sources
        .add(&root, "many", None)
        .expect("add should succeed");

    // 补扫进行中暂停（可能晚于完成——两种结局都必须收敛）。
    let _ = h.sources.pause(&summary.id, true);
    let paused = wait_until_idle(&h.sources, &summary.id);
    assert!(paused.paused, "暂停后列表必须显示已暂停");

    // 恢复 + 手动同步 → 文件数收敛到 400。
    h.sources.pause(&summary.id, false).expect("resume");
    h.sources.sync(&summary.id).expect("sync after resume");
    let idle = wait_until_idle(&h.sources, &summary.id);
    assert_eq!(idle.file_count, 400, "恢复并同步后必须收敛到全部文件");
}

#[test]
fn remove_cleans_index_but_keeps_source_files_intact() {
    let (h, root, id) = seeded_source();
    let hashes_before: Vec<String> = ["笔记.md", "plain.txt"]
        .iter()
        .map(|rel| sha256_of_file(&root.join(rel)))
        .collect();

    h.sources.remove(&id).expect("remove should succeed");
    assert!(
        h.sources.list().unwrap().is_empty(),
        "移除后来源列表必须为空"
    );
    let inspector =
        rusqlite::Connection::open(h.db_dir.path().join("insightforge.sqlite")).unwrap();
    for table in [
        "knowledge_source",
        "source_file",
        "fragment",
        "fragment_fts",
    ] {
        let count: i64 = inspector
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0, "移除必须清空 {table}");
    }

    let hashes_after: Vec<String> = ["笔记.md", "plain.txt"]
        .iter()
        .map(|rel| sha256_of_file(&root.join(rel)))
        .collect();
    assert_eq!(
        hashes_before, hashes_after,
        "移除只清理索引记录，源文件必须原样保留"
    );
}

#[test]
fn remove_while_indexing_waits_then_cleans() {
    let h = harness();
    let root = h.db_dir.path().join("many");
    for i in 0..400 {
        write_file(
            &root,
            &format!("note-{i}.md"),
            &format!("# 标题{i}\n\n第{i}段内容，中文正文若干。\n"),
        );
    }
    let summary = h
        .sources
        .add(&root, "many", None)
        .expect("add should succeed");

    // 索引进行中移除：应等待补扫停止后清理，且不 panic。
    h.sources
        .remove(&summary.id)
        .expect("remove during indexing should succeed");
    assert!(h.sources.list().unwrap().is_empty());
    let inspector =
        rusqlite::Connection::open(h.db_dir.path().join("insightforge.sqlite")).unwrap();
    let sources_left: i64 = inspector
        .query_row("SELECT COUNT(*) FROM knowledge_source", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(sources_left, 0, "移除后不得残留来源记录");
}

// ---------------------------------------------------------------------------
// 切片 7：只读边界（CONTEXT.md 铁律 1 / M0 第 4 项）——源文件永不被修改、
// 逃逸与 traversal 用例拒绝、ignore 规则生效。
// ---------------------------------------------------------------------------

#[test]
fn backfill_and_sync_never_modify_source_files() {
    let h = harness();
    let root = h.db_dir.path().join("kb");
    write_file(
        &root,
        "笔记.md",
        "# 架构\n\n本地优先的核心思想。\n\n## 边界\n\n知识库只读。\n",
    );
    write_file(
        &root,
        "plain.txt",
        "第一段，一些中文内容。\n\n第二段，更多内容。\n",
    );

    let before: Vec<(String, String, std::fs::Metadata)> = ["笔记.md", "plain.txt"]
        .iter()
        .map(|rel| {
            let path = root.join(rel);
            (
                rel.to_string(),
                sha256_of_file(&path),
                fs::metadata(&path).unwrap(),
            )
        })
        .collect();

    let summary = h
        .sources
        .add(&root, "kb", None)
        .expect("add should succeed");
    wait_until_idle(&h.sources, &summary.id);
    h.sources.sync(&summary.id).expect("sync 1");
    wait_until_idle(&h.sources, &summary.id);
    h.sources.sync(&summary.id).expect("sync 2");
    wait_until_idle(&h.sources, &summary.id);

    for (rel, hash_before, meta_before) in &before {
        let path = root.join(rel);
        let meta_after = fs::metadata(&path).unwrap();
        assert_eq!(
            sha256_of_file(&path),
            *hash_before,
            "{rel} 的内容 hash 前后必须不变"
        );
        assert_eq!(
            meta_after.modified().unwrap(),
            meta_before.modified().unwrap(),
            "{rel} 的 mtime 前后必须不变（未发生任何写入）"
        );
        assert_eq!(
            meta_after.len(),
            meta_before.len(),
            "{rel} 的大小前后必须不变"
        );
    }
}

#[test]
fn junction_escape_outside_root_is_never_indexed() {
    let h = harness();
    let base = h.db_dir.path();
    let root = base.join("kb");
    let outside = base.join("outside");
    write_file(&root, "keep.md", "# 保留\n\n正常内容。\n");
    write_file(
        &outside,
        "escape.md",
        "# 逃逸标记\n\n这段内容在知识库根目录之外，绝不能被索引。\n",
    );
    // std 的 junction API 仍 unstable：经 mklink /J 创建目录联接（无需特权）。
    let link = root.join("sublink");
    let output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&outside)
        .output()
        .expect("spawn mklink");
    assert!(
        output.status.success(),
        "mklink /J must succeed without privileges: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let summary = h
        .sources
        .add(&root, "kb", None)
        .expect("add should succeed");
    let idle = wait_until_idle(&h.sources, &summary.id);
    assert_eq!(idle.file_count, 1, "逃逸 junction 内的文件绝不能进入索引");

    let inspector =
        rusqlite::Connection::open(h.db_dir.path().join("insightforge.sqlite")).unwrap();
    let leaked: i64 = inspector
        .query_row(
            "SELECT COUNT(*) FROM fragment_fts WHERE fragment_fts MATCH '逃逸标记'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(leaked, 0, "根目录之外的内容绝不能被检索到");
}

#[test]
fn symlink_file_escape_outside_root_is_rejected() {
    let h = harness();
    let base = h.db_dir.path();
    let root = base.join("kb");
    let outside = base.join("outside");
    write_file(&root, "keep.md", "# 保留\n\n正常内容。\n");
    write_file(&outside, "escape.md", "逃逸内容标记 ABCDEF。\n");

    let link = root.join("escape.md");
    if let Err(err) = std::os::windows::fs::symlink_file(outside.join("escape.md"), &link) {
        // 本机/CI 未授予 SeCreateSymbolicLinkPrivilege 时记录跳过；
        // 目录逃逸由 junction_escape_outside_root_is_never_indexed 提供可执行证据，
        // 两者共用同一 stays_within_root 边界检查。
        eprintln!("SKIP symlink-file case (no privilege): {err}");
        return;
    }

    let summary = h
        .sources
        .add(&root, "kb", None)
        .expect("add should succeed");
    let idle = wait_until_idle(&h.sources, &summary.id);
    assert_eq!(idle.file_count, 1, "逃逸的 symlink 文件不能进入索引");
    let inspector =
        rusqlite::Connection::open(h.db_dir.path().join("insightforge.sqlite")).unwrap();
    let leaked: i64 = inspector
        .query_row(
            "SELECT COUNT(*) FROM fragment_fts WHERE fragment_fts MATCH 'ABCDEF'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(leaked, 0);
}

#[test]
fn tampered_traversal_rel_path_is_rejected_on_sync() {
    let (h, root, id) = seeded_source();
    // 根目录之外的「机密」文件：即使数据库记录被篡改也绝不能被读到。
    let secret = h.db_dir.path().join("secret.md");
    fs::write(&secret, "根目录之外的机密内容 ZZYYXX。").unwrap();

    // 直接篡改数据库（模拟被污染的 source_file 记录）。
    let inspector =
        rusqlite::Connection::open(h.db_dir.path().join("insightforge.sqlite")).unwrap();
    inspector
        .execute(
            "INSERT INTO source_file (source_id, rel_path, content_hash, size, mtime_ms, status)
             VALUES ((SELECT id FROM knowledge_source LIMIT 1), '../secret.md', 'tampered', 0, 0, 'active')",
            [],
        )
        .unwrap();
    drop(inspector);
    assert!(root.join("../secret.md").exists() || secret.exists());

    h.sources
        .sync(&id)
        .expect("补扫遇到 traversal 记录必须拒绝而非读取");
    wait_until_idle(&h.sources, &id);

    let inspector =
        rusqlite::Connection::open(h.db_dir.path().join("insightforge.sqlite")).unwrap();
    let leaked: i64 = inspector
        .query_row(
            "SELECT COUNT(*) FROM fragment_fts WHERE fragment_fts MATCH 'ZZYYXX'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(leaked, 0, "path traversal 记录绝不能导致读取根外文件");
    let tampered_fragments: i64 = inspector
        .query_row(
            "SELECT COUNT(*) FROM fragment WHERE source_file_id IN
                (SELECT id FROM source_file WHERE rel_path = '../secret.md')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        tampered_fragments, 0,
        "被篡改的 traversal 记录不得挂任何片段"
    );
}

#[test]
fn default_ignore_globs_exclude_noise_files() {
    let h = harness();
    let root = h.db_dir.path().join("kb");
    write_file(&root, "keep.md", "# 保留\n正文。\n");
    write_file(&root, "node_modules/pkg/readme.md", "噪声。");
    write_file(&root, "sub/cache.tmp", "噪声。");
    write_file(&root, ".git/config.md", "噪声。");

    let summary = h
        .sources
        .add(&root, "kb", None)
        .expect("add should succeed");
    let idle = wait_until_idle(&h.sources, &summary.id);
    assert_eq!(idle.file_count, 1, "默认忽略规则必须排除噪声文件");
}

#[test]
fn custom_ignore_globs_replace_defaults_and_are_honored() {
    let h = harness();
    let root = h.db_dir.path().join("kb");
    write_file(&root, "keep.md", "# 保留\n正文。\n");
    write_file(&root, "secret.md", "机密。");
    write_file(&root, "build/extra.md", "默认规则下会被 build 忽略。");

    let summary = h
        .sources
        .add(&root, "kb", Some(vec!["secret*".to_string()]))
        .expect("add with custom globs should succeed");
    let idle = wait_until_idle(&h.sources, &summary.id);

    assert_eq!(
        idle.ignore_globs,
        vec!["secret*".to_string()],
        "自定义规则必须替换默认规则"
    );
    assert_eq!(
        idle.file_count, 2,
        "secret.md 被忽略；默认的 build 忽略不再生效（build/extra.md 被索引）"
    );
}
