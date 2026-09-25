use crate::{ConfigManager, ConfigValue};
use serde_json::Value;
use std::fs;
use std::path::Path;

pub struct ConfigLoader;

impl ConfigLoader {
    /// JSONファイルから設定を読み込み、ConfigManagerに設定する
    pub fn load_from_json<P: AsRef<Path>>(
        config_manager: &ConfigManager,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let json_value: Value = serde_json::from_str(&content)?;
        
        Self::load_json_value(config_manager, &json_value, String::new())?;
        Ok(())
    }

    /// TOMLファイルから設定を読み込み、ConfigManagerに設定する
    pub fn load_from_toml<P: AsRef<Path>>(
        config_manager: &ConfigManager,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let toml_value: toml::Value = toml::from_str(&content)?;
        
        Self::load_toml_value(config_manager, &toml_value, String::new())?;
        Ok(())
    }

    /// 環境変数から設定を読み込み（プレフィックス付き）
    pub fn load_from_env(
        config_manager: &ConfigManager,
        prefix: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::env;
        
        for (key, value) in env::vars() {
            if key.starts_with(prefix) {
                let config_path = key
                    .strip_prefix(prefix)
                    .unwrap()
                    .trim_start_matches('_')
                    .to_lowercase()
                    .replace('_', ".");
                
                let config_value = Self::parse_env_value(&value);
                config_manager.set_config(&config_path, config_value)?;
            }
        }
        
        Ok(())
    }

    /// 設定ファイルの形式を自動判定して読み込み
    pub fn auto_load<P: AsRef<Path>>(
        config_manager: &ConfigManager,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path_ref = path.as_ref();
        
        match path_ref.extension().and_then(|s| s.to_str()) {
            Some("json") => Self::load_from_json(config_manager, path),
            Some("toml") => Self::load_from_toml(config_manager, path),
            Some("yaml") | Some("yml") => {
                // YAMLサポートはserde_yamlクレートが必要
                Err("YAML support not implemented".into())
            }
            _ => Err("Unsupported file format".into()),
        }
    }

    /// 複数の設定ファイルを順次読み込み（後から読み込まれた値で上書き）
    pub fn load_multiple<P: AsRef<Path>>(
        config_manager: &ConfigManager,
        paths: Vec<P>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for path in paths {
            Self::auto_load(config_manager, path)?;
        }
        Ok(())
    }

    /// デフォルト設定を適用
    pub fn apply_defaults(config_manager: &ConfigManager) -> Result<(), String> {
        // アプリケーションのデフォルト設定
        config_manager.set_config("app.name", ConfigValue::String("DefaultApp".to_string()))?;
        config_manager.set_config("app.version", ConfigValue::String("1.0.0".to_string()))?;
        
        // データベースのデフォルト設定
        config_manager.set_config("database.host", ConfigValue::String("localhost".to_string()))?;
        config_manager.set_config("database.port", ConfigValue::Integer(5432))?;
        config_manager.set_config("database.timeout_seconds", ConfigValue::Integer(30))?;
        config_manager.set_config("database.ssl_enabled", ConfigValue::Boolean(false))?;
        
        // サーバーのデフォルト設定
        config_manager.set_config("server.host", ConfigValue::String("127.0.0.1".to_string()))?;
        config_manager.set_config("server.port", ConfigValue::Integer(8080))?;
        config_manager.set_config("server.worker_threads", ConfigValue::Integer(4))?;
        
        // デバッグのデフォルト設定
        config_manager.set_config("debug.enabled", ConfigValue::Boolean(false))?;
        config_manager.set_config("debug.log_level", ConfigValue::String("INFO".to_string()))?;
        
        // キャッシュのデフォルト設定
        config_manager.set_config("cache.enabled", ConfigValue::Boolean(true))?;
        config_manager.set_config("cache.ttl_seconds", ConfigValue::Integer(3600))?;
        
        Ok(())
    }

