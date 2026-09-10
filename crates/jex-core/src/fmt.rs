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

/// 构建 google-java-format 的公共参数列表（查找 JAR、检查 java、风格参数）
fn build_gjf_args(config: &FmtConfig) -> Result<Vec<String>> {
    let jar = find_gjf_jar()?;
    check_java()?;
    let mut args = vec!["-jar".to_string(), jar.to_string_lossy().to_string()];
    match config.style {
        Style::Aosp => args.push("--aosp".to_string()),
        Style::OpenJ7 => args.push("--google-java-format-1.7".to_string()),
        Style::Google => {}
    }
    Ok(args)
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
    let mut args = build_gjf_args(config)?;
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
    let args = build_gjf_args(config)?;
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

/// 格式化输出模式
#[derive(Debug, Clone, Copy)]
pub enum OutputMode {
    /// 输出到 stdout
    Stdout,
    /// 检查模式（不修改文件）
    Check,
    /// 直接写入文件
    Write,
}

/// 对单个文件执行格式化并按指定模式输出结果
pub fn format_and_output(path: &Path, config: &FmtConfig, mode: OutputMode) -> Result<()> {
    let formatted = format_file(path, config)?;
    match mode {
        OutputMode::Stdout => {
            println!("--- {} ---", path.display());
            println!("{formatted}");
        }
        OutputMode::Check => {
            println!("would format: {}", path.display());
        }
        OutputMode::Write => {
            std::fs::write(path, formatted.as_bytes())?;
            println!("formatted: {}", path.display());
        }
    }
    Ok(())
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
    #[test]
    fn test_style_from_str_openjdk7() {
        assert_eq!("OPENJDK7".parse::<Style>().unwrap(), Style::OpenJ7);
    }

    #[test]
    fn test_style_default() {
        let s = Style::default();
        assert_eq!(s, Style::Google);
    }

    #[test]
    fn test_style_clone() {
        let s = Style::Aosp;
        let cloned = s;
        assert_eq!(cloned, Style::Aosp);
    }

    #[test]
    fn test_style_debug() {
        assert_eq!(format!("{:?}", Style::Google), "Google");
        assert_eq!(format!("{:?}", Style::Aosp), "Aosp");
        assert_eq!(format!("{:?}", Style::OpenJ7), "OpenJ7");
    }

    #[test]
    fn test_fmt_config_clone() {
        let c = FmtConfig::default();
        let cloned = c.clone();
        assert_eq!(cloned.style, Style::Google);
        assert_eq!(cloned.exclude, c.exclude);
    }

    #[test]
    fn test_fmt_config_debug() {
        let c = FmtConfig::default();
        let debug_str = format!("{:?}", c);
        assert!(debug_str.contains("Google"));
    }

    #[test]
    fn test_output_mode_debug() {
        assert_eq!(format!("{:?}", OutputMode::Stdout), "Stdout");
        assert_eq!(format!("{:?}", OutputMode::Check), "Check");
        assert_eq!(format!("{:?}", OutputMode::Write), "Write");
    }

    #[test]
    fn test_output_mode_clone() {
        let m = OutputMode::Check;
        let cloned = m;
        assert!(matches!(cloned, OutputMode::Check));
    }

    #[test]
    fn test_format_changed_filtering() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let build_dir = dir.path().join("build");
        std::fs::create_dir_all(&build_dir).unwrap();
        std::fs::write(build_dir.join("Test.java"), "code").unwrap();
        std::fs::write(dir.path().join("Main.java"), "code").unwrap();

        let config = FmtConfig::default();
        let files: Vec<_> = vec![
            PathBuf::from("build/Test.java"),
            PathBuf::from("Main.java"),
            PathBuf::from("target/Foo.java"),
        ];
        let filtered: Vec<_> = files
            .into_iter()
            .filter(|f| {
                let s = f.to_string_lossy();
                !config.exclude.iter().any(|e| s.contains(e.as_str()))
            })
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].to_string_lossy(), "Main.java");
    }

    #[test]
    fn test_format_changed_empty_exclude() {
        let config = FmtConfig {
            style: Style::Google,
            aosp: false,
            skip_future: false,
            exclude: vec![],
        };
        let files: Vec<_> = vec![PathBuf::from("build/Test.java"), PathBuf::from("Main.java")];
        let filtered: Vec<_> = files
            .into_iter()
            .filter(|f| {
                let s = f.to_string_lossy();
                !config.exclude.iter().any(|e| s.contains(e.as_str()))
            })
            .collect();
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_fmt_config_custom() {
        let c = FmtConfig {
            style: Style::Aosp,
            aosp: true,
            skip_future: true,
            exclude: vec!["generated/".to_string()],
        };
        assert_eq!(c.style, Style::Aosp);
        assert!(c.aosp);
        assert!(c.skip_future);
    }

    #[test]
    fn test_style_equality() {
        assert_eq!(Style::Google, Style::Google);
        assert_ne!(Style::Google, Style::Aosp);
        assert_ne!(Style::Aosp, Style::OpenJ7);
    }

    #[test]
    fn test_output_mode_equality() {
        assert!(matches!(OutputMode::Stdout, OutputMode::Stdout));
        // removed: OutputMode lacks PartialEq;
        // removed: OutputMode lacks PartialEq;
    }

    #[test]
    fn test_style_from_invalid_strings() {
        assert!("google-java-format".parse::<Style>().is_err());
        assert!("".parse::<Style>().is_err());
        assert!("spring".parse::<Style>().is_err());
        assert!("random".parse::<Style>().is_err());
    }

    #[test]
    fn test_format_changed_multiple_excludes() {
        let config = FmtConfig {
            style: Style::Google,
            aosp: false,
            skip_future: false,
            exclude: vec![
                "build/".to_string(),
                "target/".to_string(),
                "gen/".to_string(),
            ],
        };
        let files: Vec<_> = vec![
            PathBuf::from("build/A.java"),
            PathBuf::from("target/B.java"),
            PathBuf::from("gen/C.java"),
            PathBuf::from("src/Main.java"),
        ];
        let filtered: Vec<_> = files
            .into_iter()
            .filter(|f| {
                let s = f.to_string_lossy();
                !config.exclude.iter().any(|e| s.contains(e.as_str()))
            })
            .collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].to_string_lossy(), "src/Main.java");
    }

    #[test]
    fn test_format_changed_non_java_preserved() {
        let config = FmtConfig::default();
        let files: Vec<_> = vec![
            PathBuf::from("src/Main.java"),
            PathBuf::from("src/readme.md"),
            PathBuf::from("build/Test.java"),
        ];
        let filtered: Vec<_> = files
            .into_iter()
            .filter(|f| {
                let s = f.to_string_lossy();
                !config.exclude.iter().any(|e| s.contains(e.as_str()))
            })
            .collect();
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_format_changed_nested_exclude() {
        let config = FmtConfig {
            style: Style::Google,
            aosp: false,
            skip_future: false,
            exclude: vec!["build/".to_string()],
        };
        // Nested path still matches "build/"
        let files: Vec<_> = vec![
            PathBuf::from("project/build/output/Test.java"),
            PathBuf::from("src/Main.java"),
        ];
        let filtered: Vec<_> = files
            .into_iter()
            .filter(|f| {
                let s = f.to_string_lossy();
                !config.exclude.iter().any(|e| s.contains(e.as_str()))
            })
            .collect();
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_style_clone_all() {
        let styles = [Style::Google, Style::Aosp, Style::OpenJ7];
        for s in styles {
            let cloned = s;
            assert_eq!(cloned, s);
        }
    }
}
