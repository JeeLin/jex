mod config;
mod deps;
mod error;
mod jdk;
mod search;
mod run;
mod export;
mod diag;
mod util;

use clap::{Args, Parser, Subcommand};
use error::Result;

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
    Heap,
    /// 火焰图(async-profiler)
    Flame(FlameArgs),
    /// 录制 Flight Recorder
    Rec(RecArgs),
    /// 实时面板
    Top,
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
    pid: u32,
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
    pid: u32,
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
        },
        Commands::Run(a) => run::run(&a.file, &a.args),
Commands::Build => planned("1.4", "build"),
        Commands::Tree => deps::tree(),
        Commands::Why(a) => deps::why(&a.coord),
        Commands::Conflict => deps::conflict(),
Commands::Analyze => planned("2.x", "analyze"),
Commands::Export => export::maven(),
Commands::Import => planned("2.x", "import pom"),
Commands::Java(c) => match c {
            JavaCommand::Gc(a) => diag::gc(a.pid),
            JavaCommand::Threads(a) => diag::threads(a.pid),
JavaCommand::Heap => planned("2.x", "java heap"),
JavaCommand::Flame(a) => planned("2.3", &format!("java flame {}", a.pid)),
JavaCommand::Rec(a) => planned("2.4", &format!("java rec {}", a.pid)),
JavaCommand::Top => planned("2.x", "java top"),
        },
    }
}
