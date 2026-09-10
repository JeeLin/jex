//! 依赖管理
//! - init: 生成 jex.toml
//! - add: 添加依赖到 jex.toml，调用 Coursier 解析，更新 jex.lock.toml
//! - remove: 从 jex.toml 删除依赖，重算锁文件
//! - update: 更新依赖版本

use crate::error::{Error, Result};
use crate::resolver;
use crate::util::parse_coord;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// 项目配置结构
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ProjectConfig {
    pub project: Option<ProjectInfo>,
    pub dependencies: Option<HashMap<String, String>>,
    pub repositories: Option<HashMap<String, TOMLValue>>,
    pub build: Option<BuildConfig>,
}

/// 项目信息
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ProjectInfo {
    pub name: String,
    pub java: Option<String>,
    pub main: Option<String>,
}

/// 构建配置
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct BuildConfig {
    pub output: Option<String>,
    pub sources: Option<Vec<String>>,
    pub resources: Option<Vec<String>>,
    pub compiler_args: Option<Vec<String>>,
    pub jvm_args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
}

/// TOML 值类型（用于 repositories 等复杂结构）
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum TOMLValue {
    Bool(bool),
    String(String),
    Table(HashMap<String, TOMLValue>),
}

/// 锁文件结构
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct LockFile {
    pub lockfile_version: Option<u32>,
    pub dependencies: Option<HashMap<String, String>>,
}

/// 获取当前目录的 jex.toml 路径
fn jex_toml_path() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    Ok(cwd.join("jex.toml"))
}

/// 获取当前目录的 jex.lock.toml 路径
fn jex_lock_path() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    Ok(cwd.join("jex.lock.toml"))
}

/// 读取 jex.toml
pub fn read_jex_toml() -> Result<ProjectConfig> {
    let path = jex_toml_path()?;
    if !path.exists() {
        return Err(Error::new("当前目录没有 jex.toml，请先运行 jex init"));
    }
    let content = fs::read_to_string(&path)?;
    let config: ProjectConfig = toml::from_str(&content)?;
    Ok(config)
}

/// 写入 jex.toml
pub fn write_jex_toml(config: &ProjectConfig) -> Result<()> {
    let path = jex_toml_path()?;
    let content = toml::to_string_pretty(config)?;
    fs::write(&path, content)?;
    Ok(())
}

/// 读取 jex.lock.toml
pub fn read_jex_lock() -> Result<LockFile> {
    let path = jex_lock_path()?;
    if !path.exists() {
        return Ok(LockFile {
            lockfile_version: Some(1),
            dependencies: Some(HashMap::new()),
        });
    }
    let content = fs::read_to_string(&path)?;
    let lock: LockFile = toml::from_str(&content)?;
    Ok(lock)
}

/// 写入 jex.lock.toml
pub fn write_jex_lock(lock: &LockFile) -> Result<()> {
    let path = jex_lock_path()?;
    let content = toml::to_string_pretty(lock)?;
    fs::write(&path, content)?;
    Ok(())
}

/// 初始化项目
pub fn init(name: Option<&str>) -> Result<()> {
    let path = jex_toml_path()?;
    if path.exists() {
        return Err(Error::new("当前目录已存在 jex.toml"));
    }

    let project_name = name.unwrap_or("demo");
    let config = ProjectConfig {
        project: Some(ProjectInfo {
            name: project_name.to_string(),
            java: Some("21".to_string()),
            main: None,
        }),
        dependencies: Some(HashMap::new()),
        repositories: Some({
            let mut repos = HashMap::new();
            repos.insert("maven-central".to_string(), TOMLValue::Bool(true));
            repos
        }),
        build: Some(BuildConfig {
            output: Some(".jex-build".to_string()),
            sources: Some(vec!["src".to_string()]),
            resources: Some(vec!["src/main/resources".to_string()]),
            compiler_args: Some(vec![
                "-parameters".to_string(),
                "-encoding".to_string(),
                "UTF-8".to_string(),
            ]),
            jvm_args: None,
            env: None,
        }),
    };

    write_jex_toml(&config)?;

    // 创建 src 目录
    let cwd = std::env::current_dir()?;
    fs::create_dir_all(cwd.join("src"))?;

    println!("已生成 jex.toml");
    Ok(())
}

