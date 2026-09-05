# jex 产品文档

## 产品定位

`jex` 是一个**单文件二进制的 JVM 工具链 CLI**，将 uv/bun 的开发体验引入 Java 生态。

**一句话定位**：不是重造 Maven/Gradle，而是做一个「上手快、能随时退回 Maven」的轻量入口。

## 核心能力

| 能力 | 对应命令 | 说明 |
|------|----------|------|
| [已完成] JDK 版本管理 | `jex jdk` | mise 模型：声明式钉版 + 跨设备一致 |
| [已完成] 依赖管理 | `jex add/remove/update` | uv 式体验，底层 Coursier |
| [已完成] 依赖搜索 | `jex search` | apk 式搜索 Maven Central |
| [已完成] 一键运行 | `jex run` | 解析 → 编译 → 运行 |
| [已完成] 脚本模式 | `jex run script.java` | 直接运行单文件 Java 脚本（shebang + 文件内依赖声明 + 缓存编译） |
| [已完成] JDK 诊断 | `jex java gc/threads/...` | 取代难用的原生工具 |
| [已完成] 生态互通 | `jex export/import maven` | 降低退出成本（export + import 完整闭环，v0.12.0） |
| [已完成] REPL | `jex repl` | 交互式 Java 代码求值（基于 jshell，v0.11.0） |
| [已完成] Shell 补全 | `jex completions` | bash/zsh/fish/powershell 自动补全（v0.11.0） |
| [已规划] 项目模板 | `jex create` | 快速创建项目模板（web/api/cli/lib） |

### 1. 新项目启动
```bash
jex init --name demo        # 生成 jex.toml + src/
jex add com.google.code.gson:gson:2.11.0  # 添加依赖
```

### 2. 开发运行
```bash
jex run src/Main.java       # 一键运行
jex run -- --arg1 --arg2    # 传参
```

### 3. JDK 管理
```bash
jex jdk install 21          # 安装 JDK 21
jex jdk use 21              # 切换版本
jex jdk which               # 查看当前 JDK 路径
```

### 4. 依赖分析
```bash
jex tree                    # 依赖树
jex why com.google:gson     # 为何引入
jex conflict                # 冲突分析
```

### 5. 导出 Maven
```bash
jex export maven            # 生成 pom.xml
```

### 6. 脚本模式
```java
///usr/bin/env jex
//DEPS com.google.code.gson:gson:2.11.0
//JAVA 21

import com.google.gson.Gson;
import java.util.Map;

public class hello {
    public static void main(String[] args) {
        Map<String, String> data = Map.of("key", "value");
        System.out.println(new Gson().toJson(data));
    }
}
```
```bash
jex run hello.java          # 自动检测 shebang，走脚本模式
jex run src/Main.java       # 非脚本文件，走原有编译流程
```
## 功能边界

### 做什么
- 提供现代化的 CLI 体验（对标 uv/bun）
- 整合现有工具（Coursier、Adoptium、jstat/jcmd）
- 降低 Java 项目上手门槛
- 提供清晰的错误信息和建议

### 不做什么
- 不重造 Maven/Gradle 的构建逻辑
- 不替换 JVM 内部的 GC、线程管理
- 不做双向同步的 Maven/Gradle 互转（有损导出，降低退出成本）

## 用户画像

- **Java 新手**：想快速上手，不想学复杂的构建工具
- **脚本开发者**：写单文件 Java 脚本，想快速运行
- **团队协作**：需要跨设备一致的 JDK 版本管理
- **运维/诊断**：需要友好的 JVM 诊断工具
