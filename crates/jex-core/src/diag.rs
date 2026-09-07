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

/// 从 jstat 输出文本中解析所有 GC 快照
fn parse_jstat_all(output: &str) -> Vec<GcSnapshot> {
    output
        .lines()
        .filter(|line| {
            let line = line.trim();
            !line.is_empty() && !line.starts_with('S') && !line.starts_with('-')
        })
        .filter_map(parse_gc_line)
        .collect()
}

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
                let num_str: String = after_hash
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                info.id = num_str.parse().unwrap_or(0);
            }

            if line.contains(" daemon ") {
                info.daemon = true;
            }

            if let Some(prio_pos) = line.find("prio=") {
                let after_prio = &line[prio_pos + 5..];
                let num_str: String = after_prio
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                info.priority = num_str.parse().unwrap_or(5);
            }

            for state_line in lines
                .iter()
                .take(std::cmp::min(i + 10, lines.len()))
                .skip(i + 1)
            {
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

    let snapshots = parse_jstat_all(&stdout);
    for s in &snapshots {
        println!("{}", format_gc_snapshot(s));
    }
    if snapshots.is_empty() {
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

    let mut runnable = Vec::new();
    let mut blocked = Vec::new();
    let mut waiting = Vec::new();
    let mut other = Vec::new();
    for t in &snapshot.threads {
        match t.state {
            ThreadState::Runnable => runnable.push(t),
            ThreadState::Blocked => blocked.push(t),
            ThreadState::Waiting | ThreadState::TimedWaiting => waiting.push(t),
            _ => other.push(t),
        }
    }
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
            println!(
                "  {} #{} {} (prio={}){}",
                t.name,
                t.id,
                t.state.label(),
                t.priority,
                daemon
            );
        }
    }

    if !other.is_empty() {
        println!();
        println!("⚪ OTHER ({})", other.len());
        for t in &other {
            let daemon = if t.daemon { " [daemon]" } else { "" };
            println!(
                "  {} #{} {} (prio={}){}",
                t.name,
                t.id,
                t.state.label(),
                t.priority,
                daemon
            );
        }
    }

    Ok(())
}

// ─── TUI 模块 ───────────────────────────────────────────────────
// 轻量级 TUI：仅用 crossterm + ANSI 转义码，不依赖 ratatui

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use std::io::{self, Write};
use std::time::{Duration, Instant};

/// RAII guard：确保 TUI 退出时恢复终端原始状态（即使中途 panic）
struct TerminalGuard {
    stdout: io::Stdout,
}

impl TerminalGuard {
    fn enter() -> std::io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        Ok(Self { stdout })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.stdout, LeaveAlternateScreen);
    }
}

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
    writeln!(
        out,
        "{}{}╔══ JVM GC 监控 (PID: {}) [{}] ══╗{}",
        ansi::CYAN,
        ansi::BOLD,
        pid,
        status,
        ansi::RESET
    )?;
    writeln!(out)?;

    // 表头
    writeln!(
        out,
        "{}{}{:<8} {:<8} {:<8} {:<8} {:<8} {:<8} {:>6} {:>8} {:>4} {:>8}{}",
        ansi::YELLOW,
        ansi::BOLD,
        "S0%",
        "S1%",
        "Eden%",
        "Old%",
        "Meta%",
        "CCS%",
        "YGC",
        "YGCT",
        "FGC",
        "FGCT",
        ansi::RESET
    )?;

    // 数据行
    let max_rows = rows.saturating_sub(8) as usize;
    let start = snapshots.len().saturating_sub(max_rows);
    for s in &snapshots[start..] {
        writeln!(
            out,
            "{:<8.2} {:<8.2} {:<8.2} {:<8.2} {:<8.2} {:>6} {:>8.3} {:>4} {:>8.3}",
            s.s0, s.s1, s.eden, s.old, s.meta, s.ygc, s.ygct, s.fgc, s.fgct
        )?;
    }

    // 空行填充
    let shown = snapshots[start..].len();
    for _ in shown..max_rows {
        writeln!(out)?;
    }

    // 状态栏
    writeln!(
        out,
        "{}{}╔══════════════════════════════════════════════════════════════════╗{}",
        ansi::CYAN,
        ansi::BOLD,
        ansi::RESET
    )?;
    write!(
        out,
        "{}{} q {}{} 退出  {}{} p {}{} 暂停/继续  采样数: {}",
        ansi::CYAN,
        ansi::BOLD,
        ansi::WHITE_ON_RED,
        ansi::RESET,
        ansi::CYAN,
        ansi::BOLD,
        ansi::WHITE_ON_BLUE,
        ansi::RESET,
        snapshots.len()
    )?;
    writeln!(
        out,
        "{}{}╚══════════════════════════════════════════════════════════════════╝{}",
        ansi::CYAN,
        ansi::BOLD,
        ansi::RESET
    )?;
    out.flush()?;
    Ok(())
}

