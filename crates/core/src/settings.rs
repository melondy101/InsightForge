//! Settings 服务（08 号票）：非敏感设置存 SQLite（经 repository 单写队列），
//! API Key 只进 OS 凭据库（secrets 模块），并提供 AI 总开关的守门语义。
//!
//! - AI base URL / model / 总开关是**非敏感配置**，存 `app_setting`（迁移 0004）。
//! - API Key 是**敏感凭据**：只经 [`SecretStore`] 写入系统凭据库；数据库与
//!   前端拿不到明文——快照里只有 `api_key_configured` 布尔位。
//! - 守门模式：AI 业务命令入口先查 [`Settings::ai_availability`]，不可用即返回
//!   [`SettingsError::AiUnavailable`]（09/10 号票复用此模式）。
//! - 连接测试经注入的 [`AiHttpClient`] 发一次最小探测（测试注入假 client，
//!   绝无真实网络）；它属于「配置阶段」动作，不受总开关限制，但要求配置齐备。

use std::sync::Arc;

use rusqlite::Connection;
use serde::Serialize;
use thiserror::Error;

use crate::http_client::{AiHttpClient, HttpRequest};
use crate::repository::Repository;
use crate::secrets::{SecretStore, SecretsError, API_KEY_ACCOUNT};

/// app_setting 表里 AI base URL 的键。
pub const AI_BASE_URL_KEY: &str = "ai.base_url";
/// app_setting 表里 AI model 的键。
pub const AI_MODEL_KEY: &str = "ai.model";
/// app_setting 表里 AI 总开关的键（值 "true"/"false"；缺省 = 关闭）。
pub const AI_ENABLED_KEY: &str = "ai.enabled";
/// app_setting 表里本机设备身份的键（ADR 0002：首次读取时生成并持久化）。
pub const DEVICE_ID_KEY: &str = "device.id";

/// 连接测试用的探测提示词（出站最小化：max_tokens = 1）。
const PROBE_PROMPT: &str = "ping";
const PROBE_MAX_TOKENS: u32 = 1;
const SAMPLE_MAX_TOKENS: u32 = 64;
const REQUEST_TIMEOUT_SECS: u64 = 15;
const DETAIL_EXCERPT_CHARS: usize = 200;

/// AI 设置快照（前端薄投影）：Key 永不出现明文。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    pub base_url: Option<String>,
    pub model: Option<String>,
    /// Key 是否已配置（明文不出库）。
    pub api_key_configured: bool,
    /// AI 总开关（缺省关闭：隐私优先，关闭时除 AI 外全功能可用）。
    pub enabled: bool,
}

/// AI 不可用的原因（结构化「不可用状态」，供错误码与测试锁定）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiUnavailableReason {
    /// 总开关关闭。
    Disabled,
    NoBaseUrl,
    NoModel,
    NoApiKey,
}

/// AI 可用性（守门查询结果）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AiAvailability {
    Available,
    Unavailable { reason: AiUnavailableReason },
}

/// 连接测试报告：命令本身成功执行，连接成败体现在 `ok` 上。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionReport {
    pub ok: bool,
    pub http_status: Option<u16>,
    /// 已脱敏的诊断信息（响应体摘要/失败原因），不含 Key 明文。
    pub detail: Option<String>,
}

/// 样例 AI 命令的回复（演示守门模式的最小出站调用）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSampleReply {
    pub model: String,
    pub content: String,
}

/// 一次守门后的补全结果：回复文本 + 实际使用的模型（供审计与界面陈列）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiCompletion {
    pub model: String,
    pub content: String,
}

