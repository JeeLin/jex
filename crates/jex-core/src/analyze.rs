//! 依赖分析：检测未使用和未声明的依赖

use crate::deps;
use crate::error::Result;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// 分析项目依赖：检测未使用和未声明的依赖
pub fn analyze_project() -> Result<()> {
    // 1. 读取 jex.toml 中声明的依赖
    let config = deps::read_jex_toml()?;
    let declared: HashMap<String, String> = config.dependencies.unwrap_or_default();

    // 2. 扫描 src/ 下所有 .java 文件的 import 语句
    let imports = scan_imports("src")?;

    // 3. 将 import 解析为 Maven 坐标 (group:artifact)
    let mut used_artifacts: HashSet<String> = HashSet::new();
    for import in &imports {
        if let Some(coord) = import_to_coord(import) {
            used_artifacts.insert(coord);
        }
    }

    // 4. 将声明的依赖提取为 group:artifact 集合
    let mut declared_artifacts: HashMap<String, String> = HashMap::new();
    for (coord, version) in &declared {
        // coord 格式: group:artifact
        declared_artifacts.insert(coord.clone(), version.clone());
    }

    // 5. 找出未使用的依赖（声明但未引用）
    let mut unused: Vec<String> = Vec::new();
    for coord in declared_artifacts.keys() {
        if !used_artifacts.contains(coord) {
            unused.push(coord.clone());
        }
    }

    // 6. 找出未声明的依赖（引用但未声明）
    let mut undeclared: Vec<String> = Vec::new();
    for coord in &used_artifacts {
        if !declared_artifacts.contains_key(coord) {
            undeclared.push(coord.clone());
        }
    }

    // 7. 输出结果
    println!("📊 依赖分析\n");

    if !unused.is_empty() {
        println!("未使用的依赖（声明但未引用）：");
        for coord in &unused {
            let version = declared_artifacts.get(coord).unwrap();
            println!("  ⚠ {}:{} — 未在源码中发现 import", coord, version);
        }
        println!();
    }

    if !undeclared.is_empty() {
        println!("未声明的依赖（引用但未声明）：");
        for coord in &undeclared {
            println!("  ❌ {} — 在源码中引用但未在 jex.toml 中声明", coord);
        }
        println!();
    }

    let total = declared_artifacts.len();
    let issues = unused.len() + undeclared.len();
    if issues == 0 {
        println!("✅ 已分析 {} 个依赖，无问题", total);
    } else {
        println!("✅ 已分析 {} 个依赖，{} 个问题", total, issues);
    }

    Ok(())
}

/// 扫描目录下所有 .java 文件的 import 语句
fn scan_imports(dir: &str) -> Result<Vec<String>> {
    let mut imports = Vec::new();
    let path = Path::new(dir);

    if !path.exists() {
        return Ok(imports);
    }

    scan_dir(path, &mut imports)?;
    Ok(imports)
}

/// 递归扫描目录
fn scan_dir(dir: &Path, imports: &mut Vec<String>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            scan_dir(&path, imports)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("java") {
            let content = fs::read_to_string(&path)?;
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("import ") && trimmed.ends_with(';') {
                    let import = &trimmed[7..trimmed.len() - 1];
                    // 跳过 static import
                    if !import.starts_with("static ") {
                        imports.push(import.to_string());
                    }
                }
            }
        }
    }

    Ok(())
}

/// 将 Java import 语句转换为 Maven 坐标 (group:artifact)
/// 例如: com.google.gson.Gson -> com.google.code.gson:gson
/// 注意：这是一个简化的映射，实际场景可能需要更复杂的查找
fn import_to_coord(import: &str) -> Option<String> {
    // 跳过 JDK 标准库
    if import.starts_with("java.") || import.starts_with("javax.") || import.starts_with("jdk.") {
        return None;
    }

    let parts: Vec<&str> = import.split('.').collect();
    if parts.len() < 2 {
        return None;
    }

    // 尝试常见的 group:artifact 映射
    // 简化策略：取前 N 级作为 group，最后一级作为 artifact
    // 这是一个近似匹配，真实场景需要 Maven Central API 查询

    // 常见映射表
    let known_mappings: HashMap<&str, &str> = HashMap::from([
        ("com.google.gson", "com.google.code.gson:gson"),
        ("org.apache.commons.lang3", "org.apache.commons:commons-lang3"),
        ("org.apache.commons.lang", "commons-lang:commons-lang"),
        ("org.slf4j", "org.slf4j:slf4j-api"),
        ("org.junit.jupiter", "org.junit.jupiter:junit-jupiter"),
        ("org.junit", "junit:junit"),
        ("org.mockito", "org.mockito:mockito-core"),
        ("com.fasterxml.jackson", "com.fasterxml.jackson.core:jackson-databind"),
        ("com.fasterxml.jackson.annotation", "com.fasterxml.jackson.core:jackson-annotations"),
        ("com.fasterxml.jackson.databind", "com.fasterxml.jackson.core:jackson-databind"),
        ("org.springframework", "org.springframework:spring-core"),
        ("org.springframework.boot", "org.springframework.boot:spring-boot"),
        ("io.netty", "io.netty:netty-all"),
        ("org.apache.httpcomponents", "org.apache.httpcomponents:httpclient"),
        ("org.json", "org.json:json"),
        ("com.squareup.okhttp3", "com.squareup.okhttp3:okhttp"),
        ("org.apache.logging.log4j", "org.apache.logging.log4j:log4j-core"),
        ("jakarta.servlet", "jakarta.servlet:jakarta.servlet-api"),
        ("javax.servlet", "javax.servlet:javax.servlet-api"),
        ("com.mysql", "com.mysql:mysql-connector-j"),
        ("org.postgresql", "org.postgresql:postgresql"),
        ("redis.clients", "redis.clients:jedis"),
        ("com.rabbitmq", "com.rabbitmq:amqp-client"),
        ("io.micrometer", "io.micrometer:micrometer-core"),
        ("org.thymeleaf", "org.thymeleaf:thymeleaf"),
        ("com.github", "com.github.ben-manes.caffeine:caffeine"),
    ]);

    // 尝试精确匹配
    for i in (2..=parts.len().min(5)).rev() {
        let prefix = parts[..i].join(".");
        if let Some(coord) = known_mappings.get(prefix.as_str()) {
            return Some(coord.to_string());
        }
    }

    // 尝试自动推断：group = 前 N-1 级，artifact = 最后一级
    // 但 Java 的 import 通常是 package.class，不直接对应 Maven 坐标
    // 这里做一个基本的启发式
    if parts.len() >= 3 {
        let group = parts[..parts.len() - 1].join(".");
        let artifact = parts.last()?;
        return Some(format!("{}:{}", group, artifact));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_import_to_coord_known() {
        assert_eq!(
            import_to_coord("com.google.gson.Gson"),
            Some("com.google.code.gson:gson".to_string())
        );
        assert_eq!(
            import_to_coord("org.slf4j.Logger"),
            Some("org.slf4j:slf4j-api".to_string())
        );
    }

    #[test]
    fn test_import_to_coord_unknown() {
        // 未知的 import 应该返回启发式结果
        let result = import_to_coord("com.example.mylib.MyClass");
        assert!(result.is_some());
        let coord = result.unwrap();
        assert!(coord.starts_with("com.example.mylib:"));
    }

    #[test]
    fn test_import_to_coord_short() {
        assert_eq!(import_to_coord("java.util.List"), None);
    }

    #[test]
    fn test_scan_imports_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let result = scan_imports(tmp.path().to_str().unwrap());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
