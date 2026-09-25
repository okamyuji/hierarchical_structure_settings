# 階層型設定管理システム

Rustで実装された柔軟で強力な階層型設定管理システムです。JSON、TOML、環境変数からの設定読み込みをサポートし、ドット記法によるパス指定で階層的な設定値にアクセスできます。

## 特徴

- 階層構造 `database.host` のようなドット記法で設定値にアクセスできます。
- 複数の形式 JSON、TOML、環境変数から設定を読み込めます。
- 型 扱える型はString、Integer、Boolean、Arrayの4種類です。
- 設定検証 必須項目の有無と値の範囲を検査します。
- デフォルト値 アプリケーションの初期設定をまとめて適用できます。
- 環境変数による上書き 設定ファイルの値を環境変数で上書きできます。

## 使用方法

### 基本的な設定操作

```rust
use hierarchical_structure_settings::{ConfigManager, ConfigValue};

// 設定管理システムの初期化
let config = ConfigManager::new("myapp".to_string());

// 設定値の設定
config.set_config("database.host", ConfigValue::String("localhost".to_string()))?;
config.set_config("database.port", ConfigValue::Integer(5432))?;
config.set_config("debug.enabled", ConfigValue::Boolean(true))?;

// 設定値の取得
let host = config.get_config("database.host");
let port = config.get_config("database.port");

// 設定ツリーの表示
config.display_tree();
```

### 設定ファイルからの読み込み

```rust
use hierarchical_structure_settings::ConfigManager;
use hierarchical_structure_settings::config_loader::ConfigLoader;

let config = ConfigManager::new("myapp".to_string());

// デフォルト設定を適用
ConfigLoader::apply_defaults(&config)?;

// TOMLファイルから読み込み
ConfigLoader::load_from_toml(&config, "config.toml")?;

// 環境変数から読み込み（MYAPP_プレフィックス）
ConfigLoader::load_from_env(&config, "MYAPP_")?;

// 設定の検証
let warnings = ConfigLoader::validate_config(&config)?;
if !warnings.is_empty() {
    for warning in warnings {
        eprintln!("Warning: {}", warning);
    }
}
```

### 複数設定ファイルの読み込み

```rust
// 複数ファイルを順次読み込み（後勝ち）
let config_files = vec![
    "defaults.toml",
    "config.toml", 
    "local.toml"
];

ConfigLoader::load_multiple(&config, config_files)?;
```

## 設定ファイル例

### config.toml

```toml
[app]
name = "MyApplication"
version = "1.0.0"

[database]
host = "localhost"
port = 5432
username = "admin"
ssl_enabled = true

[database.pool]
min_connections = 5
max_connections = 50

[debug]
enabled = false
log_level = "INFO"
```

### config.json

```json
{
  "app": {
    "name": "MyApplication",
    "version": "1.0.0"
  },
  "database": {
    "host": "localhost",
    "port": 5432,
    "pool": {
      "min_connections": 5,
      "max_connections": 50
    }
  }
}
```

## 環境変数による上書き

次の環境変数を設定すると、設定ファイルの値を上書きできます。

```bash
# MYAPP_DATABASE_HOST=production.db.example.com
# MYAPP_DATABASE_PORT=5433
# MYAPP_DEBUG_ENABLED=true

export MYAPP_DATABASE_HOST="production.db.example.com"
export MYAPP_DATABASE_PORT=5433
export MYAPP_DEBUG_ENABLED=true
```

## 設定項目一覧

### アプリケーション設定

- `app.name`: アプリケーション名
- `app.version`: バージョン
- `app.description`: 説明

### データベース設定

- `database.host`: ホスト名
- `database.port`: ポート番号
- `database.username`: ユーザー名
- `database.password`: パスワード
- `database.ssl_enabled`: SSL有効化
- `database.pool.min_connections`: 最小接続数
- `database.pool.max_connections`: 最大接続数

### サーバー設定

- `server.host`: バインドホスト
- `server.port`: リッスンポート  
- `server.worker_threads`: ワーカースレッド数
- `server.tls.enabled`: TLS有効化
- `server.tls.cert_file`: 証明書ファイル
- `server.tls.key_file`: 秘密鍵ファイル

### デバッグ設定

- `debug.enabled`: デバッグモード有効化
- `debug.log_level`: ログレベル（DEBUG/INFO/WARN/ERROR）
- `debug.verbose_logging`: 詳細ログ出力
- `debug.output.console`: コンソール出力
- `debug.output.file`: ファイル出力

### キャッシュ設定

- `cache.enabled`: キャッシュ有効化
- `cache.type`: キャッシュタイプ（redis/memory）
- `cache.ttl_seconds`: TTL（秒）
- `cache.redis.host`: Redisホスト
- `cache.redis.port`: Redisポート

### セキュリティ設定

- `security.jwt_secret`: JWT秘密鍵
- `security.token_expiry_hours`: トークン有効期限
- `security.rate_limiting_enabled`: レート制限有効化
- `security.cors_enabled`: CORS有効化

### 通知設定

- `notifications.enabled`: 通知有効化
- `notifications.channels`: 通知チャンネル配列
- `notifications.slack.webhook_url`: Slack Webhook URL
- `notifications.webhook.url`: Webhook URL

## Cargo.toml依存関係

```toml
[dependencies]
serde_json = "1.0"
toml = "1.1"
uuid = { version = "1.26", features = ["v4"] }
```

このクレートはRust edition 2024でビルドします。

## テスト

```bash
cargo test
```

## ライセンス

MIT License

## 貢献

不具合の報告はIssueで、改善の提案はプルリクエストで受け付けます。
