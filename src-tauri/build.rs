/// dev 占位 endpoint：release 构建里出现它意味着「这个包永远收不到更新」。
const DEV_UPDATER_ENDPOINT: &str = "https://localhost/updater-dev.json";

fn main() {
    // Release 构建的 updater 必须指向真实清单地址（fail-closed）。
    // 签名公钥不在此列——它是公开数据，随 `tauri.conf.json` 进仓库；
    // 私钥才是 CI secret，缺了它产不出 .sig，客户端同样会拒绝更新。
    if cfg!(not(debug_assertions)) {
        match std::env::var("INSIGHTFORGE_UPDATER_ENDPOINT") {
            Ok(endpoint) if !endpoint.is_empty() && endpoint != DEV_UPDATER_ENDPOINT => {}
            _ => panic!(
                "Release build requires INSIGHTFORGE_UPDATER_ENDPOINT to point at a real \
                 updater manifest (got {:?}). Set it in CI/CD.",
                std::env::var("INSIGHTFORGE_UPDATER_ENDPOINT").ok()
            ),
        }
        if std::env::var("INSIGHTFORGE_CERT_THUMBPRINT")
            .unwrap_or_default()
            .is_empty()
        {
            println!(
                "cargo:warning=INSIGHTFORGE_CERT_THUMBPRINT 未设置：安装包不签名，\
                 首次下载会有 SmartScreen 提示（updater 校验不受影响）。"
            );
        }
    }

    // 构建期注入 TAURI_CONFIG：updater endpoint 与（可选的）Windows 代码签名证书。
    //
    // 为什么不在 `tauri.conf.json` 里写 `${env.VAR}`：`${env.}` 只对少数字段生效，
    // 其余原样嵌入——endpoint 会以字面量进二进制（updater 插件反序列化直接 panic），
    // certificateThumbprint 则会把 `${env.INSIGHTFORGE_CERT_THUMBPRINT}` 当成证书名交给
    // signtool，导致「没有代码签名证书就无法出包」。真源在这里，缺省时就不注入。
    let mut override_val = serde_json::Map::new();

    if let Ok(endpoint) = std::env::var("INSIGHTFORGE_UPDATER_ENDPOINT") {
        if !endpoint.is_empty() {
            override_val.insert(
                "plugins".to_string(),
                serde_json::json!({ "updater": { "endpoints": [endpoint] } }),
            );
            println!("cargo:warning=updater endpoint 覆盖为 {endpoint}");
        }
    }

    let thumbprint = std::env::var("INSIGHTFORGE_CERT_THUMBPRINT").unwrap_or_default();
    if !thumbprint.is_empty() {
        override_val.insert(
            "bundle".to_string(),
            serde_json::json!({
                "windows": {
                    "certificateThumbprint": thumbprint,
                    "timestampUrl": "https://timestamp.digicert.com"
                }
            }),
        );
        println!("cargo:warning=使用代码签名证书 {thumbprint}");
    }

    if !override_val.is_empty() {
        let patch = serde_json::Value::Object(override_val);
        let merged = match std::env::var("TAURI_CONFIG") {
            // Tauri CLI 可能已经设置过 TAURI_CONFIG，合并而非替换。
            Ok(existing) => {
                let mut existing_val: serde_json::Value =
                    serde_json::from_str(&existing).unwrap_or(serde_json::Value::Null);
                merge_json(&mut existing_val, &patch);
                existing_val.to_string()
            }
            Err(_) => patch.to_string(),
        };
        // 两份都要：本进程内 `tauri_build::build()` 读 set_var 后的环境；
        // 而 `tauri::generate_context!` 由 rustc 展开，只看得到 cargo:rustc-env——
        // 只 set_var 的话，注入的配置进不了二进制，应用会一直去问那个 dev 占位地址。
        std::env::set_var("TAURI_CONFIG", &merged);
        println!("cargo:rustc-env=TAURI_CONFIG={merged}");
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
