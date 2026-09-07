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
}
