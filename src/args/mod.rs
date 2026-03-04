use crate::args::config::ConfigArgs;
use crate::args::dcps::DcpsArgs;
use crate::args::dps::DpsArgs;
use crate::args::module::ModuleArgs;
use clap::{Parser, Subcommand};

pub mod config;
pub mod dcps;
pub mod dps;
pub mod module;

#[derive(Parser, Debug)]
#[command(
    name = "EnderCliTools",
    author = "Endkind Ender",
    version,
    about = "EnderCliTools is a lightweight collection of CLI utilities that make working in the terminal faster and more convenient.",
    allow_external_subcommands = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    Dps(DpsArgs),
    Dcps(DcpsArgs),
    Config(ConfigArgs),
    Module(ModuleArgs),
    #[command(external_subcommand)]
    External(Vec<String>),
}
