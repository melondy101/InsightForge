//! SQLite repository：应用数据目录下的唯一数据库所有者。
//!
//! 固化约定（spec）：WAL 模式 + 单一写入队列——所有读写都提交给一个
//! 专属 writer 线程串行执行，连接不与其他线程共享，天然规避写冲突。

use std::path::Path;
use std::sync::mpsc::{self, Sender};

use rusqlite::Connection;

mod migrations;

pub use migrations::{Migration, Migrator, MIGRATIONS};

/// writer 线程队列里的一个作业：拿到连接执行后自行把结果发回调用方。
type QueueJob = Box<dyn FnOnce(&mut Connection) + Send + 'static>;

/// repository 层错误类型。
#[derive(Debug)]
pub enum RepositoryError {
    /// 底层 SQLite 错误。
    Sqlite(rusqlite::Error),
    /// 文件系统错误（备份/清理损坏库等）。
    Io(std::io::Error),
    /// 打开后实际 journal 模式不是 WAL：多进程/多连接写入下无法保证持久化语义。
    UnsupportedJournalMode(String),
    /// 迁移列表非法（版本号未严格递增或重复）。
    InvalidMigrationSequence(String),
    /// writer 线程已退出（例如 repository 生命周期异常），队列不再接受作业。
    QueueClosed,
    /// writer 线程无法启动。
    WriterThreadSpawn(std::io::Error),
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepositoryError::Sqlite(err) => write!(f, "sqlite error: {err}"),
            RepositoryError::Io(err) => write!(f, "io error: {err}"),
            RepositoryError::UnsupportedJournalMode(mode) => {
                write!(f, "sqlite journal mode is `{mode}`, expected `wal`")
            }
            RepositoryError::InvalidMigrationSequence(details) => {
                write!(f, "invalid migration sequence: {details}")
            }
            RepositoryError::QueueClosed => {
                write!(f, "repository write queue is closed")
            }
            RepositoryError::WriterThreadSpawn(err) => {
                write!(f, "failed to spawn repository writer thread: {err}")
            }
        }
    }
}

impl std::error::Error for RepositoryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RepositoryError::Sqlite(err) => Some(err),
            RepositoryError::Io(err) => Some(err),
            RepositoryError::WriterThreadSpawn(err) => Some(err),
            RepositoryError::UnsupportedJournalMode(_)
            | RepositoryError::InvalidMigrationSequence(_)
            | RepositoryError::QueueClosed => None,
        }
    }
}

impl From<rusqlite::Error> for RepositoryError {
    fn from(err: rusqlite::Error) -> Self {
        RepositoryError::Sqlite(err)
    }
}

impl From<std::io::Error> for RepositoryError {
    fn from(err: std::io::Error) -> Self {
        RepositoryError::Io(err)
    }
}

/// 对既有 SQLite 文件做 `PRAGMA integrity_check`：所有行均为 `"ok"` 才算健康。
/// 任何读取/解析失败一律视为不健康（宁可保守备份，也不冒然信任损坏库）。
fn integrity_ok(path: &Path) -> bool {
    let Ok(conn) = Connection::open(path) else {
        return false;
    };
    let Ok(mut stmt) = conn.prepare("PRAGMA integrity_check") else {
        return false;
    };
    let Ok(mut rows) = stmt.query([]) else {
        return false;
    };
    loop {
        match rows.next() {
            Ok(Some(row)) => {
                let val: String = match row.get(0) {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                if val != "ok" {
                    return false;
                }
            }
            Ok(None) => break,
            Err(_) => return false,
        }
    }
    true
}

/// 把损坏库连同其 -wal/-shm 一并备份为 `<name>.corrupt.bak`，保留现场供人工恢复。
fn backup_corrupt(path: &Path) -> Result<(), RepositoryError> {
    let base = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "insightforge.sqlite".to_string());
    let main_backup = path.with_file_name(format!("{base}.corrupt.bak"));
    std::fs::copy(path, &main_backup)?;

    for suffix in ["-wal", "-shm"] {
        let aux = path.with_file_name(format!("{base}{suffix}"));
        if aux.exists() {
            let _ = std::fs::copy(&aux, path.with_file_name(format!("{base}{suffix}.corrupt.bak")));
        }
    }
    Ok(())
}

