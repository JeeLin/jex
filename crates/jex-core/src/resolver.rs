//! 原生 Maven 依赖解析器
//! - resolve_latest: 通过 Maven Central search API 获取最新版本
//! - resolve_dependencies: 通过 POM 解析传递依赖
//! - 依赖树结构 DepNode

use crate::error::{Error, Result};
use std::collections::HashSet;

/// 依赖树节点
#[derive(Debug, Clone)]
pub struct DepNode {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub scope: String,
    pub children: Vec<DepNode>,
}

/// Maven Central search API 响应
#[derive(Debug, serde::Deserialize)]
struct SearchResponse {
    response: Option<SearchResult>,
}

#[derive(Debug, serde::Deserialize)]
struct SearchResult {
    docs: Option<Vec<SearchDoc>>,
}

#[derive(Debug, serde::Deserialize)]
struct SearchDoc {
    #[serde(rename = "v")]
    version: Option<String>,
}

/// 解析 `group:artifact` 坐标为 (group, artifact)
fn parse_coord(coord: &str) -> Result<(&str, &str)> {
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() < 2 {
        return Err(Error::new(format!(
            "无效坐标格式: {}（应为 group:artifact）",
            coord
        )));
    }
    Ok((parts[0], parts[1]))
}

/// 创建带 User-Agent 的 blocking client（Maven Central 要求 User-Agent）
fn http_get(url: &str) -> Result<String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("jex/0.4.0 (Rust)")
        .build()
        .map_err(|e| Error::new(format!("HTTP client 创建失败: {}", e)))?;
    let resp = client
        .get(url)
        .send()
        .map_err(|e| Error::new(format!("HTTP 请求失败: {}", e)))?;
    resp.text()
        .map_err(|e| Error::new(format!("读取响应失败: {}", e)))
}

/// 获取 Maven Central search API 返回的版本列表（降序排列）
fn search_versions(group: &str, artifact: &str) -> Result<Vec<String>> {
    let url = format!(
        "https://search.maven.org/solrsearch/select?q=g:{}+AND+a:{}&core=gav&rows=200&wt=json",
        group, artifact
    );
    let body = http_get(&url)?;
    let sr: SearchResponse =
        serde_json::from_str(&body).map_err(|e| Error::new(format!("JSON 解析失败: {}", e)))?;
    let docs = sr.response.and_then(|r| r.docs).unwrap_or_default();
    Ok(docs.into_iter().filter_map(|d| d.version).collect())
}

/// 通过 Maven Central search API 获取最新版本号
pub fn resolve_latest(coord: &str) -> Result<String> {
    let (group, artifact) = parse_coord(coord)?;
    let versions = search_versions(group, artifact)?;
    versions
        .into_iter()
        .next()
        .ok_or_else(|| Error::new(format!("未找到依赖: {}", coord)))
}

/// 获取 POM 文件内容
fn fetch_pom(group: &str, artifact: &str, version: &str) -> Result<String> {
    let path = group.replace('.', "/");
    let url = format!(
        "https://repo1.maven.org/maven2/{}/{}/{}/{}-{}.pom",
        path, artifact, version, artifact, version
    );
    let body = http_get(&url)?;
    if body.is_empty() {
        return Err(Error::new(format!("POM 下载失败（响应为空）: {}", url)));
    }
    Ok(body)
}

