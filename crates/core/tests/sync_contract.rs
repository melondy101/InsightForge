//! 互通契约集成测试（ADR 0002）：跨设备判身份与判序所必需的两块地基——
//! 本机设备身份稳定可复用，以及参与同步的每张表都带修订号与来源设备。

use std::path::Path;
use std::sync::Arc;

use insightforge_core::{MemorySecretStore, Repository, Settings, UreqAiHttpClient};

fn settings_in(dir: &Path) -> Settings {
    let repo = Arc::new(Repository::open(&dir.join("insightforge.sqlite")).unwrap());
    Settings::new(
        repo,
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

    for table in [
        "draft",
        "draft_reference",
        "thought_card",
        "coach_session",
        "coach_turn",
        "ai_call_audit",
    ] {
        let cols = columns(&db_path, table);
        for required in ["rev", "origin_device"] {
            assert!(
                cols.iter().any(|name| name == required),
                "{table} must carry {required} for cross-device ordering, got {cols:?}"
            );
        }
    }
}
