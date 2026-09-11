use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Zh,
    En,
}

impl Lang {
    pub fn parse_lang(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "zh" | "zh-cn" => Some(Lang::Zh),
            "en" | "en-us" => Some(Lang::En),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Lang::Zh => "zh",
            Lang::En => "en",
        }
    }
}

static CURRENT_LANG: LazyLock<Mutex<Lang>> = LazyLock::new(|| Mutex::new(Lang::En));
static MESSAGES: LazyLock<Mutex<HashMap<&'static str, (&'static str, &'static str)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn init_messages() {
    let mut m = MESSAGES.lock().unwrap();
    if !m.is_empty() {
        return;
    }
    let e: &[(&str, &str, &str)] = &[
        (
            "cannot_find_home",
            "无法获取 HOME 环境变量",
            "Cannot find HOME environment variable",
        ),
        (
            "failed_get_home_dir",
            "无法获取用户主目录",
            "Failed to get user home directory",
        ),
        (
            "failed_get_home",
            "无法获取 HOME 目录",
            "Failed to get HOME directory",
        ),
        (
            "no_jex_toml",
            "当前目录没有 jex.toml，请先运行 jex init",
            "Current directory has no jex.toml, please run jex init first",
        ),
        (
            "jex_toml_exists",
            "当前目录已存在 jex.toml",
            "Current directory already has jex.toml",
        ),
        (
            "maven_central_failed",
            "请求 Maven Central 失败",
            "Failed to request Maven Central",
        ),
        ("compilation_failed", "编译失败", "Compilation failed"),
        ("execution_failed", "运行失败", "Execution failed"),
        (
            "download_jdk_failed",
            "下载 JDK 失败",
            "Failed to download JDK",
        ),
        (
            "extract_jdk_failed",
            "解压 JDK 失败",
            "Failed to extract JDK",
        ),
        (
            "extracted_jdk_not_found",
            "未找到解压后的 JDK 目录",
            "Could not find extracted JDK directory",
        ),
        ("jcmd_exec_failed", "jcmd 执行失败", "jcmd execution failed"),
        (
            "jstat_gc_exec_failed",
            "jstat -gc 执行失败",
            "jstat -gc execution failed",
        ),
        (
            "parse_jstat_gc_failed",
            "无法解析 jstat -gc 输出",
            "Could not parse jstat -gc output",
        ),
        (
            "tui_init_failed",
            "TUI 初始化失败",
            "TUI initialization failed",
        ),
        (
            "jshell_stdin_failed",
            "无法获取 jshell stdin",
            "Could not get jshell stdin",
        ),
        (
            "jshell_stdout_failed",
            "无法获取 jshell stdout",
            "Could not get jshell stdout",
        ),
        (
            "jfr_file_too_small",
            "JFR 文件太小，不是有效的 JFR 文件",
            "JFR file too small, not a valid JFR file",
        ),
        (
            "jfr_header_incomplete",
            "JFR 头部不完整",
            "JFR header incomplete",
        ),
        (
            "workspace_not_found",
            "未找到工作区（缺少 jex-workspace.toml）",
            "Workspace not found (missing jex-workspace.toml)",
        ),
        (
            "workspace_config_exists",
            "工作区配置已存在",
            "Workspace configuration already exists",
        ),
        ("pinned", "已锁定", "Pinned"),
        ("unpinned", "已解锁", "Unpinned"),
        (
            "jdk_installed",
            "JDK {} 安装成功",
            "JDK {} installed successfully",
        ),
        (
            "downloading_jdk",
            "正在从 Adoptium 下载 JDK",
            "Downloading JDK from Adoptium",
        ),
        (
            "no_jdk_installed",
            "未安装任何 JDK 版本",
            "No JDK versions installed",
        ),
        (
            "installed_jdks",
            "已安装的 JDK 版本:",
            "Installed JDK versions:",
        ),
        ("no_data", "（无数据）", "(no data)"),
        ("jvm_thread_info", "JVM 线程信息", "JVM thread info"),
        ("thread_stats", "线程统计", "Thread statistics"),
        ("total_threads", "总线程数", "Total threads"),
        ("daemon_threads", "守护线程", "Daemon threads"),
        ("deadlock_detected", "🔴 检测到死锁", "🔴 Deadlock detected"),
        ("heap_overview", "堆内存概览", "Heap memory overview"),
    ];
    for &(k, zh, en) in e {
        m.insert(k, (zh, en));
    }
}

pub fn set_lang(lang: Lang) {
    *CURRENT_LANG.lock().unwrap() = lang;
}

pub fn get_lang() -> Lang {
    *CURRENT_LANG.lock().unwrap()
}

pub fn msg(key: &'static str) -> &'static str {
    let lang = *CURRENT_LANG.lock().unwrap();
    let m = MESSAGES.lock().unwrap();
    m.get(key)
        .map(|&(zh, en)| match lang {
            Lang::Zh => zh,
            Lang::En => en,
        })
        .unwrap_or(key)
}

#[macro_export]
macro_rules! msg {
    ($key:literal) => { $crate::i18n::msg($key) };
    ($key:literal, $($arg:tt)*) => { format!("{}", $crate::i18n::msg($key)) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_lang_from_str() {
        assert_eq!(Lang::parse_lang("zh"), Some(Lang::Zh));
        assert_eq!(Lang::parse_lang("ZH"), Some(Lang::Zh));
        assert_eq!(Lang::parse_lang("zh-cn"), Some(Lang::Zh));
        assert_eq!(Lang::parse_lang("en"), Some(Lang::En));
        assert_eq!(Lang::parse_lang("en-us"), Some(Lang::En));
        assert_eq!(Lang::parse_lang("fr"), None);
        assert_eq!(Lang::parse_lang(""), None);
    }

    #[test]
    #[serial]
    fn test_lang_as_str() {
        assert_eq!(Lang::Zh.as_str(), "zh");
        assert_eq!(Lang::En.as_str(), "en");
    }

    #[test]
    #[serial]
    fn test_init_messages_populates_catalog() {
        init_messages();
        let m = MESSAGES.lock().unwrap();
        assert!(!m.is_empty());
        assert!(m.contains_key("cannot_find_home"));
        assert!(m.contains_key("compilation_failed"));
    }

    #[test]
    #[serial]
    fn test_set_and_get_lang() {
        let orig = get_lang();
        set_lang(Lang::Zh);
        assert_eq!(get_lang(), Lang::Zh);
        set_lang(Lang::En);
        assert_eq!(get_lang(), Lang::En);
        set_lang(orig);
    }

    #[test]
    #[serial]
    fn test_msg_zh() {
        init_messages();
        set_lang(Lang::Zh);
        let result = msg("cannot_find_home");
        assert_eq!(result, "无法获取 HOME 环境变量");
        set_lang(Lang::En);
    }

    #[test]
    #[serial]
    fn test_msg_en() {
        init_messages();
        set_lang(Lang::En);
        let result = msg("cannot_find_home");
        assert_eq!(result, "Cannot find HOME environment variable");
        set_lang(Lang::En);
    }

    #[test]
    #[serial]
    fn test_msg_unknown_key_returns_key() {
        init_messages();
        let result = msg("nonexistent_key_12345");
        assert_eq!(result, "nonexistent_key_12345");
    }

    #[test]
    #[serial]
    fn test_msg_with_format_args() {
        init_messages();
        set_lang(Lang::En);
        let result = msg!("jdk_installed");
        assert!(result.contains("{}"));
        set_lang(Lang::En);
    }
}
