#![allow(clippy::collapsible_else_if)]

mod args;
mod cmd;
mod config;
mod utils;
mod module_system;

use anyhow::Result;
use args::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        args::Commands::Dps(opts) => {
            cmd::dps::run(opts)?;
        }
        args::Commands::Dcps(opts) => {
            cmd::dcps::run(opts)?;
        }
        args::Commands::Config(opts) => {
            cmd::config::run(opts)?;
        }
        args::Commands::Module(opts) => {
            cmd::module::run(opts)?;
        }
        args::Commands::External(args) => {
            cmd::module::run_external(args)?;
        }
    }

    Ok(())
}
