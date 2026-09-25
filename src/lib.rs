//! ドット区切りのパスで値を読み書きする、階層型の設定管理ライブラリです。

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};

pub mod config_loader;

/// 設定ツリーの葉に入る値。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValue {
    /// 文字列。
    String(String),
    /// 整数。範囲は `i64`。
    Integer(i64),
    /// 真偽値。
    Boolean(bool),
    /// 値の配列。要素の型は混在してよい。
    Array(Vec<ConfigValue>),
}

impl std::fmt::Display for ConfigValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigValue::String(s) => write!(f, "{s}"),
            ConfigValue::Integer(i) => write!(f, "{i}"),
            ConfigValue::Boolean(b) => write!(f, "{b}"),
            ConfigValue::Array(items) => {
                let items: Vec<String> = items.iter().map(ToString::to_string).collect();
                write!(f, "[{}]", items.join(", "))
            }
        }
    }
}

/// 設定ツリーのノード。子は名前順に保持し、親は `Weak` で参照して循環参照を避ける。
#[derive(Debug)]
pub struct ConfigNode {
    name: String,
    value: RefCell<Option<ConfigValue>>,
    children: RefCell<BTreeMap<String, Rc<ConfigNode>>>,
    parent: RefCell<Weak<ConfigNode>>,
}

impl ConfigNode {
    /// 親を持たないルートノードを作る。
    pub fn new_root(name: String) -> Rc<Self> {
        Rc::new(ConfigNode {
            name,
            value: RefCell::new(None),
            children: RefCell::new(BTreeMap::new()),
            parent: RefCell::new(Weak::new()),
        })
    }

    /// 子ノードに値を設定して返す。同名の子があれば、その子の配下を残したまま値だけを入れ替える。
    pub fn add_child(self: &Rc<Self>, name: String, value: Option<ConfigValue>) -> Rc<ConfigNode> {
        let child = self.get_or_create_child(&name);
        child.value.replace(value);
        child
    }

    fn get_or_create_child(self: &Rc<Self>, name: &str) -> Rc<ConfigNode> {
        if let Some(child) = self.children.borrow().get(name) {
            return child.clone();
        }
        let child = Rc::new(ConfigNode {
            name: name.to_string(),
            value: RefCell::new(None),
            children: RefCell::new(BTreeMap::new()),
            parent: RefCell::new(Rc::downgrade(self)),
        });
        self.children
            .borrow_mut()
            .insert(name.to_string(), child.clone());
        child
    }

    /// `database.host` のようなパスで子孫の値を取得する。
    pub fn get_value(&self, path: &str) -> Option<ConfigValue> {
        let parts: Vec<&str> = path.split('.').collect();
        self.get_value_recursive(&parts)
    }

    fn get_value_recursive(&self, path: &[&str]) -> Option<ConfigValue> {
        if path.is_empty() {
            return self.value.borrow().clone();
        }

        let children = self.children.borrow();
        if let Some(child) = children.get(path[0]) {
            child.get_value_recursive(&path[1..])
        } else {
            None
        }
    }

    /// 親ノードを返す。ルートと、親が破棄されたノードは `None`。
    pub fn get_parent(&self) -> Option<Rc<ConfigNode>> {
        self.parent.borrow().upgrade()
    }

    /// ノード名を返す。
    pub fn name(&self) -> &str {
        &self.name
    }
}

const SENSITIVE_WORDS: &[&str] = &[
    "password",
    "passwd",
    "pass",
    "passphrase",
    "secret",
    "token",
    "credential",
    "key",
    "apikey",
    "pat",
    "dsn",
    "bearer",
    "webhook",
    "pwd",
    "auth",
    "authorization",
    "cookie",
    "salt",
    "jwt",
    "pin",
    "otp",
    "signature",
    "sig",
    "private",
];

// `dbpassword` のように区切りなしで連結された名前も拾う。
const SENSITIVE_FRAGMENTS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "credential",
    "apikey",
    "token",
    "privatekey",
    "signingkey",
    "accesskey",
];

// 整数の値で末尾がこれらの語なら、秘密値ではなく期限、回数、ファイルの場所などとみなす。
const NON_SECRET_LAST_WORDS: &[&str] = &["hours", "seconds", "attempts", "file", "path"];

