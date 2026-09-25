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

/// 設定ツリー全体を管理する。
///
/// パスの誤りなど、この型が返すエラーは `String`。ファイルを読む `ConfigLoader` は
/// `Box<dyn Error>` を返し、`?` でこの `String` をそのまま受け取れる。
pub struct ConfigManager {
    root: Rc<ConfigNode>,
    display_allowlist: RefCell<Vec<String>>,
}

impl ConfigManager {
    /// 指定した名前のルートを持つ空の設定ツリーを作る。
    pub fn new(root_name: String) -> Self {
        ConfigManager {
            root: ConfigNode::new_root(root_name),
            display_allowlist: RefCell::new(Vec::new()),
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

    /// 表示を許可するパスを登録する。`server.port` はそのパスだけ、`features.*` は配下のすべてに一致する。
    ///
    /// `features.*` は `features` そのものには一致しない。ワイルドカードは、後から配下に加えたキーも表示する。
    /// 許可していないパスの値は、`display_tree` と `get_display` で `***` になる。既定では何も許可しない。
    ///
    /// 空のパターン、空のセグメント、末尾の `.*` 以外に現れる `*` はエラーになる。同じパターンは一度だけ登録する。
    pub fn allow_display(&self, pattern: &str) -> Result<(), String> {
        let segments: Vec<&str> = pattern.split('.').collect();
        // split は常に1つ以上の要素を返すので、segments は空にならない。
        let (parents, last) = segments.split_at(segments.len() - 1);
        let valid = segments.iter().all(|s| !s.is_empty())
            && parents.iter().all(|s| !s.contains('*'))
            && (!last[0].contains('*') || (last[0] == "*" && !parents.is_empty()));
        if !valid {
            return Err(format!("Invalid display pattern: {pattern:?}"));
        }
        let mut allowlist = self.display_allowlist.borrow_mut();
        if !allowlist.iter().any(|p| p == pattern) {
            allowlist.push(pattern.to_string());
        }
        Ok(())
    }

    fn is_display_allowed(&self, path: &str) -> bool {
        self.display_allowlist
            .borrow()
            .iter()
            .any(|pattern| match pattern.strip_suffix(".*") {
                Some(prefix) => path
                    .strip_prefix(prefix)
                    .is_some_and(|rest| rest.starts_with('.')),
                None => pattern == path,
            })
    }

    /// 表示用の文字列を返す。許可していないパスの値は `***` になる。
    pub fn get_display(&self, path: &str) -> Option<String> {
        let value = self.get_config(path)?;
        Some(if self.is_display_allowed(path) {
            value.to_string()
        } else {
            "***".to_string()
        })
    }

    /// パスの値を取得する。値を持たない中間ノードは `None`。
    pub fn get_config(&self, path: &str) -> Option<ConfigValue> {
        self.root.get_value(path)
    }

    /// 設定ツリーを標準出力に表示する。`allow_display` で許可していないパスの値は `***` で隠す。
    pub fn display_tree(&self) {
        self.display_node(&self.root, 0, "");
    }

    fn display_node(&self, node: &ConfigNode, depth: usize, path: &str) {
        let indent = "  ".repeat(depth);
        match &*node.value.borrow() {
            Some(value) if self.is_display_allowed(path) => {
                println!("{}{}= {}", indent, node.name(), value)
            }
            Some(_) => println!("{}{}= ***", indent, node.name()),
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
    fn allow_display_matches_exact_paths_and_wildcard_subtrees() {
        let config = ConfigManager::new("app".to_string());
        config.allow_display("server.port").unwrap();
        config.allow_display("features.*").unwrap();
        for path in ["server.port", "features.a", "features.a.b"] {
            assert!(config.is_display_allowed(path), "{path}");
        }
        for path in [
            "server.host",
            "server.port.x",
            "server",
            "features",
            "featuresx.a",
            "serverx.port",
            "",
        ] {
            assert!(!config.is_display_allowed(path), "{path}");
        }
    }

    #[test]
    fn allow_display_rejects_malformed_patterns() {
        let config = ConfigManager::new("app".to_string());
        for pattern in ["", "*", ".*", "a..b", "a.", "a.*.b", "a*", "a.b*", "*.a"] {
            assert_eq!(
                config.allow_display(pattern),
                Err(format!("Invalid display pattern: {pattern:?}")),
            );
        }
        for pattern in ["a", "a.b", "a.*", "a.b.*"] {
            assert_eq!(config.allow_display(pattern), Ok(()), "{pattern}");
        }
        assert_eq!(config.display_allowlist.borrow().len(), 4);
    }

    #[test]
    fn allow_display_ignores_duplicate_patterns() {
        let config = ConfigManager::new("app".to_string());
        config.allow_display("a.b").unwrap();
        config.allow_display("a.b").unwrap();
        assert_eq!(config.display_allowlist.borrow().len(), 1);
    }

    #[test]
    fn nothing_is_displayed_until_allowed() {
        let config = ConfigManager::new("app".to_string());
        config
            .set_config("flag", ConfigValue::Boolean(true))
            .unwrap();
        assert_eq!(config.get_display("flag"), Some("***".to_string()));
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
    fn get_display_shows_only_allowed_values() {
        let config = ConfigManager::new("app".to_string());
        config
            .set_config("db.password", ConfigValue::String("pw".to_string()))
            .unwrap();
        config
            .set_config("db.port", ConfigValue::Integer(5432))
            .unwrap();
        config.allow_display("db.port").unwrap();
        assert_eq!(config.get_display("db.password"), Some("***".to_string()));
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
