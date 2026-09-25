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
let warnings = ConfigLoader::validate_config(&config);
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

`load_from_env`は、渡された接頭辞の末尾に`_`を補ってから変数名と照合します。`MYAPP`と`MYAPP_`のどちらを渡しても結果は同じです。`MYAPPX_HOST`のように接頭辞の直後が`_`でない変数は読み込みません。変数名の中の`_`は階層の区切りとして扱われるので、`MYAPP_DATABASE_HOST`の値は`database.host`に入ります。接頭辞の直後に続く余分な`_`は無視する仕様で、`MYAPP__HOST`は`host`になります。UTF-8として読めない変数と、`MYAPP_`や`MYAPP_A__B`のように空の階層ができる変数は、読み飛ばす対象です。

## 設定値の表示と秘密値の扱い

`display_tree`は設定ツリー全体を標準出力に表示する関数です。1つの値だけを表示したい場合は、同じ規則で秘密値を隠す`get_display`が使えます。どちらの関数でも、パスワードやトークンのような秘密値は`***`に置き換えて表示する仕組みです。

秘密値かどうかの判定には、設定のフルパスを小文字にし、`.`と`-`を`_`に置き換えた文字列を使います。この文字列を`_`で語に分け、`password`、`pass`、`pwd`、`secret`、`token`、`credential`、`key`、`auth`、`authorization`、`cookie`、`salt`、`pat`、`dsn`、`bearer`、`webhook`などと一致する語があれば隠す対象です。語の末尾の数字と複数形の`s`は取り除いてから比べるので、`password2`や`refresh_tokens`も隠れます。`accessToken`や`dbpassword`のように区切りなしでつながった名前は、`token`や`password`などの語を部分一致でも探して拾います。値の型は問いません。

値の側も確認しています。`postgres://user:pw@host/db`や`https://TOKEN@github.com`のように、`://`の後ろに空でないユーザー情報を持つURLは、キー名にかかわらず隠れます。配列の中にこの形のURLが1つでもあれば、配列全体が隠れる仕組みです。

判定は取りこぼしより隠しすぎを選ぶ方針です。ただし、最後の語が`hours`、`seconds`、`attempts`、`reset`、`file`、`path`、`enabled`のどれかであれば、期限や回数、フラグ、ファイルの場所を表すキーとみなして隠しません。そのため`token_expiry_hours`や`password_reset`はそのまま表示されます。

このマスクは名前と値の形に頼る補助的な仕組みで、すべての秘密値を確実に見分けられるわけではありません。たとえば全角文字で書かれたキー名は判定をすり抜けます。秘密値を含む設定ツリーを、本番環境のログへ出力しないでください。

同梱の`config.toml`と`config.json`では、秘密値の欄に`CHANGE_ME`という仮の値を入れてあります。実際の値は環境変数で渡すか、`.gitignore`で除外される`config.production.toml`などのファイルに書いてください。

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

このクレートはRust edition 2024でビルドします。コードが`if let`を`&&`でつなぐlet chainsを使っているので、Rust 1.88以上が必要です。

## テスト

```bash
cargo test
```

## ライセンス

このクレートはMIT Licenseで公開しています。全文は[LICENSE](LICENSE)にあります。

## 貢献

不具合の報告はIssueで、改善の提案はプルリクエストで受け付けます。
