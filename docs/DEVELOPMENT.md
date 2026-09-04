# jex 开发设计文档

## 整体规划

项目分 4 个 Phase，每个 Phase 是可运行的纵向切片：

### Phase 0 — 地基（已完成 ✅）
工具本体骨架，无外部功能。
- [x] 0.1 cargo init + clap 子命令骨架
- [x] 0.2 ~/.jex 全局配置目录 + 项目级配置加载
- [x] 0.3 错误体系 (anyhow) + 日志
- [x] 0.4 自检并缓存 cs (Coursier CLI)

### Phase 1 — 整合版（v1）✅ 已完成
全走现有命令行/服务，整合 Coursier + Adoptium + jstat/jcmd。
- [x] 1.1 JDK 版本管理（mise 模型）
- [x] 1.2 依赖管理（init/add/remove/update + 锁文件）
- [x] 1.3 依赖搜索（Maven Central Solr API）
- [x] 1.4 一键运行（解析 → 编译 → 运行 + 缓存）
- [x] 1.5 诊断核心（gc/threads）
- [x] 1.6 生态互通（import/export pom）

### Phase 2 — 自实现 UX 层
逐个替换外部依赖，内嵌 coursier lib、结构化诊断、捆绑 async-profiler。
- [x] 2.0 Workspace 多 crate 拆分（jex-core + jex-cli）✅ v0.3.0
- [x] 2.1 依赖解析内嵌 ✅ v0.4.0
- [x] 2.2 诊断结构化 ✅ v0.3.0（jstat/jcmd 结构化解析 + crossterm TUI）
- [x] 2.3 火焰图捆绑 ✅ v0.5.0
- [x] 2.4 JFR 自解析 ✅ v0.6.0
- [x] 2.5 诊断增强 ✅ v0.7.0

### Phase 3 — 扩展/远期
脚本模式、REPL、热重载、fmt、monorepo、IDE 集成、self update 等。

## 依赖关系

```
Phase 0 (地基) → Phase 1 (整合)
                     ↓
              1.1 JDK → 1.4 运行
              1.2 依赖 → 1.4 运行
                     ↓
              Phase 2 (替代) → Phase 3 (扩展)
```

## 里程碑版本号规则

- 语义版本风格：`v{major}.{minor}.{patch}`
- patch：bug 修复、纯修复、重构
- minor：新增功能（里程碑默认递增方式）
- major：破坏性变更

## 架构决策

1. **工具本体用 Rust**：免 JDK，避免鸡生蛋问题
2. **依赖解析用 Rust 原生 HTTP + XML 解析**：Maven Central search API + POM 解析，v0.4.0 起完全摆脱 cs CLI
3. **诊断用 JDK 自带工具**：jstat/jcmd/jfr + async-profiler，只做美化
4. **导出 Maven 有损**：降低退出成本，非双向同步

## 里程碑划分

### v0.2.0 Phase 1 整合版 ✅
- **核心功能**：JDK 管理 + 依赖管理 + 搜索 + 运行 + 诊断 + 生态互通（全部走 shell cs/原生命令）
- **子任务**：6 个（1.1–1.6）
- **版本类型**：minor

### v0.3.0 Phase 2.0 Workspace 拆分 + 诊断结构化 ✅
- **核心功能**：jex-core + jex-cli 拆分、jstat/jcmd 结构化解析、crossterm TUI、cs 自动下载、CI + mise 接入
- **子任务**：4 个
- **版本类型**：minor

### v0.4.0 依赖解析内嵌 ✅
- **核心功能**：resolver.rs 原生依赖解析（search API + POM 解析 + 深度限制 + 去重）、deps.rs/run.rs 迁移、jex tree 完整依赖树可视化、cs CLI 完全移除
- **子任务**：5 个
- **依赖**：v0.3.0
- **版本类型**：minor
- **版本号**：v0.4.0

### v0.5.0 火焰图捆绑 ✅
- **核心功能**：async-profiler 捆绑、SVG 火焰图生成、`jex java flame` 命令
- **子任务**：3 个
- **依赖**：v0.4.0
- **版本类型**：minor

### v0.6.0 JFR 自解析 ✅
- **核心功能**：JFR 录制控制（jcmd JFR.start/stop/dump）、.jfr 二进制解析、终端摘要展示、`jex java rec/analyze` 命令
- **子任务**：4 个
- **依赖**：v0.5.0
- **版本类型**：minor

### v0.7.0 诊断增强 ✅
- **核心功能**：heap 概览（jstat -gc）、top 实时面板（crossterm 刷新）、增强线程分析（死锁检测）
- **子任务**：4 个
- **依赖**：v0.6.0
- **版本类型**：minor

### v0.8.0 代码格式化 ✅ 已完成
- **核心功能**：Java 代码格式化工具，支持 google-java-format 规范
- **子任务**：4 个（格式化引擎、CLI 集成、配置支持、增量格式化）
- **依赖**：v0.7.0
- **版本类型**：minor
- **版本号**：v0.8.0

### v0.9.0 脚本模式 ✅ 已完成
- **核心功能**：直接运行单文件 Java 脚本（shebang 支持 + 临时依赖声明 + 缓存编译），对标 `uv run script.py`
- **子任务预估**：4 个（脚本解析引擎、shebang/pragma 注解、缓存编译、CLI `jex run` 增强）
- **依赖**：v0.8.0
- **版本类型**：minor
- **版本号**：v0.9.0

### v0.10.0 自更新 ✅ 已完成
- **核心功能**：`jex self update` 命令，自动检查并更新 jex 二进制到最新版本
- **子任务预估**：4 个（版本检查 API、下载管理、CLI 集成、原子更新机制）
- **依赖**：v0.9.0
- **版本类型**：minor
- **版本号**：v0.10.0

### v0.11.0 CLI 增强 ✅ 已完成
- **核心功能**：补全 stub 命令（build/analyze）、交互式 REPL、shell 自动补全
- **子任务**：4 个（build 编译命令、analyze 依赖分析、repl 交互式求值、completions shell 补全）
- **依赖**：v0.10.0
- **版本类型**：minor
- **版本号**：v0.11.0
