# 功能验收：v0.10.0 自更新

## 验收原则

- 从 git diff (milestone-v0.10.0-start) 出发，逐文件审查
- 不信任 ✅/[x] 流程标记
- 不引用步骤 3 的提交信息
- 每个子任务独立验证

## 变更概览

- **变更文件**：8（含 5 份里程碑报告 + 3 份功能代码 + 1 份 CI 配置）
- **基准 ref**：milestone-v0.10.0-start
- **验收时间**：2025-08-09

## 子任务验收

| # | 子任务 | 结论 | 证据 | 说明 |
|---|--------|------|------|------|
| 1 | 版本检查 API | ✅ | `crates/jex-core/src/update.rs` | `UpdateInfo` 结构体（latest_version/download_url/release_notes）完整；`check_latest()` 请求 `api.github.com/repos/JeeLin/jex/releases/latest`，使用 `reqwest::blocking::Client` 带 User-Agent，JSON 反序列化到 `GitHubRelease` 中间结构体，tag_name 去 `v` 前缀，调用 `find_asset_url` 匹配平台；`needs_update()` 实现逐段 semver 比较（major → minor → patch），逻辑正确 |
| 2 | 下载管理 | ✅ | `crates/jex-core/src/update.rs` | `detect_platform()` 使用 `std::env::consts::OS`/`ARCH`，覆盖 linux/macos/windows × amd64/arm64 共 6 种组合，未知平台返回明确错误；`download_binary()` 使用 reqwest blocking 下载，写入目标路径后 Unix 平台 `chmod 0o755` 设置可执行权限（`#[cfg(unix)]`）；`find_asset_url()` 辅助函数按 `os-arch` 模式匹配，Windows 附加 `.exe` 后缀 |
| 3 | CLI 集成 | ✅ | `crates/jex-cli/src/main.rs` | `Self_` 子命令注册（alias `su`），`SelfCommand::Update(SelfUpdateArgs)` 含 `--check` flag；命令处理器实现完整交互流程：检查版本 → 判断是否需要更新 → `--check` 仅提示 → 下载到 `.tmp` → `atomic_replace` → 成功提示；`lib.rs` 已导出 `pub mod update` |
| 4 | 原子更新 + 测试 | ✅ | `crates/jex-core/src/update.rs` | `atomic_replace()` 实现备份 → 重命名 → 恢复的容错策略（备份为 `.bak` 后缀，成功后删除备份，失败时恢复原文件）；`current_binary_path()` 通过 `std::env::current_exe()` 获取；**7 个单元测试**全部通过：`test_current_version`、`test_needs_update_major/minor/patch/same`、`test_detect_platform`、`test_find_asset_url` |

## Bug 修复验收

里程碑 Bugs 表格为空，无 bug 需验收。

## 未覆盖检查

- ✅ 无遗漏子任务（4/4 全部验证）
- ✅ 无遗漏 bug 修复
- ✅ 范围外变更已标注：`.github/workflows/release.yml`（CI 多平台构建矩阵，milestone 未显式要求但属于合理的配套基础设施）

## 额外验证：CI 工作流

`.github/workflows/release.yml` 存在且完整：

| 维度 | 结果 |
|------|------|
| 触发条件 | push tags `v*` ✅ |
| 平台矩阵 | 5 个目标：linux-amd64, linux-arm64, mac-amd64, mac-arm64, windows-amd64 ✅ |
| 编译检查 | lint job: fmt --check + clippy + test ✅ |
| Release 产物 | SHA256SUMS + 二进制文件上传到 GitHub Release ✅ |

## 质量门禁

| 检查项 | 结果 |
|--------|------|
| `cargo test --workspace` | ✅ 110 passed, 0 failed |
| `cargo clippy --workspace -- -D warnings` | ✅ 无 warning |
| update 模块单元测试 | ✅ 7/7 通过 |

## 汇总

- **子任务通过**：4/4
- **Bug 修复通过**：0/0（无 bug）
- **结论**：✅ 验收通过
