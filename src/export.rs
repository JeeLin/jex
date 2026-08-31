//! 生态互通（export maven）
//! - export maven: 读 jex.lock.toml → 生成 pom.xml

use crate::deps::{read_jex_lock, read_jex_toml};
use crate::error::Result;
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
        let parts: Vec<&str> = coord.split(':').collect();
        if parts.len() >= 2 {
            let dep_group = parts[0];
            let dep_artifact = parts[1];
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
