//! 陪练协议契约（10 号票 + 反代写铁律）。
//!
//! 这里钉住三件最容易在「接前端」时才发现的事：
//! AI 调用用的是**用户配的那一份**端点/模型/Key，总开关关闭时根本不出站；
//! 疑似代写的输出被就地丢弃；轮次读得回来，界面才谈得上重建对话。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use insightforge_core::{
    AiUnavailableReason, CoachError, CoachService, CoachTurnInput, HttpClientError, HttpRequest,
    HttpResponse, MemorySecretStore, Repository, SecretStore, Settings,
};

const FAKE_KEY: &str = "sk-coach-secret-123";
const FAKE_BASE: &str = "https://api.example.com/v1";
const FAKE_MODEL: &str = "coach-test-model";
const PONG: &str = "question: 这一段你想让读者带走什么？";

#[derive(Clone)]
struct FakeHttp {
    recorded: Arc<Mutex<Vec<HttpRequest>>>,
    outcomes: Arc<Mutex<VecDeque<Result<HttpResponse, HttpClientError>>>>,
}

impl FakeHttp {
    fn new() -> Self {
        Self {
            recorded: Arc::new(Mutex::new(Vec::new())),
            outcomes: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn enqueue(&self, content: &str) {
        self.outcomes.lock().unwrap().push_back(Ok(HttpResponse {
            status: 200,
            body: format!(
                r#"{{"choices":[{{"message":{{"role":"assistant","content":"{content}"}}}}]}}"#
            ),
        }));
    }

    fn requests(&self) -> Vec<HttpRequest> {
        self.recorded.lock().unwrap().clone()
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
                Err(HttpClientError::Transport(
                    "the coach asked for more responses than the test queued".to_string(),
                ))
            })
    }
}

struct Harness {
    /// 只为了持有生命周期：字段一丢，临时目录就被清掉。
    _dir: tempfile::TempDir,
    http: FakeHttp,
    settings: Arc<Settings>,
    coach: Arc<CoachService>,
}

impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let repo =
            Arc::new(Repository::open(&dir.path().join("insightforge.sqlite")).expect("open"));
        let drafts = Arc::new(insightforge_core::DraftsService::new(
            Arc::clone(&repo),
            dir.path().join("drafts"),
        ));
        let search = Arc::new(insightforge_core::SearchService::new(Arc::clone(&repo)));
        let references = Arc::new(insightforge_core::ReferencesService::new(Arc::clone(&repo)));
        let egress = Arc::new(insightforge_core::EgressBundleBuilder::new(
            Arc::clone(&repo),
            search,
            references,
        ));
        let http = FakeHttp::new();
        let settings = Arc::new(Settings::new(
            Arc::clone(&repo),
            Arc::new(MemorySecretStore::new()) as Arc<dyn SecretStore>,
            Arc::new(http.clone()),
        ));
        let coach = Arc::new(CoachService::new(
            repo,
            drafts,
            egress,
            Arc::clone(&settings),
            3,
        ));
        Self {
            _dir: dir,
            http,
            settings,
            coach,
        }
    }

    /// 陪练默认不可用（AI 总开关缺省关闭），要出站必须先按用户身份配齐。
    fn configure_ai(&self) {
        self.settings
            .set_ai_config(Some(FAKE_BASE.to_string()), Some(FAKE_MODEL.to_string()))
            .expect("set config");
        self.settings.set_api_key(FAKE_KEY).expect("set key");
        self.settings.set_ai_enabled(true).expect("enable");
    }

    fn input(&self, session_id: &str, message: &str) -> CoachTurnInput {
        CoachTurnInput {
            session_id: session_id.to_string(),
            draft_id: "draft-1".to_string(),
            user_message: message.to_string(),
        }
    }
}

#[test]
fn the_coach_refuses_to_go_out_while_the_ai_master_switch_is_off() {
    let fx = Harness::new();
    let session = fx.coach.start("draft-1").expect("start");
    fx.http.enqueue(PONG);

    let err = fx
        .coach
        .respond(&fx.input(&session.id, "我卡住了"))
        .expect_err("AI 未开启时陪练不能出站");

    assert_eq!(err.code(), "ai_disabled", "got {err:?}");
    assert!(
        matches!(
            err,
            CoachError::Settings(insightforge_core::SettingsError::AiUnavailable {
                reason: AiUnavailableReason::Disabled
            })
        ),
        "expected the structured unavailable state, got {err:?}"
    );
    assert!(
        fx.http.requests().is_empty(),
        "总开关关闭时一个请求都不该发出去"
    );
}

