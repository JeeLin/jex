//! 交互式 Java 求值（基于 jshell）

use crate::deps;
use crate::error::{Error, Result};
use crate::jdk;
use crate::run;
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

    // 2. 构建 classpath（复用 run 模块的逻辑）
    let mut classpath = String::new();
    if !class_only {
        if let Ok(lock) = deps::read_jex_lock() {
            if let Ok(paths) = run::build_classpath_vec(&lock) {
                classpath = paths.join(":");
            }
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

    // 4. 启动 jshell 进程（使用 quiet 反馈模式）
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
    let mut stdout = child.stdout.take().ok_or_else(|| Error::new("无法获取 jshell stdout"))?;

    // 5. 交互循环
    use std::io::BufRead;
    let mut reader = io::BufReader::new(&mut stdout);
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

        // 读取 jshell 输出（quiet 模式下每条输出以换行结束）
        // 使用 read_line 逐行读取，遇到空行或超时停止
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        break; // 空行表示 jshell 输出结束
                    }
                    println!("{}", trimmed);
                }
                Err(_) => break,
            }
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
