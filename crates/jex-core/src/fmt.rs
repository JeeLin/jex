use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};

/// 格式化风格
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Style {
    #[default]
    Google,
    Aosp,
    OpenJ7,
}

impl std::fmt::Display for Style {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Google => write!(f, "Google"),
            Self::Aosp => write!(f, "AOSP"),
            Self::OpenJ7 => write!(f, "OpenJ7"),
        }
    }
}

impl std::str::FromStr for Style {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_uppercase().as_str() {
            "GOOGLE" => Ok(Self::Google),
            "AOSP" => Ok(Self::Aosp),
            "OPENJ7" | "OPENJDK7" => Ok(Self::OpenJ7),
            _ => Err(Error::new(format!("未知格式化风格: {s}"))),
        }
    }
}

/// 格式化配置
#[derive(Debug, Clone)]
pub struct FmtConfig {
    pub style: Style,
    pub aosp: bool,
    pub skip_future: bool,
    pub exclude: Vec<String>,
}

impl Default for FmtConfig {
    fn default() -> Self {
        Self {
            style: Style::Google,
            aosp: false,
            skip_future: false,
            exclude: vec!["build/".to_string(), "target/".into()],
        }
    }
}

/// 查找 google-java-format JAR
fn find_gjf_jar() -> Result<PathBuf> {
    let jar_name = "google-java-format-1.19.2-all-deps.jar";
    let home = dirs::home_dir().ok_or_else(|| Error::new("无法获取 home 目录".to_string()))?;
    let tools_dir = home.join(".jex").join("tools");
    let jar_path = tools_dir.join(jar_name);
    if jar_path.exists() {
        return Ok(jar_path);
    }
    Err(Error::new(format!(
        "google-java-format JAR 未找到，请先安装到 {}",
        tools_dir.display()
    )))
}

/// 检查 java 是否可用
fn check_java() -> Result<()> {
    let output = Command::new("java")
        .arg("-version")
        .output()
        .map_err(|e| Error::new(format!("java 不可用: {e}")))?;
    if !output.status.success() {
        return Err(Error::new("java 命令执行失败".to_string()));
    }
    Ok(())
}

/// 格式化单个文件，返回格式化后的代码
pub fn format_file(path: &Path, config: &FmtConfig) -> Result<String> {
    let jar = find_gjf_jar()?;
    check_java()?;
    let mut args = vec![
        "-jar".to_string(),
        jar.to_string_lossy().to_string(),
    ];
    match config.style {
        Style::Aosp => args.push("--aosp".to_string()),
        Style::OpenJ7 => args.push("--google-java-format-1.7".to_string()),
        Style::Google => {}
    }
    if config.skip_future {
        args.push("--skip-sorting-imports".to_string());
    }
    args.push(path.to_string_lossy().to_string());
    let output = Command::new("java")
        .args(&args)
        .output()
        .map_err(|e| Error::new(format!("执行 google-java-format 失败: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::new(format!("格式化失败: {stderr}")));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// 格式化代码字符串，返回格式化后的代码
pub fn format_code(code: &str, config: &FmtConfig) -> Result<String> {
    let jar = find_gjf_jar()?;
    check_java()?;
    let mut args = vec![
        "-jar".to_string(),
        jar.to_string_lossy().to_string(),
    ];
    match config.style {
        Style::Aosp => args.push("--aosp".to_string()),
        Style::OpenJ7 => args.push("--google-java-format-1.7".to_string()),
        Style::Google => {}
    }
    let mut child = Command::new("java")
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| Error::new(format!("执行 google-java-format 失败: {e}")))?;
    use std::io::Write;
    if let Some(ref mut stdin) = child.stdin {
        stdin
            .write_all(code.as_bytes())
            .map_err(|e| Error::new(format!("写入 stdin 失败: {e}")))?;
    }
    let result = child
        .wait_with_output()
        .map_err(|e| Error::new(format!("等待 google-java-format 失败: {e}")))?;
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(Error::new(format!("格式化失败: {stderr}")));
    }
    Ok(String::from_utf8_lossy(&result.stdout).to_string())
}

/// 获取 git diff 中变更的 Java 文件列表
pub fn get_changed_files() -> Result<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["diff", "--name-only", "HEAD~1"])
        .output()
        .map_err(|e| Error::new(format!("执行 git diff 失败: {e}")))?;
    if !output.status.success() {
        return Err(Error::new("git diff 执行失败".to_string()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter(|l| l.ends_with(".java"))
        .map(PathBuf::from)
        .collect())
}

/// 格式化变更的文件（过滤排除列表），返回待格式化文件路径
pub fn format_changed(config: &FmtConfig) -> Result<Vec<PathBuf>> {
    let files = get_changed_files()?;
    Ok(files
        .into_iter()
        .filter(|f| {
            let s = f.to_string_lossy();
            !config.exclude.iter().any(|e| s.contains(e.as_str()))
        })
        .filter(|f| f.exists())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_from_str() {
        assert_eq!("google".parse::<Style>().unwrap(), Style::Google);
        assert_eq!("AOSP".parse::<Style>().unwrap(), Style::Aosp);
        assert_eq!("OpenJ7".parse::<Style>().unwrap(), Style::OpenJ7);
        assert!("invalid".parse::<Style>().is_err());
    }

    #[test]
    fn test_default_config() {
        let c = FmtConfig::default();
        assert_eq!(c.style, Style::Google);
        assert!(!c.aosp);
        assert!(!c.skip_future);
        assert!(c.exclude.contains(&"build/".to_string()));
    }

    #[test]
    fn test_style_display() {
        assert_eq!(format!("{}", Style::Google), "Google");
        assert_eq!(format!("{}", Style::Aosp), "AOSP");
        assert_eq!(format!("{}", Style::OpenJ7), "OpenJ7");
    }
}
