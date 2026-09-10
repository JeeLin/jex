//! 项目依赖分析报告（jex report）
//! - report: 生成项目依赖综合分析报告

use crate::audit;
use crate::error::Result;
use crate::license;
use crate::outdated;
use crate::tree;
use serde::{Deserialize, Serialize};

/// 报告摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total_dependencies: usize,
    pub outdated_count: usize,
    pub vulnerability_count: usize,
    pub license_issues: usize,
}

/// 依赖分析报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyReport {
    pub summary: ReportSummary,
    pub outdated: Vec<outdated::OutdatedDep>,
    pub vulnerabilities: Vec<audit::Vulnerability>,
    pub licenses: Vec<license::LicenseInfo>,
    pub tree: tree::DependencyTree,
}

/// 生成报告
pub fn generate_report() -> Result<DependencyReport> {
    let outdated = outdated::check_outdated().unwrap_or_default();
    let vulnerabilities = audit::check_vulnerabilities().unwrap_or_default();
    let licenses = license::check_licenses().unwrap_or_default();
    let tree = tree::build_dependency_tree().unwrap_or_else(|_| tree::DependencyTree {
        root: tree::DependencyNode {
            name: "root".to_string(),
            version: "".to_string(),
            license: None,
            children: Vec::new(),
        },
    });

    let summary = ReportSummary {
        total_dependencies: tree.root.children.len(),
        outdated_count: outdated.len(),
        vulnerability_count: vulnerabilities.len(),
        license_issues: licenses
            .iter()
            .filter(|l| l.category == license::LicenseCategory::Unknown)
            .count(),
    };

    Ok(DependencyReport {
        summary,
        outdated,
        vulnerabilities,
        licenses,
        tree,
    })
}

