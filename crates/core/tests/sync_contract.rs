//! 互通契约集成测试（ADR 0002）：跨设备判身份与判序所必需的地基——
//! 本机设备身份稳定可复用，参与同步的每张表都带修订号与来源设备，
//! 且「谁写的、第几版」由存储层统一打点，不依赖各个调用点自觉。

use std::path::Path;
use std::sync::Arc;

use insightforge_core::{
    AuditLogger, DraftsService, MemorySecretStore, Repository, Settings, ThoughtsService,
    UpsertThoughtCard, UreqAiHttpClient,
};

/// 参与同步的作者资产表。顺序即插入顺序：draft_reference 引用 draft，
/// coach_turn 引用 coach_session。
const SYNCED_TABLES: &[&str] = &[
    "draft",
    "draft_reference",
    "thought_card",
    "coach_session",
    "coach_turn",
    "ai_call_audit",
];

fn settings_in(dir: &Path) -> Settings {
    settings_from(&Arc::new(
        Repository::open(&dir.join("insightforge.sqlite")).unwrap(),
    ))
}

fn settings_from(repo: &Arc<Repository>) -> Settings {
    Settings::new(
        repo.clone(),
        Arc::new(MemorySecretStore::default()),
        Arc::new(UreqAiHttpClient::new()),
    )
}

fn columns(db_path: &Path, table: &str) -> Vec<String> {
    let conn = rusqlite::Connection::open(db_path).unwrap();
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .expect("pragma table_info");
    stmt.query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .map_while(Result::ok)
        .collect()
}

/// 各表的最小可插入行：主键统一用 `k1`，其余列只满足 NOT NULL。
fn seed_insert(table: &str) -> String {
    match table {
        "draft" => {
            "INSERT INTO draft (id, path, filename, baseline_hash, baseline_mtime_ms, \
                    baseline_size) VALUES ('k1', 'drafts/k1.md', 'k1.md', 'h', 1, 1)"
        }
        "draft_reference" => {
            "INSERT INTO draft_reference (id, draft_id, fragment_id, source_id, \
                    source_name, rel_path, heading_path, snippet, full_text, start_line, \
                    end_line) VALUES ('k1', 'k1', 1, 's1', '笔记', 'a.md', '', 's', 'f', 1, 1)"
        }
        "thought_card" => {
            "INSERT INTO thought_card (id, idea, source_id, source_name, rel_path, \
                    heading_path, draft_id, draft_filename) \
                    VALUES ('k1', 'i', 's1', '笔记', 'a.md', '', 'k1', 'k1.md')"
        }
        "coach_session" => {
            "INSERT INTO coach_session (id, draft_id, state, current_round, \
                    max_rounds) VALUES ('k1', 'k1', 'active', 1, 3)"
        }
        "coach_turn" => {
            "INSERT INTO coach_turn (id, session_id, draft_id, role, content, \
                    round_number) VALUES ('k1', 'k1', 'k1', 'user', 'c', 1)"
        }
        "ai_call_audit" => {
            "INSERT INTO ai_call_audit (id, model, fragment_count, fragment_ids, \
                    status) VALUES ('k1', 'm', 0, '[]', 'ok')"
        }
        other => panic!("{other} is not a synced table"),
    }
    .to_string()
}

/// 各表的一处普通列写入，用于验证「本机改动必然递增修订号」。
fn touch_update(table: &str) -> String {
    match table {
        "draft" => "UPDATE draft SET filename = 'renamed.md' WHERE id = 'k1'",
        "draft_reference" => "UPDATE draft_reference SET snippet = 's2' WHERE id = 'k1'",
        "thought_card" => "UPDATE thought_card SET idea = 'i2' WHERE id = 'k1'",
        "coach_session" => "UPDATE coach_session SET current_round = 2 WHERE id = 'k1'",
        "coach_turn" => "UPDATE coach_turn SET helpful = 1 WHERE id = 'k1'",
        "ai_call_audit" => "UPDATE ai_call_audit SET status = 'error' WHERE id = 'k1'",
        other => panic!("{other} is not a synced table"),
    }
    .to_string()
}

fn seed(repo: &Repository, table: &str) {
    let sql = seed_insert(table);
    repo.write(move |conn| conn.execute(&sql, []))
        .unwrap_or_else(|e| panic!("seed {table}: {e}"));
}

fn stamp(repo: &Repository, table: &str) -> (i64, Option<String>) {
    let sql = format!("SELECT rev, origin_device FROM {table} WHERE id = 'k1'");
    repo.read(move |conn| conn.query_row(&sql, [], |row| Ok((row.get(0)?, row.get(1)?))))
        .unwrap()
}

