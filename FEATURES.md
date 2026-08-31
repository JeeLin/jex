# jex 功能清单(全量,不分排期)

> 本文只回答「该有哪些功能」,**不排期、不做里程碑**。
> 分级仅表示范围野心:`[核心]` = 已共识的 v1 支柱;`[扩展]` = 明确要做但可稍后;`[远期]` = 探索性。
> 引擎默认:Coursier(解析)、Adoptium API(JDK 下载)、async-profiler + JFR(诊断)。工具本体 Rust 单二进制。

---

## 1. JDK / 运行时版本管理 —— mise 模型 `[核心]`

> 关键认知:Java 开发者很少**频繁切换** JDK,价值在「声明 + 跨设备一致 + 自动供给」,对标 `mise`(项目级钉版本,任意机器自动装到同一版)。

- `jex jdk install <ver>` —— 下载并安装 Temurin `<ver>`(特性版 `21` 或精确 `21.0.2`)。
- `jex jdk list` —— 列出已装版本(标 ✔)与当前版本(标 →)。
- `jex jdk list --remote` —— 查 Adoptium API 列出可装版本。
- `jex jdk use <ver>` —— 写项目钉版(`.jex-version`)/ 全局默认(`~/.jex/jdk-current`)。
- `jex jdk uninstall <ver>` —— 删除已装 JDK。
- `jex jdk which` —— 打印当前生效 JDK 的 `JAVA_HOME`。
- **声明式钉版**:`jex.toml [project].java = 21`;任意机器 `jex` 自动 provision 同一版本。
- **跨设备一致校验**:`jex jdk doctor` 检查「项目声明 vs 实际已装」是否一致(团队/CI 防漂移)。
- 多 JDK 并存 + 按项目自动选用(无需手动切)。

## 2. 依赖管理 —— 体验优于 Maven/Gradle `[核心]`

> 机会点:Maven(XML 啰嗦、串行解析慢、无好用 CLI 发现)/ Gradle(DSL 复杂、daemon 预热慢、报错晦涩)。引擎用 Coursier(并行解析 + 统一缓存 + 快),`jex` 只做 UX 层。

- 声明式依赖:`jex.toml [dependencies]`,键 = `groupId:artifactId`,值 = 版本。
- `jex add <g:a[:v]>` —— 加入依赖(无版本经 Coursier 取 latest)。
- `jex remove <coord>` —— 移除依赖。
- `jex update [coord]` —— 更新依赖版本(支持范围 `[1.0,2.0)`)。
- **锁文件** `jex.lock.toml` —— flat 全量依赖树(含传递),保证可复现。
- **统一缓存** `~/.jex/cache` —— 所有 jar 一处存,Coursier 管。
- **清晰报错**:坐标找不到 / 版本冲突 / 传递依赖冲突,给出可读解释与建议(对标 uv 的友好报错)。
- `jex tree [--depth N]` —— 打印依赖树,冲突版本高亮(对标 `mvn dependency:tree`)。
- **依赖分析 / 冲突分析**(基于 `jex.lock.toml` 已解析树,离线可算):
  - `jex why <g:a>` —— 为何引入 X / 为何是版本 V:列全部引入路径 + 冲突解决结果。
  - `jex conflict` —— **冲突分析**:扫「同 `g:a` 被解析成多版本」,列每条冲突的版本、`A→B→…→版本` 各路径、最终生效版、修复建议(加 `exclude` / 钉 direct 版)。对标 Gradle resolution report。
  - `[替代]` `jex analyze` —— 广义分析(对标 `mvn dependency:analyze`):unused(声明未用)、undeclared(用了未声明,靠传递依赖带上,脆弱)、版本漂移;需字节码引用扫描,属后期。
- 私服:`[repositories]` 配 Maven Central + 私服 URL。
- 离线 / 代理:支持 `--offline` 与 `HTTP_PROXY`(企业内网)。
- `[远期]` 依赖审计:已知漏洞扫描(接 OSV / Sonatype)。

## 3. 依赖搜索 —— apk 式 `[核心]`

> 解决 Maven 生态最痛的「找坐标」:现在靠 google "maven gson dependency"。类比 `apk search` / `apt search`。

- `jex search <keyword>` —— 查 Maven Central Solr API(`search.maven.org/solrsearch/select`),返回 `groupId:artifactId` + 最新版本 + 描述。
- 交互式选中结果 → 直接 `add`(避免手拼坐标)。
- `jex search <g:a> --versions` —— 列出某 artifact 的全部可用版本。

