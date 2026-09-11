//! 本地状态与配置
//! - ~/.jex 全局目录(缓存 / 已装 JDK / 配置)
//! - 项目级 jex.toml 脚手架

use crate::error::{Error, Result};
use std::path::PathBuf;

/// 返回 ~/.jex 全局目录路径。
pub fn jex_home() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| Error::new("无法获取用户主目录".to_string()))?;
    Ok(home.join(".jex"))
}

/// 返回 ~/.jex/m2/ 缓存目录（替代 cs fetch 的本地 jar 仓库）。
pub fn jex_m2_cache() -> Result<PathBuf> {
    let dir = jex_home()?.join("m2");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 从 jex.toml 读取格式化配置
pub fn read_fmt_config() -> Result<crate::fmt::FmtConfig> {
    use crate::fmt::{FmtConfig, Style};
    let content = std::fs::read_to_string("jex.toml")
        .map_err(|_| Error::new("无法读取 jex.toml".to_string()))?;
    let value: toml::Value = content
        .parse()
        .map_err(|e| Error::new(format!("解析 jex.toml 失败: {e}")))?;
    let mut config = FmtConfig::default();
    if let Some(fmt_section) = value.get("fmt") {
        if let Some(style_str) = fmt_section.get("style").and_then(|v| v.as_str()) {
            if let Ok(style) = style_str.parse::<Style>() {
                config.style = style;
            }
        }
        if let Some(aosp) = fmt_section.get("aosp").and_then(|v| v.as_bool()) {
            config.aosp = aosp;
        }
        if let Some(skip_future) = fmt_section.get("skip_future").and_then(|v| v.as_bool()) {
            config.skip_future = skip_future;
        }
        if let Some(exclude) = fmt_section.get("exclude").and_then(|v| v.as_array()) {
            config.exclude = exclude
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        }
    }
    Ok(config)
}

/// 读取全局语言配置
/// 优先级: ~/.jex/config.toml [i18n] lang > JEX_LANG 环境变量 > 默认 English
pub fn read_language_config() -> crate::i18n::Lang {
    if let Ok(home) = jex_home() {
        let f = home.join("config.toml");
        if let Ok(s) = std::fs::read_to_string(&f) {
            if let Ok(v) = s.parse::<toml::Value>() {
                if let Some(ls) = v
                    .get("i18n")
                    .and_then(|s| s.get("lang"))
                    .and_then(|v| v.as_str())
                {
                    if let Some(lang) = crate::i18n::Lang::parse_lang(ls) {
                        return lang;
                    }
                }
            }
        }
    }
    if let Ok(e) = std::env::var("JEX_LANG") {
        if let Some(lang) = crate::i18n::Lang::parse_lang(&e) {
            return lang;
        }
    }
    crate::i18n::Lang::En
}

/// Read a config value by dotted key (e.g. "i18n.lang")
pub fn config_get(key: &str) -> Result<String> {
    let home = jex_home()?;
    let path = home.join("config.toml");
    let content = std::fs::read_to_string(&path)
        .map_err(|_| Error::new(format!("No config at {}", path.display())))?;
    let value: toml::Value = content
        .parse()
        .map_err(|e| Error::new(format!("Invalid TOML: {e}")))?;
    let parts: Vec<&str> = key.split('.').collect();
    let mut cur = &value;
    for part in &parts {
        cur = cur
            .get(*part)
            .ok_or_else(|| Error::new(format!("Key not found: {key}")))?;
    }
    Ok(cur.to_string())
}

/// Set a config value by dotted key (e.g. "i18n.lang" = "en")
pub fn config_set(key: &str, value: &str) -> Result<String> {
    let home = jex_home()?;
    std::fs::create_dir_all(&home).ok();
    let path = home.join("config.toml");
    let mut config: toml::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| s.parse::<toml::Value>().ok())
        .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
    let parts: Vec<&str> = key.split('.').collect();
    if parts.len() != 2 {
        return Err(Error::new(
            "Key must be section.field (e.g. i18n.lang)".to_string(),
        ));
    }
    let table = config.as_table_mut().unwrap();
    let sub = table
        .entry(parts[0])
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .unwrap();
    if let Ok(b) = value.parse::<bool>() {
        sub.insert(parts[1].into(), toml::Value::Boolean(b));
    } else if let Ok(i) = value.parse::<i64>() {
        sub.insert(parts[1].into(), toml::Value::Integer(i));
    } else {
        sub.insert(parts[1].into(), toml::Value::String(value.to_string()));
    }
    let toml_str = toml::to_string_pretty(&config)?;
    // Atomic write: write to temp file, then rename
    let tmp_path = path.with_extension("toml.tmp");
    std::fs::write(&tmp_path, &toml_str)
        .map_err(|e| Error::new(format!("Write config: {e}")))?;
    std::fs::rename(&tmp_path, &path)
        .map_err(|e| Error::new(format!("Rename config: {e}")))?;
    Ok(format!("Set {key} = {value} in {}", path.display()))
}

/// List all config values
pub fn config_list() -> Result<String> {
    let home = jex_home()?;
    let path = home.join("config.toml");
    if path.exists() {
        let content =
            std::fs::read_to_string(&path).map_err(|e| Error::new(format!("Read error: {e}")))?;
        Ok(content)
    } else {
        Ok(format!(
            "No config at {}\nCreate one: jex config set i18n.lang en",
            path.display()
        ))
    }
}


