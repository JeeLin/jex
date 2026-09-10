# jex

**JVM Toolchain CLI** — Bring the uv/bun developer experience to the Java ecosystem.

Not a reinvention of Maven/Gradle, but a lightweight entry point that's easy to start and easy to roll back to Maven.

[![Rust](https://img.shields.io/badge/Rust-1.70+-orange)](https://rust-lang.org)
[![License](https://img.shields.io/badge/License-MIT-blue)](LICENSE)
[![Version](https://img.shields.io/badge/Version-0.27.0-green)](CHANGELOG.md)

## Features at a Glance

| Capability | Command | Description |
|------------|---------|-------------|
| One-click run | `jex run` | Parse → compile → run, uv-style experience |
| Script mode | `jex run script.java` | Single-file Java script (shebang + inline dependencies) |
| JDK management | `jex jdk` | Install/switch/pin, cross-device consistency check |
| Dependency management | `jex add/remove/update` | Built on Coursier, apk-style search |
| Dependency search | `jex search` | Search Maven Central |
| Dependency tree | `jex tree` | Visualize project dependency tree (`--verbose` for detailed report) |
| Dependency updates | `jex outdated` | Check for dependency updates |
| Security audit | `jex audit` | Check for known vulnerabilities |
| Dependency upgrade | `jex upgrade` | Upgrade dependencies to latest versions |
| License check | `jex license` | Check dependency licenses |
| License compliance | `jex license-check` | Check license compliance |
| Dependency compatibility | `jex check` | Check dependency version compatibility |
| Version changelog | `jex changelog` | View dependency version changelog |
| Dependency pinning | `jex pin` | Pin dependency versions |
| Dependency cache | `jex cache` | Manage dependency cache |
| Workspace management | `jex workspace` | Multi-module workspace management |
| Hot reload | `jex watch` | Listen for file changes and auto-compile/run |
| Code formatting | `jex fmt` | Code formatting (google-java-format) |
| JVM diagnostics | `jex java` | JVM diagnostics (GC/threads/heap/flamegraph/recording) |
| Interactive Java | `jex repl` | Interactive Java evaluation (jshell) |
| Shell completion | `jex completions` | Generate shell auto-completion script |
| Self update | `jex self-update` | Self-update |
| Project creation | `jex create` | Create a new project |
| Dependency update check | `jex outdated` | Check for dependency updates |
| Security vulnerability check | `jex audit` | Check for security vulnerabilities |
| Dependency upgrade | `jex upgrade` | Upgrade dependencies |
| License check | `jex license` | Check dependency licenses |
| License compliance check | `jex license-check` | Check license compliance |
| Dependency compatibility check | `jex check` | Check dependency version compatibility |
| Version changelog | `jex changelog` | View dependency version changelog |
| Dependency pinning | `jex pin` | Pin dependency versions |
| Dependency cache management | `jex cache` | Manage dependency cache |
| Workspace management | `jex workspace` | Multi-module workspace management |
| Hot reload | `jex watch` | Listen for file changes and auto-compile/run |
| Code formatting | `jex fmt` | Code formatting (google-java-format) |
| JVM diagnostics | `jex java` | JVM diagnostics (GC/threads/heap/flamegraph/recording) |
| Interactive Java | `jex repl` | Interactive Java evaluation (jshell) |
| Shell completion | `jex completions` | Generate shell auto-completion script |
| Self update | `jex self-update` | Self-update |
| Project creation | `jex create` | Create a new project |

---

# jex （中文）

**JVM 工具链 CLI** — 把 uv/bun 的开发体验带入 Java 生态。

不是重造 Maven/Gradle，而是做一个「上手快、能随时退回 Maven」的轻量入口。

[![Rust](https://img.shields.io/badge/Rust-1.70+-orange)](https://rust-lang.org)
[![License](https://img.shields.io/badge/License-MIT-blue)](LICENSE)
[![Version](https://img.shields.io/badge/Version-0.27.0-green)](CHANGELOG.md)

## 特性一览

| 能力 | 命令 | 说明 |
|------|------|------|
| 一键运行 | `jex run` | 解析 → 编译 → 运行，uv 式体验 |
| 脚本模式 | `jex run script.java` | 单文件 Java 脚本（shebang + 内联依赖） |
| JDK 管理 | `jex jdk` | 安装/切换/钉版，跨设备一致性校验 |
| 依赖管理 | `jex add/remove/update` | 底层 Coursier，apk 式搜索 |
| 依赖搜索 | `jex search` | 搜索 Maven Central |
| 依赖树 | `jex tree` | 可视化项目依赖树（`--verbose` 详细报告） |
| 依赖更新 | `jex outdated` | 检查依赖更新 |
| 安全审计 | `jex audit` | 检查依赖安全漏洞 |
| 依赖升级 | `jex upgrade` | 升级依赖到最新版本 |
| 许可证检查 | `jex license` | 检查依赖许可证 |
| 许可证合规检查 | `jex license-check` | 检查许可证合规性 |
| 依赖版本兼容性检查 | `jex check` | 检查依赖版本兼容性 |
| 依赖版本变更日志 | `jex changelog` | 查看依赖版本变更日志 |
| 依赖版本锁定 | `jex pin` | 锁定依赖版本 |
| 依赖管理缓存 | `jex cache` | 管理依赖缓存 |
| 多模块工作区管理 | `jex workspace` | 多模块工作区管理 |
| 热重载：监听文件变更自动编译运行 | `jex watch` | 热重载：监听文件变更自动编译运行 |
| 代码格式化(google-java-format) | `jex fmt` | 代码格式化(google-java-format) |
| JVM 诊断(gc / threads / heap / 火焰图 / 录制) | `jex java` | JVM 诊断(gc / threads / heap / 火焰图 / 录制) |
| 交互式 Java 求值(jshell) | `jex repl` | 交互式 Java 求值(jshell) |
| 生成 shell 自动补全脚本 | `jex completions` | 生成 shell 自动补全脚本 |
| 自更新 | `jex self-update` | 自更新 |
| 创建新项目 | `jex create` | 创建新项目 |
| 检查依赖更新 | `jex outdated` | 检查依赖更新 |
| 检查依赖安全漏洞 | `jex audit` | 检查依赖安全漏洞 |
| 升级依赖 | `jex upgrade` | 升级依赖 |
| 检查依赖许可证 | `jex license` | 检查依赖许可证 |
| 检查依赖许可证合规性 | `jex license-check` | 检查依赖许可证合规性 |
| 检查依赖版本兼容性 | `jex check` | 检查依赖版本兼容性 |
| 生成项目依赖分析报告 | `jex report` | 生成项目依赖分析报告 |
| 锁定依赖版本 | `jex pin` | 锁定依赖版本 |
| 管理依赖缓存 | `jex cache` | 管理依赖缓存 |
| 多模块工作区管理 | `jex workspace` | 多模块工作区管理 |
| 热重载：监听文件变更自动编译运行 | `jex watch` | 热重载：监听文件变更自动编译运行 |
| 代码格式化(google-java-format) | `jex fmt` | 代码格式化(google-java-format) |
| JVM 诊断(gc / threads / heap / 火焰图 / 录制) | `jex java` | JVM 诊断(gc / threads / heap / 火焰图 / 录制) |
| 交互式 Java 求值(jshell) | `jex repl` | 交互式 Java 求值(jshell) |
| 生成 shell 自动补全脚本 | `jex completions` | 生成 shell 自动补全脚本 |
| 自更新 | `jex self-update` | 自更新 |
| 创建新项目 | `jex create` | 创建新项目 |

