use clap::{Args, Parser, Subcommand, ValueEnum};
use jex_core::error::Result;
use jex_core::{audit, cache, deps, diag, export, fmt, import, jdk, jfr, license, license_check, outdated, pin, profiler, report, run, search, template, tree};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "jex",
    version,
    about = "uv/bun for Java —— 单二进制 JVM 工具链 CLI",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// JDK 版本管理(安装 / 切换 / 钉版 / 一致性校验)
    #[command(alias = "j")]
    #[command(subcommand)]
    Jdk(JdkCommand),

    /// 脚手架:生成 jex.toml + 标准目录
    Init(InitArgs),

    /// 添加依赖
    #[command(alias = "a")]
    Add(AddArgs),

    /// 删除依赖
    #[command(alias = "rm")]
    Remove(RemoveArgs),

    /// 更新依赖
    #[command(alias = "up")]
    Update,

    /// 搜索 Maven 坐标(apk 式)
    #[command(alias = "s")]
    Search(SearchArgs),

    /// 一键运行(解析 → 编译 → 运行)
    #[command(alias = "r")]
    Run(RunArgs),

    /// 仅编译
    #[command(alias = "b")]
    Build(BuildArgs),

    /// 依赖树
    #[command(alias = "t")]
    Tree(TreeArgs),

    /// 为何引入某依赖
    #[command(alias = "w")]
    Why(WhyArgs),

    /// 依赖冲突分析
    #[command(alias = "cf")]
    Conflict,

    /// 依赖分析(未使用 / 未声明)
    #[command(alias = "an")]
    Analyze,

    /// 导出 pom.xml
    #[command(alias = "e")]
    Export,

    /// 从 pom.xml 导入
    #[command(alias = "i")]
    Import(ImportArgs),

    /// 代码格式化(google-java-format)
    #[command(alias = "f")]
    Fmt(FmtArgs),

    /// JVM 诊断(gc / threads / heap / 火焰图 / 录制)
    #[command(subcommand)]
    Java(JavaCommand),

    /// 交互式 Java 求值(jshell)
    #[command(alias = "rp")]
    Repl(ReplArgs),

    /// 生成 shell 自动补全脚本
    Completions(CompletionsArgs),

    /// 自更新
    #[command(alias = "su")]
    #[command(subcommand)]
    Self_(SelfCommand),

    /// 创建新项目
    #[command(alias = "c")]
    Create(CreateArgs),

    /// 检查依赖更新
    #[command(alias = "o")]
    Outdated,

    /// 检查依赖安全漏洞
    #[command(alias = "a")]
    Audit(AuditArgs),
    /// 升级依赖
    #[command(alias = "u")]
    Upgrade(UpgradeArgs),

    /// 检查依赖许可证
    #[command(alias = "l")]
    License(LicenseArgs),

    /// 生成项目依赖分析报告
    #[command(alias = "r")]
    Report(ReportArgs),

    /// 锁定依赖版本
    #[command(alias = "p")]
    Pin(PinArgs),

    /// 管理依赖缓存
    #[command(alias = "c")]
    Cache(CacheArgs),

    /// 检查依赖许可证合规性
    #[command(alias = "lc")]
    LicenseCheck,
}
#[derive(Subcommand)]
enum JdkCommand {
    /// 下载并安装指定版本
    Install(InstallArgs),
    /// 切换版本(写 .jex-version)
    Use(UseArgs),
    /// 列出可装 / 已装版本
    List,
    /// 返回当前 JDK 的 java 路径
    Which,
    /// 校验跨设备一致性
    Doctor,
}

#[derive(Subcommand)]
enum JavaCommand {
    Gc(GcArgs),
    /// 线程概览
    Threads(ThreadsArgs),
    /// 堆概览
    Heap(GcArgs),
    /// 火焰图(async-profiler)
    Flame(FlameArgs),
    /// 录制 Flight Recorder
    Rec(RecArgs),
    /// 分析 .jfr 文件
    Analyze(AnalyzeArgs),
    /// 实时面板
    Top(GcArgs),
}

#[derive(Subcommand)]
enum SelfCommand {
    /// 检查并更新到最新版本
    Update(SelfUpdateArgs),
}

#[derive(Args)]
struct SelfUpdateArgs {
    /// 仅检查是否有新版本，不执行更新
    #[arg(long)]
    check: bool,
}

#[derive(Args)]
struct InitArgs {
    /// 项目名
    name: Option<String>,
}

