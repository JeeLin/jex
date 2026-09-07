//! 热重载（jex watch）
//! - watch: 监听文件变更，自动重新编译运行

use crate::error::Result;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// 监听配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig {
    /// 监听目录
    pub dir: String,
    /// debounce 延迟（毫秒）
    pub debounce_ms: u64,
    /// 文件过滤模式（默认只监听 .java）
    pub patterns: Vec<String>,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            dir: "src".to_string(),
            debounce_ms: 500,
            patterns: vec![".java".to_string()],
        }
    }
}

/// 文件变更事件
#[derive(Debug, Clone)]
pub struct FileEvent {
    pub path: PathBuf,
    pub kind: EventKind,
}

/// 检查文件是否匹配过滤模式
pub fn matches_patterns(path: &Path, patterns: &[String]) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = format!(".{}", ext.to_string_lossy());
        return patterns.iter().any(|p| p == &ext_str);
    }
    false
}

/// 检查路径是否应被忽略（target/、.git/ 等）
pub fn should_ignore(path: &Path) -> bool {
    let components: Vec<_> = path.components().collect();
    for component in &components {
        if let std::path::Component::Normal(name) = component {
            let name_str = name.to_string_lossy();
            if name_str == "target" || name_str == ".git" || name_str == ".dev-flow" {
                return true;
            }
        }
    }
    false
}

/// 启动文件监听器
pub fn start_watcher(
    config: &WatchConfig,
    tx: mpsc::Sender<FileEvent>,
) -> Result<RecommendedWatcher> {
    let dir = PathBuf::from(&config.dir);
    if !dir.exists() {
        return Err(crate::error::Error::new(format!(
            "监听目录不存在: {}",
            config.dir
        )));
    }

    let patterns = config.patterns.clone();
    let debounce = Duration::from_millis(config.debounce_ms);

    let mut watcher = RecommendedWatcher::new(
        move |result: std::result::Result<Event, notify::Error>| {
            if let Ok(event) = result {
                // 过滤事件类型
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {}
                    _ => return,
                }

                // 过滤文件路径
                for path in &event.paths {
                    if should_ignore(path) {
                        continue;
                    }
                    if matches_patterns(path, &patterns) {
                        let _ = tx.send(FileEvent {
                            path: path.clone(),
                            kind: event.kind,
                        });
                    }
                }
            }
        },
        Config::default(),
    )?;

    watcher.watch(&dir, RecursiveMode::Recursive)?;

    // Debounce: 等待一段时间后才处理
    let _debounce = debounce;

    Ok(watcher)
}

/// 运行监听循环（阻塞，直到 Ctrl+C）
pub fn run_watch_loop<F>(config: &WatchConfig, mut callback: F) -> Result<()>
where
    F: FnMut(&FileEvent) -> Result<()>,
{
    let (tx, rx) = mpsc::channel();
    let mut _watcher = start_watcher(config, tx)?;

    let mut last_event = Instant::now() - Duration::from_secs(1);
    let debounce = Duration::from_millis(config.debounce_ms);

    println!(
        "👀 监听 {} 目录（debounce: {}ms）...",
        config.dir, config.debounce_ms
    );
    println!("   按 Ctrl+C 退出\n");

    while let Ok(event) = rx.recv() {
        let now = Instant::now();
        if now.duration_since(last_event) < debounce {
            continue; // debounce: 跳过快速连续事件
        }
        last_event = now;

        callback(&event)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watch_config_default() {
        let config = WatchConfig::default();
        assert_eq!(config.dir, "src");
        assert_eq!(config.debounce_ms, 500);
        assert!(config.patterns.contains(&".java".to_string()));
    }

    #[test]
    fn test_matches_patterns_java() {
        let patterns = vec![".java".to_string()];
        assert!(matches_patterns(Path::new("Main.java"), &patterns));
        assert!(!matches_patterns(Path::new("Main.rs"), &patterns));
        assert!(!matches_patterns(Path::new("README.md"), &patterns));
    }

    #[test]
    fn test_matches_patterns_multiple() {
        let patterns = vec![".java".to_string(), ".kt".to_string()];
        assert!(matches_patterns(Path::new("Main.java"), &patterns));
        assert!(matches_patterns(Path::new("Main.kt"), &patterns));
        assert!(!matches_patterns(Path::new("Main.py"), &patterns));
    }

    #[test]
    fn test_should_ignore() {
        assert!(should_ignore(Path::new("target/debug/jex")));
        assert!(should_ignore(Path::new(".git/config")));
        assert!(!should_ignore(Path::new("src/.gitignore")));
        assert!(!should_ignore(Path::new("src/Main.java")));
        assert!(!should_ignore(Path::new("crates/jex-core/src/lib.rs")));
    }

    #[test]
    fn test_should_ignore_nested() {
        assert!(should_ignore(Path::new(
            "project/target/classes/Main.class"
        )));
        assert!(!should_ignore(Path::new("project/src/Main.java")));
    }
}
