//! Settings 公共命令 seam 集成测试（08 号票，spec §Testing Decisions）：
//! 真实临时目录 + 真实 SQLite 文件 + 注入的内存假 SecretStore + 注入的假
//! HTTP client（绝无真实网络），只断言公共接口可观察的行为：
//! 配置读写往返、Key 只进凭据库（数据库字节级无明文）、连接测试请求形状、
//! AI 总开关守门语义、以及无 AI 配置时其余功能不受影响。

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use insightforge_core::{
    AiAvailability, AiSettings, AiUnavailableReason, HttpClientError, HttpRequest, HttpResponse,
    MemorySecretStore, Repository, SecretStore, Settings, SettingsError, SourceStatus, Sources,
    API_KEY_ACCOUNT,
};

const FAKE_KEY: &str = "sk-test-secret-123";
const FAKE_BASE: &str = "https://api.example.com/v1";
const FAKE_MODEL: &str = "test-model";

// ---------- 测试替身 ----------

#[derive(Clone)]
struct FakeHttp {
    /// 记录收到的每个请求（seam 断言点：client 收到的参数形状）。
    recorded: Arc<Mutex<Vec<HttpRequest>>>,
    /// 预置的响应队列；空了就回默认 200 pong。
    outcomes: Arc<Mutex<VecDeque<Result<HttpResponse, HttpClientError>>>>,
}

