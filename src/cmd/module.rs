use crate::args::module::{BuildArgs, InfoArgs, InstallArgs, ModuleArgs, ModuleCommands, RemoveArgs, RunArgs};
use crate::module_system::{
    build_package, infer_source, install_module, load_registry, remove_module, run_module, save_registry,
    show_info,
};
use anyhow::{Result, bail};

pub fn run(args: ModuleArgs) -> Result<()> {
    match args.command {
        ModuleCommands::Install(args) => install(args),
        ModuleCommands::List => list(),
        ModuleCommands::Remove(args) => remove(args),
        ModuleCommands::Run(args) => run_cmd(args),
        ModuleCommands::Build(args) => build(args),
        ModuleCommands::Info(args) => info(args),
    }
}

pub fn run_external(args: Vec<String>) -> Result<()> {
    if args.is_empty() {
        bail!("unknown command");
    }
    let id_or_alias = &args[0];
    run_module(id_or_alias, None, &args[1..])
}

fn install(args: InstallArgs) -> Result<()> {
    let source = infer_source(
        args.source.clone(),
        args.url.clone(),
        args.file.clone(),
        args.tag.clone(),
    )?;
    let result = install_module(source, &args)?;
    println!("Installed {} {}", result.id, result.version);
    Ok(())
}

fn list() -> Result<()> {
    let registry = load_registry()?;
    if registry.modules.is_empty() {
        println!("No modules installed.");
        return Ok(());
    }

    for m in registry.modules {
        println!("{} {}{}", m.id, m.version, if m.unsafe_install { " (UNSAFE)" } else { "" });
    }
    Ok(())
}

fn remove(args: RemoveArgs) -> Result<()> {
    let mut registry = load_registry()?;
    let removed = remove_module(&mut registry, &args.id, args.version.as_deref())?;
    save_registry(&registry)?;
    println!("Removed {} {}", removed.id, removed.version);
    Ok(())
}

fn run_cmd(args: RunArgs) -> Result<()> {
    run_module(&args.id, Some(&args.binary), &args.args)
}

fn build(args: BuildArgs) -> Result<()> {
    let result = build_package(&args.path, args.out_dir.as_deref(), args.min_ect.as_deref())?;
    println!("{}", result.package_path.display());
    println!("{}", result.sha512_path.display());
    Ok(())
}

fn info(args: InfoArgs) -> Result<()> {
    show_info(&args.target)
}
