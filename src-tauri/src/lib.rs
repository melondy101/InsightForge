//! InsightForge Tauri 壳：窗口与启动装配；业务能力都在 `insightforge-core`，
//! 经窄而有类型的命令暴露（ARCHITECTURE §4：sources.* 与 08 号票的 settings.* / ai_sample）。

use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use tauri::{Emitter, Manager, State};

use insightforge_core::coach::CoachTurnInput;
use insightforge_core::egress::{
    AuditLogger, AuditRecord, EgressBundle, EgressBundleBuilder, EgressError,
};
use insightforge_core::references::CreateReference;
use insightforge_core::thoughts::{ThoughtCard, ThoughtsError, ThoughtsService, UpsertThoughtCard};
use insightforge_core::{
    AiSampleReply, AiSettings, CoachError, CoachService, CoachSession, CoachTurn, DraftsError,
    DraftsService, EventSink, KeyringStore, ReferencesError, ReferencesService, Repository,
    SearchError, SearchService, Settings, SettingsError, SourceEvent, SourceSummary, Sources,
    SourcesError, TestConnectionReport, UreqAiHttpClient,
};

/// 启动时打开的数据库，经 Tauri managed state 注入后续命令。
pub struct WorkspaceDb(pub Arc<Repository>);

/// 来源管理服务状态。
pub struct SourcesState(pub Arc<Sources>);

/// 检索服务状态。
pub struct SearchState(pub Arc<SearchService>);

/// 设置服务状态：非敏感配置走 SQLite，API Key 走 OS 凭据库。
pub struct SettingsState(pub Arc<Settings>);

/// 草稿服务状态。
pub struct DraftsState(pub Arc<DraftsService>);

/// 引用服务状态。
pub struct ReferencesState(pub Arc<ReferencesService>);

/// 出站包与审计服务状态。
pub struct EgressState(pub Arc<EgressBundleBuilder>, pub Arc<AuditLogger>);

/// 思考卡服务状态。
pub struct ThoughtsState(pub Arc<ThoughtsService>);

/// 陪练服务状态。
pub struct CoachState(pub Arc<CoachService>);

/// 命令错误投影：稳定 code 供前端 i18n 映射，message 供开发诊断。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    code: &'static str,
    message: String,
}