/// 删除损坏库主文件及 -wal/-shm，使 `open_with_migrations` 能在原位新建空库。
fn remove_db_files(path: &Path) -> Result<(), RepositoryError> {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(path.with_file_name(format!(
        "{}{}",
        path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        "-wal"
    )));
    let _ = std::fs::remove_file(path.with_file_name(format!(
        "{}{}",
        path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        "-shm"
    )));
    // 确认主文件已消失，否则后续 open 会在损坏文件上重建
    if path.exists() {
        return Err(RepositoryError::Io(std::io::Error::other(
            "failed to remove corrupt database file",
        )));
    }
    Ok(())
}

/// 应用数据库 repository：单一 writer 线程独占 SQLite 连接。
///
/// 克隆/共享该结构（如经 `Arc`）只共享队列入口；连接本身永远只在
/// writer 线程内被触碰。
pub struct Repository {
    write_queue: Sender<QueueJob>,
}

impl Repository {
    /// 当前二进制实际链接的 SQLite 版本号（如 3.51.3 → 3051003）。
    pub fn sqlite_libversion_number() -> i32 {
        unsafe { rusqlite::ffi::sqlite3_libversion_number() }
    }

    /// 当前二进制实际链接的 SQLite 版本字符串（如 `"3.53.2"`）。
    pub fn sqlite_libversion() -> &'static str {
        let cstr = unsafe { std::ffi::CStr::from_ptr(rusqlite::ffi::sqlite3_libversion()) };
        cstr.to_str().unwrap_or("unknown")
    }

    /// 在 `path` 打开（必要时创建）SQLite 数据库文件，并应用全部迁移。
    ///
    /// 固化约定：每次打开都把 journal 模式切到 WAL 并校验生效；
    /// 若环境不支持 WAL 则直接报错，绝不静默降级。
    ///
    /// 打开既有库前先做完整性校验：若库已损坏，备份为 `<path>.corrupt.bak` 后以空库
    /// 重新开始（安全降级——应用仍可启动，损坏文件被保留供人工恢复，而非直接崩溃）。
    pub fn open(path: &Path) -> Result<Self, RepositoryError> {
        Self::recover_if_corrupt(path)?;
        Self::open_with_migrations(path, MIGRATIONS)
    }

    /// 校验既有数据库完整性；若损坏则备份并以空库重启。返回是否发生了恢复。
    /// 文件不存在时视为无需处理，返回 `Ok(false)`。
    ///
    /// 安全降级策略：绝不因损坏库导致 `open` 崩溃；损坏文件先备份再丢弃，
    /// 让 `open_with_migrations` 在其位置新建空库。
    pub fn recover_if_corrupt(path: &Path) -> Result<bool, RepositoryError> {
        if !path.exists() {
            return Ok(false);
        }
        if integrity_ok(path) {
            return Ok(false);
        }
        backup_corrupt(path)?;
        remove_db_files(path)?;
        Ok(true)
    }

    /// 同 [`Repository::open`]，但使用调用方给定的迁移列表
    /// （shell 传入 [`MIGRATIONS`]；测试可注入临时迁移以驱动框架）。
    pub fn open_with_migrations(
        path: &Path,
        migrations: &[Migration],
    ) -> Result<Self, RepositoryError> {
        let conn = Connection::open(path)?;
        let mode: String = conn.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(RepositoryError::UnsupportedJournalMode(mode));
        }
        Migrator::new(migrations)?.run(&conn)?;

        let (write_queue, jobs) = mpsc::channel::<QueueJob>();
        std::thread::Builder::new()
            .name("insightforge-write-queue".to_string())
            .spawn(move || {
                let mut conn = conn;
                while let Ok(job) = jobs.recv() {
                    job(&mut conn);
                }
            })
            .map_err(RepositoryError::WriterThreadSpawn)?;

        Ok(Self { write_queue })
    }

    /// 把一个写操作提交到单一写入队列串行执行。
    ///
    /// 整个操作被包在一个 `BEGIN IMMEDIATE` 事务里：闭包失败即整体回滚，
    /// 成功才提交，保证「要么全落地、要么全不留」。
    pub fn write<T, F>(&self, op: F) -> Result<T, RepositoryError>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> Result<T, rusqlite::Error> + Send + 'static,
    {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.write_queue
            .send(Box::new(move |conn: &mut Connection| {
                let outcome = run_in_transaction(conn, op);
                let _ = reply_tx.send(outcome);
            }))
            .map_err(|_| RepositoryError::QueueClosed)?;
        wait_for_reply(reply_rx)
    }

    /// 把一个读操作提交给 writer 线程执行（与写共享同一连接，顺序一致）。
    pub fn read<T, F>(&self, op: F) -> Result<T, RepositoryError>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> Result<T, rusqlite::Error> + Send + 'static,
    {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.write_queue
            .send(Box::new(move |conn: &mut Connection| {
                let outcome = op(conn);
                let _ = reply_tx.send(outcome);
            }))
            .map_err(|_| RepositoryError::QueueClosed)?;
        wait_for_reply(reply_rx)
    }

    /// 当前 journal 模式（经 `PRAGMA journal_mode` 读取，可观察行为）。
    pub fn journal_mode(&self) -> Result<String, RepositoryError> {
        self.read(|conn| conn.query_row("PRAGMA journal_mode", [], |row| row.get(0)))
    }

    /// 已应用的迁移版本（升序），读自迁移簿记表。
    pub fn applied_migrations(&self) -> Result<Vec<i64>, RepositoryError> {
        self.read(|conn| {
            let mut stmt =
                conn.prepare("SELECT version FROM schema_migrations ORDER BY version")?;
            let versions = stmt
                .query_map([], |row| row.get(0))?
                .map_while(Result::ok)
                .collect();
            Ok(versions)
        })
    }
}

