# jx 详细设计(M0–M3)—— 接口与数据契约

> 本文把 PLAN.md 的 M0–M3 落到**可调用的接口级规格**:命令行为、配置文件字段、Coursier 调用命令、目录布局。
> 范围:Rust 单二进制,做「JDK 管理 + uv 式依赖 + 一键运行 + 导出 Maven」。M4/M5 诊断不在本文。

---

## 1. 目录布局

**全局(`~/.jx/`,工具自己管):**
```
~/.jx/
├── jdks/              # 已安装 JDK,JDK 根直接是 <version>/ (内含 bin/java)
│   ├── 21/
│   └── 17/
├── cache/             # Coursier 的 jar 缓存(cs --cache 指向这里)
├── bin/               # 自动下载的工具,如 cs(Coursier CLI)
├── jdk-current        # 全局默认 JDK 版本号(单行文本)
└── config.toml        # 全局配置(默认 repo 等,可空)
```

**项目级(放在项目根):**
```
<project>/
├── jx.toml           # 声明(手改)
├── jx.lock.toml      # 锁文件(自动生成,勿手改)
├── .jx-version       # JDK 版本钉(单行,优先级高于全局)
└── .jx-build/        # 编译输出(加入 .gitignore)
```

**JDK 生效优先级**:`项目 .jx-version` → `全局 ~/.jx/jdk-current` → 系统 `java`(回退)。

---

## 2. M1 —— `jx jdk`(版本管理)

| 命令 | 行为 |
|---|---|
| `jx jdk list` | 列出**已装**版本(标 ✔)与**当前**版本(标 →)。已装来自 `~/.jx/jdks/`。 |
| `jx jdk list --remote` | 调 Adoptium API 列出可装版本(LTS + 最新)。 |
| `jx jdk install <ver>` | 下载并解压 Temurin `<ver>` 到 `~/.jx/jdks/<ver>/`。<ver> 可填 `21`(特性版本,装最新 GA)或 `21.0.2`(精确)。 |
| `jx jdk use <ver>` | 设生效版本:有项目(根有 `jx.toml` 或 `.jx-version`)写 `.jx-version`,否则写 `~/.jx/jdk-current`。 |
| `jx jdk which` | 打印当前生效 JDK 的 `JAVA_HOME`(即 `~/.jx/jdks/<ver>` 或系统路径)。 |
| `jx jdk uninstall <ver>` | 删 `~/.jx/jdks/<ver>/`。 |

**Adoptium API(下载源):**
- 可用版本:`GET https://api.adoptium.net/v3/info/available_releases`(返回 LTS 列表)。
- 下载 URL:`GET https://api.adoptium.net/v3/binary/latest/{feature}/ga/{os}/{arch}/jdk/hotspot/normal/eclipse`
  - `{os}` = linux/darwin/windows(由 `uname -s` 映射);`{arch}` = x64/aarch64(由 `uname -m` 映射)。
  - 返回重定向到 `.tar.gz`(linux/mac)或 `.zip`(windows),工具下载后解压。
- 注:精确路径以 Adoptium 当前 API 为准,实现时先 `curl` 验证一次。

**版本字符串归一化**:接受 `21` / `17` / `temurin-21` / `21.0.2`,统一存为特性版本目录名(如 `21`);精确补丁版作为元数据记在 `jdks/<ver>/.jx-meta.json`。

---

## 3. M2 —— `jx init / add / run / remove`(依赖 + 运行)

| 命令 | 行为 |
|---|---|
| `jx init [--name X]` | 项目根生成 `jx.toml`,`java` 字段取当前生效 JDK 版本。已有则报错不覆盖。 |
| `jx add <coord> [--version V]` | `<coord>` = `groupId:artifactId[:version]`。无 version 时经 Coursier 取 latest。写入 `jx.toml [dependencies]`,并立即解析+缓存,更新 `jx.lock.toml`。 |
| `jx run <file.java> [-- <args>]` | 见下「运行流程」。不带 `<file>` 时用 `jx.toml [project].main`。 |
| `jx remove <coord>` | 从 `jx.toml` 删依赖,重算 `jx.lock.toml`。 |