    /// 設定の検証を実行
    pub fn validate_config(config_manager: &ConfigManager) -> Result<Vec<String>, String> {
        let mut warnings = Vec::new();
        
        // 必須設定の確認
        let required_configs = vec![
            "database.host",
            "database.port", 
            "server.host",
            "server.port",
        ];
        
        for config_path in required_configs {
            if config_manager.get_config(config_path).is_none() {
                warnings.push(format!("Required configuration missing: {}", config_path));
            }
        }
        
        // 値の範囲チェック
        if let Some(ConfigValue::Integer(port)) = config_manager.get_config("server.port")
            && !(1..=65535).contains(&port)
        {
            warnings.push("server.port must be between 1 and 65535".to_string());
        }

        if let Some(ConfigValue::Integer(threads)) = config_manager.get_config("server.worker_threads")
            && !(1..=1000).contains(&threads)
        {
            warnings.push("server.worker_threads should be between 1 and 1000".to_string());
        }
        
        Ok(warnings)
    }

    // プライベートヘルパーメソッド
    fn load_json_value(
        config_manager: &ConfigManager,
        value: &Value,
        prefix: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match value {
            Value::Object(map) => {
                for (key, val) in map {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", prefix, key)
                    };
                    Self::load_json_value(config_manager, val, path)?;
                }
            }
            _ => {
                if let Some(v) = Self::json_to_config_value(value) {
                    config_manager.set_config(&prefix, v)?;
                }
            }
        }
        Ok(())
    }

    fn load_toml_value(
        config_manager: &ConfigManager,
        value: &toml::Value,
        prefix: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match value {
            toml::Value::Table(table) => {
                for (key, val) in table {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", prefix, key)
                    };
                    Self::load_toml_value(config_manager, val, path)?;
                }
            }
            _ => {
                if let Some(v) = Self::toml_to_config_value(value) {
                    config_manager.set_config(&prefix, v)?;
                }
            }
        }
        Ok(())
    }

    fn json_to_config_value(value: &Value) -> Option<ConfigValue> {
        match value {
            Value::String(s) => Some(ConfigValue::String(s.clone())),
            Value::Number(n) => n.as_i64().map(ConfigValue::Integer),
            Value::Bool(b) => Some(ConfigValue::Boolean(*b)),
            Value::Array(arr) => {
                let config_array = arr
                    .iter()
                    .filter_map(Self::json_to_config_value)
                    .collect::<Vec<_>>();
                Some(ConfigValue::Array(config_array))
            }
            _ => None,
        }
    }

    fn toml_to_config_value(value: &toml::Value) -> Option<ConfigValue> {
        match value {
            toml::Value::String(s) => Some(ConfigValue::String(s.clone())),
            toml::Value::Integer(i) => Some(ConfigValue::Integer(*i)),
            toml::Value::Boolean(b) => Some(ConfigValue::Boolean(*b)),
            toml::Value::Array(arr) => {
                let config_array = arr
                    .iter()
                    .filter_map(Self::toml_to_config_value)
                    .collect::<Vec<_>>();
                Some(ConfigValue::Array(config_array))
            }
            _ => None,
        }
    }

    fn parse_env_value(value: &str) -> ConfigValue {
        // 環境変数の値を適切な型に変換
        if let Ok(i) = value.parse::<i64>() {
            ConfigValue::Integer(i)
        } else if let Ok(b) = value.parse::<bool>() {
            ConfigValue::Boolean(b)
        } else {
            ConfigValue::String(value.to_string())
        }
    }
}

