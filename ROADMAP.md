# jex 步骤规划

> 配套:PLAN.md(定位)、DESIGN.md(接口契约 + 第 10 节策略)、FEATURES.md(功能清单)。
> 本文件只排**逻辑步骤**,不给日历时间。每步标注 `[整合]`=v1 走现有 CLI/服务,`[替代]`=后续自实现 UX 层(见 DESIGN.md 第 10 节)。

---

## Phase 0 —— 地基(工具本体,无外部功能)

- **0.1** `cargo init` + `clap` 子命令骨架:`jdk` / `add` / `run` / `search` / `export` / `import` / `java` 占位。
- **0.2** `~/.jex` 全局配置目录 + 项目级配置加载(`jex.toml` / `.jex-version`)。
- **0.3** 错误体系(`anyhow`)+ 日志。
- **0.4** 自检并缓存 `cs`(Coursier CLI)到 `~/.jex/bin`(外部依赖自动就位)。

**验收**:`jex --help` 显示全部子命令;`cs` 自检通过。

## Phase 1 —— 整合版(v1,全走现有命令行/服务) `[整合]`

- **1.1 JDK( mise 模型 )**:Adoptium API 下载 / 安装 / 列出 / 钉版(`use`)/ `which` / `doctor`(跨设备一致性校验)。
- **1.2 依赖**:`init` / `add` / `remove` / `update` → shell `cs fetch`;`jex.toml` + `jex.lock.toml` + 统一缓存 `~/.jex/cache`;含 `jex tree` / `why` / `conflict` 依赖分析(基于已解析树)。
- **1.3 搜索**:`search <kw>` 查 Maven Central Solr API,返回坐标 + 最新版本 + 描述,选中即 `add`。
- **1.4 运行**:`run <file>` 解析依赖 → 拼 classpath → `javac` → `java`;编译缓存(源+依赖 hash)。
- **1.5 诊断(核心)**:`java gc <pid>`(包 `jstat`,结构化输出优先)/ `java threads <pid>`(包 `jcmd Thread.print`)。
- **1.6 互通**:`import pom`(已有 Maven 项目用 `jex run`)/ `export maven`(导出种子 `pom.xml`)。

**验收**:完整走通「装 JDK → 加 gson → 跑一段代码 → search → 导出/导入 pom」;对真进程 `gc`/`threads` 出清晰报告。

## Phase 2 —— 自实现 UX 层(替代阶段,逐个替换) `[替代]`

- **2.1 依赖解析内嵌**:去掉外部 `cs`,内嵌 coursier lib(单二进制);解析算法仍用现成。
- **2.2 诊断结构化**:GC / 线程改直连 JVM attach / PerfData / JMX,**不再爬 `jstat`/`jcmd` 文本**。
- **2.3 火焰图**:捆绑 async-profiler 进发布包(替代 shell 调用)。
- **2.4 JFR**:自写 JFR 解析器,替代 `jfr` CLI 出分配压力 / GC 停顿 / 热点报告。
- **2.5 诊断增强**:`java top` 实时面板、`java heap` 堆概览。

**验收**:无外部 `cs` 仍可解析;诊断不依赖 `jstat`/`jcmd` 文本仍可出报告;发布含 async-profiler;JFR 自解析出报告。

## Phase 3 —— 扩展 / 远期

- 脚本模式 `//DEPS`、REPL(`jex repl`)、`watch` 热重载、`fmt`(google-java-format)、monorepo 多模块、IDE/LSP、`self update`、私服/离线/代理、依赖审计(OSV)、Gradle 互转。

**验收**:按 FEATURES.md `[远期]` 逐条。

---

## 依赖关系

- **Phase 0** 是所有前提。
- **1.1**(JDK)是 **1.4**(运行)的前置;`run` 需要目标 JDK。
- **1.2**(依赖)是 **1.4**(运行)的前置;`run` 需要依赖解析。
- **Phase 2 每项**依赖 Phase 1 对应整合版:先有 wrap,才有 replace。
- **Phase 3** 独立于 Phase 2,可并行起步(按优先级挑)。

## 步骤总览(一条线)

```
0 地基 → 1.1 JDK → 1.2 依赖 → 1.3 搜索 → 1.4 运行 → 1.5 诊断(gc/threads) → 1.6 互通
                                                  ↓(替代期)
                           2.1 解析内嵌 → 2.2 诊断结构化 → 2.3 火焰图 → 2.4 JFR → 2.5 增强
                                                  ↓
                                   3 扩展/远期(按需挑)
```
