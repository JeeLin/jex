//! async-profiler 捆绑与火焰图生成
//! - 自动下载 async-profiler 原生二进制（Linux/macOS x86_64/aarch64）
//! - 采样 + collapsed 格式转 SVG 火焰图
//!
//! 子任务：将 async-profiler 捆绑到 jex，实现 `jex java flame` 命令。

use crate::config::jex_home;
use crate::error::{Error, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// async-profiler 当前捆绑版本
const PROFILER_VERSION: &str = "3.0";

/// async-profiler GitHub releases 基础 URL
const PROFILER_RELEASE_BASE: &str =
    "https://github.com/async-profiler/async-profiler/releases/download";

/// 检测当前操作系统
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Os {
    Linux,
    Macos,
    Windows,
}

/// 检测当前 CPU 架构
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    Aarch64,
    Unknown(String),
}

/// 平台信息
#[derive(Debug, Clone)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
}

impl Platform {
    /// 检测当前平台
    pub fn detect() -> Self {
        let os = match std::env::consts::OS {
            "linux" => Os::Linux,
            "macos" => Os::Macos,
            "windows" => Os::Windows,
            other => {
                // 不应发生，但作为兜底
                eprintln!("警告: 未知操作系统 {other}，将尝试 Linux 路径");
                Os::Linux
            }
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => Arch::X86_64,
            "aarch64" => Arch::Aarch64,
            other => Arch::Unknown(other.to_string()),
        };
        Self { os, arch }
    }

    /// 返回 async-profiler 下载文件名（不含 .zip 后缀）
    ///
    /// 格式: `async-profiler-{version}-{os}-{arch}`
    pub fn download_name(&self, version: &str) -> String {
        let os_str = match self.os {
            Os::Linux => "linux",
            Os::Macos => "macos",
            Os::Windows => "windows",
        };
        let arch_str = match &self.arch {
            Arch::X86_64 => "x64",
            Arch::Aarch64 => "arm64",
            Arch::Unknown(a) => a.as_str(),
        };
        format!("async-profiler-{version}-{os_str}-{arch_str}")
    }

    /// 返回下载 URL
    pub fn download_url(&self, version: &str) -> String {
        let name = self.download_name(version);
        format!("{PROFILER_RELEASE_BASE}/v{version}/{name}.zip")
    }

    /// 返回 profiler 二进制在解压目录中的相对路径
    ///
    /// Linux: `bin/asprof`
    /// macOS: `bin/dtrace`
    pub fn binary_relative_path(&self) -> &str {
        match self.os {
            Os::Linux => "bin/asprof",
            Os::Macos => "bin/dtrace",
            Os::Windows => "bin/asprof",
        }
    }
}

/// profiler 安装目录: ~/.jex/profiler/{version}/
pub fn profiler_home() -> Result<PathBuf> {
    let home = jex_home()?;
    let dir = home.join("profiler").join(PROFILER_VERSION);
    Ok(dir)
}

/// profiler 二进制完整路径
pub fn profiler_binary() -> Result<PathBuf> {
    let platform = Platform::detect();
    let home = profiler_home()?;
    Ok(home.join(platform.binary_relative_path()))
}

/// 检查 async-profiler 是否已下载
pub fn is_profiler_installed() -> bool {
    match profiler_binary() {
        Ok(path) => path.exists(),
        Err(_) => false,
    }
}

/// 确保 async-profiler 已下载，返回二进制路径
pub fn ensure_profiler() -> Result<PathBuf> {
    if is_profiler_installed() {
        return profiler_binary();
    }
    download_profiler()
}

