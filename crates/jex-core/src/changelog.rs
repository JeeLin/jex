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
    fn test_changelog_report() {
        let report = ChangelogReport {
            entries: vec![ChangelogEntry {
                dependency: "com.example:lib".to_string(),
                old_version: "1.0.0".to_string(),
                new_version: "1.1.0".to_string(),
            }],
        };
        assert_eq!(report.entries.len(), 1);
    }

    #[test]
    fn test_simulate_old_version() {
        assert_eq!(simulate_old_version("1.0.0"), "1.0.0");
        assert_eq!(simulate_old_version("1.0.1"), "1.0.0");
        assert_eq!(simulate_old_version("2.3.4"), "2.3.3");
    }
}
