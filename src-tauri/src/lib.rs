//! InsightForge Tauri 壳：窗口与启动装配；业务能力都在 `insightforge-core`，
//! 经窄而有类型的命令暴露（ARCHITECTURE §4：sources.* 与 08 号票的 settings.* / ai_sample）。

use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use tauri::{Emitter, Manager, State};

use insightforge_core::{
    AiSampleReply, AiSettings, DraftsError, DraftsService, EventSink, KeyringStore, Repository,
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
fn drafts_ensure_directory(
    state: State<'_, DraftsState>,
) -> Result<(), CommandError> {
    Ok(state.0.ensure_directory()?)
}

#[tauri::command]
fn drafts_list(state: State<'_, DraftsState>) -> Result<Vec<insightforge_core::DraftFile>, CommandError> {
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
fn drafts_delete(
    state: State<'_, DraftsState>,
    filename: String,
) -> Result<(), CommandError> {
    Ok(state.0.delete(&filename)?)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            app.manage(DraftsState(drafts));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            sources_add,
            sources_list,
            sources_pause,
            sources_remove,
            sources_sync,
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
            drafts_delete
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