/// 启动 GC 监控 TUI
pub fn gc_tui(pid: u32) -> Result<()> {
    if !atty::is(atty::Stream::Stdout) {
        return gc(pid);
    }

    let _guard =
        TerminalGuard::enter().map_err(|e| Error::new(format!("TUI 初始化失败: {e:?}")))?;

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
                    snapshots.extend(parse_jstat_all(&stdout_str));
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

    Ok(())
}

/// 渲染线程 TUI 的一帧
fn render_threads_frame(pid: u32, snapshot: &Option<ThreadSnapshot>) -> io::Result<()> {
    let (_cols, rows) = terminal_size();
    let mut out = io::stdout();
    write!(out, "\x1b[2J\x1b[H")?;

    let total = snapshot.as_ref().map_or(0, |s| s.total);
    writeln!(
        out,
        "{}{}╔══ JVM 线程信息 (PID: {}) — 共 {} 个线程 ══╗{}",
        ansi::CYAN,
        ansi::BOLD,
        pid,
        total,
        ansi::RESET
    )?;
    writeln!(out)?;

    if let Some(snap) = snapshot {
        // 表头
        writeln!(
            out,
            "{}{}{:<4} {:<30} {:<8} {:<6} {:<6}{}",
            ansi::YELLOW,
            ansi::BOLD,
            "状态",
            "线程名",
            "ID",
            "守护",
            "优先级",
            ansi::RESET
        )?;

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
            writeln!(
                out,
                "{}{}{} {:<30} {:<8} {:<6} {:<6}{}",
                state_color,
                t.state.symbol(),
                ansi::RESET,
                t.name,
                format!("#{}", t.id),
                daemon,
                t.priority,
                ansi::RESET
            )?;
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
        writeln!(
            out,
            "{}{}╔══════════════════════════════════════════════════════════════════╗{}",
            ansi::CYAN,
            ansi::BOLD,
            ansi::RESET
        )?;
        write!(
            out,
            "{}{} q {}{} 退出  守护线程: {}{}{}{}",
            ansi::CYAN,
            ansi::BOLD,
            ansi::WHITE_ON_RED,
            ansi::RESET,
            snap.daemon_count,
            blocked,
            deadlock,
            ansi::RESET
        )?;
        writeln!(
            out,
            "{}{}╚══════════════════════════════════════════════════════════════════╝{}",
            ansi::CYAN,
            ansi::BOLD,
            ansi::RESET
        )?;
    }
    out.flush()?;
    Ok(())
}

/// 启动线程列表 TUI
pub fn threads_tui(pid: u32) -> Result<()> {
    if !atty::is(atty::Stream::Stdout) {
        return threads(pid);
    }

    let _guard =
        TerminalGuard::enter().map_err(|e| Error::new(format!("TUI 初始化失败: {e:?}")))?;

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

    Ok(())
}
// ─── Heap 概览 ───────────────────────────────────────────────

/// 堆内存概览（jstat -gc 解析结果）
#[derive(Debug, Clone)]
pub struct HeapOverview {
    pub heap_used: u64,
    pub heap_max: u64,
    pub eden_used: u64,
    pub survivor_used: u64,
    pub old_gen_used: u64,
    pub meta_used: u64,
    pub gc_count: u64,
    pub gc_pause_ms: f64,
}

