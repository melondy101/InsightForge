//! Secrets 模块（08 号票）：BYOK API Key 只进 OS 凭据库，不进 SQLite、不进前端。
//!
//! - [`SecretStore`] 是凭据适配 seam：[`MemorySecretStore`] 供测试注入
//!   （不依赖真实凭据库），[`KeyringStore`] 是默认实现——经 `keyring`
//!   crate 走 Windows Credential Manager（macOS Keychain / Linux Secret
//!   Service 为后续票据，非 Windows 平台编译时显式返回不支持）。
//! - 约定（跨版本持久化，不可轻改）：service = [`SERVICE_NAME`]，
//!   08 号票唯一账户 = [`API_KEY_ACCOUNT`]（OpenAI 兼容 API Key）。
//! - Tauri Stronghold 是加密 vault，不等同 OS 凭据库，不混用（ADR/ARCHITECTURE §7）。

use std::collections::HashMap;
use std::sync::Mutex;

use thiserror::Error;

/// 凭据库服务名：Windows Credential Manager 里按它分组。
pub const SERVICE_NAME: &str = "insightforge";

/// OpenAI 兼容 API Key 的账户名（08 号票唯一凭据条目）。
pub const API_KEY_ACCOUNT: &str = "openai-compat.api-key";

#[derive(Debug, Error)]
pub enum SecretsError {
    #[error("secret store is not supported on this platform")]
    PlatformUnsupported,
    #[error("secret store operation failed: {0}")]
    Operation(String),
}

impl SecretsError {
    /// 稳定错误码，供前端按码做 i18n 映射。
    pub fn code(&self) -> &'static str {
        match self {
            SecretsError::PlatformUnsupported | SecretsError::Operation(_) => "secret_store_error",
        }
    }
}

/// 凭据适配 seam：按账户名写入 / 读取 / 删除一个文本凭据。
///
/// 实现必须满足 `crates/core/tests/secrets.rs` 的注入契约
/// （覆盖语义、幂等删除、缺失读 None）。注意：明文只经此 trait
/// 边界传递，实现不得把明文写进日志或错误信息。
pub trait SecretStore: Send + Sync {
    fn set_secret(&self, account: &str, secret: &str) -> Result<(), SecretsError>;
    /// 账户不存在时返回 `Ok(None)`，而不是错误。
    fn get_secret(&self, account: &str) -> Result<Option<String>, SecretsError>;
    /// 删除不存在的账户是幂等成功。
    fn delete_secret(&self, account: &str) -> Result<(), SecretsError>;
}

/// 注入用内存实现：集成测试与后续 AI 票据（09/10）的测试替身，
/// 绝不落盘。也用于「无凭据库环境」下的显式失败替换。
#[derive(Debug, Default)]
pub struct MemorySecretStore {
    entries: Mutex<HashMap<String, String>>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemorySecretStore {
    fn set_secret(&self, account: &str, secret: &str) -> Result<(), SecretsError> {
        self.entries
            .lock()
            .expect("memory secret store lock")
            .insert(account.to_string(), secret.to_string());
        Ok(())
    }

    fn get_secret(&self, account: &str) -> Result<Option<String>, SecretsError> {
        Ok(self
            .entries
            .lock()
            .expect("memory secret store lock")
            .get(account)
            .cloned())
    }

    fn delete_secret(&self, account: &str) -> Result<(), SecretsError> {
        self.entries
            .lock()
            .expect("memory secret store lock")
            .remove(account);
        Ok(())
    }
}

/// 默认实现：`keyring` crate → Windows Credential Manager。
/// service = [`SERVICE_NAME`]，账户由调用方给定（见 [`API_KEY_ACCOUNT`]）。
#[derive(Debug, Clone)]
pub struct KeyringStore {
    service: &'static str,
}

impl KeyringStore {
    pub fn new() -> Self {
        Self {
            service: SERVICE_NAME,
        }
    }
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
mod backend {
    use keyring::{self, Entry};

    use super::{KeyringStore, SecretStore, SecretsError};

    fn fail(step: &str, err: keyring::Error) -> SecretsError {
        // 只记录操作名与错误摘要；凭据明文永远不进入错误信息。
        SecretsError::Operation(format!("{step}: {err}"))
    }

    impl KeyringStore {
        fn entry(&self, account: &str) -> Result<Entry, SecretsError> {
            Entry::new(self.service, account).map_err(|err| fail("open entry", err))
        }
    }

    impl SecretStore for KeyringStore {
        fn set_secret(&self, account: &str, secret: &str) -> Result<(), SecretsError> {
            self.entry(account)?
                .set_password(secret)
                .map_err(|err| fail("set", err))
        }

        fn get_secret(&self, account: &str) -> Result<Option<String>, SecretsError> {
            match self.entry(account)?.get_password() {
                Ok(secret) => Ok(Some(secret)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(err) => Err(fail("get", err)),
            }
        }

        fn delete_secret(&self, account: &str) -> Result<(), SecretsError> {
            match self.entry(account)?.delete_credential() {
                Ok(()) => Ok(()),
                Err(keyring::Error::NoEntry) => Ok(()), // 幂等删除
                Err(err) => Err(fail("delete", err)),
            }
        }
    }
}

#[cfg(not(windows))]
mod backend {
    use super::{KeyringStore, SecretStore, SecretsError};

    // Windows 首发期间，其他平台编译可用但显式拒绝运行，
    // 绝不静默退化到任何明文存储。
    impl SecretStore for KeyringStore {
        fn set_secret(&self, _account: &str, _secret: &str) -> Result<(), SecretsError> {
            Err(SecretsError::PlatformUnsupported)
        }

        fn get_secret(&self, _account: &str) -> Result<Option<String>, SecretsError> {
            Err(SecretsError::PlatformUnsupported)
        }

        fn delete_secret(&self, _account: &str) -> Result<(), SecretsError> {
            Err(SecretsError::PlatformUnsupported)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真实 keyring 冒烟验证（可选）：默认跳过，`IF_KEYRING_SMOKE=1` 时
    /// 对真实 Windows Credential Manager 做一次 set/get/delete 往返
    /// （独立冒烟账户，结束即清理，不触碰真实用户凭据）。
    #[test]
    fn real_keyring_smoke_when_env_requested() {
        if std::env::var("IF_KEYRING_SMOKE").as_deref() != Ok("1") {
            eprintln!("SKIP: set IF_KEYRING_SMOKE=1 to run the real keyring smoke test");
            return;
        }
        let store = KeyringStore::new();
        let account = "smoke.ticket08";
        store
            .set_secret(account, "sk-test-fake-smoke")
            .expect("smoke set should succeed against the real credential store");
        assert_eq!(
            store.get_secret(account).expect("smoke get"),
            Some("sk-test-fake-smoke".to_string())
        );
        store.delete_secret(account).expect("smoke delete");
        assert_eq!(
            store.get_secret(account).expect("smoke get after delete"),
            None
        );
    }
}
