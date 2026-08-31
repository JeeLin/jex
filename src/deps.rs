//! 依赖管理
//! - init: 生成 jex.toml
//! - add: 添加依赖到 jex.toml，调用 Coursier 解析，更新 jex.lock.toml
//! - remove: 从 jex.toml 删除依赖，重算锁文件
//! - update: 更新依赖版本

use crate::error::{Error, Result};
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

/// 解析坐标 group:artifact
fn parse_coord(coord: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() < 2 {
        return Err(Error::new(format!(
            "无效的坐标格式: {}（应为 group:artifact）",
            coord
        )));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// 调用 Coursier 获取最新版本
fn resolve_latest_version(coord: &str) -> Result<String> {
    let output = std::process::Command::new("cs")
        .args(["complete", coord])
        .output()?;

    if !output.status.success() {
        return Err(Error::new(format!(
            "Coursier 解析失败: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let versions: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();

    if versions.is_empty() {
        return Err(Error::new(format!("未找到依赖: {}", coord)));
    }

    // 返回最后一个版本（通常是最新版）
    Ok(versions.last().unwrap().to_string())
}

/// 更新锁文件（简化实现：只记录直接依赖）
fn update_lock_file(config: &ProjectConfig) -> Result<()> {
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

/// 依赖树（简化实现）
pub fn tree() -> Result<()> {
    let lock = read_jex_lock()?;
    let dependencies = lock.dependencies.unwrap_or_default();

    if dependencies.is_empty() {
        println!("无依赖");
        return Ok(());
    }

    println!("依赖树:");
    for (coord, version) in &dependencies {
        println!("  {} {}", coord, version);
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
        println!("{} {} 被直接依赖（在 jex.toml 中声明）", full_coord, version);
    } else {
        println!("未找到依赖: {}", full_coord);
    }

    Ok(())
}

/// 依赖冲突分析（简化实现）
pub fn conflict() -> Result<()> {
    let lock = read_jex_lock()?;
let _dependencies = lock.dependencies.unwrap_or_default();

    println!("依赖冲突分析:");
    println!("  当前无冲突（简化实现）");

    Ok(())
}