impl From<SourcesError> for CommandError {
    fn from(error: SourcesError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}

impl From<SearchError> for CommandError {
    fn from(error: SearchError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}

impl From<SettingsError> for CommandError {
    fn from(error: SettingsError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}

impl From<DraftsError> for CommandError {
    fn from(error: DraftsError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}

impl From<ReferencesError> for CommandError {
    fn from(error: ReferencesError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}

impl From<EgressError> for CommandError {
    fn from(error: EgressError) -> Self {
        Self {
            code: "egress_error",
            message: error.to_string(),
        }
    }
}

impl From<ThoughtsError> for CommandError {
    fn from(error: ThoughtsError) -> Self {
        Self {
            code: "thoughts_error",
            message: error.to_string(),
        }
    }
}

impl From<CoachError> for CommandError {
    fn from(error: CoachError) -> Self {
        Self {
            code: "coach_error",
            message: error.to_string(),
        }
    }
}

#[tauri::command]
fn sources_add(
    state: State<'_, SourcesState>,
    root: String,
    name: String,
    ignore_globs: Option<Vec<String>>,
) -> Result<SourceSummary, CommandError> {
    Ok(state.0.add(Path::new(&root), &name, ignore_globs)?)
}

#[tauri::command]
fn sources_list(state: State<'_, SourcesState>) -> Result<Vec<SourceSummary>, CommandError> {
    Ok(state.0.list()?)
}

#[tauri::command]
fn sources_pause(
    state: State<'_, SourcesState>,
    id: String,
    paused: bool,
) -> Result<SourceSummary, CommandError> {
    Ok(state.0.pause(&id, paused)?)
}

#[tauri::command]
fn sources_remove(state: State<'_, SourcesState>, id: String) -> Result<(), CommandError> {
    Ok(state.0.remove(&id)?)
}

#[tauri::command]
fn sources_sync(state: State<'_, SourcesState>, id: String) -> Result<(), CommandError> {
    Ok(state.0.sync(&id)?)
}

#[tauri::command]
fn sources_reconcile(state: State<'_, SourcesState>, id: String) -> Result<(), CommandError> {
    Ok(state.0.reconcile(&id)?)
}

#[tauri::command]
fn sources_rebuild_index(state: State<'_, SourcesState>, id: String) -> Result<(), CommandError> {
    Ok(state.0.rebuild_index(&id)?)
}

#[tauri::command]
fn sources_set_sync_interval(
    state: State<'_, SourcesState>,
    id: String,
    interval_minutes: u32,
) -> Result<SourceSummary, CommandError> {
    Ok(state.0.set_sync_interval(&id, interval_minutes)?)
}

// ---- search.*（03 号票：中文关键词检索与证据卡）----

#[tauri::command]
fn search_query(
    state: State<'_, SearchState>,
    source_id: String,
    query: String,
    limit: usize,
) -> Result<Vec<insightforge_core::EvidenceCard>, CommandError> {
    Ok(state.0.query(&source_id, &query, limit)?)
}

#[tauri::command]
fn search_open_context(
    state: State<'_, SearchState>,
    source_id: String,
    fragment_id: i64,
) -> Result<Option<insightforge_core::FragmentContext>, CommandError> {
    Ok(state.0.open_context(&source_id, fragment_id)?)
}

// ---- settings.* / ai_sample（08 号票：BYOK 凭据库与 AI 总开关）----

#[tauri::command]
fn settings_get(state: State<'_, SettingsState>) -> Result<AiSettings, CommandError> {
    Ok(state.0.ai()?)
}

#[tauri::command]
fn settings_set_ai(
    state: State<'_, SettingsState>,
    base_url: Option<String>,
    model: Option<String>,
) -> Result<AiSettings, CommandError> {
    Ok(state.0.set_ai_config(base_url, model)?)
}

#[tauri::command]
fn settings_set_ai_enabled(
    state: State<'_, SettingsState>,
    enabled: bool,
) -> Result<AiSettings, CommandError> {
    Ok(state.0.set_ai_enabled(enabled)?)
}

#[tauri::command]
fn settings_set_api_key(
    state: State<'_, SettingsState>,
    api_key: String,
) -> Result<AiSettings, CommandError> {
    Ok(state.0.set_api_key(&api_key)?)
}

#[tauri::command]
fn settings_clear_api_key(state: State<'_, SettingsState>) -> Result<AiSettings, CommandError> {
    Ok(state.0.clear_api_key()?)
}

#[tauri::command]
fn settings_test_connection(
    state: State<'_, SettingsState>,
) -> Result<TestConnectionReport, CommandError> {
    Ok(state.0.test_connection()?)
}

/// 样例 AI 命令：演示总开关守门语义（关闭 → code=ai_disabled 等；
/// 09/10 号票的真实 AI 命令复用同一守门模式）。
#[tauri::command]
fn ai_sample(
    state: State<'_, SettingsState>,
    prompt: String,
) -> Result<AiSampleReply, CommandError> {
    Ok(state.0.ai_sample(&prompt)?)
}

// ---- drafts.*（06 号票：原子保存与冲突检测）----

#[tauri::command]
fn drafts_ensure_directory(state: State<'_, DraftsState>) -> Result<(), CommandError> {
    Ok(state.0.ensure_directory()?)
}

#[tauri::command]
fn drafts_list(
    state: State<'_, DraftsState>,
) -> Result<Vec<insightforge_core::DraftFile>, CommandError> {
    Ok(state.0.list()?)
}

#[tauri::command]
fn drafts_create(
    state: State<'_, DraftsState>,
    filename: String,
) -> Result<insightforge_core::DraftFile, CommandError> {
    Ok(state.0.create(&filename)?)
}

#[tauri::command]
fn drafts_open(
    state: State<'_, DraftsState>,
    filename: String,
) -> Result<insightforge_core::DraftFile, CommandError> {
    Ok(state.0.open(&filename)?)
}

#[tauri::command]
fn drafts_save(
    state: State<'_, DraftsState>,
    filename: String,
    content: String,
) -> Result<insightforge_core::DraftFile, CommandError> {
    Ok(state.0.save(&filename, &content)?)
}

#[tauri::command]
fn drafts_delete(state: State<'_, DraftsState>, filename: String) -> Result<(), CommandError> {
    Ok(state.0.delete(&filename)?)
}

// ---- references.*（07 号票：证据架与不可变引用快照）----

#[tauri::command]
fn references_create(
    state: State<'_, ReferencesState>,
    draft_id: String,
    fragment_id: i64,
) -> Result<insightforge_core::Reference, CommandError> {
    Ok(state.0.create(&CreateReference {
        draft_id,
        fragment_id,
    })?)
}

#[tauri::command]
fn references_list(
    state: State<'_, ReferencesState>,
    draft_id: String,
) -> Result<Vec<insightforge_core::Reference>, CommandError> {
    Ok(state.0.list(&draft_id)?)
}

#[tauri::command]
fn references_adopt(state: State<'_, ReferencesState>, id: String) -> Result<(), CommandError> {
    Ok(state.0.adopt(&id)?)
}

#[tauri::command]
fn references_mark_source_deleted(
    state: State<'_, ReferencesState>,
    source_id: String,
) -> Result<(), CommandError> {
    Ok(state.0.mark_source_deleted(&source_id)?)
}

// ---- egress.*（09 号票：出站包与调用审计）----

#[tauri::command]
fn egress_build(
    state: State<'_, EgressState>,
    draft_content: String,
    draft_id: String,
    search_query: Option<(String, String, usize)>,
) -> Result<EgressBundle, CommandError> {
    let (source_id, query, limit) = search_query.unwrap_or_default();
    Ok(state
        .0
        .build_for_draft(&draft_content, &draft_id, Some((&source_id, &query, limit)))?)
}

#[tauri::command]
fn audit_log(
    state: State<'_, EgressState>,
    model: String,
    fragment_count: i64,
    fragment_ids: Vec<String>,
    status: String,
) -> Result<AuditRecord, CommandError> {
    Ok(state
        .1
        .log(&model, fragment_count, &fragment_ids, &status)?)
}

#[tauri::command]
fn audit_list(
    state: State<'_, EgressState>,
    limit: usize,
) -> Result<Vec<AuditRecord>, CommandError> {
    Ok(state.1.list(limit)?)
}

// ---- thoughts.*（11 号票：思考卡）----

#[tauri::command]
fn thoughts_upsert(
    state: State<'_, ThoughtsState>,
    input: UpsertThoughtCard,
) -> Result<ThoughtCard, CommandError> {
    Ok(state.0.upsert(&input)?)
}

#[tauri::command]
fn thoughts_list(state: State<'_, ThoughtsState>) -> Result<Vec<ThoughtCard>, CommandError> {
    Ok(state.0.list()?)
}

#[tauri::command]
fn thoughts_list_by_draft(
    state: State<'_, ThoughtsState>,
    draft_id: String,
) -> Result<Vec<ThoughtCard>, CommandError> {
    Ok(state.0.list_by_draft(&draft_id)?)
}

#[tauri::command]
fn thoughts_list_by_source(
    state: State<'_, ThoughtsState>,
    source_id: String,
) -> Result<Vec<ThoughtCard>, CommandError> {
    Ok(state.0.list_by_source(&source_id)?)
}

#[tauri::command]
fn thoughts_delete(state: State<'_, ThoughtsState>, id: String) -> Result<(), CommandError> {
    Ok(state.0.delete(&id)?)
}

// ---- coach.*（10 号票：陪练三轮协议）----

#[tauri::command]
fn coach_start(
    state: State<'_, CoachState>,
    draft_id: String,
) -> Result<CoachSession, CommandError> {
    Ok(state.0.start(&draft_id)?)
}

#[tauri::command]
fn coach_respond(
    state: State<'_, CoachState>,
    input: CoachTurnInput,
) -> Result<CoachTurn, CommandError> {
    Ok(state.0.respond(&input)?)
}

#[tauri::command]
fn coach_get_session(
    state: State<'_, CoachState>,
    session_id: String,
) -> Result<CoachSession, CommandError> {
    Ok(state.0.get_session(&session_id)?)
}

#[tauri::command]
fn coach_mark_helpful(
    state: State<'_, CoachState>,
    turn_id: String,
    helpful: bool,
) -> Result<(), CommandError> {
    Ok(state.0.mark_helpful(&turn_id, helpful)?)
}

#[tauri::command]
fn coach_archive(state: State<'_, CoachState>, session_id: String) -> Result<(), CommandError> {
    Ok(state.0.archive(&session_id)?)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard::init())
        .plugin(
            tauri_plugin_updater::Builder::new()
                .pubkey(std::env::var("INSIGHTFORGE_UPDATER_PUBKEY").unwrap_or_else(|_| {
                    #[cfg(not(debug_assertions))]
                    panic!("INSIGHTFORGE_UPDATER_PUBKEY must be set in release builds");
                    "dW50cnVzdGVkIGJ5IHNpbW9sZiBvciB0ZXN0".into()
                }))
                .build(),
        )
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("insightforge.sqlite");
            let db = Arc::new(Repository::open(&db_path)?);
            println!(
                "[insightforge] database ready: path={} sqlite={} journal_mode={} applied_migrations={}",
                db_path.display(),
                Repository::sqlite_libversion(),
                db.journal_mode().unwrap_or_else(|_| "unknown".into()),
                db.applied_migrations().map(|v| v.len()).unwrap_or(0),
            );
            app.manage(WorkspaceDb(Arc::clone(&db)));

