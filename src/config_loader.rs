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
        if let Some(ConfigValue::Integer(port)) = config_manager.get_config("server.port") {
            if port < 1 || port > 65535 {
                warnings.push("server.port must be between 1 and 65535".to_string());
            }
        }
        
        if let Some(ConfigValue::Integer(threads)) = config_manager.get_config("server.worker_threads") {
            if threads < 1 || threads > 1000 {
                warnings.push("server.worker_threads should be between 1 and 1000".to_string());
            }
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
            Value::String(s) => {
                config_manager.set_config(&prefix, ConfigValue::String(s.clone()))?;
            }
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    config_manager.set_config(&prefix, ConfigValue::Integer(i))?;
                }
            }
            Value::Bool(b) => {
                config_manager.set_config(&prefix, ConfigValue::Boolean(*b))?;
            }
            Value::Array(arr) => {
                let config_array = arr
                    .iter()
                    .filter_map(|v| Self::json_to_config_value(v))
                    .collect::<Vec<_>>();
                config_manager.set_config(&prefix, ConfigValue::Array(config_array))?;
            }
            Value::Null => {
                // Nullは設定しない
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
            toml::Value::String(s) => {
                config_manager.set_config(&prefix, ConfigValue::String(s.clone()))?;
            }
            toml::Value::Integer(i) => {
                config_manager.set_config(&prefix, ConfigValue::Integer(*i))?;
            }
            toml::Value::Boolean(b) => {
                config_manager.set_config(&prefix, ConfigValue::Boolean(*b))?;
            }
            toml::Value::Array(arr) => {
                let config_array = arr
                    .iter()
                    .filter_map(|v| Self::toml_to_config_value(v))
                    .collect::<Vec<_>>();
                config_manager.set_config(&prefix, ConfigValue::Array(config_array))?;
            }
            _ => {
                // 他の型は無視
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
                    .filter_map(|v| Self::json_to_config_value(v))
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
                    .filter_map(|v| Self::toml_to_config_value(v))
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
}