// 環境変数は `_` を `.` に変えて読み込むので、区切りを `_` に揃えて語ごとに照合する。
fn path_words(path: &str) -> (String, Vec<String>) {
    let normalized = split_camel_case(path).replace(['.', '-'], "_");
    let words = normalized
        .split('_')
        .map(|w| w.trim_end_matches(|c: char| c.is_ascii_digit()).to_string())
        .collect();
    (normalized, words)
}

// 取りこぼしより隠しすぎを選ぶ。末尾の語による例外は考慮しない。
fn mentions_secret(path: &str) -> bool {
    let (normalized, words) = path_words(path);
    let is_word = |w: &str| {
        SENSITIVE_WORDS.contains(&w)
            || w.strip_suffix('s')
                .is_some_and(|s| SENSITIVE_WORDS.contains(&s))
    };
    words.iter().any(|w| is_word(w)) || SENSITIVE_FRAGMENTS.iter().any(|f| normalized.contains(f))
}

fn is_sensitive(path: &str) -> bool {
    let (_, words) = path_words(path);
    let allowed = words
        .last()
        .is_some_and(|w| NON_SECRET_LAST_WORDS.contains(&w.as_str()));
    mentions_secret(path) && !allowed
}

// 小文字から大文字への境目と、`SSHKey` の `H|K` のような略語の終わりに `_` を入れて小文字にする。
fn split_camel_case(path: &str) -> String {
    let chars: Vec<char> = path.chars().collect();
    let mut out = String::with_capacity(path.len());
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() && i > 0 {
            let prev = chars[i - 1];
            let next_is_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            if prev.is_lowercase()
                || prev.is_ascii_digit()
                || (prev.is_uppercase() && next_is_lower)
            {
                out.push('_');
            }
        }
        out.extend(c.to_lowercase());
    }
    out
}

// `Server=h;Password=X`、`?token=X`、`Password = X` のような `key=value` の key を名前と同じ規則で調べる。
fn has_sensitive_assignment(s: &str) -> bool {
    s.match_indices('=').any(|(i, _)| {
        let key = s[..i]
            .trim_end()
            .rsplit(|c: char| matches!(c, ';' | '&' | '?' | ',') || c.is_whitespace())
            .next()
            .unwrap_or_default();
        !key.is_empty() && mentions_secret(key)
    })
}

// パスワードの無い `https://TOKEN@host` や `@` を含む不正な形も、隠す側に倒して扱う。
fn value_has_credentials(value: &ConfigValue) -> bool {
    match value {
        ConfigValue::String(s) => {
            let url_userinfo = s
                .split_once("://")
                .and_then(|(_, rest)| rest.rsplit_once('@'))
                .is_some_and(|(userinfo, _)| !userinfo.is_empty());
            url_userinfo || has_sensitive_assignment(s)
        }
        ConfigValue::Array(items) => items.iter().any(value_has_credentials),
        ConfigValue::Integer(_) | ConfigValue::Boolean(_) => false,
    }
}

// 真偽値は1ビットの情報しか持たず秘密値になり得ないので、キー名にかかわらず表示する。
// 末尾の語による例外は整数にだけ使い、文字列は `db.password.file` のような名前でも隠す。
fn should_mask(path: &str, value: &ConfigValue) -> bool {
    match value {
        ConfigValue::Boolean(_) => false,
        ConfigValue::Integer(_) => is_sensitive(path),
        ConfigValue::String(_) | ConfigValue::Array(_) => {
            mentions_secret(path) || value_has_credentials(value)
        }
    }
}

/// 設定ツリー全体を管理する。
///
/// パスの誤りなど、この型が返すエラーは `String`。ファイルを読む `ConfigLoader` は
/// `Box<dyn Error>` を返し、`?` でこの `String` をそのまま受け取れる。
pub struct ConfigManager {
    root: Rc<ConfigNode>,
}

impl ConfigManager {
    /// 指定した名前のルートを持つ空の設定ツリーを作る。
    pub fn new(root_name: String) -> Self {
        ConfigManager {
            root: ConfigNode::new_root(root_name),
        }
    }

