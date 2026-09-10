//! 依赖许可证检查（jex license）
//! - license: 检查项目依赖的许可证类型和合规性

use crate::deps;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 许可证分类
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LicenseCategory {
    Permissive,     // 宽松许可证（MIT, Apache-2.0, BSD）
    WeakCopyleft,   // 弱 copyleft（LGPL, MPL-2.0）
    StrongCopyleft, // 强 copyleft（GPL, AGPL）
    Unknown,        // 未知许可证
}

impl LicenseCategory {
    pub fn display(&self) -> &'static str {
        match self {
            LicenseCategory::Permissive => "Permissive",
            LicenseCategory::WeakCopyleft => "Weak Copyleft",
            LicenseCategory::StrongCopyleft => "Strong Copyleft",
            LicenseCategory::Unknown => "Unknown",
        }
    }
}

/// 许可证信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseInfo {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub license: String,
    pub spdx_id: String,
    pub category: LicenseCategory,
}

/// 合规性冲突
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceConflict {
    pub license1: String,
    pub license2: String,
    pub reason: String,
}

/// 合规性报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub compatible: bool,
    pub conflicts: Vec<ComplianceConflict>,
    pub warnings: Vec<String>,
    pub summary: LicenseSummary,
}

/// 许可证摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseSummary {
    pub total_dependencies: usize,
    pub permissive: usize,
    pub weak_copyleft: usize,
    pub strong_copyleft: usize,
    pub unknown: usize,
    pub license_counts: HashMap<String, usize>,
}

/// 检查许可证
pub fn check_licenses() -> Result<Vec<LicenseInfo>> {
    let lock = deps::read_jex_lock()?;
    let dependencies = lock.dependencies.unwrap_or_default();
    let mut licenses = Vec::new();

    for (coord, version) in &dependencies {
        let parts: Vec<&str> = coord.split(':').collect();
        if parts.len() < 2 {
            continue;
        }
        let group = parts[0];
        let artifact = parts[1];

        // 查询 POM 文件获取许可证信息
        match fetch_license_from_pom(group, artifact, version) {
            Ok(license_info) => {
                licenses.push(license_info);
            }
            Err(e) => {
                eprintln!("⚠️  查询 {}:{} 许可证失败: {}", group, artifact, e);
                licenses.push(LicenseInfo {
                    group: group.to_string(),
                    artifact: artifact.to_string(),
                    version: version.to_string(),
                    license: "Unknown".to_string(),
                    spdx_id: "LicenseRef-Unknown".to_string(),
                    category: LicenseCategory::Unknown,
                });
            }
        }
    }

    Ok(licenses)
}

/// 从 POM 文件获取许可证信息
fn fetch_license_from_pom(group: &str, artifact: &str, version: &str) -> Result<LicenseInfo> {
    let client = reqwest::blocking::Client::new();
    let path = group.replace('.', "/");
    let url = format!(
        "https://repo1.maven.org/maven2/{}/{}/{}/{}.pom",
        path, artifact, version, artifact
    );

    let response = client
        .get(&url)
        .send()
        .map_err(|e| crate::error::Error::new(format!("HTTP 请求失败: {}", e)))?;

    if !response.status().is_success() {
        return Ok(LicenseInfo {
            group: group.to_string(),
            artifact: artifact.to_string(),
            version: version.to_string(),
            license: "Unknown".to_string(),
            spdx_id: "LicenseRef-Unknown".to_string(),
            category: LicenseCategory::Unknown,
        });
    }

    let pom_content: String = response
        .text()
        .map_err(|e| crate::error::Error::new(format!("读取响应失败: {}", e)))?;

    // 简单解析 POM 文件中的许可证信息
    let license = extract_license_from_pom(&pom_content);
    let spdx_id = normalize_spdx_id(&license);
    let category = categorize_license(&spdx_id);

    Ok(LicenseInfo {
        group: group.to_string(),
        artifact: artifact.to_string(),
        version: version.to_string(),
        license,
        spdx_id,
        category,
    })
}

