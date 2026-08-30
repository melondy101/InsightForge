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
