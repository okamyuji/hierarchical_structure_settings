use crate::{ConfigManager, ConfigValue};
use serde_json::Value;
use std::ffi::OsString;
use std::fs;
use std::path::Path;

/// JSON、TOML、環境変数から `ConfigManager` へ設定を読み込む。
pub struct ConfigLoader;

impl ConfigLoader {
    /// JSON ファイルを読み込む。エラーにはファイルのパスを含める。
    ///
    /// `null`、小数、`i64` に収まらない整数は読み飛ばす。配列の中でも同様に捨てるので、要素の位置がずれる。
    pub fn load_from_json<P: AsRef<Path>>(
        config_manager: &ConfigManager,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = path.as_ref();
        let content = Self::read_file(path)?;
        let json_value: Value =
            serde_json::from_str(&content).map_err(|e| format!("{}: {e}", path.display()))?;

        Self::load_json_value(config_manager, &json_value, String::new())
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(())
    }

    /// TOML ファイルを読み込む。エラーにはファイルのパスを含める。
    ///
    /// 小数と日時は読み飛ばす。配列の中でも同様に捨てるので、要素の位置がずれる。
    pub fn load_from_toml<P: AsRef<Path>>(
        config_manager: &ConfigManager,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = path.as_ref();
        let content = Self::read_file(path)?;
        let toml_value: toml::Value =
            toml::from_str(&content).map_err(|e| Self::toml_error(path, &content, &e))?;

        Self::load_toml_value(config_manager, &toml_value, String::new())
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(())
    }

    /// `<prefix>_` で始まる環境変数を読み込む。`APP_DATABASE_HOST` は `database.host` に入る。
    ///
    /// UTF-8 でない変数と、空のセグメントができる変数（`APP_`、`APP_A__B`）は読み飛ばす。
    pub fn load_from_env(
        config_manager: &ConfigManager,
        prefix: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Self::load_from_vars(config_manager, prefix, std::env::vars_os())
    }

    fn load_from_vars(
        config_manager: &ConfigManager,
        prefix: &str,
        vars: impl IntoIterator<Item = (OsString, OsString)>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // "APP" と "APP_" を同じ扱いにし、APPLE_* のような別名の変数を拾わない。
        let prefix = format!("{}_", prefix.trim_end_matches('_'));
        for (key, value) in vars {
            let (Ok(key), Ok(value)) = (key.into_string(), value.into_string()) else {
                continue;
            };
            if let Some(rest) = key.strip_prefix(&prefix) {
                let config_path = rest
                    .trim_start_matches('_')
                    .to_lowercase()
                    .replace('_', ".");
                if config_path.split('.').any(str::is_empty) {
                    continue;
                }

                let config_value = Self::parse_env_value(&value);
                config_manager.set_config(&config_path, config_value)?;
            }
        }

        Ok(())
    }

