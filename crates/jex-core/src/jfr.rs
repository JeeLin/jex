//! JFR (Java Flight Recorder) 录制与解析
//! - 通过 jcmd 控制 JFR 录制生命周期
//! - 解析 .jfr 二进制文件提取结构化事件
//! - 终端展示 JFR 事件摘要
//!
//! 子任务：实现 `jex java rec` 和 `jex java analyze` 命令。

use crate::error::{Error, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// ─── 录制生命周期 ──────────────────────────────────────────────

/// JFR 录制会话
pub struct RecordingSession {
    pub pid: u32,
    pub output_path: PathBuf,
    pub duration_secs: Option<u32>,
}

/// 获取 jcmd 路径（复用 JDK 路径）
fn jcmd_path() -> Result<String> {
    // 尝试从 PATH 找 jcmd
    let output = Command::new("which")
        .arg("jcmd")
        .output()
        .map_err(|e| Error::new(format!("找不到 jcmd: {e}")))?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Ok(path);
        }
    }

    Err(Error::new(
        "jcmd 不可用：请确保 JDK 已安装且在 PATH 中\n\
         提示: jex jdk install 21 && jex jdk use 21"
            .to_string(),
    ))
}

/// 启动 JFR 录制
pub fn start_recording(pid: u32, duration_secs: Option<u32>) -> Result<RecordingSession> {
    let output_dir = std::env::temp_dir().join(format!("jex-rec-{pid}"));
    fs::create_dir_all(&output_dir).map_err(|e| Error::new(format!("创建录制目录失败: {e}")))?;

    let filename = output_dir.join(format!("recording-{pid}.jfr"));

    let jcmd = jcmd_path()?;

    // 构建 JFR.start 参数
    let mut args = vec![
        pid.to_string(),
        "JFR.start".to_string(),
        format!("filename={}", filename.display()),
        "settings=profile".to_string(),
        "dumponexit=true".to_string(),
    ];

    if let Some(secs) = duration_secs {
        args.push(format!("duration={secs}s"));
    }

    let output = Command::new(&jcmd)
        .args(&args)
        .output()
        .map_err(|e| Error::new(format!("执行 jcmd JFR.start 失败: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::new(format!("JFR.start 失败: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("started") && !stdout.contains("Started") {
        // 某些 JDK 版本输出格式不同，检查是否有错误
        if stderr_string(&output).contains("Unable to")
            || stderr_string(&output).contains("not available")
        {
            return Err(Error::new(
                "JFR 不可用：目标 JVM 可能不支持 JFR（需要 JDK 11+）\n\
                 提示: 确保目标进程使用 JDK 11 或更高版本运行"
                    .to_string(),
            ));
        }
    }

    println!("🎙️  JFR 录制已启动 (PID {pid})");
    if let Some(secs) = duration_secs {
        println!("   录制时长: {secs} 秒");
    } else {
        println!("   按 Ctrl+C 停止录制...");
    }

    Ok(RecordingSession {
        pid,
        output_path: filename,
        duration_secs,
    })
}

/// 停止并 dump JFR 文件
pub fn dump_recording(session: RecordingSession) -> Result<PathBuf> {
    let jcmd = jcmd_path()?;

    // 如果是定时录制，等待完成
    if let Some(secs) = session.duration_secs {
        println!("   等待录制完成 ({secs}s)...");
        std::thread::sleep(std::time::Duration::from_secs(secs as u64));
    }

    // 检查文件是否已生成（dumponexit=true 会在进程退出时自动 dump）
    if session.output_path.exists() {
        let metadata = fs::metadata(&session.output_path)
            .map_err(|e| Error::new(format!("读取 JFR 文件失败: {e}")))?;
        if metadata.len() > 0 {
            println!("📊 JFR 录制完成: {}", session.output_path.display());
            return Ok(session.output_path);
        }
    }

    // 手动 dump
    let output = Command::new(&jcmd)
        .args([
            &session.pid.to_string(),
            "JFR.dump",
            &format!("filename={}", session.output_path.display()),
        ])
        .output()
        .map_err(|e| Error::new(format!("执行 jcmd JFR.dump 失败: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::new(format!("JFR.dump 失败: {stderr}")));
    }

    if !session.output_path.exists() {
        return Err(Error::new(format!(
            "JFR 文件未生成: {}",
            session.output_path.display()
        )));
    }

    println!("📊 JFR 录制完成: {}", session.output_path.display());
    Ok(session.output_path)
}

/// 列出目标 JVM 支持的 JFR 事件类型
pub fn list_event_types(pid: u32) -> Result<Vec<String>> {
    let jcmd = jcmd_path()?;

    let output = Command::new(&jcmd)
        .args([pid.to_string().as_str(), "JFR.events"])
        .output()
        .map_err(|e| Error::new(format!("执行 jcmd JFR.events 失败: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::new(format!("JFR.events 失败: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let events: Vec<String> = stdout
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .collect();

    Ok(events)
}

/// 停止正在运行的 JFR 录制（不 dump）
pub fn stop_recording(pid: u32) -> Result<()> {
    let jcmd = jcmd_path()?;

    let output = Command::new(&jcmd)
        .args([pid.to_string().as_str(), "JFR.stop"])
        .output()
        .map_err(|e| Error::new(format!("执行 jcmd JFR.stop 失败: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::new(format!("JFR.stop 失败: {stderr}")));
    }

    println!("⏹️  JFR 录制已停止 (PID {pid})");
    Ok(())
}

fn stderr_string(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

// ─── JFR 二进制解析 ──────────────────────────────────────────

/// JFR 事件类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventType {
    CpuSampling,
    GcEvent,
    FileRead,
    FileWrite,
    SocketRead,
    SocketWrite,
    ThreadBlock,
    MemAlloc,
    Unknown(String),
}

impl EventType {
    /// 从 JFR 事件类型名称映射
    pub fn from_jfr_name(name: &str) -> Self {
        match name {
            "jdk.ExecutionSample" | "jdk.NativeMethodSample" => EventType::CpuSampling,
            "jdk.GarbageCollection" | "jdk.GCHeapSummary" | "jdk.GCPhasePause" => {
                EventType::GcEvent
            }
            "jdk.FileRead" => EventType::FileRead,
            "jdk.FileWrite" => EventType::FileWrite,
            "jdk.SocketRead" => EventType::SocketRead,
            "jdk.SocketWrite" => EventType::SocketWrite,
            "jdk.ThreadLock" | "jdk.JavaMonitorWait" => EventType::ThreadBlock,
            "jdk.ObjectAllocationInNewTLAB"
            | "jdk.ObjectAllocationOutsideTLAB"
            | "jdk.AllocationRequiringGC" => EventType::MemAlloc,
            other => EventType::Unknown(other.to_string()),
        }
    }

    /// 显示名称
    pub fn display_name(&self) -> &str {
        match self {
            EventType::CpuSampling => "CPU 采样",
            EventType::GcEvent => "GC 事件",
            EventType::FileRead => "文件读取",
            EventType::FileWrite => "文件写入",
            EventType::SocketRead => "网络读取",
            EventType::SocketWrite => "网络写入",
            EventType::ThreadBlock => "线程阻塞",
            EventType::MemAlloc => "内存分配",
            EventType::Unknown(_) => "其他",
        }
    }
}

/// 解析后的 JFR 事件
#[derive(Debug, Clone)]
pub struct JfrEvent {
    pub timestamp: u64,
    pub event_type: EventType,
    pub duration: u64,
    pub thread: Option<String>,
    pub details: String,
}

/// JFR 文件头部
#[derive(Debug)]
pub struct JfrHeader {
    pub major_version: u16,
    pub minor_version: u16,
    pub chunk_size: u64,
    pub start_time: u64,
    pub duration: u64,
}

/// 解析 .jfr 文件
pub fn parse_jfr(path: &Path) -> Result<(JfrHeader, Vec<JfrEvent>)> {
    let data = fs::read(path).map_err(|e| Error::new(format!("读取 JFR 文件失败: {e}")))?;

    if data.len() < 68 {
        return Err(Error::new("JFR 文件太小，不是有效的 JFR 文件".to_string()));
    }

    // 验证 magic number: "FLR\0"
    if &data[0..4] != b"FLR\0" {
        return Err(Error::new(
            "无效的 JFR 文件格式（magic number 不匹配）".to_string(),
        ));
    }

    // 解析头部
    let header = parse_jfr_header(&data)?;

    // 解析事件
    let events = parse_jfr_events(&data, &header)?;

    Ok((header, events))
}

fn parse_jfr_header(data: &[u8]) -> Result<JfrHeader> {
    // JFR 文件格式 (big-endian):
    // 0-3:   magic "FLR\0"
    // 4-5:   major version
    // 6-7:   minor version
    // 8-15:  chunk size
    // 16-23: start time (nanoseconds since epoch)
    // 24-31: duration (nanoseconds)
    // 32-39: flags
    // 40-47: chunk count
    // 48-63: filename (metadata string ID, not actual filename)
    // 64-67: minimum record size

    if data.len() < 68 {
        return Err(Error::new("JFR 头部不完整".to_string()));
    }

    let major = u16::from_be_bytes([data[4], data[5]]);
    let minor = u16::from_be_bytes([data[6], data[7]]);
    let chunk_size = u64::from_be_bytes([
        data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
    ]);
    let start_time = u64::from_be_bytes([
        data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23],
    ]);
    let duration = u64::from_be_bytes([
        data[24], data[25], data[26], data[27], data[28], data[29], data[30], data[31],
    ]);

    Ok(JfrHeader {
        major_version: major,
        minor_version: minor,
        chunk_size,
        start_time,
        duration,
    })
}

/// 解析 JFR 事件（简化版：提取事件类型和基本数据）
fn parse_jfr_events(data: &[u8], header: &JfrHeader) -> Result<Vec<JfrEvent>> {
    let mut events = Vec::new();
    let mut offset = 68; // 跳过头部

    // JFR chunk 结构：
    // - chunk header (48 bytes minimum)
    // - metadata string pool
    // - constant pool
    // - event records

    // 简化解析：扫描已知事件类型标记
    // 实际 JFR 格式非常复杂，这里做实用级别的解析

    while offset + 8 <= data.len() {
        // 读取记录头
        let _tag = data[offset];
        let size = if offset + 4 <= data.len() {
            u32::from_be_bytes([
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
            ]) as usize
        } else {
            break;
        };

        if size == 0 || offset + size > data.len() {
            break;
        }

        // 尝试从事件数据中提取信息
        if let Some(event) = extract_event_from_record(&data[offset..offset + size], header) {
            events.push(event);
        }

        offset += size;
    }

    Ok(events)
}

/// 从事件记录中提取信息（简化版）
fn extract_event_from_record(record: &[u8], header: &JfrHeader) -> Option<JfrEvent> {
    if record.len() < 10 {
        return None;
    }

    let tag = record[0];
    // tag 的高 4 位是记录类型
    // 1 = metadata, 2 = constant pool, 3 = event
    let record_type = (tag >> 4) & 0x0F;

    if record_type != 3 {
        return None; // 只处理事件记录
    }

    // 事件记录格式：
    // - header (tag + size)
    // - event type ID (varint)
    // - timestamp (delta from chunk start, varint)
    // - duration (varint, optional)
    // - event data

    let mut pos = 5; // 跳过 tag + size
    let event_type_id = read_varint(record, &mut pos)?;
    let timestamp_delta = read_varint(record, &mut pos).unwrap_or(0);
    let duration = read_varint(record, &mut pos).unwrap_or(0);

    // 根据事件类型 ID 映射
    // JDK 11+ 的事件类型 ID 是常量，但实际值因 JDK 版本而异
    // 这里使用常见的事件类型名称模式
    let event_type = match event_type_id {
        101 => EventType::CpuSampling,
        160..=162 => EventType::GcEvent,
        110 => EventType::FileRead,
        111 => EventType::FileWrite,
        120 => EventType::SocketRead,
        121 => EventType::SocketWrite,
        130 => EventType::ThreadBlock,
        140 | 141 => EventType::MemAlloc,
        _ => {
            // 尝试从数据中提取更多信息
            let details = format!("事件类型 ID: {event_type_id}");
            return Some(JfrEvent {
                timestamp: header.start_time + timestamp_delta,
                event_type: EventType::Unknown(format!("id:{event_type_id}")),
                duration,
                thread: None,
                details,
            });
        }
    };

    Some(JfrEvent {
        timestamp: header.start_time + timestamp_delta,
        event_type,
        duration,
        thread: None,
        details: String::new(),
    })
}

/// 读取变长整数 (varint)
fn read_varint(data: &[u8], pos: &mut usize) -> Option<u64> {
    let mut result: u64 = 0;
    let mut shift = 0;

    loop {
        if *pos >= data.len() {
            return None;
        }
        let byte = data[*pos];
        *pos += 1;

        result |= ((byte & 0x7F) as u64) << shift;
        shift += 7;

        if byte & 0x80 == 0 {
            break;
        }
    }

    Some(result)
}

// ─── 聚合与展示 ──────────────────────────────────────────────

/// JFR 事件汇总统计
#[derive(Debug)]
pub struct JfrSummary {
    pub duration_secs: f64,
    pub event_counts: std::collections::HashMap<String, usize>,
    pub total_events: usize,
    pub top_cpu_hotspots: Vec<(String, usize)>,
    pub gc_count: usize,
    pub total_gc_duration_ms: u64,
    pub file_io_count: usize,
    pub total_file_bytes: u64,
    pub thread_block_count: usize,
    pub total_block_duration_ms: u64,
    pub mem_alloc_count: usize,
    pub total_alloc_bytes: u64,
}

/// 聚合 JFR 事件
pub fn summarize(events: &[JfrEvent], header: &JfrHeader) -> JfrSummary {
    let mut event_counts = std::collections::HashMap::new();
    let mut gc_count = 0;
    let mut total_gc_duration_ms = 0u64;
    let mut file_io_count = 0;
    let mut thread_block_count = 0;
    let mut total_block_duration_ms = 0u64;
    let mut mem_alloc_count = 0;

    for event in events {
        let name = event.event_type.display_name().to_string();
        *event_counts.entry(name).or_insert(0) += 1;

        match event.event_type {
            EventType::GcEvent => {
                gc_count += 1;
                total_gc_duration_ms += event.duration / 1_000_000;
            }
            EventType::FileRead | EventType::FileWrite => {
                file_io_count += 1;
            }
            EventType::ThreadBlock => {
                thread_block_count += 1;
                total_block_duration_ms += event.duration / 1_000_000;
            }
            EventType::MemAlloc => {
                mem_alloc_count += 1;
            }
            _ => {}
        }
    }

    let duration_secs = if header.duration > 0 {
        header.duration as f64 / 1_000_000_000.0
    } else {
        0.0
    };

    JfrSummary {
        duration_secs,
        event_counts,
        total_events: events.len(),
        top_cpu_hotspots: Vec::new(), // 需要更复杂的解析
        gc_count,
        total_gc_duration_ms,
        file_io_count,
        total_file_bytes: 0,
        thread_block_count,
        total_block_duration_ms,
        mem_alloc_count,
        total_alloc_bytes: 0,
    }
}

/// 终端展示 JFR 摘要
pub fn display_summary(summary: &JfrSummary) {
    println!();
    println!("┌─────────────────────────────────────────────────┐");
    println!("│          JFR 录制摘要                            │");
    println!("├─────────────────────────────────────────────────┤");
    println!(
        "│  时长: {:>8.1}s  │  总事件: {:>8}          │",
        summary.duration_secs, summary.total_events
    );
    println!("├─────────────────────────────────────────────────┤");

    if summary.gc_count > 0 {
        println!(
            "│  GC 事件:     {:>6} 次  │  总停顿: {:>6}ms  │",
            summary.gc_count, summary.total_gc_duration_ms
        );
    }
    if summary.file_io_count > 0 {
        println!(
            "│  文件 I/O:    {:>6} 次                           │",
            summary.file_io_count
        );
    }
    if summary.thread_block_count > 0 {
        println!(
            "│  线程阻塞:    {:>6} 次  │  总阻塞: {:>6}ms  │",
            summary.thread_block_count, summary.total_block_duration_ms
        );
    }
    if summary.mem_alloc_count > 0 {
        println!(
            "│  内存分配:    {:>6} 次                           │",
            summary.mem_alloc_count
        );
    }

    println!("├─────────────────────────────────────────────────┤");
    println!("│  事件分布:                                      │");

    let mut counts: Vec<_> = summary.event_counts.iter().collect();
    counts.sort_by(|a, b| b.1.cmp(a.1));

    for (name, count) in counts.iter().take(8) {
        let bar_len = (**count * 20 / summary.total_events.max(1)).min(20);
        let bar = "█".repeat(bar_len);
        println!("│    {name:<12} {:>6} {bar:<20} │", count);
    }

    println!("└─────────────────────────────────────────────────┘");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    /// Helper to encode a value as varint bytes
    fn encode_varint(mut val: u64) -> Vec<u8> {
        let mut buf = Vec::new();
        loop {
            let mut byte = (val & 0x7F) as u8;
            val >>= 7;
            if val > 0 {
                byte |= 0x80;
            }
            buf.push(byte);
            if val == 0 {
                break;
            }
        }
        buf
    }

    fn build_event_record(event_type_id: u64, ts_delta: u64, dur: u64) -> Vec<u8> {
        let et_bytes = encode_varint(event_type_id);
        let ts_bytes = encode_varint(ts_delta);
        let dur_bytes = encode_varint(dur);
        let payload_len = et_bytes.len() + ts_bytes.len() + dur_bytes.len();
        let min_size = payload_len.max(10);
        let total = 1 + 4 + min_size;
        let mut record = vec![0u8; total];
        record[0] = 0x30;
        let size = total as u32;
        record[1..5].copy_from_slice(&size.to_be_bytes());
        let mut pos = 5;
        for b in &et_bytes {
            record[pos] = *b;
            pos += 1;
        }
        for b in &ts_bytes {
            record[pos] = *b;
            pos += 1;
        }
        for b in &dur_bytes {
            record[pos] = *b;
            pos += 1;
        }
        record
    }

    #[test]
    fn test_event_type_mapping() {
        assert_eq!(
            EventType::from_jfr_name("jdk.ExecutionSample"),
            EventType::CpuSampling
        );
        assert_eq!(
            EventType::from_jfr_name("jdk.GarbageCollection"),
            EventType::GcEvent
        );
        assert_eq!(
            EventType::from_jfr_name("jdk.FileRead"),
            EventType::FileRead
        );
        assert_eq!(
            EventType::from_jfr_name("jdk.ThreadLock"),
            EventType::ThreadBlock
        );
        assert_eq!(
            EventType::from_jfr_name("jdk.ObjectAllocationInNewTLAB"),
            EventType::MemAlloc
        );
    }

    #[test]
    fn test_event_type_from_jfr_name_all_variants() {
        assert_eq!(
            EventType::from_jfr_name("jdk.NativeMethodSample"),
            EventType::CpuSampling
        );
        assert_eq!(
            EventType::from_jfr_name("jdk.GCHeapSummary"),
            EventType::GcEvent
        );
        assert_eq!(
            EventType::from_jfr_name("jdk.GCPhasePause"),
            EventType::GcEvent
        );
        assert_eq!(
            EventType::from_jfr_name("jdk.FileWrite"),
            EventType::FileWrite
        );
        assert_eq!(
            EventType::from_jfr_name("jdk.SocketRead"),
            EventType::SocketRead
        );
        assert_eq!(
            EventType::from_jfr_name("jdk.SocketWrite"),
            EventType::SocketWrite
        );
        assert_eq!(
            EventType::from_jfr_name("jdk.JavaMonitorWait"),
            EventType::ThreadBlock
        );
        assert_eq!(
            EventType::from_jfr_name("jdk.ObjectAllocationOutsideTLAB"),
            EventType::MemAlloc
        );
        assert_eq!(
            EventType::from_jfr_name("jdk.AllocationRequiringGC"),
            EventType::MemAlloc
        );
        match EventType::from_jfr_name("jdk.CustomEvent") {
            EventType::Unknown(s) => assert_eq!(s, "jdk.CustomEvent"),
            _ => panic!("Expected Unknown variant"),
        }
    }

    #[test]
    fn test_display_name_all_variants() {
        assert_eq!(EventType::CpuSampling.display_name(), "CPU 采样");
        assert_eq!(EventType::GcEvent.display_name(), "GC 事件");
        assert_eq!(EventType::FileRead.display_name(), "文件读取");
        assert_eq!(EventType::FileWrite.display_name(), "文件写入");
        assert_eq!(EventType::SocketRead.display_name(), "网络读取");
        assert_eq!(EventType::SocketWrite.display_name(), "网络写入");
        assert_eq!(EventType::ThreadBlock.display_name(), "线程阻塞");
        assert_eq!(EventType::MemAlloc.display_name(), "内存分配");
        assert_eq!(EventType::Unknown("x".into()).display_name(), "其他");
    }

    #[test]
    fn test_read_varint() {
        // Single byte: 0x05 = 5
        let data = [0x05];
        let mut pos = 0;
        assert_eq!(read_varint(&data, &mut pos), Some(5));
        assert_eq!(pos, 1);

        // Two bytes: 0x80 0x01 = 128
        let data = [0x80, 0x01];
        let mut pos = 0;
        assert_eq!(read_varint(&data, &mut pos), Some(128));
        assert_eq!(pos, 2);

        // Empty data
        let data: [u8; 0] = [];
        let mut pos = 0;
        assert_eq!(read_varint(&data, &mut pos), None);
    }

    #[test]
    fn test_read_varint_multi_byte() {
        // 3-byte varint: 0x80 0x80 0x01 = 16384
        let data = [0x80, 0x80, 0x01];
        let mut pos = 0;
        assert_eq!(read_varint(&data, &mut pos), Some(16384));
        assert_eq!(pos, 3);
    }

    #[test]
    fn test_parse_jfr_header_invalid() {
        // Too small
        let data = vec![0u8; 10];
        assert!(parse_jfr_header(&data).is_err());
    }

    #[test]
    fn test_parse_jfr_header_valid() {
        let mut data = vec![0u8; 100];
        data[4..6].copy_from_slice(&3u16.to_be_bytes());
        data[6..8].copy_from_slice(&1u16.to_be_bytes());
        data[8..16].copy_from_slice(&512u64.to_be_bytes());
        data[16..24].copy_from_slice(&999u64.to_be_bytes());
        data[24..32].copy_from_slice(&2000u64.to_be_bytes());
        let header = parse_jfr_header(&data).unwrap();
        assert_eq!(header.major_version, 3);
        assert_eq!(header.minor_version, 1);
        assert_eq!(header.chunk_size, 512);
        assert_eq!(header.start_time, 999);
        assert_eq!(header.duration, 2000);
    }

    #[test]
    fn test_parse_jfr_too_small() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("small.jfr");
        std::fs::write(&path, &vec![0u8; 10]).unwrap();
        let result = parse_jfr(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("太小"));
    }

    #[test]
    fn test_parse_jfr_bad_magic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad_magic.jfr");
        let mut data = vec![0u8; 68];
        data[0..4].copy_from_slice(b"TEST");
        std::fs::write(&path, &data).unwrap();
        let result = parse_jfr(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("magic"));
    }

    #[test]
    fn test_parse_jfr_valid_no_events() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("valid.jfr");
        let mut data = vec![0u8; 100];
        data[0..4].copy_from_slice(b"FLR\0");
        data[4..6].copy_from_slice(&2u16.to_be_bytes());
        data[6..8].copy_from_slice(&0u16.to_be_bytes());
        data[8..16].copy_from_slice(&100u64.to_be_bytes());
        data[16..24].copy_from_slice(&1000u64.to_be_bytes());
        data[24..32].copy_from_slice(&5000u64.to_be_bytes());
        std::fs::write(&path, &data).unwrap();
        let (header, events) = parse_jfr(&path).unwrap();
        assert_eq!(header.major_version, 2);
        assert_eq!(header.start_time, 1000);
        assert!(events.is_empty());
    }

    #[test]
    fn test_parse_jfr_events_empty_data() {
        let header = JfrHeader {
            major_version: 2,
            minor_version: 0,
            chunk_size: 1024,
            start_time: 0,
            duration: 0,
        };
        let data = vec![0u8; 68];
        let events = parse_jfr_events(&data, &header).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_extract_event_too_short() {
        let header = JfrHeader {
            major_version: 2,
            minor_version: 0,
            chunk_size: 0,
            start_time: 0,
            duration: 0,
        };
        let record = vec![0u8; 5];
        assert!(extract_event_from_record(&record, &header).is_none());
    }

    #[test]
    fn test_extract_event_non_event_record() {
        let header = JfrHeader {
            major_version: 2,
            minor_version: 0,
            chunk_size: 0,
            start_time: 0,
            duration: 0,
        };
        let record = vec![0x10u8; 20];
        assert!(extract_event_from_record(&record, &header).is_none());
    }

    #[test]
    fn test_extract_event_valid() {
        let header = JfrHeader {
            major_version: 2,
            minor_version: 0,
            chunk_size: 0,
            start_time: 1000,
            duration: 0,
        };
        let record = build_event_record(101, 10, 5);
        let event = extract_event_from_record(&record, &header).unwrap();
        assert_eq!(event.event_type, EventType::CpuSampling);
        assert_eq!(event.timestamp, 1010);
        assert_eq!(event.duration, 5);
    }

    #[test]
    fn test_extract_event_gc() {
        let header = JfrHeader {
            major_version: 2,
            minor_version: 0,
            chunk_size: 0,
            start_time: 0,
            duration: 0,
        };
        let record = build_event_record(160, 0, 0);
        let event = extract_event_from_record(&record, &header).unwrap();
        assert_eq!(event.event_type, EventType::GcEvent);
    }

    #[test]
    fn test_extract_event_file_read() {
        let header = JfrHeader {
            major_version: 2,
            minor_version: 0,
            chunk_size: 0,
            start_time: 0,
            duration: 0,
        };
        let record = build_event_record(110, 0, 0);
        let event = extract_event_from_record(&record, &header).unwrap();
        assert_eq!(event.event_type, EventType::FileRead);
    }

    #[test]
    fn test_extract_event_file_write() {
        let header = JfrHeader {
            major_version: 2,
            minor_version: 0,
            chunk_size: 0,
            start_time: 0,
            duration: 0,
        };
        let record = build_event_record(111, 0, 0);
        let event = extract_event_from_record(&record, &header).unwrap();
        assert_eq!(event.event_type, EventType::FileWrite);
    }

    #[test]
    fn test_extract_event_socket_read() {
        let header = JfrHeader {
            major_version: 2,
            minor_version: 0,
            chunk_size: 0,
            start_time: 0,
            duration: 0,
        };
        let record = build_event_record(120, 0, 0);
        let event = extract_event_from_record(&record, &header).unwrap();
        assert_eq!(event.event_type, EventType::SocketRead);
    }

    #[test]
    fn test_extract_event_socket_write() {
        let header = JfrHeader {
            major_version: 2,
            minor_version: 0,
            chunk_size: 0,
            start_time: 0,
            duration: 0,
        };
        let record = build_event_record(121, 0, 0);
        let event = extract_event_from_record(&record, &header).unwrap();
        assert_eq!(event.event_type, EventType::SocketWrite);
    }

    #[test]
    fn test_extract_event_thread_block() {
        let header = JfrHeader {
            major_version: 2,
            minor_version: 0,
            chunk_size: 0,
            start_time: 0,
            duration: 0,
        };
        let record = build_event_record(130, 0, 0);
        let event = extract_event_from_record(&record, &header).unwrap();
        assert_eq!(event.event_type, EventType::ThreadBlock);
    }

    #[test]
    fn test_extract_event_mem_alloc() {
        let header = JfrHeader {
            major_version: 2,
            minor_version: 0,
            chunk_size: 0,
            start_time: 0,
            duration: 0,
        };
        let record = build_event_record(140, 0, 0);
        let event = extract_event_from_record(&record, &header).unwrap();
        assert_eq!(event.event_type, EventType::MemAlloc);
    }

    #[test]
    fn test_extract_event_unknown() {
        let header = JfrHeader {
            major_version: 2,
            minor_version: 0,
            chunk_size: 0,
            start_time: 0,
            duration: 0,
        };
        let record = build_event_record(999, 0, 0);
        let event = extract_event_from_record(&record, &header).unwrap();
        match event.event_type {
            EventType::Unknown(s) => assert_eq!(s, "id:999"),
            _ => panic!("Expected Unknown"),
        }
        assert!(event.details.contains("999"));
    }

    #[test]
    fn test_summarize_empty() {
        let header = JfrHeader {
            major_version: 2,
            minor_version: 0,
            chunk_size: 1024,
            start_time: 0,
            duration: 10_000_000_000,
        };
        let events = vec![];
        let summary = summarize(&events, &header);
        assert_eq!(summary.total_events, 0);
        assert!((summary.duration_secs - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_summarize_duration_zero() {
        let header = JfrHeader {
            major_version: 2,
            minor_version: 0,
            chunk_size: 1024,
            start_time: 0,
            duration: 0,
        };
        let summary = summarize(&[], &header);
        assert_eq!(summary.duration_secs, 0.0);
    }

    #[test]
    fn test_summarize_with_events() {
        let header = JfrHeader {
            major_version: 2,
            minor_version: 0,
            chunk_size: 1024,
            start_time: 0,
            duration: 5_000_000_000,
        };
        let events = vec![
            JfrEvent {
                timestamp: 100,
                event_type: EventType::GcEvent,
                duration: 10_000_000,
                thread: None,
                details: String::new(),
            },
            JfrEvent {
                timestamp: 200,
                event_type: EventType::FileRead,
                duration: 0,
                thread: None,
                details: String::new(),
            },
            JfrEvent {
                timestamp: 300,
                event_type: EventType::ThreadBlock,
                duration: 5_000_000,
                thread: None,
                details: String::new(),
            },
            JfrEvent {
                timestamp: 400,
                event_type: EventType::MemAlloc,
                duration: 0,
                thread: None,
                details: String::new(),
            },
            JfrEvent {
                timestamp: 500,
                event_type: EventType::FileWrite,
                duration: 0,
                thread: None,
                details: String::new(),
            },
            JfrEvent {
                timestamp: 600,
                event_type: EventType::CpuSampling,
                duration: 0,
                thread: None,
                details: String::new(),
            },
        ];
        let summary = summarize(&events, &header);
        assert_eq!(summary.total_events, 6);
        assert_eq!(summary.gc_count, 1);
        assert_eq!(summary.total_gc_duration_ms, 10);
        assert_eq!(summary.file_io_count, 2);
        assert_eq!(summary.thread_block_count, 1);
        assert_eq!(summary.total_block_duration_ms, 5);
        assert_eq!(summary.mem_alloc_count, 1);
        assert!((summary.duration_secs - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_display_summary_gc() {
        let summary = JfrSummary {
            duration_secs: 10.0,
            event_counts: std::collections::HashMap::new(),
            total_events: 100,
            top_cpu_hotspots: vec![],
            gc_count: 5,
            total_gc_duration_ms: 50,
            file_io_count: 10,
            total_file_bytes: 0,
            thread_block_count: 3,
            total_block_duration_ms: 30,
            mem_alloc_count: 7,
            total_alloc_bytes: 0,
        };
        display_summary(&summary);
    }

    #[test]
    fn test_display_summary_empty() {
        let summary = JfrSummary {
            duration_secs: 0.0,
            event_counts: std::collections::HashMap::new(),
            total_events: 0,
            top_cpu_hotspots: vec![],
            gc_count: 0,
            total_gc_duration_ms: 0,
            file_io_count: 0,
            total_file_bytes: 0,
            thread_block_count: 0,
            total_block_duration_ms: 0,
            mem_alloc_count: 0,
            total_alloc_bytes: 0,
        };
        display_summary(&summary);
    }

    #[test]
    fn test_display_summary_with_event_counts() {
        let mut event_counts = std::collections::HashMap::new();
        event_counts.insert("CPU 采样".to_string(), 50);
        event_counts.insert("GC 事件".to_string(), 30);
        let summary = JfrSummary {
            duration_secs: 10.0,
            event_counts,
            total_events: 80,
            top_cpu_hotspots: vec![],
            gc_count: 30,
            total_gc_duration_ms: 100,
            file_io_count: 0,
            total_file_bytes: 0,
            thread_block_count: 0,
            total_block_duration_ms: 0,
            mem_alloc_count: 0,
            total_alloc_bytes: 0,
        };
        display_summary(&summary);
    }

    #[test]
    fn test_stderr_string() {
        let output = std::process::Output {
            status: std::process::ExitStatus::from_raw(1),
            stdout: vec![],
            stderr: b"error message".to_vec(),
        };
        assert_eq!(stderr_string(&output), "error message");
    }

    #[test]
    fn test_event_type_clone_and_eq() {
        let e1 = EventType::CpuSampling;
        let e2 = e1.clone();
        assert_eq!(e1, e2);
        let e3 = EventType::Unknown("test".into());
        let e4 = e3.clone();
        assert_eq!(e3, e4);
    }

    #[test]
    fn test_jfr_event_debug() {
        let event = JfrEvent {
            timestamp: 100,
            event_type: EventType::GcEvent,
            duration: 50,
            thread: Some("main".into()),
            details: "test".into(),
        };
        let debug = format!("{:?}", event);
        assert!(debug.contains("GcEvent"));
    }
}