#[derive(Args)]
struct AddArgs {
    /// 坐标 group:artifact:version
    coord: String,
    /// 排除的依赖(可多次)
    #[arg(long)]
    exclude: Vec<String>,
}

#[derive(Args)]
struct RemoveArgs {
    coord: String,
}

#[derive(Args)]
struct SearchArgs {
    query: String,
    #[arg(short, long, default_value_t = 20)]
    limit: usize,
    /// 列出指定 artifact 的全部可用版本
    #[arg(long)]
    versions: bool,
}

#[derive(Args)]
struct RunArgs {
    /// 入口 .java 文件
    file: String,
    /// 传给程序的参数
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

#[derive(Args)]
struct BuildArgs {
    /// 清理编译缓存后重新编译
    #[arg(long)]
    clean: bool,
}

#[derive(Args)]
struct WhyArgs {
    coord: String,
}

#[derive(Args)]
struct InstallArgs {
    version: String,
}

#[derive(Args)]
struct UseArgs {
    version: String,
}

#[derive(Args)]
struct FlameArgs {
    /// 进程 ID
    pid: u32,
    /// 采样时长（秒）
    #[arg(short, long, default_value_t = 10)]
    duration: u32,
    /// 输出 SVG 路径
    #[arg(short, long)]
    output: Option<String>,
}

#[derive(Args)]
struct GcArgs {
    /// 进程 ID
    pid: u32,
}

#[derive(Args)]
struct ThreadsArgs {
    /// 进程 ID
    pid: u32,
}

#[derive(Args)]
struct FmtArgs {
    /// 要格式化的文件或目录(默认当前目录)
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,
    /// 检查模式(不修改文件，只报告)
    #[arg(long)]
    check: bool,
    /// 输出格式化后的代码到 stdout
    #[arg(long)]
    stdout: bool,
    /// 增量模式(只格式化 git diff 变更的文件)
    #[arg(long)]
    changed: bool,
}

#[derive(Args)]
struct RecArgs {
    /// JVM 进程 PID
    pid: u32,
    /// 录制时长（秒），不指定则交互式录制
    #[arg(short, long)]
    duration: Option<u32>,
    /// 输出 .jfr 文件路径
    #[arg(short, long)]
    output: Option<String>,
}

#[derive(Args)]
struct AnalyzeArgs {
    /// .jfr 文件路径
    file: String,
    /// 按事件类型过滤
    #[arg(short, long)]
    r#type: Option<String>,
    /// 展示 top N 热点
    #[arg(short, long, default_value = "10")]
    top: usize,
}

#[derive(Args)]
struct ReplArgs {
    /// 仅加载已编译的 class（不加载依赖 jar）
    #[arg(long)]
    class_only: bool,
}

#[derive(Clone, ValueEnum)]
enum Shell {
    Bash,
    Zsh,
    Fish,
    Powershell,
}

#[derive(Args)]
struct CompletionsArgs {
    /// 目标 shell
    shell: Shell,
}

#[derive(Args)]
struct ImportArgs {
    /// pom.xml 路径（默认当前目录 pom.xml）
    #[arg(default_value = "pom.xml")]
    path: String,
}

#[derive(Args)]
struct CreateArgs {
    /// 项目名称
    name: String,
    /// 模板类型（lib/cli/api/web，默认 lib）
    #[arg(short, long, default_value = "lib")]
    template: String,
    /// 包名（默认 com.example）
    #[arg(short, long, default_value = "com.example")]
    package: String,
}

#[derive(Args)]
struct UpgradeArgs {
    /// 依赖坐标（留空则升级全部）
    coord: Option<String>,
}

#[derive(Args)]
struct AuditArgs {
    /// 输出 JSON 格式报告
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct LicenseArgs {
    /// 输出 JSON 格式报告
    #[arg(long)]
    json: bool,
    /// 仅检查合规性
    #[arg(long)]
    check: bool,
}

#[derive(Args)]
struct TreeArgs {
    /// 显示深度限制
    #[arg(short, long)]
    depth: Option<usize>,
    /// 输出 JSON 格式
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ReportArgs {
    /// 输出 JSON 格式报告
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct PinArgs {
    /// 依赖坐标（如 com.google.code.gson:gson）
    coord: Option<String>,

    /// 锁定所有依赖
    #[arg(long)]
    all: bool,

    /// 显示锁定状态
    #[arg(long)]
    list: bool,

    /// 解锁指定依赖
    #[arg(long)]
    unpin: Option<String>,
}

#[derive(Args)]
struct CacheArgs {
    /// 清理缓存
    #[command(subcommand)]
    command: CacheCommand,
}

#[derive(Subcommand)]
enum CacheCommand {
    /// 清理缓存
    Clean {
        /// 清理全局缓存
        #[arg(long)]
        global: bool,
    },
    /// 显示缓存内容
    List,
    /// 显示缓存路径
    Path,
}
fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("错误: {e}");
        std::process::exit(1);
    }
}

/// 根据 FmtArgs 的 check/stdout 标志确定输出模式
fn fmt_output_mode(args: &FmtArgs) -> fmt::OutputMode {
    if args.stdout {
        fmt::OutputMode::Stdout
    } else if args.check {
        fmt::OutputMode::Check
    } else {
        fmt::OutputMode::Write
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Jdk(c) => match c {
            JdkCommand::Install(a) => jdk::install(&a.version),
            JdkCommand::Use(a) => jdk::use_version(&a.version),
            JdkCommand::List => jdk::list(),
            JdkCommand::Which => jdk::which(),
            JdkCommand::Doctor => jdk::doctor(),
        },
        Commands::Init(a) => deps::init(a.name.as_deref()),
        Commands::Add(a) => deps::add(&a.coord, None),
        Commands::Remove(a) => deps::remove(&a.coord),
        Commands::Update => deps::update(None),
        Commands::Search(a) => {
            if a.versions {
                search::versions(&a.query)
            } else {
                search::search(&a.query, a.limit)
            }
        }
        Commands::Run(a) => {
            use jex_core::script::parse_script;
            use std::path::Path;
            let path = Path::new(&a.file);
            if path.exists() {
                if let Ok(meta) = parse_script(path) {
                    if meta.is_script {
                        // Script mode: use cache compilation
                        let class_dir = run::get_or_compile(path, &meta)?;
                        println!("脚本模式：{}", a.file);
                        println!("缓存目录：{}", class_dir.display());
                        return Ok(());
                    }
                }
            }
            // Fallback to original run
            run::run(&a.file, &a.args)
        }
        Commands::Build(a) => {
            let files = run::collect_java_files()?;
            if files.is_empty() {
                println!("src/ 下没有 .java 文件");
                return Ok(());
            }
            let file_refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
            let (build, _classpath) = run::compile(&file_refs, a.clean)?;
            println!("✅ Build complete → {}", build.display());
            Ok(())
        }
        Commands::Tree(a) => {
            let tree = tree::build_dependency_tree()?;

            if a.json {
                println!("{}", serde_json::to_string_pretty(&tree)?);
            } else {
                let output = tree::render_tree(&tree, a.depth);
                println!("{}", output);
            }
            Ok(())
        },
        Commands::Report(a) => {
            let report = report::generate_report()?;

            if a.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                let output = report::render_report(&report);
                println!("{}", output);
            }
            Ok(())
        },
        Commands::Pin(a) => {
            if a.list {
                let pinned = pin::list_pinned()?;
                if pinned.is_empty() {
                    println!("没有锁定的依赖");
                } else {
                    println!("锁定的依赖:");
                    for dep in &pinned {
                        println!("  {}:{}", dep.coord, dep.version);
                    }
                }
                Ok(())
            } else if let Some(unpin_coord) = &a.unpin {
                pin::unpin_dependency(unpin_coord)
            } else if a.all {
                pin::pin_all()
            } else if let Some(coord) = &a.coord {
                pin::pin_dependency(coord)
            } else {
                eprintln!("请指定依赖坐标或使用 --all/--list/--unpin");
                std::process::exit(1);
            }
        },
        Commands::Why(a) => deps::why(&a.coord),
        Commands::Cache(a) => match &a.command {
            CacheCommand::Clean { global } => cache::clean_cache(*global),
            CacheCommand::List => {
                let entries = cache::list_cache()?;
                if entries.is_empty() {
                    println!("缓存为空");
                } else {
                    println!("缓存内容:");
                    for entry in &entries {
                        println!("  {} ({})", entry.name, format_size(entry.size));
                    }
                }
                Ok(())
            }
            CacheCommand::Path => {
                let path = cache::cache_path()?;
                println!("{}", path.display());
                Ok(())
            }
        },
        Commands::LicenseCheck => {
            let report = license_check::check_all_licenses()?;
            println!("📋 许可证合规性报告");
            println!("═══════════════════════════════════════");
            println!("✅ 兼容: {} 个", report.compatible.len());
            println!("❌ 不兼容: {} 个", report.incompatible.len());
            println!("❓ 未知: {} 个", report.unknown.len());
            if !report.incompatible.is_empty() {
                println!("\n不兼容的依赖:");
                for dep in &report.incompatible {
                    println!("  {}", dep);
                }
            }
            if !report.unknown.is_empty() {
                println!("\n未知许可证的依赖:");
                for dep in &report.unknown {
                    println!("  {}", dep);
                }
            }
            Ok(())
        },
        Commands::Conflict => deps::conflict(),
        Commands::Analyze => {
            use jex_core::analyze;
            analyze::analyze_project()
        }
        Commands::Export => export::maven(),
        Commands::Import(a) => {
            let pom_path = std::path::Path::new(&a.path);
            import::import_maven(pom_path)
        }
        Commands::Fmt(a) => {
            let mut config = fmt::FmtConfig::default();
            if let Ok(toml_config) = jex_core::config::read_fmt_config() {
                config = toml_config;
            }
            let mode = fmt_output_mode(&a);
            if a.changed {
                let files = fmt::format_changed(&config)?;
                if files.is_empty() {
                    println!("没有需要格式化的变更文件");
                    return Ok(());
                }
                for file in &files {
                    if let Err(e) = fmt::format_and_output(file, &config, mode) {
                        eprintln!("格式化 {file:?} 失败: {e}");
                    }
                }
            } else {
                for path_str in &a.paths {
                    let path = std::path::Path::new(path_str);
                    if path.is_file() {
                        if let Err(e) = fmt::format_and_output(path, &config, mode) {
                            eprintln!("格式化 {path:?} 失败: {e}");
                        }
                    } else if path.is_dir() {
                        for entry in std::fs::read_dir(path)? {
                            let entry = entry?;
                            let entry_path = entry.path();
                            if entry_path.is_file()
                                && entry_path.to_string_lossy().ends_with(".java")
                            {
                                if let Err(e) = fmt::format_and_output(&entry_path, &config, mode) {
                                    eprintln!("格式化 {:?} 失败: {e}", entry_path);
                                }
                            }
                        }
                    } else {
                        eprintln!("路径不存在或不是文件/目录: {path:?}");
                    }
                }
            }
            Ok(())
        }
        Commands::Java(c) => match c {
            JavaCommand::Gc(a) => diag::gc_tui(a.pid),
            JavaCommand::Threads(a) => diag::threads_tui(a.pid),
            JavaCommand::Heap(a) => diag::display_heap(a.pid),
            JavaCommand::Flame(a) => {
                let flame = profiler::profile(a.pid, a.duration)?;
                if let Some(ref path) = a.output {
                    let dest = std::path::PathBuf::from(path);
                    if let Some(parent) = dest.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::copy(&flame.svg_path, &dest).map_err(|e| {
                        jex_core::error::Error::new(format!("复制 SVG 到 {path} 失败: {e}"))
                    })?;
                    println!("📊 火焰图已复制到: {path}");
                } else {
                    println!("📊 火焰图已生成: {}", flame.svg_path.display());
                }
                Ok(())
            }
            JavaCommand::Rec(a) => {
                let output_path = a.output.as_ref().map(std::path::PathBuf::from);
                let session = jfr::start_recording(a.pid, a.duration)?;
                let jfr_path = if a.duration.is_some() {
                    jfr::dump_recording(session)?
                } else {
                    println!("\n按 Enter 停止录制并生成报告...");
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input).ok();
                    jfr::dump_recording(session)?
                };
                if let Some(ref dest) = output_path {
                    std::fs::copy(&jfr_path, dest).map_err(|e| {
                        jex_core::error::Error::new(format!("复制 JFR 文件失败: {e}"))
                    })?;
                    println!("📄 JFR 文件已复制到: {}", dest.display());
                }
                let (header, events) = jfr::parse_jfr(&jfr_path)?;
                let summary = jfr::summarize(&events, &header);
                jfr::display_summary(&summary);
                Ok(())
            }
            JavaCommand::Analyze(a) => {
                let path = std::path::PathBuf::from(&a.file);
                let (header, events) = jfr::parse_jfr(&path)?;
                let summary = jfr::summarize(&events, &header);
                jfr::display_summary(&summary);
                Ok(())
            }
            JavaCommand::Top(a) => diag::top_tui(a.pid),
        },
        Commands::Repl(a) => {
            use jex_core::repl;
            repl::start_repl(a.class_only)
        }
        Commands::Completions(a) => {
            use clap_complete;
            let mut cmd = <Cli as clap::CommandFactory>::command();
            let shell = match a.shell {
                Shell::Bash => clap_complete::Shell::Bash,
                Shell::Zsh => clap_complete::Shell::Zsh,
                Shell::Fish => clap_complete::Shell::Fish,
                Shell::Powershell => clap_complete::Shell::PowerShell,
            };
            let bin_name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
            Ok(())
        }
        Commands::Self_(c) => match c {
            SelfCommand::Update(a) => {
                use jex_core::update;
                let current = update::current_version();
                println!("当前版本: {}", current);
                println!("正在检查最新版本...");
                let info = update::check_latest()?;
                println!("最新版本: {}", info.latest_version);
                if !update::needs_update(current, &info.latest_version) {
                    println!("✅ 已是最新版本");
                    return Ok(());
                }
                if a.check {
                    println!("💡 有新版本可用: {}", info.latest_version);
                    return Ok(());
                }
                println!("📥 正在下载...");
                let current_path = update::current_binary_path()?;
                let tmp_path = current_path.with_extension("tmp");
                update::download_binary(&info.download_url, &tmp_path)?;
                println!("🔄 正在替换...");
                update::atomic_replace(&current_path, &tmp_path)?;
                println!("✅ 更新完成！新版本: {}", info.latest_version);
                Ok(())
            }
        },
        Commands::Create(a) => template::create_project(&a.name, &a.template, Some(&a.package)),
        Commands::Outdated => outdated::check_outdated().map(|deps| {
            if deps.is_empty() {
                println!("✅ 所有依赖已是最新版本");
            } else {
                println!("📦 找到 {} 个可更新依赖:\n", deps.len());
                for dep in &deps {
                    println!("  {}:{}: {} → {}", dep.group, dep.artifact, dep.current, dep.latest);
                }
            }
        }),
        Commands::Upgrade(a) => match &a.coord {
            Some(coord) => outdated::upgrade_dep(coord),
            None => outdated::upgrade_all(),
        },
        Commands::Audit(a) => {
            let vulns = audit::check_vulnerabilities()?;
            let report = audit::generate_report(vulns);

            if a.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                if report.total_vulnerabilities == 0 {
                    println!("✅ 未发现安全漏洞");
                } else {
                    println!("🔒 发现 {} 个安全漏洞:\n", report.total_vulnerabilities);
                    println!("  CRITICAL: {}", report.critical);
                    println!("  HIGH:     {}", report.high);
                    println!("  MEDIUM:   {}", report.medium);
                    println!("  LOW:      {}", report.low);
                    println!();

                    for vuln in &report.vulnerabilities {
                        println!("  {} [{}] {}:{}", vuln.severity.display(), vuln.cve_id, vuln.group, vuln.artifact);
                        println!("    {}", vuln.description);
                        if let Some(suggestion) = audit::get_fix_suggestion(vuln) {
                            println!("    修复建议: {}", suggestion);
                        }
                        println!();
                    }
                }
            }
            Ok(())
        },
        Commands::License(a) => {
            let licenses = license::check_licenses()?;
            let report = license::analyze_compatibility(&licenses);

            if a.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else if a.check {
                if report.compatible {
                    println!("✅ 许可证兼容性检查通过");
                } else {
                    println!("❌ 许可证兼容性检查失败");
                    for conflict in &report.conflicts {
                        println!("  冲突: {} vs {} - {}", conflict.license1, conflict.license2, conflict.reason);
                    }
                }
                for warning in &report.warnings {
                    println!("  ⚠️  {}", warning);
                }
            } else {
                println!("📋 许可证分布:\n");
                println!("  总计: {} 个依赖", report.summary.total_dependencies);
                println!("  宽松: {}", report.summary.permissive);
                println!("  弱 copyleft: {}", report.summary.weak_copyleft);
                println!("  强 copyleft: {}", report.summary.strong_copyleft);
                println!("  未知: {}", report.summary.unknown);
                println!();

                for (license, count) in &report.summary.license_counts {
                    println!("  {}: {} 个", license, count);
                }

                if !report.warnings.is_empty() {
                    println!("\n⚠️  警告:");
                    for warning in &report.warnings {
                        println!("  {}", warning);
                    }
                }
            }
            Ok(())
        },
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