#[derive(Debug, Error)]
pub enum SettingsError {
    /// AI 命令在总开关关闭或配置不齐时必须返回的明确不可用状态。
    #[error("AI is unavailable")]
    AiUnavailable { reason: AiUnavailableReason },
    #[error("invalid base url: {0}")]
    InvalidBaseUrl(String),
    #[error("api key must not be blank")]
    InvalidApiKey,
    #[error("AI endpoint returned HTTP {0}")]
    HttpStatus(u16),
    #[error("AI response could not be parsed")]
    InvalidResponse,
    #[error(transparent)]
    Secrets(#[from] SecretsError),
    #[error(transparent)]
    Repository(#[from] crate::repository::RepositoryError),
    #[error(transparent)]
    Client(#[from] crate::http_client::HttpClientError),
}

impl SettingsError {
    /// 稳定错误码，供前端按码做 i18n 映射。
    pub fn code(&self) -> &'static str {
        match self {
            SettingsError::AiUnavailable { reason } => match reason {
                AiUnavailableReason::Disabled => "ai_disabled",
                AiUnavailableReason::NoBaseUrl => "ai_no_base_url",
                AiUnavailableReason::NoModel => "ai_no_model",
                AiUnavailableReason::NoApiKey => "ai_no_api_key",
            },
            SettingsError::InvalidBaseUrl(_) => "invalid_base_url",
            SettingsError::InvalidApiKey => "invalid_api_key",
            SettingsError::HttpStatus(_) => "ai_http_status",
            SettingsError::InvalidResponse => "invalid_response",
            SettingsError::Secrets(_) => "secret_store_error",
            SettingsError::Repository(_) => "repository_error",
            SettingsError::Client(_) => "client_error",
        }
    }
}

/// 结构化变体按值比较；包装底层错误的变体按错误文本比较
/// （主要服务于测试断言「返回了哪个明确的不可用状态」）。
impl PartialEq for SettingsError {
    fn eq(&self, other: &Self) -> bool {
        use SettingsError::*;
        match (self, other) {
            (AiUnavailable { reason: a }, AiUnavailable { reason: b }) => a == b,
            (InvalidBaseUrl(a), InvalidBaseUrl(b)) => a == b,
            (InvalidApiKey, InvalidApiKey) => true,
            (HttpStatus(a), HttpStatus(b)) => a == b,
            (InvalidResponse, InvalidResponse) => true,
            (Secrets(a), Secrets(b)) => a.to_string() == b.to_string(),
            (Repository(a), Repository(b)) => a.to_string() == b.to_string(),
            (Client(a), Client(b)) => a.to_string() == b.to_string(),
            _ => false,
        }
    }
}

/// Settings 服务：克隆/共享经 `Arc`，全部方法 `&self`。
pub struct Settings {
    repo: Arc<Repository>,
    secrets: Arc<dyn SecretStore>,
    http: Arc<dyn AiHttpClient>,
}

impl Settings {
    pub fn new(
        repo: Arc<Repository>,
        secrets: Arc<dyn SecretStore>,
        http: Arc<dyn AiHttpClient>,
    ) -> Self {
        Self {
            repo,
            secrets,
            http,
        }
    }

    /// 本机设备身份（ADR 0002）：首次调用生成 UUIDv4 并落库，此后跨重启复用。
    ///
    /// 「读 → 缺失则写 → 回读」整体在一个 `BEGIN IMMEDIATE` 事务内完成，
    /// 因此即便两个调用方同时首次读取，最终也只有一个身份会留在库里。
    pub fn device_id(&self) -> Result<String, SettingsError> {
        let id = self.repo.write(|conn| {
            if get_setting(conn, DEVICE_ID_KEY)?.is_none() {
                set_setting(conn, DEVICE_ID_KEY, &uuid::Uuid::new_v4().to_string())?;
            }
            get_setting(conn, DEVICE_ID_KEY)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
        })?;
        Ok(id)
    }

    /// 读取 AI 设置快照（全新库：未配置、开关关闭）。
    pub fn ai(&self) -> Result<AiSettings, SettingsError> {
        let (base_url, model, enabled) = self.repo.read(read_ai_settings)?;
        let api_key_configured = self.key_configured()?;
        Ok(AiSettings {
            base_url,
            model,
            api_key_configured,
            enabled,
        })
    }

    /// 保存 base URL 与 model（`None` 或空白 = 清除；base URL 校验 http/https）。
    pub fn set_ai_config(
        &self,
        base_url: Option<String>,
        model: Option<String>,
    ) -> Result<AiSettings, SettingsError> {
        let base_url = normalize_text(base_url);
        if let Some(url) = &base_url {
            validate_base_url(url).map_err(SettingsError::InvalidBaseUrl)?;
        }
        let model = normalize_text(model);

        let base_key = AI_BASE_URL_KEY;
        let model_key = AI_MODEL_KEY;
        let base_row = base_url.clone();
        let model_row = model.clone();
        self.repo.write(move |conn| {
            match base_row {
                Some(value) => set_setting(conn, base_key, &value)?,
                None => delete_setting(conn, base_key)?,
            }
            match model_row {
                Some(value) => set_setting(conn, model_key, &value)?,
                None => delete_setting(conn, model_key)?,
            }
            Ok(())
        })?;
        self.ai()
    }

    /// 设置 AI 总开关。
    pub fn set_ai_enabled(&self, enabled: bool) -> Result<AiSettings, SettingsError> {
        let key = AI_ENABLED_KEY;
        let value = if enabled { "true" } else { "false" };
        self.repo.write(move |conn| set_setting(conn, key, value))?;
        self.ai()
    }

    /// 保存 API Key：只进凭据库，绝不落库、绝不回显。
    pub fn set_api_key(&self, api_key: &str) -> Result<AiSettings, SettingsError> {
        let trimmed = api_key.trim();
        if trimmed.is_empty() {
            return Err(SettingsError::InvalidApiKey);
        }
        self.secrets.set_secret(API_KEY_ACCOUNT, trimmed)?;
        self.ai()
    }

    /// 清除 API Key（幂等）。
    pub fn clear_api_key(&self) -> Result<AiSettings, SettingsError> {
        self.secrets.delete_secret(API_KEY_ACCOUNT)?;
        self.ai()
    }

    /// AI 可用性查询（守门入口，09/10 号票复用）。
    pub fn ai_availability(&self) -> Result<AiAvailability, SettingsError> {
        let ai = self.ai()?;
        Ok(ai_availability_of(&ai))
    }

    /// 样例 AI 命令：演示并锁定「总开关守门」语义——不可用时返回
    /// [`SettingsError::AiUnavailable`]；可用时经注入的 client 发一次最小补全。
    pub fn ai_sample(&self, prompt: &str) -> Result<AiSampleReply, SettingsError> {
        let done = self.complete(prompt, SAMPLE_MAX_TOKENS)?;
        Ok(AiSampleReply {
            model: done.model,
            content: done.content,
        })
    }

    /// 领域服务复用的「守门后一次补全」：端点、模型、Key 全部来自用户配置，
    /// 总开关关闭或配置不齐就明确报不可用——绝不退回到某个默认端点去顶替用户的选择。
    pub fn complete(&self, prompt: &str, max_tokens: u32) -> Result<AiCompletion, SettingsError> {
        let ai = self.ai()?;
        if let AiAvailability::Unavailable { reason } = ai_availability_of(&ai) {
            return Err(SettingsError::AiUnavailable { reason });
        }
        let base_url = ai.base_url.expect("available implies base url");
        let model = ai.model.expect("available implies model");
        let response = self.post_completion(&base_url, &model, prompt, max_tokens)?;
        Ok(AiCompletion {
            model,
            content: extract_content(&response.body)?,
        })
    }

    /// 连接测试：对当前配置发一次最小探测（出站仅 `max_tokens=1` 的 ping）。
    /// 不受总开关限制（配置阶段动作），但要求 base/model/key 齐备。
    pub fn test_connection(&self) -> Result<TestConnectionReport, SettingsError> {
        let ai = self.ai()?;
        if ai.base_url.is_none() {
            return Err(SettingsError::AiUnavailable {
                reason: AiUnavailableReason::NoBaseUrl,
            });
        }
        if ai.model.is_none() {
            return Err(SettingsError::AiUnavailable {
                reason: AiUnavailableReason::NoModel,
            });
        }
        let api_key =
            self.secrets
                .get_secret(API_KEY_ACCOUNT)?
                .ok_or(SettingsError::AiUnavailable {
                    reason: AiUnavailableReason::NoApiKey,
                })?;
        let base_url = ai.base_url.expect("checked above");
        let model = ai.model.expect("checked above");

        let outcome = self.post_completion(&base_url, &model, PROBE_PROMPT, PROBE_MAX_TOKENS);
        let report = match outcome {
            Ok(response) => {
                let ok = response.status == 200;
                let detail = if ok {
                    None
                } else {
                    Some(sanitize(&excerpt(&response.body), &api_key))
                };
                TestConnectionReport {
                    ok,
                    http_status: Some(response.status),
                    detail,
                }
            }
            Err(err) => TestConnectionReport {
                ok: false,
                http_status: None,
                detail: Some(sanitize(&err.to_string(), &api_key)),
            },
        };
        Ok(report)
    }

    /// 组装并发送一次 OpenAI 兼容补全请求（守门之后才可调用）。
    fn post_completion(
        &self,
        base_url: &str,
        model: &str,
        prompt: &str,
        max_tokens: u32,
    ) -> Result<crate::http_client::HttpResponse, SettingsError> {
        // Key 只组装进 Authorization 头；不进请求体、不进日志。
        let api_key = self
            .secrets
            .get_secret(API_KEY_ACCOUNT)?
            .unwrap_or_default();
        let request = HttpRequest {
            url: chat_completions_url(base_url),
            auth_header: format!("Bearer {api_key}"),
            body: chat_request_body(model, prompt, max_tokens),
            timeout_secs: REQUEST_TIMEOUT_SECS,
        };
        Ok(self.http.post_json(request)?)
    }

    fn key_configured(&self) -> Result<bool, SettingsError> {
        Ok(self.secrets.get_secret(API_KEY_ACCOUNT)?.is_some())
    }
}

/// 快照 → 可用性判定（纯函数，守门规则唯一出处）。
fn ai_availability_of(ai: &AiSettings) -> AiAvailability {
    if !ai.enabled {
        return AiAvailability::Unavailable {
            reason: AiUnavailableReason::Disabled,
        };
    }
    if ai.base_url.is_none() {
        return AiAvailability::Unavailable {
            reason: AiUnavailableReason::NoBaseUrl,
        };
    }
    if ai.model.is_none() {
        return AiAvailability::Unavailable {
            reason: AiUnavailableReason::NoModel,
        };
    }
    if !ai.api_key_configured {
        return AiAvailability::Unavailable {
            reason: AiUnavailableReason::NoApiKey,
        };
    }
    AiAvailability::Available
}

/// 规范化可空文本输入：trim；空白 = None（清除）。
fn normalize_text(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

/// base URL 校验：必须是 http(s) 绝对地址且无空白。
fn validate_base_url(url: &str) -> Result<(), String> {
    let ok = (url.starts_with("http://") || url.starts_with("https://"))
        && !url.contains(char::is_whitespace)
        && url.len() > "http://".len();
    if ok {
        Ok(())
    } else {
        Err(format!(
            "expected an absolute http(s) url without whitespace, got {url:?}"
        ))
    }
}

/// `{base}/chat/completions`（容忍 base 末尾多余的 `/`）。
fn chat_completions_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    format!("{trimmed}/chat/completions")
}

/// OpenAI 兼容补全请求体（测试断言其形状与最小化）。
fn chat_request_body(model: &str, prompt: &str, max_tokens: u32) -> String {
    serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "max_tokens": max_tokens,
    })
    .to_string()
}

