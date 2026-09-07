//! 依赖许可证自动检查（jex license --check）
//! - license_check: 添加依赖时自动检查许可证合规性

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 许可证检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseCheckResult {
    pub spdx_id: String,
    pub is_compatible: bool,
    pub issues: Vec<String>,
}

/// 许可证报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseReport {
    pub compatible: Vec<String>,
    pub incompatible: Vec<String>,
    pub unknown: Vec<String>,
}

/// 已知的宽松许可证
const PERMISSIVE_LICENSES: &[&str] = &[
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Unlicense",
    "0BSD",
    "Zlib",
];

/// 已知的弱 copyleft 许可证
const WEAK_COPYLEFT_LICENSES: &[&str] = &["LGPL-2.1", "LGPL-3.0", "MPL-2.0", "EPL-1.0", "EPL-2.0"];

/// 检查依赖许可证
pub fn check_dependency_license(coord: &str) -> Result<LicenseCheckResult> {
    // 从 Maven Central 查询 POM 获取许可证信息
    let spdx_id = query_maven_pom(coord).unwrap_or_else(|| "Unknown".to_string());

    let is_compatible = is_license_compatible(&spdx_id);
    let mut issues = Vec::new();

    if spdx_id == "Unknown" {
        issues.push("无法获取许可证信息".to_string());
    } else if !is_compatible {
        issues.push(format!("许可证 {} 可能与项目不兼容", spdx_id));
    }

    Ok(LicenseCheckResult {
        spdx_id,
        is_compatible,
        issues,
    })
}

/// 查询 Maven Central POM 获取许可证
fn query_maven_pom(coord: &str) -> Option<String> {
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() != 2 {
        return None;
    }

    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let versions = vec!["1.0.0", "2.0.0", "3.0.0"];

    for version in versions {
        let url = format!(
            "https://repo1.maven.org/maven2/{}/{}/{}/{}.pom",
            group, artifact, version, artifact
        );

        if let Ok(response) = reqwest::blocking::get(&url) {
            if response.status().is_success() {
                if let Ok(pom_content) = response.text() {
                    return extract_license_from_pom(&pom_content);
                }
            }
        }
    }

    None
}

/// 从 POM 内容提取许可证
fn extract_license_from_pom(pom_content: &str) -> Option<String> {
    // 简化的 XML 解析，实际应使用 proper XML 解析器
    if pom_content.contains("<name>MIT</name>") {
        Some("MIT".to_string())
    } else if pom_content.contains("<name>Apache License, Version 2.0</name>") {
        Some("Apache-2.0".to_string())
    } else if pom_content.contains("<name>BSD License</name>") {
        Some("BSD-3-Clause".to_string())
    } else if pom_content.contains("<name>ISC License</name>") {
        Some("ISC".to_string())
    } else if pom_content.contains("<name>The Unlicense</name>") {
        Some("Unlicense".to_string())
    } else {
        // 尝试提取 SPDX 标识符
        let start_tag = "<url>";
        let end_tag = "</url>";
        if let (Some(start), Some(end)) = (
            pom_content.find(start_tag),
            pom_content.find(end_tag),
        ) {
            let url = &pom_content[start + start_tag.len()..end];
            if url.contains("spdx.org") {
                // 提取 SPDX 标识符
                if let Some(pos) = url.rfind('/') {
                    return Some(url[pos + 1..].to_string());
                }
            }
        }
        None
    }
}

/// 检查许可证是否兼容
fn is_license_compatible(spdx_id: &str) -> bool {
    if spdx_id == "Unknown" {
        return false;
    }

    let compatible: HashSet<&str> = PERMISSIVE_LICENSES
        .iter()
        .chain(WEAK_COPYLEFT_LICENSES.iter())
        .copied()
        .collect();

    compatible.contains(spdx_id)
}