/// 从 POM 内容中提取许可证
fn extract_license_from_pom(pom: &str) -> String {
    // 简单的字符串解析（生产环境应使用 XML 解析器）
    if let Some(start) = pom.find("<license><name>") {
        let rest = &pom[start + 15..];
        if let Some(end) = rest.find("</name>") {
            return rest[..end].to_string();
        }
    }
    "Unknown".to_string()
}

/// 规范化 SPDX ID
fn normalize_spdx_id(license: &str) -> String {
    match license.to_lowercase().as_str() {
        "mit" => "MIT".to_string(),
        "apache license 2.0" | "apache-2.0" => "Apache-2.0".to_string(),
        "bsd 2-clause" | "bsd 2 clause" => "BSD-2-Clause".to_string(),
        "bsd 3-clause" | "bsd 3 clause" => "BSD-3-Clause".to_string(),
        "gpl v3" | "gpl-3.0" | "gnu general public license v3" => "GPL-3.0".to_string(),
        "gpl v2" | "gpl-2.0" | "gnu general public license v2" => "GPL-2.0".to_string(),
        "lgpl v3" | "lgpl-3.0" | "gnu lesser general public license v3" => "LGPL-3.0".to_string(),
        "lgpl v2.1" | "lgpl-2.1" | "gnu lesser general public license v2.1" => {
            "LGPL-2.1".to_string()
        }
        "epl 2.0" | "eclipse public license 2.0" => "EPL-2.0".to_string(),
        "mpl 2.0" | "mozilla public license 2.0" => "MPL-2.0".to_string(),
        _ => license.to_string(),
    }
}

/// 分类许可证
fn categorize_license(spdx_id: &str) -> LicenseCategory {
    match spdx_id {
        "MIT" | "Apache-2.0" | "BSD-2-Clause" | "BSD-3-Clause" | "ISC" | "0BSD" => {
            LicenseCategory::Permissive
        }
        "LGPL-2.1" | "LGPL-3.0" | "MPL-2.0" | "EPL-2.0" => LicenseCategory::WeakCopyleft,
        "GPL-2.0" | "GPL-3.0" | "AGPL-3.0" => LicenseCategory::StrongCopyleft,
        _ => LicenseCategory::Unknown,
    }
}

