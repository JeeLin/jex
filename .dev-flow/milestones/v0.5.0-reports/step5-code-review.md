# 代码审查：v0.5.0 火焰图捆绑

## 变更概览

- **变更文件**：17 个（16 modified + 1 new）
- **新增模块**：crates/jex-core/src/profiler.rs (698 行)
- **审查时间**：2025-07-02

## 问题列表

| # | 严重程度 | 文件 | 行号 | 描述 |
|---|----------|------|------|------|
| 1 | 🟡 | crates/jex-cli/src/main.rs | 239-249 | `--output` 复制失败被 `.ok()` 静默吞掉，但随后仍 `println!("已复制到: ...")` 误导用户 |
| 2 | 🟡 | crates/jex-core/src/profiler.rs | 284-340 | `profile()` 函数过长（~50 行），混合平台特定命令构建、错误处理、文件操作三个职责 |
| 3 | 🟢 | crates/jex-core/src/profiler.rs | 152-176 | `download_profiler` 中 `extracted_dir.exists()` 分支与 `find_profiler_dir` 分支的移动逻辑重复 |
| 4 | 🟢 | crates/jex-core/src/profiler.rs | 399-435 | `convert_collapsed_to_svg` 三种 fallback 串联 if-else，无策略模式难扩展 |
| 5 | 🟢 | crates/jex-core/src/profiler.rs | 575 | `test_platform_detect` 只验证"不是 Windows"，对平台检测逻辑无实际验证力 |

## 汇总

- 🔴 必须修复：0
- 🟡 应该修复：2（错误处理、过长函数）
- 🟢 可选改进：3
- **结论**：存在 🟡 + 🟢，需打回开发阶段修复