/// 解析 jstat -gc 输出的容量/利用率列，返回 HeapOverview
///
/// jstat -gc 输出格式（KB）：
/// ```text
/// S0C    S1C  S0U    S1U     EC      EU       OC       OU      MC     MU
/// 10240 10240 0.0  5120.0 81920  40960.0  204800  102400.0 524288 262144.0 ...
/// ```
fn parse_jstat_gc(output: &str) -> Option<HeapOverview> {
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("S0C") || trimmed.starts_with("-") {
            continue;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 12 {
            continue;
        }

        let parse_kb = |s: &str| -> f64 {
            if s == "-" {
                0.0
            } else {
                s.parse::<f64>().unwrap_or(0.0)
            }
        };

        // 容量列（KB）：S0C, S1C, EC, OC, MC
        let s0c = parse_kb(parts[0]);
        let s1c = parse_kb(parts[1]);
        let ec = parse_kb(parts[4]);
        let oc = parse_kb(parts[6]);
        let _mc = parse_kb(parts[8]);

        // 利用率列（KB）：S0U, S1U, EU, OU, MU
        let s0u = parse_kb(parts[2]);
        let s1u = parse_kb(parts[3]);
        let eu = parse_kb(parts[5]);
        let ou = parse_kb(parts[7]);
        let mu = parse_kb(parts[9]);

        let parse_u64 = |s: &str| -> u64 {
            if s == "-" {
                0
            } else {
                s.parse::<u64>().unwrap_or(0)
            }
        };

        // jstat -gc header: S0C S1C S0U S1U EC EU OC OU MC MU CCS CCSC YGC YGCT FGC FGCT GCT
        let ygc = parse_u64(parts[12]);
        let ygct = parse_kb(parts[13]);
        let fgc = parse_u64(parts[14]);
        let fgct = parse_kb(parts[15]);

        let survivor_used = (s0u + s1u) as u64;
        let old_gen_used = ou as u64;
        let eden_used = eu as u64;
        let meta_used = mu as u64;
        let heap_used = survivor_used + eden_used + old_gen_used;
        let heap_max = ((s0c + s1c + ec + oc) as u64).max(1);
        let gc_count = ygc + fgc;
        let gc_pause_ms = (ygct + fgct) * 1000.0;

        return Some(HeapOverview {
            heap_used,
            heap_max,
            eden_used,
            survivor_used,
            old_gen_used,
            meta_used,
            gc_count,
            gc_pause_ms,
        });
    }
    None
}

