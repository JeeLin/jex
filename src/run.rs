//! 一键运行（解析 → 编译 → 运行 + 缓存）
//! - run: 自动解析依赖 → 拼 classpath → javac → java

use crate::deps;
use crate::error::{Error, Result};
use crate::jdk;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 获取 ~/.jex/cache 目录路径
fn cache_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| Error::new("找不到 HOME 环境变量"))?;
    Ok(PathBuf::from(home).join(".jex").join("cache"))
}

/// 获取项目构建输出目录
fn build_dir() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    Ok(cwd.join(".jex-build"))
}

/// 从 jex.lock.toml 构建 classpath
fn build_classpath(lock: &deps::LockFile, cache: &Path) -> Result<String> {
    let dependencies = lock.dependencies.clone().unwrap_or_default();
    let mut jars = Vec::new();

    for coord in dependencies.keys() {
        // 简化实现：直接查找 cache 中的 jar
        // 实际应该调用 Coursier 获取确切路径
        let parts: Vec<&str> = coord.split(':').collect();
        if parts.len() >= 2 {
            let group = parts[0].replace('.', "/");
            let artifact = parts[1];
            // 猜测 jar 路径（简化实现）
            let jar_pattern = format!("{}-*.jar", artifact);
            let group_dir = cache.join(&group).join(artifact);
            if group_dir.exists() {
                for entry in fs::read_dir(&group_dir)? {
                    let entry = entry?;
                    let name = entry.file_name();
                    if let Some(name_str) = name.to_str() {
                        if name_str.contains(&jar_pattern.replace("*", "")) && name_str.ends_with(".jar") {
                            jars.push(entry.path());
                        }
                    }
                }
            }
        }
    }

    Ok(jars.iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(":"))
}

/// 运行 Java 文件
pub fn run(file: &str, args: &[String]) -> Result<()> {
    // 1. 检查文件是否存在
    let file_path = Path::new(file);
    if !file_path.exists() {
        return Err(Error::new(format!("文件不存在: {}", file)));
    }

    // 2. 读取配置
    let config = deps::read_jex_toml()?;
    let lock = deps::read_jex_lock()?;

    // 3. 获取 JDK 路径
    let java_home = jdk::which_java_home()?;
    let java_bin = java_home.join("bin").join("java");
    let javac_bin = java_home.join("bin").join("javac");

    if !java_bin.exists() {
        return Err(Error::new(format!("java 不存在: {}", java_bin.display())));
    }
    if !javac_bin.exists() {
        return Err(Error::new(format!("javac 不存在: {}", javac_bin.display())));
    }

    // 4. 构建 classpath
    let cache = cache_dir()?;
    let classpath = build_classpath(&lock, &cache)?;

    // 5. 创建构建输出目录
    let build = build_dir()?;
    fs::create_dir_all(&build)?;

    // 6. 编译
    println!("编译 {}...", file);

    let mut compile_cmd = Command::new(&javac_bin);
    compile_cmd
        .arg("-cp")
        .arg(&classpath)
        .arg("-d")
        .arg(&build)
        .arg(file);

    // 添加编译参数
    if let Some(build_config) = &config.build {
        if let Some(compiler_args) = &build_config.compiler_args {
            for arg in compiler_args {
                compile_cmd.arg(arg);
            }
        }
    }

    let status = compile_cmd.status()?;
    if !status.success() {
        return Err(Error::new("编译失败"));
    }

    // 7. 运行
    println!("运行 {}...", file);

    // 提取主类名（从文件名）
    let main_class = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Error::new("无法提取主类名"))?;

    let mut run_cmd = Command::new(&java_bin);
    run_cmd
        .arg("-cp")
        .arg(format!("{}:{}", build.display(), classpath))
        .arg(main_class);

    // 添加运行参数
    if let Some(build_config) = &config.build {
        if let Some(jvm_args) = &build_config.jvm_args {
            for arg in jvm_args {
                run_cmd.arg(arg);
            }
        }
    }

    // 添加用户参数
    for arg in args {
        run_cmd.arg(arg);
    }

    let status = run_cmd.status()?;
    if !status.success() {
        return Err(Error::new("运行失败"));
    }

    Ok(())
}
