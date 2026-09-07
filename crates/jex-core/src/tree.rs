//! 依赖树可视化（jex tree）
//! - tree: 可视化项目依赖树结构

use crate::deps;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// 依赖树
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyTree {
    pub root: DependencyNode,
}

/// 依赖节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyNode {
    pub name: String,
    pub version: String,
    pub license: Option<String>,
    pub children: Vec<DependencyNode>,
}

/// 构建依赖树
pub fn build_dependency_tree() -> Result<DependencyTree> {
    let lock = deps::read_jex_lock()?;
    let dependencies = lock.dependencies.unwrap_or_default();

    let mut root = DependencyNode {
        name: "root".to_string(),
        version: "".to_string(),
        license: None,
        children: Vec::new(),
    };

    for (coord, version) in &dependencies {
        let parts: Vec<&str> = coord.split(':').collect();
        if parts.len() < 2 {
            continue;
        }
        let group = parts[0];
        let artifact = parts[1];

        let node = DependencyNode {
            name: format!("{}:{}", group, artifact),
            version: version.clone(),
            license: None, // 后续可集成 license.rs
            children: Vec::new(),
        };

        root.children.push(node);
    }

    Ok(DependencyTree { root })
}

/// 渲染依赖树
pub fn render_tree(tree: &DependencyTree, depth: Option<usize>) -> String {
    let mut output = String::new();
    let max_depth = depth.unwrap_or(usize::MAX);

    render_node(&tree.root, &mut output, "", true, 0, max_depth);

    output
}

/// 渲染节点
fn render_node(
    node: &DependencyNode,
    output: &mut String,
    prefix: &str,
    is_last: bool,
    current_depth: usize,
    max_depth: usize,
) {
    if current_depth > 0 {
        let connector = if is_last { "└── " } else { "├── " };
        output.push_str(&format!("{}{}{}\n", prefix, connector, node.name));
        output.push_str(&format!("{}    {}\n", prefix, node.version));
    } else {
        output.push_str(&format!("{}\n", node.name));
    }

    if current_depth < max_depth {
        let child_prefix = if current_depth == 0 {
            String::new()
        } else if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };

        let child_count = node.children.len();
        for (i, child) in node.children.iter().enumerate() {
            let is_last_child = i == child_count - 1;
            render_node(
                child,
                output,
                &child_prefix,
                is_last_child,
                current_depth + 1,
                max_depth,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_build_dependency_tree() {
        let tree = build_dependency_tree();
        assert!(tree.is_ok());
        let tree = tree.unwrap();
        assert_eq!(tree.root.name, "root");
    }

    #[test]
    fn test_render_tree() {
        let tree = DependencyTree {
            root: DependencyNode {
                name: "root".to_string(),
                version: "".to_string(),
                license: None,
                children: vec![
                    DependencyNode {
                        name: "com.google.code.gson:gson".to_string(),
                        version: "2.11.0".to_string(),
                        license: Some("MIT".to_string()),
                        children: Vec::new(),
                    },
                    DependencyNode {
                        name: "org.junit.jupiter:junit-jupiter".to_string(),
                        version: "5.10.0".to_string(),
                        license: Some("Apache-2.0".to_string()),
                        children: Vec::new(),
                    },
                ],
            },
        };

        let output = render_tree(&tree, None);
        assert!(output.contains("root"));
        assert!(output.contains("com.google.code.gson:gson"));
        assert!(output.contains("org.junit.jupiter:junit-jupiter"));
    }

    #[test]
    fn test_render_tree_with_depth() {
        let tree = DependencyTree {
            root: DependencyNode {
                name: "root".to_string(),
                version: "".to_string(),
                license: None,
                children: vec![DependencyNode {
                    name: "child".to_string(),
                    version: "1.0".to_string(),
                    license: None,
                    children: vec![DependencyNode {
                        name: "grandchild".to_string(),
                        version: "0.5".to_string(),
                        license: None,
                        children: Vec::new(),
                    }],
                }],
            },
        };

        let output = render_tree(&tree, Some(1));
        assert!(output.contains("child"));
        assert!(!output.contains("grandchild"));
    }

    #[test]
    #[serial]
    fn test_build_dependency_tree_with_deps() {
        let tmp = tempfile::tempdir().unwrap();
        let lock_content = r#"lockfile_version = 1

[dependencies]
"com.google.code.gson:gson" = "2.11.0"
"org.junit.jupiter:junit-jupiter" = "5.10.0"
"#;
        std::fs::write(tmp.path().join("jex.lock.toml"), lock_content).unwrap();

        let orig = std::env::current_dir().unwrap();
        // SAFETY: we restore immediately after
        std::env::set_current_dir(tmp.path()).unwrap();
        let result = build_dependency_tree();
        let _ = std::env::set_current_dir(&orig);

        let tree = result.unwrap();
        assert_eq!(tree.root.name, "root");
        assert_eq!(tree.root.children.len(), 2);
        // children order is non-deterministic (HashMap), check both exist
        let names: Vec<_> = tree.root.children.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"com.google.code.gson:gson"));
        assert!(names.contains(&"org.junit.jupiter:junit-jupiter"));
        let gson = tree.root.children.iter().find(|c| c.name == "com.google.code.gson:gson").unwrap();
        assert_eq!(gson.version, "2.11.0");
    }

    #[test]
    #[serial]
    fn test_build_dependency_tree_lock_malformed() {
        let tmp = tempfile::tempdir().unwrap();
        // Write invalid TOML to trigger parse error
        std::fs::write(tmp.path().join("jex.lock.toml"), "this is not valid toml {{{").unwrap();

        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let result = build_dependency_tree();
        let _ = std::env::set_current_dir(&orig);

        assert!(result.is_err());
    }

    #[test]
    fn test_render_tree_single_child_not_last() {
        // Test the "├── " connector (is_last = false)
        let tree = DependencyTree {
            root: DependencyNode {
                name: "root".to_string(),
                version: "".to_string(),
                license: None,
                children: vec![
                    DependencyNode {
                        name: "first".to_string(),
                        version: "1.0".to_string(),
                        license: None,
                        children: Vec::new(),
                    },
                    DependencyNode {
                        name: "second".to_string(),
                        version: "2.0".to_string(),
                        license: None,
                        children: Vec::new(),
                    },
                ],
            },
        };
        let output = render_tree(&tree, None);
        assert!(output.contains("├── first"));
        assert!(output.contains("└── second"));
    }

    #[test]
    fn test_render_tree_depth_zero() {
        // depth=0 means only root, no children rendered
        let tree = DependencyTree {
            root: DependencyNode {
                name: "root".to_string(),
                version: "".to_string(),
                license: None,
                children: vec![DependencyNode {
                    name: "child".to_string(),
                    version: "1.0".to_string(),
                    license: None,
                    children: Vec::new(),
                }],
            },
        };
        let output = render_tree(&tree, Some(0));
        assert!(output.contains("root"));
        assert!(!output.contains("child"));
    }
}
