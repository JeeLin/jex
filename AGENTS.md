# jex 项目约定

## 项目定位

`jex` = Rust 单二进制 JVM 工具链 CLI，对标 uv/bun for Java。
整合 SDKMAN + jbang + Coursier + JDK 诊断。

## 技术栈

| 组件 | 技术 | 说明 |
|------|------|------|
| 工具本体 | Rust | 单原生二进制，不需要 JVM |
| CLI 框架 | clap + anyhow | 子命令 + 错误处理 |
| 依赖解析 | Coursier (cs) | MVP 用 shell cs，稳定后内嵌 |
| JDK 下载 | Adoptium API | api.adoptium.net |
| 诊断 | jstat/jcmd + async-profiler | 不重造，只做美化 |

## 目录结构

```
/workspace/java/
├── AGENTS.md              # 本文件
├── Cargo.toml             # Rust 项目清单
├── src/
│   ├── main.rs            # 入口，注册子命令
│   ├── config.rs          # ~/.jex 配置目录 + 项目脚手架
│   └── error.rs           # 错误体系
├── docs/
│   ├── PRODUCT.md         # 产品文档
│   └── DEVELOPMENT.md     # 开发设计文档
└── .dev-flow/
    └── milestones/        # 里程碑文档
```

## 开发规范

### 代码风格
- 遵循 Rust 官方 style guide
- 使用 `cargo fmt` 格式化
- 使用 `cargo clippy` 检查

### 提交规范
- 一个子功能点一个 commit
- commit message 格式：`<type>: <description>`
- type: feat / fix / docs / refactor / chore

### 测试
- 单元测试：`cargo test`
- 编译检查：`cargo check`
- Lint 检查：`cargo clippy -- -D warnings`

## 质量门禁

| 检查项 | 命令 | 通过标准 |
|--------|------|----------|
| 编译检查 | `cargo check` | 无 error |
| Lint 检查 | `cargo clippy -- -D warnings` | 无 error（warning 可忽略） |
| 测试 | `cargo test` | 全部通过 |

## 审查维度

- **功能完整性**：子任务是否完整实现
- **代码质量**：命名、结构、错误处理
- **安全性**：文件操作、网络请求、命令注入
- **可维护性**：模块划分、职责清晰
