//! Repository 公共 API 集成测试：真实临时目录 + 真实 SQLite 文件，headless 调用。

use std::path::Path;
use std::sync::Arc;

use insightforge_core::{Migration, Migrator, Repository, RepositoryError};

#[test]
fn open_creates_sqlite_file_at_requested_path() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let db_path = dir.path().join("insightforge.sqlite");

    Repository::open(&db_path).expect("open should create the database file");

    assert!(
        Path::new(&db_path).exists(),
        "database file should exist at the requested path after open"
    );
}

#[test]
fn open_defaults_to_wal_journal_mode() {
    let dir = tempfile::tempdir().expect("temp dir should be created");

    let repo = Repository::open(&dir.path().join("insightforge.sqlite"))
        .expect("open should create the database file");

    assert_eq!(
        repo.journal_mode()
            .expect("journal_mode should be readable"),
        "wal",
        "repository must pin WAL journal mode on every open"
    );
}

#[test]
fn open_applies_pending_migrations_and_records_versions() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let db_path = dir.path().join("insightforge.sqlite");
    let migrations = [
        Migration {
            version: 1,
            name: "create_fixture_note",
            sql: "CREATE TABLE fixture_note (id INTEGER PRIMARY KEY)",
        },
        Migration {
            version: 2,
            name: "create_fixture_second",
            sql: "CREATE TABLE fixture_second (id INTEGER PRIMARY KEY)",
        },
    ];

    let repo =
        Repository::open_with_migrations(&db_path, &migrations).expect("open with migrations");

    assert_eq!(
        repo.applied_migrations()
            .expect("applied migrations readable"),
        vec![1, 2],
        "bookkeeping must record every applied migration version in order"
    );

    // 直接检查数据库最终状态：迁移 SQL 确实执行过（spec 允许断言数据库最终状态）。
    let inspector = rusqlite::Connection::open(&db_path).expect("re-open database file");
    let mut stmt = inspector
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .expect("query sqlite_master");
    let tables: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .expect("query_map tables")
        .map_while(Result::ok)
        .collect();
    assert!(tables.contains(&"fixture_note".to_string()));
    assert!(tables.contains(&"fixture_second".to_string()));
}

#[test]
fn reopening_database_does_not_reapply_finished_migrations() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let db_path = dir.path().join("insightforge.sqlite");
    // 迁移 SQL 故意不带 IF NOT EXISTS：若框架重复应用，第二次 open 必然报错。
    let migrations = [Migration {
        version: 1,
        name: "create_fixture_note",
        sql: "CREATE TABLE fixture_note (id INTEGER PRIMARY KEY)",
    }];

    let first =
        Repository::open_with_migrations(&db_path, &migrations).expect("first open applies it");
    drop(first);

    let second =
        Repository::open_with_migrations(&db_path, &migrations).expect("second open is idempotent");

    assert_eq!(
        second
            .applied_migrations()
            .expect("applied migrations readable"),
        vec![1],
        "reopening must keep the recorded versions unchanged"
    );
}

#[test]
fn migrator_rejects_migration_versions_that_do_not_strictly_increase() {
    let bad_sequence = [
        Migration {
            version: 2,
            name: "later_version",
            sql: "CREATE TABLE should_never_exist (id INTEGER PRIMARY KEY)",
        },
        Migration {
            version: 1,
            name: "earlier_version",
            sql: "CREATE TABLE should_never_exist_either (id INTEGER PRIMARY KEY)",
        },
    ];

    let error = Migrator::new(&bad_sequence).expect_err("decreasing versions must be rejected");

    assert!(
        error.to_string().contains("strictly increase"),
        "error should explain the violated ordering rule, got: {error}"
    );
}

