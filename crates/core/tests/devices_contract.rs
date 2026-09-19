//! 设备注册与撤销契约（ADR 0002 §3）。
//!
//! 判序链路上最贵的一种错，是「一台早该作废的设备还在往库里写修订」。
//! 这些测试钉住的就是那条边界：没登记过的不接受、撤销之后不可逆、
//! 撤销本身不破坏历史记录。

use std::sync::Arc;

use insightforge_core::{DeviceRegistry, DeviceStatus, DevicesError, Repository};
use rusqlite::params;

struct Fixture {
    /// 只为了持有生命周期：字段一丢，临时目录就被清掉。
    _dir: tempfile::TempDir,
    repo: Arc<Repository>,
    registry: Arc<DeviceRegistry>,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let repo = Arc::new(Repository::open(&dir.path().join("insightforge.sqlite")).unwrap());
        let registry = Arc::new(DeviceRegistry::new(Arc::clone(&repo)));
        Self {
            _dir: dir,
            repo,
            registry,
        }
    }

    /// 把一次心跳回拨到纪元起点，好让「再次登记会刷新 last_seen_at」可被观察：
    /// 两次 `now_iso()` 有可能落在同一微秒，靠真实时钟断言会变脆。
    fn rewind_last_seen(&self, id: &str) {
        let id = id.to_string();
        self.repo
            .write(move |conn| {
                conn.execute(
                    "UPDATE sync_device SET last_seen_at = ?2 WHERE id = ?1",
                    params![id, "1970-01-01T00:00:00.000000Z"],
                )?;
                Ok(())
            })
            .unwrap();
    }

    /// 把撤销时刻回拨到纪元起点：第二次撤销若改写了它，就说明幂等被破坏了。
    /// 靠真实时钟断言会因两次 `now_iso()` 落在同一微秒而变脆。
    fn rewind_revocation(&self, id: &str) {
        let id = id.to_string();
        self.repo
            .write(move |conn| {
                conn.execute(
                    "UPDATE sync_device SET revoked_at = ?2 WHERE id = ?1",
                    params![id, "1970-01-01T00:00:00.000000Z"],
                )?;
                Ok(())
            })
            .unwrap();
    }

    fn device_count(&self) -> i64 {
        self.repo
            .read(move |conn| {
                let count: i64 =
                    conn.query_row("SELECT COUNT(*) FROM sync_device", [], |row| row.get(0))?;
                Ok(count)
            })
            .unwrap()
    }
}

#[test]
fn a_registered_device_is_accepted_for_inbound_revisions() {
    let fx = Fixture::new();

    let device = fx.registry.register("device-a", "书房台式机").unwrap();

    assert_eq!(device.status, DeviceStatus::Active);
    assert_eq!(device.revoked_at, None);
    assert_eq!(device.first_seen_at, device.last_seen_at);
    assert!(fx.registry.accepts("device-a").unwrap());
}

#[test]
fn an_unregistered_device_is_never_accepted() {
    let fx = Fixture::new();

    fx.registry.register("device-a", "书房台式机").unwrap();

    assert!(!fx.registry.accepts("device-ghost").unwrap());
}

#[test]
fn registering_the_same_device_twice_keeps_one_row_and_its_first_seen() {
    let fx = Fixture::new();
    let first = fx.registry.register("device-a", "书房台式机").unwrap();
    fx.rewind_last_seen("device-a");

    let again = fx
        .registry
        .register("device-a", "书房台式机（改名）")
        .unwrap();

    assert_eq!(again.first_seen_at, first.first_seen_at);
    assert!(again.last_seen_at > first.last_seen_at);
    assert_eq!(again.label, "书房台式机（改名）");
    assert_eq!(again.status, DeviceStatus::Active);
    assert_eq!(fx.device_count(), 1);
}

#[test]
fn revoking_a_device_stops_its_revisions_being_accepted() {
    let fx = Fixture::new();
    fx.registry.register("device-a", "旧笔记本").unwrap();

    let revoked = fx.registry.revoke("device-a").unwrap();

    assert_eq!(revoked.status, DeviceStatus::Revoked);
    assert!(revoked.revoked_at.is_some());
    assert!(!fx.registry.accepts("device-a").unwrap());
}

#[test]
fn a_revoked_device_cannot_be_registered_again() {
    let fx = Fixture::new();
    fx.registry.register("device-a", "被盗用的身份").unwrap();
    fx.registry.revoke("device-a").unwrap();

    let err = fx
        .registry
        .register("device-a", "换一个标签就想复活")
        .unwrap_err();

    assert!(
        matches!(&err, DevicesError::Revoked(moved) if moved.as_str() == "device-a"),
        "expected a revoked device to refuse re-registration, got {err:?}"
    );
    assert!(!fx.registry.accepts("device-a").unwrap());
}

#[test]
fn revoking_twice_keeps_the_first_revocation_time() {
    let fx = Fixture::new();
    fx.registry.register("device-a", "旧笔记本").unwrap();
    let first = fx.registry.revoke("device-a").unwrap();
    fx.rewind_revocation("device-a");

    let again = fx.registry.revoke("device-a").unwrap();

    assert_eq!(
        again.revoked_at.as_deref(),
        Some("1970-01-01T00:00:00.000000Z"),
        "重复撤销把第一次的撤销时刻冲掉了"
    );
    assert_eq!(again.status, DeviceStatus::Revoked);
    assert_eq!(again.first_seen_at, first.first_seen_at);
}

#[test]
fn revoking_a_device_that_was_never_registered_reports_not_registered() {
    let fx = Fixture::new();

    let err = fx.registry.revoke("device-ghost").unwrap_err();

    assert!(
        matches!(&err, DevicesError::Unknown(missing) if missing.as_str() == "device-ghost"),
        "expected an unregistered device to be reported, got {err:?}"
    );
}

#[test]
fn the_registry_lists_every_device_including_revoked_ones() {
    let fx = Fixture::new();
    fx.registry.register("device-a", "书房台式机").unwrap();
    fx.registry.register("device-b", "旧笔记本").unwrap();
    fx.registry.revoke("device-b").unwrap();

    let devices = fx.registry.list().unwrap();

    let statuses: Vec<(&str, DeviceStatus)> = devices
        .iter()
        .map(|device| (device.id.as_str(), device.status))
        .collect();
    assert_eq!(
        statuses,
        vec![
            ("device-a", DeviceStatus::Active),
            ("device-b", DeviceStatus::Revoked),
        ]
    );
}

#[test]
fn registering_without_a_label_falls_back_to_the_device_id() {
    let fx = Fixture::new();

    let device = fx.registry.register("  device-a  ", "   ").unwrap();

    assert_eq!(device.id, "device-a");
    assert_eq!(device.label, "device-a");
}

#[test]
fn registering_without_an_id_is_refused() {
    let fx = Fixture::new();

    let err = fx.registry.register("   ", "书房台式机").unwrap_err();

    assert!(matches!(err, DevicesError::InvalidId), "got {err:?}");
    assert!(fx.registry.list().unwrap().is_empty());
}

#[test]
fn every_rejection_carries_a_stable_code_for_the_frontend_to_map() {
    let fx = Fixture::new();

    assert_eq!(DevicesError::InvalidId.code(), "invalid_device_id");
    assert_eq!(
        fx.registry.revoke("device-ghost").unwrap_err().code(),
        "device_not_registered"
    );

    fx.registry.register("device-a", "旧笔记本").unwrap();
    fx.registry.revoke("device-a").unwrap();
    assert_eq!(
        fx.registry.register("device-a", "标签").unwrap_err().code(),
        "device_revoked"
    );
}
