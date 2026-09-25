use std::process::Command;

#[test]
fn demo_binary_loads_config_files_and_env() {
    let output = Command::new(env!("CARGO_BIN_EXE_hierarchical_structure_settings"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("APP_DEMOCHECK_VALUE", "77")
        .env("APPX_LEAK", "1")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "✓ config.tomlの読み込み完了",
        "✓ config.jsonの読み込み完了",
        "✓ 環境変数の読み込み完了",
        "\n    value= Integer(77)",
        "✓ 実行時設定の追加完了",
        "アプリケーション名: String(\"HierarchicalConfigApp\")",
        "  ログレベル: DEBUG",
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
}
