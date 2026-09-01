//! 诊断核心（gc/threads）
//! - gc: jstat -gcutil 结构化解析 + 表格展示
//! - threads: jcmd Thread.print 结构化解析 + 线程统计
//!
//! 子任务 3：将 jstat/jcmd 文本输出解析为结构化数据（Rust struct），
//! 改进线程计数准确性，为后续诊断增强（火焰图、JFR）打基础。

use crate::error::{Error, Result};
use std::process::Command;

// ─── GC 结构化数据 ──────────────────────────────────────────────

/// 单条 GC 采样（jstat -gcutil 解析结果）
#[derive(Debug, Clone)]
pub struct GcSnapshot {
    pub s0: f64,
    pub s1: f64,
    pub eden: f64,
    pub old: f64,
    pub meta: f64,
    pub ccs: f64,
    pub ygc: u64,
    pub ygct: f64,
    pub fgc: u64,
    pub fgct: f64,
    pub gct: f64,
}

/// 解析 jstat -gcutil 单行数据
fn parse_gc_line(line: &str) -> Option<GcSnapshot> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 11 {
        return None;
    }

    let nums: Vec<&str> = parts
        .iter()
        .filter(|p| p.parse::<f64>().is_ok() || **p == "-")
        .cloned()
        .collect();

    let start = if nums.len() >= 12 { 1 } else { 0 };
    if nums.len() - start < 11 {
        return None;
    }

    let parse_f = |s: &str| -> f64 {
        if s == "-" {
            0.0
        } else {
            s.parse::<f64>().unwrap_or(0.0)
        }
    };
    let parse_u = |s: &str| -> u64 {
        if s == "-" {
            0
        } else {
            s.parse::<u64>().unwrap_or(0)
        }
    };

    Some(GcSnapshot {
        s0: parse_f(nums[start]),
        s1: parse_f(nums[start + 1]),
        eden: parse_f(nums[start + 2]),
        old: parse_f(nums[start + 3]),
        meta: parse_f(nums[start + 4]),
        ccs: parse_f(nums[start + 5]),
        ygc: parse_u(nums[start + 6]),
        ygct: parse_f(nums[start + 7]),
        fgc: parse_u(nums[start + 8]),
        fgct: parse_f(nums[start + 9]),
        gct: parse_f(nums[start + 10]),
    })
}

/// 格式化单条 GC 采样为表格行
fn format_gc_snapshot(s: &GcSnapshot) -> String {
    format!(
        "{:>6.2} {:>6.2} {:>6.2} {:>6.2} {:>6.2} {:>6.2} {:>5} {:>7.3} {:>4} {:>7.3} {:>7.3}",
        s.s0, s.s1, s.eden, s.old, s.meta, s.ccs, s.ygc, s.ygct, s.fgc, s.fgct, s.gct
    )
}

/// GC 表格头
fn gc_table_header() -> String {
    format!(
        "{:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>5} {:>7} {:>4} {:>7} {:>7}",
        "S0%", "S1%", "Eden%", "Old%", "Meta%", "CCS%", "YGC", "YGCT", "FGC", "FGCT", "GCT"
    )
}

// ─── Thread 结构化数据 ──────────────────────────────────────────

/// 单线程信息
#[derive(Debug, Clone)]
pub struct ThreadInfo {
    pub name: String,
    pub id: u32,
    pub daemon: bool,
    pub state: ThreadState,
    pub priority: u32,
}

/// 线程状态枚举
#[derive(Debug, Clone, PartialEq)]
pub enum ThreadState {
    Runnable,
    Blocked,
    Waiting,
    TimedWaiting,
    Sleeping,
    Unknown(String),
}

impl ThreadState {
    pub fn symbol(&self) -> &str {
        match self {
            ThreadState::Runnable => "🟢",
            ThreadState::Blocked => "🔴",
            ThreadState::Waiting | ThreadState::TimedWaiting => "🟡",
            ThreadState::Sleeping => "🔵",
            ThreadState::Unknown(_) => "⚪",
        }
    }

