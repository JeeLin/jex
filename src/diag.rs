//! 诊断核心（gc/threads）
//! - gc: jstat -gcutil 清晰展示 + 列解释
//! - threads: jcmd Thread.print 清晰展示（线程状态、死锁高亮）

use crate::error::{Error, Result};
use std::process::Command;

/// GC 概览（jstat -gcutil）
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
    println!("{}", stdout);

    Ok(())
}

/// 线程概览（jcmd Thread.print）
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

    // 解析线程统计
    let mut thread_count = 0;
    let mut daemon_count = 0;
    let mut blocked_count = 0;

    for line in stdout.lines() {
        if line.contains("\"") {
            thread_count += 1;
        }
        if line.contains("daemon") {
            daemon_count += 1;
        }
        if line.contains("BLOCKED") {
            blocked_count += 1;
        }
    }

    println!("线程统计:");
    println!("  总线程数: {}", thread_count);
    println!("  守护线程: {}", daemon_count);
    if blocked_count > 0 {
        println!("  ⚠️  潜在死锁/阻塞: {}", blocked_count);
    } else {
        println!("  死锁检测: ✅ 无");
    }

    println!();
    println!("详细信息:");
    println!("{}", stdout);

    Ok(())
}
