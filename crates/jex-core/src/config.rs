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
}
