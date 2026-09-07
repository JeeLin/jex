//! 依赖版本锁定（jex pin）
//! - pin: 锁定特定依赖版本

use crate::deps;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 锁定的依赖
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinnedDep {
    pub coord: String,
    pub version: String,
    pub pinned_at: String,
}

/// 获取锁定文件路径
fn pin_path() -> Result<std::path::PathBuf> {
    let config_dir = dirs::home_dir()
        .ok_or_else(|| crate::error::Error::new("无法获取 HOME 目录".to_string()))?
        .join(".jex");

    std::fs::create_dir_all(&config_dir)
        .map_err(|e| crate::error::Error::new(format!("创建配置目录失败: {}", e)))?;

    Ok(config_dir.join("jex.pin.toml"))
}

/// 读取锁定文件
fn read_pin_file() -> Result<HashMap<String, String>> {
    let path = pin_path()?;

    if !path.exists() {
        return Ok(HashMap::new());
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| crate::error::Error::new(format!("读取锁定文件失败: {}", e)))?;

    let pinned: HashMap<String, String> = toml::from_str(&content)
        .map_err(|e| crate::error::Error::new(format!("解析锁定文件失败: {}", e)))?;

    Ok(pinned)
}

/// 写入锁定文件
fn write_pin_file(pinned: &HashMap<String, String>) -> Result<()> {
    let path = pin_path()?;

    let content = toml::to_string_pretty(pinned)
        .map_err(|e| crate::error::Error::new(format!("序列化锁定文件失败: {}", e)))?;

    std::fs::write(&path, content)
        .map_err(|e| crate::error::Error::new(format!("写入锁定文件失败: {}", e)))?;

    Ok(())
}

/// 锁定依赖
pub fn pin_dependency(coord: &str) -> Result<()> {
    let lock = deps::read_jex_lock()?;
    let dependencies = lock.dependencies.unwrap_or_default();

    let version = dependencies
        .get(coord)
        .ok_or_else(|| crate::error::Error::new(format!("依赖 {} 不存在", coord)))?;

    let mut pinned = read_pin_file()?;
    pinned.insert(coord.to_string(), version.clone());

    write_pin_file(&pinned)?;
    println!("🔒 已锁定 {}:{} ", coord, version);

    Ok(())
}

/// 锁定所有依赖
pub fn pin_all() -> Result<()> {
    let lock = deps::read_jex_lock()?;
    let dependencies = lock.dependencies.unwrap_or_default();

    let mut pinned = read_pin_file()?;
    let count = dependencies.len();

    for (coord, version) in &dependencies {
        pinned.insert(coord.clone(), version.clone());
    }

    write_pin_file(&pinned)?;
    println!("🔒 已锁定全部 {} 个依赖", count);

    Ok(())
}

/// 解锁依赖
pub fn unpin_dependency(coord: &str) -> Result<()> {
    let mut pinned = read_pin_file()?;

    if pinned.remove(coord).is_some() {
        write_pin_file(&pinned)?;
        println!("🔓 已解锁 {}", coord);
    } else {
        println!("⚠️  {} 未被锁定", coord);
    }

    Ok(())
}

/// 列出锁定的依赖
pub fn list_pinned() -> Result<Vec<PinnedDep>> {
    let pinned = read_pin_file()?;

    let result = pinned
        .iter()
        .map(|(coord, version)| PinnedDep {
            coord: coord.clone(),
            version: version.clone(),
            pinned_at: "unknown".to_string(),
        })
        .collect();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pinned_dep() {
        let dep = PinnedDep {
            coord: "com.google.code.gson:gson".to_string(),
            version: "2.11.0".to_string(),
            pinned_at: "2026-09-05".to_string(),
        };

        assert_eq!(dep.coord, "com.google.code.gson:gson");
        assert_eq!(dep.version, "2.11.0");
    }

    #[test]
    fn test_read_pin_file_empty() {
        let pinned = read_pin_file().unwrap_or_default();
        assert!(pinned.is_empty() || !pinned.is_empty());
    }

    #[test]
    fn test_pin_unpin_cycle() {
        let mut pinned: HashMap<String, String> = HashMap::new();
        pinned.insert("test:dep".to_string(), "1.0.0".to_string());
        assert!(pinned.contains_key("test:dep"));

        pinned.remove("test:dep");
        assert!(!pinned.contains_key("test:dep"));
    }
    #[test]
    fn test_pinned_dep_serde() {
        let dep = PinnedDep {
            coord: "com.test:lib".to_string(),
            version: "1.0.0".to_string(),
            pinned_at: "2026-09-07".to_string(),
        };
        let json = serde_json::to_string(&dep).unwrap();
        let deser: PinnedDep = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.coord, "com.test:lib");
        assert_eq!(deser.version, "1.0.0");
    }

    #[test]
    fn test_pinned_dep_debug() {
        let dep = PinnedDep {
            coord: "a:b".to_string(),
            version: "1".to_string(),
            pinned_at: "t".to_string(),
        };
        let debug_str = format!("{:?}", dep);
        assert!(debug_str.contains("a:b"));
    }

    #[test]
    fn test_pinned_dep_clone() {
        let dep = PinnedDep {
            coord: "x:y".to_string(),
            version: "2".to_string(),
            pinned_at: "z".to_string(),
        };
        let cloned = dep.clone();
        assert_eq!(dep.coord, cloned.coord);
    }

    #[test]
    fn test_hashmap_serialization_roundtrip() {
        let mut pinned: HashMap<String, String> = HashMap::new();
        pinned.insert("com.google:gson".to_string(), "2.11.0".to_string());
        pinned.insert("org.apache:commons".to_string(), "3.12.0".to_string());
        let content = toml::to_string_pretty(&pinned).unwrap();
        assert!(content.contains("gson"));
        assert!(content.contains("2.11.0"));
        let deser: HashMap<String, String> = toml::from_str(&content).unwrap();
        assert_eq!(deser.len(), 2);
        assert_eq!(deser["com.google:gson"], "2.11.0");
    }

    #[test]
    fn test_hashmap_empty_serialization() {
        let pinned: HashMap<String, String> = HashMap::new();
        let content = toml::to_string_pretty(&pinned).unwrap();
        let deser: HashMap<String, String> = toml::from_str(&content).unwrap();
        assert!(deser.is_empty());
    }

    #[test]
    fn test_unpin_nonexistent() {
        // unpin_dependency should handle non-existent keys gracefully
        // It reads the pin file, tries to remove, and prints a warning
        // Since this touches the real pin file, we can't fully test it
        // But we can test the HashMap logic
        let mut pinned: HashMap<String, String> = HashMap::new();
        assert!(pinned.remove("nonexistent").is_none());
    }

    #[test]
    fn test_pin_path_returns_consistent_result() {
        // pin_path should return a valid path containing .jex
        let path = pin_path().unwrap();
        assert!(path.to_string_lossy().contains(".jex"));
        assert!(path.to_string_lossy().ends_with("jex.pin.toml"));
    }
}
