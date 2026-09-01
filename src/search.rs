//! 依赖搜索（Maven Central Solr API）
//! - search: 查询 Maven Central，返回 groupId:artifactId + 最新版本 + 描述
//! - versions: 列出某 artifact 的全部可用版本

use crate::error::{Error, Result};
use crate::util::parse_coord;
use serde::Deserialize;

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

/// 搜索 Maven Central
pub fn search(keyword: &str, limit: usize) -> Result<()> {
    let url = format!(
        "https://search.maven.org/solrsearch/select?q={}&rows={}&wt=json",
        keyword, limit
    );

    let output = std::process::Command::new("curl")
        .args(["-s", "-L", &url])
        .output()?;

    if !output.status.success() {
        return Err(Error::new("请求 Maven Central 失败"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: SearchResponse = serde_json::from_str(&stdout)?;

    let docs = match response.response {
        Some(r) => r.docs.unwrap_or_default(),
        None => Vec::new(),
    };

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

    let output = std::process::Command::new("curl")
        .args(["-s", "-L", &url])
        .output()?;

    if !output.status.success() {
        return Err(Error::new("请求 Maven Central 失败"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: SearchResponse = serde_json::from_str(&stdout)?;

    let docs = match response.response {
        Some(r) => r.docs.unwrap_or_default(),
        None => Vec::new(),
    };

    if docs.is_empty() {
        println!("未找到版本: {}", coord);
        return Ok(());
    }

    println!("{} 的版本:", coord);

    for doc in &docs {
        if let Some(version) = &doc.version_count {
            println!(
                "  {} (共 {} 个版本)",
                doc.latest_version.as_deref().unwrap_or("?"),
                version
            );
        } else {
            println!("  {}", doc.latest_version.as_deref().unwrap_or("?"));
        }
    }

    Ok(())
}
