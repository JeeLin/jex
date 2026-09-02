//! JDK 版本管理（自动下载 + 版本切换）
//! - install: 下载并解压 Temurin 到 ~/.jex/jdks/<ver>/
//! - use: 写项目 .jex-version 或全局 ~/.jex/jdk-current
//! - list: 列出已装版本（✔）与当前版本（→）
//! - which: 打印当前生效 JDK 的 JAVA_HOME
//! - doctor: 检查项目声明 vs 实际已装是否一致

use crate::config::jex_home;
use crate::error::{Error, Result};
use crate::util::{adoptium_arch_str, adoptium_os_str};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
/// 获取 ~/.jex/jdks/ 目录路径
fn jdks_dir() -> Result<PathBuf> {
    Ok(jex_home()?.join("jdks"))
}

/// 获取全局默认 JDK 版本号路径 (~/.jex/jdk-current)
fn global_jdk_current_path() -> Result<PathBuf> {
    Ok(jex_home()?.join("jdk-current"))
}

/// 获取当前目录的 .jex-version 路径
fn project_jex_version_path() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    Ok(cwd.join(".jex-version"))
}

/// 读取指定路径的版本号（单行文本）
fn read_version_file(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    Ok(Some(content.trim().to_string()))
}

/// 写入版本号到文件
fn write_version_file(path: &Path, version: &str) -> Result<()> {
    fs::write(path, format!("{}\n", version))?;
    Ok(())
}

/// 获取当前生效的 JDK 版本号
/// 优先级：项目 .jex-version → 全局 ~/.jex/jdk-current → None
pub fn current_version() -> Result<Option<String>> {
    // 1. 项目级
    if let Some(v) = read_version_file(&project_jex_version_path()?)? {
        return Ok(Some(v));
    }
    // 2. 全局级
    if let Some(v) = read_version_file(&global_jdk_current_path()?)? {
        return Ok(Some(v));
    }
    Ok(None)
}

/// 获取当前生效 JDK 的 JAVA_HOME 路径
pub fn which_java_home() -> Result<PathBuf> {
    if let Some(ver) = current_version()? {
        let home = jdks_dir()?.join(&ver);
        if home.exists() {
            return Ok(home);
        }
        return Err(Error::new(format!(
            "版本 {} 已声明但未安装，请运行 jex jdk install {}",
            ver, ver
        )));
    }

    // 回退到系统 java
    Err(Error::new(
        "未设置 JDK 版本，请运行 jex jdk use <version> 或 jex jdk install <version>",
    ))
}

/// 列出已安装的 JDK 版本
pub fn list_installed() -> Result<Vec<String>> {
    let dir = jdks_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut versions = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                versions.push(name.to_string());
            }
        }
    }
    versions.sort();
    Ok(versions)
}

/// 安装 JDK 版本
pub fn install(version: &str) -> Result<()> {
    let dir = jdks_dir()?;
    fs::create_dir_all(&dir)?;

    let target_dir = dir.join(version);
    if target_dir.exists() {
        println!("JDK {} 已安装", version);
        return Ok(());
    }

    println!("正在从 Adoptium 下载 JDK {}...", version);

    // 构建下载 URL
    let os = adoptium_os_str()?;
    let arch = adoptium_arch_str()?;
    let url = format!(
        "https://api.adoptium.net/v3/binary/latest/{}/ga/{}/{}/jdk/hotspot/normal/eclipse",
        version, os, arch
    );

    // 下载并解压
    let temp_dir = dir.join(format!("{}.tmp", version));
    fs::create_dir_all(&temp_dir)?;

    // 使用 curl 下载
    let archive_name = if os == "windows" {
        "jdk.zip"
    } else {
        "jdk.tar.gz"
    };
    let archive_path = temp_dir.join(archive_name);

    let status = std::process::Command::new("curl")
        .args(["-L", "-o", archive_path.to_str().unwrap(), &url])
        .status()?;

    if !status.success() {
        fs::remove_dir_all(&temp_dir)?;
        return Err(Error::new("下载 JDK 失败"));
    }

    // 解压
    if os == "windows" {
        // Windows: 使用 PowerShell 解压
        let status = std::process::Command::new("powershell")
            .args([
                "-Command",
                &format!(
                    "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                    archive_path.display(),
                    temp_dir.display()
                ),
            ])
            .status()?;

        if !status.success() {
            fs::remove_dir_all(&temp_dir)?;
            return Err(Error::new("解压 JDK 失败"));
        }
    } else {
        // Linux/macOS: 使用 tar 解压
        let status = std::process::Command::new("tar")
            .args([
                "-xzf",
                archive_path.to_str().unwrap(),
                "-C",
                temp_dir.to_str().unwrap(),
            ])
            .status()?;

        if !status.success() {
            fs::remove_dir_all(&temp_dir)?;
            return Err(Error::new("解压 JDK 失败"));
        }
    }

    // 找到解压后的目录（通常以 jdk- 开头）
    let mut extracted_dir = None;
    for entry in fs::read_dir(&temp_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("jdk-") {
                    extracted_dir = Some(path);
                    break;
                }
            }
        }
    }

    let extracted_dir = extracted_dir.ok_or_else(|| Error::new("未找到解压后的 JDK 目录"))?;

    // 重命名到目标目录
    fs::rename(&extracted_dir, &target_dir)?;
    fs::remove_dir_all(&temp_dir)?;

    println!("JDK {} 安装成功: {}", version, target_dir.display());
    Ok(())
}