    /// 拡張子（`.json` / `.toml`）で形式を判定して読み込む。
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
                Err(format!("YAML support not implemented: {}", path_ref.display()).into())
            }
            _ => Err(format!("Unsupported file format: {}", path_ref.display()).into()),
        }
    }

    /// 複数のファイルを順に読み込む。後のファイルの値が優先され、最初のエラーで止まる。
    pub fn load_multiple<P: AsRef<Path>>(
        config_manager: &ConfigManager,
        paths: Vec<P>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for path in paths {
            Self::auto_load(config_manager, path)?;
        }
        Ok(())
    }

    /// アプリケーションの既定値を設定する。
    pub fn apply_defaults(config_manager: &ConfigManager) -> Result<(), String> {
        // アプリケーションのデフォルト設定
        config_manager.set_config("app.name", ConfigValue::String("DefaultApp".to_string()))?;
        config_manager.set_config("app.version", ConfigValue::String("1.0.0".to_string()))?;

        // データベースのデフォルト設定
        config_manager.set_config(
            "database.host",
            ConfigValue::String("localhost".to_string()),
        )?;
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

    /// 必須項目の欠落と値の範囲を検査し、警告の一覧を返す。問題が無ければ空。
    pub fn validate_config(config_manager: &ConfigManager) -> Vec<String> {
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

        if let Some(ConfigValue::Integer(threads)) =
            config_manager.get_config("server.worker_threads")
            && !(1..=1000).contains(&threads)
        {
            warnings.push("server.worker_threads should be between 1 and 1000".to_string());
        }

        warnings
    }

    // toml のエラー表示は該当行をそのまま含み、秘密値が漏れ得るので、位置と説明だけを返す。
    fn toml_error(path: &Path, content: &str, error: &toml::de::Error) -> String {
        let (line, column) = error.span().map_or((0, 0), |span| {
            let before = &content[..span.start];
            let line = before.matches('\n').count() + 1;
            let column = before
                .rsplit('\n')
                .next()
                .unwrap_or_default()
                .chars()
                .count()
                + 1;
            (line, column)
        });
        format!(
            "{}: TOML parse error at line {line}, column {column}: {}",
            path.display(),
            error.message()
        )
    }

    fn read_file(path: &Path) -> Result<String, String> {
        fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
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
        let warnings = ConfigLoader::validate_config(&config);
        assert!(warnings.is_empty());
    }

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(name)
    }

    struct TempFile(std::path::PathBuf);

    impl AsRef<Path> for TempFile {
        fn as_ref(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn write_temp(name: &str, content: &str) -> TempFile {
        let path = std::env::temp_dir().join(format!("hss_{}_{}", std::process::id(), name));
        fs::write(&path, content).unwrap();
        TempFile(path)
    }

    fn s(v: &str) -> ConfigValue {
        ConfigValue::String(v.to_string())
    }

    #[test]
    fn loads_nested_values_from_toml_file() {
        let config = ConfigManager::new("t".to_string());
        ConfigLoader::load_from_toml(&config, fixture("config.toml")).unwrap();
        assert!(config.get_config("app.name").is_some());
        assert!(matches!(
            config.get_config("database.port"),
            Some(ConfigValue::Integer(_))
        ));
    }

    #[test]
    fn loads_nested_values_from_json_file() {
        let config = ConfigManager::new("t".to_string());
        ConfigLoader::load_from_json(&config, fixture("config.json")).unwrap();
        assert_eq!(
            config.get_config("database.pool.min_connections"),
            Some(ConfigValue::Integer(5))
        );
        assert_eq!(
            config.get_config("database.ssl_enabled"),
            Some(ConfigValue::Boolean(true))
        );
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
    fn toml_syntax_error_does_not_echo_the_line() {
        let config = ConfigManager::new("t".to_string());
        let toml = write_temp("leak.toml", "a = 1\npassword = \"TOPSECRET\n");
        let message = ConfigLoader::load_from_toml(&config, &toml)
            .unwrap_err()
            .to_string();
        assert!(!message.contains("TOPSECRET"), "{message}");
        assert!(message.contains("leak.toml"), "{message}");
        assert!(message.contains("line 2, column 22:"), "{message}");
    }

    #[test]
    fn content_errors_include_file_path() {
        let config = ConfigManager::new("t".to_string());
        let json = write_temp("emptykey.json", r#"{"": 1}"#);
        let toml = write_temp("emptykey.toml", "\"\" = 1");
        for message in [
            ConfigLoader::load_from_json(&config, &json)
                .unwrap_err()
                .to_string(),
            ConfigLoader::load_from_toml(&config, &toml)
                .unwrap_err()
                .to_string(),
        ] {
            assert!(message.contains("emptykey."), "{message}");
            assert!(message.contains("Invalid config path"), "{message}");
        }
    }

    #[test]
    fn load_fails_on_missing_file_and_invalid_syntax() {
        let config = ConfigManager::new("t".to_string());
        let errors = [
            ConfigLoader::load_from_json(&config, "no_such.json"),
            ConfigLoader::load_from_toml(&config, "no_such.toml"),
            ConfigLoader::load_from_json(&config, write_temp("bad.json", "{")),
            ConfigLoader::load_from_toml(&config, write_temp("bad.toml", "a = ")),
        ];
        for (err, name) in
            errors
                .into_iter()
                .zip(["no_such.json", "no_such.toml", "bad.json", "bad.toml"])
        {
            let message = err.unwrap_err().to_string();
            assert!(message.contains(name), "{message}");
        }
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
            assert_eq!(
                err.to_string(),
                format!("YAML support not implemented: {name}")
            );
        }
        for name in ["c.ini", "noext"] {
            let err = ConfigLoader::auto_load(&config, name).unwrap_err();
            assert_eq!(err.to_string(), format!("Unsupported file format: {name}"));
        }
    }

    #[test]
    fn load_multiple_later_file_wins_and_stops_on_error() {
        let first = write_temp("m1.toml", "k = \"first\"\nonly_first = 1");
        let second = write_temp("m2.json", r#"{"k":"second"}"#);
        let config = ConfigManager::new("t".to_string());
        ConfigLoader::load_multiple(&config, vec![first.as_ref(), second.as_ref()]).unwrap();
        assert_eq!(config.get_config("k"), Some(s("second")));
        assert_eq!(
            config.get_config("only_first"),
            Some(ConfigValue::Integer(1))
        );
        let bad = Path::new("bad.ini");
        assert!(ConfigLoader::load_multiple(&config, vec![bad, first.as_ref()]).is_err());
        let first_path = first.0.clone();
        drop(first);
        assert!(!first_path.exists());
    }

    fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        pairs
            .iter()
            .map(|(k, v)| (OsString::from(k), OsString::from(v)))
            .collect()
    }

    #[cfg(unix)]
    #[test]
    fn load_from_vars_skips_non_utf8_entries() {
        use std::os::unix::ffi::OsStringExt;
        let invalid = || OsString::from_vec(vec![0xff, 0xfe]);
        let mut env = vars(&[("HSSTEST_OK", "1")]);
        env.push((OsString::from("HSSTEST_BAD_VALUE"), invalid()));
        env.push((invalid(), OsString::from("x")));
        let config = ConfigManager::new("t".to_string());
        ConfigLoader::load_from_vars(&config, "HSSTEST", env).unwrap();
        assert_eq!(config.get_config("ok"), Some(ConfigValue::Integer(1)));
        assert_eq!(config.get_config("bad.value"), None);
    }

    #[test]
    fn load_from_vars_maps_prefixed_vars_to_paths() {
        let env = vars(&[
            ("HSSTEST_DATABASE_HOST", "db.example.com"),
            ("HSSTEST_DATABASE_PORT", "5433"),
            ("HSSTESTX_LEAK", "1"),
            ("OTHER_KEY", "x"),
            ("HSSTEST_", "empty"),
            ("HSSTEST_A__B", "gap"),
        ]);
        for prefix in ["HSSTEST", "HSSTEST_"] {
            let config = ConfigManager::new("t".to_string());
            ConfigLoader::load_from_vars(&config, prefix, env.clone()).unwrap();
            assert_eq!(
                config.get_config("database.host"),
                Some(s("db.example.com"))
            );
            assert_eq!(
                config.get_config("database.port"),
                Some(ConfigValue::Integer(5433))
            );
            assert_eq!(config.get_config("x.leak"), None, "{prefix}");
            assert_eq!(config.get_config("key"), None, "{prefix}");
            assert_eq!(config.get_config(""), None, "{prefix}");
            assert_eq!(config.get_config("a"), None, "{prefix}");
        }
    }

    #[test]
    fn load_from_env_reads_process_environment() {
        let config = ConfigManager::new("t".to_string());
        ConfigLoader::load_from_env(&config, "HSS_UNUSED_PREFIX_ZZ").unwrap();
        assert_eq!(config.get_config("anything"), None);
    }

    #[test]
    fn parse_env_value_prefers_integer_then_bool_then_string() {
        assert_eq!(
            ConfigLoader::parse_env_value("-42"),
            ConfigValue::Integer(-42)
        );
        assert_eq!(
            ConfigLoader::parse_env_value("true"),
            ConfigValue::Boolean(true)
        );
        assert_eq!(
            ConfigLoader::parse_env_value("false"),
            ConfigValue::Boolean(false)
        );
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
        config
            .set_config("server.port", ConfigValue::Integer(port))
            .unwrap();
        config
            .set_config("server.worker_threads", ConfigValue::Integer(threads))
            .unwrap();
        ConfigLoader::validate_config(&config)
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
        let warnings = ConfigLoader::validate_config(&config);
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