/// 从 OpenAI 兼容响应提取 `choices[0].message.content`。
fn extract_content(body: &str) -> Result<String, SettingsError> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|_| SettingsError::InvalidResponse)?;
    value
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .map(str::to_string)
        .ok_or(SettingsError::InvalidResponse)
}

/// 诊断信息脱敏：把可能被端点/传输层回显的 Key 明文替换为 `***`。
fn sanitize(detail: &str, api_key: &str) -> String {
    if api_key.is_empty() {
        detail.to_string()
    } else {
        detail.replace(api_key, "***")
    }
}

/// 响应体摘要（截断，避免把大响应体灌进报告/日志）。
fn excerpt(text: &str) -> String {
    text.chars().take(DETAIL_EXCERPT_CHARS).collect()
}

// ---------- app_setting SQL（经 repository 单写队列执行） ----------

fn read_ai_settings(conn: &Connection) -> rusqlite::Result<(Option<String>, Option<String>, bool)> {
    let base_url = get_setting(conn, AI_BASE_URL_KEY)?;
    let model = get_setting(conn, AI_MODEL_KEY)?;
    let enabled = get_setting(conn, AI_ENABLED_KEY)?.is_some_and(|v| v == "true");
    Ok((base_url, model, enabled))
}

fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM app_setting WHERE key = ?1")?;
    let mut rows = stmt.query(rusqlite::params![key])?;
    match rows.next()? {
        Some(row) => Ok(Some(row.get(0)?)),
        None => Ok(None),
    }
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO app_setting (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

pub fn delete_setting(conn: &Connection, key: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM app_setting WHERE key = ?1",
        rusqlite::params![key],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_url_appends_chat_completions_and_tolerates_trailing_slash() {
        assert_eq!(
            chat_completions_url("https://api.example.com/v1"),
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("https://api.example.com/v1/"),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn base_url_validation_accepts_http_https_and_rejects_the_rest() {
        assert!(validate_base_url("http://127.0.0.1:18787/v1").is_ok());
        assert!(validate_base_url("https://api.openai.com/v1").is_ok());
        assert!(validate_base_url("api.openai.com/v1").is_err());
        assert!(validate_base_url("ftp://api.openai.com").is_err());
        assert!(validate_base_url("https://bad url").is_err());
        assert!(validate_base_url("http://").is_err());
    }

    #[test]
    fn chat_request_body_is_minimal_and_key_free() {
        let body = chat_request_body("m1", "ping", 1);
        let value: serde_json::Value = serde_json::from_str(&body).expect("json");
        assert_eq!(value["model"], "m1");
        assert_eq!(value["messages"][0]["content"], "ping");
        assert_eq!(value["max_tokens"], 1);
        assert!(!body.contains("sk-"), "请求体不含 Key");
    }

    #[test]
    fn sanitize_replaces_every_key_occurrence() {
        assert_eq!(
            sanitize("bad key sk-secret-1 and sk-secret-1 again", "sk-secret-1"),
            "bad key *** and *** again"
        );
        assert_eq!(sanitize("no key here", ""), "no key here");
    }

    #[test]
    fn excerpt_truncates_by_chars() {
        assert_eq!(excerpt("hello"), "hello");
        assert_eq!(excerpt(&"你好世界".repeat(100)).chars().count(), 200);
    }

    #[test]
    fn content_extraction_reads_choices_message() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"pong"}}]}"#;
        assert_eq!(extract_content(body).expect("content"), "pong");
        assert!(extract_content(r#"{"error":{"message":"nope"}}"#).is_err());
        assert!(extract_content("not json").is_err());
    }
}