#[test]
fn concurrent_writes_serialize_through_the_single_write_queue() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let db_path = dir.path().join("insightforge.sqlite");
    let migrations = [Migration {
        version: 1,
        name: "create_queue_probe",
        sql: "CREATE TABLE queue_probe (id INTEGER PRIMARY KEY, writer INTEGER NOT NULL)",
    }];
    let repo = Arc::new(
        Repository::open_with_migrations(&db_path, &migrations).expect("open with migrations"),
    );

    let writer_count = 8u64;
    let writes_per_writer = 25u64;
    let mut handles = Vec::new();
    for writer in 0..writer_count {
        let repo = Arc::clone(&repo);
        handles.push(std::thread::spawn(move || {
            for index in 0..writes_per_writer {
                let writer = i64::try_from(writer).expect("writer id fits in i64");
                repo.write(move |conn| {
                    conn.execute(
                        "INSERT INTO queue_probe (writer) VALUES (?1)",
                        rusqlite::params![writer],
                    )?;
                    Ok(index)
                })
                .expect("write through the queue should succeed");
            }
        }));
    }
    for handle in handles {
        handle.join().expect("writer thread should not panic");
    }

    let total: i64 = repo
        .read(|conn| conn.query_row("SELECT COUNT(*) FROM queue_probe", [], |row| row.get(0)))
        .expect("count read should succeed");
    assert_eq!(
        i64::try_from(writer_count * writes_per_writer).expect("total fits in i64"),
        total,
        "every queued write must land exactly once"
    );
}

#[test]
fn linked_sqlite_meets_the_specified_minimum_version() {
    // spec 铁律：必须使用 >= 3.51.3 的 SQLite（包含 2026 WAL-reset 修复）。
    assert!(
        Repository::sqlite_libversion_number() >= 3_051_003,
        "linked SQLite {} (version number {}) is below the required 3.51.3 (3051003)",
        Repository::sqlite_libversion(),
        Repository::sqlite_libversion_number()
    );
}

fn queue_probe_repository() -> (tempfile::TempDir, Repository) {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let db_path = dir.path().join("insightforge.sqlite");
    let migrations = [Migration {
        version: 1,
        name: "create_queue_probe",
        sql: "CREATE TABLE queue_probe (id INTEGER PRIMARY KEY, writer INTEGER NOT NULL)",
    }];
    let repo = Repository::open_with_migrations(&db_path, &migrations).expect("open");
    (dir, repo)
}

#[test]
fn failed_write_rolls_back_partial_work() {
    let (_dir, repo) = queue_probe_repository();

    let error = repo
        .write(|conn| {
            conn.execute(
                "INSERT INTO queue_probe (writer) VALUES (1)",
                rusqlite::params![],
            )?;
            // 写了一半之后用一条必然失败的 SQL 触发写错误。
            conn.query_row("SELECT id FROM missing_table", [], |row| {
                row.get::<_, i64>(0)
            })?;
            Ok(())
        })
        .expect_err("a write that fails midway must surface the error");

    assert!(
        matches!(error, RepositoryError::Sqlite(_)),
        "underlying sqlite error should surface, got: {error}"
    );

    let committed: i64 = repo
        .read(|conn| conn.query_row("SELECT COUNT(*) FROM queue_probe", [], |row| row.get(0)))
        .expect("count read should succeed");
    assert_eq!(
        committed, 0,
        "partial work of a failed write must roll back"
    );
}

#[test]
fn queue_keeps_accepting_writes_after_a_failed_write() {
    let (_dir, repo) = queue_probe_repository();

    repo.write(|conn| {
        conn.execute(
            "INSERT INTO queue_probe (writer) VALUES (1)",
            rusqlite::params![],
        )?;
        conn.query_row("SELECT id FROM missing_table", [], |row| {
            row.get::<_, i64>(0)
        })?;
        Ok(())
    })
    .expect_err("first write fails as designed");

    let writer = 2_i64;
    repo.write(move |conn| {
        conn.execute(
            "INSERT INTO queue_probe (writer) VALUES (?1)",
            rusqlite::params![writer],
        )?;
        Ok(())
    })
    .expect("queue must stay usable after a failed write");

    let committed: i64 = repo
        .read(|conn| conn.query_row("SELECT COUNT(*) FROM queue_probe", [], |row| row.get(0)))
        .expect("count read should succeed");
    assert_eq!(
        committed, 1,
        "only the successful write should be committed"
    );
}
