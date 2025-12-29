//! MDX - A fast TUI Markdown viewer with Vim-style navigation

use anyhow::{Context, Result};
use clap::Parser;
use mdx_core::{Config, Document};
use mdx_tui::App;
use std::path::PathBuf;

/// A fast TUI Markdown viewer
#[derive(Parser, Debug)]
#[command(name = "mdx")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to markdown file
    #[arg(value_name = "FILE")]
    file: PathBuf,

    /// Disable security restrictions (use for trusted content only)
    #[arg(long)]
    insecure: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Load configuration
    let (mut config, mut warnings) = Config::load().context("Failed to load configuration")?;

    // Override security settings if --insecure flag is set
    if args.insecure {
        config.security.safe_mode = false;
        config.security.no_exec = false;
        // Clear security warnings when using --insecure
        warnings.clear();
    }

    // Load document
    let (doc, doc_warnings) = Document::load(&args.file)
        .with_context(|| format!("Failed to load document: {}", args.file.display()))?;

    // Combine warnings from config and document
    warnings.extend(doc_warnings);

    // Create app with warnings
    let app = App::new(config, doc, warnings);

    // Run TUI
    mdx_tui::run(app).context("TUI application error")?;

    Ok(())
}