/// 下载 async-profiler
fn download_profiler() -> Result<PathBuf> {
    let platform = Platform::detect();
    let url = platform.download_url(PROFILER_VERSION);
    let install_dir = profiler_home()?;

    println!("📦 正在下载 async-profiler v{PROFILER_VERSION}...");
    println!("   URL: {url}");

    // 创建临时目录
    let tmp_dir = install_dir.with_file_name(format!("{PROFILER_VERSION}.tmp"));
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir).map_err(|e| Error::new(format!("清理临时目录失败: {e}")))?;
    }
    fs::create_dir_all(&tmp_dir).map_err(|e| Error::new(format!("创建临时目录失败: {e}")))?;

    // 下载 zip 文件
    let zip_path = tmp_dir.join("profiler.zip");
    download_file(&url, &zip_path)?;

    // 解压
    println!("📂 正在解压...");
    unzip(&zip_path, &tmp_dir)?;

    // 定位解压后的目录
    let archive_name = platform.download_name(PROFILER_VERSION);
    let extracted_dir = tmp_dir.join(&archive_name);
    let source = if extracted_dir.exists() {
        extracted_dir
    } else {
        find_profiler_dir(&tmp_dir).ok_or_else(|| {
            let _ = fs::remove_dir_all(&tmp_dir);
            Error::new(format!(
                "解压后找不到 profiler 二进制目录（期望: {}）",
                platform.binary_relative_path()
            ))
        })?
    };
    fs::rename(&source, &install_dir)
        .map_err(|e| Error::new(format!("移动 profiler 目录失败: {e}")))?;
    // 清理临时目录
    let _ = fs::remove_dir_all(&tmp_dir);

    // 验证安装
    let binary = profiler_binary()?;
    if !binary.exists() {
        return Err(Error::new(format!(
            "profiler 二进制不存在: {}",
            binary.display()
        )));
    }

    // Linux: 设置可执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&binary)
            .map_err(|e| Error::new(format!("读取文件权限失败: {e}")))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&binary, perms)
            .map_err(|e| Error::new(format!("设置可执行权限失败: {e}")))?;
    }

    println!(
        "✅ async-profiler v{PROFILER_VERSION} 已安装: {}",
        binary.display()
    );
    Ok(binary)
}

/// 下载文件（使用 reqwest blocking）
fn download_file(url: &str, dest: &Path) -> Result<()> {
    let response = reqwest::blocking::get(url).map_err(|e| Error::new(format!("下载失败: {e}")))?;

    if !response.status().is_success() {
        return Err(Error::new(format!("下载失败: HTTP {}", response.status())));
    }

    let bytes = response
        .bytes()
        .map_err(|e| Error::new(format!("读取响应失败: {e}")))?;

    fs::write(dest, &bytes).map_err(|e| Error::new(format!("写入文件失败: {e}")))?;

    Ok(())
}

/// 解压 zip 文件（使用系统 unzip 命令）
fn unzip(zip_path: &Path, dest_dir: &Path) -> Result<()> {
    let output = Command::new("unzip")
        .args(["-q", "-o"])
        .arg(zip_path)
        .arg("-d")
        .arg(dest_dir)
        .output()
        .map_err(|e| Error::new(format!("执行 unzip 失败: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::new(format!("解压失败: {stderr}")));
    }

    Ok(())
}

/// 在目录中查找包含 profiler 二进制的子目录
fn find_profiler_dir(base: &Path) -> Option<PathBuf> {
    let platform = Platform::detect();
    let binary_path = platform.binary_relative_path();

    if let Ok(entries) = fs::read_dir(base) {
        for entry in entries.flatten() {
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                let candidate = entry.path().join(binary_path);
                if candidate.exists() {
                    return Some(entry.path());
                }
            }
        }
    }
    None
}

/// 采样结果信息
pub struct FlameResult {
    /// collapsed 格式的中间文件路径
    pub collapsed_path: PathBuf,
    /// 生成的火焰图 SVG 路径
    pub svg_path: PathBuf,
    /// 采样时长（秒）
    pub duration_secs: u32,
}

/// 采样输出目录: /tmp/jex-flame-{pid}-{timestamp}/
fn flame_output_dir(pid: u32) -> PathBuf {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    PathBuf::from(format!("/tmp/jex-flame-{pid}-{timestamp}"))
}

/// 对指定 PID 启动 async-profiler 采样
///
/// duration_secs: 采样时长（秒），默认 10 秒
pub fn profile(pid: u32, duration_secs: u32) -> Result<FlameResult> {
    let output_dir = flame_output_dir(pid);
    fs::create_dir_all(&output_dir).map_err(|e| Error::new(format!("创建输出目录失败: {e}")))?;

    let collapsed_path = output_dir.join("collapsed.txt");
    let svg_path = output_dir.join("flamegraph.svg");

    println!("🔥 开始采样 PID {pid}，时长 {duration_secs} 秒...");
    let output = run_profiler(pid, duration_secs, &collapsed_path)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(classify_profiler_error(&stderr, pid));
    }

    println!("·········· 采样完成");
    if !collapsed_path.exists() {
        return Err(Error::new(format!(
            "collapsed 文件未生成: {}",
            collapsed_path.display()
        )));
    }

    convert_collapsed_to_svg(&collapsed_path, &svg_path)?;
    println!("📊 火焰图已生成: {}", svg_path.display());

    Ok(FlameResult {
        collapsed_path,
        svg_path,
        duration_secs,
    })
}

