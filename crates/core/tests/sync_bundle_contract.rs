//! 同步出站包契约（ADR 0002 §5，铁律 3 的修订）：
//! 同步载荷是一类**独立**的出站包——它必须自带「发了几条、发了哪些字段」的清单，
//! 并且在结构上就带不走设备本地的绝对路径与保存基线。
//! 光在文档里写「同步载荷永不含路径」不算约束：这里让它在编译期与运行期都成立。

use std::path::Path;
use std::sync::Arc;

use insightforge_core::{
    AuditLogger, DraftsService, Repository, SyncBundle, DEVICE_LOCAL_COLUMNS, SYNCED_ASSETS,
};

struct Fixture {
    /// 只为了持有生命周期：字段一丢，临时目录就被清掉。
    _dir: tempfile::TempDir,
    repo: Arc<Repository>,
    drafts: Arc<DraftsService>,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let repo = Arc::new(Repository::open(&dir.path().join("insightforge.sqlite")).unwrap());
        let drafts = Arc::new(DraftsService::new(
            Arc::clone(&repo),
            dir.path().join("drafts"),
        ));
        Self {
            _dir: dir,
            repo,
            drafts,
        }
    }

    fn drafts_dir(&self) -> &Path {
        self.drafts.dir()
    }

    fn seed_draft(&self, content: &str) -> String {
        self.drafts.create("k1.md").unwrap();
        self.drafts.save("k1.md", content).unwrap().id
    }

    /// 播种其余四张表。外键在这个构建里是真 enforced 的（绕过 parent 直接插
    /// `draft_reference` 会拿到 `FOREIGN KEY constraint failed`），所以先把
    /// knowledge_source → source_file → fragment 这条链建起来，并用 `seed_draft`
    /// 返回的真实逻辑 ID 串起来——载荷里的每一行都是真实可达的数据。
    fn seed_other_assets(&self, draft_id: &str) {
        let statements = [
            "INSERT INTO knowledge_source (id, name, root_path) VALUES ('s1', '笔记', 'C:/kb')"
                .to_string(),
            "INSERT INTO source_file (id, source_id, rel_path, content_hash, size, mtime_ms)
             VALUES (1, 's1', 'a.md', 'h', 10, 1)"
                .to_string(),
            "INSERT INTO fragment (id, source_file_id, ordinal, heading_path,
                 start_line, end_line, text, content_hash)
             VALUES (1, 1, 0, '', 1, 2, '原文', 'h')"
                .to_string(),
            format!(
                "INSERT INTO draft_reference (id, draft_id, fragment_id, source_id,
                     source_name, rel_path, heading_path, snippet, full_text, start_line,
                     end_line)
                 VALUES ('r1', '{draft_id}', 1, 's1', '笔记', 'a.md', '', '摘', '全文', 1, 2)"
            ),
            format!(
                "INSERT INTO thought_card (id, idea, source_id, source_name, rel_path,
                     heading_path, draft_id, draft_filename)
                 VALUES ('t1', '想法', 's1', '笔记', 'a.md', '', '{draft_id}', 'k1.md')"
            ),
            format!(
                "INSERT INTO coach_session (id, draft_id, state, current_round, max_rounds)
                 VALUES ('c1', '{draft_id}', 'active', 1, 3)"
            ),
            format!(
                "INSERT INTO coach_turn (id, session_id, draft_id, role, content, action,
                     round_number, helpful)
                 VALUES ('ct1', 'c1', '{draft_id}', 'coach', '先写结论', 'question', 1, 0)"
            ),
        ];
        for sql in statements {
            self.repo
                .write(move |conn| {
                    conn.execute(&sql, [])?;
                    Ok(())
                })
                .unwrap();
        }
    }

    fn device_id(&self) -> String {
        insightforge_core::Settings::new(
            Arc::clone(&self.repo),
            Arc::new(insightforge_core::MemorySecretStore::default()),
            Arc::new(insightforge_core::UreqAiHttpClient::new()),
        )
        .device_id()
        .unwrap()
    }
}