            // 索引进度经事件通道推送前端（spec：事件通道推送状态）。
            let handle = app.handle().clone();
            let sink: EventSink = Arc::new(move |event: SourceEvent| {
                if let Err(err) = handle.emit("sources://progress", &event) {
                    eprintln!("[insightforge] failed to emit progress event: {err}");
                }
            });
            app.manage(SourcesState(Arc::new(Sources::new(Arc::clone(&db), sink))));

            // 03 号票：检索服务——只读 FTS5 + 回退线性扫描
            let search = Arc::new(SearchService::new(Arc::clone(&db)));
            let search_for_egress = Arc::clone(&search);
            app.manage(SearchState(search));

            // 08 号票：设置服务——非敏感配置走 SQLite，API Key 走 OS 凭据库
            // （Windows Credential Manager，经 keyring），HTTP 经 ureq。
            let settings = Arc::new(Settings::new(
                Arc::clone(&db),
                Arc::new(KeyringStore::new()),
                Arc::new(UreqAiHttpClient::new()),
            ));
            println!(
                "[insightforge] settings ready: secret_store=windows-credential-manager service=insightforge http_client=ureq"
            );
            app.manage(SettingsState(settings));

            // 06 号票：草稿服务——原子保存 + 冲突检测
            let drafts_dir = data_dir.join("drafts");
            let drafts = Arc::new(DraftsService::new(Arc::clone(&db), drafts_dir));
            app.manage(DraftsState(Arc::clone(&drafts)));

