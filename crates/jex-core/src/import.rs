//! 生态互通（import maven）
//! - import maven: 读 pom.xml → 解析依赖 → 写入 jex.toml

use crate::deps::{self, ProjectConfig};
use crate::error::{Error, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::path::Path;

/// 从 pom.xml 解析出的依赖
#[derive(Debug, Clone, PartialEq)]
pub struct PomDependency {
    pub group_id: String,
    pub artifact_id: String,
    pub version: Option<String>,
    pub scope: Option<String>,
}

/// 解析 pom.xml，提取依赖列表
pub fn parse_pom(path: &Path) -> Result<Vec<PomDependency>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| Error::new(format!("无法读取 {}: {}", path.display(), e)))?;

    let mut reader = Reader::from_str(&content);

    let mut buf = Vec::new();
    let mut deps: Vec<PomDependency> = Vec::new();
    let mut mgmt_versions: HashMap<(String, String), String> = HashMap::new();

    let mut in_dependencies = false;
    let mut in_dep_mgmt = false;
    let mut in_dep = false;
    let mut current_dep: PomDependency = PomDependency {
        group_id: String::new(),
        artifact_id: String::new(),
        version: None,
        scope: None,
    };

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "dependencies" => {
                        // 判断是顶层 dependencies 还是 dependencyManagement 内的
                        if in_dep_mgmt {
                            // dependencyManagement 内的 dependencies，跳过解析
                        } else {
                            in_dependencies = true;
                        }
                    }
                    "dependencyManagement" => {
                        in_dep_mgmt = true;
                    }
                    "dependency" if in_dependencies || in_dep_mgmt => {
                        in_dep = true;
                        current_dep = PomDependency {
                            group_id: String::new(),
                            artifact_id: String::new(),
                            version: None,
                            scope: None,
                        };
                    }
                    _ => {}
                }

                // 提取文本内容
                if in_dep {
                    match tag.as_str() {
                        "groupId" => {
                            if let Ok(Event::Text(text)) = reader.read_event_into(&mut buf) {
                                current_dep.group_id = text.unescape()?.trim().to_string();
                            }
                        }
                        "artifactId" => {
                            if let Ok(Event::Text(text)) = reader.read_event_into(&mut buf) {
                                current_dep.artifact_id = text.unescape()?.trim().to_string();
                            }
                        }
                        "version" => {
                            if let Ok(Event::Text(text)) = reader.read_event_into(&mut buf) {
                                let ver = text.unescape()?.trim().to_string();
                                if in_dep_mgmt {
                                    // 记录 dependencyManagement 版本
                                    mgmt_versions.insert(
                                        (current_dep.group_id.clone(), current_dep.artifact_id.clone()),
                                        ver,
                                    );
                                } else {
                                    current_dep.version = Some(ver);
                                }
                            }
                        }
                        "scope" => {
                            if let Ok(Event::Text(text)) = reader.read_event_into(&mut buf) {
                                current_dep.scope = Some(text.unescape()?.trim().to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "dependency" if in_dep => {
                        // 应用 dependencyManagement 版本
                        if current_dep.version.is_none() {
                            if let Some(ver) = mgmt_versions.get(&(
                                current_dep.group_id.clone(),
                                current_dep.artifact_id.clone(),
                            )) {
                                current_dep.version = Some(ver.clone());
                            }
                        }

                        // 只收集 dependencies 区块内的依赖（跳过 dependencyManagement 内的）
                        if !in_dep_mgmt {
                            // 过滤 scope：只导入 compile scope（无 scope 或 scope=compile）
                            let scope = current_dep.scope.as_deref().unwrap_or("compile");
                            if scope == "compile" || scope.is_empty() {
                                deps.push(current_dep.clone());
                            }
                        }

                        in_dep = false;
                    }
                    "dependencies" => {
                        in_dependencies = false;
                    }
                    "dependencyManagement" => {
                        in_dep_mgmt = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::new(format!("解析 pom.xml 失败: {}", e))),
            _ => {}
        }
        buf.clear();
    }

    Ok(deps)
}

/// 将 PomDependency 列表转换为 jex 坐标字符串
pub fn pom_to_jex_coords(deps: &[PomDependency]) -> Vec<String> {
    deps.iter()
        .filter_map(|d| {
            let ver = d.version.as_deref()?;
            Some(format!("{}:{}:{}", d.group_id, d.artifact_id, ver))
        })
        .collect()
}

/// 将导入的依赖与现有 jex.toml 合并
pub fn merge_with_existing(
    imported: &[String],
    existing: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut merged = existing.clone();
    let mut warnings: Vec<String> = Vec::new();

    for coord in imported {
        // 解析 group:artifact:version
        let parts: Vec<&str> = coord.split(':').collect();
        if parts.len() < 3 {
            continue;
        }
        let key = format!("{}:{}", parts[0], parts[1]);
        let version = parts[2];

        if let Some(existing_ver) = merged.get(&key) {
            if existing_ver != version {
                warnings.push(format!(
                    "⚠ {} 版本冲突：现有 {}，导入 {}（保留现有版本）",
                    key, existing_ver, version
                ));
            }
            // 已有依赖，保留现有版本，不覆盖
        } else {
            // 新依赖，追加
            merged.insert(key, version.to_string());
        }
    }

    for w in &warnings {
        println!("{}", w);
    }

    merged
}

/// 导入 pom.xml 到 jex 项目
pub fn import_maven(pom_path: &Path) -> Result<()> {
    // 1. 解析 pom.xml
    let pom_deps = parse_pom(pom_path)?;
    println!("📦 从 pom.xml 导入依赖\n");
    println!("解析到 {} 个依赖：", pom_deps.len());

    for dep in &pom_deps {
        let scope = dep.scope.as_deref().unwrap_or("compile");
        if let Some(ref ver) = dep.version {
            println!("  ✅ {}:{}:{}", dep.group_id, dep.artifact_id, ver);
        } else {
            println!(
                "  ⚠ {}:{}（无版本号，需手动指定）",
                dep.group_id, dep.artifact_id
            );
        }
        if scope != "compile" {
            println!("    （scope={}，已跳过）", scope);
        }
    }
    println!();

    // 2. 转换坐标
    let coords = pom_to_jex_coords(&pom_deps);

    // 3. 读取现有配置
    let mut config = match deps::read_jex_toml() {
        Ok(c) => c,
        Err(_) => {
            // 没有 jex.toml，创建新的
            println!("未找到 jex.toml，将创建新项目配置");
            ProjectConfig {
                project: None,
                dependencies: Some(HashMap::new()),
                repositories: None,
                build: None,
            }
        }
    };

    let existing = config.dependencies.clone().unwrap_or_default();

    // 4. 合并
    let merged = merge_with_existing(&coords, &existing);
    config.dependencies = Some(merged.clone());

    // 5. 写回 jex.toml
    deps::write_jex_toml(&config)?;

    let added = merged.len() - existing.len();
    println!(
        "导入完成！已添加 {} 个依赖到 jex.toml（共 {} 个依赖）",
        added,
        merged.len()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pom_simple() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
    <modelVersion>4.0.0</modelVersion>
    <dependencies>
        <dependency>
            <groupId>com.google.code.gson</groupId>
            <artifactId>gson</artifactId>
            <version>2.11.0</version>
        </dependency>
    </dependencies>
</project>"#;

        let tmp = tempfile::tempdir().unwrap();
        let pom_path = tmp.path().join("pom.xml");
        std::fs::write(&pom_path, xml).unwrap();

        let deps = parse_pom(&pom_path).unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].group_id, "com.google.code.gson");
        assert_eq!(deps[0].artifact_id, "gson");
        assert_eq!(deps[0].version.as_deref(), Some("2.11.0"));
    }

    #[test]
    fn test_parse_pom_with_dependency_management() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
    <modelVersion>4.0.0</modelVersion>
    <dependencyManagement>
        <dependencies>
            <dependency>
                <groupId>com.google.code.gson</groupId>
                <artifactId>gson</artifactId>
                <version>2.10.0</version>
            </dependency>
        </dependencies>
    </dependencyManagement>
    <dependencies>
        <dependency>
            <groupId>com.google.code.gson</groupId>
            <artifactId>gson</artifactId>
        </dependency>
    </dependencies>
</project>"#;

        let tmp = tempfile::tempdir().unwrap();
        let pom_path = tmp.path().join("pom.xml");
        std::fs::write(&pom_path, xml).unwrap();

        let deps = parse_pom(&pom_path).unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].version.as_deref(), Some("2.10.0"));
    }

    #[test]
    fn test_parse_pom_scope_filtering() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
    <modelVersion>4.0.0</modelVersion>
    <dependencies>
        <dependency>
            <groupId>com.google.code.gson</groupId>
            <artifactId>gson</artifactId>
            <version>2.11.0</version>
        </dependency>
        <dependency>
            <groupId>junit</groupId>
            <artifactId>junit</artifactId>
            <version>4.13.2</version>
            <scope>test</scope>
        </dependency>
    </dependencies>
