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
                if let Some(ls) = v.get("i18n").and_then(|s| s.get("lang")).and_then(|v| v.as_str()) {
                    if let Some(lang) = crate::i18n::Lang::from_str(ls) { return lang; }
                }
            }
        }
    }
    if let Ok(e) = std::env::var("JEX_LANG") {
        if let Some(lang) = crate::i18n::Lang::from_str(&e) { return lang; }
    }
    crate::i18n::Lang::En
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
