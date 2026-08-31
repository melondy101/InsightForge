//! 02 号票领域 schema 集成测试：迁移 0001–0003 应在 `Repository::open` 时建立
//! 来源/源文件/片段表与 trigram FTS 索引（断言数据库最终状态）。

use insightforge_core::Repository;

fn open_repo() -> (tempfile::TempDir, Repository) {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let repo = Repository::open(&dir.path().join("insightforge.sqlite"))
        .expect("open should create and migrate the database");
    (dir, repo)
}

fn table_names(db_path: &std::path::Path) -> Vec<String> {
    let inspector = rusqlite::Connection::open(db_path).expect("re-open database file");
    let mut stmt = inspector
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .expect("query sqlite_master");
    stmt.query_map([], |row| row.get(0))
        .expect("query_map tables")
        .map_while(Result::ok)
        .collect()
}

#[test]
fn open_applies_ticket02_migrations_and_creates_domain_tables() {
    let (dir, repo) = open_repo();

    assert_eq!(
        repo.applied_migrations()
            .expect("applied migrations readable"),
        // 02 号票拥有 0001–0003；0004 为 08 号票经主控授权新增（app_setting）；
        // 0010 为 06 号票新增（draft 表）；0011 为 07 号票新增（draft_reference 表）。
        vec![1, 2, 3, 4, 10, 11],
        "migrations must apply in strict order (02: 0001-0003, 08: 0004, 06: 0010, 07: 0011)"
    );

    let tables = table_names(&dir.path().join("insightforge.sqlite"));
    for expected in [
        "knowledge_source",
        "source_file",
        "fragment",
        "fragment_fts",
    ] {
        assert!(
            tables.iter().any(|name| name == expected),
            "table {expected} must exist after open, got {tables:?}"
        );
    }
}

#[test]
fn source_file_schema_keeps_fields_for_file_identity_and_status() {
    let (dir, _repo) = open_repo();

    let inspector = rusqlite::Connection::open(dir.path().join("insightforge.sqlite"))
        .expect("re-open database file");
    let mut stmt = inspector
        .prepare("PRAGMA table_info(source_file)")
        .expect("pragma table_info");
    let columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .expect("query_map columns")
        .map_while(Result::ok)
        .collect();

    for expected in [
        "rel_path",
        "file_identity",
        "content_hash",
        "size",
        "mtime_ms",
        "status",
    ] {
        assert!(
            columns.iter().any(|name| name == expected),
            "source_file must reserve column {expected} for file identity + state, got {columns:?}"
        );
    }
}

#[test]
fn fragment_fts_trigram_tokenizer_matches_chinese_queries() {
    let (_dir, repo) = open_repo();

    repo.write(|conn| {
        conn.execute(
            "INSERT INTO knowledge_source (id, name, root_path) VALUES ('s1', '笔记', 'C:\\notes')",
            rusqlite::params![],
        )?;
        conn.execute(
            "INSERT INTO source_file (id, source_id, rel_path, content_hash, size, mtime_ms)
             VALUES (1, 's1', 'a.md', 'hash', 10, 1)",
            rusqlite::params![],
        )?;
        conn.execute(
            "INSERT INTO fragment (id, source_file_id, ordinal, heading_path, start_line, end_line, text, content_hash)
             VALUES (10, 1, 0, '', 1, 1, '本地优先的中文检索知识库', 'hash-text')",
            rusqlite::params![],
        )?;
        conn.execute(
            "INSERT INTO fragment_fts (fragment_id, text) VALUES (10, '本地优先的中文检索知识库')",
            rusqlite::params![],
        )?;
        Ok(())
    })
    .expect("seeding fragment + fts rows through the write queue should succeed");

    let hits: Vec<i64> = repo
        .read(|conn| {
            let mut stmt =
                conn.prepare("SELECT fragment_id FROM fragment_fts WHERE fragment_fts MATCH ?1")?;
            let ids = stmt
                .query_map(rusqlite::params!["中文检索"], |row| row.get(0))?
                .map_while(Result::ok)
                .collect();
            Ok(ids)
        })
        .expect("trigram FTS query should succeed");

    assert_eq!(
        hits,
        vec![10],
        "trigram tokenizer must match a Chinese query of 4 characters"
    );
}