**运行流程(`jx run`):**
1. 无 `jx.toml` → 提示先 `jx init`(或自动 init)。
2. 读 `jx.toml` + `jx.lock.toml`,用 `~/.jx/cache` 里的 jar 拼 classpath。
3. 编译:对 `<file.java>` 及同项目源码 `javac -cp <cp> -d .jx-build`。基于「源文件 hash + 依赖 hash」决定是否跳过(命中缓存则不重编)。
4. 运行:`java -cp .jx-build:<cp> <MainClass> [args]`,JDK 取 §1 优先级里的那个。
5. `-- <args>` 之后的参数原样传给程序。

---

## 4. `jx.toml` 字段规格(精确)

```toml
[project]
name    = "demo"            # 必填。用于 build 目录名 + 导出 artifactId
java    = "21"              # 选填。目标 JDK 特性版本;省略→用当前生效 JDK
main    = "src/Main.java"   # 选填。默认入口,`jx run` 无参时用

[dependencies]
# 简写:值直接是版本号
"com.google.code.gson:gson"        = "2.11.0"   # 键 = groupId:artifactId
"org.apache.commons:commons-lang3" = "3.14.0"

# 完整写法:内联表,可带 exclusions(排除的传递依赖,按 g:a,不限版本)
"com.example:foo" = { version = "1.2.0", exclude = [
    "com.example:bar",          # 从 foo 的传递依赖里剔除(与 Maven <exclusion> 同语义)
    "org.slf4j:slf4j-api",
] }

[repositories]
maven-central = true         # 默认开;关闭则需显式列私服
# my-nexus     = "https://repo.example.com/maven"   # 选填私服

[build]
output        = ".jx-build"                     # 选填。编译输出目录,默认 .jx-build
sources       = ["src"]                          # 选填。源码目录,默认 src
resources     = ["src/main/resources"]           # 选填。资源目录
compiler-args = ["-parameters", "-encoding", "UTF-8"]   # 选填。透传给 javac
jvm-args      = ["-Xmx512m"]                     # 选填。透传给 java 运行
env           = { APP_ENV = "dev" }              # 选填。运行环境变量
```

- 坐标键**必须**是 `groupId:artifactId`(Coursier/Maven 同款分隔符 `:`)。
- `version` 支持精确 `2.11.0` 与范围 `[2.0,3.0)`(交 Coursier 解释)。

---

## 5. `jx.lock.toml` 规格(对标 uv.lock)

自动生成,记录**完整解析后的扁平依赖树**(含传递依赖),保证可复现:

```toml
# 自动生成,勿手改
lockfile-version = 1
[dependencies]
"com.google.code.gson:gson"         = "2.11.0"
"org.apache.commons:commons-lang3"  = "3.14.0"
"com.google.code.findbugs:jsr305"   = "3.0.2"   # 传递依赖,被显式列出
```

- 每项附 `hash`(选填)用于编译缓存失效判断。
- `jx add/remove` 与 `jx run`(当 toml 变)时重算。

---

## 6. Coursier 调用方式(精确命令)

MVP 用 shell `cs`(Coursier CLI)。若 `cs` 不在 PATH,`jx` 首次使用时自动下载到 `~/.jx/bin/cs`(从 Coursier GitHub Release)。

| 用途 | 命令 |
|---|---|
| 解析+下载某依赖(含传递)到缓存 | `cs fetch <g:a:v> --cache ~/.jx/cache` |
| 直接拿 classpath 字符串 | `cs fetch -p <g:a:v> --cache ~/.jx/cache` → 打印 `a.jar:b.jar:...` |
| 取 latest 版本号 | `cs complete <g:a>` 或 `cs resolve <g:a>`(看范围解析) |
| 看传递依赖树 | `cs dependency-tree <g:a:v>` |

- `jx` 内部统一用 `cs fetch -p` 拿 classpath,直接喂给 `javac`/`java` 的 `-cp`。
- 缓存:Coursier 自管在 `~/.jx/cache`,`jx` 不重复实现。
- 坐标分隔符 `:` 与 Maven 一致;版本范围 `[1.0,2.0)` 原生支持。
- 稳定后(可选):内嵌 coursier Java lib 消除外部 `cs` 依赖 —— MVP 不做。

---

## 7. M3 —— `jx export maven`(导出 pom.xml)