/// Read jdk.default_version from config, or None
pub fn config_default_jdk_version() -> Option<String> {
    config_get("jdk.default_version").ok().map(|s| {
        // toml::Value::String serializes as "value" with quotes
        s.trim_matches('"').to_string()
    }).filter(|s| !s.is_empty())
}

/// Read jdk.mirror from config, or None
pub fn config_jdk_mirror() -> Option<String> {
    config_get("jdk.mirror").ok().map(|s| s.trim_matches('"').to_string()).filter(|s| !s.is_empty())
}

/// Read proxy.http from config, or None
pub fn config_proxy_http() -> Option<String> {
    config_get("proxy.http").ok().map(|s| s.trim_matches('"').to_string()).filter(|s| !s.is_empty())
}

/// Read proxy.https from config, or None
pub fn config_proxy_https() -> Option<String> {
    config_get("proxy.https").ok().map(|s| s.trim_matches('"').to_string()).filter(|s| !s.is_empty())
}

/// Read cache.max_size from config, or default 1GB
pub fn config_cache_max_size() -> u64 {
    config_get("cache.max_size").ok()
        .and_then(|s| s.trim_matches('"').parse::<u64>().ok())
        .unwrap_or(1024 * 1024 * 1024)
}

/// Read maven.central_mirror from config, or None
pub fn config_maven_mirror() -> Option<String> {
    config_get("maven.central_mirror").ok().map(|s| s.trim_matches('"').to_string()).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_jex_home() {
        let home = jex_home().unwrap();
        assert!(home.to_string_lossy().contains(".jex"));
        // 确保是绝对路径
        assert!(home.is_absolute());
    }

    #[test]
    fn test_jex_m2_cache() {
        let cache = jex_m2_cache().unwrap();
        assert!(cache.to_string_lossy().contains("m2"));
        assert!(cache.to_string_lossy().contains(".jex"));
        // 确保目录已创建
        assert!(cache.exists());
    }

    #[test]
    fn test_jex_m2_cache_idempotent() {
        // 多次调用应该成功（幂等）
        let cache1 = jex_m2_cache().unwrap();
        let cache2 = jex_m2_cache().unwrap();
        assert_eq!(cache1, cache2);
    }

    #[test]
    #[serial]
    fn test_read_fmt_config_full() {
        let tmp = tempfile::tempdir().unwrap();
        let toml_content = r#"
[fmt]
style = "AOSP"
aosp = true
skip_future = true
exclude = ["build/", "out/", "generated/"]
"#;
        std::fs::write(tmp.path().join("jex.toml"), toml_content).unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let config = read_fmt_config().unwrap();
        std::env::set_current_dir(&orig).unwrap();
        assert_eq!(config.style, crate::fmt::Style::Aosp);
        assert!(config.aosp);
        assert!(config.skip_future);
        assert_eq!(config.exclude, vec!["build/", "out/", "generated/"]);
    }

    #[test]
    #[serial]
    fn test_read_fmt_config_minimal() {
        let tmp = tempfile::tempdir().unwrap();
        let toml_content = r#"
[fmt]
style = "Google"
"#;
        std::fs::write(tmp.path().join("jex.toml"), toml_content).unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let config = read_fmt_config().unwrap();
        std::env::set_current_dir(&orig).unwrap();
        assert_eq!(config.style, crate::fmt::Style::Google);
        assert!(!config.aosp);
        assert!(!config.skip_future);
    }

    #[test]
    #[serial]
    fn test_read_fmt_config_no_fmt_section() {
        let tmp = tempfile::tempdir().unwrap();
        let toml_content = r#"
[project]
name = "my-app"
"#;
        std::fs::write(tmp.path().join("jex.toml"), toml_content).unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let config = read_fmt_config().unwrap();
        std::env::set_current_dir(&orig).unwrap();
        // Should use defaults
        assert_eq!(config.style, crate::fmt::Style::Google);
        assert!(!config.aosp);
        assert!(!config.skip_future);
        assert!(config.exclude.contains(&"build/".to_string()));
    }

    #[test]
    #[serial]
    fn test_read_fmt_config_invalid_toml() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("jex.toml"), "{{{{invalid").unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let result = read_fmt_config();
        std::env::set_current_dir(&orig).unwrap();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("解析 jex.toml 失败"));
    }

    #[test]
    #[serial]
    fn test_read_fmt_config_invalid_style() {
        let tmp = tempfile::tempdir().unwrap();
        let toml_content = r#"
[fmt]
style = "InvalidStyle"
"#;
        std::fs::write(tmp.path().join("jex.toml"), toml_content).unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let config = read_fmt_config().unwrap();
        std::env::set_current_dir(&orig).unwrap();
        // Invalid style should keep default (Google)
        assert_eq!(config.style, crate::fmt::Style::Google);
    }

    #[test]
    #[serial]
    fn test_read_fmt_config_exclude_with_non_strings() {
        let tmp = tempfile::tempdir().unwrap();
        let toml_content = r#"
[fmt]
exclude = ["build/", 123, true, "out/"]
"#;
        std::fs::write(tmp.path().join("jex.toml"), toml_content).unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let config = read_fmt_config().unwrap();
        std::env::set_current_dir(&orig).unwrap();
        // Non-string items should be filtered out
        assert_eq!(config.exclude, vec!["build/", "out/"]);
    }
}
