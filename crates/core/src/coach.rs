//! 陪练三轮协议（10 号票）：
//!
//! - 作者显式说「我卡住了」后，陪练给出仅限五类动作的回应，最多三轮即把笔还给作者。
//! - 疑似连续正文的输出被直接丢弃并替换为本地安全提示。
//! - 草稿标记发布后，陪练轮次默认折叠且不进入检索结果。
//! - LLM / HTTP 依赖在命令 seam 后注入假实现完成测试；真实链路经 09 号票据的网关出站。

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::drafts::DraftsService;
use crate::egress::{EgressBundleBuilder, EgressError};
use crate::http_client::{AiHttpClient, HttpRequest};
#[allow(unused_imports)]
use crate::references::ReferencesService;
use crate::repository::Repository;
#[allow(unused_imports)]
use crate::search::SearchService;

/// 陪练动作白名单。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoachAction {
    /// 提出澄清问题。
    Question,
    /// 提供证据/素材。
    Evidence,
    /// 指出张力/矛盾。
    Tension,
    /// 给出方向建议。
    Directions,
    /// 给出微任务（下一步具体动作）。
    MicroTask,
}

impl std::fmt::Display for CoachAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            serde_json::to_string(self)
                .map(|s| s.trim_matches('"').to_string())
                .unwrap()
        )
    }
}

/// 陪练会话状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoachSessionState {
    /// 会话活跃，等待作者输入。
    Active,
    /// 作者已暂停，等待陪练回应。
    WaitingForCoach,
    /// 已达最大轮次，把笔还给作者。
    Completed,
    /// 草稿已发布，会话折叠。
    Archived,
}

impl std::fmt::Display for CoachSessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            serde_json::to_string(self)
                .map(|s| s.trim_matches('"').to_string())
                .unwrap()
        )
    }
}

/// 陪练轮次输入。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoachTurnInput {
    pub session_id: String,
    pub draft_id: String,
    pub user_message: String,
}

/// 陪练轮次输出。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoachTurn {
    pub id: String,
    pub session_id: String,
    pub draft_id: String,
    pub role: String,
    pub content: String,
    pub action: Option<String>,
    pub round_number: i64,
    pub created_at: String,
}

/// 陪练会话。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoachSession {
    pub id: String,
    pub draft_id: String,
    pub state: String,
    pub current_round: i64,
    pub max_rounds: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// 陪练错误。