`jx export maven [--out pom.xml]`:

1. 读 `jx.lock.toml` 的扁平依赖列表。
2. 生成 `pom.xml`:
   - `groupId` 默认 `local.<project.name>`(可在 `jx.toml` 配);`artifactId` = `[project].name`;`version` = `0.1.0`。
   - `<dependencies>` 逐条 `<dependency><groupId><artifactId><version>`。
3. **有损说明**:`scope` 全默认 `compile`(v1 不支持 test/provided 等 scope);`properties`/多模块不生成。**`exclusions` 已支持**(见 §4 的 `exclude`),`export maven` 会原样输出 `<exclusions>`。定位是「种子 pom / 降低退出成本」,非双向同步。
4. **验收**:`mvn -f pom.xml compile` 通过。

> 巧思:因为 lock 是 flat 全量列表,导出的 pom **不会**触发 Maven 重新解析传递依赖的版本冲突 —— 比手写的窄 pom 更稳。

---

## 8. 术语小词典

- **JDK / JRE**:JDK = Java 开发工具包(含编译器 `javac` + 运行 `java`);JRE = 仅运行环境。装的是 JDK。
- **classpath(类路径)**:JVM 找 `.class` / `.jar` 的搜索路径,多个用 `:` 分隔。`-cp` 指定。
- **传递依赖(transitive dependency)**:A 依赖 B,B 又依赖 C,则 C 是 A 的传递依赖。Coursier 自动拉全。
- **GA(General Availability)**:正式发布版(相对预览/早期访问)。`21` 装最新 GA。
- **lockfile(锁文件)**:把「解析后的确切版本」冻结下来,保证别人跑出来一模一样(对标 `uv.lock` / `package-lock.json`)。
- **LTS(Long Term Support)**:长期支持版(如 8/11/17/21),企业常用。

---

## 9. 下一步

设计契约已锁。下一步可选:
- **scaffold M0**:`cargo init` + `clap` 子命令骨架(`jdk`/`add`/`run`/`export` 占位)+ `~/.jx` 配置目录。
- **细化 M1**:先实现 Adoptium 下载 + JDK 切换(不依赖 Coursier,最容易跑通)。
- 你定。

## 10. 策略:先整合现有命令行,后续逐个自实现 UX 层

**原则**:只替换「客户端 / UX 层」,**永远不替换「数据源 / 真相源」**。JVM 内部的 GC、线程、profile 数据来自 HotSpot;async-profiler 是 profiling agent;Maven Central / Adoptium 是服务 —— 这些始终外部。我们替换的是「人怎么调、怎么看」的那一层。

**整合 → 替代 映射:**

| 能力 | v1:整合现有命令行 | 后续:自己实现的替代 |
|---|---|---|
| JDK 下载/安装 | 调 Adoptium API(本就是服务) | 自管镜像/校验(替换*我们的下载层*,不是源) |
| 依赖解析 | shell `cs`(Coursier) | 内嵌 coursier lib,去外部依赖;**解析算法不重造** |
| 一键运行 | 调 `javac`+`java` | 始终调(编译器/运行器是 JDK 的,不重造) |
| GC 展示 | 包 `jstat`(文本解析) | 直连 JVM attach / PerfData / JMX,结构化读取 |
| 线程展示 | 包 `jcmd Thread.print` | 同上,走结构化路径 |
| 堆/火焰/JFR | 包 `jmap` / async-profiler / `jfr` | 火焰图:**捆绑** async-profiler(不重造);JFR:**自写解析器**替代 `jfr` CLI |

**要点:**
1. `javac`/`java`/Adoptium/Maven Central 永远不碰内部,只集成。
2. async-profiler 是特例:捆绑进发布包,不重写(等价 profiler 不现实)。
3. **整合阶段唯一陷阱**:`jstat`/`jcmd` 是爬文本,跨 JDK 版本/locale 易脆。缓解:整合期尽量用结构化输出(`jcmd <pid> PerfCounter.print`、JMX);替代期直接用 attach API,彻底不爬文本。
4. 此策略顺带定了诊断 v1 范围:`gc`+`threads` 用 `jstat`/`jcmd` 整合;`flame`/`rec` 在替代期用捆绑 async-profiler + 自写 JFR 解析做。
