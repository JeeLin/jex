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

/// 将 Rust `std::env::consts::OS` 转为 Coursier（cs）下载 URL 用的操作系统名。
/// F11 修复：消除 config.rs 与 jdk.rs 中重复的 OS match 块（cs 使用的值）。
pub fn cs_os_str() -> Result<&'static str> {
    match std::env::consts::OS {
        "linux" => Ok("linux"),
        "macos" => Ok("macos"),
        _ => Err(Error::new(format!(
            "不支持的操作系统: {}",
            std::env::consts::OS
        ))),
    }
}

/// 将 Rust `std::env::consts::ARCH` 转为 Coursier（cs）下载 URL 用的架构名。
/// F11 修复：消除 config.rs 与 jdk.rs 中重复的 ARCH match 块（cs 使用的值）。
pub fn cs_arch_str() -> Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("amd64"),
        "aarch64" => Ok("arm64"),
        _ => Err(Error::new(format!(
            "不支持的架构: {}",
            std::env::consts::ARCH
        ))),
    }
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
