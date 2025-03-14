use anyhow::Result;
use civ7_mod_manager::{ModDirEntry, ModManager, ModProvision, ModSpec};
use clap::{Parser, Subcommand};
use std::{path::PathBuf, str::FromStr as _, sync::Arc};
use tokio::task::LocalSet;

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[arg(long, global = true, default_value_os_t = civ7_mod_manager::default_root_dir())]
    root_dir: PathBuf,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    List,
    Cleanup,
    #[command(aliases = &["add"])]
    Install {
        #[arg(group = "mod_spec", required = true)]
        specs: Option<Vec<ModSpec>>,
        #[arg(long, group = "mod_spec", requires = "dirname")]
        spec: Option<ModSpec>,
        #[arg(long, requires = "spec")]
        dirname: Option<String>,
    },
    #[command(aliases = &["remove"])]
    Uninstall {
        #[arg(required = true)]
        dirname_or_specs: Vec<String>,
    },
    Update {
        dirname_or_specs: Option<Vec<String>>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    let manager = Arc::new(ModManager::load(args.root_dir)?);

    match args.command {
        Commands::List => {
            list(&manager).await?;
        }
        Commands::Cleanup => {
            cleanup(&manager).await?;
        }
        Commands::Install {
            specs: Some(specs), ..
        } => {
            let jobs = LocalSet::new();
            for spec in specs {
                let manager = manager.clone();
                jobs.spawn_local(async move {
                    install_or_update(&manager, None, spec.clone())
                        .await
                        .inspect_err(|err| eprintln!("{err}"))
                });
            }
            jobs.await;
        }
        Commands::Install {
            spec: Some(spec),
            dirname,
            ..
        } => {
            install_or_update(&manager, dirname, spec).await?;
        }
        Commands::Uninstall { dirname_or_specs } => {
            let jobs = LocalSet::new();
            for dirname_or_spec in dirname_or_specs {
                let dirname = resolve_dirname_from_string(&manager, &dirname_or_spec).await?;
                let manager = manager.clone();
                jobs.spawn_local(async move {
                    uninstall(&manager, &dirname)
                        .await
                        .inspect_err(|err| eprintln!("{err}"))
                });
            }
            jobs.await;
        }
        Commands::Update {
            dirname_or_specs: Some(dirname_or_specs),
        } => {
            let jobs = LocalSet::new();
            for dirname_or_spec in dirname_or_specs {
                let dirname = resolve_dirname_from_string(&manager, &dirname_or_spec).await?;
                if let Some(spec) = manager.manifest().read().await.get(&dirname).cloned() {
                    let manager = manager.clone();
                    jobs.spawn_local(async move {
                        install_or_update(&manager, Some(dirname), spec)
                            .await
                            .inspect_err(|err| eprintln!("{err}"))
                    });
                } else {
                    eprintln!("{dirname} is not managed");
                }
            }
            jobs.await;
        }
        Commands::Update {
            dirname_or_specs: None,
        } => {
            let jobs = LocalSet::new();
            for (dirname, spec) in manager.manifest().read().await.clone() {
                let manager = manager.clone();
                jobs.spawn_local(async move {
                    install_or_update(&manager, Some(dirname), spec)
                        .await
                        .inspect_err(|err| eprintln!("{err}"))
                });
            }
            jobs.await;
        }
        _ => unreachable!(),
    }

    Ok(())
}

async fn resolve_dirname_from_string(
    manager: &ModManager,
    dirname_or_spec: &str,
) -> Result<String> {
    if manager.mod_path(dirname_or_spec).is_dir() {
        let dirname = dirname_or_spec;
        return Ok(dirname.to_string());
    }

    if let Ok(spec) = ModSpec::from_str(dirname_or_spec) {
        let dirname = spec.resolve_dirname()?;
        return Ok(dirname);
    }

    anyhow::bail!("Could not resolve dirname from {dirname_or_spec}");
}

async fn list(manager: &ModManager) -> Result<()> {
    use chrono::{DateTime, Local};
    use prettytable::Table;

    let dirs = manager.list_dirs().await?;

    let mut table = Table::new();
    table.set_format(*prettytable::format::consts::FORMAT_CLEAN);
    table.set_titles(prettytable::row![
        b -> "dirname",
        b -> "source",
        b -> "identifier",
        br -> "size",
        br -> "last updated",
    ]);

    for dir in dirs {
        match dir {
            ModDirEntry::Managed(entry, dirname, spec) => {
                let content_size =
                    dir_size::get_size_in_human_bytes(&entry.path()).unwrap_or_default();

                let last_updated = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .map(|system_time| {
                        DateTime::<Local>::from(system_time)
                            .format("%F %T %Z")
                            .to_string()
                    })
                    .unwrap_or_default();

                table.add_row(prettytable::row![
                    i -> dirname,
                    spec.source,
                    spec.identifier,
                    r -> content_size,
                    r -> last_updated,
                ]);
            }
            ModDirEntry::Unmanaged(_, dirname) => {
                table.add_row(prettytable::row![FDi -> dirname, FD -> "not managed"]);
            }
        }
    }

    table.printstd();

    Ok(())
}

async fn cleanup(manager: &ModManager) -> Result<()> {
    let removed = manager.cleanup().await?;
    for (dirname, spec) in removed {
        eprintln!("Removed {spec} from {dirname}");
    }
    Ok(())
}

async fn install_or_update(
    manager: &ModManager,
    dirname: Option<String>,
    spec: ModSpec,
) -> Result<()> {
    let dirname = match dirname {
        Some(dirname) => dirname,
        None => spec.resolve_dirname()?,
    };

    match manager
        .install_or_update(dirname.clone(), spec.clone())
        .await?
    {
        ModProvision::Installed(_) => {
            eprintln!("Installed {spec} to {dirname}");
        }
        ModProvision::Updated(_) => {
            eprintln!("Updated {spec} in {dirname}");
        }
        ModProvision::Unchanged => {
            eprintln!("{spec} is up-to-date");
        }
    }

    Ok(())
}

async fn uninstall(manager: &ModManager, dirname: &str) -> Result<()> {
    manager.uninstall(dirname).await?;
    eprintln!("Uninstalled {dirname}");
    Ok(())
}
