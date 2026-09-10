//! 自更新模块：检查最新版本、下载二进制、原子替换

use crate::error::{Error, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// GitHub 仓库所有者/名称
const GITHUB_REPO: &str = "JeeLin/jex";

/// 版本信息
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateInfo {
    /// 最新版本号（如 "0.10.0"）
    pub latest_version: String,
    /// 下载 URL
    pub download_url: String,
    /// 版本说明
    pub release_notes: String,
}

/// GitHub Release API 响应
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

/// 获取当前版本号
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// 检查最新版本
pub fn check_latest() -> Result<UpdateInfo> {
    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        GITHUB_REPO
    );
    let client = reqwest::blocking::Client::builder()
        .user_agent("jex-updater")
        .build()
        .map_err(|e| Error::new(format!("创建 HTTP 客户端失败: {}", e)))?;

    let response = client
        .get(&url)
        .send()
        .map_err(|e| Error::new(format!("请求 GitHub API 失败: {}", e)))?;

    if !response.status().is_success() {
        return Err(Error::new(format!(
            "GitHub API 返回错误: {}",
            response.status()
        )));
    }

    let release: GitHubRelease = response
        .json()
        .map_err(|e| Error::new(format!("解析 GitHub API 响应失败: {}", e)))?;

    let tag = release.tag_name.trim_start_matches('v').to_string();
    let platform = detect_platform()?;
    let download_url = find_asset_url(&release.assets, &platform)?;

    Ok(UpdateInfo {
        latest_version: tag,
        download_url,
        release_notes: release.body.unwrap_or_default(),
    })
}

/// 判断是否需要更新
pub fn needs_update(current: &str, latest: &str) -> bool {
    let current_parts: Vec<u32> = current.split('.').filter_map(|s| s.parse().ok()).collect();
    let latest_parts: Vec<u32> = latest.split('.').filter_map(|s| s.parse().ok()).collect();

    for (c, l) in current_parts.iter().zip(latest_parts.iter()) {
        if l > c {
            return true;
        } else if l < c {
            return false;
        }
    }
    latest_parts.len() > current_parts.len()
}

/// 检测当前平台
pub fn detect_platform() -> Result<(String, String)> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let os_str = match os {
        "linux" => "linux",
        "macos" => "macos",
        "windows" => "windows",
        _ => return Err(Error::new(format!("不支持的操作系统: {}", os))),
    };

    let arch_str = match arch {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => return Err(Error::new(format!("不支持的架构: {}", arch))),
    };

    Ok((os_str.to_string(), arch_str.to_string()))
}

/// 从 assets 中查找匹配的下载 URL
fn find_asset_url(assets: &[GitHubAsset], platform: &(String, String)) -> Result<String> {
    let suffix = match platform.0.as_str() {
        "windows" => ".exe",
        _ => "",
    };

    let pattern = format!("{}-{}", platform.0, platform.1);

    for asset in assets {
        if asset.name.contains(&pattern) && asset.name.ends_with(suffix) {
            return Ok(asset.browser_download_url.clone());
        }
    }

    Err(Error::new(format!("未找到匹配的二进制文件: {}", pattern)))
}

/// 下载二进制到指定路径
pub fn download_binary(url: &str, dest: &Path) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("jex-updater")
        .build()
        .map_err(|e| Error::new(format!("创建 HTTP 客户端失败: {}", e)))?;

    let response = client
        .get(url)
        .send()
        .map_err(|e| Error::new(format!("下载失败: {}", e)))?;

    if !response.status().is_success() {
        return Err(Error::new(format!("下载返回错误: {}", response.status())));
    }

    let bytes = response
        .bytes()
        .map_err(|e| Error::new(format!("读取响应体失败: {}", e)))?;

    std::fs::write(dest, &bytes).map_err(|e| Error::new(format!("写入文件失败: {}", e)))?;

    // Unix: 设置可执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(dest)
            .map_err(|e| Error::new(format!("获取文件权限失败: {}", e)))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(dest, perms)
            .map_err(|e| Error::new(format!("设置文件权限失败: {}", e)))?;
    }

    Ok(())
}

/// 原子替换当前二进制
pub fn atomic_replace(current_path: &Path, new_path: &Path) -> Result<()> {
    // 1. 备份当前二进制
    let backup_path = current_path.with_extension("bak");

    // 2. 重命名当前二进制为备份
    std::fs::rename(current_path, &backup_path)
        .map_err(|e| Error::new(format!("备份当前二进制失败: {}", e)))?;

    // 3. 尝试重命名新二进制为当前路径
    match std::fs::rename(new_path, current_path) {
        Ok(_) => {
            // 成功：删除备份
            let _ = std::fs::remove_file(&backup_path);
            Ok(())
        }
        Err(e) => {
            // 失败：恢复备份
            let _ = std::fs::rename(&backup_path, current_path);
            Err(Error::new(format!("替换二进制失败: {}", e)))
        }
    }
}

