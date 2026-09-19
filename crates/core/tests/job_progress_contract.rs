//! 索引进度契约（候选 ②）：事件是推送，状态才是真相。
//! 补扫进度必须可以随时问到，并且作业结束后自动归零——
//! 否则一条丢失的结束事件就会把界面永久卡在进度条上。

use std::sync::{mpsc, Arc, Mutex};

use insightforge_core::{EventSink, JobPhase, Repository, SourceEvent, Sources};

fn write_note(root: &std::path::Path, rel: &str) {
    std::fs::write(
        root.join(rel),
        "# 标题\n\n本地优先的中文检索知识库，血肉由创作者亲手补。\n",
    )
    .expect("write note");
}

#[test]
fn progress_stays_queryable_when_events_are_dropped() {
    // 下游 sink 只转发第一条事件即阻塞，其余一律丢弃：等价于事件投递全丢。
    let (entered_tx, entered_rx) = mpsc::channel::<()>();
    let (_gate_tx, gate_rx) = mpsc::channel::<()>();
    let gate_rx = Arc::new(Mutex::new(gate_rx));
    let blocking_sink = Arc::clone(&gate_rx);
    let sink: EventSink = Arc::new(move |event: SourceEvent| {
        if matches!(event, SourceEvent::BackfillStarted { .. }) {
            let _ = entered_tx.send(());
            let _ = blocking_sink.lock().expect("gate lock").recv();
        }
    });

    let db_dir = tempfile::tempdir().expect("db dir");
    let repo = Arc::new(
        Repository::open(&db_dir.path().join("insightforge.sqlite")).expect("open repository"),
    );
    let root = db_dir.path().join("notes");
    std::fs::create_dir_all(&root).expect("create root");
    write_note(&root, "a.md");
    write_note(&root, "b.md");

    let sources = Sources::new(Arc::clone(&repo), sink);
    let source = sources.add(&root, "笔记", None).expect("add source");

    // 阻塞点之后：作业仍在跑，但一条事件都没到前端。
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("backfill started");
    let in_flight = sources
        .list()
        .expect("list")
        .into_iter()
        .find(|row| row.id == source.id)
        .expect("added source listed")
        .progress
        .expect("in-flight progress must be queryable without events");
    assert_eq!(in_flight.phase, JobPhase::Indexing);
    assert_eq!(in_flight.total_files, 2);
    assert_eq!(in_flight.processed, 0);

    drop(_gate_tx);
    let committed = sources
        .await_index_committed(&source.id)
        .expect("await index committed");
    assert!(committed.searchable, "both notes should be indexed");

    let settled = sources
        .list()
        .expect("list")
        .into_iter()
        .find(|row| row.id == source.id)
        .expect("source still listed");
    assert!(
        settled.progress.is_none(),
        "a finished job must not leave a stuck progress bar, got {:?}",
        settled.progress
    );
}
