# 功能验收：v0.11.0

## 验收原则

- 从 git diff (milestone-v0.11.0-start) 出发，逐文件审查
- 不信任 ✅/[x] 流程标记
- 不引用步骤 3 的提交信息
- 每个子任务和 bug 独立验证

## 变更概览

- **变更文件**：7（5 源码 + 2 Cargo 配置）
- **基准 ref**：milestone-v0.11.0-start
- **验收时间**：v0.11.0 里程碑

## 子任务验收

| # | 子任务 | 结论 | 证据 | 说明 |
|---|--------|------|------|------|
| 1 | `jex build` 独立编译命令 | ✅ | `main.rs`: `Commands::Build(a)` 实现，调用 `collect_java_files()` + `compile()`；`run.rs`: `compile()` 提取为公共函数，`collect_java_files()` 递归遍历 src/ | 功能目标、接口设计（`--clean` 参数）、后端流程均实现。输出 "✅ Build complete → {path}" |
| 2 | `jex analyze` 依赖分析 | ✅ | `analyze.rs`（新建 228 行）：`analyze_project()` 读取 jex.toml 依赖 → 扫描 src/ import → 已知映射表 + 启发式推断 → 输出未使用（⚠）/未声明（❌）报告 | 功能目标、输出格式、后端流程均实现。4 个单元测试覆盖 |
| 3 | `jex repl` 交互式求值 | ✅ | `repl.rs`（新建 125 行）：`start_repl()` 获取 jshell 路径 → 构建 classpath（复用 `run::build_classpath_vec()`）→ 启动 jshell 进程 → 交互循环（读取输入/发送/读取输出）→ `/exit` 退出 | 功能目标、接口设计（`--class-only` 参数）、退出命令（`/exit`/`/quit`/`/q`）均实现 |
| 4 | `jex completions` shell 补全 | ✅ | `main.rs`: `Commands::Completions(a)` 使用 `clap_complete::generate()` 生成补全脚本；`Shell` 枚举（Bash/Zsh/Fish/Powershell）；`clap_complete` 依赖已添加到 Cargo.toml | 功能目标、4 种 shell 支持均实现 |

## Bug 修复验收

| # | 优先级 | 标题 | 结论 | 证据 | 说明 |
|---|--------|------|------|------|------|
| 1 | 🔴 | jex build 传递无效文件名 | ✅ | `run.rs`: `collect_java_files()` 递归遍历 src/ 下所有 .java 文件，返回 `Vec<String>`；`main.rs`: `Commands::Build` 调用 `collect_java_files()` 后转为 `Vec<&str>` 传给 `compile()` | 已修复：不再传递 `"*"` 通配符 |
| 2 | 🟡 | run() 重复编译逻辑 | ✅ | `run.rs`: `run()` 函数调用 `compile(&[file], false)?` 并解构为 `(build, classpath)`，不再重复读取 lock 文件和构建 classpath | 已修复：完全复用 compile() |
| 3 | 🟡 | REPL 输出读取可能挂起 | ✅ | `repl.rs`: 使用 `BufReader::read_line()` 逐行读取，遇到空行或 EOF 停止，不再使用阻塞的 `stdout.read()` | 已修复：非阻塞读取 |
| 4 | 🟡 | repl.rs 与 run.rs 重复 classpath 构建逻辑 | ✅ | `run.rs`: `build_classpath_vec()` 提取为 `pub fn`；`repl.rs`: 调用 `run::build_classpath_vec(&lock)` 复用 | 已修复：classpath 构建统一 |
| 5 | 🟢 | run() lock 文件双重读取 | ✅ | `run.rs`: `compile()` 返回 `(PathBuf, String)`，`run()` 直接使用返回的 classpath，不再重新读取 lock 文件 | 已修复：一次读取 |

## 未覆盖检查

- ✅ 无遗漏子任务（4 个子任务全部实现）
- ✅ 无遗漏 bug 修复（5 个已修复 bug 均验证通过）
- ✅ 无范围外变更（所有变更均在里程碑文档定义范围内）
- ✅ `lib.rs` 正确导出 `analyze` 和 `repl` 模块
- ✅ `planned()` 辅助函数已移除（不再有未实现的 stub 命令）

## 汇总

- **子任务通过**：4/4
- **Bug 修复通过**：5/5
- **结论**：✅ 验收通过