/// 构建平台特定的 profiler 命令
fn run_profiler(pid: u32, duration_secs: u32, collapsed: &Path) -> Result<std::process::Output> {
    let binary = ensure_profiler()?;
    let platform = Platform::detect();
    let mut cmd = Command::new(&binary);
    match platform.os {
        Os::Linux => {
            cmd.args([
                "-d",
                &duration_secs.to_string(),
                "-f",
                collapsed.to_str().unwrap_or(""),
                "-o",
                "collapsed",
                &pid.to_string(),
            ]);
        }
        Os::Macos => {
            cmd.args([
                "-p",
                &pid.to_string(),
                "-d",
                &duration_secs.to_string(),
                "-f",
                collapsed.to_str().unwrap_or(""),
            ]);
        }
        Os::Windows => {
            return Err(Error::new(
                "Windows 暂不支持 async-profiler 火焰图".to_string(),
            ));
        }
    }
    cmd.output()
        .map_err(|e| Error::new(format!("执行 profiler 失败: {e}")))
}

/// 分类 profiler 错误信息
fn classify_profiler_error(stderr: &str, pid: u32) -> Error {
    if stderr.contains("permission denied") || stderr.contains("Permission denied") {
        Error::new("权限不足: Linux 需要 root 或 kernel.perf_event_paranoid <= 2\n\u{2003}提示: sudo sysctl kernel.perf_event_paranoid=1".to_string())
    } else if stderr.contains("No such process") || stderr.contains("does not exist") {
        Error::new(format!("进程 {pid} 不存在"))
    } else {
        Error::new(format!("profiler 执行失败: {stderr}"))
    }
}

/// 将 collapsed 格式转为 SVG 火焰图
///
/// 优先使用 profiler 内置的 converter，fallback 到纯 Rust 实现
fn convert_collapsed_to_svg(collapsed: &Path, svg: &Path) -> Result<()> {
    // 方法1: 尝试使用 profiler 内置的 profiler.sh 转换
    let profiler_dir = profiler_home()?;
    let converter_script = profiler_dir.join("converter.sh");
    if converter_script.exists() {
        let output = Command::new("sh")
            .arg(&converter_script)
            .arg("-d")
            .arg(collapsed)
            .arg("-o")
            .arg(svg)
            .output();
        if let Ok(out) = output {
            if out.status.success() && svg.exists() {
                return Ok(());
            }
        }
    }

    // 方法2: 尝试使用 profiler 内置的 flamegraph.pl
    let flamegraph_pl = profiler_dir.join("flamegraph.pl");
    if flamegraph_pl.exists() {
        let output = Command::new("perl")
            .arg(&flamegraph_pl)
            .arg("--title")
            .arg("JVM Flame Graph")
            .arg("--subtitle")
            .arg("jex java flame")
            .arg("--width")
            .arg("1200")
            .arg("--countname")
            .arg("samples")
            .arg(collapsed)
            .output();
        if let Ok(out) = output {
            if out.status.success() {
                let svg_content = &out.stdout;
                fs::write(svg, svg_content)
                    .map_err(|e| Error::new(format!("写入 SVG 失败: {e}")))?;
                return Ok(());
            }
        }
    }

    // 方法3: 使用 perf script + 原生 flamegraph（如果可用）
    // 这里做一个简单的 fallback：读取 collapsed 并生成基础 SVG
    generate_basic_svg(collapsed, svg)
}

