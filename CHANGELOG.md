# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.9.0] - 2026-09-03

### Added
- 脚本模式：`jex run script.java` 支持 shebang 检测，直接运行单文件 Java 脚本
- 脚本解析引擎：支持 `#!/usr/bin/env jex` 和 `///usr/bin/env jex` shebang 格式
- 文件内依赖声明：支持 `//DEPS group:artifact:version` 注解（兼容 jbang 格式）
- 文件内 Java 版本声明：支持 `//JAVA 21` 注解（可选）
- 缓存编译：基于内容哈希的缓存机制，依赖或源码未变时跳过编译
- 集成测试：6 个端到端测试用例覆盖脚本解析和缓存逻辑

## [0.8.0] - 2026-09-03

### Added
- 代码格式化：`jex fmt` 命令，支持 google-java-format 规范
- 格式化引擎：Style 枚举（Google/Aosp/OpenJ7）、FmtConfig 配置、format_file/format_code/format_and_output
- CLI 集成：`jex fmt <paths>` 支持文件/目录批量格式化，`--check` 检查模式，`--stdout` 输出到标准输出
- 配置支持：从 jex.toml `[fmt]` section 读取格式化配置（style/aosp/skip_future/exclude）
- 增量格式化：`--changed` 模式只格式化 git diff 变更的 .java 文件


## [0.7.0] - 2025-07-02

### Added
- 堆概览：`jex java heap <pid>` 展示 JVM 堆内存使用情况（区域分布 + GC 统计）
- Top 实时面板：`jex java top <pid>` 持续刷新展示 CPU/堆/GC/线程指标
- 增强线程分析：线程状态分布、死锁检测
## [0.6.0] - 2025-07-02

### Added
- JFR 录制：`jex java rec <pid>` 启动/停止 JFR 录制（支持定时录制 + 交互式）
- JFR 解析：纯 Rust 解析 .jfr 二进制文件（含 LZ4 压缩 chunk）
- JFR 展示：终端摘要表格（GC/IO/线程阻塞/内存分配事件统计）
- JFR 分析：`jex java analyze <file.jfr>` 离线分析 .jfr 文件

### Fixed
- Clippy clean：手写范围模式改为 `160..=162`，测试中 vec! 改为数组
## [0.5.0] - 2025-07-02

### Added
- async-profiler 自动下载与捆绑：`jex java flame <pid>` 命令
- 平台检测（Linux/macOS x86_64/aarch64）支持
- 火焰图生成（SVG 格式，含 collapsed 中间文件、纯 Rust fallback 生成器）
- 浏览器自动打开（macOS `open` / Linux `xdg-open`）
- `--duration` 采样时长参数（默认 10 秒）
- `--output` SVG 输出路径参数
- 跨模块单元测试覆盖（error/config/deps/export/jdk/run/search/util 共 63 个新测试）

### Changed
- Cargo 依赖收敛到 workspace 根：通过 `[workspace.dependencies]` 统一管理版本号
- CI 触发条件：恢复 push/PR to master + tags 触发
- 质量门禁：新增 `cargo llvm-cov` 覆盖率检查
## [0.4.0] - 2025-07-02

### Added
- 原生依赖解析核心模块：crates/jex-core/src/resolver.rs，通过 Maven Central search API + POM 解析实现 resolve_latest / resolve_dependencies / DepNode / format_tree
- reqwest + quick-xml 依赖：替换 cs CLI shell 调用，全部 HTTP 解析在 Rust 进程内完成
- jex_m2_cache：~/.jex/m2/ 本地 jar 仓库目录
- 依赖树深度限制（MAX_DEPTH=8）+ visited HashSet 去重，避免递归爆炸
- 单元测试：resolver::tests::test_resolve_latest_gson（真实网络）+ test_format_tree

### Changed
- deps.rs：resolve_latest_version 委托给 resolver::resolve_latest，删除 cs complete 调用
- run.rs：build_classpath 改用 resolver::resolve_dependencies + jex_m2_cache 路径生成
- deps.rs tree()：从简化输出升级为递归依赖树（含传递依赖）
- HTTP 客户端自定义 User-Agent（"jex/0.4.0 (Rust)"），绕过 Maven Central 默认 403

### Removed
- config.rs：删除 ensure_cs() / cs_path() / cs_download_url()，v0.4.0 不再依赖 cs CLI
- util.rs：删除 v0.3.0 引入但已无引用的 cs_os_str / cs_arch_str

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
