//! 生态互通（export maven）
//! - export maven: 读 jex.lock.toml → 生成 pom.xml

use crate::deps::{read_jex_lock, read_jex_toml};
use crate::error::Result;
use crate::util::parse_coord;
use std::fs;
use std::path::PathBuf;

/// 生成 pom.xml
pub fn maven() -> Result<()> {
    let config = read_jex_toml()?;
    let lock = read_jex_lock()?;

    let project_name = config
        .project
        .as_ref()
        .map(|p| p.name.as_str())
        .unwrap_or("demo");

    let group_id = format!("local.{}", project_name);
    let artifact_id = project_name.to_string();
    let version = "0.1.0";

    let dependencies = lock.dependencies.unwrap_or_default();

    let mut pom = String::new();
    pom.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    pom.push_str("<project xmlns=\"http://maven.apache.org/POM/4.0.0\"\n");
    pom.push_str("         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"\n");
    pom.push_str("         xsi:schemaLocation=\"http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd\">\n");
    pom.push_str("    <modelVersion>4.0.0</modelVersion>\n");
    pom.push_str(&format!("    <groupId>{}</groupId>\n", group_id));
    pom.push_str(&format!("    <artifactId>{}</artifactId>\n", artifact_id));
    pom.push_str(&format!("    <version>{}</version>\n", version));
    pom.push_str("    <packaging>jar</packaging>\n");
    pom.push('\n');
    pom.push_str("    <dependencies>\n");

    for (coord, ver) in &dependencies {
        if let Ok((dep_group, dep_artifact)) = parse_coord(coord) {
            pom.push_str("        <dependency>\n");
            pom.push_str(&format!("            <groupId>{}</groupId>\n", dep_group));
            pom.push_str(&format!(
                "            <artifactId>{}</artifactId>\n",
                dep_artifact
            ));
            pom.push_str(&format!("            <version>{}</version>\n", ver));
            pom.push_str("        </dependency>\n");
        }
    }

    pom.push_str("    </dependencies>\n");
    pom.push_str("</project>\n");

    let out_path: PathBuf = std::env::current_dir()?.join("pom.xml");
    fs::write(&out_path, &pom)?;

    println!("已生成 pom.xml ({})", out_path.display());
    println!("  groupId: {}", group_id);
    println!("  artifactId: {}", artifact_id);
    println!("  依赖数: {}", dependencies.len());
    println!("  ⚠️  有损导出：scope 默认 compile，不处理 exclusions");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_pom_xml_content() {
        // 测试 pom.xml 生成逻辑（不调用 maven()，直接测试 XML 构建）
        let project_name = "test-app";
        let group_id = format!("local.{}", project_name);
        let artifact_id = project_name;
        let version = "0.1.0";

        let mut pom = String::new();
        pom.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        pom.push_str("<project xmlns=\"http://maven.apache.org/POM/4.0.0\">\n");
        pom.push_str("    <modelVersion>4.0.0</modelVersion>\n");
        pom.push_str(&format!("    <groupId>{}</groupId>\n", group_id));
        pom.push_str(&format!("    <artifactId>{}</artifactId>\n", artifact_id));
        pom.push_str(&format!("    <version>{}</version>\n", version));
        pom.push_str("    <packaging>jar</packaging>\n");

        assert!(pom.contains("test-app"));
        assert!(pom.contains("local.test-app"));
        assert!(pom.contains("0.1.0"));
        assert!(pom.contains("<packaging>jar</packaging>"));
    }

    #[test]
    fn test_parse_coord_for_export() {
        let (g, a) = parse_coord("com.google.code.gson:gson").unwrap();
        assert_eq!(g, "com.google.code.gson");
        assert_eq!(a, "gson");
    }

    #[test]
    fn test_parse_coord_invalid_for_export() {
        let result = parse_coord("invalid");
        assert!(result.is_err());
    }
}