/// 渲染报告
pub fn render_report(report: &DependencyReport) -> String {
    let mut output = String::new();

    output.push_str("📊 项目依赖分析报告\n");
    output.push_str("═══════════════════════════════════════\n\n");

    // 摘要
    output.push_str("📋 摘要\n");
    output.push_str(&format!(
        "  总依赖数: {}\n",
        report.summary.total_dependencies
    ));
    output.push_str(&format!("  可更新: {}\n", report.summary.outdated_count));
    output.push_str(&format!(
        "  安全漏洞: {}\n",
        report.summary.vulnerability_count
    ));
    output.push_str(&format!(
        "  许可证问题: {}\n\n",
        report.summary.license_issues
    ));

    // 可更新依赖
    if !report.outdated.is_empty() {
        output.push_str("📦 可更新依赖\n");
        output.push_str("───────────────────────────────────────\n");
        for dep in &report.outdated {
            output.push_str(&format!(
                "  {}:{}: {} → {}\n",
                dep.group, dep.artifact, dep.current, dep.latest
            ));
        }
        output.push('\n');
    }

    // 安全漏洞
    if !report.vulnerabilities.is_empty() {
        output.push_str("🔒 安全漏洞\n");
        output.push_str("───────────────────────────────────────\n");
        for vuln in &report.vulnerabilities {
            output.push_str(&format!(
                "  [{}] {}:{} ({})\n",
                vuln.severity, vuln.group, vuln.artifact, vuln.cve_id
            ));
        }
        output.push('\n');
    }

    // 许可证信息
    let mut license_counts = std::collections::HashMap::new();
    for license in &report.licenses {
        *license_counts.entry(license.spdx_id.clone()).or_insert(0) += 1;
    }

    if !license_counts.is_empty() {
        output.push_str("📜 许可证分布\n");
        output.push_str("───────────────────────────────────────\n");
        for (license, count) in &license_counts {
            output.push_str(&format!("  {}: {} 个\n", license, count));
        }
        output.push('\n');
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_summary() {
        let summary = ReportSummary {
            total_dependencies: 10,
            outdated_count: 2,
            vulnerability_count: 1,
            license_issues: 0,
        };
        assert_eq!(summary.total_dependencies, 10);
        assert_eq!(summary.outdated_count, 2);
        assert_eq!(summary.vulnerability_count, 1);
        assert_eq!(summary.license_issues, 0);
    }

    #[test]
    fn test_report_summary_zero() {
        let summary = ReportSummary {
            total_dependencies: 0,
            outdated_count: 0,
            vulnerability_count: 0,
            license_issues: 0,
        };
        assert_eq!(summary.total_dependencies, 0);
        assert_eq!(summary.outdated_count, 0);
    }

    #[test]
    fn test_report_summary_serialize() {
        let summary = ReportSummary {
            total_dependencies: 5,
            outdated_count: 2,
            vulnerability_count: 1,
            license_issues: 3,
        };
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("total_dependencies"));
        let deserialized: ReportSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total_dependencies, 5);
        assert_eq!(deserialized.license_issues, 3);
    }

    #[test]
    fn test_dependency_report_serialize() {
        let report = DependencyReport {
            summary: ReportSummary {
                total_dependencies: 1,
                outdated_count: 0,
                vulnerability_count: 0,
                license_issues: 0,
            },
            outdated: Vec::new(),
            vulnerabilities: Vec::new(),
            licenses: Vec::new(),
            tree: tree::DependencyTree {
                root: tree::DependencyNode {
                    name: "root".to_string(),
                    version: "".to_string(),
                    license: None,
                    children: Vec::new(),
                },
            },
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("summary"));
        let deserialized: DependencyReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.summary.total_dependencies, 1);
    }

    #[test]
    fn test_render_report_empty() {
        let report = DependencyReport {
            summary: ReportSummary {
                total_dependencies: 0,
                outdated_count: 0,
                vulnerability_count: 0,
                license_issues: 0,
            },
            outdated: Vec::new(),
            vulnerabilities: Vec::new(),
            licenses: Vec::new(),
            tree: tree::DependencyTree {
                root: tree::DependencyNode {
                    name: "root".to_string(),
                    version: "".to_string(),
                    license: None,
                    children: Vec::new(),
                },
            },
        };
        let output = render_report(&report);
        assert!(output.contains("项目依赖分析报告"));
        assert!(output.contains("总依赖数: 0"));
        assert!(output.contains("可更新: 0"));
        assert!(output.contains("安全漏洞: 0"));
        assert!(output.contains("许可证问题: 0"));
    }

    #[test]
    fn test_render_report_with_outdated() {
        let report = DependencyReport {
            summary: ReportSummary {
                total_dependencies: 2,
                outdated_count: 1,
                vulnerability_count: 0,
                license_issues: 0,
            },
            outdated: vec![outdated::OutdatedDep {
                group: "com.google.code.gson".to_string(),
                artifact: "gson".to_string(),
                current: "2.10.0".to_string(),
                latest: "2.11.0".to_string(),
            }],
            vulnerabilities: Vec::new(),
            licenses: Vec::new(),
            tree: tree::DependencyTree {
                root: tree::DependencyNode {
                    name: "root".to_string(),
                    version: "".to_string(),
                    license: None,
                    children: Vec::new(),
                },
            },
        };
        let output = render_report(&report);
        assert!(output.contains("可更新依赖"));
        assert!(output.contains("gson"));
        assert!(output.contains("2.10.0"));
        assert!(output.contains("2.11.0"));
    }

    #[test]
    fn test_render_report_with_vulns() {
        let report = DependencyReport {
            summary: ReportSummary {
                total_dependencies: 1,
                outdated_count: 0,
                vulnerability_count: 1,
                license_issues: 0,
            },
            outdated: Vec::new(),
            vulnerabilities: vec![audit::Vulnerability {
                group: "org.apache.logging.log4j".to_string(),
                artifact: "log4j-core".to_string(),
                current_version: "2.14.0".to_string(),
                cve_id: "CVE-2021-44228".to_string(),
                severity: audit::Severity::Critical,
                description: "Log4Shell".to_string(),
                fixed_version: None,
            }],
            licenses: Vec::new(),
            tree: tree::DependencyTree {
                root: tree::DependencyNode {
                    name: "root".to_string(),
                    version: "".to_string(),
                    license: None,
                    children: Vec::new(),
                },
            },
        };
        let output = render_report(&report);
        assert!(output.contains("安全漏洞"));
        assert!(output.contains("log4j-core"));
        assert!(output.contains("CVE-2021-44228"));
    }

    #[test]
    fn test_render_report_with_licenses() {
        let report = DependencyReport {
            summary: ReportSummary {
                total_dependencies: 1,
                outdated_count: 0,
                vulnerability_count: 0,
                license_issues: 0,
            },
            outdated: Vec::new(),
            vulnerabilities: Vec::new(),
            licenses: vec![license::LicenseInfo {
                group: "com.google.code.gson".to_string(),
                artifact: "gson".to_string(),
                version: "2.11.0".to_string(),
                license: "Apache-2.0".to_string(),
                spdx_id: "Apache-2.0".to_string(),
                category: license::LicenseCategory::Permissive,
            }],
            tree: tree::DependencyTree {
                root: tree::DependencyNode {
                    name: "root".to_string(),
                    version: "".to_string(),
                    license: None,
                    children: Vec::new(),
                },
            },
        };
        let output = render_report(&report);
        assert!(output.contains("许可证分布"));
        assert!(output.contains("Apache-2.0"));
    }

    #[test]
    fn test_render_report_with_all_sections() {
        let report = DependencyReport {
            summary: ReportSummary {
                total_dependencies: 3,
                outdated_count: 1,
                vulnerability_count: 1,
                license_issues: 1,
            },
            outdated: vec![outdated::OutdatedDep {
                group: "a".to_string(),
                artifact: "b".to_string(),
                current: "1.0".to_string(),
                latest: "2.0".to_string(),
            }],
            vulnerabilities: vec![audit::Vulnerability {
                group: "c".to_string(),
                artifact: "d".to_string(),
                current_version: "1.0".to_string(),
                cve_id: "CVE-1".to_string(),
                severity: audit::Severity::High,
                description: "test".to_string(),
                fixed_version: None,
            }],
            licenses: vec![license::LicenseInfo {
                group: "e".to_string(),
                artifact: "f".to_string(),
                version: "1.0".to_string(),
                license: "MIT".to_string(),
                spdx_id: "MIT".to_string(),
                category: license::LicenseCategory::Permissive,
            }],
            tree: tree::DependencyTree {
                root: tree::DependencyNode {
                    name: "root".to_string(),
                    version: "".to_string(),
                    license: None,
                    children: Vec::new(),
                },
            },
        };
        let output = render_report(&report);
        assert!(output.contains("总依赖数: 3"));
        assert!(output.contains("可更新: 1"));
        assert!(output.contains("安全漏洞: 1"));
        assert!(output.contains("许可证问题: 1"));
    }
}
