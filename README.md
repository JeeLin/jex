# jex

**JVM 工具链 CLI** — 把 uv/bun 的开发体验带入 Java 生态。

不是重造 Maven/Gradle，而是做一个「上手快、能随时退回 Maven」的轻量入口。

[![Rust](https://img.shields.io/badge/Rust-1.70+-orange)](https://rust-lang.org)
[![License](https://img.shields.io/badge/License-MIT-blue)](LICENSE)
[![Version](https://img.shields.io/badge/Version-0.27.0-green)](CHANGELOG.md)

## 特性一览

| 能力 | 命令 | 说明 |
|------|------|------|
| 一键运行 | `jex run` | 解析 → 编译 → 运行，uv 式体验 |
| 脚本模式 | `jex run script.java` | 单文件 Java 脚本（shebang + 内联依赖） |
| JDK 管理 | `jex jdk` | 安装/切换/钉版，跨设备一致性校验 |
| 依赖管理 | `jex add/remove/update` | 底层 Coursier，apk 式搜索 |
| 依赖搜索 | `jex search` | 搜索 Maven Central |
| 依赖树 | `jex tree` | 可视化项目依赖树（`--verbose` 详细报告） |
| 依赖审计 | `jex audit` | 安全漏洞检查 + 自动修复 |
| 许可证检查 | `jex license` / `jex license-check` | 合规性检查 |
| 版本检查 | `jex outdated/upgrade` | 依赖更新检查与升级 |
| 依赖锁定 | `jex pin` | 锁定特定版本 |
| 兼容性检查 | `jex check` | 依赖版本兼容性校验 |
| 缓存管理 | `jex cache` | 清理/查看依赖缓存 |
| 分析报告 | `jex report` | 项目依赖综合分析报告 |
| 导入/导出 | `jex export/import` | Maven pom.xml 互通 |
| REPL | `jex repl` | 交互式 Java 代码求值（jshell） |
| 诊断 | `jex java gc/threads/heap` | JVM 诊断（gc / threads / heap / 火焰图） |
| 代码格式化 | `jex fmt` | google-java-format 集成 |
| 项目模板 | `jex create` | 快速创建项目（web/api/cli/lib） |
| 热重载 | `jex watch` | 监听文件变更自动编译运行 |
| Monorepo | `jex workspace` | 多模块项目统一管理 |
| Shell 补全 | `jex completions` | bash/zsh/fish/powershell |
| 版本日志 | `jex changelog` | 依赖版本变更日志 |

## 快速开始

### 安装

```bash
# 从源码构建
git clone https://github.com/user/jex.git
cd jex
cargo build --release
cp target/release/jex /usr/local/bin/
```

### 基本用法

```bash
# 初始化项目
jex init my-app
cd my-app

# 添加依赖
jex add com.google.code.gson:gson

# 运行
jex run src/Main.java

# 一键运行（解析 + 编译 + 运行）
jex run

# 传参
jex run -- --port 8080
```

### 脚本模式

```java
///usr/bin/env jex
//DEPS com.google.code.gson:gson:2.11.0

import com.google.gson.Gson;
public class hello {
    public static void main(String[] args) {
        System.out.println(new Gson().toJson("Hello, jex!"));
    }
}
```

```bash
chmod +x hello.java
./hello.java
```

### JDK 管理

```bash
jex jdk install 21          # 安装 JDK 21
jex jdk use 21              # 切换版本
jex jdk list                # 列出可用版本
jex jdk which               # 查看当前 JDK 路径
jex jdk doctor              # 校验跨设备一致性
```

### 依赖分析

```bash
jex tree                    # 依赖树
jex tree --verbose          # 详细报告（许可证/漏洞分布）
jex why com.google:gson     # 为何引入某依赖
jex conflict                # 冲突分析
jex outdated                # 检查依赖更新
jex upgrade                 # 升级依赖
```

### Monorepo 工作区

```bash
jex workspace init          # 初始化工作区
jex workspace list          # 列出模块
jex workspace status        # 显示汇总状态
jex workspace build         # 批量编译
```

### 热重载

```bash
jex watch                   # 监听 src/ 目录
jex watch --dir app/        # 监听指定目录
jex watch --debounce 300    # 设置防抖延迟（ms）
```

## 技术架构

| 组件 | 技术 |
|------|------|
| 工具本体 | Rust（单原生二进制） |
| CLI 框架 | clap + anyhow |
| 依赖解析 | Coursier (cs) |
| JDK 下载 | Adoptium API |
| 诊断 | jstat/jcmd + async-profiler |

### 项目结构

```
jex/
├── Cargo.toml              # 工作区配置
├── crates/
│   ├── jex-core/           # 核心库
│   │   └── src/
│   │       ├── lib.rs      # 模块注册
│   │       ├── config.rs   # 配置管理
│   │       ├── deps.rs     # 依赖解析
│   │       ├── jdk.rs      # JDK 管理
│   │       ├── run.rs      # 编译运行
│   │       ├── tree.rs     # 依赖树
│   │       ├── workspace.rs# Monorepo 支持
│   │       └── ...         # 其他模块
│   └── jex-cli/            # CLI 入口
│       └── src/main.rs     # 命令注册与分发
└── docs/
    ├── PRODUCT.md          # 产品文档
    └── DEVELOPMENT.md      # 开发设计文档
```

## 开发

### 前置要求

- Rust 1.70+
- JDK（运行时）
- Coursier（依赖解析）

### 构建

```bash
cargo build --release
```

### 测试

```bash
cargo test                  # 运行所有测试
cargo clippy -- -D warnings # Lint 检查
cargo llvm-cov              # 覆盖率报告
```

### 提交规范

```
feat:     新功能
fix:      Bug 修复
docs:     文档更新
refactor: 重构
test:     测试补充
chore:    构建/工具链变更
```

## 版本历史

详见 [CHANGELOG.md](CHANGELOG.md)

| 版本 | 主要变更 |
|------|----------|
| v0.27.0 | Monorepo 支持（`jex workspace`） |
| v0.26.0 | 热重载（`jex watch`） |
| v0.25.0 | 依赖树增强（`jex tree --verbose`） |
| v0.24.0 | 依赖版本变更日志（`jex changelog`） |
| v0.23.0 | 安全审计增强（`jex audit --fix`） |
| v0.22.0 | 依赖版本兼容性检查（`jex check`） |
| v0.21.0 | 许可证自动检查（`jex license-check`） |
| v0.20.0 | 依赖缓存管理（`jex cache`） |
| v0.19.0 | 依赖版本锁定（`jex pin`） |
| v0.18.0 | 项目依赖分析报告（`jex report`） |
| v0.17.0 | 依赖树可视化（`jex tree`） |

## 许可证

[MIT](LICENSE)