</project>"#;

        let tmp = tempfile::tempdir().unwrap();
        let pom_path = tmp.path().join("pom.xml");
        std::fs::write(&pom_path, xml).unwrap();

        let deps = parse_pom(&pom_path).unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].artifact_id, "gson");
    }

    #[test]
    fn test_parse_pom_empty() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
    <modelVersion>4.0.0</modelVersion>
</project>"#;

        let tmp = tempfile::tempdir().unwrap();
        let pom_path = tmp.path().join("pom.xml");
        std::fs::write(&pom_path, xml).unwrap();

        let deps = parse_pom(&pom_path).unwrap();
        assert!(deps.is_empty());
    }

    #[test]
    fn test_pom_to_jex_coords() {
        let deps = vec![
            PomDependency {
                group_id: "com.google.code.gson".to_string(),
                artifact_id: "gson".to_string(),
                version: Some("2.11.0".to_string()),
                scope: None,
            },
            PomDependency {
                group_id: "org.slf4j".to_string(),
                artifact_id: "slf4j-api".to_string(),
                version: None, // 无版本
                scope: None,
            },
        ];

        let coords = pom_to_jex_coords(&deps);
        assert_eq!(coords.len(), 1);
        assert_eq!(coords[0], "com.google.code.gson:gson:2.11.0");
    }

    #[test]
    fn test_merge_with_existing_no_conflict() {
        let imported = vec![
            "com.google.code.gson:gson:2.11.0".to_string(),
            "org.slf4j:slf4j-api:2.0.9".to_string(),
        ];
        let existing = HashMap::new();

        let merged = merge_with_existing(&imported, &existing);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged.get("com.google.code.gson:gson").unwrap(), "2.11.0");
    }

    #[test]
    fn test_merge_with_existing_conflict() {
        let imported = vec!["com.google.code.gson:gson:2.11.0".to_string()];
        let mut existing = HashMap::new();
        existing.insert("com.google.code.gson:gson".to_string(), "2.10.0".to_string());

        let merged = merge_with_existing(&imported, &existing);
        assert_eq!(merged.len(), 1);
        // 保留现有版本
        assert_eq!(merged.get("com.google.code.gson:gson").unwrap(), "2.10.0");
    }

    #[test]
    fn test_parse_pom_multiple_deps() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
    <modelVersion>4.0.0</modelVersion>
    <dependencies>
        <dependency>
            <groupId>com.google.code.gson</groupId>
            <artifactId>gson</artifactId>
            <version>2.11.0</version>
        </dependency>
        <dependency>
            <groupId>org.slf4j</groupId>
            <artifactId>slf4j-api</artifactId>
            <version>2.0.9</version>
        </dependency>
        <dependency>
            <groupId>org.apache.commons</groupId>
            <artifactId>commons-lang3</artifactId>
            <version>3.14.0</version>
        </dependency>
    </dependencies>
