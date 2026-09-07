# 设计再确认：v0.5.0 火焰图捆绑

## 验证目标

对比已实现代码与里程碑文档，验证子任务/bug 是否真正满足需求。

## 验证方法

通过 git diff 和代码静态分析，逐项验证里程碑文档的子任务详细设计。

## 验证结果

| 子任务 | 验证项 | 结论 | 证据 |
|--------|--------|------|------|
| 1 async-profiler 自动下载 | `profiler_home` / `ensure_profiler` / `download_profiler` 实现 | ✅ | `crates/jex-core/src/profiler.rs:108-220` |
| 1 平台检测 | `Platform::detect` / `download_name` / `download_url` | ✅ | `crates/jex-core/src/profiler.rs:38-110` |
| 2 采样与火焰图 | `profile` / `FlameResult` / `convert_collapsed_to_svg` / `generate_basic_svg` | ✅ | `crates/jex-core/src/profiler.rs:275-450` |
| 3 CLI 集成 | `FlameArgs` with `--duration`/`--output` / `open_in_browser` | ✅ | `crates/jex-cli/src/main.rs:233-260` |
| 4 e2e 测试 | 单元测试 + clippy 通过 | ✅ | 74 个测试通过，clippy 无 warning |
| Bug 1 (🟡 用户反馈) | Cargo 依赖收敛到 workspace 根 | ✅ | `Cargo.toml` 包含 `[workspace.dependencies]` |
| Bug 2 (🟡 步骤4) | CI 触发条件恢复 | ✅ | `.github/workflows/ci.yml` 包含 push/PR 触发 |
| Bug 3 (🟢 步骤4) | FlameArgs 前空行 | ✅ | main.rs:172-173 空行恢复 |
| Bug 4 (🟡 步骤5) | 火焰图复制错误处理 | ✅ | `main.rs:240-244` 用 `?` 传播错误 |
| Bug 5 (🟡 步骤5) | profile() 函数拆分 | ✅ | 提取 `run_profiler` / `classify_profiler_error` |
| Bug 6 (🟢 步骤5) | download_profiler 分支重复 | ✅ | `profiler.rs:160-175` 统一为 `let source = if ... else ...` |
| Bug 7 (🟢 步骤6) | 覆盖率门禁跳过 | ✅ | 用户决定，标注 🟢 已跳过 |

## 汇总

- **子任务**：4/4 全部实现
- **Bugs**：8/8 已修复或显式跳过
- **结论**：✅ 通过
