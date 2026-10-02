use anyhow::Result;
use clap::Parser;
use devsweep::cli::{self, Cli, Commands};
use devsweep::ui::{run_tui, App};
use std::path::PathBuf;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Tui(args)) => {
            let app = App::new(args.path);
            run_tui(app)?;
        }
        Some(Commands::Scan(args)) => {
            cli::run_scan_command(args)?;
        }
        Some(Commands::Clean(args)) => {
            cli::run_clean_command(args)?;
        }
        None => {
            let target_path = cli.default_path.unwrap_or_else(|| PathBuf::from("."));
            let app = App::new(target_path);
            run_tui(app)?;
        }
    }

    Ok(())
}
