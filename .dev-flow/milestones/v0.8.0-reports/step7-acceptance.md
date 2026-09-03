# 功能验收：v0.8.0 代码格式化

## 验收原则

- 从 git diff (milestone-v0.8.0-start) 出发，逐文件审查
- 不信任 ✅/[x] 流程标记
- 不引用步骤 3 的提交信息
- 每个子任务和 bug 独立验证

## 变更概览

- **变更文件**：38（含报告/锁文件）；核心代码文件 6 个（fmt.rs、main.rs、config.rs、lib.rs、Cargo.toml×2）
- **基准 ref**：milestone-v0.8.0-start
- **验收时间**：2026-09-03

## 子任务验收

| # | 子任务 | 结论 | 证据 | 说明 |
|---|--------|------|------|------|
| 1 | 格式化引擎 | ✅ | `crates/jex-core/src/fmt.rs` (228 行) | Style 枚举含 Google/Aosp/OpenJ7 三变体，带 FromStr + Display 实现；FmtConfig 含 style/aosp/skip_future/exclude 四字段及 Default impl（默认 Google 风格，排除 build/、target/）；`build_gjf_args` 提取 JAR 查找 + java 检查 + 风格参数构建；`format_file` 通过 `java -jar` 调用 google-java-format 处理文件路径；`format_code` 通过 stdin/stdout 管道处理代码字符串；`find_gjf_jar` 从 `~/.jex/tools/` 查找 JAR；`check_java` 验证 java 可用性。3 个单元测试覆盖 Style 解析、默认配置、Display。 |
| 2 | CLI 集成 | ✅ | `crates/jex-cli/src/main.rs` (+93 行) | Commands 枚举新增 `Fmt(FmtArgs)` 变体（别名 `f`）；FmtArgs 结构体含 paths（默认 `.`）、check、stdout、changed 四字段；run() 中 Fmt 分支完整：加载配置 → 根据 changed 标志分发 → 非 changed 时按 file/dir 递归遍历 .java 文件 → 均调用 `fmt::format_and_output`。新增 `fmt_output_mode` 辅助函数根据 check/stdout 标志确定 OutputMode。 |
| 3 | 配置支持 | ✅ | `crates/jex-core/src/config.rs` (+38 行) | `pub fn read_fmt_config()` 读取 `jex.toml`，解析 `[fmt]` section，逐一提取 style（String → Style parse）、aosp（bool）、skip_future（bool）、exclude（Array<String>），返回 FmtConfig；缺失字段使用 Default 值。1 个单元测试验证无文件时返回 Err。`lib.rs` 已注册 `pub mod fmt`，`Cargo.toml` 已添加 `dirs` 依赖。 |
| 4 | 增量格式化 | ✅ | `crates/jex-core/src/fmt.rs` lines 139-167 | `get_changed_files()` 执行 `git diff --name-only HEAD~1`，通过 `.ends_with(".java")` 过滤仅保留 Java 文件；`format_changed(config)` 在此基础上使用 `config.exclude` 列表通过 `!s.contains(e)` 过滤排除路径，并附加 `.exists()` 检查。返回待格式化文件列表供调用方批量处理。 |

## Bug 修复验收

| # | 优先级 | 标题 | 结论 | 证据 | 说明 |
|---|--------|------|------|------|------|
| 1 | 🟡 | 提取格式化输出逻辑 | ✅ | `crates/jex-core/src/fmt.rs` lines 191-213; `crates/jex-cli/src/main.rs` lines 284-333 | `format_and_output(path, config, mode)` 封装了 Stdout/Check/Write 三种输出模式的逻辑；main.rs Fmt handler 中对每个文件均调用此 helper，无重复的 stdout/check/write 分支。OutputMode 枚举定义在 fmt.rs 中，由 main.rs 的 `fmt_output_mode` 函数转换。 |
| 2 | 🟡 | 提取 JAR 参数构建逻辑 | ✅ | `crates/jex-core/src/fmt.rs` lines 54-67, 101-137 | `build_gjf_args(config)` 统一执行 JAR 查找（`find_gjf_jar`）、java 检查（`check_java`）、风格参数构建；`format_file`（line 101）和 `format_code`（line 118）均通过 `build_gjf_args(config)?` 获取参数后各自追加特有参数（format_file 追加 skip-sorting-imports 和文件路径，format_code 不追加），消除了重复代码。 |

## 未覆盖检查

- ✅ **无遗漏子任务**：4 个子任务均在 diff 中找到对应实现代码
- ✅ **无遗漏 bug 修复**：2 个标记 [x] 的 bug 均在 diff 中找到对应修复代码
- ✅ **范围外变更已标注**：`Cargo.lock` 变更（+158 行）为新增 `dirs` crate 的传递依赖（dirs → dirs-sys → libredox/redox_users/windows-sys），属正常依赖更新；`Cargo.toml` / `crates/jex-core/Cargo.toml` 各新增 1 行 `dirs.workspace = true` 为 fmt.rs 中 `dirs::home_dir()` 所需；里程碑报告文件（step2/4/5/6）为流程产物，非代码变更

## 汇总

- **子任务通过**：4/4
- **Bug 修复通过**：2/2
- **结论**：✅ 验收通过