#[test]
fn a_sync_bundle_carries_every_synced_author_asset() {
    let fx = Fixture::new();
    let draft_id = fx.seed_draft("# 标题\n\n正文。\n");
    fx.seed_other_assets(&draft_id);
    AuditLogger::new(Arc::clone(&fx.repo))
        .log("gpt-test", 1, &["f1".to_string()], "ok")
        .unwrap();

    let bundle = SyncBundle::build(&fx.repo, &fx.drafts, &fx.device_id()).unwrap();

    for &(table, _) in SYNCED_ASSETS {
        assert!(
            bundle.records.iter().any(|record| record.table == table),
            "{table} 是作者资产，必须出现在同步载荷里"
        );
    }
    assert_eq!(bundle.record_count, bundle.records.len());
    for record in &bundle.records {
        assert!(
            record.rev > 0,
            "{} 的每条资产都该有存储层打点的修订号",
            record.table
        );
        assert_eq!(
            record.origin_device.as_deref(),
            Some(fx.device_id().as_str()),
            "{} 的修订必须标明是哪台设备写的",
            record.table
        );
    }
}

#[test]
fn the_bundle_declares_its_counts_and_fields_before_anything_is_sent() {
    let fx = Fixture::new();
    let draft_id = fx.seed_draft("正文。\n");
    fx.seed_other_assets(&draft_id);

    let bundle = SyncBundle::build(&fx.repo, &fx.drafts, &fx.device_id()).unwrap();
    let declared = bundle.declared();

    assert_eq!(declared.len(), SYNCED_ASSETS.len(), "每张同步表都要申报");
    for entry in declared {
        let actual = bundle
            .records
            .iter()
            .filter(|record| record.table == entry.table)
            .count();
        assert_eq!(
            entry.record_count, actual,
            "{} 申报的条数必须等于实际条数，否则明示就是假的",
            entry.table
        );
        assert!(
            !entry.fields.is_empty(),
            "{} 必须说清带了哪些字段",
            entry.table
        );
    }
}

#[test]
fn no_synced_field_is_a_device_local_path_or_save_baseline() {
    let fx = Fixture::new();
    let draft_id = fx.seed_draft("正文。\n");
    fx.seed_other_assets(&draft_id);

    let bundle = SyncBundle::build(&fx.repo, &fx.drafts, &fx.device_id()).unwrap();
    let serialized = serde_json::to_string(&bundle).unwrap();

    // 白名单本身先守住：任何设备本地列进了清单就是设计违约。
    for &(_, columns) in SYNCED_ASSETS {
        for column in columns {
            assert!(
                !DEVICE_LOCAL_COLUMNS.contains(column),
                "{column} 是设备本地列，不能出现在同步白名单里"
            );
        }
    }
    for record in &bundle.records {
        for column in DEVICE_LOCAL_COLUMNS {
            assert!(
                !record.fields.contains_key(*column),
                "{} 的载荷里出现了设备本地列 {column}",
                record.table
            );
        }
    }
    // 端到端：草稿目录的绝对路径（就是 draft.path 的内容）不该在任何字节里出现。
    assert!(
        !serialized.contains(&fx.drafts_dir().to_string_lossy().to_string()),
        "同步载荷里出现了本机绝对路径"
    );
}

#[test]
fn draft_content_travels_but_its_filename_is_all_the_path_that_does() {
    let fx = Fixture::new();
    fx.seed_draft("# 标题\n\n只有这台机器上的文件知道的内容。\n");

    let bundle = SyncBundle::build(&fx.repo, &fx.drafts, &fx.device_id()).unwrap();
    let draft = bundle
        .records
        .iter()
        .find(|record| record.table == "draft")
        .expect("draft 记录");

    assert_eq!(draft.id, draft.fields["id"].as_str().unwrap());
    assert_eq!(draft.fields["filename"], "k1.md");
    assert!(
        draft.fields["content"]
            .as_str()
            .unwrap()
            .contains("只有这台机器上的文件知道的内容"),
        "草稿正文要能同步过去，否则网页端拿到的是空壳"
    );
    assert!(draft.fields.get("path").is_none());
}

#[test]
fn a_sync_is_audited_as_a_sync_and_not_as_an_ai_call() {
    let fx = Fixture::new();
    let logger = AuditLogger::new(Arc::clone(&fx.repo));

    logger
        .log("gpt-test", 3, &["f1".to_string()], "ok")
        .unwrap();
    logger.log_sync(2, &["k1.md".to_string()], "ok").unwrap();

    let records = logger.list(10).unwrap();
    assert_eq!(records[0].kind, "sync", "同步审计要能和 AI 调用分开看");
    assert_eq!(records[0].model, "sync");
    assert_eq!(records[0].fragment_count, 2);
    assert_eq!(records[1].kind, "ai", "既有的 AI 调用记录语义不变");
}
