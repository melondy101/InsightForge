//! 陪练三轮协议（10 号票）：
//!
//! - 作者显式说「我卡住了」后，陪练给出仅限五类动作的回应，最多三轮即把笔还给作者。
//! - 疑似连续正文的输出被直接丢弃并替换为本地安全提示。
//! - 草稿标记发布后，陪练轮次默认折叠且不进入检索结果。
//! - LLM / HTTP 依赖在命令 seam 后注入假实现完成测试；真实链路经 09 号票据的网关出站。

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::clock::now_iso;
use crate::drafts::DraftsService;
use crate::egress::{EgressBundleBuilder, EgressError};
use crate::identity::new_id;
#[allow(unused_imports)]
use crate::references::ReferencesService;
use crate::repository::Repository;
#[allow(unused_imports)]
use crate::search::SearchService;
use crate::settings::Settings;

/// 一次陪练回应的输出上限：陪练给的是提示与微任务，不是正文。
const COACH_MAX_TOKENS: u32 = 400;

/// 疑似代写时的本地安全提示（铁律 2：只提示、不改写）。
/// 注：这段文字会进对话记录，尚未走 i18n（见 docs/HANDOFF.md 的文案债清单）。
const SUSPECTED_BODY_HINT: &str =
    "I can't write that for you. Try summarizing your idea in one sentence.";

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
    /// 作者是否标过这一轮「有帮助」。
    pub helpful: bool,
}

/// 陪练会话。
#[derive(Debug, Clone, PartialEq, Serialize)]
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
    /// AI 侧的不可用/失败原因（总开关、配置缺失、HTTP、响应不可解析）。
    /// 陪练与其余能力共用同一份守门与配置，不另开一条出站路径。
    #[error(transparent)]
    Settings(#[from] crate::settings::SettingsError),
    #[error("invalid action")]
    InvalidAction,
    #[error("max rounds reached")]
    MaxRoundsReached,
    #[error("session not found")]
    SessionNotFound,
    #[error("session is not active")]
    SessionNotActive,
}

impl CoachError {
    /// 稳定错误码：前端按码走 i18n，不展示英文 `to_string()`。
    /// AI 相关的码直接沿用 [`crate::settings::SettingsError::code`]，
    /// 让「AI 未启用」在设置页与陪练面板里是同一个码、同一句话。
    pub fn code(&self) -> &'static str {
        match self {
            Self::Repository(_) => "repository_error",
            Self::Egress(_) => "egress_error",
            Self::Settings(err) => err.code(),
            Self::InvalidAction => "invalid_action",
            Self::MaxRoundsReached => "max_rounds_reached",
            Self::SessionNotFound => "session_not_found",
            Self::SessionNotActive => "session_not_active",
        }
    }
}

/// 陪练服务：管理会话状态、轮次记录、AI 调用与白名单解析。
#[allow(dead_code)]
pub struct CoachService {
    repo: Arc<Repository>,
    drafts: Arc<DraftsService>,
    egress: Arc<EgressBundleBuilder>,
    settings: Arc<Settings>,
    max_rounds: i64,
}

impl CoachService {
    pub fn new(
        repo: Arc<Repository>,
        drafts: Arc<DraftsService>,
        egress: Arc<EgressBundleBuilder>,
        settings: Arc<Settings>,
        max_rounds: i64,
    ) -> Self {
        Self {
            repo,
            drafts,
            egress,
            settings,
            max_rounds,
        }
    }

