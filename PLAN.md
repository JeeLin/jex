# jx —— 把 uv / bun 的体验引入 Java 生态

> 一个**单文件二进制的 JVM 工具链 CLI**:版本管理 + uv 式依赖 + 一键运行 + 友好的 JDK 诊断封装 + 可导出 Maven。
> 定位:不是重造 Maven/Gradle,而是做一个「上手快、能随时退回 Maven」的轻量入口。

---

## 1. 这是什么(一句话)

`jx` = **SDKMAN 的版本管理** + **jbang 的一键运行** + **Coursier 的依赖解析** + **JDK 诊断工具(jstat/jcmd/jfr)的友好封装**,收进一个二进制。

对应 uv/bun 的能力拆解:

| uv / bun 能力 | jx 对应 |
|---|---|
| 版本管理 `uv python install` | `jx jdk install/use` |
| 依赖解析(快、可缓存) | `jx add`(底层 Coursier) |
| 一键运行 `uv run` / `bun x` | `jx run Main.java` |
| 包仓库 | Maven Central(直接连) |
| 工具链扩展(bun test/bundle) | `jx java gc/threads/heap/flame` |
| 导出到主流生态 | `jx export maven` → `pom.xml` |

---

## 2. 与现有工具的关系(不重造轮子)

| 已有工具 | 它干了什么 | jx 怎么用 |
|---|---|---|
| **jbang** | 运行 Java 像脚本 + 下载 JDK | `run`/`jdk` 是其超集,借鉴思路 |
| **Coursier** | JVM 上最快的依赖解析器 | **直接调用**,不自己写解析器 |
| **SDKMAN!** | JDK 多版本管理 | `jdk` 子命令复用其目录/逻辑 |
| **Elide** | 自称 "Bun for JVM"(内嵌 javac) | 最接近的实验性先验,参考其取舍 |
| **async-profiler** | 原生火焰图 | `jx java flame` 捆绑它 |

---

## 3. 技术选型(默认决策,可推翻)

| 决策 | 默认 | 理由 / 替代 |
|---|---|---|
| 工具本体语言 | **Rust** | 单原生二进制,**不需要 JVM 就能跑自己**(避免鸡生蛋:工具管 JDK,但自己不能依赖 JDK 启动)。uv 也是 Rust。替代:Go(更易上手)、Kotlin(你 Java 背景最熟,但工具自身需系统 JDK 来编译,仍可接受) |
| 依赖解析 | **Coursier**(`cs` 二进制) | JVM 里等价于 uv resolver 的东西,直接调,省 80% 最难活。MVP 用 shell `cs`,稳定后内嵌 coursier lib |
| JDK 下载源 | **Adoptium / Temurin API** | `api.adoptium.net`;若本机已装 SDKMAN 可 shell 委托 |
| 诊断底层 | **JDK 自带** `jstat`/`jcmd`/`jfr` + 捆绑 **async-profiler** | 不重造,只做解析/美化 |
| CLI 框架 | Rust `clap` + `anyhow` | 子命令 + 错误处理标准组合 |

> **关键判断**:工具自己用 Rust 写(免 JDK),但 `jx run` 需要**目标 JDK** —— 那由 M1 的 `jx jdk` 负责提供。两层分离后就没有鸡生蛋问题。

---

## 4. 项目结构(落地后长这样)

```
/workspace/java/
├── PLAN.md
├── Cargo.toml
├── src/
│   ├── main.rs           # 入口,注册子命令
│   ├── jdk.rs            # JDK 安装/切换/缓存
│   ├── deps.rs           # manifest 读写 + 调 Coursier
│   ├── run.rs            # 解析→编译→运行 + 缓存
│   ├── diag.rs           # jstat/jcmd/jfr 封装 + 美化
│   ├── export.rs         # 依赖树 → pom.xml
│   └── config.rs         # ~/.jx 配置目录
└── tests/
```

---

## 5. 核心数据模型:`jx.toml`

放在项目根目录,对标 `pyproject.toml` / `package.json`:

```toml
[project]
name = "demo"
java = "21"                 # 目标 JDK 版本(可锁定,影响 jx run 用哪个 JDK)

[dependencies]
"com.google.code.gson:gson"        = "2.11.0"   # Maven 坐标 = groupId:artifactId
"org.apache.commons:commons-lang3" = "3.14.0"

[repositories]            # 默认就是 Maven Central,可加私服
maven-central = true
```

- 坐标用完整 `groupId:artifactId`,版本字符串;不发明简写(避免和 Maven 坐标体系脱节)。
- 解析结果(含传递依赖)缓存到 `~/.jx/cache`,并落一份锁文件 `jx.lock.toml`(对标 `uv.lock`)。

---

## 6. 里程碑(每个都是可运行的纵向切片)