    pub fn label(&self) -> &str {
        match self {
            ThreadState::Runnable => "RUNNABLE",
            ThreadState::Blocked => "BLOCKED",
            ThreadState::Waiting => "WAITING",
            ThreadState::TimedWaiting => "TIMED_WAITING",
            ThreadState::Sleeping => "SLEEPING",
            ThreadState::Unknown(s) => s,
        }
    }

    fn from_jcmd(s: &str) -> Self {
        match s.trim() {
            "RUNNABLE" => ThreadState::Runnable,
            "BLOCKED" => ThreadState::Blocked,
            "WAITING" => ThreadState::Waiting,
            "TIMED_WAITING" => ThreadState::TimedWaiting,
            "SLEEPING" => ThreadState::Sleeping,
            other => ThreadState::Unknown(other.to_string()),
        }
    }
}

/// 线程快照汇总
#[derive(Debug, Clone)]
pub struct ThreadSnapshot {
    pub threads: Vec<ThreadInfo>,
    pub total: usize,
    pub daemon_count: usize,
    pub blocked_count: usize,
    pub deadlock_detected: bool,
    pub deadlock_threads: Vec<String>,
}

/// 解析 jcmd Thread.print 输出为结构化数据
fn parse_threads_output(output: &str) -> ThreadSnapshot {
    let mut threads = Vec::new();
    let mut deadlock_detected = false;
    let mut deadlock_threads = Vec::new();
    let mut in_deadlock_section = false;

    let lines: Vec<&str> = output.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim_start();

        if line.contains("Found one Java-level deadlock") {
            deadlock_detected = true;
            in_deadlock_section = true;
            i += 1;
            continue;
        }

        if in_deadlock_section {
            if line.starts_with('"') {
                if let Some(last_q) = line.rfind('"') {
                    if last_q > 0 {
                        deadlock_threads.push(line[1..last_q].to_string());
                    }
                }
            } else if line.is_empty() {
                in_deadlock_section = false;
            }
            i += 1;
            continue;
        }

        if let Some(stripped) = line.strip_prefix('"') {
            let mut info = ThreadInfo {
                name: String::new(),
                id: 0,
                daemon: false,
                state: ThreadState::Unknown("UNKNOWN".to_string()),
                priority: 5,
            };

            if let Some(end_quote) = stripped.find('"') {
                info.name = stripped[..end_quote].to_string();
            }

            if let Some(hash_pos) = line.find('#') {
                let after_hash = &line[hash_pos + 1..];
                let num_str: String = after_hash.chars().take_while(|c| c.is_ascii_digit()).collect();
                info.id = num_str.parse().unwrap_or(0);
            }

            if line.contains(" daemon ") {
                info.daemon = true;
            }

            if let Some(prio_pos) = line.find("prio=") {
                let after_prio = &line[prio_pos + 5..];
                let num_str: String = after_prio.chars().take_while(|c| c.is_ascii_digit()).collect();
                info.priority = num_str.parse().unwrap_or(5);
            }

            for state_line in lines.iter().take(std::cmp::min(i + 10, lines.len())).skip(i + 1) {
                let state_line = state_line.trim();
                if let Some(state_pos) = state_line.find("java.lang.Thread.State: ") {
                    let state_str = &state_line[state_pos + 24..];
                    info.state = ThreadState::from_jcmd(state_str);
                    break;
                }
            }

            threads.push(info);
        }

        i += 1;
    }

    let total = threads.len();
    let daemon_count = threads.iter().filter(|t| t.daemon).count();
    let blocked_count = threads
        .iter()
        .filter(|t| t.state == ThreadState::Blocked)
        .count();

    ThreadSnapshot {
        threads,
        total,
        daemon_count,
        blocked_count,
        deadlock_detected,
        deadlock_threads,
    }
}

// ─── 公共 API ───────────────────────────────────────────────────

