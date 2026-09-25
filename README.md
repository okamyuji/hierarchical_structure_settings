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

`display_tree`は設定ツリー全体を標準出力に表示する関数です。1つの値だけを表示したい場合は、同じ規則に従う`get_display`が使えます。どちらの関数も、`allow_display`で許可したパスの値だけを表示し、それ以外の値は`***`に置き換えます。既定では何も許可していないので、登録しなければすべての値が隠れる仕組みです。

```rust
config.allow_display("server.port"); // このパスだけを表示する
config.allow_display("features.*");  // features 配下のすべてを表示する
```

パターンの書き方は2通りです。完全なパスを書いた場合は、そのパスにだけ一致します。末尾を`.*`にした場合は、その配下のあらゆる深さのパスに一致しますが、`features`そのものや`featuresx.a`のような別名には一致しません。

この方式は、秘密値かどうかを名前や値の形から推測しない設計です。表示してよい項目を開発者が明示するので、見分け方に抜け道が生じる余地がなくなります。その代わり、許可したパスに秘密値を入れれば、そのまま表示されます。パスワードや接続文字列が入り得るパスは、許可リストに加えないでください。

なお、TOMLの構文エラーは行番号と列番号だけを報告し、誤りのある行の中身はエラー文に含めません。

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
