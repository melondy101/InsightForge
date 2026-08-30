//! InsightForge Tauri 壳：窗口与启动装配；业务能力都在 `insightforge-core`，
//! 经窄而有类型的命令暴露（ARCHITECTURE §4：sources.add / list / pause / remove / sync）。

use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use tauri::{Emitter, Manager, State};

use insightforge_core::{EventSink, Repository, SourceEvent, SourceSummary, Sources, SourcesError};

/// 启动时打开的数据库，经 Tauri managed state 注入后续命令。
pub struct WorkspaceDb(pub Arc<Repository>);

/// 来源管理服务状态。
pub struct SourcesState(pub Arc<Sources>);

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
            app.manage(SourcesState(Arc::new(Sources::new(db, sink))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            sources_add,
            sources_list,
            sources_pause,
            sources_remove,
            sources_sync
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