    /// 启动陪练会话（仅由显式动作触发）。
    pub fn start(&self, draft_id: &str) -> Result<CoachSession, CoachError> {
        let id = new_id();
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
        // 先报「三轮用完」再报「会话不活跃」：前者才是作者要听懂的那句话，
        // 而且它是把笔还给作者的信号，不是「会话坏了」的信号。
        if session.current_round >= self.max_rounds {
            return Err(CoachError::MaxRoundsReached);
        }
        if session.state != CoachSessionState::Active.to_string()
            && session.state != CoachSessionState::WaitingForCoach.to_string()
        {
            return Err(CoachError::SessionNotActive);
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

    /// 该草稿最近一次陪练会话（没有则 `None`）。
    ///
    /// 界面需要它才谈得上「继续上次」：没有这条查询，重新打开陪练只能再造一个会话，
    /// 作者会看到两轮互不知情的对话。
    pub fn latest_session_for_draft(
        &self,
        draft_id: &str,
    ) -> Result<Option<CoachSession>, CoachError> {
        let draft_id = draft_id.to_string();
        let session = self.repo.read(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, draft_id, state, current_round, max_rounds, created_at, updated_at
                 FROM coach_session
                 WHERE draft_id = ?1
                 ORDER BY created_at DESC, id DESC
                 LIMIT 1",
            )?;
            let mut rows = stmt.query(rusqlite::params![draft_id])?;
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
        })?;
        Ok(session)
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
        let id = new_id();
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
            helpful: false,
        })
    }

    /// 生成陪练回应：走 [`Settings::complete`]——端点、模型与 Key 都是用户配的那一份，
    /// 总开关关闭时根本不发请求。
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

        let content = self.settings.complete(&prompt, COACH_MAX_TOKENS)?.content;

        // 检查是否为疑似连续正文（安全过滤）。
        if is_suspected_body_text(&content) {
            return Ok(SUSPECTED_BODY_HINT.to_string());
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

    /// 一次会话的全部轮次（按轮次与写入顺序排列），供界面重建对话。
    pub fn turns(&self, session_id: &str) -> Result<Vec<CoachTurn>, CoachError> {
        let session_id = session_id.to_string();
        let turns = self
            .repo
            .read(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, session_id, draft_id, role, content, action, round_number, created_at, helpful
                     FROM coach_turn
                     WHERE session_id = ?1
                     ORDER BY round_number, created_at, id",
                )?;
                let mut rows = stmt.query(rusqlite::params![session_id])?;
                let mut turns = Vec::new();
                while let Some(row) = rows.next()? {
                    turns.push(CoachTurn {
                        id: row.get(0)?,
                        session_id: row.get(1)?,
                        draft_id: row.get(2)?,
                        role: row.get(3)?,
                        content: row.get(4)?,
                        action: row.get(5)?,
                        round_number: row.get(6)?,
                        created_at: row.get(7)?,
                        helpful: row.get::<_, i64>(8)? != 0,
                    });
                }
                Ok(turns)
            })?;
        Ok(turns)
    }
}

/// 检测疑似连续正文（安全过滤）。
fn is_suspected_body_text(text: &str) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() > 100 {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// 在临时目录里装配一个跑在真实 SQLite 上的陪练服务。
    fn service_in(dir: &std::path::Path) -> CoachService {
        let repo = Arc::new(Repository::open(&dir.join("insightforge.sqlite")).unwrap());
        let drafts_dir = dir.join("drafts");
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
        let settings = Arc::new(Settings::new(
            Arc::clone(&repo),
            Arc::new(crate::secrets::MemorySecretStore::default()),
            http,
        ));
        CoachService::new(repo, drafts, egress, settings, 3)
    }

    #[test]
    fn fresh_database_can_start_and_read_back_a_session() {
        let dir = tempfile::tempdir().unwrap();
        let service = service_in(dir.path());

        let session = service.start("draft-1").unwrap();
        assert_eq!(session.draft_id, "draft-1");
        assert_eq!(session.state, CoachSessionState::Active.to_string());
        assert_eq!(session.current_round, 0);

        assert_eq!(service.get_session(&session.id).unwrap(), session);
    }

    #[test]
    fn parse_action_detects_question() {
        let dir = tempfile::tempdir().unwrap();
        let service = service_in(dir.path());

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
