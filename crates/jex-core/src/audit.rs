//! 依赖安全检查（jex audit）
//! - audit: 检查项目依赖是否有已知安全漏洞

use crate::deps;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// 严重程度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl PartialOrd for Severity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Severity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority().cmp(&other.priority())
    }
}

impl Severity {
    fn priority(&self) -> u8 {
        match self {
            Severity::Low => 0,
            Severity::Medium => 1,
            Severity::High => 2,
            Severity::Critical => 3,
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            Severity::Critical | Severity::High => "\x1b[31m", // 红色
            Severity::Medium => "\x1b[33m",                    // 黄色
            Severity::Low => "\x1b[32m",                       // 绿色
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
        let summary = vuln["summary"]
            .as_str()
            .unwrap_or("No description")
            .to_string();

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
    let severity_str = vuln["severity"][0]["score"].as_str().unwrap_or("MEDIUM");

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
    let critical = vulnerabilities
        .iter()
        .filter(|v| v.severity == Severity::Critical)
        .count();
    let high = vulnerabilities
        .iter()
        .filter(|v| v.severity == Severity::High)
        .count();
    let medium = vulnerabilities
        .iter()
        .filter(|v| v.severity == Severity::Medium)
        .count();
    let low = vulnerabilities
        .iter()
        .filter(|v| v.severity == Severity::Low)
        .count();

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
        assert!(Severity::Low < Severity::Critical);
        assert!(Severity::Medium == Severity::Medium);
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(Severity::Critical.display(), "CRITICAL");
        assert_eq!(Severity::High.display(), "HIGH");
        assert_eq!(Severity::Medium.display(), "MEDIUM");
        assert_eq!(Severity::Low.display(), "LOW");
    }

    #[test]
    fn test_severity_fmt_trait() {
        assert_eq!(format!("{}", Severity::Critical), "CRITICAL");
        assert_eq!(format!("{}", Severity::High), "HIGH");
        assert_eq!(format!("{}", Severity::Medium), "MEDIUM");
        assert_eq!(format!("{}", Severity::Low), "LOW");
    }

    #[test]
    fn test_severity_color() {
        assert_eq!(Severity::Critical.color(), "\x1b[31m");
        assert_eq!(Severity::High.color(), "\x1b[31m");
        assert_eq!(Severity::Medium.color(), "\x1b[33m");
        assert_eq!(Severity::Low.color(), "\x1b[32m");
    }

    #[test]
    fn test_severity_priority() {
        // Critical > High > Medium > Low
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Medium > Severity::Low);
    }

