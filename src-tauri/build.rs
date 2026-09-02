fn main() {
    // Release 构建必须设置以下环境变量，否则 updater 配置不完整（fail-closed）
    if cfg!(not(debug_assertions)) {
        let required = [
            "INSIGHTFORGE_UPDATER_PUBKEY",
            "INSIGHTFORGE_UPDATER_ENDPOINT",
            "INSIGHTFORGE_CERT_THUMBPRINT",
        ];
        let mut missing = Vec::new();
        for key in &required {
            if std::env::var(key).is_err() {
                missing.push(*key);
            }
        }
        if !missing.is_empty() {
            panic!(
                "Release build requires updater env vars: {:?}. \
                 Set them in CI/CD or local signing environment.",
                missing
            );
        }
    }

    // 通过 TAURI_CONFIG 覆盖 plugins.updater.endpoints。
    // 原因：tauri.conf.json 中的 ${env.VAR} 不会被 tauri-codegen 插值（plugin config 原样嵌入二进制），
    // 而 endpoints 必须是合法 URL，否则 updater 插件反序列化 panic 导致应用无法启动。
    // 配置文件中使用 dev 占位 URL（https://localhost/updater-dev.json）保证 dev 无 env 也能启动；
    // 此处若设置了 INSIGHTFORGE_UPDATER_ENDPOINT，则用真实值覆盖。
    if let Ok(endpoint) = std::env::var("INSIGHTFORGE_UPDATER_ENDPOINT") {
        if !endpoint.is_empty() {
            // 构造仅覆盖 plugins.updater.endpoints 的 JSON
            let override_val = serde_json::json!({
                "plugins": {
                    "updater": {
                        "endpoints": [endpoint]
                    }
                }
            });

            // 若 TAURI_CONFIG 已存在（如 Tauri CLI 设置），合并之；否则直接设置
            let merged = match std::env::var("TAURI_CONFIG") {
                Ok(existing) => {
                    let mut existing_val: serde_json::Value =
                        serde_json::from_str(&existing).unwrap_or(serde_json::Value::Null);
                    merge_json(&mut existing_val, &override_val);
                    existing_val.to_string()
                }
                Err(_) => override_val.to_string(),
            };

            std::env::set_var("TAURI_CONFIG", merged);
            println!(
                "cargo:warning=Overriding plugins.updater.endpoints via TAURI_CONFIG to {endpoint}"
            );
        }
    }

    tauri_build::build()
}

/// 简易 JSON merge patch（RFC 7396 子集）：将 patch 合并到 target。
fn merge_json(target: &mut serde_json::Value, patch: &serde_json::Value) {
    match (target, patch) {
        (serde_json::Value::Object(target_map), serde_json::Value::Object(patch_map)) => {
            for (key, value) in patch_map {
                if value.is_null() {
                    target_map.remove(key);
                } else {
                    merge_json(
                        target_map
                            .entry(key.clone())
                            .or_insert(serde_json::Value::Null),
                        value,
                    );
                }
            }
        }
        (target, patch) => {
            *target = patch.clone();
        }
    }
}