/// 分析合规性
pub fn analyze_compatibility(licenses: &[LicenseInfo]) -> ComplianceReport {
    let mut conflicts = Vec::new();
    let mut warnings = Vec::new();

    // 检测 GPL 与其他许可证的冲突
    let gpl_licenses: Vec<&LicenseInfo> = licenses
        .iter()
        .filter(|l| l.category == LicenseCategory::StrongCopyleft)
        .collect();

    if !gpl_licenses.is_empty() {
        let permissive_licenses: Vec<&LicenseInfo> = licenses
            .iter()
            .filter(|l| l.category == LicenseCategory::Permissive)
            .collect();

        if !permissive_licenses.is_empty() {
            conflicts.push(ComplianceConflict {
                license1: gpl_licenses[0].spdx_id.clone(),
                license2: permissive_licenses[0].spdx_id.clone(),
                reason: "GPL 与宽松许可证可能存在兼容性问题".to_string(),
            });
        }

        warnings.push(format!(
            "检测到 {} 个 GPL 许可证依赖，可能影响项目许可证选择",
            gpl_licenses.len()
        ));
    }

    // 检测许可证互斥
    let has_agpl = licenses.iter().any(|l| l.spdx_id == "AGPL-3.0");
    let has_gpl = licenses.iter().any(|l| l.spdx_id.starts_with("GPL"));

    if has_agpl && has_gpl {
        warnings.push("同时包含 AGPL 和 GPL 许可证，需注意版本兼容性".to_string());
    }

    // 生成摘要
    let mut license_counts = HashMap::new();
    for license in licenses {
        *license_counts.entry(license.spdx_id.clone()).or_insert(0) += 1;
    }

    let summary = LicenseSummary {
        total_dependencies: licenses.len(),
        permissive: licenses
            .iter()
            .filter(|l| l.category == LicenseCategory::Permissive)
            .count(),
        weak_copyleft: licenses
            .iter()
            .filter(|l| l.category == LicenseCategory::WeakCopyleft)
            .count(),
        strong_copyleft: licenses
            .iter()
            .filter(|l| l.category == LicenseCategory::StrongCopyleft)
            .count(),
        unknown: licenses
            .iter()
            .filter(|l| l.category == LicenseCategory::Unknown)
            .count(),
        license_counts,
    };

    ComplianceReport {
        compatible: conflicts.is_empty(),
        conflicts,
        warnings,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_license_category_display() {
        assert_eq!(LicenseCategory::Permissive.display(), "Permissive");
        assert_eq!(LicenseCategory::WeakCopyleft.display(), "Weak Copyleft");
        assert_eq!(LicenseCategory::StrongCopyleft.display(), "Strong Copyleft");
        assert_eq!(LicenseCategory::Unknown.display(), "Unknown");
    }

    #[test]
    fn test_normalize_spdx_id() {
        assert_eq!(normalize_spdx_id("mit"), "MIT");
        assert_eq!(normalize_spdx_id("Apache License 2.0"), "Apache-2.0");
        assert_eq!(normalize_spdx_id("gpl v3"), "GPL-3.0");
    }

    #[test]
    fn test_categorize_license() {
        assert_eq!(categorize_license("MIT"), LicenseCategory::Permissive);
        assert_eq!(
            categorize_license("Apache-2.0"),
            LicenseCategory::Permissive
        );
        assert_eq!(
            categorize_license("GPL-3.0"),
            LicenseCategory::StrongCopyleft
        );
        assert_eq!(
            categorize_license("LGPL-2.1"),
            LicenseCategory::WeakCopyleft
        );
    }

    #[test]
    fn test_analyze_compatibility_no_conflicts() {
        let licenses = vec![
            LicenseInfo {
                group: "test".to_string(),
                artifact: "a".to_string(),
                version: "1.0".to_string(),
                license: "MIT".to_string(),
                spdx_id: "MIT".to_string(),
                category: LicenseCategory::Permissive,
            },
            LicenseInfo {
                group: "test".to_string(),
                artifact: "b".to_string(),
                version: "1.0".to_string(),
                license: "Apache-2.0".to_string(),
                spdx_id: "Apache-2.0".to_string(),
                category: LicenseCategory::Permissive,
            },
        ];

        let report = analyze_compatibility(&licenses);
        assert!(report.compatible);
        assert!(report.conflicts.is_empty());
        assert_eq!(report.summary.permissive, 2);
    }

    #[test]
    fn test_analyze_compatibility_with_conflicts() {
        let licenses = vec![
            LicenseInfo {
                group: "test".to_string(),
                artifact: "a".to_string(),
                version: "1.0".to_string(),
                license: "MIT".to_string(),
                spdx_id: "MIT".to_string(),
                category: LicenseCategory::Permissive,
            },
            LicenseInfo {
                group: "test".to_string(),
                artifact: "b".to_string(),
                version: "1.0".to_string(),
                license: "GPL-3.0".to_string(),
                spdx_id: "GPL-3.0".to_string(),
                category: LicenseCategory::StrongCopyleft,
            },
        ];

        let report = analyze_compatibility(&licenses);
        assert!(!report.compatible);
        assert!(!report.conflicts.is_empty());
        assert_eq!(report.summary.strong_copyleft, 1);
    }
    #[test]
    fn test_extract_license_from_pom_with_license() {
        let pom = "<project><licenses><license><name>The MIT License</name></license></licenses></project>";
        assert_eq!(extract_license_from_pom(pom), "The MIT License");
    }

    #[test]
    fn test_extract_license_from_pom_no_license() {
        let pom = r#"<?xml version="1.0"?>
<project><name>test</name></project>"#;
        assert_eq!(extract_license_from_pom(pom), "Unknown");
    }

    #[test]
    fn test_extract_license_from_pom_empty() {
        assert_eq!(extract_license_from_pom(""), "Unknown");
    }

    #[test]
    fn test_normalize_spdx_id_all_variants() {
        assert_eq!(normalize_spdx_id("MIT"), "MIT");
        assert_eq!(normalize_spdx_id("mit"), "MIT");
        assert_eq!(normalize_spdx_id("Apache License 2.0"), "Apache-2.0");
        assert_eq!(normalize_spdx_id("apache-2.0"), "Apache-2.0");
        assert_eq!(normalize_spdx_id("BSD 2-Clause"), "BSD-2-Clause");
        assert_eq!(normalize_spdx_id("bsd 2 clause"), "BSD-2-Clause");
        assert_eq!(normalize_spdx_id("BSD 3-Clause"), "BSD-3-Clause");
        assert_eq!(normalize_spdx_id("bsd 3 clause"), "BSD-3-Clause");
        assert_eq!(normalize_spdx_id("GPL v3"), "GPL-3.0");
        assert_eq!(normalize_spdx_id("GPL-3.0"), "GPL-3.0");
        assert_eq!(
            normalize_spdx_id("GNU General Public License v3"),
            "GPL-3.0"
        );
        assert_eq!(normalize_spdx_id("GPL v2"), "GPL-2.0");
        assert_eq!(normalize_spdx_id("LGPL v3"), "LGPL-3.0");
        assert_eq!(normalize_spdx_id("LGPL v2.1"), "LGPL-2.1");
        assert_eq!(normalize_spdx_id("EPL 2.0"), "EPL-2.0");
        assert_eq!(normalize_spdx_id("MPL 2.0"), "MPL-2.0");
        // Unknown stays as-is
        assert_eq!(normalize_spdx_id("WTFPL"), "WTFPL");
        assert_eq!(
            normalize_spdx_id("Custom-Proprietary"),
            "Custom-Proprietary"
        );
    }

    #[test]
    fn test_categorize_license_all_variants() {
        // Permissive
        assert_eq!(categorize_license("MIT"), LicenseCategory::Permissive);
        assert_eq!(
            categorize_license("Apache-2.0"),
            LicenseCategory::Permissive
        );
        assert_eq!(
            categorize_license("BSD-2-Clause"),
            LicenseCategory::Permissive
        );
        assert_eq!(
            categorize_license("BSD-3-Clause"),
            LicenseCategory::Permissive
        );
        assert_eq!(categorize_license("ISC"), LicenseCategory::Permissive);
        assert_eq!(categorize_license("0BSD"), LicenseCategory::Permissive);
        // Weak copyleft
        assert_eq!(
            categorize_license("LGPL-2.1"),
            LicenseCategory::WeakCopyleft
        );
        assert_eq!(
            categorize_license("LGPL-3.0"),
            LicenseCategory::WeakCopyleft
        );
        assert_eq!(categorize_license("MPL-2.0"), LicenseCategory::WeakCopyleft);
        assert_eq!(categorize_license("EPL-2.0"), LicenseCategory::WeakCopyleft);
        // Strong copyleft
        assert_eq!(
            categorize_license("GPL-2.0"),
            LicenseCategory::StrongCopyleft
        );
        assert_eq!(
            categorize_license("GPL-3.0"),
            LicenseCategory::StrongCopyleft
        );
        assert_eq!(
            categorize_license("AGPL-3.0"),
            LicenseCategory::StrongCopyleft
        );
        // Unknown
        assert_eq!(categorize_license("WTFPL"), LicenseCategory::Unknown);
        assert_eq!(categorize_license("BSD-4-Clause"), LicenseCategory::Unknown);
    }

    #[test]
    fn test_license_summary_serialize() {
        let mut counts = HashMap::new();
        counts.insert("MIT".to_string(), 5);
        counts.insert("Apache-2.0".to_string(), 3);
        let summary = LicenseSummary {
            total_dependencies: 10,
            permissive: 8,
            weak_copyleft: 1,
            strong_copyleft: 1,
            unknown: 0,
            license_counts: counts,
        };
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("total_dependencies"));
        assert!(json.contains("MIT"));
        let deserialized: LicenseSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total_dependencies, 10);
        assert_eq!(deserialized.permissive, 8);
    }

    #[test]
    fn test_compliance_conflict_serialize() {
        let conflict = ComplianceConflict {
            license1: "MIT".to_string(),
            license2: "GPL-3.0".to_string(),
            reason: "incompatible".to_string(),
        };
        let json = serde_json::to_string(&conflict).unwrap();
        assert!(json.contains("MIT"));
        let deserialized: ComplianceConflict = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.license1, "MIT");
    }

    #[test]
    fn test_compliance_report_serialize() {
        let report = ComplianceReport {
            compatible: true,
            conflicts: vec![],
            warnings: vec!["check dependencies".to_string()],
            summary: LicenseSummary {
                total_dependencies: 1,
                permissive: 1,
                weak_copyleft: 0,
                strong_copyleft: 0,
                unknown: 0,
                license_counts: HashMap::new(),
            },
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("compatible"));
        let deserialized: ComplianceReport = serde_json::from_str(&json).unwrap();
        assert!(deserialized.compatible);
        assert_eq!(deserialized.warnings.len(), 1);
    }

    #[test]
    fn test_analyze_compatibility_empty() {
        let report = analyze_compatibility(&[]);
        assert!(report.compatible);
        assert!(report.conflicts.is_empty());
        assert_eq!(report.summary.total_dependencies, 0);
    }

    #[test]
    fn test_analyze_compatibility_weak_copyleft_only() {
        let licenses = vec![LicenseInfo {
            group: "a".to_string(),
            artifact: "b".to_string(),
            version: "1.0".to_string(),
            license: "LGPL-2.1".to_string(),
            spdx_id: "LGPL-2.1".to_string(),
            category: LicenseCategory::WeakCopyleft,
        }];
        let report = analyze_compatibility(&licenses);
        assert!(report.compatible);
        assert_eq!(report.summary.weak_copyleft, 1);
    }

    #[test]
    fn test_analyze_compatibility_unknown_only() {
        let licenses = vec![LicenseInfo {
            group: "a".to_string(),
            artifact: "b".to_string(),
            version: "1.0".to_string(),
            license: "Proprietary".to_string(),
            spdx_id: "LicenseRef-Unknown".to_string(),
            category: LicenseCategory::Unknown,
        }];
        let report = analyze_compatibility(&licenses);
        assert!(report.compatible);
        assert_eq!(report.summary.unknown, 1);
    }

    #[test]
    fn test_license_info_clone() {
        let info = LicenseInfo {
            group: "g".to_string(),
            artifact: "a".to_string(),
            version: "1.0".to_string(),
            license: "MIT".to_string(),
            spdx_id: "MIT".to_string(),
            category: LicenseCategory::Permissive,
        };
        let cloned = info.clone();
        assert_eq!(cloned.group, "g");
        assert_eq!(cloned.spdx_id, "MIT");
    }

    #[test]
    fn test_license_category_clone() {
        let c = LicenseCategory::StrongCopyleft;
        let cloned = c;
        assert_eq!(cloned, LicenseCategory::StrongCopyleft);
    }

    #[test]
    fn test_normalize_spdx_id_case_insensitive() {
        assert_eq!(normalize_spdx_id("MIT"), "MIT");
        assert_eq!(normalize_spdx_id("mit"), "MIT");
        assert_eq!(normalize_spdx_id("Mit"), "MIT");
    }
}
