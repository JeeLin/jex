//! 依赖安全检查（jex audit）
//! - audit: 检查项目依赖是否有已知安全漏洞

use crate::deps;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// 严重程度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

impl Severity {
    pub fn color(&self) -> &'static str {
        match self {
            Severity::Critical | Severity::High => "\x1b[31m",  // 红色
            Severity::Medium => "\x1b[33m",  // 黄色
            Severity::Low => "\x1b[32m",     // 绿色
        }
    }

    pub fn display(&self) -> &'static str {
        match self {
            Severity::Critical => "CRITICAL",
            Severity::High => "HIGH",
            Severity::Medium => "MEDIUM",
            Severity::Low => "LOW",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}

/// 漏洞信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
    pub group: String,
    pub artifact: String,
    pub current_version: String,
    pub cve_id: String,
    pub severity: Severity,
    pub description: String,
    pub fixed_version: Option<String>,
}

/// 检查漏洞
pub fn check_vulnerabilities() -> Result<Vec<Vulnerability>> {
    let lock = deps::read_jex_lock()?;
    let dependencies = lock.dependencies.unwrap_or_default();
    let mut vulnerabilities = Vec::new();

    for (coord, current_version) in &dependencies {
        let parts: Vec<&str> = coord.split(':').collect();
        if parts.len() < 2 {
            continue;
        }
        let group = parts[0];
        let artifact = parts[1];

        // 查询 OSV 数据库
        match query_osv(group, artifact, current_version) {
            Ok(vulns) => {
                vulnerabilities.extend(vulns);
            }
            Err(e) => {
                eprintln!("⚠️  查询 {} 漏洞信息失败: {}", coord, e);
            }
        }
    }

    // 按严重程度排序
    vulnerabilities.sort_by(|a, b| b.severity.cmp(&a.severity));

    Ok(vulnerabilities)
}

/// 查询 OSV 数据库
fn query_osv(group: &str, artifact: &str, version: &str) -> Result<Vec<Vulnerability>> {
    let client = reqwest::blocking::Client::new();
    let query = format!(
        r#"{{"package":{{"name":"{}","ecosystem":"Maven","namespace":"{}"}},"version":"{}"}}"#,
        artifact, group, version
    );

    let response = client
        .post("https://api.osv.dev/v1/query")
        .header("Content-Type", "application/json")
        .body(query)
        .send()
        .map_err(|e| crate::error::Error::new(format!("HTTP 请求失败: {}", e)))?;

    if !response.status().is_success() {
        return Ok(Vec::new());
    }

    let body: serde_json::Value = response
        .json()
        .map_err(|e| crate::error::Error::new(format!("JSON 解析失败: {}", e)))?;
    let vulns = body["vulns"].as_array().cloned().unwrap_or_default();

    let mut result = Vec::new();
    for vuln in vulns {
        let cve_id = vuln["id"].as_str().unwrap_or("unknown").to_string();
        let summary = vuln["summary"].as_str().unwrap_or("No description").to_string();

        // 提取严重程度
        let severity = extract_severity(&vuln);

        // 提取修复版本
        let fixed_version = extract_fixed_version(&vuln, version);

        result.push(Vulnerability {
            group: group.to_string(),
            artifact: artifact.to_string(),
            current_version: version.to_string(),
            cve_id,
            severity,
            description: summary,
            fixed_version,
        });
    }

    Ok(result)
}

/// 提取严重程度
fn extract_severity(vuln: &serde_json::Value) -> Severity {
    let severity_str = vuln["severity"][0]["score"]
        .as_str()
        .unwrap_or("MEDIUM");

    match severity_str.to_uppercase().as_str() {
        "CRITICAL" => Severity::Critical,
        "HIGH" => Severity::High,
        "MEDIUM" => Severity::Medium,
        "LOW" => Severity::Low,
        _ => Severity::Medium,
    }
}

/// 提取修复版本
fn extract_fixed_version(vuln: &serde_json::Value, current_version: &str) -> Option<String> {
    let versions = vuln["affected"][0]["versions"].as_array()?;

    for version_info in versions {
        let fixed = version_info["fixed"].as_str();
        if let Some(fixed_version) = fixed {
            // 确保修复版本比当前版本高
            if fixed_version > current_version {
                return Some(fixed_version.to_string());
            }
        }
    }

    None
}

/// 获取修复建议
pub fn get_fix_suggestion(vuln: &Vulnerability) -> Option<String> {
    vuln.fixed_version.as_ref().map(|fixed| {
        format!(
            "升级 {}:{} 从 {} 到 {}",
            vuln.group, vuln.artifact, vuln.current_version, fixed
        )
    })
}

/// 漏洞报告
#[derive(Debug, Serialize)]
pub struct AuditReport {
    pub total_vulnerabilities: usize,
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub vulnerabilities: Vec<Vulnerability>,
}

/// 生成审计报告
pub fn generate_report(vulnerabilities: Vec<Vulnerability>) -> AuditReport {
    let critical = vulnerabilities.iter().filter(|v| v.severity == Severity::Critical).count();
    let high = vulnerabilities.iter().filter(|v| v.severity == Severity::High).count();
    let medium = vulnerabilities.iter().filter(|v| v.severity == Severity::Medium).count();
    let low = vulnerabilities.iter().filter(|v| v.severity == Severity::Low).count();

    AuditReport {
        total_vulnerabilities: vulnerabilities.len(),
        critical,
        high,
        medium,
        low,
        vulnerabilities,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Medium > Severity::Low);
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(Severity::Critical.display(), "CRITICAL");
        assert_eq!(Severity::High.display(), "HIGH");
        assert_eq!(Severity::Medium.display(), "MEDIUM");
        assert_eq!(Severity::Low.display(), "LOW");
    }

    #[test]
    fn test_get_fix_suggestion() {
        let vuln = Vulnerability {
            group: "com.google.code.gson".to_string(),
            artifact: "gson".to_string(),
            current_version: "2.10.0".to_string(),
            cve_id: "CVE-2022-25647".to_string(),
            severity: Severity::High,
            description: "Deserialization of Untrusted Data".to_string(),
            fixed_version: Some("2.11.0".to_string()),
        };

        let suggestion = get_fix_suggestion(&vuln);
        assert!(suggestion.is_some());
        assert!(suggestion.unwrap().contains("2.11.0"));
    }

    #[test]
    fn test_generate_report() {
        let vulns = vec![
            Vulnerability {
                group: "test".to_string(),
                artifact: "a".to_string(),
                current_version: "1.0".to_string(),
                cve_id: "CVE-001".to_string(),
                severity: Severity::High,
                description: "test".to_string(),
                fixed_version: None,
            },
            Vulnerability {
                group: "test".to_string(),
                artifact: "b".to_string(),
                current_version: "1.0".to_string(),
                cve_id: "CVE-002".to_string(),
                severity: Severity::Low,
                description: "test".to_string(),
                fixed_version: None,
            },
        ];

        let report = generate_report(vulns);
        assert_eq!(report.total_vulnerabilities, 2);
        assert_eq!(report.high, 1);
        assert_eq!(report.low, 1);
    }
}