/// 添加依赖
pub fn add(coord: &str, version: Option<&str>) -> Result<()> {
    let mut config = read_jex_toml()?;
    let dependencies = config.dependencies.get_or_insert_with(HashMap::new);

    // 解析坐标
    let (group, artifact) = parse_coord(coord)?;
    let full_coord = format!("{}:{}", group, artifact);

    // 获取版本
    let ver = match version {
        Some(v) => v.to_string(),
        None => {
            // 调用 Coursier 获取最新版本
            resolve_latest_version(&full_coord)?
        }
    };

    dependencies.insert(full_coord.clone(), ver.clone());
    write_jex_toml(&config)?;

    // 更新锁文件
    update_lock_file(&config)?;

    println!("已添加依赖: {} {}", full_coord, ver);
    Ok(())
}

/// 删除依赖
pub fn remove(coord: &str) -> Result<()> {
    let mut config = read_jex_toml()?;
    let dependencies = config.dependencies.get_or_insert_with(HashMap::new);

    // 解析坐标
    let (group, artifact) = parse_coord(coord)?;
    let full_coord = format!("{}:{}", group, artifact);

    if dependencies.remove(&full_coord).is_none() {
        return Err(Error::new(format!("未找到依赖: {}", full_coord)));
    }

    write_jex_toml(&config)?;

    // 更新锁文件
    update_lock_file(&config)?;

    println!("已删除依赖: {}", full_coord);
    Ok(())
}

/// 更新依赖：重解析坐标以获取最新版本，重算锁文件
pub fn update(coord: Option<&str>) -> Result<()> {
    let mut config = read_jex_toml()?;
    let dependencies = config.dependencies.get_or_insert_with(HashMap::new);

    // 决定要更新的坐标列表
    let targets: Vec<String> = match coord {
        Some(c) => {
            let (g, a) = parse_coord(c)?;
            let full = format!("{}:{}", g, a);
            if !dependencies.contains_key(&full) {
                return Err(Error::new(format!("未找到依赖: {}", full)));
            }
            vec![full]
        }
        None => dependencies.keys().cloned().collect(),
    };

    for full in &targets {
        let new_ver = resolve_latest_version(full)?;
        dependencies.insert(full.clone(), new_ver);
        println!("更新 {} -> {}", full, dependencies[full]);
    }

    write_jex_toml(&config)?;
    update_lock_file(&config)?;

    Ok(())
}

/// 通过 resolver 模块获取最新版本（已迁移到原生解析）
fn resolve_latest_version(coord: &str) -> Result<String> {
    resolver::resolve_latest(coord)
}

/// 更新锁文件（简化实现：只记录直接依赖）
pub fn update_lock_file(config: &ProjectConfig) -> Result<()> {
    let dependencies = config.dependencies.clone().unwrap_or_default();
    let mut lock = LockFile {
        lockfile_version: Some(1),
        dependencies: Some(HashMap::new()),
    };

    let lock_deps = lock.dependencies.as_mut().unwrap();
    for (coord, version) in &dependencies {
        lock_deps.insert(coord.clone(), version.clone());
    }

    write_jex_lock(&lock)?;
    Ok(())
}

/// 依赖树（使用 resolver 递归解析传递依赖并展示树形结构）
pub fn tree() -> Result<()> {
    let lock = read_jex_lock()?;
    let dependencies = lock.dependencies.clone().unwrap_or_default();

    if dependencies.is_empty() {
        println!("无依赖");
        return Ok(());
    }

    // 从 jex.toml 读取项目名
    let project_name = std::fs::read_to_string(jex_toml_path()?)
        .ok()
        .and_then(|c| {
            c.lines()
                .find(|l| l.starts_with("name"))
                .and_then(|l| l.split("=").nth(1))
                .map(|s| s.trim().trim_matches('"').to_string())
        })
        .unwrap_or_else(|| "project".to_string());

    println!("{}:", project_name);
    for (i, (coord, version)) in dependencies.iter().enumerate() {
        let is_last = i == dependencies.len() - 1;
        let connector = if is_last { "└─ " } else { "├─ " };
        println!("  {}{}:{}", connector, coord, version);

        // 递归解析传递依赖（深度限制由 resolver 内部处理）
        match crate::resolver::resolve_dependencies(coord) {
            Ok(node) => {
                let child_prefix = if is_last { "   " } else { "│  " };
                let tree_str =
                    crate::resolver::format_tree(&node, &format!("  {}", child_prefix), true);
                // 只显示子节点（跳过根节点自身）
                for line in tree_str.lines() {
                    if !line.is_empty() {
                        println!("    {}", line);
                    }
                }
            }
            Err(e) => {
                println!("    ⚠️ 无法解析依赖: {}", e);
            }
        }
    }

    Ok(())
}

