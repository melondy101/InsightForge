//! Secrets 模块公共接口测试（08 号票）：
//! 凭据适配 seam 的测试不依赖真实凭据库——默认实现（Windows Credential
//! Manager）之外，任何 `SecretStore` 实现都必须满足这里的注入契约。
//! 真实 keyring 冒烟验证见模块内 env 门控测试（`IF_KEYRING_SMOKE=1`）。

use std::sync::Arc;

use insightforge_core::{MemorySecretStore, SecretStore, SecretsError, SERVICE_NAME};

const TEST_ACCOUNT: &str = "openai-compat.api-key";

fn store() -> Arc<dyn SecretStore> {
    Arc::new(MemorySecretStore::new())
}

#[test]
fn set_then_get_returns_the_same_secret() {
    let store = store();
    store
        .set_secret(TEST_ACCOUNT, "sk-test-secret-123")
        .expect("set should succeed");
    assert_eq!(
        store.get_secret(TEST_ACCOUNT).expect("get should succeed"),
        Some("sk-test-secret-123".to_string()),
        "读回的凭据必须与写入的一致"
    );
}

#[test]
fn set_overwrites_the_previous_secret() {
    let store = store();
    store.set_secret(TEST_ACCOUNT, "first").expect("first set");
    store
        .set_secret(TEST_ACCOUNT, "second")
        .expect("overwrite set");
    assert_eq!(
        store.get_secret(TEST_ACCOUNT).expect("get"),
        Some("second".to_string()),
        "重复写入必须覆盖旧值"
    );
}

#[test]
fn get_on_missing_account_returns_none() {
    let store = store();
    assert_eq!(
        store.get_secret(TEST_ACCOUNT).expect("get should succeed"),
        None,
        "未写入的账户读回必须是 None"
    );
}

#[test]
fn delete_removes_the_secret() {
    let store = store();
    store.set_secret(TEST_ACCOUNT, "sk-gone").expect("set");
    store.delete_secret(TEST_ACCOUNT).expect("delete");
    assert_eq!(
        store.get_secret(TEST_ACCOUNT).expect("get"),
        None,
        "删除后读回必须是 None"
    );
}

#[test]
fn delete_on_missing_account_is_idempotent() {
    let store = store();
    store
        .delete_secret(TEST_ACCOUNT)
        .expect("delete of a missing account must not fail");
}

#[test]
fn accounts_are_isolated_from_each_other() {
    let store = store();
    store.set_secret("account-a", "secret-a").expect("set a");
    store.set_secret("account-b", "secret-b").expect("set b");
    assert_eq!(
        store.get_secret("account-a").expect("get a"),
        Some("secret-a".to_string())
    );
    assert_eq!(
        store.get_secret("account-b").expect("get b"),
        Some("secret-b".to_string())
    );
}

#[test]
fn service_name_convention_is_stable() {
    // 服务名/账户名是跨版本持久化的约定（Windows Credential Manager 里可见），
    // 改动会让已保存的凭据「失联」——这里锁定它。
    assert_eq!(SERVICE_NAME, "insightforge");
}

#[test]
fn secrets_error_maps_to_stable_code() {
    let err = SecretsError::Operation("boom".to_string());
    assert_eq!(err.code(), "secret_store_error");
    let err = SecretsError::PlatformUnsupported;
    assert_eq!(err.code(), "secret_store_error");
}
