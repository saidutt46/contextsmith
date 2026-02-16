use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;

use contextsmith::cli::{Cli, ColorMode, Command};
use contextsmith::commands;
use contextsmith::commands::init::{InitOptions, InitResult};
use contextsmith::error::ContextSmithError;

fn main() {
    let cli = Cli::parse();

    // Configure color output
    match cli.color {
        ColorMode::Always => colored::control::set_override(true),
        ColorMode::Never => colored::control::set_override(false),
        ColorMode::Auto => {}
    }

    // Init tracing
    let filter = match cli.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter)),
        )
        .with_target(false)
        .init();

    if let Err(err) = run(cli) {
        eprintln!("{} {err}", "error:".red().bold());
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), ContextSmithError> {
    match cli.command {
        Command::Init {
            root,
            config,
            force,
            no_cache,
        } => {
            let root = resolve_root(root.or(cli.root))?;
            let result = commands::init::run(InitOptions {
                root: root.clone(),
                config_path: config,
                force,
                no_cache: no_cache || cli.no_cache,
            })?;
            print_init_result(&result);
            Ok(())
        }
        Command::Diff { .. } => commands::not_implemented("diff"),
        Command::Collect { .. } => commands::not_implemented("collect"),
        Command::Pack { .. } => commands::not_implemented("pack"),
        Command::Trim { .. } => commands::not_implemented("trim"),
        Command::Map { .. } => commands::not_implemented("map"),
        Command::Stats { .. } => commands::not_implemented("stats"),
        Command::Explain { .. } => commands::not_implemented("explain"),
    }
}

fn resolve_root(root: Option<PathBuf>) -> Result<PathBuf, ContextSmithError> {
    match root {
        Some(p) => Ok(p),
        None => std::env::current_dir()
            .map_err(|e| ContextSmithError::io("getting current directory", e)),
    }
}

fn print_init_result(result: &InitResult) {
    println!(
        "{} Created config at {}",
        "ok".green().bold(),
        result.config_path.display()
    );
    if let Some(ref cache_dir) = result.cache_dir {
        println!(
            "{} Created cache at {}",
            "ok".green().bold(),
            cache_dir.display()
        );
    }
    println!();
    println!("Next steps:");
    println!(
        "  1. Edit {} to customize settings",
        "contextsmith.toml".bold()
    );
    println!(
        "  2. Run {} to see your project map",
        "contextsmith map".bold()
    );
    println!(
        "  3. Run {} to collect context",
        "contextsmith collect".bold()
    );
}