/// 基础 SVG 火焰图生成器（纯 Rust，无外部依赖）
///
/// 将 collapsed 格式转为简单的水平条形火焰图 SVG
fn generate_basic_svg(collapsed: &Path, svg: &Path) -> Result<()> {
    let content = fs::read_to_string(collapsed)
        .map_err(|e| Error::new(format!("读取 collapsed 文件失败: {e}")))?;

    // 解析 collapsed 格式: "stack;frame1;frame2;... count"
    let mut frames: Vec<(String, u64)> = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((stack, count_str)) = line.rsplit_once(' ') {
            if let Ok(count) = count_str.parse::<u64>() {
                frames.push((stack.to_string(), count));
            }
        }
    }

    if frames.is_empty() {
        return Err(Error::new("collapsed 文件为空或格式错误".to_string()));
    }

    // 计算总采样数
    let total: u64 = frames.iter().map(|(_, c)| *c).sum();

    // 构建 SVG
    let width = 1200;
    let row_height = 18;
    let header_height = 40;
    let footer_height = 20;
    let frame_width_min = 1.0;

    let total_height = header_height + (frames.len() as u16 * row_height) + footer_height;
    let mut svg_content = String::new();
    svg_content.push_str("<?xml version=\"1.0\" standalone=\"no\"?>\n");
    svg_content.push_str("<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd\">\n");
    svg_content.push_str(&format!(
        "<svg version=\"1.1\" width=\"{width}\" height=\"{total_height}\" xmlns=\"http://www.w3.org/2000/svg\">\n"
    ));
    svg_content.push_str("<style>\n");
    svg_content.push_str("  text { font-family: monospace; font-size: 12px; fill: #000; }\n");
    svg_content.push_str("  .title { font-size: 14px; font-weight: bold; }\n");
    svg_content.push_str("  .frame { cursor: pointer; }\n");
    svg_content.push_str("  .frame:hover rect { opacity: 0.8; }\n");
    svg_content.push_str("</style>\n");
    svg_content.push_str("<rect width=\"100%\" height=\"100%\" fill=\"#f8f8f8\"/>\n");
    svg_content.push_str(&format!(
        "<text x=\"10\" y=\"24\" class=\"title\">🔥 JVM Flame Graph — {total} samples</text>\n"
    ));

    // 为每个 frame 绘制矩形
    let mut y = header_height;
    for (stack, count) in frames.iter() {
        let frame_width = (*count as f64 / total as f64) * (width as f64 - 20.0);
        let frame_width = frame_width.max(frame_width_min);

        // 根据深度（栈帧数）选择颜色
        let depth = stack.matches(';').count();
        let hue = (depth * 30) % 360;
        let color = format!("hsl({hue}, 70%, 60%)");

        // 显示最后一个 frame 名称
        let label = stack.rsplit(';').next().unwrap_or(stack);
        let truncated_label = if label.len() > 60 {
            format!("{}...", &label[..57])
        } else {
            label.to_string()
        };

        let text_y = y + row_height - 4;
        let escaped_label = xml_escape(&truncated_label);
        svg_content.push_str(&format!(
            "<g class=\"frame\" title=\"{stack} ({count} samples)\">\n"
        ));
        svg_content.push_str(&format!(
            "  <rect x=\"10\" y=\"{y}\" width=\"{frame_width:.1}\" height=\"{row_height}\" fill=\"{color}\" stroke=\"#fff\" stroke-width=\"0.5\"/>\n"
        ));
        svg_content.push_str(&format!(
            "  <text x=\"12\" y=\"{text_y}\" font-size=\"11\">{escaped_label}</text>\n"
        ));
        svg_content.push_str("</g>\n");

        y += row_height;
    }

    svg_content.push_str("</svg>\n");
    fs::write(svg, &svg_content).map_err(|e| Error::new(format!("写入 SVG 文件失败: {e}")))?;

    Ok(())
}

