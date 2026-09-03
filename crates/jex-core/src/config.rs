//! 本地状态与配置
//! - ~/.jex 全局目录(缓存 / 已装 JDK / 配置)
//! - 项目级 jex.toml 脚手架

use crate::error::{Error, Result};
use std::path::PathBuf;

/// 返回 ~/.jex 全局目录路径。
pub fn jex_home() -> Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| Error::new("找不到 HOME 环境变量"))?;
    Ok(PathBuf::from(home).join(".jex"))
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_read_fmt_config_no_file() {
        // 在没有 jex.toml 的目录调用应返回 Err
        let result = read_fmt_config();
        assert!(result.is_err());
    }
}
