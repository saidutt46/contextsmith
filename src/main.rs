use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;

use contextsmith::cli::{Cli, ColorMode, Command};
use contextsmith::commands;
use contextsmith::commands::collect::CollectCommandOptions;
use contextsmith::commands::diff::DiffCommandOptions;
use contextsmith::commands::explain::ExplainCommandOptions;
use contextsmith::commands::init::{InitOptions, InitResult};
use contextsmith::commands::pack::PackCommandOptions;
use contextsmith::commands::stats::StatsCommandOptions;
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
        Command::Diff {
            rev_range,
            staged,
            untracked,
            since,
            hunks_only,
            context,
            include_related,
            format,
            out,
            stdout,
            budget,
        } => {
            let root = resolve_root(cli.root)?;
            commands::diff::run(DiffCommandOptions {
                root,
                rev_range,
                staged,
                untracked,
                since,
                hunks_only,
                context_lines: context,
                include_related,
                format,
                out,
                stdout,
                quiet: cli.quiet,
                budget,
                model: None,
                config_path: cli.config,
            })
        }
        Command::Collect {
            query,
            scope,
            files,
            grep,
            symbol,
            exclude,
            lang,
            path,
            diff,
            span,
            max_snippets,
            include_defs,
            include_refs,
            include_imports,
            tests,
            rank,
            max_files,
            format,
            out,
            stdout,
            budget,
            ..
        } => {
            let root = resolve_root(cli.root)?;
            // Treat positional query as implicit --grep when no explicit mode is set.
            let effective_grep = grep.or(query);
            let mut ignored_flags_used = Vec::new();
            if scope.is_some() {
                ignored_flags_used.push("--scope".to_string());
            }
            if diff.is_some() {
                ignored_flags_used.push("--diff".to_string());
            }
            if span.is_some() {
                ignored_flags_used.push("--span".to_string());
            }
            if max_snippets.is_some() {
                ignored_flags_used.push("--max-snippets".to_string());
            }
            if include_defs {
                ignored_flags_used.push("--include-defs".to_string());
            }
            if include_refs {
                ignored_flags_used.push("--include-refs".to_string());
            }
            if include_imports {
                ignored_flags_used.push("--include-imports".to_string());
            }
            if tests {
                ignored_flags_used.push("--tests".to_string());
            }
            if rank.is_some() {
                ignored_flags_used.push("--rank".to_string());
            }
            commands::collect::run(CollectCommandOptions {
                root,
                files,
                grep: effective_grep,
                symbol,
                exclude,
                lang,
                path,
                context_lines: 3,
                max_files,
                format,
                out,
                stdout,
                quiet: cli.quiet,
                budget,
                model: None,
                config_path: cli.config,
                ignored_flags_used,
            })
        }
        Command::Pack {
            bundle,
            budget,
            chars,
            model,
            reserve,
            strategy,
            must,
            drop,
            format,
            stdout,
            out,
        } => commands::pack::run(PackCommandOptions {
            bundle,
            budget,
            chars,
            model,
            reserve,
            strategy,
            must,
            drop,
            format,
            stdout,
            out,
            quiet: cli.quiet,
            config_path: cli.config,
        }),
        Command::Trim { .. } => commands::not_implemented("trim"),
        Command::Map { .. } => commands::not_implemented("map"),
        Command::Stats {
            bundle,
            top_files,
            by_lang,
            by_type,
            tokens,
        } => {
            let root = resolve_root(cli.root)?;
            commands::stats::run(StatsCommandOptions {
                bundle,
                root,
                top_files,
                by_lang,
                by_type,
                tokens,
                quiet: cli.quiet,
                config_path: cli.config,
            })
        }
        Command::Explain {
            bundle,
            detailed,
            top,
            show_weights,
        } => commands::explain::run(ExplainCommandOptions {
            bundle,
            detailed,
            top,
            show_weights,
            quiet: cli.quiet,
        }),
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
