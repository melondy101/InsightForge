//! 间隔校验契约（ARCHITECTURE「watcher 不是事实来源」的另一半）：
//! 最终一致由 5/15/30 分钟间隔校验兜底，所以这个设置必须真的决定「什么时候再扫一次」。
//! 它曾经只是往 KV 里写一个没人读的键。

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::TimeDelta;
use insightforge_core::clock::{format_iso, parse_iso};
use insightforge_core::{
    Repository, SourceStatus, SourceSummary, Sources, SourcesError, DEFAULT_SYNC_INTERVAL_MINUTES,
};

struct Harness {
    /// 只为了持有生命周期：字段一丢，临时目录就被清掉。
    _db: tempfile::TempDir,
    _root: tempfile::TempDir,
    repo: Arc<Repository>,
    sources: Sources,
}

impl Harness {
    fn new() -> Self {
        let db = tempfile::tempdir().expect("temp dir");
        let repo = Arc::new(
            Repository::open(&db.path().join("insightforge.sqlite")).expect("open repository"),
        );
        Self {
            sources: Sources::new(Arc::clone(&repo), Arc::new(|_| {})),
            _db: db,
            repo,
            _root: tempfile::tempdir().expect("root dir"),
        }
    }

    /// 授权一个真实目录并等首次补扫落定（落定才会写下 `last_synced_at`）。
    fn add_source(&self) -> SourceSummary {
        std::fs::write(self._root.path().join("note.md"), "# 标题\n\n正文。\n")
            .expect("write note");
        let added = self
            .sources
            .add(self._root.path(), "笔记", None)
            .expect("add source");
        self.wait_until_idle(&added.id)
    }

