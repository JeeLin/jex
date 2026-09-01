//! 本地状态与配置
//! - ~/.jex 全局目录(缓存 / 已装 JDK / 配置)
//! - 项目级 jex.toml 脚手架
//! - cs CLI 自动下载

use crate::error::{Error, Result};
use crate::util::{cs_arch_str, cs_os_str};
use std::path::PathBuf;

/// 返回 ~/.jex 全局目录路径。
pub fn jex_home() -> Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| Error::new("找不到 HOME 环境变量"))?;
    Ok(PathBuf::from(home).join(".jex"))
}

/// 返回 ~/.jex/bin/ 目录路径。
fn bin_dir() -> Result<PathBuf> {
    let dir = jex_home()?.join("bin");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 返回 cs 可执行文件路径。
fn cs_path() -> Result<PathBuf> {
    Ok(bin_dir()?.join("cs"))
}

/// 确保 cs CLI 可用：检查本地是否存在，不存在则从 GitHub Releases 下载。
/// 返回 cs 可执行文件的完整路径。
pub fn ensure_cs() -> Result<PathBuf> {
    let path = cs_path()?;
    if path.exists() {
        return Ok(path);
    }

    println!("cs 未安装，正在自动下载...");

    let url = cs_download_url()?;
    let temp_path = bin_dir()?.join("cs.tmp");

    // 下载
    let status = std::process::Command::new("curl")
        .args(["-L", "-o", temp_path.to_str().unwrap(), &url])
        .status()?;

    if !status.success() {
        let _ = std::fs::remove_file(&temp_path);
        return Err(Error::new("下载 cs 失败"));
    }

    // 设置可执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&temp_path, perms)?;
    }

    // 重命名为最终路径
    std::fs::rename(&temp_path, &path)?;

    println!("cs 安装成功: {}", path.display());
    Ok(path)
}

/// 根据当前 OS 和 ARCH 构造 cs 下载 URL。
/// F11 修复：通过 `cs_os_str()` / `cs_arch_str()` 复用 util.rs 中的 OS/ARCH 映射。
fn cs_download_url() -> Result<String> {
    let os = cs_os_str()?;
    let arch = cs_arch_str()?;
    Ok(format!(
        "https://github.com/coursier/coursier/releases/latest/download/cs-{}-{}",
        os, arch
    ))
}