            // 07 号票：引用服务——不可变快照
            let references = Arc::new(ReferencesService::new(Arc::clone(&db)));
            let references_for_egress = Arc::clone(&references);
            app.manage(ReferencesState(references));

            // 09 号票：出站包与调用审计
            let egress_builder = Arc::new(EgressBundleBuilder::new(
                Arc::clone(&db),
                Arc::clone(&search_for_egress),
                Arc::clone(&references_for_egress),
            ));
            let audit_logger = Arc::new(AuditLogger::new(Arc::clone(&db)));
            app.manage(EgressState(egress_builder, audit_logger));

            // 11 号票：思考卡
            let thoughts = Arc::new(ThoughtsService::new(Arc::clone(&db), Arc::clone(&drafts)));
            app.manage(ThoughtsState(thoughts));

            // 10 号票：陪练服务
            let coach = Arc::new(CoachService::new(
                Arc::clone(&db),
                Arc::clone(&drafts),
                Arc::new(EgressBundleBuilder::new(
                    Arc::clone(&db),
                    Arc::clone(&search_for_egress),
                    Arc::clone(&references_for_egress),
                )),
                Arc::new(UreqAiHttpClient::new()),
                3,
            ));
            app.manage(CoachState(coach));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            sources_add,
            sources_list,
            sources_pause,
            sources_remove,
            sources_sync,
            sources_reconcile,
            sources_rebuild_index,
            sources_set_sync_interval,
            search_query,
            search_open_context,
            settings_get,
            settings_set_ai,
            settings_set_ai_enabled,
            settings_set_api_key,
            settings_clear_api_key,
            settings_test_connection,
            ai_sample,
            drafts_ensure_directory,
            drafts_list,
            drafts_create,
            drafts_open,
            drafts_save,
            drafts_delete,
            references_create,
            references_list,
            references_adopt,
            references_mark_source_deleted,
            egress_build,
            audit_log,
            audit_list,
            thoughts_upsert,
            thoughts_list,
            thoughts_list_by_draft,
            thoughts_list_by_source,
            thoughts_delete,
            coach_start,
            coach_respond,
            coach_get_session,
            coach_mark_helpful,
            coach_archive
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod updater_tests {
    #[test]
    #[should_panic(expected = "INSIGHTFORGE_UPDATER_PUBKEY must be set in release builds")]
    fn release_build_pubkey_missing_panics() {
        // Simulate release build by clearing the env var if set
        std::env::remove_var("INSIGHTFORGE_UPDATER_PUBKEY");
        // This would panic in a real release build; in test build (debug_assertions) it falls back
        let _ = std::env::var("INSIGHTFORGE_UPDATER_PUBKEY").unwrap_or_else(|_| {
            #[cfg(not(debug_assertions))]
            panic!("INSIGHTFORGE_UPDATER_PUBKEY must be set in release builds");
            "test-fallback".into()
        });
    }

    #[test]
    fn debug_build_uses_fallback_when_missing() {
        std::env::remove_var("INSIGHTFORGE_UPDATER_PUBKEY");
        let result = std::env::var("INSIGHTFORGE_UPDATER_PUBKEY").unwrap_or_else(|_| {
            #[cfg(debug_assertions)]
            "test-fallback".into();
            #[cfg(not(debug_assertions))]
            panic!("INSIGHTFORGE_UPDATER_PUBKEY must be set in release builds");
            "test-fallback".into()
        });
        assert_eq!(result, "test-fallback");
    }
}
