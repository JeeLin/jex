use std::path::Path;

use crate::error::{Error, Result};

/// 脚本元数据：从 Java 文件的 shebang 和 pragma 注解中解析
#[derive(Debug, Clone, Default)]
pub struct ScriptMeta {
    /// Java 版本要求（如 "21"、"21+"）
    pub java_version: Option<String>,
    /// 依赖列表（如 "com.google.code.gson:gson:2.11.0"）
    pub deps: Vec<String>,
    /// 是否是脚本文件（有 shebang 或 pragma）
    pub is_script: bool,
}

/// 从 Java 文件路径解析脚本元数据
pub fn parse_script(path: &Path) -> Result<ScriptMeta> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| Error::new(format!("无法读取文件 {}: {}", path.display(), e)))?;

    let mut meta = ScriptMeta::default();

    for line in content.lines() {
        let trimmed = line.trim();

        // 检查 shebang
        if trimmed.starts_with("#!/") || trimmed.starts_with("///") {
            meta.is_script = true;
            continue;
        }

        // 检查 //DEPS 注解
        if let Some(deps) = trimmed.strip_prefix("//DEPS ") {
            let dep = deps.trim().to_string();
            if !dep.is_empty() {
                meta.deps.push(dep);
                meta.is_script = true;
            }
            continue;
        }

        // 检查 //JAVA 注解
        if let Some(java_ver) = trimmed.strip_prefix("//JAVA ") {
            let ver = java_ver.trim().to_string();
            if !ver.is_empty() {
                meta.java_version = Some(ver);
                meta.is_script = true;
            }
            continue;
        }

        // 如果遇到非注解行，停止解析（避免扫描整个文件）
        if !trimmed.is_empty() && !trimmed.starts_with("//") {
            break;
        }
    }

    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_shebang() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "#!/usr/bin/env jex\n\nimport java.util.List;\n\npublic class Main {{}}"
        )
        .unwrap();

        let meta = parse_script(file.path()).unwrap();
        assert!(meta.is_script);
        assert!(meta.deps.is_empty());
        assert!(meta.java_version.is_none());
    }

    #[test]
    fn test_parse_deps() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "//DEPS com.google.code.gson:gson:2.11.0\n//DEPS org.apache.commons:commons-lang3:3.14.0\n\nimport com.google.gson.Gson;\n\npublic class Main {{}}"
        )
        .unwrap();

        let meta = parse_script(file.path()).unwrap();
        assert!(meta.is_script);
        assert_eq!(meta.deps.len(), 2);
        assert_eq!(meta.deps[0], "com.google.code.gson:gson:2.11.0");
        assert_eq!(meta.deps[1], "org.apache.commons:commons-lang3:3.14.0");
    }

    #[test]
    fn test_parse_java_version() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "//JAVA 21\n\nimport java.util.List;\n\npublic class Main {{}}"
        )
        .unwrap();

        let meta = parse_script(file.path()).unwrap();
        assert!(meta.is_script);
        assert_eq!(meta.java_version.as_deref(), Some("21"));
    }

    #[test]
    fn test_parse_combined() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            "///usr/bin/env jex\n//DEPS com.google.code.gson:gson:2.11.0\n//JAVA 21\n\nimport com.google.gson.Gson;\n\npublic class Main {{}}"
        )
        .unwrap();

        let meta = parse_script(file.path()).unwrap();
        assert!(meta.is_script);
        assert_eq!(meta.deps.len(), 1);
        assert_eq!(meta.java_version.as_deref(), Some("21"));
    }

    #[test]
    fn test_parse_non_script() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "import java.util.List;\n\npublic class Main {{}}").unwrap();

        let meta = parse_script(file.path()).unwrap();
        assert!(!meta.is_script);
        assert!(meta.deps.is_empty());
        assert!(meta.java_version.is_none());
    }

    #[test]
    fn test_parse_empty_deps() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "//DEPS \n\nimport java.util.List;").unwrap();

        let meta = parse_script(file.path()).unwrap();
        assert!(!meta.is_script);
        assert!(meta.deps.is_empty());
    }
}
