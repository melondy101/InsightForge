//! 索引进度状态机（候选 ②）：把「补扫到哪了 / 结束了没」从只有事件边沿，
//! 变成随时可读的一份状态。事件是推送，状态是真相——事件丢了，界面也不该卡住。

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use crate::sources::{EventSink, SourceEvent};

/// 补扫阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobPhase {
    Indexing,
    Completed,
    Cancelled,
    Failed,
}

/// 某来源当前进度（前端薄投影；`Sources::list` 直接带出去）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSnapshot {
    pub phase: JobPhase,
    pub processed: u64,
    pub total_files: u64,
    pub fragment_count: u64,
}

impl JobSnapshot {
    fn indexing(total_files: u64) -> Self {
        Self {
            phase: JobPhase::Indexing,
            processed: 0,
            total_files,
            fragment_count: 0,
        }
    }
}

fn still_indexing(jobs: &std::collections::HashMap<String, JobSnapshot>, source_id: &str) -> bool {
    jobs.get(source_id)
        .is_some_and(|job| job.phase == JobPhase::Indexing)
}

#[derive(Default)]
pub struct JobProgress {
    jobs: std::sync::Mutex<std::collections::HashMap<String, JobSnapshot>>,
    settled: std::sync::Condvar,
}

impl JobProgress {
    pub fn new() -> Self {
        Self::default()
    }

    /// 挂在事件通道上：先记状态，再转发下游（Tauri 事件 / 测试收集器）。
    /// 状态机不取代事件，只是让它不再成为唯一的信息来源。
    pub fn pipe(self: &Arc<Self>, downstream: EventSink) -> EventSink {
        let state = Arc::clone(self);
        Arc::new(move |event: SourceEvent| {
            state.record(&event);
            downstream(event);
        })
    }

    pub fn record(&self, event: &SourceEvent) {
        let (id, snapshot) = match event {
            SourceEvent::BackfillStarted {
                source_id,
                total_files,
            } => (source_id, JobSnapshot::indexing(*total_files)),
            SourceEvent::FileIndexed {
                source_id,
                rel_path: _,
                processed,
                total_files,
                fragment_count,
            } => (
                source_id,
                JobSnapshot {
                    phase: JobPhase::Indexing,
                    processed: *processed,
                    total_files: *total_files,
                    fragment_count: *fragment_count,
                },
            ),
            SourceEvent::BackfillCompleted {
                source_id,
                indexed_files,
                total_files,
            } => (
                source_id,
                JobSnapshot {
                    phase: JobPhase::Completed,
                    processed: *indexed_files,
                    total_files: *total_files,
                    fragment_count: 0,
                },
            ),
            SourceEvent::BackfillCancelled {
                source_id,
                indexed_files,
            } => (
                source_id,
                JobSnapshot {
                    phase: JobPhase::Cancelled,
                    processed: *indexed_files,
                    total_files: 0,
                    fragment_count: 0,
                },
            ),
            SourceEvent::BackfillFailed { source_id, .. } => (
                source_id,
                JobSnapshot {
                    phase: JobPhase::Failed,
                    processed: 0,
                    total_files: 0,
                    fragment_count: 0,
                },
            ),
        };
        let mut jobs = self.jobs.lock().expect("progress lock");
        jobs.insert(id.clone(), snapshot);
        self.settled.notify_all();
    }

    /// 作业已登记、还没发出任何事件时也算「在跑」。
    /// 少了这一步，`add` 之后立刻 `await_index_committed` 会因为状态还是空的而直接返回，
    /// 于是拿到 `searchable == false`（实测踩到过的竞态）。
    pub fn begin(&self, source_id: &str) {
        let mut jobs = self.jobs.lock().expect("progress lock");
        jobs.insert(source_id.to_string(), JobSnapshot::indexing(0));
    }

    pub fn snapshot(&self, source_id: &str) -> Option<JobSnapshot> {
        self.jobs
            .lock()
            .expect("progress lock")
            .get(source_id)
            .cloned()
    }

    /// 只在「还在跑」时暴露进度：跑完就消失，界面不会因为错过一条结束事件而卡在进度条上。
    pub fn running(&self, source_id: &str) -> Option<JobSnapshot> {
        self.snapshot(source_id)
            .filter(|job| job.phase == JobPhase::Indexing)
    }

    pub fn is_indexing(&self, source_id: &str) -> bool {
        self.running(source_id).is_some()
    }

