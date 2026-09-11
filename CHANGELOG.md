# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

## [0.29.0] - 2025-01-27

### Added
- Interactive dependency tree browser: `jex tree -i` (ratatui TUI with vim navigation, search, expand/collapse)
- Interactive project health dashboard: `jex dashboard` (dependency updates, security audit, license overview)
- Interactive project init wizard: `jex init -i` (5-step guided project creation)
- TUI module: `crates/jex-core/src/tui/` with tree_view, dashboard, init_wizard
- 9 unit tests for init wizard navigation and TOML generation

## [0.28.0] - 2025-01-27

### Added
- i18n module: Chinese/English language switching (`Lang` enum, `msg!` macro)
- `jex config get/set/list` subcommand for managing `~/.jex/config.toml`
- Config options: JDK mirror, default JDK version, proxy, cache size, Maven mirror
- Windows compatibility: `dirs::home_dir()` instead of `HOME` env var
- Bilingual README (English/Chinese)
- 8 new i18n unit tests (419 total tests passing)

### Fixed
- config_set: atomic write via temp file + rename to prevent corruption
- Fixed command alias conflicts (audit, report, repl, cache, why)
- Renamed `Lang::from_str` to `Lang::parse_lang` (clippy: std trait conflict)
- Fixed unused variable warnings in export.rs, cache.rs

### Changed
- Improved test coverage from 63% to 68%

## [0.27.0] - 2025-01-20

### Added
- Monorepo support: `jex workspace` (init/list/status/build)
- Multi-module dependency sharing and batch operations

## [0.26.0] - 2025-01-15

### Added
- Hot reload: `jex watch` for automatic recompile on file changes