/// 获取堆内存概览：运行 jstat -gc 并解析输出
pub fn heap_overview(pid: u32) -> Result<HeapOverview> {
    let output = Command::new("jstat")
        .args(["-gc", &pid.to_string()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::new(format!("jstat -gc 执行失败: {}", stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_jstat_gc(&stdout).ok_or_else(|| Error::new("无法解析 jstat -gc 输出".to_string()))
}

/// 格式化显示堆概览
pub fn display_heap(pid: u32) -> Result<()> {
    let overview = heap_overview(pid)?;

    println!("JVM 堆概览 (PID: {})", pid);
    println!();

    // 堆使用进度条
    let used_pct = if overview.heap_max > 0 {
        overview.heap_used as f64 / overview.heap_max as f64 * 100.0
    } else {
        0.0
    };
    let bar_len = 40;
    let filled = ((used_pct / 100.0) * bar_len as f64) as usize;
    let bar: String = "=".repeat(filled) + &"-".repeat((bar_len as usize).saturating_sub(filled));
    let color = if used_pct > 80.0 {
        ansi::RED
    } else if used_pct > 60.0 {
        ansi::YELLOW
    } else {
        ansi::GREEN
    };
    println!(
        "  堆使用: {}[{}]{} {:.1}%  ({:.1} MB / {:.1} MB)",
        color,
        bar,
        ansi::RESET,
        used_pct,
        overview.heap_used as f64 / 1024.0,
        overview.heap_max as f64 / 1024.0
    );
    println!();
    println!("  Eden:    {:.1} MB", overview.eden_used as f64 / 1024.0);
    println!(
        "  Survivor:{:.1} MB",
        overview.survivor_used as f64 / 1024.0
    );
    println!("  Old Gen: {:.1} MB", overview.old_gen_used as f64 / 1024.0);
    println!("  Meta:    {:.1} MB", overview.meta_used as f64 / 1024.0);
    println!();
    println!(
        "  GC 次数: {}  累计暂停: {:.1} ms",
        overview.gc_count, overview.gc_pause_ms
    );
    println!();

    Ok(())
}

// ─── Top 快照 / TUI ───────────────────────────────────────────

/// Top 实时快照汇总
#[derive(Debug, Clone)]
pub struct TopSnapshot {
    pub heap_used_pct: f64,
    pub gc_count: u64,
    pub gc_pause_ms: f64,
    pub thread_count: usize,
    pub daemon_count: usize,
}

/// 获取线程信息（返回 ThreadSnapshot，不打印）
pub fn threads_info(pid: u32) -> Result<ThreadSnapshot> {
    let args: Vec<String> = vec![pid.to_string(), "Thread.print".to_string()];
    let output = Command::new("jcmd").args(&args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::new(format!("jcmd 执行失败: {}", stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_threads_output(&stdout))
}

/// 生成 Top 快照：聚合堆 + 线程信息
pub fn top_snapshot(pid: u32) -> Result<TopSnapshot> {
    let heap = heap_overview(pid)?;
    let threads = threads_info(pid)?;

    let heap_used_pct = if heap.heap_max > 0 {
        heap.heap_used as f64 / heap.heap_max as f64 * 100.0
    } else {
        0.0
    };

    Ok(TopSnapshot {
        heap_used_pct,
        gc_count: heap.gc_count,
        gc_pause_ms: heap.gc_pause_ms,
        thread_count: threads.total,
        daemon_count: threads.daemon_count,
    })
}

/// 渲染 Top 快照的一帧
fn render_top_frame(pid: u32, snapshot: &TopSnapshot) -> io::Result<()> {
    let mut out = io::stdout();
    write!(out, "\x1b[2J\x1b[H")?;

    writeln!(
        out,
        "{}{}╔══ JVM Top (PID: {}) ══╗{}",
        ansi::CYAN,
        ansi::BOLD,
        pid,
        ansi::RESET
    )?;
    writeln!(out)?;

    let bar_len = 40;
    let filled = ((snapshot.heap_used_pct / 100.0) * bar_len as f64) as usize;
    let bar: String = "=".repeat(filled) + &"-".repeat((bar_len as usize).saturating_sub(filled));
    let color = if snapshot.heap_used_pct > 80.0 {
        ansi::RED
    } else if snapshot.heap_used_pct > 60.0 {
        ansi::YELLOW
    } else {
        ansi::GREEN
    };
    writeln!(
        out,
        "  堆: {}[{}]{} {:.1}%",
        color,
        bar,
        ansi::RESET,
        snapshot.heap_used_pct
    )?;
    writeln!(out)?;
    writeln!(
        out,
        "  GC: {} 次  暂停: {:.1} ms",
        snapshot.gc_count, snapshot.gc_pause_ms
    )?;
    writeln!(
        out,
        "  线程: {} (守护: {})",
        snapshot.thread_count, snapshot.daemon_count
    )?;
    writeln!(out)?;

    writeln!(
        out,
        "{}{}╔══════════════════════════════════════════════════════════════╗{}",
        ansi::CYAN,
        ansi::BOLD,
        ansi::RESET
    )?;
    writeln!(
        out,
        "{}{}  q {}{} 退出",
        ansi::CYAN,
        ansi::BOLD,
        ansi::WHITE_ON_RED,
        ansi::RESET
    )?;
    writeln!(
        out,
        "{}{}╚══════════════════════════════════════════════════════════════╝{}",
        ansi::CYAN,
        ansi::BOLD,
        ansi::RESET
    )?;
    out.flush()?;
    Ok(())
}

/// Top 实时面板：非 TTY 打印一次快照，TTY 进入 raw mode 每秒刷新
pub fn top_tui(pid: u32) -> Result<()> {
    if !atty::is(atty::Stream::Stdout) {
        let snapshot = top_snapshot(pid)?;
        render_top_frame(pid, &snapshot)?;
        return Ok(());
    }

    let _guard =
        TerminalGuard::enter().map_err(|e| Error::new(format!("TUI 初始化失败: {e:?}")))?;

    let mut should_quit = false;
    let tick_rate = Duration::from_secs(1);

    loop {
        if let Ok(snapshot) = top_snapshot(pid) {
            let _ = render_top_frame(pid, &snapshot);
        }

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

    Ok(())
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
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
        let line =
            "12345   0.00  45.23  67.89  12.34  95.67  92.10   125   1.234    3   0.567  1.801";
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
        assert_eq!(
            ThreadState::from_jcmd("TIMED_WAITING"),
            ThreadState::TimedWaiting
        );
        assert!(matches!(
            ThreadState::from_jcmd("UNKNOWN"),
            ThreadState::Unknown(_)
        ));
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
        assert!(
            snapshot.deadlock_detected,
            "deadlock_detected should be true"
        );
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

    #[test]
    fn test_heap_overview_parse() {
        // jstat -gc header + data line (17 columns)
        let header = "S0C    S1C    S0U    S1U      EC       EU        OC         OU       MC     MU    CCS   CCSC   YGC     YGCT    FGC    FGCT      GCT";
        let data = "  10240.0 10240.0     0.0  5120.0  81920.0  40960.0  204800.0  102400.0  524288.0 262144.0 524288.0 262144.0   125    1.234     3    0.567   1.801";
        let full = format!("{}\n{}\n", header, data);
        let result = parse_jstat_gc(&full);
        assert!(result.is_some(), "should parse jstat -gc output");
        let h = result.unwrap();
        // heap_used = survivor + eden + old = (0+5120) + 40960 + 102400 = 148480 KB
        assert_eq!(h.heap_used, 148480);
        // heap_max = s0c+s1c+ec+oc = 10240+10240+81920+204800 = 307200 KB
        assert_eq!(h.heap_max, 307200);
        assert_eq!(h.eden_used, 40960);
        assert_eq!(h.survivor_used, 5120);
        assert_eq!(h.old_gen_used, 102400);
        assert_eq!(h.meta_used, 262144);
        assert_eq!(h.gc_count, 128); // 125 YGC + 3 FGC
        assert!((h.gc_pause_ms - 1801.0).abs() < 0.1); // (1.234+0.567)*1000
    }

    #[test]
    fn test_heap_overview_parse_empty() {
        let result = parse_jstat_gc("");
        assert!(result.is_none());
    }

    #[test]
    fn test_heap_overview_parse_header_only() {
        let output = "S0C    S1C    S0U    S1U      EC       EU        OC         OU";
        let result = parse_jstat_gc(output);
        assert!(result.is_none());
    }

    #[test]
    fn test_display_heap_parse_roundtrip() {
        let header = "S0C    S1C    S0U    S1U      EC       EU        OC         OU       MC     MU    CCS   CCSC   YGC     YGCT    FGC    FGCT      GCT";
        let data = "  20480.0 20480.0 10240.0 10240.0 163840.0 81920.0  409600.0  204800.0  524288.0 262144.0 524288.0 262144.0   500   10.000    10    2.000  12.000";
        let full = format!("{}\n{}\n", header, data);
        let h = parse_jstat_gc(&full).unwrap();
        assert_eq!(h.heap_used, 10240 + 10240 + 81920 + 204800);
        assert_eq!(h.heap_max, 20480 + 20480 + 163840 + 409600);
        assert_eq!(h.gc_count, 510); // 500 + 10
        assert!((h.gc_pause_ms - 12000.0).abs() < 0.1); // (10+2)*1000
    }

    #[test]
    fn test_top_snapshot_struct_fields() {
        let snap = TopSnapshot {
            heap_used_pct: 75.5,
            gc_count: 42,
            gc_pause_ms: 3.15,
            thread_count: 25,
            daemon_count: 18,
        };
        assert!((snap.heap_used_pct - 75.5).abs() < 0.001);
        assert_eq!(snap.gc_count, 42);
        assert!((snap.gc_pause_ms - 3.15).abs() < 0.001);
        assert_eq!(snap.thread_count, 25);
        assert_eq!(snap.daemon_count, 18);
    }

    #[test]
    fn test_thread_state_unknown_symbol() {
        let state = ThreadState::Unknown("CUSTOM".to_string());
        assert_eq!(state.symbol(), "⚪");
    }

    #[test]
    fn test_thread_state_labels() {
        assert_eq!(ThreadState::Runnable.label(), "RUNNABLE");
        assert_eq!(ThreadState::Blocked.label(), "BLOCKED");
        assert_eq!(ThreadState::Waiting.label(), "WAITING");
        assert_eq!(ThreadState::TimedWaiting.label(), "TIMED_WAITING");
        assert_eq!(ThreadState::Sleeping.label(), "SLEEPING");
        let unknown = ThreadState::Unknown("CUSTOM".to_string());
        assert_eq!(unknown.label(), "CUSTOM");
    }

    #[test]
    fn test_format_gc_snapshot() {
        let s = GcSnapshot {
            s0: 10.5,
            s1: 20.3,
            eden: 30.7,
            old: 40.1,
            meta: 50.9,
            ccs: 60.2,
            ygc: 100,
            ygct: 1.5,
            fgc: 5,
            fgct: 0.5,
            gct: 2.0,
        };
        let formatted = format_gc_snapshot(&s);
        assert!(formatted.contains("10.50"));
        assert!(formatted.contains("100"));
        assert!(formatted.contains("5"));
    }

    #[test]
    fn test_gc_table_header() {
        let header = gc_table_header();
        assert!(header.contains("S0%"));
        assert!(header.contains("S1%"));
        assert!(header.contains("Eden%"));
        assert!(header.contains("Old%"));
        assert!(header.contains("Meta%"));
        assert!(header.contains("CCS%"));
        assert!(header.contains("YGC"));
        assert!(header.contains("YGCT"));
        assert!(header.contains("FGC"));
        assert!(header.contains("FGCT"));
        assert!(header.contains("GCT"));
    }

    #[test]
    fn test_parse_jstat_all_multi_line() {
        let output = "S0     S1     Eden    Old\n"
            .to_string()
            + "  0.00  45.23  67.89  12.34  95.67  92.10   125   1.234    3   0.567  1.801\n"
            + " 10.00  20.00  30.00  40.00  50.00  60.00   200   2.000    5   1.000  3.000\n"
            + "--------------------------------------------------------------\n";
        let snapshots = parse_jstat_all(&output);
        assert_eq!(snapshots.len(), 2);
        assert!((snapshots[0].s0 - 0.00).abs() < 0.001);
        assert!((snapshots[1].s0 - 10.00).abs() < 0.001);
    }

    #[test]
    fn test_parse_jstat_all_empty() {
        let output = "";
        let snapshots = parse_jstat_all(output);
        assert!(snapshots.is_empty());
    }

    #[test]
    fn test_parse_gc_line_with_dashes() {
        let line = "  -  45.23  67.89  12.34  95.67  92.10   -   1.234    -   0.567  1.801";
        let snapshot = parse_gc_line(line);
        assert!(snapshot.is_some());
        let s = snapshot.unwrap();
        assert!((s.s0 - 0.0).abs() < 0.001);
        assert_eq!(s.ygc, 0);
        assert_eq!(s.fgc, 0);
    }

    #[test]
    fn test_thread_state_from_jcmd_sleeping() {
        assert_eq!(ThreadState::from_jcmd("SLEEPING"), ThreadState::Sleeping);
    }

    #[test]
    fn test_parse_threads_output_empty() {
        let output = "";
        let snapshot = parse_threads_output(output);
        assert_eq!(snapshot.total, 0);
        assert!(!snapshot.deadlock_detected);
    }

    #[test]
    fn test_parse_threads_output_sleeping() {
        // The parser looks for Thread.State on lines AFTER the header,
        // so we need two lines - the first thread's state is parsed from the second line.
        let output = r#""sleeping-thread" #10 prio=5
java.lang.Thread.State: SLEEPING
"#;
        let snapshot = parse_threads_output(output);
        assert_eq!(snapshot.total, 1);
        assert_eq!(snapshot.threads[0].state, ThreadState::Sleeping);
    }

    #[test]
    fn test_heap_overview_struct_fields() {
        let h = HeapOverview {
            heap_used: 1000,
            heap_max: 2000,
            eden_used: 300,
            survivor_used: 100,
            old_gen_used: 600,
            meta_used: 200,
            gc_count: 10,
            gc_pause_ms: 5.5,
        };
        assert_eq!(h.heap_used, 1000);
        assert_eq!(h.gc_count, 10);
    }
}
