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
    /// Survivor 0 使用率 (%)
    pub s0: f64,
    /// Survivor 1 使用率 (%)
    pub s1: f64,
    /// Eden 区使用率 (%)
    pub eden: f64,
    /// 老年代使用率 (%)
    pub old: f64,
    /// 元空间使用率 (%)
    pub meta: f64,
    /// 压缩类空间使用率 (%)
    pub ccs: f64,
    /// Young GC 次数
    pub ygc: u64,
    /// Young GC 耗时 (s)
    pub ygct: f64,
    /// Full GC 次数
    pub fgc: u64,
    /// Full GC 耗时 (s)
    pub fgct: f64,
    /// 总 GC 耗时 (s)
    pub gct: f64,
}

/// 解析 jstat -gcutil 单行数据
fn parse_gc_line(line: &str) -> Option<GcSnapshot> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    // jstat -gcutil 输出格式：
    //   S0     S1     E      O      M     CCS    YGC   YGCT    FGC   FGCT    GCT
    //  0.00  45.23  67.89  12.34  95.67  92.10   125   1.234    3   0.567  1.801
    if parts.len() < 11 {
        return None;
    }

    // 过滤出数字列（跳过可能的 PID 前缀）
    let nums: Vec<&str> = parts
        .iter()
        .filter(|p| p.parse::<f64>().is_ok() || **p == "-")
        .cloned()
        .collect();

    // 跳过可能的 PID 前缀：jstat 带 pid 时第一列是 PID 数字，总共 12 个数字
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
    /// 线程名
    pub name: String,
    /// 线程编号
    pub id: u32,
    /// 是否守护线程
    pub daemon: bool,
    /// 线程状态
    pub state: ThreadState,
    /// 线程优先级
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
    /// 状态着色符号（用于 TUI）
    pub fn symbol(&self) -> &str {
        match self {
            ThreadState::Runnable => "🟢",
            ThreadState::Blocked => "🔴",
            ThreadState::Waiting | ThreadState::TimedWaiting => "🟡",
            ThreadState::Sleeping => "🔵",
            ThreadState::Unknown(_) => "⚪",
        }
    }

    /// 状态标签
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

        // 检测死锁段落
        if line.contains("Found one Java-level deadlock") {
            deadlock_detected = true;
            in_deadlock_section = true;
            i += 1;
            continue;
        }

        // 在死锁段落中收集线程名
        if in_deadlock_section {
            if line.starts_with('"') {
                // 死锁段落中线程名格式: "main": 或 "thread-name"
                // 找最后一个 '"'，取第一个到最后一个之间的内容
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

        // 线程声明行格式: "main" #1 daemon prio=5 ...
        // 或: "main" #1 prio=5 ...
        if let Some(stripped) = line.strip_prefix('"') {
            let mut info = ThreadInfo {
                name: String::new(),
                id: 0,
                daemon: false,
                state: ThreadState::Unknown("UNKNOWN".to_string()),
                priority: 5,
            };

            // 解析线程名
            if let Some(end_quote) = stripped.find('"') {
                info.name = stripped[..end_quote].to_string();
            }

            // 解析 #N（线程编号）
            if let Some(hash_pos) = line.find('#') {
                let after_hash = &line[hash_pos + 1..];
                let num_str: String = after_hash.chars().take_while(|c| c.is_ascii_digit()).collect();
                info.id = num_str.parse().unwrap_or(0);
            }

            // 检测 daemon
            if line.contains(" daemon ") {
                info.daemon = true;
            }

            // 解析 prio=N
            if let Some(prio_pos) = line.find("prio=") {
                let after_prio = &line[prio_pos + 5..];
                let num_str: String = after_prio.chars().take_while(|c| c.is_ascii_digit()).collect();
                info.priority = num_str.parse().unwrap_or(5);
            }

            // 向下查找线程状态行: java.lang.Thread.State: XXX
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

    // 解析并格式化输出
    println!("{}", gc_table_header());
    println!("{}", "-".repeat(78));

    let mut snapshot_count = 0u32;
    for line in stdout.lines() {
        if line.trim().is_empty() || line.starts_with('S') || line.starts_with('-') {
            continue; // 跳过标题行和空行
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
    println!(
        "列说明: S0/S1=Survivor, Eden=新生代, Old=老年代, Meta=元空间, CCS=压缩类空间"
    );
    println!(
        "        YGC/YGCT=Young GC 次数/耗时, FGC/FGCT=Full GC 次数/耗时, GCT=总耗时"
    );

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

    // 汇总统计
    println!("线程统计:");
    println!("  总线程数: {}", snapshot.total);
    println!("  守护线程: {}", snapshot.daemon_count);
    if snapshot.blocked_count > 0 {
        println!("  ⚠️  BLOCKED: {}", snapshot.blocked_count);
    }
    if snapshot.deadlock_detected {
        println!(
            "  🔴 检测到死锁! 涉及线程: {:?}",
            snapshot.deadlock_threads
        );
    } else {
        println!("  死锁检测: ✅ 无");
    }

    println!();
    println!("详细信息:");

    // 按状态分组展示
    let runnable: Vec<_> = snapshot
        .threads
        .iter()
        .filter(|t| t.state == ThreadState::Runnable)
        .collect();
    let blocked: Vec<_> = snapshot
        .threads
        .iter()
        .filter(|t| t.state == ThreadState::Blocked)
        .collect();
    let waiting: Vec<_> = snapshot
        .threads
        .iter()
        .filter(|t| t.state == ThreadState::Waiting || t.state == ThreadState::TimedWaiting)
        .collect();
    let other: Vec<_> = snapshot
        .threads
        .iter()
        .filter(|t| {
            t.state != ThreadState::Runnable
                && t.state != ThreadState::Blocked
                && t.state != ThreadState::Waiting
                && t.state != ThreadState::TimedWaiting
        })
        .collect();

    if !blocked.is_empty() {
        println!();
        println!("🔴 BLOCKED ({})", blocked.len());
        for t in &blocked {
            let daemon = if t.daemon { " [daemon]" } else { "" };
            println!(
                "  {} #{} (prio={}){}",
                t.name, t.id, t.priority, daemon
            );
        }
    }

    if !runnable.is_empty() {
        println!();
        println!("🟢 RUNNABLE ({})", runnable.len());
        for t in &runnable {
            let daemon = if t.daemon { " [daemon]" } else { "" };
            println!(
                "  {} #{} (prio={}){}",
                t.name, t.id, t.priority, daemon
            );
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
        assert_eq!(
            ThreadState::from_jcmd("RUNNABLE"),
            ThreadState::Runnable
        );
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
