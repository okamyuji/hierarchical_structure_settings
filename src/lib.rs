use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

pub mod config_loader;

// 設定値を表す列挙型
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    String(String),
    Integer(i64),
    Boolean(bool),
    Array(Vec<ConfigValue>),
}

// 設定ノードの構造体
pub struct ConfigNode {
    name: String,
    value: RefCell<Option<ConfigValue>>,
    children: RefCell<HashMap<String, Rc<ConfigNode>>>,
    parent: RefCell<Weak<ConfigNode>>,
}

impl ConfigNode {
    // 新しいルートノードを作成
    pub fn new_root(name: String) -> Rc<Self> {
        Rc::new(ConfigNode {
            name,
            value: RefCell::new(None),
            children: RefCell::new(HashMap::new()),
            parent: RefCell::new(Weak::new()),
        })
    }

    // 子ノードを追加
    pub fn add_child(self: &Rc<Self>, name: String, value: Option<ConfigValue>) -> Rc<ConfigNode> {
        let child = Rc::new(ConfigNode {
            name: name.clone(),
            value: RefCell::new(value),
            children: RefCell::new(HashMap::new()),
            parent: RefCell::new(Rc::downgrade(self)),
        });

        self.children.borrow_mut().insert(name, child.clone());
        child
    }

    // 設定値を取得（パス指定）
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

    // 親ノードへの参照を取得
    pub fn get_parent(&self) -> Option<Rc<ConfigNode>> {
        self.parent.borrow().upgrade()
    }

    // ノード名を取得
    pub fn name(&self) -> &str {
        &self.name
    }
}

const SENSITIVE_KEYS: [&str; 10] = [
    "password",
    "passwd",
    "credential",
    "private_key",
    "apikey",
    "secret",
    "token",
    "api_key",
    "access_key",
    "webhook_url",
];

// 環境変数は `_` を `.` に変えて読み込むので、区切りを揃えたフルパスで照合する。
fn is_sensitive(path: &str) -> bool {
    let path = path.to_lowercase().replace('.', "_");
    SENSITIVE_KEYS.iter().any(|k| path.contains(k))
}

// 設定管理システムのメイン構造体
pub struct ConfigManager {
    root: Rc<ConfigNode>,
}

impl ConfigManager {
    pub fn new(root_name: String) -> Self {
        ConfigManager {
            root: ConfigNode::new_root(root_name),
        }
    }

    // 設定値を設定
    pub fn set_config(&self, path: &str, value: ConfigValue) -> Result<(), String> {
        let parts: Vec<&str> = path.split('.').collect();
        self.set_config_recursive(&self.root, &parts, value)
    }

    fn set_config_recursive(
        &self,
        node: &Rc<ConfigNode>,
        path: &[&str],
        value: ConfigValue,
    ) -> Result<(), String> {
        if path.len() == 1 {
            // 最終ノードに到達
            let _child = node.add_child(path[0].to_string(), Some(value));
            return Ok(());
        }

        // 中間ノードを作成または取得
        let mut children = node.children.borrow_mut();
        let child = if let Some(existing) = children.get(path[0]) {
            existing.clone()
        } else {
            let new_child = Rc::new(ConfigNode {
                name: path[0].to_string(),
                value: RefCell::new(None),
                children: RefCell::new(HashMap::new()),
                parent: RefCell::new(Rc::downgrade(node)),
            });
            children.insert(path[0].to_string(), new_child.clone());
            new_child
        };

        drop(children); // 借用を明示的に終了
        self.set_config_recursive(&child, &path[1..], value)
    }

    // 設定値を取得
    pub fn get_config(&self, path: &str) -> Option<ConfigValue> {
        self.root.get_value(path)
    }

    // 設定ツリーを表示
    pub fn display_tree(&self) {
        self.display_node(&self.root, 0, "");
    }

    fn display_node(&self, node: &ConfigNode, depth: usize, path: &str) {
        let indent = "  ".repeat(depth);
        match &*node.value.borrow() {
            Some(_) if is_sensitive(path) => println!("{}{}= ***", indent, node.name()),
            Some(value) => println!("{}{}= {:?}", indent, node.name(), value),
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
}

impl ConfigManager {
    pub fn update_config(&self, path: &str, value: ConfigValue) -> Result<(), String> {
        let parts: Vec<&str> = path.split('.').collect();
        self.update_config_recursive(&self.root, &parts, value)
    }

    fn update_config_recursive(
        &self,
        node: &Rc<ConfigNode>,
        path: &[&str],
        value: ConfigValue,
    ) -> Result<(), String> {
        if path.len() == 1 {
            let children = node.children.borrow();
            if let Some(child) = children.get(path[0]) {
                child.value.replace(Some(value));
                return Ok(());
            } else {
                return Err(format!("Path not found: {}", path[0]));
            }
        }

        let children = node.children.borrow();
        if let Some(child) = children.get(path[0]) {
            let child = child.clone();
            drop(children);
            self.update_config_recursive(&child, &path[1..], value)
        } else {
            Err(format!("Path not found: {}", path.join(".")))
        }
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
        ] {
            assert!(is_sensitive(path), "{path}");
        }
        for path in ["database.host", "api.auth.methods", "server.port", ""] {
            assert!(!is_sensitive(path), "{path}");
        }
    }

    #[test]
    fn update_config_errors_on_missing_path() {
        let config = ConfigManager::new("app".to_string());
        config.set_config("a.b", ConfigValue::Integer(1)).unwrap();
        assert_eq!(
            config.update_config("a.x", ConfigValue::Integer(2)),
            Err("Path not found: x".to_string())
        );
        assert_eq!(
            config.update_config("z.b", ConfigValue::Integer(2)),
            Err("Path not found: z.b".to_string())
        );
    }
}