</project>"#;

        let tmp = tempfile::tempdir().unwrap();
        let pom_path = tmp.path().join("pom.xml");
        std::fs::write(&pom_path, xml).unwrap();

        let deps = parse_pom(&pom_path).unwrap();
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].artifact_id, "gson");
        assert_eq!(deps[1].artifact_id, "slf4j-api");
        assert_eq!(deps[2].artifact_id, "commons-lang3");
    }

    #[test]
    fn test_merge_with_existing_add_new() {
        let imported = vec![
            "com.google.code.gson:gson:2.11.0".to_string(),
            "org.slf4j:slf4j-api:2.0.9".to_string(),
            "org.apache.commons:commons-lang3:3.14.0".to_string(),
        ];
        let mut existing = HashMap::new();
        existing.insert("com.google.code.gson:gson".to_string(), "2.10.0".to_string());

        let merged = merge_with_existing(&imported, &existing);
        assert_eq!(merged.len(), 3);
        // 保留现有版本
        assert_eq!(merged.get("com.google.code.gson:gson").unwrap(), "2.10.0");
        // 新依赖追加
        assert_eq!(merged.get("org.slf4j:slf4j-api").unwrap(), "2.0.9");
        assert_eq!(merged.get("org.apache.commons:commons-lang3").unwrap(), "3.14.0");
    }

    #[test]
    #[test]
    fn test_merge_with_existing_invalid_coord() {
        let imported = vec!["invalid-coord".to_string()];
        let existing = HashMap::new();

        let merged = merge_with_existing(&imported, &existing);
        // 无效坐标被跳过
        assert!(merged.is_empty());
    }

    #[test]
    fn test_parse_pom_missing_dependencies_section() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.example</groupId>
    <artifactId>my-app</artifactId>
</project>"#;

        let tmp = tempfile::tempdir().unwrap();
        let pom_path = tmp.path().join("pom.xml");
        std::fs::write(&pom_path, xml).unwrap();

        let deps = parse_pom(&pom_path).unwrap();
        // 没有 dependencies 区块，返回空列表
        assert!(deps.is_empty());
    }
}
