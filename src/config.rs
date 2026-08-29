//! 本地状态与配置(Phase 0 占位)
//! - ~/.jx 全局目录(缓存 / 已装 JDK / 配置)
//! - 项目级 jx.toml 脚手架
use crate::error::{Error, Result};
use std::path::PathBuf;

/// 返回 ~/.jx 全局目录路径。
pub fn jx_home() -> Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| Error::new("找不到 HOME 环境变量"))?;
    Ok(PathBuf::from(home).join(".jx"))
}

/// 确保 ~/.jx 存在,返回其路径。
pub fn ensure_jx_home() -> Result<PathBuf> {
    let p = jx_home()?;
    std::fs::create_dir_all(&p)?;
    Ok(p)
}

/// `jx init`:在当前目录生成 jx.toml + src/ 标准目录。
pub fn scaffold(name: Option<&str>) -> Result<()> {
    let _ = ensure_jx_home()?;
    let cwd = std::env::current_dir()?;
    let toml = cwd.join("jx.toml");
    if toml.exists() {
        return Err(Error::new("当前目录已存在 jx.toml"));
    }
    let project_name = name.unwrap_or("demo");
    let content = format!(
        "[project]\n\
         name = \"{name}\"\n\
         java = \"21\"\n\
         \n\
         [dependencies]\n\
         \n\
         [repositories]\n\
         maven-central = true\n\
         \n\
         [build]\n\
         output = \".jx-build\"\n\
         sources = [\"src\"]\n\
         compiler-args = [\"-parameters\", \"-encoding\", \"UTF-8\"]\n",
        name = project_name
    );
    std::fs::write(&toml, content)?;
    std::fs::create_dir_all(cwd.join("src"))?;
    println!("已生成 {} (全局配置目录: {})", toml.display(), jx_home()?.display());
    Ok(())
}
