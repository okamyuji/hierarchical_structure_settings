use std::process::Command;

#[test]
fn demo_binary_loads_config_files_and_env() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_hierarchical_structure_settings"));
    // env_clear() だとカバレッジ計測用の変数まで消えるので、デモが読む APP* だけを外す。
    for (key, _) in std::env::vars().filter(|(k, _)| k.starts_with("APP")) {
        command.env_remove(key);
    }
    let output = command
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("APP_DEMOCHECK_VALUE", "77")
        .env("APPX_LEAK", "1")
        .env("APP_DB_PASSWORD", "31415926535")
        .env("APP_STORAGE_S3_SECRET_KEY", "AKIAREALSECRET")
        .env("APP_API_TOKEN", "tok123")
        .env("APP_DATABASE_HOST", "postgres://u:hostpw@db/app")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "✓ config.tomlの読み込み完了",
        "✓ config.jsonの読み込み完了",
        "✓ 環境変数の読み込み完了",
        "データベース: String(\"config_app_db\")",
        "ホスト: ***",
        "password_reset= Boolean(true)",
        "token_expiry_hours= Integer(24)",
        "\n    value= Integer(77)",
        "✓ 実行時設定の追加完了",
        "アプリケーション名: String(\"HierarchicalConfigApp\")",
        "  ログレベル: DEBUG",
        "  ✓ ユーザー登録",
        "  ✗ API v2",
        "更新後のデバッグ設定: Boolean(false)",
        "    - https://admin.example.com",
        "    - ko_KR",
        "    - webhook",
        "=== デモ完了 ===",
    ] {
        assert!(
            stdout.contains(expected),
            "missing {expected:?} in:\n{stdout}"
        );
    }
    assert!(
        !stdout.contains("leak= "),
        "APPX_ must not be loaded:\n{stdout}"
    );
    for secret in [
        "CHANGE_ME",
        "31415926535",
        "AKIAREALSECRET",
        "tok123",
        "hostpw",
    ] {
        assert!(!stdout.contains(secret), "{secret} leaked:\n{stdout}");
    }
    assert!(stdout.contains("password= ***"), "{stdout}");
    let position = |needle: &str| stdout.find(needle).unwrap_or_else(|| panic!("{needle}"));
    assert!(
        position("\n  app/") < position("\n  database/"),
        "children must be sorted"
    );
}
