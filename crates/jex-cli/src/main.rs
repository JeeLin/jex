use clap::{Args, Parser, Subcommand};
use jex_core::error::Result;
use jex_core::{deps, diag, export, jdk, jfr, profiler, run, search};
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
    Build,

    /// 依赖树
    #[command(alias = "t")]
    Tree,

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
    Import,

    /// JVM 诊断(gc / threads / heap / 火焰图 / 录制)
    #[command(subcommand)]
    Java(JavaCommand),
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

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("错误: {e}");
        std::process::exit(1);
    }
}

/// 提示命令还未实现的辅助函数
fn planned(phase: &str, detail: &str) -> Result<()> {
    println!("[{phase}] {detail} 尚未实现");
    Ok(())
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
        Commands::Run(a) => run::run(&a.file, &a.args),
        Commands::Build => planned("1.4", "build"),
        Commands::Tree => deps::tree(),
        Commands::Why(a) => deps::why(&a.coord),
        Commands::Conflict => deps::conflict(),
        Commands::Analyze => planned("2.x", "analyze"),
        Commands::Export => export::maven(),
        Commands::Import => planned("2.x", "import pom"),
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
                    std::fs::copy(&flame.svg_path, &dest)
                        .map_err(|e| jex_core::error::Error::new(format!("复制 SVG 到 {path} 失败: {e}")))?;
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
                    // 交互式：等待用户 Ctrl+C
                    println!("\n按 Enter 停止录制并生成报告...");
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input).ok();
                    jfr::dump_recording(session)?
                };
                // 如果指定了输出路径，复制
                if let Some(ref dest) = output_path {
                    std::fs::copy(&jfr_path, dest)
                        .map_err(|e| jex_core::error::Error::new(format!("复制 JFR 文件失败: {e}")))?;
                    println!("📄 JFR 文件已复制到: {}", dest.display());
                }
                // 解析并展示
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
    }
}
