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
    entries.sort_by_key(|e| std::cmp::Reverse(e.size));

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
    #[test]
    fn test_dir_size_empty() {
        let dir = tempfile::tempdir().unwrap();
        let size = dir_size(dir.path()).unwrap();
        assert_eq!(size, 0);
    }

    #[test]
    fn test_dir_size_with_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), vec![0u8; 100]).unwrap();
        std::fs::write(dir.path().join("b.txt"), vec![0u8; 200]).unwrap();
        let size = dir_size(dir.path()).unwrap();
        assert_eq!(size, 300);
    }

    #[test]
    fn test_dir_size_recursive() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), vec![0u8; 100]).unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("b.txt"), vec![0u8; 200]).unwrap();
        let size = dir_size(dir.path()).unwrap();
        assert_eq!(size, 300);
    }

    #[test]
    fn test_dir_size_not_dir() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file.txt");
        std::fs::write(&file, vec![0u8; 50]).unwrap();
        let size = dir_size(&file).unwrap();
        assert_eq!(size, 0);
    }

    #[test]
    fn test_clean_cache_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let nonexistent = dir.path().join("does_not_exist");
        // clean_cache for a nonexistent path should print warning and return Ok
        // We can't easily test global=true as it uses cache_path
        // Instead test the non-global path behavior by creating a temp cache dir
        let cache = std::env::current_dir().unwrap().join(".jex").join("cache");
        // Just verify the function works when cache doesn't exist
        if !cache.exists() {
            let result = clean_cache(false);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_cache_entry_serde() {
        let entry = CacheEntry {
            name: "dep.jar".to_string(),
            size: 2048,
            modified: "2026-09-07T10:00:00".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: CacheEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "dep.jar");
        assert_eq!(deserialized.size, 2048);
        assert_eq!(deserialized.modified, "2026-09-07T10:00:00");
    }

    #[test]
    fn test_cache_entry_debug() {
        let entry = CacheEntry {
            name: "test".to_string(),
            size: 0,
            modified: String::new(),
        };
        let debug_str = format!("{:?}", entry);
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_cache_entry_clone() {
        let entry = CacheEntry {
            name: "x".to_string(),
            size: 42,
            modified: "y".to_string(),
        };
        let cloned = entry.clone();
        assert_eq!(entry.name, cloned.name);
        assert_eq!(entry.size, cloned.size);
    }

    #[test]
    fn test_format_size_edge_cases() {
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024 * 1024 - 1), "1024.0 KB");
        assert_eq!(format_size(1024 * 1024 * 1024 - 1), "1024.0 MB");
    }
}