/// 在一个 IMMEDIATE 事务里执行写闭包：成功提交，失败回滚。
fn run_in_transaction<T, F>(conn: &mut Connection, op: F) -> Result<T, rusqlite::Error>
where
    F: FnOnce(&Connection) -> Result<T, rusqlite::Error>,
{
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let value = op(&tx)?;
    tx.commit()?;
    Ok(value)
}

/// 阻塞等待 writer 线程把作业结果发回来。
fn wait_for_reply<T>(
    reply_rx: mpsc::Receiver<Result<T, rusqlite::Error>>,
) -> Result<T, RepositoryError>
where
    T: Send + 'static,
{
    let outcome = reply_rx.recv().map_err(|_| RepositoryError::QueueClosed)?;
    outcome.map_err(RepositoryError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recover_if_corrupt_returns_false_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("absent.sqlite");
        assert!(!path.exists());
        let recovered = Repository::recover_if_corrupt(&path).unwrap();
        assert!(!recovered, "missing file needs no recovery");
    }

    #[test]
    fn recover_if_corrupt_returns_false_for_healthy_db() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("healthy.sqlite");
        // 先正常开库（建表 + 迁移），形成健康库
        let _repo = Repository::open(&path).unwrap();
        let backup = path.with_file_name("healthy.sqlite.corrupt.bak");
        assert!(!backup.exists(), "healthy db must not be backed up");

        let recovered = Repository::recover_if_corrupt(&path).unwrap();
        assert!(!recovered, "healthy db must report no recovery");
        assert!(!backup.exists(), "no .corrupt.bak must be created for healthy db");
    }

    #[test]
    fn recover_if_corrupt_backs_up_and_recovers_corrupt_db() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("broken.sqlite");
        // 写入非 SQLite 的垃圾字节，使其完整性校验失败
        std::fs::write(&path, b"this is not a sqlite database file at all").unwrap();
        let backup = path.with_file_name("broken.sqlite.corrupt.bak");

        let recovered = Repository::recover_if_corrupt(&path).unwrap();
        assert!(recovered, "corrupt db must be reported as recovered");
        assert!(backup.exists(), "corrupt db must be backed up to .corrupt.bak");
        assert!(!path.exists(), "corrupt db file must be removed before rebuild");

        // 重建后 open 必须成功（空库安全降级），且可被正常写入
        let repo = Repository::open(&path).unwrap();
        let applied = repo.applied_migrations().unwrap();
        assert!(!applied.is_empty(), "rebuilt db should have migrations applied");
    }
}
