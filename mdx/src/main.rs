//! MDX - A fast TUI Markdown viewer with Vim-style navigation

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use mdx_core::{Config, Document};
use mdx_tui::App;
use std::path::PathBuf;

/// A fast TUI Markdown viewer
#[derive(Parser, Debug)]
#[command(name = "mdx")]
#[command(author, version, about, long_about = None)]
#[command(args_conflicts_with_subcommands = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[command(flatten)]
    view: Option<ViewArgs>,
}

#[derive(Parser, Debug)]
struct ViewArgs {
    /// Path to markdown file (reads from stdin if not provided)
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,

    /// Disable security restrictions (use for trusted content only)
    #[arg(long)]
    insecure: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize default configuration file
    InitConfig,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle subcommands
    if let Some(command) = cli.command {
        match command {
            Commands::InitConfig => {
                let config_path =
                    Config::write_default().context("Failed to initialize config file")?;
                println!("Created default config file at: {}", config_path.display());
                return Ok(());
            }
        }
    }

    // Default behavior: open markdown file or read from stdin
    let view_args = cli.view.unwrap_or(ViewArgs {
        file: None,
        insecure: false,
    });

    // Load configuration
    let (mut config, mut warnings) = Config::load().context("Failed to load configuration")?;

    // Override security settings if --insecure flag is set
    if view_args.insecure {
        config.security.safe_mode = false;
        config.security.no_exec = false;
        // Clear security warnings when using --insecure
        warnings.clear();
    }

    // Load document from file or stdin
    let (doc, doc_warnings) = if let Some(file_path) = view_args.file {
        Document::load(&file_path)
            .with_context(|| format!("Failed to load document: {}", file_path.display()))?
    } else {
        Document::from_stdin().context("Failed to read document from stdin")?
    };

    // Combine warnings from config and document
    warnings.extend(doc_warnings);

    // Create app with warnings
    let app = App::new(config, doc, warnings);

    // Run TUI
    mdx_tui::run(app).context("TUI application error")?;

    Ok(())
}