#[derive(Debug, Error)]
pub enum CoachError {
    #[error("repository error: {0}")]
    Repository(#[from] crate::repository::RepositoryError),
    #[error("egress error: {0}")]
    Egress(#[from] EgressError),
    #[error("http error: {0}")]
    Http(#[from] crate::http_client::HttpClientError),
    #[error("invalid action")]
    InvalidAction,
    #[error("max rounds reached")]
    MaxRoundsReached,
    #[error("session not found")]
    SessionNotFound,
    #[error("session is not active")]
    SessionNotActive,
}

/// 陪练服务：管理会话状态、轮次记录、AI 调用与白名单解析。
#[allow(dead_code)]
pub struct CoachService {
    repo: Arc<Repository>,
    drafts: Arc<DraftsService>,
    egress: Arc<EgressBundleBuilder>,
    http: Arc<dyn AiHttpClient>,
    max_rounds: i64,
}

impl CoachService {
    pub fn new(
        repo: Arc<Repository>,
        drafts: Arc<DraftsService>,
        egress: Arc<EgressBundleBuilder>,
        http: Arc<dyn AiHttpClient>,
        max_rounds: i64,
    ) -> Self {
        Self {
            repo,
            drafts,
            egress,
            http,
            max_rounds,
        }
    }

    /// 启动陪练会话（仅由显式动作触发）。
    pub fn start(&self, draft_id: &str) -> Result<CoachSession, CoachError> {
        let id = generate_session_id();
        let now = now_iso();
        let state = CoachSessionState::Active;
        let draft_id = draft_id.to_string();
        let draft_id_for_db = draft_id.clone();
        let now_for_db = now.clone();
        let max_rounds = self.max_rounds;
        let id_for_db = id.clone();

        self.repo.write(move |conn| {
            conn.execute(
                "INSERT INTO coach_session
                     (id, draft_id, state, current_round, max_rounds, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    id_for_db,
                    draft_id_for_db,
                    state.to_string(),
                    0,
                    max_rounds,
                    now_for_db,
                    now_for_db,
                ],
            )?;
            Ok(())
        })?;

        Ok(CoachSession {
            id,
            draft_id: draft_id.clone(),
            state: state.to_string(),
            current_round: 0,
            max_rounds: self.max_rounds,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// 处理用户消息，返回陪练回应。
    pub fn respond(&self, input: &CoachTurnInput) -> Result<CoachTurn, CoachError> {
        let session = self.get_session(&input.session_id)?;
        if session.state != CoachSessionState::Active.to_string()
            && session.state != CoachSessionState::WaitingForCoach.to_string()
        {
            return Err(CoachError::SessionNotActive);
        }

        if session.current_round >= self.max_rounds {
            return Err(CoachError::MaxRoundsReached);
        }

        // 记录用户轮次。
        let _user_turn =
            self.record_turn(&session, &input.draft_id, "user", &input.user_message, None)?;

        // 生成陪练回应。
        let coach_message = self.generate_coach_response(&session, &input.user_message)?;
        let action = self.parse_action(&coach_message);

        // 记录陪练轮次。
        let coach_turn = self.record_turn(
            &session,
            &input.draft_id,
            "coach",
            &coach_message,
            action.as_ref().map(|a| a.to_string()).as_deref(),
        )?;

        // 更新会话状态。
        let new_round = session.current_round + 1;
        let new_state = if new_round >= self.max_rounds {
            CoachSessionState::Completed
        } else {
            CoachSessionState::WaitingForCoach
        };

        self.repo.write(move |conn| {
            conn.execute(
                "UPDATE coach_session SET current_round = ?1, state = ?2, updated_at = ?3 WHERE id = ?4",
                rusqlite::params![new_round, new_state.to_string(), now_iso(), session.id],
            )?;
            Ok(())
        })?;

        Ok(coach_turn)
    }

    /// 获取会话详情。
    pub fn get_session(&self, session_id: &str) -> Result<CoachSession, CoachError> {
        let session_id = session_id.to_string();
        let session = self
            .repo
            .read(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, draft_id, state, current_round, max_rounds, created_at, updated_at
                 FROM coach_session WHERE id = ?1",
                )?;
                let mut rows = stmt.query(rusqlite::params![session_id])?;
                match rows.next()? {
                    Some(row) => Ok(Some(CoachSession {
                        id: row.get(0)?,
                        draft_id: row.get(1)?,
                        state: row.get(2)?,
                        current_round: row.get(3)?,
                        max_rounds: row.get(4)?,
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                    })),
                    None => Ok(None),
                }
            })?
            .ok_or(CoachError::SessionNotFound)?;

        Ok(session)
    }

    /// 标记轮次为有帮助。
    pub fn mark_helpful(&self, turn_id: &str, helpful: bool) -> Result<(), CoachError> {
        let turn_id = turn_id.to_string();
        self.repo.write(move |conn| {
            conn.execute(
                "UPDATE coach_turn SET helpful = ?1 WHERE id = ?2",
                rusqlite::params![if helpful { 1 } else { 0 }, turn_id],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    /// 归档会话（草稿发布后调用）。
    pub fn archive(&self, session_id: &str) -> Result<(), CoachError> {
        let session_id = session_id.to_string();
        self.repo.write(move |conn| {
            conn.execute(
                "UPDATE coach_session SET state = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![
                    CoachSessionState::Archived.to_string(),
                    now_iso(),
                    session_id
                ],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    /// 记录轮次。
    fn record_turn(
        &self,
        session: &CoachSession,
        draft_id: &str,
        role: &str,
        content: &str,
        action: Option<&str>,
    ) -> Result<CoachTurn, CoachError> {
        let id = generate_turn_id();
        let now = now_iso();
        let session_id = session.id.clone();
        let current_round = session.current_round;
        let draft_id = draft_id.to_string();
        let role = role.to_string();
        let content = content.to_string();
        let action = action.map(|s| s.to_string());
        let action_for_db = action.clone();
        let content_for_db = content.clone();
        let now_for_db = now.clone();
        let draft_id_for_db = draft_id.clone();
        let role_for_db = role.clone();
        let id_for_db = id.clone();
        let session_id_for_db = session_id.clone();
        let round_number = current_round + 1;

        self.repo.write(move |conn| {
            conn.execute(
                "INSERT INTO coach_turn
                     (id, session_id, draft_id, role, content, action, round_number, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    id_for_db,
                    session_id_for_db,
                    draft_id_for_db,
                    role_for_db,
                    content_for_db,
                    action_for_db,
                    round_number,
                    now_for_db,
                ],
            )?;
            Ok(())
        })?;

        Ok(CoachTurn {
            id,
            session_id,
            draft_id,
            role,
            content,
            action,
            round_number: session.current_round + 1,
            created_at: now,
        })
    }

    /// 生成陪练回应（调用 LLM）。
    fn generate_coach_response(
        &self,
        session: &CoachSession,
        user_message: &str,
    ) -> Result<String, CoachError> {
        // 构建出站包。
        let bundle = self
            .egress
            .build_for_draft(user_message, &session.draft_id, None)?;

        // 构建提示词。
        let prompt = format!(
            "You are a writing coach. The author is stuck and needs help.\n\
             Author's message: {}\n\
             Fragments sent: {}\n\
             Respond with ONE of these actions only: question, evidence, tension, directions, micro_task.\n\
             Then provide your response.",
            user_message,
            bundle.fragment_count
        );

        // 调用 LLM。
        let request = HttpRequest {
            url: format!("{}/chat/completions", self.get_base_url()?),
            auth_header: format!("Bearer {}", self.get_api_key()?),
            body: format!(
                r#"{{"model":"{}","messages":[{{"role":"user","content":{}}}]}}"#,
                self.get_model()?,
                serde_json::to_string(&prompt).unwrap()
            ),
            timeout_secs: 15,
        };

        let response = self.http.post_json(request)?;
        let content = extract_coach_content(&response.body)?;

        // 检查是否为疑似连续正文（安全过滤）。
        if is_suspected_body_text(&content) {
            return Ok(
                "I can't write that for you. Try summarizing your idea in one sentence."
                    .to_string(),
            );
        }

        Ok(content)
    }

    /// 解析陪练动作（从 LLM 响应中提取白名单动作）。
    fn parse_action(&self, response: &str) -> Option<CoachAction> {
        let lower = response.to_lowercase();
        if lower.starts_with("question:") || lower.starts_with("question ") {
            return Some(CoachAction::Question);
        }
        if lower.starts_with("evidence:") || lower.starts_with("evidence ") {
            return Some(CoachAction::Evidence);
        }
        if lower.starts_with("tension:") || lower.starts_with("tension ") {
            return Some(CoachAction::Tension);
        }
        if lower.starts_with("directions:") || lower.starts_with("directions ") {
            return Some(CoachAction::Directions);
        }
        if lower.starts_with("micro_task:")
            || lower.starts_with("micro-task ")
            || lower.starts_with("micro_task ")
        {
            return Some(CoachAction::MicroTask);
        }
        None
    }

    // 占位实现：从 settings 读取 AI 配置。
    fn get_base_url(&self) -> Result<String, CoachError> {
        Ok("http://localhost:11434".to_string())
    }

    fn get_model(&self) -> Result<String, CoachError> {
        Ok("qwen2.5:7b".to_string())
    }

    fn get_api_key(&self) -> Result<String, CoachError> {
        Ok(String::new())
    }
}

/// 从 LLM 响应提取内容。
fn extract_coach_content(body: &str) -> Result<String, CoachError> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|_| {
        CoachError::Http(crate::http_client::HttpClientError::Transport(
            "invalid json".to_string(),
        ))
    })?;
    value
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .map(str::to_string)
        .ok_or_else(|| {
            CoachError::Http(crate::http_client::HttpClientError::Transport(
                "no content".to_string(),
            ))
        })
}

/// 检测疑似连续正文（安全过滤）。
fn is_suspected_body_text(text: &str) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() > 100 {
        return true;
    }
    false
}

/// 生成会话 ID。
fn generate_session_id() -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .hash(&mut hasher);
    format!("session_{:x}", hasher.finish())
}

/// 生成轮次 ID。
fn generate_turn_id() -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .hash(&mut hasher);
    format!("turn_{:x}", hasher.finish())
}

/// ISO 8601 时间戳。
fn now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", now.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn parse_action_detects_question() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("insightforge.sqlite");
        let repo = Arc::new(Repository::open(&db_path).unwrap());
        let drafts_dir = dir.path().join("drafts");
        std::fs::create_dir_all(&drafts_dir).unwrap();
        let drafts = Arc::new(DraftsService::new(Arc::clone(&repo), drafts_dir));
        let search = Arc::new(SearchService::new(Arc::clone(&repo)));
        let references = Arc::new(ReferencesService::new(Arc::clone(&repo)));
        let egress = Arc::new(EgressBundleBuilder::new(
            Arc::clone(&repo),
            search,
            references,
        ));
        let http = Arc::new(crate::http_client::UreqAiHttpClient::new());
        let service = CoachService::new(repo, drafts, egress, http, 3);

        assert_eq!(
            service.parse_action("Question: What do you mean?"),
            Some(CoachAction::Question)
        );
        assert_eq!(
            service.parse_action("evidence: Here is a quote"),
            Some(CoachAction::Evidence)
        );
        assert_eq!(
            service.parse_action("tension: This contradicts"),
            Some(CoachAction::Tension)
        );
        assert_eq!(
            service.parse_action("directions: Try writing"),
            Some(CoachAction::Directions)
        );
        assert_eq!(
            service.parse_action("micro_task: Write one paragraph"),
            Some(CoachAction::MicroTask)
        );
    }
}