// 使用例のテスト
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_loading() {
        let config = ConfigManager::new("test".to_string());
        
        // デフォルト設定を適用
        ConfigLoader::apply_defaults(&config).unwrap();
        
        // 設定値の確認
        assert_eq!(
            config.get_config("database.host"),
            Some(ConfigValue::String("localhost".to_string()))
        );
        assert_eq!(
            config.get_config("database.port"),
            Some(ConfigValue::Integer(5432))
        );
        
        // 設定の検証
        let warnings = ConfigLoader::validate_config(&config).unwrap();
        assert!(warnings.is_empty());
    }

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(name)
    }

    fn write_temp(name: &str, content: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("hss_{}_{}", std::process::id(), name));
        fs::write(&path, content).unwrap();
        path
    }

    fn s(v: &str) -> ConfigValue {
        ConfigValue::String(v.to_string())
    }

    #[test]
    fn loads_nested_values_from_toml_file() {
        let config = ConfigManager::new("t".to_string());
        ConfigLoader::load_from_toml(&config, fixture("config.toml")).unwrap();
        assert!(config.get_config("app.name").is_some());
        assert!(matches!(config.get_config("database.port"), Some(ConfigValue::Integer(_))));
    }

    #[test]
    fn loads_nested_values_from_json_file() {
        let config = ConfigManager::new("t".to_string());
        ConfigLoader::load_from_json(&config, fixture("config.json")).unwrap();
        assert_eq!(config.get_config("database.pool.min_connections"), Some(ConfigValue::Integer(5)));
        assert_eq!(config.get_config("database.ssl_enabled"), Some(ConfigValue::Boolean(true)));
        assert_eq!(
            config.get_config("database.replication.slave_hosts"),
            Some(ConfigValue::Array(vec![
                s("db-slave1.example.com"),
                s("db-slave2.example.com"),
                s("db-slave3.example.com"),
            ]))
        );
    }

    #[test]
    fn json_skips_null_and_float_and_keeps_nested_arrays() {
        let path = write_temp(
            "types.json",
            r#"{"a":{"n":null,"f":1.5,"arr":[1,"x",true,null,[2]]}}"#,
        );
        let config = ConfigManager::new("t".to_string());
        ConfigLoader::load_from_json(&config, &path).unwrap();
        assert_eq!(config.get_config("a.n"), None);
        assert_eq!(config.get_config("a.f"), None);
        assert_eq!(
            config.get_config("a.arr"),
            Some(ConfigValue::Array(vec![
                ConfigValue::Integer(1),
                s("x"),
                ConfigValue::Boolean(true),
                ConfigValue::Array(vec![ConfigValue::Integer(2)]),
            ]))
        );
    }

    #[test]
    fn toml_skips_unsupported_types_and_keeps_nested_arrays() {
        let path = write_temp(
            "types.toml",
            "[a]\nf = 1.5\nb = false\narr = [[1, 2], [\"x\", true], [1.5]]\n",
        );
        let config = ConfigManager::new("t".to_string());
        ConfigLoader::load_from_toml(&config, &path).unwrap();
        assert_eq!(config.get_config("a.f"), None);
        assert_eq!(config.get_config("a.b"), Some(ConfigValue::Boolean(false)));
        assert_eq!(
            config.get_config("a.arr"),
            Some(ConfigValue::Array(vec![
                ConfigValue::Array(vec![ConfigValue::Integer(1), ConfigValue::Integer(2)]),
                ConfigValue::Array(vec![s("x"), ConfigValue::Boolean(true)]),
                ConfigValue::Array(vec![]),
            ]))
        );
    }

    #[test]
    fn load_fails_on_missing_file_and_invalid_syntax() {
        let config = ConfigManager::new("t".to_string());
        assert!(ConfigLoader::load_from_json(&config, "no_such.json").is_err());
        assert!(ConfigLoader::load_from_toml(&config, "no_such.toml").is_err());
        assert!(ConfigLoader::load_from_json(&config, write_temp("bad.json", "{")).is_err());
        assert!(ConfigLoader::load_from_toml(&config, write_temp("bad.toml", "a = ")).is_err());
    }

    #[test]
    fn auto_load_dispatches_by_extension() {
        let config = ConfigManager::new("t".to_string());
        ConfigLoader::auto_load(&config, write_temp("x.json", r#"{"k":"json"}"#)).unwrap();
        assert_eq!(config.get_config("k"), Some(s("json")));
        ConfigLoader::auto_load(&config, write_temp("x.toml", "k = \"toml\"")).unwrap();
        assert_eq!(config.get_config("k"), Some(s("toml")));
        for name in ["c.yaml", "c.yml"] {
            let err = ConfigLoader::auto_load(&config, name).unwrap_err();
            assert_eq!(err.to_string(), "YAML support not implemented");
        }
        for name in ["c.ini", "noext"] {
            let err = ConfigLoader::auto_load(&config, name).unwrap_err();
            assert_eq!(err.to_string(), "Unsupported file format");
        }
    }

    #[test]
    fn load_multiple_later_file_wins_and_stops_on_error() {
        let first = write_temp("m1.toml", "k = \"first\"\nonly_first = 1");
        let second = write_temp("m2.json", r#"{"k":"second"}"#);
        let config = ConfigManager::new("t".to_string());
        ConfigLoader::load_multiple(&config, vec![first.clone(), second]).unwrap();
        assert_eq!(config.get_config("k"), Some(s("second")));
        assert_eq!(config.get_config("only_first"), Some(ConfigValue::Integer(1)));
        let bad = Path::new("bad.ini").to_path_buf();
        assert!(ConfigLoader::load_multiple(&config, vec![bad, first]).is_err());
    }

    #[test]
    fn load_from_env_maps_prefixed_vars_to_paths() {
        // SAFETY: このテストだけが HSSTEST 接頭辞の環境変数を読み書きする。
        unsafe {
            std::env::set_var("HSSTEST_DATABASE_HOST", "db.example.com");
            std::env::set_var("HSSTEST_DATABASE_PORT", "5433");
        }
        let config = ConfigManager::new("t".to_string());
        ConfigLoader::load_from_env(&config, "HSSTEST").unwrap();
        assert_eq!(config.get_config("database.host"), Some(s("db.example.com")));
        assert_eq!(config.get_config("database.port"), Some(ConfigValue::Integer(5433)));
        assert_eq!(config.get_config("hsstest"), None);
    }

    #[test]
    fn parse_env_value_prefers_integer_then_bool_then_string() {
        assert_eq!(ConfigLoader::parse_env_value("-42"), ConfigValue::Integer(-42));
        assert_eq!(ConfigLoader::parse_env_value("true"), ConfigValue::Boolean(true));
        assert_eq!(ConfigLoader::parse_env_value("false"), ConfigValue::Boolean(false));
        assert_eq!(ConfigLoader::parse_env_value("TRUE"), s("TRUE"));
    }

    #[test]
    fn apply_defaults_sets_every_default() {
        let config = ConfigManager::new("t".to_string());
        ConfigLoader::apply_defaults(&config).unwrap();
        let expected = [
            ("app.name", s("DefaultApp")),
            ("app.version", s("1.0.0")),
            ("database.timeout_seconds", ConfigValue::Integer(30)),
            ("database.ssl_enabled", ConfigValue::Boolean(false)),
            ("server.host", s("127.0.0.1")),
            ("server.port", ConfigValue::Integer(8080)),
            ("server.worker_threads", ConfigValue::Integer(4)),
            ("debug.enabled", ConfigValue::Boolean(false)),
            ("debug.log_level", s("INFO")),
            ("cache.enabled", ConfigValue::Boolean(true)),
            ("cache.ttl_seconds", ConfigValue::Integer(3600)),
        ];
        for (path, value) in expected {
            assert_eq!(config.get_config(path), Some(value), "{path}");
        }
    }

    fn warnings_for(port: i64, threads: i64) -> Vec<String> {
        let config = ConfigManager::new("t".to_string());
        ConfigLoader::apply_defaults(&config).unwrap();
        config.set_config("server.port", ConfigValue::Integer(port)).unwrap();
        config
            .set_config("server.worker_threads", ConfigValue::Integer(threads))
            .unwrap();
        ConfigLoader::validate_config(&config).unwrap()
    }

    #[test]
    fn validate_config_checks_port_and_thread_boundaries() {
        assert!(warnings_for(1, 1).is_empty());
        assert!(warnings_for(65535, 1000).is_empty());
        assert_eq!(
            warnings_for(0, 0),
            vec![
                "server.port must be between 1 and 65535",
                "server.worker_threads should be between 1 and 1000",
            ]
        );
        assert_eq!(warnings_for(65536, 1001).len(), 2);
    }

    #[test]
    fn validate_config_reports_missing_required_keys() {
        let config = ConfigManager::new("t".to_string());
        let warnings = ConfigLoader::validate_config(&config).unwrap();
        assert_eq!(
            warnings,
            vec![
                "Required configuration missing: database.host",
                "Required configuration missing: database.port",
                "Required configuration missing: server.host",
                "Required configuration missing: server.port",
            ]
        );
    }
}