/// 从 POM XML 中提取依赖列表（group:artifact:version，仅 compile/runtime scope）
fn parse_pom_dependencies(pom: &str) -> Result<Vec<(String, String, String)>> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(pom);
    let mut buf = Vec::new();
    let mut deps: Vec<(String, String, String)> = Vec::new();

    // 状态机：追踪当前在哪个 XML 元素内
    let mut in_deps = false;
    let mut in_dep = false;
    let mut current_group = String::new();
    let mut current_artifact = String::new();
    let mut current_version = String::new();
    let mut current_scope = String::from("compile");
    let mut current_element = String::new();
    let mut skip_optional = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "dependencies" if !in_deps => in_deps = true,
                    "dependency" if in_deps => {
                        in_dep = true;
                        current_group.clear();
                        current_artifact.clear();
                        current_version.clear();
                        current_scope = "compile".to_string();
                    }
                    _ => {
                        if in_dep {
                            current_element = tag;
                        }
                    }
                }
            }
            Ok(Event::Text(e)) => {
                if in_dep && !current_element.is_empty() {
                    let text = e.unescape().unwrap_or_default().to_string();
                    match current_element.as_str() {
                        "groupId" => current_group = text,
                        "artifactId" => current_artifact = text,
                        "version" => current_version = text,
                        "scope" => current_scope = text,
                        "optional" if text == "true" => {
                            skip_optional = true;
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "dependency" if in_dep => {
                        in_dep = false;
                        current_element.clear();
                        // 只保留 compile/runtime scope 的非 optional 依赖
                        if !skip_optional
                            && (current_scope == "compile" || current_scope == "runtime")
                            && !current_version.is_empty()
                            && !current_group.is_empty()
                            && !current_artifact.is_empty()
                        {
                            deps.push((
                                current_group.clone(),
                                current_artifact.clone(),
                                current_version.clone(),
                            ));
                        }
                        skip_optional = false;
                    }
                    "dependencies" if in_deps => {
                        in_deps = false;
                    }
                    _ => {}
                }
                current_element.clear();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(deps)
}

/// 解析依赖的传递闭包（有深度限制）
pub fn resolve_dependencies(coord: &str) -> Result<DepNode> {
    let (group, artifact) = parse_coord(coord)?;
    let mut visited = HashSet::new();
    resolve_node(group, artifact, None, &mut visited, 0)
}

/// 递归解析单个依赖节点
fn resolve_node(
    group: &str,
    artifact: &str,
    version: Option<&str>,
    visited: &mut HashSet<String>,
    depth: usize,
) -> Result<DepNode> {
    const MAX_DEPTH: usize = 8;
    let coord_key = format!("{}:{}", group, artifact);

    // 已访问或超深，返回叶节点
    if visited.contains(&coord_key) || depth >= MAX_DEPTH {
        let ver = version.unwrap_or("?");
        return Ok(DepNode {
            group: group.to_string(),
            artifact: artifact.to_string(),
            version: ver.to_string(),
            scope: "compile".to_string(),
            children: vec![],
        });
    }
    visited.insert(coord_key);

    // 获取版本
    let ver_str = match version {
        Some(v) => v.to_string(),
        None => resolve_latest(&format!("{}:{}", group, artifact))?,
    };

    // 下载并解析 POM
    let children = match fetch_pom(group, artifact, &ver_str) {
        Ok(pom) => match parse_pom_dependencies(&pom) {
            Ok(raw_deps) => {
                let mut children = Vec::new();
                for (g, a, v) in raw_deps {
                    if let Ok(child) = resolve_node(&g, &a, Some(&v), visited, depth + 1) {
                        children.push(child);
                    }
                }
                children
            }
            Err(_) => vec![],
        },
        Err(_) => vec![],
    };

    Ok(DepNode {
        group: group.to_string(),
        artifact: artifact.to_string(),
        version: ver_str,
        scope: "compile".to_string(),
        children,
    })
}

/// 将依赖树格式化为缩进字符串
pub fn format_tree(node: &DepNode, prefix: &str, is_last: bool) -> String {
    let connector = if is_last { "└─ " } else { "├─ " };
    let mut result = format!(
        "{}{}{}:{}:{}\n",
        prefix, connector, node.group, node.artifact, node.version
    );
    let child_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
    for (i, child) in node.children.iter().enumerate() {
        result.push_str(&format_tree(
            child,
            &child_prefix,
            i == node.children.len() - 1,
        ));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_coord_valid() {
        let (g, a) = parse_coord("com.google.code.gson:gson").unwrap();
        assert_eq!(g, "com.google.code.gson");
        assert_eq!(a, "gson");
    }

    #[test]
    fn test_parse_coord_invalid() {
        assert!(parse_coord("no-colon").is_err());
    }

    #[test]
    #[ignore] // 需要网络访问 Maven Central
    fn test_resolve_latest_gson() {
        let result = resolve_latest("com.google.code.gson:gson");
        assert!(result.is_ok());
        let ver = result.unwrap();
        // gson 最新版本应该是 2.x
        assert!(ver.starts_with("2."), "got: {}", ver);
    }

    #[test]
    fn test_format_tree() {
        let node = DepNode {
            group: "com.google.code.gson".to_string(),
            artifact: "gson".to_string(),
            version: "2.11.0".to_string(),
            scope: "compile".to_string(),
            children: vec![],
        };
        let output = format_tree(&node, "", true);
        assert!(output.contains("gson:2.11.0"));
    }

    #[test]
    fn test_format_tree_not_last() {
        let node = DepNode {
            group: "com.google.code.gson".to_string(),
            artifact: "gson".to_string(),
            version: "2.11.0".to_string(),
            scope: "compile".to_string(),
            children: vec![],
        };
        let output = format_tree(&node, "", false);
        assert!(output.contains("├─ "));
        // No child prefix since there are no children
    }

    #[test]
    fn test_format_tree_with_children() {
        let child = DepNode {
            group: "org.apache.commons".to_string(),
            artifact: "commons-lang3".to_string(),
            version: "3.12.0".to_string(),
            scope: "compile".to_string(),
            children: vec![],
        };
        let parent = DepNode {
            group: "com.example".to_string(),
            artifact: "parent".to_string(),
            version: "1.0.0".to_string(),
            scope: "compile".to_string(),
            children: vec![child],
        };
        let output = format_tree(&parent, "", true);
        assert!(output.contains("└─ com.example:parent:1.0.0"));
        assert!(output.contains("commons-lang3:3.12.0"));
        assert!(output.contains("   └─ "));
    }

    #[test]
    fn test_format_tree_multiple_children() {
        let child1 = DepNode {
            group: "g1".to_string(),
            artifact: "a1".to_string(),
            version: "1.0".to_string(),
            scope: "compile".to_string(),
            children: vec![],
        };
        let child2 = DepNode {
            group: "g2".to_string(),
            artifact: "a2".to_string(),
            version: "2.0".to_string(),
            scope: "compile".to_string(),
            children: vec![],
        };
        let parent = DepNode {
            group: "pg".to_string(),
            artifact: "pa".to_string(),
            version: "0.1".to_string(),
            scope: "compile".to_string(),
            children: vec![child1, child2],
        };
        let output = format_tree(&parent, "", false);
        assert!(output.contains("├─ pg:pa:0.1"));
        assert!(output.contains("│  ├─ g1:a1:1.0"));
        assert!(output.contains("│  └─ g2:a2:2.0"));
    }

    #[test]
    fn test_format_tree_nested() {
        let grandchild = DepNode {
            group: "gc".to_string(),
            artifact: "ga".to_string(),
            version: "1.0".to_string(),
            scope: "compile".to_string(),
            children: vec![],
        };
        let child = DepNode {
            group: "c".to_string(),
            artifact: "ca".to_string(),
            version: "2.0".to_string(),
            scope: "compile".to_string(),
            children: vec![grandchild],
        };
        let root = DepNode {
            group: "r".to_string(),
            artifact: "ra".to_string(),
            version: "3.0".to_string(),
            scope: "compile".to_string(),
            children: vec![child],
        };
        let output = format_tree(&root, "", false);
        assert!(output.contains("gc:ga:1.0"));
    }

    #[test]
    fn test_format_tree_with_prefix_and_not_last() {
        let node = DepNode {
            group: "g".to_string(),
            artifact: "a".to_string(),
            version: "1.0".to_string(),
            scope: "compile".to_string(),
            children: vec![],
        };
        let output = format_tree(&node, "  ", false);
        assert!(output.contains("  ├─ g:a:1.0"));
    }

    #[test]
    fn test_format_tree_not_last_with_prefix_and_children() {
        let child = DepNode {
            group: "cg".to_string(),
            artifact: "ca".to_string(),
            version: "2.0".to_string(),
            scope: "compile".to_string(),
            children: vec![],
        };
        let parent = DepNode {
            group: "pg".to_string(),
            artifact: "pa".to_string(),
            version: "1.0".to_string(),
            scope: "compile".to_string(),
            children: vec![child],
        };
        let output = format_tree(&parent, "  ", false);
        assert!(output.contains("  ├─ pg:pa:1.0"));
        assert!(output.contains("  │  └─ cg:ca:2.0"));
    }

    #[test]
    fn test_parse_pom_empty() {
        let result = parse_pom_dependencies("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_pom_no_dependencies() {
        let pom = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <groupId>com.example</groupId>
    <artifactId>my-project</artifactId>
    <version>1.0.0</version>
</project>"#;
        let result = parse_pom_dependencies(pom).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_pom_single_dependency() {
        let pom = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <dependencies>
        <dependency>
            <groupId>com.google.code.gson</groupId>
            <artifactId>gson</artifactId>
            <version>2.11.0</version>
        </dependency>
    </dependencies>
</project>"#;
        let result = parse_pom_dependencies(pom).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], ("com.google.code.gson".to_string(), "gson".to_string(), "2.11.0".to_string()));
    }

    #[test]
    fn test_parse_pom_runtime_scope() {
        let pom = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <dependencies>
        <dependency>
            <groupId>org.slf4j</groupId>
            <artifactId>slf4j-api</artifactId>
            <version>2.0.9</version>
            <scope>runtime</scope>
        </dependency>
    </dependencies>
</project>"#;
        let result = parse_pom_dependencies(pom).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "org.slf4j");
    }

    #[test]
    fn test_parse_pom_test_scope_excluded() {
        let pom = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <dependencies>
        <dependency>
            <groupId>junit</groupId>
            <artifactId>junit</artifactId>
            <version>4.13.2</version>
            <scope>test</scope>
        </dependency>
    </dependencies>
</project>"#;
        let result = parse_pom_dependencies(pom).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_pom_optional_dependency_excluded() {
        let pom = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <dependencies>
        <dependency>
            <groupId>com.google.errorprone</groupId>
            <artifactId>error_prone_annotations</artifactId>
            <version>2.18.0</version>
            <optional>true</optional>
        </dependency>
    </dependencies>
</project>"#;
        let result = parse_pom_dependencies(pom).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_pom_multiple_dependencies() {
        let pom = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <dependencies>
        <dependency>
            <groupId>g1</groupId>
            <artifactId>a1</artifactId>
            <version>1.0</version>
        </dependency>
        <dependency>
            <groupId>g2</groupId>
            <artifactId>a2</artifactId>
            <version>2.0</version>
            <scope>runtime</scope>
        </dependency>
        <dependency>
            <groupId>g3</groupId>
            <artifactId>a3</artifactId>
            <version>3.0</version>
            <scope>test</scope>
        </dependency>
        <dependency>
            <groupId>g4</groupId>
            <artifactId>a4</artifactId>
            <version>4.0</version>
            <optional>true</optional>
        </dependency>
    </dependencies>
</project>"#;
        let result = parse_pom_dependencies(pom).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "g1");
        assert_eq!(result[1].0, "g2");
    }

    #[test]
    fn test_parse_pom_missing_version_excluded() {
        let pom = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <dependencies>
        <dependency>
            <groupId>g1</groupId>
            <artifactId>a1</artifactId>
        </dependency>
    </dependencies>
</project>"#;
        let result = parse_pom_dependencies(pom).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_pom_missing_group_excluded() {
        let pom = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <dependencies>
        <dependency>
            <artifactId>a1</artifactId>
            <version>1.0</version>
        </dependency>
    </dependencies>
</project>"#;
        let result = parse_pom_dependencies(pom).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_pom_missing_artifact_excluded() {
        let pom = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <dependencies>
        <dependency>
            <groupId>g1</groupId>
            <version>1.0</version>
        </dependency>
    </dependencies>
</project>"#;
        let result = parse_pom_dependencies(pom).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_pom_optional_false_not_excluded() {
        let pom = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <dependencies>
        <dependency>
            <groupId>g1</groupId>
            <artifactId>a1</artifactId>
            <version>1.0</version>
            <optional>false</optional>
        </dependency>
    </dependencies>
</project>"#;
        let result = parse_pom_dependencies(pom).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_resolve_node_cycle_detection() {
        let mut visited = HashSet::new();
        visited.insert("com.example:lib".to_string());
        let result = resolve_node("com.example", "lib", Some("1.0"), &mut visited, 0).unwrap();
        assert_eq!(result.version, "1.0");
        assert!(result.children.is_empty());
    }

    #[test]
    fn test_resolve_node_depth_limit() {
        let mut visited = HashSet::new();
        let result = resolve_node("com.example", "lib", Some("1.0"), &mut visited, 8).unwrap();
        assert_eq!(result.version, "1.0");
        assert!(result.children.is_empty());
    }

    #[test]
    fn test_resolve_node_version_none() {
        let mut visited = HashSet::new();
        let result = resolve_node("com.example", "lib", None, &mut visited, 8).unwrap();
        assert_eq!(result.version, "?");
    }

    #[test]
    fn test_dep_node_clone() {
        let node = DepNode {
            group: "g".to_string(),
            artifact: "a".to_string(),
            version: "1.0".to_string(),
            scope: "compile".to_string(),
            children: vec![],
        };
        let cloned = node.clone();
        assert_eq!(cloned.group, "g");
        assert_eq!(cloned.artifact, "a");
        assert_eq!(cloned.version, "1.0");
    }

    #[test]
    fn test_dep_node_debug() {
        let node = DepNode {
            group: "g".to_string(),
            artifact: "a".to_string(),
            version: "1.0".to_string(),
            scope: "compile".to_string(),
            children: vec![],
        };
        let debug_str = format!("{:?}", node);
        assert!(debug_str.contains("DepNode"));
        assert!(debug_str.contains("g"));
    }

    #[test]
    fn test_dep_node_with_children_clone() {
        let child = DepNode {
            group: "cg".to_string(),
            artifact: "ca".to_string(),
            version: "2.0".to_string(),
            scope: "compile".to_string(),
            children: vec![],
        };
        let parent = DepNode {
            group: "pg".to_string(),
            artifact: "pa".to_string(),
            version: "1.0".to_string(),
            scope: "compile".to_string(),
            children: vec![child],
        };
        let cloned = parent.clone();
        assert_eq!(cloned.children.len(), 1);
        assert_eq!(cloned.children[0].artifact, "ca");
    }

    #[test]
    fn test_search_response_deserialize_empty() {
        let json = r#"{"response": null}"#;
        let sr: SearchResponse = serde_json::from_str(json).unwrap();
        assert!(sr.response.is_none());
    }

    #[test]
    fn test_search_response_deserialize_full() {
        let json = r#"{"response": {"docs": [{"v": "2.11.0"}, {"v": "2.10.0"}]}}"#;
        let sr: SearchResponse = serde_json::from_str(json).unwrap();
        let docs = sr.response.unwrap().docs.unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].version.as_deref(), Some("2.11.0"));
        assert_eq!(docs[1].version.as_deref(), Some("2.10.0"));
    }

    #[test]
    fn test_search_response_deserialize_no_docs() {
        let json = r#"{"response": {"docs": null}}"#;
        let sr: SearchResponse = serde_json::from_str(json).unwrap();
        let docs = sr.response.unwrap().docs.unwrap_or_default();
        assert!(docs.is_empty());
    }

    #[test]
    fn test_search_response_deserialize_no_version() {
        let json = r#"{"response": {"docs": [{}]}}"#;
        let sr: SearchResponse = serde_json::from_str(json).unwrap();
        let docs = sr.response.unwrap().docs.unwrap();
        assert_eq!(docs.len(), 1);
        assert!(docs[0].version.is_none());
    }

    #[test]
    fn test_search_response_deserialize_invalid_json() {
        let result = serde_json::from_str::<SearchResponse>("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_coord_too_many_parts() {
        let (g, a) = parse_coord("g:a:extra").unwrap();
        assert_eq!(g, "g");
        assert_eq!(a, "a");
    }

    #[test]
    fn test_parse_pom_malformed_xml() {
        let pom = "<not valid xml>";
        let result = parse_pom_dependencies(pom).unwrap();
        assert!(result.is_empty());
    }

}
