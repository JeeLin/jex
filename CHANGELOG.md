# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.2.0] - 2025-08-31

### Added
- JDK 版本管理：`jex jdk` 子命令（install/use/list/which/doctor），支持 Adoptium API 下载和 mise 模型版本切换
- 依赖管理：`jex init/add/remove/update` 子命令，管理 jex.toml 和 jex.lock.toml 锁文件
- 依赖搜索：`jex search` 子命令，查询 Maven Central Solr API，支持 `--versions` 列出全部版本
- 一键运行：`jex run` 子命令，自动解析依赖→拼 classpath→javac→java，基于 hash 跳过重复编译
- 诊断核心：`jex java gc/threads` 子命令，封装 jstat/jcmd 输出为可读报告
- 生态互通：`jex export maven` 子命令，从 jex.lock.toml 生成 pom.xml
- 公共工具模块：`src/util.rs` 提取坐标解析等共享函数

### Fixed
- 修复 parse_coord() 在 deps.rs 和 search.rs 中的重复定义，提取至共享 util 模块
- 修复 search.rs versions() 函数未接入 CLI 的问题，添加 `--versions` 选项
- 修复 export.rs 内联坐标解析未使用共享实现的问题
- 修复代码格式不一致问题（cargo fmt）
