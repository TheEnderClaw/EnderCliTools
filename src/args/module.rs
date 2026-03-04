use clap::{Args, Subcommand};

#[derive(Args, Debug, Clone)]
pub struct ModuleArgs {
    #[command(subcommand)]
    pub command: ModuleCommands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ModuleCommands {
    Install(InstallArgs),
    List,
    Remove(RemoveArgs),
    Run(RunArgs),
    Build(BuildArgs),
    Info(InfoArgs),
}

#[derive(Args, Debug, Clone)]
pub struct InstallArgs {
    pub source: Option<String>,

    #[arg(long)]
    pub url: Option<String>,

    #[arg(long)]
    pub file: Option<String>,

    #[arg(long)]
    pub tag: Option<String>,

    #[arg(long)]
    pub allow_insecure_tls: bool,

    #[arg(long)]
    pub allow_http: bool,

    #[arg(long)]
    pub allow_missing_sha512: bool,

    #[arg(long)]
    pub allow_major_compat_mismatch: bool,

    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug, Clone)]
pub struct RemoveArgs {
    pub id: String,
    #[arg(long)]
    pub version: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct RunArgs {
    pub id: String,
    pub binary: String,
    #[arg(last = true)]
    pub args: Vec<String>,
}

#[derive(Args, Debug, Clone)]
pub struct BuildArgs {
    pub path: String,

    #[arg(long)]
    pub out_dir: Option<String>,

    #[arg(long)]
    pub min_ect: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct InfoArgs {
    pub target: String,
}