/// 获取当前二进制路径
pub fn current_binary_path() -> Result<PathBuf> {
    std::env::current_exe().map_err(|e| Error::new(format!("获取当前可执行文件路径失败: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_version() {
        let version = current_version();
        assert!(!version.is_empty());
    }

    #[test]
    fn test_needs_update_major() {
        assert!(needs_update("0.9.0", "1.0.0"));
        assert!(!needs_update("1.0.0", "0.9.0"));
    }

    #[test]
    fn test_needs_update_minor() {
        assert!(needs_update("0.9.0", "0.10.0"));
        assert!(!needs_update("0.10.0", "0.9.0"));
    }

    #[test]
    fn test_needs_update_patch() {
        assert!(needs_update("0.10.0", "0.10.1"));
        assert!(!needs_update("0.10.1", "0.10.0"));
    }

    #[test]
    fn test_needs_update_same() {
        assert!(!needs_update("0.10.0", "0.10.0"));
    }

    #[test]
    fn test_detect_platform() {
        let result = detect_platform();
        assert!(result.is_ok());
        let (os, arch) = result.unwrap();
        assert!(["linux", "macos", "windows"].contains(&os.as_str()));
        assert!(["amd64", "arm64"].contains(&arch.as_str()));
    }

    #[test]
    fn test_find_asset_url() {
        let assets = vec![
            GitHubAsset {
                name: "jex-linux-amd64".to_string(),
                browser_download_url: "https://example.com/jex-linux-amd64".to_string(),
            },
            GitHubAsset {
                name: "jex-linux-arm64".to_string(),
                browser_download_url: "https://example.com/jex-linux-arm64".to_string(),
            },
            GitHubAsset {
                name: "jex-windows-amd64.exe".to_string(),
                browser_download_url: "https://example.com/jex-windows-amd64.exe".to_string(),
            },
        ];

        let url = find_asset_url(&assets, &("linux".to_string(), "amd64".to_string())).unwrap();
        assert_eq!(url, "https://example.com/jex-linux-amd64");

        let url = find_asset_url(&assets, &("windows".to_string(), "amd64".to_string())).unwrap();
        assert_eq!(url, "https://example.com/jex-windows-amd64.exe");
    }

    #[test]
    fn test_needs_update_equal_three_part() {
        assert!(!needs_update("1.2.3", "1.2.3"));
    }

    #[test]
    fn test_needs_update_major_reversed() {
        assert!(!needs_update("2.0.0", "1.0.0"));
    }

    #[test]
    fn test_needs_update_minor_reversed() {
        assert!(!needs_update("1.2.0", "1.1.0"));
    }

    #[test]
    fn test_needs_update_patch_reversed() {
        assert!(!needs_update("1.0.2", "1.0.1"));
    }

    #[test]
    fn test_find_asset_url_not_found() {
        let assets = vec![
            GitHubAsset {
                name: "jex-linux-amd64".to_string(),
                browser_download_url: "https://example.com/jex-linux-amd64".to_string(),
            },
        ];
        let result = find_asset_url(&assets, &("windows".to_string(), "arm64".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_find_asset_url_multiple_platforms() {
        let assets = vec![
            GitHubAsset { name: "jex-linux-amd64".to_string(), browser_download_url: "url1".to_string() },
            GitHubAsset { name: "jex-linux-arm64".to_string(), browser_download_url: "url2".to_string() },
            GitHubAsset { name: "jex-darwin-amd64".to_string(), browser_download_url: "url3".to_string() },
            GitHubAsset { name: "jex-darwin-arm64".to_string(), browser_download_url: "url4".to_string() },
            GitHubAsset { name: "jex-windows-amd64.exe".to_string(), browser_download_url: "url5".to_string() },
        ];
        assert_eq!(find_asset_url(&assets, &("linux".to_string(), "arm64".to_string())).unwrap(), "url2");
        assert_eq!(find_asset_url(&assets, &("darwin".to_string(), "amd64".to_string())).unwrap(), "url3");
        assert_eq!(find_asset_url(&assets, &("darwin".to_string(), "arm64".to_string())).unwrap(), "url4");
        assert_eq!(find_asset_url(&assets, &("windows".to_string(), "amd64".to_string())).unwrap(), "url5");
    }

    #[test]
    fn test_find_asset_url_empty_assets() {
        let assets: Vec<GitHubAsset> = vec![];
        let result = find_asset_url(&assets, &("linux".to_string(), "amd64".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn test_update_info_struct() {
        let info = UpdateInfo {
            latest_version: "1.0.0".to_string(),
            download_url: "https://example.com/jex".to_string(),
            release_notes: "Initial release".to_string(),
        };
        assert_eq!(info.latest_version, "1.0.0");
        assert!(info.download_url.contains("example.com"));
        assert_eq!(info.release_notes, "Initial release");
    }

    #[test]
    fn test_update_info_clone() {
        let info = UpdateInfo {
            latest_version: "2.0.0".to_string(),
            download_url: "https://example.com/jex".to_string(),
            release_notes: "v2".to_string(),
        };
        let cloned = info.clone();
        assert_eq!(cloned.latest_version, "2.0.0");
        assert_eq!(cloned.release_notes, "v2");
    }
}
