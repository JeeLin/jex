//! 一键运行（解析 → 编译 → 运行 + 缓存）
//! - compile: 编译 Java 文件（供 jex build 和 jex run 共用）
//! - run: 自动解析依赖 → 拼 classpath → javac → java

use crate::deps;
use crate::error::{Error, Result};
use crate::jdk;
use crate::resolver;
use crate::script::ScriptMeta;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
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

/// 计算内容哈希（用于缓存键）
fn content_hash(content: &str) -> String {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// 计算依赖列表的哈希
fn deps_hash(deps: &[String]) -> String {
    let mut hasher = DefaultHasher::new();
    for dep in deps {
        dep.hash(&mut hasher);
    }
    format!("{:x}", hasher.finish())
}

/// 获取脚本缓存目录
fn script_cache_dir(source_hash: &str) -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| Error::new("无法获取用户主目录"))?;
    Ok(home
        .join(".jex")
        .join("cache")
        .join("scripts")
        .join(source_hash))
}

/// 缓存编译：首次编译后缓存 class 文件，依赖或源码未变时跳过编译
pub fn get_or_compile(script_path: &Path, meta: &ScriptMeta) -> Result<PathBuf> {
    // 1. 计算源码哈希
    let source =
        fs::read_to_string(script_path).map_err(|e| Error::new(format!("无法读取源码: {}", e)))?;
    let src_hash = content_hash(&source);

    // 2. 计算依赖哈希
    let dep_hash = deps_hash(&meta.deps);
    let combined_hash = content_hash(&format!("{}:{}", src_hash, dep_hash));

    // 3. 检查缓存
    let cache_dir = script_cache_dir(&combined_hash)?;
    let class_dir = cache_dir.join("classes");
    if class_dir.exists() {
        // 缓存命中：检查是否有 .class 文件
        if fs::read_dir(&class_dir)?.any(|e| {
            e.ok()
                .and_then(|e| e.path().extension().map(|ext| ext == "class"))
                .unwrap_or(false)
        }) {
            return Ok(class_dir);
        }
    }

    // 4. 缓存未命中：需要编译
    // 这里返回缓存目录路径，由调用方负责实际编译
    fs::create_dir_all(&class_dir)?;
    Ok(class_dir)
}

/// 编译 Java 文件（独立编译命令，供 jex build 和 jex run 共用）
pub fn compile(files: &[&str], clean: bool) -> Result<PathBuf> {
    // 1. 读取配置
    let config = deps::read_jex_toml()?;
    let lock = deps::read_jex_lock()?;

    // 2. 获取 JDK 路径
    let java_home = jdk::which_java_home()?;
    let javac_bin = java_home.join("bin").join("javac");

    if !javac_bin.exists() {
        return Err(Error::new(format!("javac 不存在: {}", javac_bin.display())));
    }

    // 3. 构建 classpath
    let classpath = build_classpath(&lock)?;

    // 4. 创建构建输出目录
    let build = build_dir()?;
    if clean && build.exists() {
        fs::remove_dir_all(&build)?;
    }
    fs::create_dir_all(&build)?;

    // 5. 编译
    println!("Compiling {} files...", files.len());

    let mut compile_cmd = Command::new(&javac_bin);
    compile_cmd
        .arg("-cp")
        .arg(&classpath)
        .arg("-d")
        .arg(&build);

    for file in files {
        compile_cmd.arg(file);
    }

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

    Ok(build)
}

/// 收集 src/ 下所有 .java 文件
pub fn collect_java_files() -> Result<Vec<String>> {
    let src_dir = Path::new("src");
    if !src_dir.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    collect_java_files_recursive(src_dir, &mut files)?;
    Ok(files)
}

fn collect_java_files_recursive(dir: &Path, files: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_java_files_recursive(&path, files)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("java") {
            files.push(path.to_string_lossy().to_string());
        }
    }
    Ok(())
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

    // 3. 获取 JDK 路径
    let java_home = jdk::which_java_home()?;
    let java_bin = java_home.join("bin").join("java");

    if !java_bin.exists() {
        return Err(Error::new(format!("java 不存在: {}", java_bin.display())));
    }

    // 4. 编译（复用 compile 函数）
    let build = compile(&[file], false)?;
    let lock = deps::read_jex_lock()?;
    let classpath = build_classpath(&lock)?;

    // 5. 运行
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deps::LockFile;
    use std::collections::HashMap;

    #[test]
    fn test_build_dir() {
        let dir = build_dir().unwrap();
        assert!(dir.to_string_lossy().contains(".jex-build"));
        assert!(dir.is_absolute());
    }

    #[test]
    fn test_run_file_not_found() {
        let result = run("/nonexistent/file.java", &[]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("文件不存在"));
    }

    #[test]
    fn test_build_classpath_empty_lock() {
        let lock = LockFile {
            lockfile_version: Some(1),
            dependencies: Some(HashMap::new()),
        };
        let result = build_classpath(&lock);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_build_classpath_with_deps() {
        let lock = LockFile {
            lockfile_version: Some(1),
            dependencies: Some({
                let mut deps = HashMap::new();
                deps.insert(
                    "com.google.code.gson:gson".to_string(),
                    "2.11.0".to_string(),
                );
                deps
            }),
        };
        // 即使 resolver 可能失败，也应该返回一个 classpath（退化为预期路径）
        let result = build_classpath(&lock);
        assert!(result.is_ok());
        let cp = result.unwrap();
        assert!(cp.contains("gson"));
    }

    #[test]
    fn test_content_hash() {
        let h1 = content_hash("hello");
        let h2 = content_hash("hello");
        let h3 = content_hash("world");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_deps_hash() {
        let h1 = deps_hash(&["a:b:1.0".to_string()]);
        let h2 = deps_hash(&["a:b:1.0".to_string()]);
        let h3 = deps_hash(&["a:b:2.0".to_string()]);
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_get_or_compile_creates_cache_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join("test.java");
        std::fs::write(&script, "//DEPS a:b:1.0\npublic class Test {}").unwrap();
        let meta = ScriptMeta {
            java_version: None,
            deps: vec!["a:b:1.0".to_string()],
            is_script: true,
        };
        let result = get_or_compile(&script, &meta);
        assert!(result.is_ok());
        let class_dir = result.unwrap();
        assert!(class_dir.exists());
    }

    #[test]
    fn test_get_or_compile_cache_hit() {
        let tmp = tempfile::tempdir().unwrap();
        let script = tmp.path().join("test.java");
        std::fs::write(&script, "//DEPS a:b:1.0\npublic class Test {}").unwrap();
        let meta = ScriptMeta {
            java_version: None,
            deps: vec!["a:b:1.0".to_string()],
            is_script: true,
        };
        // First call creates cache
        let dir1 = get_or_compile(&script, &meta).unwrap();
        // Create a .class file to simulate previous compilation
        std::fs::write(dir1.join("Test.class"), b"mock").unwrap();
        // Second call should hit cache
        let dir2 = get_or_compile(&script, &meta).unwrap();
        assert_eq!(dir1, dir2);
    }

    #[test]
    fn test_collect_java_files_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let result = collect_java_files();
        std::env::set_current_dir(&orig).unwrap();
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