#[test]
fn the_coach_uses_the_users_configured_endpoint_model_and_key() {
    let fx = Harness::new();
    fx.configure_ai();
    let session = fx.coach.start("draft-1").expect("start");
    fx.http.enqueue(PONG);

    let turn = fx
        .coach
        .respond(&fx.input(&session.id, "我卡住了"))
        .expect("respond");

    let requests = fx.http.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(
        request.url,
        format!("{FAKE_BASE}/chat/completions"),
        "陪练不能自带一套端点"
    );
    assert_eq!(request.auth_header, format!("Bearer {FAKE_KEY}"));
    assert!(
        request.body.contains(FAKE_MODEL),
        "请求体要用用户配的模型：{}",
        request.body
    );
    assert!(!request.body.contains(FAKE_KEY), "Key 不能进请求体");
    assert_eq!(turn.content, PONG);
    assert_eq!(turn.action.as_deref(), Some("question"));
}

#[test]
fn a_response_that_looks_like_continuing_the_body_is_discarded() {
    let fx = Harness::new();
    fx.configure_ai();
    let session = fx.coach.start("draft-1").expect("start");
    fx.http.enqueue(&"word ".repeat(120));

    let turn = fx
        .coach
        .respond(&fx.input(&session.id, "接着写"))
        .expect("respond");

    assert!(
        !turn.content.contains("word"),
        "疑似连续正文必须被丢弃，而不是存进对话里"
    );
    assert_eq!(turn.action, None, "被丢弃的回应不该带白名单动作");
}

#[test]
fn a_sessions_turns_read_back_in_conversation_order() {
    let fx = Harness::new();
    fx.configure_ai();
    let session = fx.coach.start("draft-1").expect("start");
    fx.http.enqueue(PONG);
    fx.coach
        .respond(&fx.input(&session.id, "第一段怎么写"))
        .expect("first round");
    fx.http.enqueue("micro_task: 用一句话写出这一节的结论");
    fx.coach
        .respond(&fx.input(&session.id, "我想讲清一个观点"))
        .expect("second round");

    let turns = fx.coach.turns(&session.id).expect("turns");

    let roles: Vec<&str> = turns.iter().map(|turn| turn.role.as_str()).collect();
    assert_eq!(roles, vec!["user", "coach", "user", "coach"]);
    assert_eq!(
        turns
            .iter()
            .map(|turn| turn.round_number)
            .collect::<Vec<i64>>(),
        vec![1, 1, 2, 2]
    );
    assert_eq!(turns[3].content, "micro_task: 用一句话写出这一节的结论");
}

#[test]
fn the_third_round_ends_the_session_and_further_turns_are_refused() {
    let fx = Harness::new();
    fx.configure_ai();
    let session = fx.coach.start("draft-1").expect("start");
    for round in 0..3 {
        fx.http.enqueue(&format!("question: 第 {round} 轮追问"));
        fx.coach
            .respond(&fx.input(&session.id, "还是有点卡"))
            .expect("round within the budget");
    }

    let after = fx.coach.get_session(&session.id).expect("session");
    assert_eq!(after.state, "completed");

    fx.http.enqueue("question: 不该再来一轮");
    let err = fx
        .coach
        .respond(&fx.input(&session.id, "再问一次"))
        .expect_err("三轮之后要把笔还给作者");
    assert_eq!(err.code(), "max_rounds_reached");
}

#[test]
fn a_helpfulness_mark_round_trips_through_the_transcript() {
    let fx = Harness::new();
    fx.configure_ai();
    let session = fx.coach.start("draft-1").expect("start");
    fx.http.enqueue(PONG);
    let turn = fx
        .coach
        .respond(&fx.input(&session.id, "我卡住了"))
        .expect("respond");
    assert!(!turn.helpful, "新轮次默认未标记");

    fx.coach.mark_helpful(&turn.id, true).expect("mark");

    let turns = fx.coach.turns(&session.id).expect("turns");
    let marked = turns
        .iter()
        .find(|row| row.id == turn.id)
        .expect("the coach turn is in the transcript");
    assert!(marked.helpful);

    fx.coach.mark_helpful(&turn.id, false).expect("unmark");
    let turns = fx.coach.turns(&session.id).expect("turns");
    assert!(
        !turns
            .iter()
            .find(|row| row.id == turn.id)
            .expect("turn")
            .helpful
    );
}

#[test]
fn the_api_key_never_appears_in_the_stored_transcript() {
    let fx = Harness::new();
    fx.configure_ai();
    let session = fx.coach.start("draft-1").expect("start");
    fx.http.enqueue(PONG);
    fx.coach
        .respond(&fx.input(&session.id, "我卡住了"))
        .expect("respond");

    let dumped = format!("{:?}", fx.coach.turns(&session.id).expect("turns"));

    assert!(
        !dumped.contains(FAKE_KEY),
        "对话记录里不能出现 Key（那是出站凭据）"
    );
}
