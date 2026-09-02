//! 阻塞型工作（扫描/解析/hash/写入）的专门 executor 与显式取消令牌
//! （ARCHITECTURE §3：已开始的 blocking work 不能靠丢弃 handle 停止）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use thiserror::Error;

/// executor 队列里的一个作业。
type Job = Box<dyn FnOnce() + Send + 'static>;

#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("executor is shut down")]
    Closed,
}

/// 固定工作线程数的阻塞型 executor：命令路径只负责投递作业，
/// 重活全部在 executor 线程上执行，UI 路径不被阻塞。
#[derive(Debug)]
pub struct Executor {
    sender: Option<mpsc::Sender<Job>>,
    workers: Vec<std::thread::JoinHandle<()>>,
}

impl Executor {
    /// 创建 executor 并启动 `worker_count` 个工作线程。
    pub fn new(worker_count: usize) -> Self {
        let (sender, receiver) = mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        let workers = (0..worker_count.max(1))
            .map(|index| {
                let receiver = Arc::clone(&receiver);
                std::thread::Builder::new()
                    .name(format!("insightforge-executor-{index}"))
                    .spawn(move || loop {
                        let job = receiver.lock().expect("executor receiver lock").recv();
                        match job {
                            Ok(job) => job(),
                            Err(_) => break, // 队列关闭，工作线程退出
                        }
                    })
                    .expect("spawn executor worker thread")
            })
            .collect();
        Self {
            sender: Some(sender),
            workers,
        }
    }

    /// 投递一个作业；executor 已关闭时返回 [`ExecutorError::Closed`]。
    pub fn spawn(&self, job: Job) -> Result<(), ExecutorError> {
        self.sender
            .as_ref()
            .ok_or(ExecutorError::Closed)?
            .send(job)
            .map_err(|_| ExecutorError::Closed)
    }
}

impl Drop for Executor {
    fn drop(&mut self) {
        // 丢弃 sender 关闭队列，工作线程处理完当前作业后退出，join 等待收尾。
        self.sender.take();
        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }
    }
}

/// 显式取消令牌：可克隆共享，一次置位处处可见。
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// 请求取消（幂等）。
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// 是否已被请求取消。
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn jobs_run_on_executor_threads_and_all_complete() {
        let executor = Executor::new(2);
        let completed = Arc::new(AtomicUsize::new(0));
        let foreign_thread_hits = Arc::new(AtomicUsize::new(0));
        let caller_thread = std::thread::current().id();

        for _ in 0..16 {
            let completed = Arc::clone(&completed);
            let foreign_thread_hits = Arc::clone(&foreign_thread_hits);
            executor
                .spawn(Box::new(move || {
                    if std::thread::current().id() != caller_thread {
                        foreign_thread_hits.fetch_add(1, Ordering::SeqCst);
                    }
                    completed.fetch_add(1, Ordering::SeqCst);
                }))
                .expect("spawn while executor alive should work");
        }

        let deadline = Instant::now() + Duration::from_secs(10);
        while completed.load(Ordering::SeqCst) < 16 {
            assert!(
                Instant::now() < deadline,
                "executor jobs did not finish in time"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            foreign_thread_hits.load(Ordering::SeqCst) > 0,
            "作业必须运行在 executor 线程而非调用线程"
        );
    }

    #[test]
    fn cancel_token_is_shared_across_clones() {
        let token = CancelToken::new();
        let clone = token.clone();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(clone.is_cancelled(), "克隆令牌必须共享取消状态");
    }
}
