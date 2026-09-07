//! 依赖依赖树增强（jex tree --verbose）
//! - tree_verbose: 显示依赖树详细信息（版本、许可证、漏洞状态、过滤）

use crate::audit::{self, Vulnerability};
use crate::deps;
use crate::error::Result;
use crate::license;
use serde::{Deserialize, Serialize};

/// 树报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeReport {
    pub nodes: Vec<TreeNode>,
}

/// 树节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub name: String,
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub license: String,
    pub license_category: String,
    pub vulnerabilities: Vec<VulnSummary>,
}

/// 漏洞摘要（轻量）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulnSummary {
    pub cve_id: String,
    pub severity: String,
    pub fixed_version: Option<String>,
}

/// 生成详细依赖树
pub fn tree_verbose() -> Result<TreeReport> {
    let lock = deps::read_jex_lock()?;
    let dependencies = lock.dependencies.unwrap_or_default();

    // 收集许可证信息
    let license_map = build_license_map();

    // 收集漏洞信息
    let vuln_map = build_vuln_map();

    let mut nodes = Vec::new();
    for (coord, version) in &dependencies {
        let parts: Vec<&str> = coord.split(':').collect();
        if parts.len() != 2 {
            continue;
        }
        let group = parts[0].to_string();
        let artifact = parts[1].to_string();

        // 查找许可证
        let (license_name, license_category) = license_map
            .get(coord)
            .map(|info| (info.license.clone(), info.category.display().to_string()))
            .unwrap_or_else(|| ("Unknown".to_string(), "Unknown".to_string()));

        // 查找漏洞
        let vulns: Vec<VulnSummary> = vuln_map
            .get(coord)
            .map(|vs| {
                vs.iter()
                    .map(|v| VulnSummary {
                        cve_id: v.cve_id.clone(),
                        severity: v.severity.display().to_string(),
                        fixed_version: v.fixed_version.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        nodes.push(TreeNode {
            name: format!("{}:{}", group, artifact),
            group,
            artifact,
            version: version.clone(),
            license: license_name,
            license_category,
            vulnerabilities: vulns,
        });
    }

    Ok(TreeReport { nodes })
}

/// 过滤树报告
pub fn filter_report(report: &TreeReport, filter: &str) -> TreeReport {
    let lower_filter = filter.to_lowercase();
    let filtered_nodes: Vec<TreeNode> = report
        .nodes
        .iter()
        .filter(|node| {
            node.name.to_lowercase().contains(&lower_filter)
                || node.group.to_lowercase().contains(&lower_filter)
                || node.artifact.to_lowercase().contains(&lower_filter)
                || node.version.to_lowercase().contains(&lower_filter)
                || node.license.to_lowercase().contains(&lower_filter)
                || node.license_category.to_lowercase().contains(&lower_filter)
                || node
                    .vulnerabilities
                    .iter()
                    .any(|v| v.cve_id.to_lowercase().contains(&lower_filter))
        })
        .cloned()
        .collect();

    TreeReport {
        nodes: filtered_nodes,
    }
}

/// 渲染详细依赖树（终端友好格式）
pub fn render_verbose(report: &TreeReport) -> String {
    let mut output = String::new();

    output.push_str("📦 Root\n");

    let count = report.nodes.len();
    for (i, node) in report.nodes.iter().enumerate() {
        let is_last = i == count - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let child_prefix = if is_last { "    " } else { "│   " };

        // 基本信息行
        output.push_str(&format!(
            "{}{}{} v{}\n",
            connector, node.artifact, node.version, ""
        ));

        // 许可证信息
        let category_emoji = match node.license_category.as_str() {
            "Permissive" => "🟢",
            "Weak Copyleft" => "🟡",
            "Strong Copyleft" => "🔴",
            _ => "⚪",
        };
        output.push_str(&format!(
            "{}   📄 {} {} ({})\n",
            child_prefix, category_emoji, node.license, node.license_category
        ));

        // 漏洞信息
        if node.vulnerabilities.is_empty() {
            output.push_str(&format!(
                "{}   🛡️  No known vulnerabilities\n",
                child_prefix
            ));
        } else {
            for vuln in &node.vulnerabilities {
                let sev_emoji = match vuln.severity.as_str() {
                    "CRITICAL" | "HIGH" => "🔴",
                    "MEDIUM" => "🟡",
                    _ => "🟢",
                };
                let fix_note = vuln
                    .fixed_version
                    .as_ref()
                    .map(|v| format!(" → fix: v{}", v))
                    .unwrap_or_default();
                output.push_str(&format!(
                    "{}   ⚠️  {} {} {}\n",
                    child_prefix, sev_emoji, vuln.cve_id, fix_note
                ));
            }
        }

        if !is_last {
            output.push_str(&format!("{}\n", child_prefix));
        }
    }

    // 摘要
    output.push_str(&format!(
        "\n📊 Summary: {} dependencies\n",
        report.nodes.len()
    ));

    let vuln_count: usize = report.nodes.iter().map(|n| n.vulnerabilities.len()).sum();
    output.push_str(&format!(
        "   🔍 Vulnerabilities: {}\n",
        if vuln_count > 0 {
            format!("{} found", vuln_count)
        } else {
            "None".to_string()
        }
    ));

    // 许可证分布
    let mut license_dist: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for node in &report.nodes {
        *license_dist
            .entry(node.license_category.clone())
            .or_insert(0) += 1;
    }
    if !license_dist.is_empty() {
        output.push_str("   📄 License distribution:\n");
        for (cat, count) in &license_dist {
            output.push_str(&format!("      {}: {}\n", cat, count));
        }
    }

    output
}

/// 构建许可证映射 (coord -> LicenseInfo)
fn build_license_map() -> std::collections::HashMap<String, license::LicenseInfo> {
    match license::check_licenses() {
        Ok(infos) => infos
            .into_iter()
            .map(|info| {
                let coord = format!("{}:{}", info.group, info.artifact);
                (coord, info)
            })
            .collect(),
        Err(_) => std::collections::HashMap::new(),
    }
}

/// 构建漏洞映射 (coord -> Vec<Vulnerability>)
fn build_vuln_map() -> std::collections::HashMap<String, Vec<Vulnerability>> {
    match audit::check_vulnerabilities() {
        Ok(vulns) => {
            let mut map: std::collections::HashMap<String, Vec<Vulnerability>> =
                std::collections::HashMap::new();
            for v in vulns {
                let coord = format!("{}:{}", v.group, v.artifact);
                map.entry(coord).or_default().push(v);
            }
            map
        }
        Err(_) => std::collections::HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_report_empty() {
        let report = TreeReport { nodes: vec![] };
        assert!(report.nodes.is_empty());
        let out = render_verbose(&report);
        assert!(out.contains("Root"));
        assert!(out.contains("0 dependencies"));
        assert!(out.contains("Vulnerabilities: None"));
    }

    #[test]
    fn test_tree_node_basic() {
        let node = TreeNode {
            name: "serde:serde".to_string(),
            group: "serde".to_string(),
            artifact: "serde".to_string(),
            version: "1.0.0".to_string(),
            license: "MIT".to_string(),
            license_category: "Permissive".to_string(),
            vulnerabilities: vec![],
        };
        assert_eq!(node.name, "serde:serde");
        assert_eq!(node.version, "1.0.0");
        assert_eq!(node.license, "MIT");
        assert!(node.vulnerabilities.is_empty());
    }

    #[test]
    fn test_vuln_summary() {
        let vuln = VulnSummary {
            cve_id: "CVE-2024-1234".to_string(),
            severity: "HIGH".to_string(),
            fixed_version: Some("2.0.0".to_string()),
        };
        assert_eq!(vuln.cve_id, "CVE-2024-1234");
        assert_eq!(vuln.severity, "HIGH");
        assert_eq!(vuln.fixed_version.as_deref(), Some("2.0.0"));
    }

    #[test]
    fn test_filter_report_by_name() {
        let report = TreeReport {
            nodes: vec![
                TreeNode {
                    name: "com.google.code.gson:gson".to_string(),
                    group: "com.google.code.gson".to_string(),
                    artifact: "gson".to_string(),
                    version: "2.11.0".to_string(),
                    license: "Apache-2.0".to_string(),
                    license_category: "Permissive".to_string(),
                    vulnerabilities: vec![],
                },
                TreeNode {
                    name: "org.junit.jupiter:junit-jupiter".to_string(),
                    group: "org.junit.jupiter".to_string(),
                    artifact: "junit-jupiter".to_string(),
                    version: "5.10.0".to_string(),
                    license: "EPL-2.0".to_string(),
                    license_category: "Permissive".to_string(),
                    vulnerabilities: vec![],
                },
            ],
        };

        let filtered = filter_report(&report, "gson");
        assert_eq!(filtered.nodes.len(), 1);
        assert_eq!(filtered.nodes[0].artifact, "gson");
    }

    #[test]
    fn test_filter_report_by_license() {
        let report = TreeReport {
            nodes: vec![
                TreeNode {
                    name: "a:a".to_string(),
                    group: "a".to_string(),
                    artifact: "a".to_string(),
                    version: "1.0.0".to_string(),
                    license: "MIT".to_string(),
                    license_category: "Permissive".to_string(),
                    vulnerabilities: vec![],
                },
                TreeNode {
                    name: "b:b".to_string(),
                    group: "b".to_string(),
                    artifact: "b".to_string(),
                    version: "2.0.0".to_string(),
                    license: "GPL-3.0".to_string(),
                    license_category: "Strong Copyleft".to_string(),
                    vulnerabilities: vec![],
                },
            ],
        };

        let filtered = filter_report(&report, "Permissive");
        assert_eq!(filtered.nodes.len(), 1);
        assert_eq!(filtered.nodes[0].license_category, "Permissive");
    }

    #[test]
    fn test_render_verbose_with_nodes() {
        let report = TreeReport {
            nodes: vec![TreeNode {
                name: "com.google.code.gson:gson".to_string(),
                group: "com.google.code.gson".to_string(),
                artifact: "gson".to_string(),
                version: "2.11.0".to_string(),
                license: "Apache-2.0".to_string(),
                license_category: "Permissive".to_string(),
                vulnerabilities: vec![VulnSummary {
                    cve_id: "CVE-2024-0001".to_string(),
                    severity: "MEDIUM".to_string(),
                    fixed_version: Some("2.12.0".to_string()),
                }],
            }],
        };

        let output = render_verbose(&report);
        assert!(output.contains("Root"));
        assert!(output.contains("gson"));
        assert!(output.contains("Apache-2.0"));
        assert!(output.contains("CVE-2024-0001"));
        assert!(output.contains("Permissive"));
        assert!(output.contains("1 dependencies"));
    }

    #[test]
    fn test_render_verbose_no_vulns() {
        let report = TreeReport {
            nodes: vec![TreeNode {
                name: "org.example:lib".to_string(),
                group: "org.example".to_string(),
                artifact: "lib".to_string(),
                version: "1.0.0".to_string(),
                license: "MIT".to_string(),
                license_category: "Permissive".to_string(),
                vulnerabilities: vec![],
            }],
        };

        let output = render_verbose(&report);
        assert!(output.contains("No known vulnerabilities"));
    }

    #[test]
    fn test_filter_case_insensitive() {
        let report = TreeReport {
            nodes: vec![TreeNode {
                name: "COM.GOOGLE:GSON".to_string(),
                group: "COM.GOOGLE".to_string(),
                artifact: "GSON".to_string(),
                version: "2.11.0".to_string(),
                license: "Apache-2.0".to_string(),
                license_category: "Permissive".to_string(),
                vulnerabilities: vec![],
            }],
        };

        let filtered = filter_report(&report, "gson");
        assert_eq!(filtered.nodes.len(), 1);
    }
}