### M0 — 骨架 + CLI 框架
- **目标**:`jx --help` 列出所有子命令占位;`~/.jx` 配置目录就绪。
- **复用**:`clap`、`anyhow`。
- **自研**:项目初始化(cargo)、错误体系、配置目录。
- **验收**:能 `cargo run -- --help` 看到 `jdk / add / run / java / export` 子命令。

### M1 — JDK 版本管理 `jx jdk`
- **目标**:列可装/已装、`install 21` 下载 Temurin、`use 17` 切换(写 `.jx-version`)、`jx which java` 返回当前 JDK 的 `java` 路径。
- **复用**:Adoptium API(或 shell SDKMAN)。
- **自研**:下载/解压/缓存/版本切换逻辑。
- **验收**:装 21 后 `jx which java` 指向它;`jx run` 用对版本。

### M2 — uv 式依赖 + 一键运行 `jx init/add/run`
- **目标**:`init` 生成 `jx.toml`;`add <coord>` 写入并调 Coursier 解析、缓存 jar;`run Main.java` 解析→拼 classpath→`javac`→`java`,编译产物缓存。
- **复用**:Coursier(`cs`)。
- **自研**:manifest 读写、classpath 拼接、编译缓存。
- **验收**:写个用 gson 的 `Main.java`,`jx run` 能跑;二次运行命中缓存、不重新解析。

### M3 — 导出 Maven `jx export maven`
- **目标**:读解析后的依赖树 → 生成 `pom.xml`。
- **自研**:依赖树 → pom 模板(注意**有损**,见 §7)。
- **验收**:生成的 `pom.xml` 能 `mvn compile` 通过。

### M4 — `java` 命令拓展:诊断包装 `jx java ...`
- **目标**:
  - `jx java gc <pid>` —— `jstat -gcutil` 美化 + 列解释
  - `jx java threads <pid>` —— `jstack`/`jcmd Thread.print`
  - `jx java heap <pid>` —— `jmap` / JFR 堆概览
  - `jx java flame <pid>` —— async-profiler 火焰图(HTML)
- **复用**:JDK 自带工具 + async-profiler。
- **自研**:输出解析/美化。
- **验收**:对跑着的 Java 进程出可读报告。

### M5 — 深度 GC 分析(JFR)
- **目标**:`jx java rec <pid>` 起 Flight Recorder,停下后解析 JFR(分配压力 / GC 停顿拆解 / 热点方法)出分析报告。
- **复用**:JFR + 解析器(或 `jfr summary`/`jfr asm`)。
- **自研**:报告聚合。
- **验收**:对 demo 进程录 30s 出分析。

> **建议推进顺序**:M0 → M1 → M2 → M3 先跑通「版本 + 依赖 + 运行 + 导出」主线;M4 → M5 诊断后置。主线价值最大、最快可见。

---

## 7. 关键难点与应对(提前知道,别撞了才慌)

1. **不能真的给 `java` 加子命令。** `java` 是 JDK 自带二进制,无法插入 `gc` 子命令,除非 PATH 劫持。正确做法:`jx java gc ...` 二级命令,顶多提供可选 `alias java=jx-java`。设计里明说,不承诺「扩展 java 原生命令」。
2. **导出 pom 是有损的。** uv 能导出 `requirements.txt` 因两边都是扁平清单;Maven 有 `<scope>`/`<exclusions>`/`<profile>`/多模块,扁平 `jx.toml` 只能生成「种子 pom」——让人能 `mvn` 接着跑,非全等往返。定位成**降低退出成本**,不是双向同步。
3. **Coursier 调用方式。** MVP 用 shell `cs`(需用户先装 `cs`);稳定后内嵌 coursier lib(增体积/启动,但免外部依赖)。
4. **工具自身免 JDK。** Rust 原生二进制解决;但 `run` 的目标 JDK 由 M1 提供(两层分离)。

---

## 8. 需要你拍板的几件事

1. **工具本体语言**:Rust(默认,免 JDK、学习曲线陡)/ Go(更易上手)/ Kotlin(你最熟,但工具自身需系统 JDK 编译)。
2. **工具名**:`jx`(占位,顺口)/ 你另有偏好?
3. **v1 范围**:只做 M0–M3 主线(版本+依赖+运行+导出),还是把 M4 诊断也并进首版?

---

## 9. 建议的第一步

从 **M0 + M1** 起步:先立起 Rust 骨架和「JDK 版本管理」——这一步不依赖 Coursier、不依赖诊断,最容易跑通且立刻有用(`jx jdk install 21 && jx which java`)。

> 学习收益:这个项目把 **Rust + JVM 内部机制 + 构建系统 + 性能诊断** 串成一条线,正好嵌进你自动化重学地图的 Layer 0(编程工具链)。
