//! 公共工具函数

use crate::error::{Error, Result};

/// 解析 Maven 坐标 `group:artifact` 格式
pub fn parse_coord(coord: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() < 2 {
        return Err(Error::new(format!(
            "无效的坐标格式: {}（应为 group:artifact）",
            coord
        )));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}