#[test]
fn device_id_is_created_once_and_reused_across_restarts() {
    let dir = tempfile::tempdir().unwrap();

    let first = settings_in(dir.path()).device_id().unwrap();
    // 重新打开库 = 重启应用：设备身份必须原样复用，否则历史修订会认不出主人。
    let after_restart = settings_in(dir.path()).device_id().unwrap();

    assert!(!first.is_empty());
    assert_eq!(first, after_restart);
}

#[test]
fn separate_databases_get_separate_device_ids() {
    let one = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();

    assert_ne!(
        settings_in(one.path()).device_id().unwrap(),
        settings_in(other.path()).device_id().unwrap()
    );
}

#[test]
fn every_synced_table_carries_revision_and_origin_device() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("insightforge.sqlite");
    let _repo = Repository::open(&db_path).unwrap();

    for table in SYNCED_TABLES {
        let cols = columns(&db_path, table);
        for required in ["rev", "origin_device"] {
            assert!(
                cols.iter().any(|name| name == required),
                "{table} must carry {required} for cross-device ordering, got {cols:?}"
            );
        }
    }
}

/// 本机每次写入作者资产都要自动留下「第几版、谁写的」，
/// 且不需要每个调用点各写一遍。
#[test]
fn each_local_write_bumps_revision_and_records_the_writing_device() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(Repository::open(&dir.path().join("insightforge.sqlite")).unwrap());
    let device = settings_from(&repo).device_id().unwrap();

    for table in SYNCED_TABLES {
        seed(&repo, table);
        assert_eq!(
            stamp(&repo, table),
            (1, Some(device.clone())),
            "a first local write to {table} must be revision 1 of this device"
        );

        let sql = touch_update(table);
        repo.write(move |conn| conn.execute(&sql, []))
            .unwrap_or_else(|e| panic!("touch {table}: {e}"));
        assert_eq!(
            stamp(&repo, table),
            (2, Some(device.clone())),
            "a second local write to {table} must be revision 2"
        );
    }
}

/// 拉取远端修订时要原样落地对方声明的版本号与来源设备，不能被本机再次递增。
#[test]
fn applying_a_remote_revision_keeps_the_number_the_sender_declared() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(Repository::open(&dir.path().join("insightforge.sqlite")).unwrap());
    settings_from(&repo).device_id().unwrap();
    seed(&repo, "draft");

    repo.write(|conn| {
        conn.execute(
            "UPDATE draft SET rev = 7, origin_device = 'other-device', filename = 'remote.md' \
             WHERE id = 'k1'",
            [],
        )
    })
    .unwrap();

    assert_eq!(
        stamp(&repo, "draft"),
        (7, Some("other-device".to_string())),
        "an explicitly declared remote revision must land untouched, or the two devices would \
         never agree on which write is newer"
    );
}

/// 逻辑身份必须取自「另一台设备撞不上」的空间。此前的 id = 名字哈希 + 本机时间，
/// 两台设备各自新建同名资产有可观察概率拿到同一身份，同步会把两条不同资产合成一条。
#[test]
fn author_asset_ids_come_from_a_uuid_space_other_devices_cannot_reach() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(Repository::open(&dir.path().join("insightforge.sqlite")).unwrap());

    let drafts = Arc::new(DraftsService::new(
        Arc::clone(&repo),
        dir.path().join("drafts"),
    ));
    drafts.ensure_directory().unwrap();
    let draft = drafts.create("untitled.md").unwrap();
    assert_uuid(&draft.id, "draft");

    let thoughts = ThoughtsService::new(Arc::clone(&repo), Arc::clone(&drafts));
    let card = thoughts
        .upsert(&UpsertThoughtCard {
            id: None,
            idea: "把索引当作可重建的投影".to_string(),
            source_id: "s1".to_string(),
            source_name: "笔记".to_string(),
            rel_path: "a.md".to_string(),
            heading_path: String::new(),
            draft_id: draft.id.clone(),
            draft_filename: "untitled.md".to_string(),
        })
        .unwrap();
    assert_uuid(&card.id, "thought_card");

    let audit = AuditLogger::new(Arc::clone(&repo))
        .log("m", 0, &[], "ok")
        .unwrap();
    assert_uuid(&audit.id, "ai_call_audit");
}

fn assert_uuid(id: &str, what: &str) {
    let parsed = uuid::Uuid::parse_str(id).unwrap_or_else(|e| {
        panic!("{what} id {id:?} must be a UUID other devices can't clash on: {e}")
    });
    assert_eq!(
        parsed.get_version_num(),
        4,
        "{what} id {id:?} must be version 4 (random), got version {:?}",
        parsed.get_version()
    );
}
