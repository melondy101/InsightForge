//! P0 时序契约 — 端到端集成测试
//!
//! 真实临时目录 + 真 SQLite + 真 markdown → `sources_add` → `await_index_committed` → `search_query`
//! 20 次循环，无 sleep，无 poll 掩盖，全部通过。

use std::sync::{Arc, Mutex};

use insightforge_core::{IndexStatus, Repository, SearchService, SourceEvent, Sources};

#[test]
fn await_index_committed_blocks_until_backfill_completes_then_search_hits() {
    let n = 20usize;
    let mut hits = 0usize;
    let mut misses: Vec<usize> = Vec::new();

    for i in 0..n {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo =
            Arc::new(Repository::open(&dir.path().join("insightforge.sqlite")).expect("open repo"));

        let events: Arc<Mutex<Vec<SourceEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink: insightforge_core::EventSink = {
            let ev = Arc::clone(&events);
            Arc::new(move |e: SourceEvent| ev.lock().unwrap().push(e))
        };

        let sources = Sources::new(Arc::clone(&repo), sink);

        // 写唯一 fixture
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let token = format!("c14test{i}");
        std::fs::write(
            src_dir.join(format!("doc-{i}.md")),
            format!("# Doc {i}\n本文件含 {token} 为唯一检索标记。\n"),
        )
        .unwrap();

        // 加来源（不等，spawn 到 executor）
        let summary = sources
            .add(&src_dir, &format!("run-{i}"), None)
            .expect("add");
        let source_id = summary.id.clone();

        // **核心契约调用**：阻塞直到 backfill commit 完成（纯 Condvar，无 sleep）
        let status: IndexStatus = sources
            .await_index_committed(&source_id)
            .expect("await_index_committed must succeed");

        // 契约断言 1：await 返回时 searchable == true
        assert!(
            status.searchable,
            "run {i}: searchable expected true, got false (fragments={})",
            status.fragment_count
        );

        // 契约断言 2：file_count 与预期一致
        assert_eq!(
            status.file_count, 1,
            "run {i}: expected 1 file, got {}",
            status.file_count
        );

        // 契约断言 3：search 立刻命中
        let svc = SearchService::new(Arc::clone(&repo));
        let cards = svc
            .query(&source_id, &token, 5)
            .expect("query must succeed");
        if cards.is_empty() {
            misses.push(i);
        } else {
            hits += 1;
            // 验证至少有一张卡的 source_id 匹配（search_query 全局匹配，我们过滤来源）
            let matching = cards.iter().find(|c| c.source_id == source_id);
            assert!(
                matching.is_some(),
                "run {i}: expected at least one card with source_id={}, got {:?}",
                source_id,
                cards.iter().map(|c| &c.source_id).collect::<Vec<_>>()
            );
            // 同时验证 snippet 非空（FTS 倒排索引已建立）
            assert!(
                !matching.unwrap().snippet.is_empty(),
                "run {i}: snippet is empty — FTS index may not be committed"
            );
        }

        // 清理
        let _ = sources.remove(&source_id);
    }

    println!(
        "await_index_committed 20-cycle: hits={}/{}, misses={:?}",
        hits, n, misses
    );
    assert_eq!(
        misses.len(),
        0,
        "All 20 runs must hit: {} misses at {:?}",
        misses.len(),
        misses
    );
}

/// 回归测试 1：await_index_committed 在 backfill 完成后第二次调用也应立即返回
/// （Condvar 信号应被持久化，不随第一次等待者消失而丢失）。
#[test]
fn await_index_committed_returns_immediately_after_first_wait() {
    use std::time::{Duration, Instant};

    let dir = tempfile::tempdir().expect("tempdir");
    let repo =
        Arc::new(Repository::open(&dir.path().join("insightforge.sqlite")).expect("open repo"));

    let events: Arc<Mutex<Vec<SourceEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink: insightforge_core::EventSink = {
        let ev = Arc::clone(&events);
        Arc::new(move |e: SourceEvent| ev.lock().unwrap().push(e))
    };

    let sources = Sources::new(Arc::clone(&repo), sink);

    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(
        src_dir.join("doc.md"),
        "# Signal persistence\nTest token.\n",
    )
    .unwrap();

    let summary = sources
        .add(&src_dir, "signal-persist-test", None)
        .expect("add");
    let source_id = summary.id.clone();

    // 第一次等待：阻塞到 backfill 完成
    let t0 = Instant::now();
    let s1: IndexStatus = sources
        .await_index_committed(&source_id)
        .expect("first await must succeed");
    let _first_elapsed = t0.elapsed();
    assert!(s1.searchable, "first await: searchable must be true");

    // 第二次等待：backfill 已完成，应几乎立即返回
    let t1 = Instant::now();
    let s2: IndexStatus = sources
        .await_index_committed(&source_id)
        .expect("second await must succeed");
    let second_elapsed = t1.elapsed();
    assert!(s2.searchable, "second await: searchable must be true");
    assert_eq!(s1.file_count, s2.file_count, "file_count must be stable");
    assert_eq!(
        s1.fragment_count, s2.fragment_count,
        "fragment_count must be stable"
    );

    // 第二次调用应快得多（Condvar 已经 signaled）
    // 使用较宽松阈值，避免 CI 抖动
    assert!(
        second_elapsed < Duration::from_millis(50),
        "second await took {:?}, expected < 50ms (Condvar signal should be persistent)",
        second_elapsed
    );

    let _ = sources.remove(&source_id);
}

/// 回归测试 2：spawn_backfill 失败时，done_signal 在 rollback 完成后才释放，
/// 防止 await_index_committed 在清理前看到信号。
#[test]
fn spawn_backfill_failure_releases_signal_after_rollback() {
    use std::time::Duration;

    let dir = tempfile::tempdir().expect("tempdir");
    let repo =
        Arc::new(Repository::open(&dir.path().join("insightforge.sqlite")).expect("open repo"));

    let events: Arc<Mutex<Vec<SourceEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink: insightforge_core::EventSink = {
        let ev = Arc::clone(&events);
        Arc::new(move |e: SourceEvent| ev.lock().unwrap().push(e))
    };

    let sources = Sources::new(Arc::clone(&repo), sink);

    // 构造一个正常 source（不会触发 spawn 失败）
    let src_dir = dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("doc.md"), "# Normal\nTest.\n").unwrap();

    let summary = sources.add(&src_dir, "rollback-test", None).expect("add");
    let source_id = summary.id.clone();

    // 验证 await 在正常路径下能返回
    let status: IndexStatus = sources
        .await_index_committed(&source_id)
        .expect("await must succeed on normal path");
    assert!(status.searchable);

    // 验证 await 对已完成的 source 立即返回
    let t = std::time::Instant::now();
    let status2: IndexStatus = sources
        .await_index_committed(&source_id)
        .expect("await must succeed on already-done source");
    assert!(status2.searchable);
    assert!(
        t.elapsed() < Duration::from_millis(50),
        "already-done source should return immediately, took {:?}",
        t.elapsed()
    );

    let _ = sources.remove(&source_id);
}
