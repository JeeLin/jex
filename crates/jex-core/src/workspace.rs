//! Monorepo 支持（jex workspace）
//! - workspace: 多模块项目统一管理

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 工作区配置（TOML 顶层）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRoot {
    pub workspace: WorkspaceConfig,
}

/// 工作区配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// 模块 glob 模式列表
    pub members: Vec<String>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            members: vec!["crates/*".to_string()],
        }
    }
}

/// 模块信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    /// 模块名
    pub name: String,
    /// 模块路径
    pub path: PathBuf,
    /// 依赖数量
    pub dependencies_count: usize,
}

/// 工作区状态汇总
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStatus {
    /// 工作区根目录
    pub root: PathBuf,
    /// 模块列表
    pub modules: Vec<ModuleInfo>,
    /// 总依赖数
    pub total_dependencies: usize,
}

/// 查找工作区根目录（向上遍历查找 jex-workspace.toml）
pub fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let config_path = current.join("jex-workspace.toml");
        if config_path.exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// 解析工作区配置
pub fn parse_workspace_config(root: &Path) -> Result<WorkspaceConfig> {
    let config_path = root.join("jex-workspace.toml");
    if !config_path.exists() {
        return Err(crate::error::Error::new(format!(
            "工作区配置不存在: {}",
            config_path.display()
        )));
    }
    let content = std::fs::read_to_string(&config_path)?;
    let root: WorkspaceRoot = toml::from_str(&content)
        .map_err(|e| crate::error::Error::new(format!("解析工作区配置失败: {}", e)))?;
    Ok(root.workspace)
}

/// 根据 glob 模式发现模块
pub fn discover_modules(root: &Path, config: &WorkspaceConfig) -> Result<Vec<ModuleInfo>> {
    let mut modules = Vec::new();

    for pattern in &config.members {
        let expanded = expand_glob_pattern(root, pattern);
        let entries = std::fs::read_dir(&expanded).map_err(|_| {
            crate::error::Error::new(format!("无法读取目录: {}", expanded.display()))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let jex_toml = path.join("jex.toml");
                if jex_toml.exists() {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let dependencies_count = count_dependencies(&path)?;
                    modules.push(ModuleInfo {
                        name,
                        path,
                        dependencies_count,
                    });
                }
            }
        }
    }

    modules.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(modules)
}

/// 展开 glob 模式中的通配符（简单实现：支持 * 匹配一级目录）
fn expand_glob_pattern(root: &Path, pattern: &str) -> PathBuf {
    if pattern.contains('*') {
        // 取 * 之前的前缀部分
        let prefix = pattern.split('*').next().unwrap_or("");
        root.join(prefix.trim_end_matches('/'))
    } else {
        root.join(pattern)
    }
}

/// 统计模块的依赖数量
fn count_dependencies(module_path: &Path) -> Result<usize> {
    let jex_toml = module_path.join("jex.toml");
    if !jex_toml.exists() {
        return Ok(0);
    }
    let content = std::fs::read_to_string(&jex_toml)?;
    // 简单计数 [dependencies] 下的条目数
    let mut in_deps = false;
    let mut count = 0;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[dependencies]" {
            in_deps = true;
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_deps = false;
            continue;
        }
        if in_deps && trimmed.contains('=') && !trimmed.starts_with('#') {
            count += 1;
        }
    }
    Ok(count)
}

/// 获取工作区状态
pub fn workspace_status(root: &Path) -> Result<WorkspaceStatus> {
    let config = parse_workspace_config(root)?;
    let modules = discover_modules(root, &config)?;
    let total_dependencies = modules.iter().map(|m| m.dependencies_count).sum();
    Ok(WorkspaceStatus {
        root: root.to_path_buf(),
        modules,
        total_dependencies,
    })
}

/// 初始化工作区（创建 jex-workspace.toml）
pub fn workspace_init(dir: &Path) -> Result<PathBuf> {
    let config_path = dir.join("jex-workspace.toml");
    if config_path.exists() {
        return Err(crate::error::Error::new(format!(
            "工作区配置已存在: {}",
            config_path.display()
        )));
    }
    let template = r#"[workspace]
members = ["crates/*"]
"#;
    std::fs::write(&config_path, template)?;
    Ok(config_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_workspace(base: &Path) {
        // 创建工作区根
        fs::write(
            base.join("jex-workspace.toml"),
            "[workspace]\nmembers = [\"modules/*\"]\n",
        )
        .unwrap();

        // 创建模块 a
        let mod_a = base.join("modules").join("a");
        fs::create_dir_all(&mod_a).unwrap();
        fs::write(
            mod_a.join("jex.toml"),
            "[dependencies]\nserde = \"1.0\"\ntoml = \"0.8\"\n",
        )
        .unwrap();

        // 创建模块 b
        let mod_b = base.join("modules").join("b");
        fs::create_dir_all(&mod_b).unwrap();
        fs::write(
            mod_b.join("jex.toml"),
            "[dependencies]\nreqwest = \"0.12\"\n",
        )
        .unwrap();

        // 创建非模块目录（无 jex.toml）
        fs::create_dir_all(base.join("modules").join("not-a-module")).unwrap();
    }

    #[test]
    fn test_find_workspace_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::write(root.join("jex-workspace.toml"), "").unwrap();

        let sub = root.join("a").join("b");
        fs::create_dir_all(&sub).unwrap();

        let found = find_workspace_root(&sub).unwrap();
        assert_eq!(found, root);
    }

    #[test]
    fn test_find_workspace_root_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let result = find_workspace_root(tmp.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_discover_modules() {
        let tmp = tempfile::tempdir().unwrap();
        setup_test_workspace(tmp.path());

        let config = parse_workspace_config(tmp.path()).unwrap();
        let modules = discover_modules(tmp.path(), &config).unwrap();

        assert_eq!(modules.len(), 2);
        assert_eq!(modules[0].name, "a");
        assert_eq!(modules[0].dependencies_count, 2);
        assert_eq!(modules[1].name, "b");
        assert_eq!(modules[1].dependencies_count, 1);
    }

    #[test]
    fn test_workspace_status() {
        let tmp = tempfile::tempdir().unwrap();
        setup_test_workspace(tmp.path());

        let status = workspace_status(tmp.path()).unwrap();
        assert_eq!(status.modules.len(), 2);
        assert_eq!(status.total_dependencies, 3);
    }

    #[test]
    fn test_workspace_init() {
        let tmp = tempfile::tempdir().unwrap();
        let path = workspace_init(tmp.path()).unwrap();
        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("[workspace]"));
    }

    #[test]
    fn test_workspace_init_already_exists() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("jex-workspace.toml"), "").unwrap();
        let result = workspace_init(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_workspace_config_default() {
        let config = WorkspaceConfig::default();
        assert_eq!(config.members, vec!["crates/*".to_string()]);
    }
}
