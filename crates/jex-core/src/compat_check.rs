//! 依赖版本兼容性检查（jex check）

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 兼容性报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityReport {
    pub compatible: Vec<String>,
    pub conflicts: Vec<VersionConflict>,
    pub warnings: Vec<String>,
}

/// 版本冲突
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionConflict {
    pub dependency: String,
    pub required_by: Vec<String>,
    pub versions: Vec<String>,
}

/// 检查依赖版本兼容性
pub fn check_compatibility() -> Result<CompatibilityReport> {
    let lock = crate::deps::read_jex_lock()?;
    let dependencies = lock.dependencies.unwrap_or_default();

    let mut compatible = Vec::new();
    let mut conflicts = Vec::new();
    let mut warnings = Vec::new();

    // 检查依赖之间的版本约束
    let mut version_map: HashMap<String, Vec<String>> = HashMap::new();

    for (coord, version) in &dependencies {
        let parts: Vec<&str> = coord.split(':').collect();
        if parts.len() == 2 {
            let artifact = parts[1].to_string();
            version_map.entry(artifact).or_default().push(version.clone());
        }
    }

    // 检查冲突
    for (artifact, versions) in &version_map {
        if versions.len() > 1 {
            // 检查是否是同一版本
            let unique_versions: Vec<&String> = versions.iter().collect::<std::collections::HashSet<_>>().into_iter().collect();
            if unique_versions.len() > 1 {
                conflicts.push(VersionConflict {
                    dependency: artifact.clone(),
                    required_by: vec![],
                    versions: versions.clone(),
                });
            } else {
                compatible.push(artifact.clone());
            }
        } else {
            compatible.push(artifact.clone());
        }
    }

    // 检查可能的警告
    for (coord, version) in &dependencies {
        if version.is_empty() {
            warnings.push(format!("{} 缺少版本信息", coord));
        }
    }

    Ok(CompatibilityReport {
        compatible,
        conflicts,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compatibility_report() {
        let report = CompatibilityReport {
            compatible: vec!["serde".to_string()],
            conflicts: vec![],
            warnings: vec![],
        };

        assert_eq!(report.compatible.len(), 1);
        assert!(report.conflicts.is_empty());
    }

    #[test]
    fn test_version_conflict() {
        let conflict = VersionConflict {
            dependency: "tokio".to_string(),
            required_by: vec!["reqwest".to_string(), "hyper".to_string()],
            versions: vec!["1.0.0".to_string(), "1.1.0".to_string()],
        };

        assert_eq!(conflict.dependency, "tokio");
        assert_eq!(conflict.versions.len(), 2);
    }

    #[test]
    fn test_check_compatibility_empty_deps() {
        // When no lock file exists, read_jex_lock returns empty deps.
        // check_compatibility should succeed with an empty report.
        let result = check_compatibility();
        let report = result.unwrap();
        assert!(report.compatible.is_empty());
        assert!(report.conflicts.is_empty());
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn test_compatibility_report_serialization() {
        let report = CompatibilityReport {
            compatible: vec!["serde".to_string(), "tokio".to_string()],
            conflicts: vec![VersionConflict {
                dependency: "log".to_string(),
                required_by: vec!["app".to_string()],
                versions: vec!["1.0.0".to_string(), "2.0.0".to_string()],
            }],
            warnings: vec!["missing version".to_string()],
        };
        let json = serde_json::to_string(&report).unwrap();
        let deserialized: CompatibilityReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.compatible.len(), 2);
        assert_eq!(deserialized.conflicts.len(), 1);
        assert_eq!(deserialized.warnings.len(), 1);
    }

    #[test]
    fn test_version_conflict_debug_clone() {
        let conflict = VersionConflict {
            dependency: "test".to_string(),
            required_by: vec![],
            versions: vec!["1.0".to_string()],
        };
        let debug = format!("{:?}", conflict);
        assert!(debug.contains("test"));
        let cloned = conflict.clone();
        assert_eq!(cloned.dependency, "test");
    }

    #[test]
    fn test_compatibility_report_debug_clone() {
        let report = CompatibilityReport {
            compatible: vec![],
            conflicts: vec![],
            warnings: vec![],
        };
        let debug = format!("{:?}", report);
        assert!(debug.contains("CompatibilityReport"));
        let cloned = report.clone();
        assert!(cloned.compatible.is_empty());
    }
}
