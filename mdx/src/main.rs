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
    /// Path to markdown file
    #[arg(value_name = "FILE")]
    file: PathBuf,

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
                let config_path = Config::write_default()
                    .context("Failed to initialize config file")?;
                println!("Created default config file at: {}", config_path.display());
                return Ok(());
            }
        }
    }

    // Default behavior: open markdown file
    let view_args = cli.view.ok_or_else(|| {
        anyhow::anyhow!("No file specified. Use 'mdx <FILE>' to view a file or 'mdx init-config' to initialize configuration.")
    })?;

    // Load configuration
    let (mut config, mut warnings) = Config::load().context("Failed to load configuration")?;

    // Override security settings if --insecure flag is set
    if view_args.insecure {
        config.security.safe_mode = false;
        config.security.no_exec = false;
        // Clear security warnings when using --insecure
        warnings.clear();
    }

    // Load document
    let (doc, doc_warnings) = Document::load(&view_args.file)
        .with_context(|| format!("Failed to load document: {}", view_args.file.display()))?;

    // Combine warnings from config and document
    warnings.extend(doc_warnings);

    // Create app with warnings
    let app = App::new(config, doc, warnings);

    // Run TUI
    mdx_tui::run(app).context("TUI application error")?;

    Ok(())
}