/// 切换 JDK 版本
pub fn use_version(version: &str) -> Result<()> {
    let dir = jdks_dir()?.join(version);
    if !dir.exists() {
        return Err(Error::new(format!(
            "JDK {} 未安装，请先运行 jex jdk install {}",
            version, version
        )));
    }

    // 检查是否在项目目录中
    let cwd = std::env::current_dir()?;
    let in_project = cwd.join("jex.toml").exists() || cwd.join(".jex-version").exists();

    if in_project {
        // 项目级：写 .jex-version
        write_version_file(&project_jex_version_path()?, version)?;
        println!("项目 JDK 版本已切换为 {} (.jex-version)", version);
    } else {
        // 全局级：写 ~/.jex/jdk-current
        write_version_file(&global_jdk_current_path()?, version)?;
        println!("全局 JDK 版本已切换为 {} (~/.jex/jdk-current)", version);
    }

    Ok(())
}

/// 列出已安装版本和当前版本
pub fn list() -> Result<()> {
    let installed = list_installed()?;
    let current = current_version()?;

    if installed.is_empty() {
        println!("未安装任何 JDK 版本");
        println!("  使用 jex jdk install <version> 安装");
        return Ok(());
    }

    println!("已安装的 JDK 版本:");
    for ver in &installed {
        let marker = match &current {
            Some(c) if c == ver => " →",
            _ => "   ",
        };
        println!("  {} {} ✔", marker, ver);
    }

    if current.is_none() {
        println!("\n未设置默认版本，请运行 jex jdk use <version>");
    }

    Ok(())
}

/// 打印当前生效 JDK 的 JAVA_HOME
pub fn which() -> Result<()> {
    let home = which_java_home()?;
    println!("{}", home.display());
    Ok(())
}

/// 校验跨设备一致性
pub fn doctor() -> Result<()> {
    let current = current_version()?;
    let installed = list_installed()?;

    println!("JDK 一致性检查:");

    match &current {
        Some(ver) => {
            let dir = jdks_dir()?.join(ver);
            if dir.exists() {
                println!("  ✅ 版本 {} 已声明且已安装", ver);
            } else {
                println!("  ❌ 版本 {} 已声明但未安装", ver);
                println!("     请运行: jex jdk install {}", ver);
            }
        }
        None => {
            println!("  ⚠️  未设置默认 JDK 版本");
            println!("     请运行: jex jdk use <version>");
        }
    }

    if installed.is_empty() {
        println!("  ⚠️  未安装任何 JDK 版本");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_jdks_dir() {
        let dir = jdks_dir().unwrap();
        assert!(dir.to_string_lossy().contains("jdks"));
        assert!(dir.to_string_lossy().contains(".jex"));
    }

    #[test]
    fn test_global_jdk_current_path() {
        let path = global_jdk_current_path().unwrap();
        assert!(path.to_string_lossy().contains("jdk-current"));
    }

    #[test]
    fn test_project_jex_version_path() {
        let path = project_jex_version_path().unwrap();
        assert!(path.to_string_lossy().contains(".jex-version"));
    }

    #[test]
    fn test_read_version_file_not_exists() {
        let path = PathBuf::from("/tmp/nonexistent-version-file");
        let result = read_version_file(&path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_read_version_file_exists() {
        let dir = std::env::temp_dir().join("jex-test-read-version");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("version.txt");
        let mut f = fs::File::create(&path).unwrap();
        writeln!(f, "21\n").unwrap();

        let result = read_version_file(&path).unwrap();
        assert_eq!(result, Some("21".to_string()));

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn test_write_version_file() {
        let dir = std::env::temp_dir().join("jex-test-write-version");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("version.txt");

        write_version_file(&path, "17").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.trim(), "17");

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }

    #[test]
    fn test_current_version_no_files() {
        // 在没有 .jex-version 的目录中调用
        let result = current_version();
        // 应该成功，可能返回 Some（全局）或 None
        assert!(result.is_ok());
    }

    #[test]
    fn test_adoptium_os_str_jdk() {
        let os = adoptium_os_str().unwrap();
        assert!(os == "linux" || os == "mac" || os == "windows");
    }

    #[test]
    fn test_adoptium_arch_str_jdk() {
        let arch = adoptium_arch_str().unwrap();
        assert!(arch == "x64" || arch == "aarch64");
    }

    #[test]
    fn test_list_installed_empty() {
        let result = list_installed();
        assert!(result.is_ok());
    }

    #[test]
    fn test_doctor_no_jdk() {
        // 在没有 JDK 的环境中，doctor 不应该 panic
        let result = doctor();
        let _ = result;
    }
}
