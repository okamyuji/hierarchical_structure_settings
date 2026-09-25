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
        .env("APP_APP_SECRET", "appsec")
        .env("APP_FEATURES_API_TOKEN", "feattok")
        .env("APP_DATABASE_HOST", "postgres://u:hostpw@db/app")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "✓ config.tomlの読み込み完了",
        "✓ config.jsonの読み込み完了",
        "✓ 環境変数の読み込み完了",
        "データベース: config_app_db",
        "バインドアドレス: 0.0.0.0:8080",
        "ホスト: ***",
        "password_reset= ***",
        "database_name= config_app_db",
        "token_expiry_hours= ***",
        "\n    value= ***",
        "✓ 実行時設定の追加完了",
        "アプリケーション名: HierarchicalConfigApp",
        "  ログレベル: DEBUG",
        "  ✓ ユーザー登録",
        "  ✗ API v2",
        "更新後のデバッグ設定: false",
        "現在のデバッグ設定: true",
        "更新後のログレベル: INFO",
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
        "appsec",
        "feattok",
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
