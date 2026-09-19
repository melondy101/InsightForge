//! 设备注册表（ADR 0002 §3）。
//!
//! 跨设备判序的前提是「这台设备还作数吗」：入站修订带着 `origin_device`，
//! 服务端要先回答接受还是拒绝。两条规则钉在这里，而不是散在各个调用点：
//! 未注册的设备不接受；撤销不可逆——被盗用的设备身份不能靠重新注册复活。

use std::sync::Arc;

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use thiserror::Error;

use crate::clock::now_iso;
use crate::repository::Repository;

/// 一台登记过的设备。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// 该设备的 `app_setting['device.id']`（UUIDv4，跨设备唯一）。
    pub id: String,
    pub label: String,
    pub status: DeviceStatus,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceStatus {
    Active,
    Revoked,
}

#[derive(Debug, Error)]
pub enum DevicesError {
    #[error("repository error: {0}")]
    Repository(#[from] crate::repository::RepositoryError),
    #[error("device {0} is revoked and cannot be re-registered")]
    Revoked(String),
    #[error("device {0} is not registered")]
    Unknown(String),
    #[error("device id must not be empty")]
    InvalidId,
}

impl DevicesError {
    /// 稳定错误码：前端据此走 i18n 映射，不展示 `to_string()` 的英文诊断。
    pub fn code(&self) -> &'static str {
        match self {
            Self::Revoked(_) => "device_revoked",
            Self::Unknown(_) => "device_not_registered",
            Self::InvalidId => "invalid_device_id",
            Self::Repository(_) => "repository_error",
        }
    }
}

pub struct DeviceRegistry {
    repo: Arc<Repository>,
}

impl DeviceRegistry {
    pub fn new(repo: Arc<Repository>) -> Self {
        Self { repo }
    }

    /// 登记一台设备：新设备落地为 active；已 active 的设备只刷新 `last_seen_at`
    /// 与标签；已撤销的设备一律拒绝，返回 [`DevicesError::Revoked`]。
    pub fn register(&self, id: &str, label: &str) -> Result<Device, DevicesError> {
        let id = id.trim().to_string();
        if id.is_empty() {
            return Err(DevicesError::InvalidId);
        }
        let label = match label.trim() {
            // 空标签回落到设备 id：列表里不出现空白行，核心也不替界面发明文案。
            "" => id.clone(),
            text => text.to_string(),
        };
        let now = now_iso();
        // 写入闭包必须持有所有权（单写队列要求 'static），拒绝理由里再要用一次 id，所以先留一份。
        let rejected_id = id.clone();

        let registered: Option<Device> = self.repo.write(move |conn| {
            let id: &str = &id;
            let label: &str = &label;
            let status: Option<String> = conn
                .query_row(
                    "SELECT status FROM sync_device WHERE id = ?1",
                    params![id],
                    |row| row.get(0),
                )
                .optional()?;
            match status.as_deref() {
                Some("revoked") => return Ok(None),
                Some(_) => {
                    conn.execute(
                        "UPDATE sync_device SET label = ?2, last_seen_at = ?3 WHERE id = ?1",
                        params![id, label, now],
                    )?;
                }
                None => {
                    conn.execute(
                        "INSERT INTO sync_device
                             (id, label, status, first_seen_at, last_seen_at)
                         VALUES (?1, ?2, 'active', ?3, ?3)",
                        params![id, label, now],
                    )?;
                }
            }
            device_by_id(conn, id)
        })?;

        registered.ok_or(DevicesError::Revoked(rejected_id))
    }

    /// 撤销一台设备。重复撤销幂等：已撤销的行不会被再次改写，
    /// 所以 `revoked_at` 保留第一次撤销的时刻；从没登记过的设备报 [`DevicesError::Unknown`]。
    pub fn revoke(&self, id: &str) -> Result<Device, DevicesError> {
        let id = id.trim().to_string();
        let unknown_id = id.clone();
        let now = now_iso();
        let device = self.repo.write(move |conn| {
            let id: &str = &id;
            conn.execute(
                "UPDATE sync_device
                 SET status = 'revoked', revoked_at = ?2
                 WHERE id = ?1 AND status <> 'revoked'",
                params![id, now],
            )?;
            device_by_id(conn, id)
        })?;
        device.ok_or(DevicesError::Unknown(unknown_id))
    }

    /// 列出全部登记（含已撤销），供「设备管理」界面陈列。
    pub fn list(&self) -> Result<Vec<Device>, DevicesError> {
        let devices = self.repo.read(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, label, status, first_seen_at, last_seen_at, revoked_at
                 FROM sync_device
                 ORDER BY first_seen_at, id",
            )?;
            let mut rows = stmt.query([])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                out.push(read_device(row)?);
            }
            Ok(out)
        })?;
        Ok(devices)
    }

    /// 入站修订可否落地：未注册与已撤销都拒绝。
    pub fn accepts(&self, id: &str) -> Result<bool, DevicesError> {
        // 与 register/revoke 同样去空白：带空白的同一个 id 不能被当成两台设备。
        let id = id.trim().to_string();
        let status = self.repo.read(move |conn| {
            conn.query_row(
                "SELECT status FROM sync_device WHERE id = ?1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()
        })?;
        Ok(matches!(status.as_deref(), Some("active")))
    }
}

fn device_by_id(conn: &Connection, id: &str) -> Result<Option<Device>, rusqlite::Error> {
    conn.query_row(
        "SELECT id, label, status, first_seen_at, last_seen_at, revoked_at
         FROM sync_device
         WHERE id = ?1",
        params![id],
        read_device,
    )
    .optional()
}

fn read_device(row: &Row<'_>) -> Result<Device, rusqlite::Error> {
    let status: String = row.get(2)?;
    Ok(Device {
        id: row.get(0)?,
        label: row.get(1)?,
        status: match status.as_str() {
            "revoked" => DeviceStatus::Revoked,
            _ => DeviceStatus::Active,
        },
        first_seen_at: row.get(3)?,
        last_seen_at: row.get(4)?,
        revoked_at: row.get(5)?,
    })
}
