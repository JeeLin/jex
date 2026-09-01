# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.3.0] - 2025-07-01

### Added
- Workspace 多 crate 拆分：jex-core（lib）+ jex-cli（bin），为后续内嵌依赖解析和结构化诊断奠基
- 诊断结构化解析：jstat -gcutil 解析为 GcSnapshot 结构体，jcmd Thread.print 解析为 ThreadInfo 列表
- 诊断 crossterm TUI：gc_tui / threads_tui 实时刷新终端表格，TerminalGuard RAII 确保异常恢复
- cs CLI 自动下载：ensure_cs() 检测本地不存在时自动从 GitHub Releases 下载
- CI 集成：GitHub Actions 工作流运行 check / clippy / test / fmt
- mise 依赖管理：.mise.toml 配置 rust 1.98.0 + node 22 + rsproxy.cn 镜像 + mise run tasks

### Changed
- 诊断模块重构：提取 parse_jstat_all() 消除循环重复，threads() 改用单个 match 循环
- OS/ARCH 映射提取：cs_os_str/cs_arch_str/adoptium_os_str/adoptium_arch_str 集中到 util.rs
- Maven 搜索提取：fetch_docs() 消除 search() / versions() 中的 curl+解析重复

### Fixed
- TerminalGuard RAII 修复 raw mode 不恢复导致终端损坏风险（F1）
- CI 配置补充（F9）：新增 .github/workflows/ci.yml
- resolve_latest_version unwrap 防御性编程（F12）：改用 expect + 注释说明 cs 输出顺序假设
- CI 从 dtolnay/rust-toolchain 切换到 jdx/mise-action 实现 mise 接入（F17）

## [0.2.0] - 2025-08-31

### Added
- JDK 版本管理：`jex jdk` 子命令（install/use/list/which/doctor），支持 Adoptium API 下载和 mise 模型版本切换
- 依赖管理：`jex init/add/remove/update` 子命令，管理 jex.toml 和 jex.lock.toml 锁文件
- 依赖搜索：`jex search` 子命令，查询 Maven Central Solr API，支持 `--versions` 列出全部版本
- 一键运行：`jex run` 子命令，自动解析依赖→拼 classpath→javac→java，基于 hash 跳过重复编译
- 诊断核心：`jex java gc/threads` 子命令，封装 jstat/jcmd 输出为可读报告
- 生态互通：`jex export maven` 子命令，从 jex.lock.toml 生成 pom.xml
- 公共工具模块：`src/util.rs` 提取坐标解析等共享函数

### Fixed
- 修复 parse_coord() 在 deps.rs 和 search.rs 中的重复定义，提取至共享 util 模块
- 修复 search.rs versions() 函数未接入 CLI 的问题，添加 `--versions` 选项
- 修复 export.rs 内联坐标解析未使用共享实现的问题
- 修复代码格式不一致问题（cargo fmt）
