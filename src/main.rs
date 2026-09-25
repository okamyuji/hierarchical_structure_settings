use hierarchical_structure_settings::config_loader::ConfigLoader;
use hierarchical_structure_settings::{ConfigManager, ConfigValue};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 階層型設定管理システム デモ ===");
    println!();

    // 設定管理システムを初期化
    let config = ConfigManager::new("DemoApp".to_string());

    // 1. 外部設定ファイルからの読み込み
    println!("1. 外部設定ファイルからの読み込み:");
    load_config_files(&config)?;

    // 設定ツリーを表示
    println!("\n外部ファイルから読み込んだ設定ツリー:");
    config.display_tree();

    // 2. プログラムでの追加設定（外部ファイルに加えて）
    println!("\n2. プログラムでの追加設定:");
    add_runtime_config(&config)?;

    // 3. 設定値の取得と使用例
    println!("\n3. 設定値の取得例:");
    demonstrate_config_usage(&config);

    // 4. 設定値の更新例
    println!("\n4. 設定値の更新例:");
    demonstrate_config_updates(&config)?;

    // 5. 配列設定の例
    println!("\n5. 配列設定の例:");
    demonstrate_array_config(&config)?;

    println!("\n=== デモ完了 ===");
    Ok(())
}

/// 外部設定ファイルから設定を読み込む
fn load_config_files(config: &ConfigManager) -> Result<(), Box<dyn std::error::Error>> {
    // TOMLファイルの読み込み
    if std::path::Path::new("config.toml").exists() {
        println!("  config.tomlから設定を読み込んでいます...");
        ConfigLoader::load_from_toml(config, "config.toml")?;
        println!("  ✓ config.tomlの読み込み完了");
    } else {
        println!("  ⚠ config.tomlが見つかりません");
    }

    // JSONファイルの読み込み（追加設定として）
    if std::path::Path::new("config.json").exists() {
        println!("  config.jsonから追加設定を読み込んでいます...");
        ConfigLoader::load_from_json(config, "config.json")?;
        println!("  ✓ config.jsonの読み込み完了");
    } else {
        println!("  ⚠ config.jsonが見つかりません");
    }

    // 環境変数からの読み込み
    println!("  環境変数から設定を読み込んでいます (APP_プレフィックス)...");
    match ConfigLoader::load_from_env(config, "APP_") {
        Ok(_) => println!("  ✓ 環境変数の読み込み完了"),
        Err(e) => println!("  ⚠ 環境変数読み込みエラー: {}", e),
    }

    Ok(())
}

/// プログラムでの追加設定を行う（外部ファイルの設定に加えて）
fn add_runtime_config(config: &ConfigManager) -> Result<(), String> {
    println!("  実行時に追加の設定をプログラムから行います:");

    // 実行時にのみ決まる動的設定
    config.set_config(
        "runtime.session_id",
        ConfigValue::String(uuid::Uuid::new_v4().to_string()),
    )?;
    config.set_config(
        "runtime.start_time",
        ConfigValue::Integer(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        ),
    )?;
    config.set_config(
        "runtime.pid",
        ConfigValue::Integer(std::process::id() as i64),
    )?;

    // デバッグモードを開発中として強制有効化（外部設定を上書き）
    config.set_config("debug.enabled", ConfigValue::Boolean(true))?;
    config.set_config("debug.log_level", ConfigValue::String("DEBUG".to_string()))?;

    // 追加の機能フラグ（実験的機能）
    config.set_config("features.experimental_ui", ConfigValue::Boolean(true))?;
    config.set_config(
        "features.performance_monitoring",
        ConfigValue::Boolean(true),
    )?;

    println!("  ✓ 実行時設定の追加完了");
    Ok(())
}

