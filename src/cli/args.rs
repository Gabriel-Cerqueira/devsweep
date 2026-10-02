use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "devsweep",
    author = "Gabriel Cerqueira",
    version,
    about = "Fast, safe disk cleaner and visualizer for developer build caches and artifacts",
    long_about = "DevSweep identifies disposable build artifacts across multiple programming ecosystems (Rust, Node, Python, Java, .NET, C/C++, Flutter, etc.) and safely reclaims disk space."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to scan when launching default TUI mode
    #[arg(value_name = "PATH", global = false)]
    pub default_path: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Launch the interactive terminal user interface (default)
    Tui(PathArgs),

    /// Scan directory and output formatted report to terminal
    Scan(ScanArgs),

    /// Clean build artifacts directly from command line
    Clean(CleanArgs),
}

#[derive(Args, Debug)]
pub struct PathArgs {
    /// Target directory to scan (defaults to current directory)
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: PathBuf,
}

#[derive(Args, Debug)]
pub struct ScanArgs {
    /// Target directory to scan
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: PathBuf,

    /// Filter by ecosystem
    #[arg(short = 't', long = "type", value_enum)]
    pub ecosystem: Option<CliEcosystem>,

    /// Minimum inactive days
    #[arg(long = "older-than", value_name = "DAYS")]
    pub older_than: Option<i64>,

    /// Output format
    #[arg(long = "json")]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct CleanArgs {
    /// Target directory to scan and clean
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: PathBuf,

    /// Clean all discovered projects without prompting
    #[arg(short = 'a', long = "all")]
    pub all: bool,

    /// Filter by ecosystem to clean
    #[arg(short = 't', long = "type", value_enum)]
    pub ecosystem: Option<CliEcosystem>,

    /// Clean only projects inactive for more than N days
    #[arg(long = "older-than", value_name = "DAYS")]
    pub older_than: Option<i64>,

    /// Use permanent deletion instead of Recycle Bin
    #[arg(long = "permanent")]
    pub permanent: bool,

    /// Dry run: simulate deletion without deleting files
    #[arg(long = "dry-run")]
    pub dry_run: bool,

    /// Bypass confirmation prompt
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliEcosystem {
    Rust,
    Node,
    Python,
    Java,
    Dotnet,
    Cpp,
    Flutter,
    Elixir,
    Php,
}

impl CliEcosystem {
    pub fn to_scanner_ecosystem(self) -> crate::scanner::model::Ecosystem {
        match self {
            CliEcosystem::Rust => crate::scanner::model::Ecosystem::Rust,
            CliEcosystem::Node => crate::scanner::model::Ecosystem::Node,
            CliEcosystem::Python => crate::scanner::model::Ecosystem::Python,
            CliEcosystem::Java => crate::scanner::model::Ecosystem::GradleJava,
            CliEcosystem::Dotnet => crate::scanner::model::Ecosystem::DotNet,
            CliEcosystem::Cpp => crate::scanner::model::Ecosystem::CppCmake,
            CliEcosystem::Flutter => crate::scanner::model::Ecosystem::FlutterDart,
            CliEcosystem::Elixir => crate::scanner::model::Ecosystem::Elixir,
            CliEcosystem::Php => crate::scanner::model::Ecosystem::PhpComposer,
        }
    }
}
