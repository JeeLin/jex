//! 公共工具函数

use crate::error::{Error, Result};

/// 解析 Maven 坐标 `group:artifact` 格式
pub fn parse_coord(coord: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() < 2 {
        return Err(Error::new(format!(
            "无效的坐标格式: {}（应为 group:artifact）",
            coord
        )));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// 将 Rust `std::env::consts::OS` 转为 Adoptium API URL 用的操作系统名。
/// F11 修复：与 cs_os_str 同样在 jdk.rs 中重复，统一到 util.rs。
pub fn adoptium_os_str() -> Result<&'static str> {
    match std::env::consts::OS {
        "linux" => Ok("linux"),
        "macos" => Ok("mac"),
        "windows" => Ok("windows"),
        _ => Err(Error::new(format!(
            "不支持的操作系统: {}",
            std::env::consts::OS
        ))),
    }
}

/// 将 Rust `std::env::consts::ARCH` 转为 Adoptium API URL 用的架构名。
/// F11 修复：与 cs_arch_str 同样在 jdk.rs 中重复，统一到 util.rs。
pub fn adoptium_arch_str() -> Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("x64"),
        "aarch64" => Ok("aarch64"),
        _ => Err(Error::new(format!(
            "不支持的架构: {}",
            std::env::consts::ARCH
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_coord_valid() {
        let (g, a) = parse_coord("com.google.code.gson:gson").unwrap();
        assert_eq!(g, "com.google.code.gson");
        assert_eq!(a, "gson");
    }

    #[test]
    fn test_parse_coord_simple() {
        let (g, a) = parse_coord("org.slf4j:slf4j-api").unwrap();
        assert_eq!(g, "org.slf4j");
        assert_eq!(a, "slf4j-api");
    }

    #[test]
    fn test_parse_coord_no_colon() {
        let result = parse_coord("invalid-coord");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("无效的坐标格式"));
    }

    #[test]
    fn test_parse_coord_empty() {
        let result = parse_coord("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_coord_only_colon() {
        // ":" splits to ["", ""] which has 2 elements, so it's valid
        let (g, a) = parse_coord(":").unwrap();
        assert_eq!(g, "");
        assert_eq!(a, "");
    }

    #[test]
    fn test_adoptium_os_str() {
        let result = adoptium_os_str();
        assert!(result.is_ok());
        // 在 Linux CI 中
        let os = result.unwrap();
        assert!(os == "linux" || os == "mac" || os == "windows");
    }

    #[test]
    fn test_adoptium_arch_str() {
        let result = adoptium_arch_str();
        assert!(result.is_ok());
        let arch = result.unwrap();
        assert!(arch == "x64" || arch == "aarch64");
    }
}