/// 检查所有依赖的许可证
pub fn check_all_licenses() -> Result<LicenseReport> {
    let lock = crate::deps::read_jex_lock()?;
    let dependencies = lock.dependencies.unwrap_or_default();

    let mut compatible = Vec::new();
    let mut incompatible = Vec::new();
    let mut unknown = Vec::new();

    for coord in dependencies.keys() {
        let result = check_dependency_license(coord)?;

        if result.spdx_id == "Unknown" {
            unknown.push(coord.clone());
        } else if result.is_compatible {
            compatible.push(coord.clone());
        } else {
            incompatible.push(coord.clone());
        }
    }

    Ok(LicenseReport {
        compatible,
        incompatible,
        unknown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_license_check_result() {
        let result = LicenseCheckResult {
            spdx_id: "MIT".to_string(),
            is_compatible: true,
            issues: Vec::new(),
        };

        assert_eq!(result.spdx_id, "MIT");
        assert!(result.is_compatible);
    }

    #[test]
    fn test_is_license_compatible() {
        assert!(is_license_compatible("MIT"));
        assert!(is_license_compatible("Apache-2.0"));
        assert!(is_license_compatible("BSD-3-Clause"));
        assert!(!is_license_compatible("GPL-2.0"));
        assert!(!is_license_compatible("GPL-3.0"));
        assert!(!is_license_compatible("Unknown"));
    }
    #[test]
    fn test_license_report_struct() {
        let report = LicenseReport {
            compatible: vec!["MIT".to_string()],
            incompatible: vec!["GPL-2.0".to_string()],
            unknown: vec!["Custom".to_string()],
        };
        assert_eq!(report.compatible.len(), 1);
        assert_eq!(report.incompatible.len(), 1);
        assert_eq!(report.unknown.len(), 1);
    }

    #[test]
    fn test_extract_license_from_pom_mit() {
        let pom = r#"<license><name>MIT</name></license>"#;
        assert_eq!(extract_license_from_pom(pom), Some("MIT".to_string()));
    }

    #[test]
    fn test_extract_license_from_pom_apache() {
        let pom = r#"<license><name>Apache License, Version 2.0</name></license>"#;
        assert_eq!(extract_license_from_pom(pom), Some("Apache-2.0".to_string()));
    }

    #[test]
    fn test_extract_license_from_pom_bsd() {
        let pom = r#"<license><name>BSD License</name></license>"#;
        assert_eq!(extract_license_from_pom(pom), Some("BSD-3-Clause".to_string()));
    }

    #[test]
    fn test_extract_license_from_pom_isc() {
        let pom = r#"<license><name>ISC License</name></license>"#;
        assert_eq!(extract_license_from_pom(pom), Some("ISC".to_string()));
    }

    #[test]
    fn test_extract_license_from_pom_unlicense() {
        let pom = r#"<license><name>The Unlicense</name></license>"#;
        assert_eq!(extract_license_from_pom(pom), Some("Unlicense".to_string()));
    }

    #[test]
    fn test_extract_license_from_pom_spdx_url() {
        let pom = r#"<url>https://spdx.org/licenses/MIT.html</url>"#;
        assert_eq!(extract_license_from_pom(pom), Some("MIT.html".to_string()));
    }

    #[test]
    fn test_extract_license_from_pom_spdx_url_no_slash() {
        let pom = r#"<url>https://example.com</url>"#;
        assert_eq!(extract_license_from_pom(pom), None);
    }

    #[test]
    fn test_extract_license_from_pom_no_license() {
        let pom = r#"<project></project>"#;
        assert_eq!(extract_license_from_pom(pom), None);
    }

    #[test]
    fn test_is_license_compatible_weak_copyleft() {
        assert!(is_license_compatible("LGPL-2.1"));
        assert!(is_license_compatible("LGPL-3.0"));
        assert!(is_license_compatible("MPL-2.0"));
        assert!(is_license_compatible("EPL-1.0"));
        assert!(is_license_compatible("EPL-2.0"));
    }

    #[test]
    fn test_is_license_compatible_permissive() {
        assert!(is_license_compatible("MIT"));
        assert!(is_license_compatible("Apache-2.0"));
        assert!(is_license_compatible("BSD-2-Clause"));
        assert!(is_license_compatible("BSD-3-Clause"));
        assert!(is_license_compatible("ISC"));
        assert!(is_license_compatible("Unlicense"));
        assert!(is_license_compatible("0BSD"));
        assert!(is_license_compatible("Zlib"));
    }

    #[test]
    fn test_is_license_compatible_incompatible() {
        assert!(!is_license_compatible("GPL-2.0"));
        assert!(!is_license_compatible("GPL-3.0"));
        assert!(!is_license_compatible("AGPL-3.0"));
        assert!(!is_license_compatible("SSPL-1.0"));
        assert!(!is_license_compatible(""));
    }

    #[test]
    fn test_check_dependency_license_unknown_coord() {
        // A coord with too many parts should return None from query_maven_pom
        let result = check_dependency_license("invalid:too:many:parts").unwrap();
        assert_eq!(result.spdx_id, "Unknown");
        assert!(!result.is_compatible);
        assert!(!result.issues.is_empty());
    }

    #[test]
    fn test_check_dependency_license_bad_coord() {
        let result = check_dependency_license("single-part").unwrap();
        assert_eq!(result.spdx_id, "Unknown");
        assert!(!result.is_compatible);
    }

    #[test]
    fn test_license_check_result_serde() {
        let result = LicenseCheckResult {
            spdx_id: "Apache-2.0".to_string(),
            is_compatible: true,
            issues: vec!["note".to_string()],
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: LicenseCheckResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.spdx_id, "Apache-2.0");
        assert!(deserialized.is_compatible);
    }

    #[test]
    fn test_license_report_serde() {
        let report = LicenseReport {
            compatible: vec![],
            incompatible: vec![],
            unknown: vec![],
        };
        let json = serde_json::to_string(&report).unwrap();
        let deserialized: LicenseReport = serde_json::from_str(&json).unwrap();
        assert!(deserialized.compatible.is_empty());
    }
}
