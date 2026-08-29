mod config;
mod error;

use clap::{Args, Parser, Subcommand};
use error::Result;

#[derive(Parser)]
#[command(
    name = "jx",
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

    /// 脚手架:生成 jx.toml + 标准目录
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
    /// 切换版本(写 .jx-version)
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
    /// GC 概览
    Gc,
    /// 线程概览
    Threads,
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

/// 统一的“规划中”占位输出,标注该命令归属的 Phase。
fn planned(phase: &str, detail: &str) {
    println!("[规划中 · {phase}] {detail}");
    println!("  该能力将在对应 Phase 落地,当前为 Phase 0 骨架。");
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Jdk(c) => match c {
            JdkCommand::Install(a) => Ok(planned("1.1", &format!("jdk install {}", a.version))),
            JdkCommand::Use(a) => Ok(planned("1.1", &format!("jdk use {}", a.version))),
            JdkCommand::List => Ok(planned("1.1", "jdk list")),
            JdkCommand::Which => Ok(planned("1.1", "jdk which")),
            JdkCommand::Doctor => Ok(planned("1.1", "jdk doctor")),
        },
        Commands::Init(a) => config::scaffold(a.name.as_deref()),
        Commands::Add(a) => Ok(planned("1.2", &format!("add {} (exclude {:?})", a.coord, a.exclude))),
        Commands::Remove(a) => Ok(planned("1.2", &format!("remove {}", a.coord))),
        Commands::Update => Ok(planned("1.2", "update")),
        Commands::Search(a) => Ok(planned("1.3", &format!("search {} (limit {})", a.query, a.limit))),
        Commands::Run(a) => Ok(planned("1.4", &format!("run {} {:?}", a.file, a.args))),
        Commands::Build => Ok(planned("1.4", "build")),
        Commands::Tree => Ok(planned("1.2", "tree")),
        Commands::Why(a) => Ok(planned("1.2", &format!("why {}", a.coord))),
        Commands::Conflict => Ok(planned("1.2", "conflict")),
        Commands::Analyze => Ok(planned("2.x", "analyze")),
        Commands::Export => Ok(planned("1.6", "export maven")),
        Commands::Import => Ok(planned("1.6", "import pom")),
        Commands::Java(c) => match c {
            JavaCommand::Gc => Ok(planned("1.5", "java gc")),
            JavaCommand::Threads => Ok(planned("1.5", "java threads")),
            JavaCommand::Heap => Ok(planned("1.5", "java heap")),
            JavaCommand::Flame(a) => Ok(planned("2.3", &format!("java flame {}", a.pid))),
            JavaCommand::Rec(a) => Ok(planned("2.4", &format!("java rec {}", a.pid))),
            JavaCommand::Top => Ok(planned("1.5", "java top")),
        },
    }
}