/// GC 概览（jstat -gcutil），结构化解析后格式化输出
pub fn gc(pid: u32) -> Result<()> {
    println!("JVM GC 监控 (PID: {})", pid);
    println!("按 Ctrl+C 停止");
    println!();

    let output = Command::new("jstat")
        .args(["-gcutil", &pid.to_string(), "1000"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::new(format!("jstat 执行失败: {}", stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    println!("{}", gc_table_header());
    println!("{}", "-".repeat(78));

    let mut snapshot_count = 0u32;
    for line in stdout.lines() {
        if line.trim().is_empty() || line.starts_with('S') || line.starts_with('-') {
            continue;
        }
        if let Some(snapshot) = parse_gc_line(line) {
            println!("{}", format_gc_snapshot(&snapshot));
            snapshot_count += 1;
        }
    }

    if snapshot_count == 0 {
        println!("（无数据）");
    }

    println!();
    println!("列说明: S0/S1=Survivor, Eden=新生代, Old=老年代, Meta=元空间, CCS=压缩类空间");
    println!("        YGC/YGCT=Young GC 次数/耗时, FGC/FGCT=Full GC 次数/耗时, GCT=总耗时");

    Ok(())
}

/// 线程概览（jcmd Thread.print），结构化解析后格式化输出
pub fn threads(pid: u32) -> Result<()> {
    println!("JVM 线程信息 (PID: {})", pid);
    println!();

    let args: Vec<String> = vec![pid.to_string(), "Thread.print".to_string()];
    let output = Command::new("jcmd").args(&args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::new(format!("jcmd 执行失败: {}", stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let snapshot = parse_threads_output(&stdout);

    println!("线程统计:");
    println!("  总线程数: {}", snapshot.total);
    println!("  守护线程: {}", snapshot.daemon_count);
    if snapshot.blocked_count > 0 {
        println!("  ⚠️  BLOCKED: {}", snapshot.blocked_count);
    }
    if snapshot.deadlock_detected {
        println!("  🔴 检测到死锁! 涉及线程: {:?}", snapshot.deadlock_threads);
    } else {
        println!("  死锁检测: ✅ 无");
    }

    println!();
    println!("详细信息:");

    let runnable: Vec<_> = snapshot.threads.iter().filter(|t| t.state == ThreadState::Runnable).collect();
    let blocked: Vec<_> = snapshot.threads.iter().filter(|t| t.state == ThreadState::Blocked).collect();
    let waiting: Vec<_> = snapshot.threads.iter().filter(|t| t.state == ThreadState::Waiting || t.state == ThreadState::TimedWaiting).collect();
    let other: Vec<_> = snapshot.threads.iter().filter(|t| {
        t.state != ThreadState::Runnable
            && t.state != ThreadState::Blocked
            && t.state != ThreadState::Waiting
            && t.state != ThreadState::TimedWaiting
    }).collect();

    if !blocked.is_empty() {
        println!();
        println!("🔴 BLOCKED ({})", blocked.len());
        for t in &blocked {
            let daemon = if t.daemon { " [daemon]" } else { "" };
            println!("  {} #{} (prio={}){}", t.name, t.id, t.priority, daemon);
        }
    }

    if !runnable.is_empty() {
        println!();
        println!("🟢 RUNNABLE ({})", runnable.len());
        for t in &runnable {
            let daemon = if t.daemon { " [daemon]" } else { "" };
            println!("  {} #{} (prio={}){}", t.name, t.id, t.priority, daemon);
        }
    }

    if !waiting.is_empty() {
        println!();
        println!("🟡 WAITING ({})", waiting.len());
        for t in &waiting {
            let daemon = if t.daemon { " [daemon]" } else { "" };
            println!("  {} #{} {} (prio={}){}", t.name, t.id, t.state.label(), t.priority, daemon);
        }
    }

    if !other.is_empty() {
        println!();
        println!("⚪ OTHER ({})", other.len());
        for t in &other {
            let daemon = if t.daemon { " [daemon]" } else { "" };
            println!("  {} #{} {} (prio={}){}", t.name, t.id, t.state.label(), t.priority, daemon);
        }
    }

    Ok(())
}

// ─── TUI 模块 ───────────────────────────────────────────────────
// 轻量级 TUI：仅用 crossterm + ANSI 转义码，不依赖 ratatui

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use std::io::{self, Write};
use std::time::{Duration, Instant};

/// 获取终端尺寸 (cols, rows)
fn terminal_size() -> (u16, u16) {
    crossterm::terminal::size().unwrap_or((80, 24))
}

/// ANSI 颜色代码
mod ansi {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const CYAN: &str = "\x1b[36m";
    pub const WHITE_ON_RED: &str = "\x1b[37;41m";
    pub const WHITE_ON_BLUE: &str = "\x1b[37;44m";
    pub const GRAY: &str = "\x1b[90m";
}

/// 渲染 GC TUI 的一帧
fn render_gc_frame(pid: u32, snapshots: &[GcSnapshot], paused: bool) -> io::Result<()> {
    let (_cols, rows) = terminal_size();
    let mut out = io::stdout();
    write!(out, "\x1b[2J\x1b[H")?;

    // 标题行
    let status = if paused { "已暂停" } else { "刷新中" };
    writeln!(out, "{}{}╔══ JVM GC 监控 (PID: {}) [{}] ══╗{}",
        ansi::CYAN, ansi::BOLD, pid, status, ansi::RESET)?;
    writeln!(out)?;

    // 表头
    writeln!(out, "{}{}{:<8} {:<8} {:<8} {:<8} {:<8} {:>6} {:>8} {:>4} {:>8}{}",
        ansi::YELLOW, ansi::BOLD,
        "S0%", "S1%", "Eden%", "Old%", "Meta%", "YGC", "YGCT", "FGC", "FGCT",
        ansi::RESET)?;

    // 数据行
    let max_rows = rows.saturating_sub(8) as usize;
    let start = snapshots.len().saturating_sub(max_rows);
    for s in &snapshots[start..] {
        writeln!(out, "{:<8.2} {:<8.2} {:<8.2} {:<8.2} {:<8.2} {:>6} {:>8.3} {:>4} {:>8.3}",
            s.s0, s.s1, s.eden, s.old, s.meta, s.ygc, s.ygct, s.fgc, s.fgct)?;
    }

    // 空行填充
    let shown = snapshots[start..].len();
    for _ in shown..max_rows {
        writeln!(out)?;
    }

    // 状态栏
    writeln!(out, "{}{}╔══════════════════════════════════════════════════════════════════╗{}",
        ansi::CYAN, ansi::BOLD, ansi::RESET)?;
    write!(out, "{}{} q {}{} 退出  {}{} p {}{} 暂停/继续  采样数: {}",
        ansi::CYAN, ansi::BOLD,
        ansi::WHITE_ON_RED, ansi::RESET,
        ansi::CYAN, ansi::BOLD,
        ansi::WHITE_ON_BLUE, ansi::RESET,
        snapshots.len())?;
    writeln!(out, "{}{}╚══════════════════════════════════════════════════════════════════╝{}",
        ansi::CYAN, ansi::BOLD, ansi::RESET)?;
    out.flush()?;
    Ok(())
}

/// 启动 GC 监控 TUI
pub fn gc_tui(pid: u32) -> Result<()> {
    if !atty::is(atty::Stream::Stdout) {
        return gc(pid);
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let mut snapshots: Vec<GcSnapshot> = Vec::new();
    let mut paused = false;
    let mut should_quit = false;
    let tick_rate = Duration::from_secs(1);
    let mut last_refresh = Instant::now();

    loop {
        if !paused && last_refresh.elapsed() >= tick_rate {
            if let Ok(output) = Command::new("jstat")
                .args(["-gcutil", &pid.to_string(), "1000"])
                .output()
            {
                if output.status.success() {
                    let stdout_str = String::from_utf8_lossy(&output.stdout);
                    for line in stdout_str.lines() {
                        if line.trim().is_empty() || line.starts_with('S') || line.starts_with('-') {
                            continue;
                        }
                        if let Some(snapshot) = parse_gc_line(line) {
                            snapshots.push(snapshot);
                        }
                    }
                }
            }
            last_refresh = Instant::now();
        }

        render_gc_frame(pid, &snapshots, paused)?;

        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let KeyCode::Char('q') = key.code {
                        should_quit = true;
                    } else if let KeyCode::Char('p') = key.code {
                        paused = !paused;
                    }
                }
            }
        }

        if should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen)?;
    Ok(())
}

/// 渲染线程 TUI 的一帧
fn render_threads_frame(pid: u32, snapshot: &Option<ThreadSnapshot>) -> io::Result<()> {
    let (_cols, rows) = terminal_size();
    let mut out = io::stdout();
    write!(out, "\x1b[2J\x1b[H")?;

    let total = snapshot.as_ref().map_or(0, |s| s.total);
    writeln!(out, "{}{}╔══ JVM 线程信息 (PID: {}) — 共 {} 个线程 ══╗{}",
        ansi::CYAN, ansi::BOLD, pid, total, ansi::RESET)?;
    writeln!(out)?;

    if let Some(snap) = snapshot {
        // 表头
        writeln!(out, "{}{}{:<4} {:<30} {:<8} {:<6} {:<6}{}",
            ansi::YELLOW, ansi::BOLD,
            "状态", "线程名", "ID", "守护", "优先级",
            ansi::RESET)?;

        // 数据行
        let max_rows = rows.saturating_sub(8) as usize;
        let display_count = snap.threads.len().min(max_rows);
        for t in snap.threads.iter().take(display_count) {
            let state_color = match t.state {
                ThreadState::Runnable => ansi::GREEN,
                ThreadState::Blocked => ansi::RED,
                ThreadState::Waiting | ThreadState::TimedWaiting => ansi::YELLOW,
                ThreadState::Sleeping => ansi::BLUE,
                _ => ansi::GRAY,
            };
            let daemon = if t.daemon { "✔" } else { "" };
            writeln!(out, "{}{}{} {:<30} {:<8} {:<6} {:<6}{}",
                state_color, t.state.symbol(), ansi::RESET,
                t.name, format!("#{}", t.id), daemon, t.priority,
                ansi::RESET)?;
        }

        // 空行填充
        for _ in display_count..max_rows {
            writeln!(out)?;
        }

        // 统计栏
        let blocked = if snap.blocked_count > 0 {
            format!(" ⚠️ BLOCKED: {}", snap.blocked_count)
        } else {
            String::new()
        };
        let deadlock = if snap.deadlock_detected {
            " 🔴 死锁!".to_string()
        } else {
            String::new()
        };
        writeln!(out, "{}{}╔══════════════════════════════════════════════════════════════════╗{}",
            ansi::CYAN, ansi::BOLD, ansi::RESET)?;
        write!(out, "{}{} q {}{} 退出  守护线程: {}{}{}{}",
            ansi::CYAN, ansi::BOLD,
            ansi::WHITE_ON_RED, ansi::RESET,
            snap.daemon_count, blocked, deadlock, ansi::RESET)?;
        writeln!(out, "{}{}╚══════════════════════════════════════════════════════════════════╝{}",
            ansi::CYAN, ansi::BOLD, ansi::RESET)?;
    }
    out.flush()?;
    Ok(())
}

/// 启动线程列表 TUI
pub fn threads_tui(pid: u32) -> Result<()> {
    if !atty::is(atty::Stream::Stdout) {
        return threads(pid);
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let mut snapshot: Option<ThreadSnapshot> = None;
    let mut should_quit = false;
    let tick_rate = Duration::from_secs(2);
    let mut last_refresh = Instant::now();

    loop {
        if last_refresh.elapsed() >= tick_rate {
            let args: Vec<String> = vec![pid.to_string(), "Thread.print".to_string()];
            if let Ok(output) = Command::new("jcmd").args(&args).output() {
                if output.status.success() {
                    let stdout_str = String::from_utf8_lossy(&output.stdout);
                    snapshot = Some(parse_threads_output(&stdout_str));
                }
            }
            last_refresh = Instant::now();
        }

        render_threads_frame(pid, &snapshot)?;

        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let KeyCode::Char('q') = key.code {
                        should_quit = true;
                    }
                }
            }
        }

        if should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gc_line() {
        let line = "  0.00  45.23  67.89  12.34  95.67  92.10   125   1.234    3   0.567  1.801";
        let snapshot = parse_gc_line(line);
        assert!(snapshot.is_some());
        let s = snapshot.unwrap();
        assert!((s.s0 - 0.00).abs() < 0.001);
        assert!((s.s1 - 45.23).abs() < 0.001);
        assert!((s.eden - 67.89).abs() < 0.001);
        assert!((s.old - 12.34).abs() < 0.001);
        assert_eq!(s.ygc, 125);
        assert!((s.ygct - 1.234).abs() < 0.001);
        assert_eq!(s.fgc, 3);
    }

    #[test]
    fn test_parse_gc_line_with_pid() {
        let line = "12345   0.00  45.23  67.89  12.34  95.67  92.10   125   1.234    3   0.567  1.801";
        let snapshot = parse_gc_line(line);
        assert!(snapshot.is_some());
        let s = snapshot.unwrap();
        assert_eq!(s.ygc, 125);
    }

    #[test]
    fn test_parse_gc_line_insufficient_fields() {
        let line = "  0.00  45.23  67.89";
        assert!(parse_gc_line(line).is_none());
    }

    #[test]
    fn test_thread_state_from_jcmd() {
        assert_eq!(ThreadState::from_jcmd("RUNNABLE"), ThreadState::Runnable);
        assert_eq!(ThreadState::from_jcmd("BLOCKED"), ThreadState::Blocked);
        assert_eq!(ThreadState::from_jcmd("WAITING"), ThreadState::Waiting);
        assert_eq!(ThreadState::from_jcmd("TIMED_WAITING"), ThreadState::TimedWaiting);
        assert!(matches!(ThreadState::from_jcmd("UNKNOWN"), ThreadState::Unknown(_)));
    }

    #[test]
    fn test_parse_threads_output_simple() {
        let output = r#""main" #1 daemon prio=5 java.lang.Thread.State: RUNNABLE
"pool-1-thread-1" #12 prio=5 java.lang.Thread.State: WAITING
"worker-1" #15 daemon prio=5 java.lang.Thread.State: BLOCKED
"#;
        let snapshot = parse_threads_output(output);
        assert_eq!(snapshot.total, 3);
        assert_eq!(snapshot.daemon_count, 2);
        assert_eq!(snapshot.blocked_count, 1);
        assert!(!snapshot.deadlock_detected);
    }

    #[test]
    fn test_parse_threads_output_with_deadlock() {
        let output = r#""main" #1 prio=5 java.lang.Thread.State: BLOCKED
"t2" #2 prio=5 java.lang.Thread.State: BLOCKED

Found one Java-level deadlock:
=============================
"main":
  waiting to lock monitor 0x00007f... (a java.lang.Object),
  held by thread 2
"t2":
  waiting to lock monitor 0x00007f... (a java.lang.Object),
  held by thread 1
"#;
        let snapshot = parse_threads_output(output);
        assert!(snapshot.deadlock_detected, "deadlock_detected should be true");
        assert_eq!(snapshot.deadlock_threads.len(), 2);
        assert!(snapshot.deadlock_threads.contains(&"main".to_string()));
        assert!(snapshot.deadlock_threads.contains(&"t2".to_string()));
    }

    #[test]
    fn test_thread_state_symbols() {
        assert_eq!(ThreadState::Runnable.symbol(), "🟢");
        assert_eq!(ThreadState::Blocked.symbol(), "🔴");
        assert_eq!(ThreadState::Waiting.symbol(), "🟡");
        assert_eq!(ThreadState::TimedWaiting.symbol(), "🟡");
        assert_eq!(ThreadState::Sleeping.symbol(), "🔵");
    }
}
