use std::path::PathBuf;

use clap::Parser;

/// Hallucinated Reference Detector - Detect fabricated references in academic PDFs
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the PDF file to check
    pdf_path: PathBuf,

    /// Disable colored output
    #[arg(long)]
    no_color: bool,

    /// OpenAlex API key
    #[arg(long)]
    openalex_key: Option<String>,

    /// Semantic Scholar API key
    #[arg(long)]
    s2_api_key: Option<String>,

    /// Path to output log file
    #[arg(long)]
    output: Option<PathBuf>,

    /// Path to offline DBLP database
    #[arg(long)]
    dblp_offline: Option<PathBuf>,

    /// Download and build offline DBLP database at the given path
    #[arg(long)]
    update_dblp: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _args = Args::parse();
    todo!("Phase 3: implement CLI")
}