    #[test]
    fn test_vulnerability_serialize() {
        let vuln = Vulnerability {
            group: "com.google.code.gson".to_string(),
            artifact: "gson".to_string(),
            current_version: "2.10.0".to_string(),
            cve_id: "CVE-2022-25647".to_string(),
            severity: Severity::High,
            description: "Deserialization of Untrusted Data".to_string(),
            fixed_version: Some("2.11.0".to_string()),
        };
        let json = serde_json::to_string(&vuln).unwrap();
        assert!(json.contains("CVE-2022-25647"));
        assert!(json.contains("gson"));

        let deserialized: Vulnerability = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.cve_id, "CVE-2022-25647");
        assert_eq!(deserialized.severity, Severity::High);
    }

    #[test]
    fn test_vulnerability_no_fixed_version() {
        let vuln = Vulnerability {
            group: "org.apache.logging.log4j".to_string(),
            artifact: "log4j-core".to_string(),
            current_version: "2.14.0".to_string(),
            cve_id: "CVE-2021-44228".to_string(),
            severity: Severity::Critical,
            description: "Log4Shell".to_string(),
            fixed_version: None,
        };
        let json = serde_json::to_string(&vuln).unwrap();
        let deserialized: Vulnerability = serde_json::from_str(&json).unwrap();
        assert!(deserialized.fixed_version.is_none());
    }

    #[test]
    fn test_get_fix_suggestion() {
        let vuln = Vulnerability {
            group: "com.google.code.gson".to_string(),
            artifact: "gson".to_string(),
            current_version: "2.10.0".to_string(),
            cve_id: "CVE-2022-25647".to_string(),
            severity: Severity::High,
            description: "test".to_string(),
            fixed_version: Some("2.11.0".to_string()),
        };
        let suggestion = get_fix_suggestion(&vuln);
        assert!(suggestion.is_some());
        assert!(suggestion.unwrap().contains("2.11.0"));
    }

    #[test]
    fn test_get_fix_suggestion_none() {
        let vuln = Vulnerability {
            group: "test".to_string(),
            artifact: "test".to_string(),
            current_version: "1.0".to_string(),
            cve_id: "CVE-001".to_string(),
            severity: Severity::Low,
            description: "test".to_string(),
            fixed_version: None,
        };
        let suggestion = get_fix_suggestion(&vuln);
        assert!(suggestion.is_none());
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
        assert_eq!(report.critical, 0);
        assert_eq!(report.medium, 0);
    }

    #[test]
    fn test_generate_report_empty() {
        let report = generate_report(vec![]);
        assert_eq!(report.total_vulnerabilities, 0);
        assert_eq!(report.critical, 0);
        assert_eq!(report.high, 0);
        assert_eq!(report.medium, 0);
        assert_eq!(report.low, 0);
        assert!(report.vulnerabilities.is_empty());
    }

    #[test]
    fn test_generate_report_all_severities() {
        let vulns = vec![
            Vulnerability {
                group: "a".to_string(),
                artifact: "a".to_string(),
                current_version: "1.0".to_string(),
                cve_id: "CVE-1".to_string(),
                severity: Severity::Critical,
                description: "".to_string(),
                fixed_version: None,
            },
            Vulnerability {
                group: "b".to_string(),
                artifact: "b".to_string(),
                current_version: "1.0".to_string(),
                cve_id: "CVE-2".to_string(),
                severity: Severity::High,
                description: "".to_string(),
                fixed_version: None,
            },
            Vulnerability {
                group: "c".to_string(),
                artifact: "c".to_string(),
                current_version: "1.0".to_string(),
                cve_id: "CVE-3".to_string(),
                severity: Severity::Medium,
                description: "".to_string(),
                fixed_version: None,
            },
            Vulnerability {
                group: "d".to_string(),
                artifact: "d".to_string(),
                current_version: "1.0".to_string(),
                cve_id: "CVE-4".to_string(),
                severity: Severity::Low,
                description: "".to_string(),
                fixed_version: None,
            },
        ];

        let report = generate_report(vulns);
        assert_eq!(report.total_vulnerabilities, 4);
        assert_eq!(report.critical, 1);
        assert_eq!(report.high, 1);
        assert_eq!(report.medium, 1);
        assert_eq!(report.low, 1);
    }

    #[test]
    fn test_extract_severity_variants() {
        // CRITICAL
        let vuln = serde_json::json!({"severity": [{"score": "CRITICAL"}]});
        assert_eq!(extract_severity(&vuln), Severity::Critical);

        // HIGH
        let vuln = serde_json::json!({"severity": [{"score": "HIGH"}]});
        assert_eq!(extract_severity(&vuln), Severity::High);

        // MEDIUM
        let vuln = serde_json::json!({"severity": [{"score": "MEDIUM"}]});
        assert_eq!(extract_severity(&vuln), Severity::Medium);

        // LOW
        let vuln = serde_json::json!({"severity": [{"score": "LOW"}]});
        assert_eq!(extract_severity(&vuln), Severity::Low);

        // Unknown defaults to MEDIUM
        let vuln = serde_json::json!({"severity": [{"score": "UNKNOWN"}]});
        assert_eq!(extract_severity(&vuln), Severity::Medium);

        // No severity defaults to MEDIUM
        let vuln = serde_json::json!({});
        assert_eq!(extract_severity(&vuln), Severity::Medium);
    }

    #[test]
    fn test_extract_fixed_version() {
        // Has fixed version
        let vuln = serde_json::json!({
            "affected": [{"versions": [{"fixed": "2.11.0"}]}]
        });
        assert_eq!(
            extract_fixed_version(&vuln, "2.10.0"),
            Some("2.11.0".to_string())
        );

        // Fixed version not higher than current
        // Fixed version not higher than current (string comparison: "1.0.0" < "2.10.0")
        let vuln = serde_json::json!({
            "affected": [{"versions": [{"fixed": "1.0.0"}]}]
        });
        assert_eq!(extract_fixed_version(&vuln, "2.10.0"), None);

        // No affected array
        let vuln = serde_json::json!({});
        assert_eq!(extract_fixed_version(&vuln, "2.10.0"), None);

        // Empty versions array
        let vuln = serde_json::json!({"affected": [{"versions": []}]});
        assert_eq!(extract_fixed_version(&vuln, "2.10.0"), None);

        // No fixed field
        let vuln = serde_json::json!({"affected": [{"versions": [{}]}]});
        assert_eq!(extract_fixed_version(&vuln, "2.10.0"), None);
    }

    #[test]
    fn test_audit_report_serialize() {
        let report = AuditReport {
            total_vulnerabilities: 1,
            critical: 0,
            high: 1,
            medium: 0,
            low: 0,
            vulnerabilities: vec![Vulnerability {
                group: "test".to_string(),
                artifact: "a".to_string(),
                current_version: "1.0".to_string(),
                cve_id: "CVE-001".to_string(),
                severity: Severity::High,
                description: "test".to_string(),
                fixed_version: None,
            }],
        };
        let json = serde_json::to_string_pretty(&report).unwrap();
        assert!(json.contains("total_vulnerabilities"));
        assert!(json.contains("CVE-001"));
    }

    #[test]
    fn test_severity_clone() {
        let s = Severity::Critical;
        let s2 = s.clone();
        assert_eq!(s, s2);
    }

    #[test]
    fn test_vulnerability_clone() {
        let v = Vulnerability {
            group: "g".to_string(),
            artifact: "a".to_string(),
            current_version: "1.0".to_string(),
            cve_id: "CVE-1".to_string(),
            severity: Severity::High,
            description: "d".to_string(),
            fixed_version: Some("2.0".to_string()),
        };
        let v2 = v.clone();
        assert_eq!(v.cve_id, v2.cve_id);
        assert_eq!(v.severity, v2.severity);
    }
}
