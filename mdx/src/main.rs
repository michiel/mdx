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
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Load configuration
    let config = Config::load().context("Failed to load configuration")?;

    // Load document
    let doc = Document::load(&args.file)
        .with_context(|| format!("Failed to load document: {}", args.file.display()))?;

    // Create app
    let app = App::new(config, doc);

    // Run TUI
    mdx_tui::run(app).context("TUI application error")?;

    Ok(())
}
