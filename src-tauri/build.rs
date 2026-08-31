fn main() {
    // Release 构建必须设置以下环境变量，否则 updater 配置不完整
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

    tauri_build::build()
}