impl FakeHttp {
    fn new() -> Self {
        Self {
            recorded: Arc::new(Mutex::new(Vec::new())),
            outcomes: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn enqueue(&self, outcome: Result<HttpResponse, HttpClientError>) {
        self.outcomes.lock().unwrap().push_back(outcome);
    }

    fn requests(&self) -> Vec<HttpRequest> {
        self.recorded.lock().unwrap().clone()
    }
}

impl Default for FakeHttp {
    fn default() -> Self {
        Self::new()
    }
}

impl insightforge_core::AiHttpClient for FakeHttp {
    fn post_json(&self, request: HttpRequest) -> Result<HttpResponse, HttpClientError> {
        self.recorded.lock().unwrap().push(request);
        self.outcomes
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| {
                Ok(HttpResponse {
                    status: 200,
                    body: r#"{"choices":[{"message":{"role":"assistant","content":"fake-pong"}}]}"#
                        .to_string(),
                })
            })
    }
}

struct Harness {
    _dir: tempfile::TempDir,
    db_path: std::path::PathBuf,
    secrets: Arc<MemorySecretStore>,
    http: FakeHttp,
    settings: Settings,
}

fn harness() -> Harness {
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("insightforge.sqlite");
    let repo = Arc::new(Repository::open(&db_path).expect("open repository"));
    let secrets = Arc::new(MemorySecretStore::new());
    let http = FakeHttp::new();
    let settings = Settings::new(
        Arc::clone(&repo),
        Arc::clone(&secrets) as Arc<dyn SecretStore>,
        Arc::new(http.clone()),
    );
    Harness {
        _dir: dir,
        db_path,
        secrets,
        http,
        settings,
    }
}

impl Harness {
    /// 用注入的假 key/base/model 把 AI 配置齐备并打开总开关。
    fn configure_ai(&self) {
        self.settings
            .set_ai_config(Some(FAKE_BASE.to_string()), Some(FAKE_MODEL.to_string()))
            .expect("set config");
        self.settings.set_api_key(FAKE_KEY).expect("set api key");
        self.settings.set_ai_enabled(true).expect("enable");
    }
}

fn db_bytes(db_path: &Path) -> Vec<u8> {
    let mut bytes = std::fs::read(db_path).expect("read db file");
    for suffix in ["-wal", "-shm"] {
        let side = db_path.with_extension(format!("sqlite{suffix}"));
        if let Ok(mut side_bytes) = std::fs::read(&side) {
            bytes.append(&mut side_bytes);
        }
    }
    bytes
}

// ---------- 配置读写 ----------

#[test]
fn settings_roundtrip_persists_base_model_and_enabled_across_reopen() {
    let h = harness();
    h.settings
        .set_ai_config(Some(FAKE_BASE.to_string()), Some(FAKE_MODEL.to_string()))
        .expect("set config");
    h.settings.set_ai_enabled(true).expect("enable");

    // 模拟应用重启：同一个数据库文件重新打开。
    let repo = Arc::new(Repository::open(&h.db_path).expect("reopen"));
    let reopened = Settings::new(
        Arc::clone(&repo),
        Arc::new(MemorySecretStore::new()) as Arc<dyn SecretStore>,
        Arc::new(FakeHttp::default()),
    );
    let ai = reopened.ai().expect("read settings after reopen");
    assert_eq!(ai.base_url.as_deref(), Some(FAKE_BASE));
    assert_eq!(ai.model.as_deref(), Some(FAKE_MODEL));
    assert!(ai.enabled, "总开关状态必须随库持久化");
}

#[test]
fn set_ai_config_trims_values_and_clears_on_empty() {
    let h = harness();
    let ai = h
        .settings
        .set_ai_config(
            Some("  https://api.example.com/v1  ".to_string()),
            Some("  m1 ".to_string()),
        )
        .expect("set config");
    assert_eq!(ai.base_url.as_deref(), Some("https://api.example.com/v1"));
    assert_eq!(ai.model.as_deref(), Some("m1"));

    let ai = h
        .settings
        .set_ai_config(Some(String::new()), Some("   ".to_string()))
        .expect("clear config with blanks");
    assert_eq!(ai.base_url, None, "空白等价于清除");
    assert_eq!(ai.model, None);
}

#[test]
fn base_url_must_be_http_or_https() {
    let h = harness();
    for bad in [
        "api.example.com/v1",
        "ftp://api.example.com",
        "https://bad url",
    ] {
        let err = h
            .settings
            .set_ai_config(Some(bad.to_string()), None)
            .expect_err("scheme-less or malformed url must be rejected");
        assert!(
            matches!(err, SettingsError::InvalidBaseUrl(_)),
            "{bad} 应被判为 InvalidBaseUrl，实际 {err:?}"
        );
    }
}

// ---------- API Key 只进凭据库 ----------

#[test]
fn api_key_lands_in_secret_store_but_never_in_database_or_snapshot() {
    let h = harness();
    h.settings.set_api_key(FAKE_KEY).expect("set api key");

    assert_eq!(
        h.secrets.get_secret(API_KEY_ACCOUNT).expect("store get"),
        Some(FAKE_KEY.to_string()),
        "Key 必须进入 SecretStore"
    );

    let ai = h.settings.ai().expect("read settings");
    assert!(ai.api_key_configured, "快照必须显示已配置");
    let snapshot_json = serde_json::to_string(&ai).expect("serialize snapshot");
    assert!(
        !snapshot_json.contains(FAKE_KEY),
        "快照序列化结果不得包含 Key 明文: {snapshot_json}"
    );

    // 字节级断言：数据库与 WAL 侧车文件里都没有 Key 明文。
    let bytes = db_bytes(&h.db_path);
    let key_bytes = FAKE_KEY.as_bytes();
    assert!(
        bytes
            .windows(key_bytes.len())
            .all(|window| window != key_bytes),
        "数据库文件字节中不得出现 Key 明文"
    );
}

#[test]
fn api_key_set_overwrite_and_clear_lifecycle() {
    let h = harness();
    h.settings.set_api_key("sk-first").expect("first set");
    h.settings.set_api_key("sk-second").expect("overwrite");
    assert_eq!(
        h.secrets.get_secret(API_KEY_ACCOUNT).expect("store get"),
        Some("sk-second".to_string()),
        "重复保存必须覆盖"
    );

    h.settings.clear_api_key().expect("clear");
    assert_eq!(
        h.secrets.get_secret(API_KEY_ACCOUNT).expect("store get"),
        None
    );
    let ai = h.settings.ai().expect("read settings");
    assert!(!ai.api_key_configured, "清除后快照必须显示未配置");
}

#[test]
fn blank_api_key_is_rejected() {
    let h = harness();
    let err = h
        .settings
        .set_api_key("   ")
        .expect_err("blank key must be rejected");
    assert!(matches!(err, SettingsError::InvalidApiKey));
}

// ---------- AI 总开关守门 ----------

#[test]
fn ai_sample_is_gated_by_master_switch() {
    let h = harness();
    let err = h
        .settings
        .ai_sample("ping")
        .expect_err("switch off must gate the sample command");
    assert_eq!(
        err,
        SettingsError::AiUnavailable {
            reason: AiUnavailableReason::Disabled
        },
        "总开关关闭时 AI 命令必须返回明确的不可用状态"
    );

    h.configure_ai();
    let reply = h.settings.ai_sample("ping").expect("switch on must allow");
    assert_eq!(reply.model, FAKE_MODEL);
    assert_eq!(reply.content, "fake-pong");

    h.settings.set_ai_enabled(false).expect("disable again");
    let err = h
        .settings
        .ai_sample("ping")
        .expect_err("switching off must gate again");
    assert_eq!(
        err,
        SettingsError::AiUnavailable {
            reason: AiUnavailableReason::Disabled
        }
    );
}

#[test]
fn enabled_ai_names_the_first_missing_config() {
    let h = harness();
    h.settings.set_ai_enabled(true).expect("enable");

    let err = h.settings.ai_sample("ping").expect_err("no base url yet");
    assert_eq!(
        err,
        SettingsError::AiUnavailable {
            reason: AiUnavailableReason::NoBaseUrl
        }
    );

    h.settings
        .set_ai_config(Some(FAKE_BASE.to_string()), None)
        .expect("set base only");
    let err = h.settings.ai_sample("ping").expect_err("no model yet");
    assert_eq!(
        err,
        SettingsError::AiUnavailable {
            reason: AiUnavailableReason::NoModel
        }
    );

    h.settings
        .set_ai_config(Some(FAKE_BASE.to_string()), Some(FAKE_MODEL.to_string()))
        .expect("set base + model");
    let err = h.settings.ai_sample("ping").expect_err("no api key yet");
    assert_eq!(
        err,
        SettingsError::AiUnavailable {
            reason: AiUnavailableReason::NoApiKey
        }
    );

    h.settings.set_api_key(FAKE_KEY).expect("set key");
    assert!(
        h.settings.ai_sample("ping").is_ok(),
        "配置齐备后不再返回不可用"
    );
}

#[test]
fn ai_availability_query_reflects_switch_and_config() {
    let h = harness();
    assert_eq!(
        h.settings.ai_availability().expect("query"),
        AiAvailability::Unavailable {
            reason: AiUnavailableReason::Disabled
        }
    );
    h.configure_ai();
    assert_eq!(
        h.settings.ai_availability().expect("query"),
        AiAvailability::Available
    );
}

// ---------- 连接测试 ----------

#[test]
fn test_connection_sends_minimal_openai_compatible_request() {
    let h = harness();
    h.configure_ai();

    let report = h.settings.test_connection().expect("test connection");
    assert!(report.ok, "假端点返回 200，测试必须报告成功");
    assert_eq!(report.http_status, Some(200));

    let requests = h.http.requests();
    assert_eq!(requests.len(), 1, "连接测试恰好发出一次请求");
    let request = &requests[0];
    assert_eq!(request.url, format!("{FAKE_BASE}/chat/completions"));
    assert_eq!(
        request.auth_header,
        format!("Bearer {FAKE_KEY}"),
        "client 收到的是组装好的 Authorization 头"
    );
    assert!(request.timeout_secs > 0, "探测必须带显式超时");

    let body: serde_json::Value = serde_json::from_str(&request.body).expect("body 是 JSON");
    assert_eq!(body["model"], FAKE_MODEL);
    assert_eq!(body["messages"][0]["content"], "ping");
    assert_eq!(body["max_tokens"], 1, "探测出站必须最小化");
    assert!(
        !request.body.contains(FAKE_KEY),
        "请求体不得包含 Key 明文（Key 只经 Authorization 头）"
    );
}

#[test]
fn test_connection_reports_http_failure_with_sanitized_detail() {
    let h = harness();
    h.configure_ai();
    h.http.enqueue(Ok(HttpResponse {
        status: 401,
        body: format!(r#"{{"error":{{"message":"invalid key {FAKE_KEY}"}}}}"#),
    }));

    let report = h.settings.test_connection().expect("test connection");
    assert!(!report.ok, "非 200 必须报告失败");
    assert_eq!(report.http_status, Some(401));
    let detail = report.detail.expect("失败要带诊断信息");
    assert!(detail.contains("invalid"), "诊断信息应保留响应体摘要");
    assert!(
        !detail.contains(FAKE_KEY),
        "诊断信息不得泄漏 Key 明文: {detail}"
    );
}

#[test]
fn test_connection_reports_transport_failure_without_leaking_key() {
    let h = harness();
    h.configure_ai();
    h.http.enqueue(Err(HttpClientError::Transport(format!(
        "dial tcp: connection refused (auth {FAKE_KEY})"
    ))));

    let report = h.settings.test_connection().expect("test connection");
    assert!(!report.ok);
    assert_eq!(report.http_status, None, "传输层失败没有 HTTP 状态");
    let detail = report.detail.expect("失败要带诊断信息");
    assert!(detail.contains("refused"), "诊断信息应保留失败原因");
    assert!(
        !detail.contains(FAKE_KEY),
        "诊断信息不得泄漏 Key 明文: {detail}"
    );
}

#[test]
fn test_connection_requires_configuration_even_when_switch_is_off() {
    let h = harness();
    // 连接测试属于「配置阶段」动作：总开关关闭也允许执行，
    // 但配置不齐时按缺失项返回明确的不可用状态。
    let err = h
        .settings
        .test_connection()
        .expect_err("nothing configured");
    assert_eq!(
        err,
        SettingsError::AiUnavailable {
            reason: AiUnavailableReason::NoBaseUrl
        }
    );

    h.settings
        .set_ai_config(Some(FAKE_BASE.to_string()), Some(FAKE_MODEL.to_string()))
        .expect("set base + model");
    let err = h.settings.test_connection().expect_err("no api key yet");
    assert_eq!(
        err,
        SettingsError::AiUnavailable {
            reason: AiUnavailableReason::NoApiKey
        }
    );

    // 开关关闭但配置齐备：连接测试照常可用（配置完成后才打开开关是正常顺序）。
    h.settings.set_api_key(FAKE_KEY).expect("set key");
    assert!(
        h.settings
            .test_connection()
            .expect("test despite disabled")
            .ok
    );
}

// ---------- 无 AI 配置时其余功能不受影响 ----------

#[test]
fn fresh_install_defaults_and_sources_work_without_any_ai_configuration() {
    let h = harness();
    let ai = h.settings.ai().expect("fresh settings readable");
    assert_eq!(
        ai,
        AiSettings {
            base_url: None,
            model: None,
            api_key_configured: false,
            enabled: false
        },
        "全新库默认：未配置、未开 Key、总开关关闭"
    );

    // ADR 0001：不配 Key / 关闭 AI 时，除 AI 外全功能可用。
    let repo = Arc::new(Repository::open(&h.db_path).expect("reopen for sources"));
    let sources = Sources::new(Arc::clone(&repo), Arc::new(|_| {}));
    let root = h._dir.path().join("kb");
    std::fs::create_dir_all(&root).expect("create kb");
    std::fs::write(root.join("a.md"), "# 标题\n\n正文。\n").expect("write file");

    let summary = sources.add(&root, "笔记", None).expect("add source");
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let idle = sources
            .list()
            .expect("list")
            .into_iter()
            .find(|s| s.id == summary.id)
            .expect("source exists");
        if idle.status == SourceStatus::Idle {
            assert_eq!(idle.file_count, 1, "补扫收敛");
            break;
        }
        assert!(Instant::now() < deadline, "backfill did not finish in time");
        std::thread::sleep(Duration::from_millis(25));
    }
}