    /// 阻塞到该来源落定（结束/取消/失败），或从未跑过。带超时用于可取消的等待路径。
    pub fn await_settled(&self, source_id: &str, timeout: Option<Duration>) -> bool {
        let jobs = self.jobs.lock().expect("progress lock");
        match timeout {
            None => {
                let settled = self
                    .settled
                    .wait_while(jobs, |jobs| still_indexing(jobs, source_id))
                    .expect("progress wait");
                drop(settled);
                true
            }
            Some(limit) => self
                .settled
                .wait_timeout_while(jobs, limit, |jobs| still_indexing(jobs, source_id))
                .map(|(_jobs, waited)| !waited.timed_out())
                .expect("progress wait"),
        }
    }

    /// 来源被移除后清掉状态，避免 id 复用时的陈旧进度。
    pub fn forget(&self, source_id: &str) {
        self.jobs.lock().expect("progress lock").remove(source_id);
        self.settled.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;

    fn collect() -> (EventSink, mpsc::Receiver<SourceEvent>) {
        let (tx, rx) = mpsc::channel();
        (
            Arc::new(move |event: SourceEvent| {
                let _ = tx.send(event);
            }),
            rx,
        )
    }

    fn started(id: &str, total: u64) -> SourceEvent {
        SourceEvent::BackfillStarted {
            source_id: id.to_string(),
            total_files: total,
        }
    }

    fn indexed(id: &str, processed: u64, total: u64, fragments: u64) -> SourceEvent {
        SourceEvent::FileIndexed {
            source_id: id.to_string(),
            rel_path: "a.md".to_string(),
            processed,
            total_files: total,
            fragment_count: fragments,
        }
    }

    fn completed(id: &str, files: u64) -> SourceEvent {
        SourceEvent::BackfillCompleted {
            source_id: id.to_string(),
            indexed_files: files,
            total_files: files,
        }
    }

    #[test]
    fn a_registered_job_counts_as_running_before_it_emits() {
        let progress = JobProgress::new();
        progress.begin("s1");

        assert!(
            progress.is_indexing("s1"),
            "登记后、首条事件前的窗口必须算在跑，否则等待者会误判已落定"
        );
        assert!(!progress.await_settled("s1", Some(Duration::from_millis(20))));

        progress.record(&completed("s1", 0));
        assert!(progress.await_settled("s1", Some(Duration::from_millis(20))));
    }

    #[test]
    fn in_flight_progress_is_readable_without_listening_to_events() {
        let progress = Arc::new(JobProgress::new());
        // 下游是个聋掉的 sink：模拟 Tauri dev 模式下事件投递丢失。
        progress.record(&started("s1", 10));
        progress.record(&indexed("s1", 4, 10, 12));

        let job = progress.running("s1").expect("indexing job is visible");
        assert_eq!((job.processed, job.total_files), (4, 10));
        assert_eq!(job.fragment_count, 12);
    }

    #[test]
    fn a_finished_job_never_lingers_as_progress() {
        let progress = Arc::new(JobProgress::new());
        progress.record(&started("s1", 3));
        progress.record(&completed("s1", 3));

        assert!(
            progress.running("s1").is_none(),
            "progress must clear itself, or a missed end event leaves a stuck bar"
        );
        assert_eq!(
            progress.snapshot("s1").map(|job| job.phase),
            Some(JobPhase::Completed),
            "the settled state is still queryable for diagnostics"
        );
    }

    #[test]
    fn pipe_records_and_forwards_every_event() {
        let progress = Arc::new(JobProgress::new());
        let (downstream, rx) = collect();
        let piped = progress.pipe(downstream);

        piped(started("s1", 2));
        piped(indexed("s1", 1, 2, 5));

        assert_eq!(
            rx.try_iter().count(),
            2,
            "downstream sink still gets events"
        );
        assert_eq!(progress.running("s1").unwrap().processed, 1);
    }

    #[test]
    fn await_settled_returns_immediately_when_nothing_runs() {
        let progress = Arc::new(JobProgress::new());
        assert!(progress.await_settled("never-ran", None));
    }

    #[test]
    fn await_settled_wakes_when_the_job_lands() {
        let progress = Arc::new(JobProgress::new());
        progress.record(&started("s1", 1));

        let finisher = {
            let progress = Arc::clone(&progress);
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(30));
                progress.record(&completed("s1", 1));
            })
        };

        assert!(
            progress.await_settled("s1", Some(Duration::from_secs(5))),
            "must be woken by the end event, not by the deadline"
        );
        finisher.join().expect("finisher thread");
        assert!(!progress.is_indexing("s1"));
    }

    #[test]
    fn unknown_source_is_already_settled() {
        let progress = JobProgress::new();
        assert!(!progress.is_indexing("s9"));
        assert!(progress.await_settled("s9", Some(Duration::from_millis(20))));
    }
}
