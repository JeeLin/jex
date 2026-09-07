//! 依赖安全审计增强（jex audit --fix）

use crate::error::Result;
use serde::{Deserialize, Serialize};

/// 审计修复报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditFixReport {
    pub vulnerabilities: Vec<Vulnerability>,
    pub fixes: Vec<FixSuggestion>,
}

/// 漏洞信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
    pub dependency: String,
    pub severity: String,
    pub description: String,
    pub fixed_version: Option<String>,
}

/// 修复建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixSuggestion {
    pub dependency: String,
    pub current_version: String,
    pub suggested_version: String,
    pub reason: String,
}

/// 执行审计并生成修复建议
pub fn audit_with_fix() -> Result<AuditFixReport> {
    let lock = crate::deps::read_jex_lock()?;
    let dependencies = lock.dependencies.unwrap_or_default();

    let mut vulnerabilities = Vec::new();
    let mut fixes = Vec::new();

    // 模拟审计逻辑（实际应查询安全数据库）
    for (coord, version) in &dependencies {
        // 检查是否有已知漏洞
        if let Some(vuln) = check_vulnerability(coord, version) {
            vulnerabilities.push(vuln.clone());
            
            // 生成修复建议
            if let Some(fix) = generate_fix_suggestion(coord, version, &vuln) {
                fixes.push(fix);
            }
        }
    }

    Ok(AuditFixReport {
        vulnerabilities,
        fixes,
    })
}

/// 检查依赖是否有已知漏洞
fn check_vulnerability(coord: &str, _version: &str) -> Option<Vulnerability> {
    // 模拟漏洞检查（实际应查询安全数据库）
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() == 2 {
        let artifact = parts[1];
        // 示例：检查某些常见依赖的漏洞
        if artifact == "log4j-core" || artifact == "log4j-api" {
            return Some(Vulnerability {
                dependency: coord.to_string(),
                severity: "HIGH".to_string(),
                description: "Log4j 远程代码执行漏洞".to_string(),
                fixed_version: Some("2.17.0".to_string()),
            });
        }
    }
    None
}

/// 生成修复建议
fn generate_fix_suggestion(
    coord: &str,
    current_version: &str,
    vuln: &Vulnerability,
) -> Option<FixSuggestion> {
    vuln.fixed_version.as_ref().map(|fixed_version| FixSuggestion {
        dependency: coord.to_string(),
        current_version: current_version.to_string(),
        suggested_version: fixed_version.clone(),
        reason: vuln.description.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_fix_report() {
        let report = AuditFixReport {
            vulnerabilities: vec![],
            fixes: vec![],
        };

        assert!(report.vulnerabilities.is_empty());
        assert!(report.fixes.is_empty());
    }

    #[test]
    fn test_vulnerability() {
        let vuln = Vulnerability {
            dependency: "org.apache.logging.log4j:log4j-core".to_string(),
            severity: "HIGH".to_string(),
            description: "Log4j 远程代码执行漏洞".to_string(),
            fixed_version: Some("2.17.0".to_string()),
        };

        assert_eq!(vuln.severity, "HIGH");
        assert!(vuln.fixed_version.is_some());
    }
}
