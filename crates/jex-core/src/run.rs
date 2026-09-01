//! 一键运行（解析 → 编译 → 运行 + 缓存）
//! - run: 自动解析依赖 → 拼 classpath → javac → java

use crate::deps;
use crate::error::{Error, Result};
use crate::jdk;
use crate::resolver;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 获取项目构建输出目录
fn build_dir() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    Ok(cwd.join(".jex-build"))
}

/// 从 jex.lock.toml 构建 classpath（使用 resolver 推导依赖树）
fn build_classpath(lock: &deps::LockFile) -> Result<String> {
    let dependencies = lock.dependencies.clone().unwrap_or_default();
    let mut paths: Vec<String> = Vec::new();

    for (coord, version) in dependencies.iter() {
        // 解析该坐标的依赖树
        match resolver::resolve_dependencies(coord) {
            Ok(node) => {
                // 收集所有 jar 路径（仅直接依赖 + 顶层传递依赖）
                let local_path = format!(
                    "{}/{}/{}/{}-{}.jar",
                    crate::config::jex_m2_cache()?.display(),
                    node.group.replace('.', "/"),
                    node.artifact,
                    node.artifact,
                    node.version
                );
                // 注：v0.4.0 仅生成预期路径，实际下载由用户后续做
                paths.push(local_path);
            }
            Err(_) => {
                // 解析失败时退化为预期路径
                let parts: Vec<&str> = coord.split(':').collect();
                if parts.len() >= 2 {
                    let path = format!(
                        "{}/{}/{}/{}-{}.jar",
                        crate::config::jex_m2_cache()?.display(),
                        parts[0].replace('.', "/"),
                        parts[1],
                        parts[1],
                        version
                    );
                    paths.push(path);
                }
            }
        }
    }

    Ok(paths.join(":"))
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
    let classpath = build_classpath(&lock)?;
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

    // 提取主类名：优先 [project].main，否则从文件名推导
    let main_class = config
        .project
        .as_ref()
        .and_then(|p| p.main.as_deref())
        .or_else(|| file_path.file_stem().and_then(|s| s.to_str()))
        .ok_or_else(|| {
            Error::new("无法确定主类名：请在 jex.toml 中设置 [project].main 或传入文件路径")
        })?;
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
