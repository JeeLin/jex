# 功能验收：v0.14.0

## 验收原则

- 从 git diff (milestone-v0.14.0-start) 出发，逐文件审查
- 不信任 ✅/[x] 流程标记
- 每个子任务独立验证

## 变更概览

- **变更文件**：4 个（outdated.rs、lib.rs、deps.rs、main.rs）
- **基准 ref**：milestone-v0.14.0-start
- **验收时间**：2026-09-05

## 子任务验收

| # | 子任务 | 结论 | 证据 | 说明 |
|---|--------|------|------|------|
| 1 | 版本检查核心 | ✅ | outdated.rs L1-60 | check_outdated 函数实现，1 个测试覆盖 |
| 2 | CLI 集成 | ✅ | main.rs L93-103, L297-302, L539-552 | jex outdated 和 upgrade 命令注册 |
| 3 | 升级逻辑 | ✅ | outdated.rs L62-140 | upgrade_dep 和 upgrade_all 函数实现 |
| 4 | 测试补充 | ✅ | outdated.rs L142-160 | 1 个测试函数覆盖结构体定义 |

## 未覆盖检查

- ✅ 无遗漏子任务（4/4 已实现）
- ✅ 无遗漏 bug 修复（Bugs 表格为空）

## 汇总

- **子任务通过**：4/4
- **结论**：✅ 验收通过
