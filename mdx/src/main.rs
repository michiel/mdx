//! MDX - A fast TUI Markdown viewer with Vim-style navigation

use anyhow::Result;
use clap::Parser;
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
    let _args = Args::parse();

    // TODO: Implementation in Stage 3
    println!("MDX - Implementation in progress");
    println!("Stage 0: Project scaffold complete!");

    Ok(())
}