    fn wait_until_idle(&self, id: &str) -> SourceSummary {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let summary = self.summary(id);
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

    fn summary(&self, id: &str) -> SourceSummary {
        self.sources
            .list()
            .expect("list")
            .into_iter()
            .find(|source| source.id == id)
            .expect("source exists")
    }

    /// 把「上次同步时刻」改写成确定值，免得断言依赖真实时钟。
    fn stamp_last_sync(&self, id: &str, stamp: Option<&str>) {
        let id = id.to_string();
        let stamp = stamp.map(|text| text.to_string());
        self.repo
            .write(move |conn| {
                conn.execute(
                    "UPDATE knowledge_source SET last_synced_at = ?2 WHERE id = ?1",
                    rusqlite::params![id, stamp],
                )?;
                Ok(())
            })
            .expect("stamp last_synced_at");
    }

    fn set_raw_interval(&self, id: &str, value: &str) {
        let key = format!("sources.sync_interval.{id}");
        let value = value.to_string();
        self.repo
            .write(move |conn| {
                conn.execute(
                    "INSERT INTO app_setting (key, value) VALUES (?1, ?2)
                     ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                    rusqlite::params![key, value],
                )?;
                Ok(())
            })
            .expect("seed interval");
    }

    fn stored_interval_setting(&self, id: &str) -> Option<String> {
        let key = format!("sources.sync_interval.{id}");
        self.repo
            .read(move |conn| {
                let value: Option<String> = conn
                    .query_row(
                        "SELECT value FROM app_setting WHERE key = ?1",
                        rusqlite::params![key],
                        |row| row.get(0),
                    )
                    .unwrap_or(None);
                Ok(value)
            })
            .expect("read setting")
    }
}

/// 以补扫落定的那一刻为基准，往后偏移若干分钟。
fn minutes_after(synced_at: &str, minutes: i64) -> String {
    let synced = parse_iso(synced_at).expect("last_synced_at is the project format");
    format_iso(synced + TimeDelta::minutes(minutes))
}

#[test]
fn a_new_source_carries_the_documented_default_interval() {
    let fx = Harness::new();

    let source = fx.add_source();

    assert_eq!(source.sync_interval_minutes, DEFAULT_SYNC_INTERVAL_MINUTES);
}

#[test]
fn last_sync_is_stamped_with_the_project_clock_format() {
    // 混合精度（SQLite 的 3 位毫秒 vs 项目的 6 位微秒）会让按字符串判先后出错。
    let fx = Harness::new();

    let source = fx.add_source();

    let stamp = source.last_synced_at.expect("a finished sync stamps");
    assert_eq!(stamp.chars().count(), 27, "fixed width, got {stamp}");
    assert!(parse_iso(&stamp).is_some());
}

#[test]
fn only_the_documented_intervals_can_be_selected() {
    let fx = Harness::new();
    let source = fx.add_source();

    for minutes in [0_u32, 1, 7, 60] {
        let err = fx
            .sources
            .set_sync_interval(&source.id, minutes)
            .expect_err("{minutes} should not be a selectable interval");
        assert!(
            matches!(&err, SourcesError::InvalidSyncInterval(got) if *got == minutes),
            "got {err:?}"
        );
        assert_eq!(err.code(), "invalid_sync_interval");
        assert_eq!(
            fx.summary(&source.id).sync_interval_minutes,
            DEFAULT_SYNC_INTERVAL_MINUTES,
            "被拒绝的设置不能留下半个改动"
        );
    }

    for minutes in [5_u32, 15, 30] {
        let updated = fx
            .sources
            .set_sync_interval(&source.id, minutes)
            .expect("a documented interval is accepted");
        assert_eq!(updated.sync_interval_minutes, minutes);
        assert_eq!(fx.summary(&source.id).sync_interval_minutes, minutes);
    }
}

#[test]
fn a_source_becomes_due_exactly_when_its_interval_has_elapsed() {
    let fx = Harness::new();
    let source = fx.add_source();
    fx.sources
        .set_sync_interval(&source.id, 15)
        .expect("15 is selectable");
    let base = source.last_synced_at.expect("sync stamped");

    assert!(
        fx.sources
            .sync_due(&minutes_after(&base, 14))
            .expect("due check")
            .is_empty(),
        "第 14 分钟还不该重扫"
    );
    assert_eq!(
        fx.sources
            .sync_due(&minutes_after(&base, 15))
            .expect("due check"),
        vec![source.id],
        "第 15 分钟该到点"
    );
}

#[test]
fn the_selected_interval_is_what_decides_when_a_source_is_due() {
    let fx = Harness::new();
    let source = fx.add_source();
    fx.sources
        .set_sync_interval(&source.id, 5)
        .expect("5 is selectable");
    let base = source.last_synced_at.expect("sync stamped");

    assert_eq!(
        fx.sources
            .sync_due(&minutes_after(&base, 5))
            .expect("due check"),
        vec![source.id],
        "间隔改成 5 分钟，第 5 分钟就该到点"
    );
}

#[test]
fn a_paused_source_is_never_due() {
    let fx = Harness::new();
    let source = fx.add_source();
    let base = source.last_synced_at.expect("sync stamped");
    fx.sources.pause(&source.id, true).expect("pause");

    let due = fx
        .sources
        .sync_due(&minutes_after(&base, 60))
        .expect("due check");

    assert!(due.is_empty(), "暂停的来源不该被心跳叫醒：{due:?}");
}

#[test]
fn a_source_that_never_finished_a_sync_is_due() {
    let fx = Harness::new();
    let source = fx.add_source();
    fx.stamp_last_sync(&source.id, None);

    let due = fx
        .sources
        .sync_due("2026-01-02T03:04:05.000000Z")
        .expect("due check");

    assert_eq!(due, vec![source.id]);
}

#[test]
fn an_unreadable_last_sync_stamp_counts_as_due_so_the_next_sync_heals_it() {
    let fx = Harness::new();
    let source = fx.add_source();
    fx.stamp_last_sync(&source.id, Some("昨天下午"));

    let due = fx
        .sources
        .sync_due("2026-01-02T03:04:05.000000Z")
        .expect("due check");

    assert_eq!(due, vec![source.id], "读不懂的时间戳不该让来源永不校验");
}

#[test]
fn an_interval_outside_the_documented_choices_falls_back_to_the_default() {
    let fx = Harness::new();
    let source = fx.add_source();
    // 直接写 KV，绕过校验：模拟旧版本或外部工具留下的非法值。
    fx.set_raw_interval(&source.id, "7");

    let summary = fx.summary(&source.id);

    assert_eq!(summary.sync_interval_minutes, DEFAULT_SYNC_INTERVAL_MINUTES);
    assert!(
        fx.sources
            .sync_due(&minutes_after(
                &summary.last_synced_at.expect("sync stamped"),
                6
            ))
            .expect("due check")
            .is_empty(),
        "坏值不能被当成 7 分钟用"
    );
}

#[test]
fn removing_a_source_takes_its_interval_setting_with_it() {
    let fx = Harness::new();
    let source = fx.add_source();
    fx.sources
        .set_sync_interval(&source.id, 30)
        .expect("30 is selectable");
    assert_eq!(
        fx.stored_interval_setting(&source.id).as_deref(),
        Some("30")
    );

    fx.sources.remove(&source.id).expect("remove source");

    assert_eq!(fx.stored_interval_setting(&source.id), None);
}
