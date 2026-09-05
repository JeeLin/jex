//! 依赖版本检查（jex outdated/upgrade）
//! - outdated: 检查依赖是否有新版本
//! - upgrade: 升级指定或全部依赖

use crate::deps;
use crate::error::Result;
use crate::resolver;
/// 过时的依赖信息
#[derive(Debug, Clone)]
pub struct OutdatedDep {
    pub group: String,
    pub artifact: String,
    pub current: String,
    pub latest: String,
}

/// 检查过时的依赖
pub fn check_outdated() -> Result<Vec<OutdatedDep>> {
    let _config = deps::read_jex_toml()?;
    let lock = deps::read_jex_lock()?;

    let dependencies = lock.dependencies.unwrap_or_default();
    let mut outdated = Vec::new();

    for (coord, current_version) in &dependencies {
        // 查询最新版本
        match resolver::resolve_latest(coord) {
            Ok(latest_version) => {
                if &latest_version != current_version {
                    let parts: Vec<&str> = coord.split(':').collect();
                    let group = parts.first().unwrap_or(&"").to_string();
                    let artifact = parts.get(1).unwrap_or(&"").to_string();
                    outdated.push(OutdatedDep {
                        group,
                        artifact,
                        current: current_version.clone(),
                        latest: latest_version,
                    });
                }
            }
            Err(e) => {
                // 查询失败时跳过，不阻断其他依赖检查
                eprintln!("⚠️  查询 {} 失败: {}", coord, e);
            }
        }
    }

    Ok(outdated)
}

/// 升级单个依赖
pub fn upgrade_dep(coord: &str) -> Result<()> {
    // 查询最新版本
    let latest_version = resolver::resolve_latest(coord)?;

    // 读取现有配置
    let mut config = deps::read_jex_toml()?;
    let existing = config.dependencies.clone().unwrap_or_default();

    if let Some(current_version) = existing.get(coord) {
        if current_version == &latest_version {
            println!("✅ {} 已是最新版本 ({})", coord, latest_version);
            return Ok(());
        }
        println!("📦 升级 {}: {} → {}", coord, current_version, latest_version);
    } else {
        println!("📦 添加 {}: {}", coord, latest_version);
    }

    // 更新配置
    let mut new_deps = existing.clone();
    new_deps.insert(coord.to_string(), latest_version);
    config.dependencies = Some(new_deps);

    // 写回 jex.toml
    deps::write_jex_toml(&config)?;

    // 重新生成锁文件
    deps::update_lock_file(&config)?;

    println!("✅ 升级完成");
    Ok(())
}

/// 升级全部依赖
pub fn upgrade_all() -> Result<()> {
    let outdated = check_outdated()?;

    if outdated.is_empty() {
        println!("✅ 所有依赖已是最新版本");
        return Ok(());
    }

    println!("📦 找到 {} 个可更新依赖:\n", outdated.len());

    for dep in &outdated {
        println!("  {}:{}: {} → {}", dep.group, dep.artifact, dep.current, dep.latest);
    }

    println!();

    // 读取现有配置
    let mut config = deps::read_jex_toml()?;
    let existing = config.dependencies.clone().unwrap_or_default();
    let mut new_deps = existing.clone();

    // 更新所有过时依赖
    for dep in &outdated {
        let key = format!("{}:{}", dep.group, dep.artifact);
        new_deps.insert(key, dep.latest.clone());
    }

    config.dependencies = Some(new_deps);

    // 写回 jex.toml
    deps::write_jex_toml(&config)?;

    // 重新生成锁文件
    deps::update_lock_file(&config)?;

    println!("✅ 升级完成，已更新 {} 个依赖", outdated.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outdated_dep_struct() {
        let dep = OutdatedDep {
            group: "com.google.code.gson".to_string(),
            artifact: "gson".to_string(),
            current: "2.10.0".to_string(),
            latest: "2.11.0".to_string(),
        };

        assert_eq!(dep.group, "com.google.code.gson");
        assert_eq!(dep.artifact, "gson");
        assert_eq!(dep.current, "2.10.0");
        assert_eq!(dep.latest, "2.11.0");
    }
}
