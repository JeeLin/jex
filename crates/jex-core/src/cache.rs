//! 依赖缓存管理（jex cache）
//! - cache: 管理依赖缓存

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 缓存条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub name: String,
    pub size: u64,
    pub modified: String,
}

/// 获取缓存目录路径
pub fn cache_path() -> Result<PathBuf> {
    // 优先查找 Coursier 缓存目录
    let home = dirs::home_dir()
        .ok_or_else(|| crate::error::Error::new("无法获取 HOME 目录".to_string()))?;

    // 尝试常见缓存位置
    let candidates = vec![
        home.join(".cache").join("coursier"),
        home.join(".coursier").join("cache"),
        home.join(".jex").join("cache"),
    ];

    for path in candidates {
        if path.exists() {
            return Ok(path);
        }
    }

    // 默认使用 .jex/cache
    let default_path = home.join(".jex").join("cache");
    std::fs::create_dir_all(&default_path)
        .map_err(|e| crate::error::Error::new(format!("创建缓存目录失败: {}", e)))?;

    Ok(default_path)
}

/// 计算目录大小
fn dir_size(path: &std::path::Path) -> Result<u64> {
    let mut total = 0;

    if path.is_dir() {
        for entry in std::fs::read_dir(path)
            .map_err(|e| crate::error::Error::new(format!("读取目录失败: {}", e)))?
        {
            let entry = entry.map_err(|e| crate::error::Error::new(format!("读取条目失败: {}", e)))?;
            let metadata = entry
                .metadata()
                .map_err(|e| crate::error::Error::new(format!("获取元数据失败: {}", e)))?;

            if metadata.is_file() {
                total += metadata.len();
            } else if metadata.is_dir() {
                total += dir_size(&entry.path())?;
            }
        }
    }

    Ok(total)
}

/// 格式化文件大小
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// 清理缓存
pub fn clean_cache(global: bool) -> Result<()> {
    let path = if global {
        cache_path()?
    } else {
        // 项目级缓存：当前目录下的 .jex/cache
        std::env::current_dir()
            .map_err(|e| crate::error::Error::new(format!("获取当前目录失败: {}", e)))?
            .join(".jex")
            .join("cache")
    };

    if !path.exists() {
        println!("⚠️  缓存目录不存在: {}", path.display());
        return Ok(());
    }

    let size_before = dir_size(&path)?;
    println!("📂 缓存目录: {}", path.display());
    println!("📊 当前缓存大小: {}", format_size(size_before));

    // 清理缓存
    std::fs::remove_dir_all(&path)
        .map_err(|e| crate::error::Error::new(format!("清理缓存失败: {}", e)))?;

    // 重新创建目录
    std::fs::create_dir_all(&path)
        .map_err(|e| crate::error::Error::new(format!("创建缓存目录失败: {}", e)))?;

    println!("✅ 缓存已清理，释放 {}", format_size(size_before));

    Ok(())
}

/// 列出缓存内容
pub fn list_cache() -> Result<Vec<CacheEntry>> {
    let path = cache_path()?;

    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();

    for entry in std::fs::read_dir(&path)
        .map_err(|e| crate::error::Error::new(format!("读取缓存目录失败: {}", e)))?
    {
        let entry = entry.map_err(|e| crate::error::Error::new(format!("读取条目失败: {}", e)))?;
        let metadata = entry
            .metadata()
            .map_err(|e| crate::error::Error::new(format!("获取元数据失败: {}", e)))?;

        let name = entry.file_name().to_string_lossy().to_string();
        let size = if metadata.is_file() {
            metadata.len()
        } else if metadata.is_dir() {
            dir_size(&entry.path())?
        } else {
            0
        };

        let modified = metadata
            .modified()
            .map(|t| {
                let datetime: std::time::SystemTime = t;
                format!("{:?}", datetime)
            })
            .unwrap_or_else(|_| "unknown".to_string());

        entries.push(CacheEntry {
            name,
            size,
            modified,
        });
    }

    // 按大小排序
    entries.sort_by(|a, b| b.size.cmp(&a.size));

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_entry() {
        let entry = CacheEntry {
            name: "test".to_string(),
            size: 1024,
            modified: "2026-09-05".to_string(),
        };

        assert_eq!(entry.name, "test");
        assert_eq!(entry.size, 1024);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(100), "100 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn test_cache_path() {
        let path = cache_path().unwrap();
        assert!(path.to_string_lossy().contains(".jex") || path.to_string_lossy().contains("coursier"));
    }
}