    /// パスに値を設定する。途中のノードは必要に応じて作り、既存ノードの配下は残す。
    ///
    /// 空のセグメントを含むパス（`""`、`a..b`、`.a`、`a.`）はエラーになる。
    pub fn set_config(&self, path: &str, value: ConfigValue) -> Result<(), String> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.iter().any(|p| p.is_empty()) {
            return Err(format!("Invalid config path: {path:?}"));
        }
        // split は常に1つ以上の要素を返すので、parts は空にならない。
        let (parents, last) = parts.split_at(parts.len() - 1);
        let mut node = self.root.clone();
        for part in parents {
            node = node.get_or_create_child(part);
        }
        node.add_child(last[0].to_string(), Some(value));
        Ok(())
    }

    /// 表示用の文字列を返す。`display_tree` と同じ規則で秘密値は `***` になる。
    pub fn get_display(&self, path: &str) -> Option<String> {
        let value = self.get_config(path)?;
        Some(if should_mask(path, &value) {
            "***".to_string()
        } else {
            value.to_string()
        })
    }

    /// パスの値を取得する。値を持たない中間ノードは `None`。
    pub fn get_config(&self, path: &str) -> Option<ConfigValue> {
        self.root.get_value(path)
    }

    /// 設定ツリーを標準出力に表示する。秘密値らしい名前、資格情報入りの URL、
    /// `password=` のような代入を含む値は `***` で隠す。真偽値は常に表示する。
    pub fn display_tree(&self) {
        self.display_node(&self.root, 0, "");
    }

    fn display_node(&self, node: &ConfigNode, depth: usize, path: &str) {
        let indent = "  ".repeat(depth);
        match &*node.value.borrow() {
            Some(value) if should_mask(path, value) => {
                println!("{}{}= ***", indent, node.name())
            }
            Some(value) => println!("{}{}= {}", indent, node.name(), value),
            None => println!("{}{}/", indent, node.name()),
        }

        for child in node.children.borrow().values() {
            let child_path = if path.is_empty() {
                child.name().to_string()
            } else {
                format!("{}.{}", path, child.name())
            };
            self.display_node(child, depth + 1, &child_path);
        }
    }

    /// 既存パスの値を入れ替える。パスが無ければフルパスを含むエラーを返す。
    pub fn update_config(&self, path: &str, value: ConfigValue) -> Result<(), String> {
        let mut node = self.root.clone();
        for part in path.split('.') {
            let child = node.children.borrow().get(part).cloned();
            node = child.ok_or_else(|| format!("Path not found: {path:?}"))?;
        }
        node.value.replace(Some(value));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_management() {
        let config = ConfigManager::new("app".to_string());

        // 設定値を設定
        config
            .set_config(
                "database.host",
                ConfigValue::String("localhost".to_string()),
            )
            .unwrap();
        config
            .set_config("database.port", ConfigValue::Integer(5432))
            .unwrap();
        config
            .set_config("debug.enabled", ConfigValue::Boolean(true))
            .unwrap();

        // 設定値を取得
        assert_eq!(
            config.get_config("database.host"),
            Some(ConfigValue::String("localhost".to_string()))
        );
        assert_eq!(
            config.get_config("database.port"),
            Some(ConfigValue::Integer(5432))
        );
        assert_eq!(
            config.get_config("debug.enabled"),
            Some(ConfigValue::Boolean(true))
        );
    }

    #[test]
    fn get_config_returns_none_for_missing_or_intermediate_path() {
        let config = ConfigManager::new("app".to_string());
        config.set_config("a.b.c", ConfigValue::Integer(1)).unwrap();
        assert_eq!(config.get_config("a.b.x"), None);
        assert_eq!(config.get_config("a.b"), None);
        assert_eq!(config.get_config("a.b.c"), Some(ConfigValue::Integer(1)));
    }

    #[test]
    fn set_config_keeps_siblings_and_overwrites_leaf() {
        let config = ConfigManager::new("app".to_string());
        config.set_config("a.x", ConfigValue::Integer(1)).unwrap();
        config.set_config("a.y", ConfigValue::Integer(2)).unwrap();
        config.set_config("a.x", ConfigValue::Integer(3)).unwrap();
        assert_eq!(config.get_config("a.x"), Some(ConfigValue::Integer(3)));
        assert_eq!(config.get_config("a.y"), Some(ConfigValue::Integer(2)));
    }

    #[test]
    fn set_config_on_existing_node_keeps_its_children() {
        let config = ConfigManager::new("app".to_string());
        config.set_config("a.b.c", ConfigValue::Integer(1)).unwrap();
        config.set_config("a.b", ConfigValue::Integer(2)).unwrap();
        assert_eq!(config.get_config("a.b"), Some(ConfigValue::Integer(2)));
        assert_eq!(config.get_config("a.b.c"), Some(ConfigValue::Integer(1)));
    }

    #[test]
    fn add_child_with_existing_name_keeps_subtree() {
        let root = ConfigNode::new_root("root".to_string());
        let child = root.add_child("a".to_string(), None);
        child.add_child("b".to_string(), Some(ConfigValue::Integer(1)));
        root.add_child("a".to_string(), Some(ConfigValue::Integer(2)));
        assert_eq!(root.get_value("a"), Some(ConfigValue::Integer(2)));
        assert_eq!(root.get_value("a.b"), Some(ConfigValue::Integer(1)));
    }

    #[test]
    fn set_config_rejects_empty_path_segments() {
        let config = ConfigManager::new("app".to_string());
        for path in ["", "a..b", ".a", "a."] {
            assert_eq!(
                config.set_config(path, ConfigValue::Integer(1)),
                Err(format!("Invalid config path: {path:?}")),
            );
        }
        assert_eq!(config.get_config("a"), None);
    }

    #[test]
    fn child_node_knows_its_name_and_parent() {
        let root = ConfigNode::new_root("root".to_string());
        let child = root.add_child("child".to_string(), None);
        assert_eq!(child.name(), "child");
        assert_eq!(child.get_parent().unwrap().name(), "root");
        assert!(root.get_parent().is_none());
    }

    #[test]
    fn update_config_replaces_value_and_keeps_children() {
        let config = ConfigManager::new("app".to_string());
        config.set_config("a.b", ConfigValue::Integer(1)).unwrap();
        config.set_config("a.b.c", ConfigValue::Integer(9)).unwrap();
        config
            .update_config("a.b", ConfigValue::Integer(2))
            .unwrap();
        assert_eq!(config.get_config("a.b"), Some(ConfigValue::Integer(2)));
        assert_eq!(config.get_config("a.b.c"), Some(ConfigValue::Integer(9)));
    }

    #[test]
    fn is_sensitive_matches_full_path_case_insensitively() {
        for path in [
            "database.password",
            "Auth.Password",
            "storage.s3.secret.key",
            "api.key",
            "api.token",
            "aws.access_key",
            "slack.webhook_url",
            "my.passwd",
            "private.key",
            "auth.credentials",
            "apikey",
            "notifications.webhook.url",
            "db.password.hash",
            "db.password.enc",
            "secret.arn",
            "auth.token.value",
            "password.plain",
            "api.key.id",
            "client.secret.value",
            "private.key.pem",
            "credentials.json",
            "token.1",
            "refresh.tokens",
            "password2",
            "dbpassword",
            "session_key",
            "encryption_key",
            "jwt_signing_key",
            "smtp_pass",
            "passphrase",
            "sentry.dsn",
            "github_pat",
            "slack_webhook",
            "auth_bearer",
            "api-key",
            "accesstoken",
            "authToken",
            "privateKey",
            "signingkey",
            "accesskey",
            "auth.header",
            "authorization.header",
            "db.pwd",
            "auth",
            "session.cookie",
            "hash.salt",
            "authHeader",
            "authorizationHeader",
            "sessionCookie",
            "userPwd",
            "clientAuth",
            "saltValue",
            "SSHKey",
            "userPWD",
            "HMACKey",
            "DBPass",
            "SMTPPwd",
            "SMTPAuth",
            "auth.jwt",
            "atm.pin",
            "mfa.otp",
            "request.signature",
            "url.sig",
            "private",
            "features.password_reset",
            "api.key.enabled",
            "api.auth.api_key_header",
        ] {
            assert!(is_sensitive(path), "{path}");
        }
        for path in [
            "database.host",
            "server.port",
            "security.token_expiry_hours",
            "server.tls.key_file",
            "api.base_path",
            "cache.ttl_seconds",
            "notifications.webhook.retry_attempts",
            "",
        ] {
            assert!(!is_sensitive(path), "{path}");
        }
    }

    #[test]
    fn value_has_credentials_detects_userinfo_with_password() {
        let s = |v: &str| ConfigValue::String(v.to_string());
        assert!(value_has_credentials(&s("postgres://user:pw@db:5432/app")));
        assert!(value_has_credentials(&s("https://u:p@example.com")));
        assert!(value_has_credentials(&s("https://ghp_token@github.com")));
        assert!(value_has_credentials(&s("http://u@x:PW@h")));
        assert!(value_has_credentials(&s("http://u:p/PW@h")));
        assert!(value_has_credentials(&ConfigValue::Array(vec![
            s("https://example.com"),
            s("https://u:PW@h"),
        ])));
        assert!(!value_has_credentials(&ConfigValue::Array(vec![s(
            "https://example.com"
        )])));
        assert!(!value_has_credentials(&s("https://example.com/path")));
        assert!(!value_has_credentials(&s("https://@example.com")));
        assert!(!value_has_credentials(&s("user:pw@host")));
        assert!(!value_has_credentials(&ConfigValue::Integer(1)));
    }

    #[test]
    fn should_mask_ignores_booleans_and_checks_connection_strings() {
        let s = |v: &str| ConfigValue::String(v.to_string());
        assert!(!should_mask(
            "features.password_reset",
            &ConfigValue::Boolean(true)
        ));
        assert!(!should_mask(
            "api.key.enabled",
            &ConfigValue::Boolean(false)
        ));
        assert!(should_mask("features.password_reset", &s("x")));
        assert!(should_mask("db.connection", &s("Server=h;Password=X")));
        assert!(should_mask("db.connection", &s("server=h;PWD=X")));
        assert!(!should_mask("db.connection", &s("Server=h;Database=app")));
        assert!(should_mask(
            "db.replicas",
            &ConfigValue::Array(vec![s("Server=a"), s("Server=b;Password=X")])
        ));
        assert!(should_mask("db.password", &ConfigValue::Integer(1)));
        for value in [
            "Server=h;Passwd=S2",
            "a=1&secret=S3",
            "Server=h; Password = S4",
            "DefaultEndpointsProtocol=https;AccountKey=S5",
            "https://h/p?token=S6",
            "https://h/p?x=1&sig=S7",
            "user=a password=b",
        ] {
            assert!(should_mask("db.connection", &s(value)), "{value}");
        }
        assert!(!should_mask("db.connection", &s("a=1&b=2")));
        assert!(!should_mask("db.connection", &s("=x")));
        assert!(should_mask("db.password_hours", &s("V")));
        assert!(should_mask("db.password.file", &s("/run/secrets/pw")));
        assert!(!should_mask(
            "security.token_expiry_hours",
            &ConfigValue::Integer(24)
        ));
    }

    #[test]
    fn config_value_display_shows_plain_values() {
        let value = ConfigValue::Array(vec![
            ConfigValue::String("a".to_string()),
            ConfigValue::Integer(1),
            ConfigValue::Boolean(true),
            ConfigValue::Array(vec![]),
        ]);
        assert_eq!(value.to_string(), "[a, 1, true, []]");
    }

    #[test]
    fn get_display_masks_secrets_and_formats_values() {
        let config = ConfigManager::new("app".to_string());
        config
            .set_config("db.password", ConfigValue::String("pw".to_string()))
            .unwrap();
        config
            .set_config(
                "db.host",
                ConfigValue::String("postgres://u:p@h".to_string()),
            )
            .unwrap();
        config
            .set_config("db.port", ConfigValue::Integer(5432))
            .unwrap();
        assert_eq!(config.get_display("db.password"), Some("***".to_string()));
        assert_eq!(config.get_display("db.host"), Some("***".to_string()));
        assert_eq!(config.get_display("db.port"), Some("5432".to_string()));
        assert_eq!(config.get_display("db.missing"), None);
    }

    #[test]
    fn update_config_errors_on_missing_path() {
        let config = ConfigManager::new("app".to_string());
        config.set_config("a.b", ConfigValue::Integer(1)).unwrap();
        assert_eq!(
            config.update_config("a.x", ConfigValue::Integer(2)),
            Err("Path not found: \"a.x\"".to_string())
        );
        assert_eq!(
            config.update_config("z.b", ConfigValue::Integer(2)),
            Err("Path not found: \"z.b\"".to_string())
        );
    }
}