## 4. 一键运行 `[核心]`

- `jex run <file.java>` —— 自动解析依赖 → 拼 classpath → `javac` → `java`。
- `jex run`(无参)—— 用 `jex.toml [project].main` 指定的入口。
- `-- <args>` —— 原样传参给程序。
- **编译缓存**:基于「源文件 hash + 依赖 hash」跳过重复编译。
- 多文件 / 包结构编译(扫描项目源码,不只单文件)。
- `[扩展]` 脚本模式:单文件 `//DEPS` 注释式依赖(像 jbang),零 `jex.toml` 也能跑。
- `[远期]` `jex repl` —— 交互式 REPL(接 JShell)。
- `[远期]` `jex watch` —— 文件变更热重载重跑。

## 5. 诊断 —— 取代 JVM 自带工具的命令行体验 `[核心]+[扩展]`

> 目标:成为人们首选的 JVM 诊断入口,**取代记 `jstat -gcutil <pid> 1000` 这类原始命令**。
> 边界:`jex` 封装 JVM 自带诊断协议(`jcmd`/`jstat`/`jfr` 是数据源),做**展示/UX 层**取代自带的*难用界面*,**不重写 HotSpot**。

- `[核心]` `jex java gc <pid>` —— `jstat` 清晰展示 + 列解释(各区占用、GC 次数/耗时趋势)。
- `[核心]` `jex java threads <pid>` —— `jcmd Thread.print` 清晰展示(线程状态、死锁高亮)。
- `[扩展]` `jex java heap <pid>` —— 堆概览(`jmap` / JFR 堆转储摘要)。
- `[扩展]` `jex java flame <pid>` —— async-profiler 火焰图(输出 HTML)。
- `[扩展]` `jex java rec <pid>` —— 起 Flight Recorder,停下后分析(分配压力 / GC 停顿拆解 / 热点方法)。
- `[扩展]` `jex java top` —— 实时面板(类 atop,持续刷新 GC/线程/CPU)。
- 报告可保存 / 导出(火焰图 HTML、GC 报告文本)。

## 6. 生态互通 `[扩展]`

- `jex import pom` —— 已有 Maven 项目直接用 `jex run` 跑(**降低新人门槛**,比 export 更优先)。
- `jex export maven` —— 导出 `pom.xml`(种子 pom,有损:scope 默认 compile、不处理 exclusions;降低退出成本)。
- `[远期]` `jex import gradle` / `jex export gradle` —— 与 Gradle 互转。

## 7. 开发者体验 / 工程化 `[远期]`

- `jex init` —— 脚手架(生成 `jex.toml` + 标准目录)。
- `jex fmt` —— 格式化(封装 google-java-format / spotless)。
- `jex test` —— 测试运行器(封装 JUnit;Java 已有 mvn test,价值低,可不做)。
- `jex build` / `jex clean` —— 构建 / 清理。
- monorepo 多模块(工作区,类似 cargo workspace)。
- IDE 集成:生成 `pom.xml` / 让 IDE 识别,或 `[远期]` LSP。
- `jex cache clean` —— 清缓存。
- 配置 `~/.jex/config.toml`(默认 repo、代理等)。
- shell 补全(zsh / bash / fish)。

## 8. 工具自身分发 `[扩展]`

- 单二进制下载(`curl ... | sh` 一键装)。
- `jex self update` —— 自更新。
- 跨平台 / 跨架构(linux/mac/windows × x64/aarch64)。

---

## 术语小词典

- **mise**:一个用 Rust 写的开发工具/版本管理器(前身 rtx),项目级钉语言与工具版本、跨设备一致、能自动安装。我们要的 JDK 模型与之同构。
- **Coursier**:JVM 上最快的依赖解析器,并行解析 + 统一缓存;`jex` 的解析引擎。
- **JFR(Java Flight Recorder)**:JVM 内置的低开销事件录制(分配、GC、方法热点),是深度诊断的数据源。
- **async-profiler**:采样型性能分析器,出 CPU/分配火焰图;火焰图的数据源。
- **Solr API**:Maven Central 的搜索接口(`search.maven.org/solrsearch`),`jex search` 查它找坐标。
- **锁文件(lockfile)**:冻结「解析后的确切版本」,保证别人跑出一模一样的结果(对标 `uv.lock`)。
- **传递依赖(transitive dependency)**:A 依赖 B、B 依赖 C,则 C 是 A 的传递依赖;Coursier 自动拉全。
