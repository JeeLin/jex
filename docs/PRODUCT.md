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
| [已完成] 项目模板 | `jex create` | 快速创建项目模板（web/api/cli/lib，v0.13.0） |
| [已完成] 依赖版本检查 | `jex outdated/upgrade` | 检查依赖更新并自动升级（v0.14.0） |
| [已完成] 依赖安全检查 | `jex audit` | 检查依赖已知漏洞（v0.15.0） |
| [已完成] 依赖许可证检查 | `jex license` | 检查依赖许可证合规性（v0.16.0） |
| [已完成] 依赖树可视化 | `jex tree` | 可视化项目依赖树结构（v0.17.0） |
| [已完成] 项目依赖分析报告 | `jex report` | 生成项目依赖综合分析报告（v0.18.0） |
| [已完成] 依赖版本锁定 | `jex pin` | 锁定特定依赖版本（v0.19.0） |
| [已完成] 依赖缓存管理 | `jex cache` | 管理依赖缓存（v0.20.0） |
| [已完成] 依赖许可证自动检查 | `jex license-check` | 自动检查依赖许可证合规性（v0.21.0） |
| [已完成] 依赖版本兼容性检查 | `jex check` | 检查依赖版本兼容性（v0.22.0） |
| [已完成] 依赖安全审计增强 | `jex audit --fix` | 自动修复已知漏洞的依赖版本（v0.23.0） |

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
| [已完成] 依赖版本更新日志 | `jex changelog` | 生成依赖版本更新日志（v0.24.0） |
| [已完成] 依赖依赖树增强 | `jex tree --verbose` | 显示依赖树详细信息（版本、许可证、漏洞） |
| [已规划] 热重载 | `jex watch` | 文件变更自动重新编译运行（开发体验） |
| [已完成] Monorepo 支持 | `jex workspace` | 多模块项目统一管理（依赖共享、批量操作） |
| [待规划] 交互式依赖树浏览 | `jex tree -i` | 基于 ratatui 的 TUI 交互式依赖树浏览（展开/折叠/搜索/过滤） |
| [待规划] 交互式报告仪表盘 | `jex dashboard` | 基于 ratatui 的 TUI 仪表盘（依赖健康度、漏洞概览、许可证分布） |
| [待规划] 交互式项目初始化 | `jex init -i` | 基于 ratatui 的 TUI 向导式项目创建（模板选择、配置预览） |
| [待规划] IDE 集成 | `jex ide` | 与主流 IDE（VS Code/IntelliJ）集成（项目检测、依赖提示） |
