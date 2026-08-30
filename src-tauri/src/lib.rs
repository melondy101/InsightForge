//! InsightForge Tauri 壳：只负责窗口与启动装配；所有业务能力都在
//! `insightforge-core` 内，经窄而有类型的命令逐步暴露（见 CONTEXT.md §六）。

use tauri::Manager;

use insightforge_core::Repository;

/// 启动时打开的数据库，经 Tauri managed state 注入后续命令。
pub struct WorkspaceDb(pub Repository);

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("insightforge.sqlite");
            let db = Repository::open(&db_path)?;
            println!(
                "[insightforge] database ready: path={} sqlite={} journal_mode={} applied_migrations={}",
                db_path.display(),
                Repository::sqlite_libversion(),
                db.journal_mode().unwrap_or_else(|_| "unknown".into()),
                db.applied_migrations().map(|v| v.len()).unwrap_or(0),
            );
            app.manage(WorkspaceDb(db));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