/// 設定値の取得と使用方法のデモ
fn demonstrate_config_usage(config: &ConfigManager) {
    // 基本的な設定値の取得
    if let Some(app_name) = config.get_config("app.name") {
        println!("アプリケーション名: {:?}", app_name);
    }

    if let Some(ConfigValue::String(env)) = config.get_config("app.environment") {
        println!("実行環境: {}", env);
    }

    // データベース接続情報の取得
    let db_host = config.get_config("database.host");
    let db_port = config.get_config("database.port");
    let db_name = config.get_config("database.database_name");

    println!("\nデータベース接続情報:");
    println!("  ホスト: {:?}", db_host);
    println!("  ポート: {:?}", db_port);
    println!("  データベース: {:?}", db_name);

    // サーバー設定の取得
    if let (Some(ConfigValue::String(host)), Some(ConfigValue::Integer(port))) = (
        config.get_config("server.host"),
        config.get_config("server.port"),
    ) {
        println!("\nサーバー設定:");
        println!("  バインドアドレス: {}:{}", host, port);
    }

    // デバッグ設定の確認
    if let Some(ConfigValue::Boolean(debug_enabled)) = config.get_config("debug.enabled") {
        println!(
            "\nデバッグモード: {}",
            if debug_enabled { "有効" } else { "無効" }
        );

        if debug_enabled
            && let Some(ConfigValue::String(log_level)) = config.get_config("debug.log_level")
        {
            println!("  ログレベル: {}", log_level);
        }
    }

    // 機能フラグの確認
    println!("\n有効な機能:");
    let features = vec![
        ("user_registration", "ユーザー登録"),
        ("email_verification", "メール認証"),
        ("two_factor_auth", "二段階認証"),
        ("admin_panel", "管理パネル"),
        ("api_v2", "API v2"),
    ];

    for (feature_key, feature_name) in features {
        let path = format!("features.{}", feature_key);
        if let Some(ConfigValue::Boolean(enabled)) = config.get_config(&path) {
            if enabled {
                println!("  ✓ {}", feature_name);
            } else {
                println!("  ✗ {}", feature_name);
            }
        }
    }
}

/// 設定値の更新デモ
fn demonstrate_config_updates(config: &ConfigManager) -> Result<(), String> {
    println!("デバッグモードを無効化...");

    // 現在の値を表示
    if let Some(current_debug) = config.get_config("debug.enabled") {
        println!("現在のデバッグ設定: {:?}", current_debug);
    }

    config.update_config("debug.enabled", ConfigValue::Boolean(false))?;
    config.update_config("debug.log_level", ConfigValue::String("INFO".to_string()))?;

    println!("デバッグ設定を更新しました");

    // 更新後の値を確認
    if let Some(updated_debug) = config.get_config("debug.enabled") {
        println!("更新後のデバッグ設定: {:?}", updated_debug);
    }

    // ログレベルも確認
    if let Some(updated_log_level) = config.get_config("debug.log_level") {
        println!("更新後のログレベル: {:?}", updated_log_level);
    }

    Ok(())
}

/// 配列設定のデモ
fn demonstrate_array_config(config: &ConfigManager) -> Result<(), String> {
    // 許可されたオリジンのリストを設定
    let allowed_origins = vec![
        ConfigValue::String("https://example.com".to_string()),
        ConfigValue::String("https://app.example.com".to_string()),
        ConfigValue::String("https://admin.example.com".to_string()),
    ];
    config.set_config(
        "security.cors.allowed_origins",
        ConfigValue::Array(allowed_origins),
    )?;

    // サポートする言語のリストを設定
    let supported_locales = vec![
        ConfigValue::String("ja_JP".to_string()),
        ConfigValue::String("en_US".to_string()),
        ConfigValue::String("zh_CN".to_string()),
        ConfigValue::String("ko_KR".to_string()),
    ];
    config.set_config(
        "i18n.supported_locales",
        ConfigValue::Array(supported_locales),
    )?;

    // 通知チャンネルのリストを設定
    let notification_channels = vec![
        ConfigValue::String("email".to_string()),
        ConfigValue::String("slack".to_string()),
        ConfigValue::String("webhook".to_string()),
    ];
    config.set_config(
        "notifications.channels",
        ConfigValue::Array(notification_channels),
    )?;

    // 設定した配列の内容を表示
    println!("設定した配列:");

    if let Some(ConfigValue::Array(origins)) = config.get_config("security.cors.allowed_origins") {
        println!("  許可されたオリジン:");
        for origin in origins {
            if let ConfigValue::String(url) = origin {
                println!("    - {}", url);
            }
        }
    }

    if let Some(ConfigValue::Array(locales)) = config.get_config("i18n.supported_locales") {
        println!("  サポート言語:");
        for locale in locales {
            if let ConfigValue::String(lang) = locale {
                println!("    - {}", lang);
            }
        }
    }

    if let Some(ConfigValue::Array(channels)) = config.get_config("notifications.channels") {
        println!("  通知チャンネル:");
        for channel in channels {
            if let ConfigValue::String(ch) = channel {
                println!("    - {}", ch);
            }
        }
    }

    Ok(())
}