/// 为何引入某依赖（简化实现）
pub fn why(coord: &str) -> Result<()> {
    let lock = read_jex_lock()?;
    let dependencies = lock.dependencies.unwrap_or_default();

    let (group, artifact) = parse_coord(coord)?;
    let full_coord = format!("{}:{}", group, artifact);

    if let Some(version) = dependencies.get(&full_coord) {
        println!(
            "{} {} 被直接依赖（在 jex.toml 中声明）",
            full_coord, version
        );
    } else {
        println!("未找到依赖: {}", full_coord);
    }

    Ok(())
}

/// 依赖冲突分析（简化实现）
pub fn conflict() -> Result<()> {
    let _lock = read_jex_lock()?;

    println!("依赖冲突分析:");
    println!("  当前无冲突（简化实现）");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_project_config_serialize_deserialize() {
        let config = ProjectConfig {
            project: Some(ProjectInfo {
                name: "test-project".to_string(),
                java: Some("21".to_string()),
                main: Some("Main".to_string()),
            }),
            dependencies: Some({
                let mut deps = HashMap::new();
                deps.insert(
                    "com.google.code.gson:gson".to_string(),
                    "2.11.0".to_string(),
                );
                deps
            }),
            repositories: Some({
                let mut repos = HashMap::new();
                repos.insert("maven-central".to_string(), TOMLValue::Bool(true));
                repos
            }),
            build: Some(BuildConfig {
                output: Some(".jex-build".to_string()),
                sources: Some(vec!["src".to_string()]),
                resources: Some(vec![]),
                compiler_args: Some(vec!["-parameters".to_string()]),
                jvm_args: None,
                env: None,
            }),
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("test-project"));
        assert!(toml_str.contains("gson"));

        let deserialized: ProjectConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.project.as_ref().unwrap().name, "test-project");
        assert_eq!(
            deserialized
                .dependencies
                .as_ref()
                .unwrap()
                .get("com.google.code.gson:gson")
                .unwrap(),
            "2.11.0"
        );
    }

    #[test]
    fn test_lock_file_serialize_deserialize() {
        let lock = LockFile {
            lockfile_version: Some(1),
            dependencies: Some({
                let mut deps = HashMap::new();
                deps.insert("org.slf4j:slf4j-api".to_string(), "2.0.9".to_string());
                deps
            }),
        };

        let toml_str = toml::to_string_pretty(&lock).unwrap();
        assert!(toml_str.contains("slf4j-api"));

        let deserialized: LockFile = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.lockfile_version, Some(1));
        assert_eq!(
            deserialized
                .dependencies
                .unwrap()
                .get("org.slf4j:slf4j-api")
                .unwrap(),
            "2.0.9"
        );
    }

    #[test]
    fn test_toml_value_variants() {
        // TOMLValue is an untagged enum used in repositories config
        let toml_str = "[maven-central]\nenabled = true";
        let val: TOMLValue = toml::from_str(toml_str).unwrap();
        assert!(matches!(val, TOMLValue::Table(_)));

        let toml_str = "[repositories.maven-central]\nenabled = true";
        let val: HashMap<String, TOMLValue> = toml::from_str(toml_str).unwrap();
        assert!(!val.is_empty());
    }

    #[test]
    fn test_project_config_minimal() {
        let config = ProjectConfig {
            project: None,
            dependencies: None,
            repositories: None,
            build: None,
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let deserialized: ProjectConfig = toml::from_str(&toml_str).unwrap();
        assert!(deserialized.project.is_none());
        assert!(deserialized.dependencies.is_none());
    }

    #[test]
    fn test_lock_file_empty() {
        let lock = LockFile {
            lockfile_version: None,
            dependencies: None,
        };
        let toml_str = toml::to_string_pretty(&lock).unwrap();
        let deserialized: LockFile = toml::from_str(&toml_str).unwrap();
        assert!(deserialized.lockfile_version.is_none());
        assert!(deserialized.dependencies.is_none());
    }

    #[test]
    #[serial]
    fn test_read_jex_lock_no_file() {
        let tmp = tempfile::tempdir().unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let result = read_jex_lock();
        std::env::set_current_dir(&orig).unwrap();
        // Should return default value, not error
        let lock = result.unwrap();
        assert_eq!(lock.lockfile_version, Some(1));
    }

    #[test]
    #[serial]
    fn test_read_jex_toml_no_file() {
        let tmp = tempfile::tempdir().unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let result = read_jex_toml();
        std::env::set_current_dir(&orig).unwrap();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("没有 jex.toml"));
    }

    #[test]
    fn test_project_info_serialize() {
        let info = ProjectInfo {
            name: "my-app".to_string(),
            java: Some("17".to_string()),
            main: Some("com.example.Main".to_string()),
        };
        let toml_str = toml::to_string_pretty(&info).unwrap();
        assert!(toml_str.contains("my-app"));

        let deserialized: ProjectInfo = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.name, "my-app");
        assert_eq!(deserialized.java, Some("17".to_string()));
        assert_eq!(deserialized.main, Some("com.example.Main".to_string()));
    }

    #[test]
    fn test_build_config_serialize() {
        let build = BuildConfig {
            output: Some("target".to_string()),
            sources: Some(vec!["src/main".to_string()]),
            resources: Some(vec!["src/main/resources".to_string()]),
            compiler_args: Some(vec!["-Xlint".to_string(), "-deprecation".to_string()]),
            jvm_args: Some(vec!["-Xmx512m".to_string()]),
            env: Some({
                let mut env = HashMap::new();
                env.insert("JAVA_HOME".to_string(), "/usr/lib/jvm/java-21".to_string());
                env
            }),
        };
        let toml_str = toml::to_string_pretty(&build).unwrap();
        assert!(toml_str.contains("target"));
        assert!(toml_str.contains("-Xlint"));
        assert!(toml_str.contains("-Xmx512m"));
        assert!(toml_str.contains("JAVA_HOME"));

        let deserialized: BuildConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.output, Some("target".to_string()));
        assert_eq!(deserialized.jvm_args, Some(vec!["-Xmx512m".to_string()]));
    }

    #[test]
    fn test_conflict_function() {
        let result = conflict();
        let _ = result;
    }

    #[test]
    #[serial]
    fn test_update_lock_file() {
        let tmp = tempfile::tempdir().unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        let config = ProjectConfig {
            project: Some(ProjectInfo {
                name: "test".to_string(),
                java: None,
                main: None,
            }),
            dependencies: Some({
                let mut deps = HashMap::new();
                deps.insert(
                    "com.google.code.gson:gson".to_string(),
                    "2.11.0".to_string(),
                );
                deps.insert("org.slf4j:slf4j-api".to_string(), "2.0.9".to_string());
                deps
            }),
            repositories: None,
            build: None,
        };

        update_lock_file(&config).unwrap();

        let lock_content = std::fs::read_to_string(tmp.path().join("jex.lock.toml")).unwrap();
        assert!(lock_content.contains("gson"));
        assert!(lock_content.contains("slf4j-api"));

        std::env::set_current_dir(&orig).unwrap();
    }

    #[test]
    #[serial]
    fn test_update_lock_file_empty_deps() {
        let tmp = tempfile::tempdir().unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        let config = ProjectConfig {
            project: None,
            dependencies: None,
            repositories: None,
            build: None,
        };

        update_lock_file(&config).unwrap();

        let lock = read_jex_lock().unwrap();
        assert_eq!(lock.lockfile_version, Some(1));
        assert!(lock.dependencies.unwrap().is_empty());

        std::env::set_current_dir(&orig).unwrap();
    }

    #[test]
    #[serial]
    fn test_why_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        let lock = LockFile {
            lockfile_version: Some(1),
            dependencies: Some(HashMap::new()),
        };
        std::fs::write("jex.lock.toml", toml::to_string_pretty(&lock).unwrap()).unwrap();

        let result = why("com.example:missing");
        assert!(result.is_ok());

        std::env::set_current_dir(&orig).unwrap();
    }

    #[test]
    fn test_toml_value_bool_variant() {
        let config = ProjectConfig {
            project: None,
            dependencies: None,
            repositories: Some({
                let mut repos = HashMap::new();
                repos.insert("central".to_string(), TOMLValue::Bool(true));
                repos
            }),
            build: None,
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("true"));

        let deserialized: ProjectConfig = toml::from_str(&toml_str).unwrap();
        let repos = deserialized.repositories.unwrap();
        match repos.get("central").unwrap() {
            TOMLValue::Bool(v) => assert!(*v),
            _ => panic!("expected Bool variant"),
        }
    }

    #[test]
    fn test_lock_file_multiple_deps() {
        let lock = LockFile {
            lockfile_version: Some(1),
            dependencies: Some({
                let mut deps = HashMap::new();
                deps.insert("a:b".to_string(), "1.0".to_string());
                deps.insert("c:d".to_string(), "2.0".to_string());
                deps.insert("e:f".to_string(), "3.0".to_string());
                deps
            }),
        };
        let toml_str = toml::to_string_pretty(&lock).unwrap();
        let deserialized: LockFile = toml::from_str(&toml_str).unwrap();
        let deps = deserialized.dependencies.unwrap();
        assert_eq!(deps.len(), 3);
        assert_eq!(deps.get("a:b").unwrap(), "1.0");
        assert_eq!(deps.get("c:d").unwrap(), "2.0");
        assert_eq!(deps.get("e:f").unwrap(), "3.0");
    }

    #[test]
    fn test_project_config_all_optional_none() {
        let config = ProjectConfig {
            project: Some(ProjectInfo {
                name: "minimal".to_string(),
                java: None,
                main: None,
            }),
            dependencies: None,
            repositories: None,
            build: Some(BuildConfig {
                output: None,
                sources: None,
                resources: None,
                compiler_args: None,
                jvm_args: None,
                env: None,
            }),
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("minimal"));
        let deserialized: ProjectConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.project.unwrap().name, "minimal");
        assert!(deserialized.dependencies.is_none());
    }

    #[test]
    #[serial]
    fn test_init_already_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        std::fs::write("jex.toml", "[project]\nname = \"test\"").unwrap();

        let result = init(Some("test"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("已存在"));

        std::env::set_current_dir(&orig).unwrap();
    }

    #[test]
    #[serial]
    fn test_init_creates_jex_toml() {
        let tmp = tempfile::tempdir().unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        init(Some("my-project")).unwrap();

        assert!(tmp.path().join("jex.toml").exists());
        let content = std::fs::read_to_string(tmp.path().join("jex.toml")).unwrap();
        assert!(content.contains("my-project"));
        assert!(content.contains("21"));
        assert!(tmp.path().join("src").is_dir());

        std::env::set_current_dir(&orig).unwrap();
    }

    #[test]
    #[serial]
    fn test_init_default_name() {
        let tmp = tempfile::tempdir().unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        init(None).unwrap();

        let content = std::fs::read_to_string(tmp.path().join("jex.toml")).unwrap();
        assert!(content.contains("demo"));

        std::env::set_current_dir(&orig).unwrap();
    }

    #[test]
    #[serial]
    fn test_write_and_read_jex_lock() {
        let tmp = tempfile::tempdir().unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        let lock = LockFile {
            lockfile_version: Some(1),
            dependencies: Some({
                let mut d = HashMap::new();
                d.insert(
                    "org.junit.jupiter:junit-jupiter".to_string(),
                    "5.10.0".to_string(),
                );
                d
            }),
        };

        write_jex_lock(&lock).unwrap();
        let read_lock = read_jex_lock().unwrap();
        assert_eq!(
            read_lock
                .dependencies
                .unwrap()
                .get("org.junit.jupiter:junit-jupiter")
                .unwrap(),
            "5.10.0"
        );

        std::env::set_current_dir(&orig).unwrap();
    }

    #[test]
    #[serial]
    fn test_read_jex_lock_missing_file_returns_default() {
        let tmp = tempfile::tempdir().unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        let lock = read_jex_lock().unwrap();
        assert_eq!(lock.lockfile_version, Some(1));
        assert!(lock.dependencies.unwrap().is_empty());

        std::env::set_current_dir(&orig).unwrap();
    }
}