/// XML 转义
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 尝试用系统默认浏览器打开文件
pub fn open_in_browser(path: &Path) -> Result<()> {
    let output = match std::env::consts::OS {
        "macos" => Command::new("open").arg(path).output(),
        "linux" => Command::new("xdg-open").arg(path).output(),
        _ => {
            println!("ℹ️  请手动打开: {}", path.display());
            return Ok(());
        }
    };

    match output {
        Ok(out) if out.status.success() => {
            println!("🌐 正在打开浏览器...");
            Ok(())
        }
        _ => {
            println!("ℹ️  请手动打开: {}", path.display());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detect() {
        let platform = Platform::detect();
        // 在 Linux CI 环境中
        assert!(
            platform.os == Os::Linux || platform.os == Os::Macos,
            "期望 Linux 或 macOS"
        );
    }

    #[test]
    fn test_platform_download_name_linux_x64() {
        let platform = Platform {
            os: Os::Linux,
            arch: Arch::X86_64,
        };
        assert_eq!(
            platform.download_name("3.0"),
            "async-profiler-3.0-linux-x64"
        );
    }

    #[test]
    fn test_platform_download_name_macos_arm64() {
        let platform = Platform {
            os: Os::Macos,
            arch: Arch::Aarch64,
        };
        assert_eq!(
            platform.download_name("3.0"),
            "async-profiler-3.0-macos-arm64"
        );
    }

    #[test]
    fn test_platform_download_url() {
        let platform = Platform {
            os: Os::Linux,
            arch: Arch::X86_64,
        };
        let url = platform.download_url("3.0");
        assert!(url.contains("async-profiler-3.0-linux-x64.zip"));
        assert!(url.starts_with("https://"));
    }

    #[test]
    fn test_platform_binary_relative_path() {
        assert_eq!(
            Platform {
                os: Os::Linux,
                arch: Arch::X86_64
            }
            .binary_relative_path(),
            "bin/asprof"
        );
        assert_eq!(
            Platform {
                os: Os::Macos,
                arch: Arch::Aarch64
            }
            .binary_relative_path(),
            "bin/dtrace"
        );
    }

    #[test]
    fn test_profiler_home() {
        let home = profiler_home().unwrap();
        assert!(home.to_string_lossy().contains(".jex"));
        assert!(home.to_string_lossy().contains("profiler"));
    }

    #[test]
    fn test_is_profiler_installed_not_found() {
        // 在测试环境中 profiler 未安装
        let result = is_profiler_installed();
        // 不做断言，只确保函数不 panic
        let _ = result;
    }

    #[test]
    fn test_flame_output_dir() {
        let dir = flame_output_dir(12345);
        let dir_str = dir.to_string_lossy();
        assert!(dir_str.contains("jex-flame-12345"));
        assert!(dir_str.starts_with("/tmp/"));
    }

    #[test]
    fn test_generate_basic_svg() {
        let tmp_dir = PathBuf::from("/tmp/jex-test-svg");
        let _ = fs::create_dir_all(&tmp_dir);

        let collapsed_path = tmp_dir.join("test.collapsed");
        let svg_path = tmp_dir.join("test.svg");

        // 写入测试数据
        fs::write(
            &collapsed_path,
            "main;thread1;func1 100\nmain;thread1;func2 50\nmain;thread2;func3 30\n",
        )
        .unwrap();

        let result = generate_basic_svg(&collapsed_path, &svg_path);
        assert!(result.is_ok());
        assert!(svg_path.exists());

        let svg_content = fs::read_to_string(&svg_path).unwrap();
        assert!(svg_content.contains("flamegraph.svg") || svg_content.contains("Flame Graph"));
        assert!(svg_content.contains("func1"));

        // 清理
        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_generate_basic_svg_empty() {
        let tmp_dir = PathBuf::from("/tmp/jex-test-svg-empty");
        let _ = fs::create_dir_all(&tmp_dir);

        let collapsed_path = tmp_dir.join("empty.collapsed");
        let svg_path = tmp_dir.join("empty.svg");

        fs::write(&collapsed_path, "").unwrap();

        let result = generate_basic_svg(&collapsed_path, &svg_path);
        assert!(result.is_err());

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("a & b"), "a &amp; b");
        assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
        assert_eq!(xml_escape("\"quoted\""), "&quot;quoted&quot;");
        assert_eq!(xml_escape("normal"), "normal");
    }

    #[test]
    fn test_open_in_browser() {
        // 只测试函数不 panic，不验证浏览器是否真的打开
        let fake_path = Path::new("/tmp/nonexistent-flame.svg");
        let _ = open_in_browser(fake_path);
    }
}
