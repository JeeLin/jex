//! 依赖搜索（Maven Central Solr API）
//! - search: 查询 Maven Central，返回 groupId:artifactId + 最新版本 + 描述
//! - versions: 列出某 artifact 的全部可用版本

use crate::error::{Error, Result};
use crate::util::parse_coord;
use serde::Deserialize;
use std::process::Command;

/// Maven Central 搜索结果
#[derive(Debug, Deserialize)]
struct SearchResponse {
    response: Option<SearchResult>,
}

/// 搜索结果详情
#[derive(Debug, Deserialize)]
struct SearchResult {
    docs: Option<Vec<SearchDoc>>,
}

/// 单个搜索文档
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SearchDoc {
    #[serde(rename = "g")]
    group_id: Option<String>,
    #[serde(rename = "a")]
    artifact_id: Option<String>,
    #[serde(rename = "latestVersion")]
    latest_version: Option<String>,
    #[serde(rename = "versionCount")]
    version_count: Option<u32>,
    #[serde(rename = "description")]
    description: Option<String>,
    #[serde(rename = "ec")]
    extensions: Option<Vec<String>>,
}

/// 通过 curl 拉取 Maven Central Solr URL，解析为 SearchDoc 列表。
/// 提取此函数消除 search() / versions() 中相同的 curl + 反序列化逻辑（F10 修复）。
fn fetch_docs(url: &str) -> Result<Vec<SearchDoc>> {
    let output = Command::new("curl").args(["-s", "-L", url]).output()?;
    if !output.status.success() {
        return Err(Error::new("请求 Maven Central 失败"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: SearchResponse = serde_json::from_str(&stdout)?;
    Ok(response.response.and_then(|r| r.docs).unwrap_or_default())
}

/// 搜索 Maven Central
pub fn search(keyword: &str, limit: usize) -> Result<()> {
    let url = format!(
        "https://search.maven.org/solrsearch/select?q={}&rows={}&wt=json",
        keyword, limit
    );
    let docs = fetch_docs(&url)?;
    if docs.is_empty() {
        println!("未找到结果: {}", keyword);
        return Ok(());
    }
    println!("搜索结果: \"{}\"", keyword);
    println!();
    for doc in &docs {
        let group = doc.group_id.as_deref().unwrap_or("?");
        let artifact = doc.artifact_id.as_deref().unwrap_or("?");
        let version = doc.latest_version.as_deref().unwrap_or("?");
        let desc = doc.description.as_deref().unwrap_or("");
        println!("  {}:{} ({})", group, artifact, version);
        if !desc.is_empty() {
            println!("    {}", desc);
        }
    }
    Ok(())
}

/// 列出某 artifact 的全部可用版本
pub fn versions(coord: &str) -> Result<()> {
    let (group, artifact) = parse_coord(coord)?;
    let url = format!(
        "https://search.maven.org/solrsearch/select?q=g:{}+AND+a:{}&core=gav&rows=100&wt=json",
        group, artifact
    );
    let docs = fetch_docs(&url)?;
    if docs.is_empty() {
        println!("未找到版本: {}", coord);
        return Ok(());
    }
    println!("{} 的版本:", coord);
    for doc in &docs {
        if let Some(count) = &doc.version_count {
            println!(
                "  {} (共 {} 个版本)",
                doc.latest_version.as_deref().unwrap_or("?"),
                count
            );
        } else {
            println!("  {}", doc.latest_version.as_deref().unwrap_or("?"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_response_deserialize() {
        let json = r#"{
            "response": {
                "docs": [
                    {
                        "g": "com.google.code.gson",
                        "a": "gson",
                        "latestVersion": "2.11.0",
                        "versionCount": 30,
                        "description": "Gson library",
                        "ec": ["jar", "sources"]
                    }
                ]
            }
        }"#;
        let response: SearchResponse = serde_json::from_str(json).unwrap();
        let docs = response.response.unwrap().docs.unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].group_id.as_deref(), Some("com.google.code.gson"));
        assert_eq!(docs[0].artifact_id.as_deref(), Some("gson"));
        assert_eq!(docs[0].latest_version.as_deref(), Some("2.11.0"));
        assert_eq!(docs[0].version_count, Some(30));
        assert_eq!(docs[0].description.as_deref(), Some("Gson library"));
    }

    #[test]
    fn test_search_response_empty() {
        let json = r#"{"response": {"docs": []}}"#;
        let response: SearchResponse = serde_json::from_str(json).unwrap();
        let docs = response.response.unwrap().docs.unwrap();
        assert!(docs.is_empty());
    }

    #[test]
    fn test_search_response_no_response() {
        let json = r#"{}"#;
        let response: SearchResponse = serde_json::from_str(json).unwrap();
        assert!(response.response.is_none());
    }

    #[test]
    fn test_search_doc_optional_fields() {
        let json = r#"{
            "response": {
                "docs": [
                    {
                        "g": "org.example",
                        "a": "lib"
                    }
                ]
            }
        }"#;
        let response: SearchResponse = serde_json::from_str(json).unwrap();
        let doc = &response.response.unwrap().docs.unwrap()[0];
        assert_eq!(doc.group_id.as_deref(), Some("org.example"));
        assert_eq!(doc.artifact_id.as_deref(), Some("lib"));
        assert!(doc.latest_version.is_none());
        assert!(doc.version_count.is_none());
        assert!(doc.description.is_none());
        assert!(doc.extensions.is_none());
    }

    #[test]
    fn test_search_invalid_json() {
        let result = serde_json::from_str::<SearchResponse>("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_fetch_docs_url_format() {
        let keyword = "gson";
        let limit = 10;
        let url = format!(
            "https://search.maven.org/solrsearch/select?q={}&rows={}&wt=json",
            keyword, limit
        );
        assert!(url.contains("gson"));
        assert!(url.contains("rows=10"));
        assert!(url.contains("wt=json"));
    }
}
