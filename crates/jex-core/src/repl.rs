//! 交互式 Java 求值（基于 jshell）

use crate::deps;
use crate::error::{Error, Result};
use crate::jdk;
use std::io::{self, Write};
use std::process::{Command, Stdio};

/// 启动交互式 REPL
pub fn start_repl(class_only: bool) -> Result<()> {
    // 1. 获取 JDK 路径
    let java_home = jdk::which_java_home()?;
    let jshell_bin = java_home.join("bin").join("jshell");

    if !jshell_bin.exists() {
        return Err(Error::new(format!(
            "jshell 不存在: {}（需要 JDK 9+）",
            jshell_bin.display()
        )));
    }

    // 2. 构建 classpath
    let mut classpath = String::new();
    if !class_only {
        if let Ok(lock) = deps::read_jex_lock() {
                let dependencies = lock.dependencies.unwrap_or_default();
                let mut paths = Vec::new();
                for (coord, version) in &dependencies {
                    let parts: Vec<&str> = coord.split(':').collect();
                    if parts.len() >= 2 {
                        let path = format!(
                            "{}/{}/{}/{}-{}.jar",
                            crate::config::jex_m2_cache()?.display(),
                            parts[0].replace('.', "/"),
                            parts[1],
                            parts[1],
                            version
                        );
                        paths.push(path);
                    }
                }
                classpath = paths.join(":");
        }
    }

    // 3. 打印欢迎信息
    let dep_count = if classpath.is_empty() {
        0
    } else {
        classpath.matches(".jar").count()
    };
    println!("jex repl");
    if dep_count > 0 {
        println!("项目依赖已加载（{} 个 jar）", dep_count);
    }
    println!("输入 Java 代码，/exit 退出\n");

    // 4. 启动 jshell 进程
    let mut cmd = Command::new(&jshell_bin);
    cmd.arg("--feedback").arg("quiet");

    if !classpath.is_empty() {
        cmd.arg("--class-path").arg(&classpath);
    }

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        Error::new(format!(
            "启动 jshell 失败: {}（需要 JDK 9+）",
            e
        ))
    })?;

    let stdin = child.stdin.as_mut().ok_or_else(|| Error::new("无法获取 jshell stdin"))?;
    let stdout = child.stdout.as_mut().ok_or_else(|| Error::new("无法获取 jshell stdout"))?;

    // 5. 交互循环
    let mut stdout_lock = io::stdout();

    loop {
        // 打印提示符
        print!("jex> ");
        stdout_lock.flush()?;

        // 读取用户输入
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        // 检查退出命令
        if input == "/exit" || input == "/quit" || input == "/q" {
            println!("👋");
            break;
        }

        if input.is_empty() {
            continue;
        }

        // 发送输入到 jshell
        writeln!(stdin, "{}", input)?;
        stdin.flush()?;

        // 读取 jshell 输出
        // jshell 在 quiet 模式下，表达式结果会直接输出
        // 我们需要读取直到下一个提示符或 EOF
        let mut output_lines = Vec::new();
        let mut buffer = String::new();

        // 使用非阻塞方式读取输出
        // jshell quiet 模式下，每条语句的输出以换行结束
        let mut byte_buf = [0u8; 1024];
        loop {
            use std::io::Read;
            match stdout.read(&mut byte_buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&byte_buf[..n]);
                    buffer.push_str(&chunk);
                    // 检查是否有完整行
                    while let Some(pos) = buffer.find('\n') {
                        let line = buffer[..pos].to_string();
                        buffer = buffer[pos + 1..].to_string();
                        if !line.trim().is_empty() {
                            output_lines.push(line);
                        }
                    }
                }
                Err(_) => break,
            }

            // 简单的延迟避免忙等
            if !buffer.is_empty() || !output_lines.is_empty() {
                break;
            }
        }

        // 输出结果
        for line in &output_lines {
            println!("{}", line);
        }

        // 如果有残余 buffer
        let remaining = buffer.trim();
        if !remaining.is_empty() {
            println!("{}", remaining);
        }
    }

    // 等待 jshell 退出
    let _ = child.kill();
    let _ = child.wait();

    Ok(())
}

#[cfg(test)]
mod tests {
    // REPL 需要 JDK 9+，在 CI 环境可能不可用
    // 基本的编译测试已在集成测试中覆盖
}
