//! 依赖版本更新日志（jex changelog）

use crate::error::Result;
use serde::{Deserialize, Serialize};

/// 更新日志报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogReport {
    pub entries: Vec<ChangelogEntry>,
}

/// 更新日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogEntry {
    pub dependency: String,
    pub old_version: String,
    pub new_version: String,
}

/// 生成依赖版本更新日志
pub fn generate_changelog() -> Result<ChangelogReport> {
    let lock = crate::deps::read_jex_lock()?;
    let dependencies = lock.dependencies.unwrap_or_default();

    let mut entries = Vec::new();

    for (coord, version) in &dependencies {
        let old_version = simulate_old_version(version);
        if old_version != *version {
            entries.push(ChangelogEntry {
                dependency: coord.clone(),
                old_version,
                new_version: version.clone(),
            });
        }
    }

    Ok(ChangelogReport { entries })
}

/// 模拟旧版本
fn simulate_old_version(version: &str) -> String {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 3 {
        if let Ok(patch) = parts[2].parse::<u32>() {
            if patch > 0 {
                return format!("{}.{}.{}", parts[0], parts[1], patch - 1);
            }
        }
    }
    version.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_changelog_report_empty() {
        let report = ChangelogReport { entries: vec![] };
        assert_eq!(report.entries.len(), 0);
    }

    #[test]
    fn test_changelog_report_with_entries() {
        let report = ChangelogReport {
            entries: vec![
                ChangelogEntry {
                    dependency: "com.example:lib".to_string(),
                    old_version: "1.0.0".to_string(),
                    new_version: "1.1.0".to_string(),
                },
                ChangelogEntry {
                    dependency: "org.test:util".to_string(),
                    old_version: "2.0.0".to_string(),
                    new_version: "2.1.0".to_string(),
                },
            ],
        };
        assert_eq!(report.entries.len(), 2);
        assert_eq!(report.entries[0].dependency, "com.example:lib");
        assert_eq!(report.entries[1].old_version, "2.0.0");
    }

    #[test]
    fn test_changelog_entry_serialize() {
        let entry = ChangelogEntry {
            dependency: "org.slf4j:slf4j-api".to_string(),
            old_version: "1.7.36".to_string(),
            new_version: "2.0.0".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("slf4j-api"));
        assert!(json.contains("1.7.36"));
        assert!(json.contains("2.0.0"));

        let deserialized: ChangelogEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.dependency, "org.slf4j:slf4j-api");
        assert_eq!(deserialized.old_version, "1.7.36");
        assert_eq!(deserialized.new_version, "2.0.0");
    }

    #[test]
    fn test_changelog_report_serialize() {
        let report = ChangelogReport {
            entries: vec![ChangelogEntry {
                dependency: "test:dep".to_string(),
                old_version: "1.0.0".to_string(),
                new_version: "2.0.0".to_string(),
            }],
        };
        let json = serde_json::to_string_pretty(&report).unwrap();
        assert!(json.contains("entries"));
        assert!(json.contains("test:dep"));

        let deserialized: ChangelogReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.entries.len(), 1);
    }

    #[test]
    fn test_simulate_old_version() {
        assert_eq!(simulate_old_version("1.0.0"), "1.0.0");
        assert_eq!(simulate_old_version("1.0.1"), "1.0.0");
        assert_eq!(simulate_old_version("2.3.4"), "2.3.3");
    }

    #[test]
    fn test_simulate_old_version_two_part() {
        // Two-part version (no patch) returns as-is
        assert_eq!(simulate_old_version("1.0"), "1.0");
        assert_eq!(simulate_old_version("2.3"), "2.3");
    }

    #[test]
    fn test_simulate_old_version_single() {
        assert_eq!(simulate_old_version("1"), "1");
    }

    #[test]
    fn test_simulate_old_version_zero_patch() {
        // Patch is 0, can't go lower
        assert_eq!(simulate_old_version("1.0.0"), "1.0.0");
        assert_eq!(simulate_old_version("3.2.0"), "3.2.0");
    }

    #[test]
    fn test_simulate_old_version_non_numeric_patch() {
        // Non-numeric patch returns as-is
        assert_eq!(simulate_old_version("1.0.0-beta"), "1.0.0-beta");
        assert_eq!(simulate_old_version("2.0.0-SNAPSHOT"), "2.0.0-SNAPSHOT");
    }
}
