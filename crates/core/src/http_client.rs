//! AI HTTP client seam（08 号票）：对 OpenAI 兼容端点的最小传输层抽象。
//!
//! - [`AiHttpClient`] 是可注入 seam：测试注入假实现，**绝不真发网络请求**；
//!   [`UreqAiHttpClient`] 是默认真实实现（`ureq`，阻塞式，适合命令路径上
//!   用户显式触发的一次性调用）。
//! - Key 明文只以组装好的 `Authorization` 头进入本层；错误信息不得包含头内容。
//! - 仅传输层语义：非 2xx 不是 Err（状态码在 [`HttpResponse`] 里交给上层
//!   做策略判断），连接/超时等传输层失败才是 Err。

use std::time::Duration;

use thiserror::Error;

/// 一次 OpenAI 兼容 JSON POST 请求（seam 形状：测试断言 client 收到的就是它）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    /// 完整 URL（如 `{base_url}/chat/completions`）。
    pub url: String,
    /// 组装好的 Authorization 头值（`Bearer <key>`）。明文 Key 只在此出现。
    pub auth_header: String,
    /// 请求体（JSON 字符串，由上层拼装——传输层不关心业务形状）。
    pub body: String,
    /// 显式超时（秒）。
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP 状态码（非 2xx 也不是传输层错误）。
    pub status: u16,
    /// 原始响应体文本。
    pub body: String,
}

#[derive(Debug, Error)]
pub enum HttpClientError {
    #[error("request timed out")]
    Timeout,
    #[error("transport failure: {0}")]
    Transport(String),
}

pub trait AiHttpClient: Send + Sync {
    fn post_json(&self, request: HttpRequest) -> Result<HttpResponse, HttpClientError>;
}

/// 默认真实实现：`ureq`（rustls/ring TLS，webpki 根证书）。
/// 每次请求按 `timeout_secs` 建独立 Agent（调用频率为用户显式触发的一次性动作）。
#[derive(Debug, Clone)]
pub struct UreqAiHttpClient;

impl UreqAiHttpClient {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UreqAiHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AiHttpClient for UreqAiHttpClient {
    fn post_json(&self, request: HttpRequest) -> Result<HttpResponse, HttpClientError> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(request.timeout_secs)))
            .http_status_as_error(false)
            .build()
            .into();
        let mut response = agent
            .post(&request.url)
            .header("Authorization", &request.auth_header)
            .header("Content-Type", "application/json")
            .send(request.body.as_str())
            .map_err(|err| match err {
                ureq::Error::Timeout(_) => HttpClientError::Timeout,
                other => HttpClientError::Transport(other.to_string()),
            })?;
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .read_to_string()
            .map_err(|err| HttpClientError::Transport(err.to_string()))?;
        Ok(HttpResponse { status, body })
    }
}
