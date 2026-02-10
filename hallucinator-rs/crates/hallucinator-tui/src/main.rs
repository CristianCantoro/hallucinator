use std::io;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use ratatui::crossterm::event;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

mod action;
mod app;
mod backend;
mod tui_event;
mod input;
mod model;
mod theme;
mod view;

use app::App;

/// Hallucinator TUI — batch academic reference validation with a terminal interface.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// PDF files to check
    pdf_paths: Vec<PathBuf>,

    /// OpenAlex API key
    #[arg(long)]
    openalex_key: Option<String>,

    /// Semantic Scholar API key
    #[arg(long)]
    s2_api_key: Option<String>,

    /// Path to offline DBLP database
    #[arg(long)]
    dblp_offline: Option<PathBuf>,

    /// Comma-separated list of databases to disable
    #[arg(long, value_delimiter = ',')]
    disable_dbs: Vec<String>,

    /// Flag author mismatches from OpenAlex (default: skipped)
    #[arg(long)]
    check_openalex_authors: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let args = Args::parse();

    // Validate any PDF paths provided on the command line
    for path in &args.pdf_paths {
        if !path.exists() {
            anyhow::bail!("PDF file not found: {}", path.display());
        }
    }

    // Resolve config from CLI flags > env vars > defaults
    let openalex_key = args
        .openalex_key
        .or_else(|| std::env::var("OPENALEX_KEY").ok());
    let s2_api_key = args
        .s2_api_key
        .or_else(|| std::env::var("S2_API_KEY").ok());
    let dblp_offline_path = args
        .dblp_offline
        .or_else(|| std::env::var("DBLP_OFFLINE_PATH").ok().map(PathBuf::from));

    let db_timeout_secs: u64 = std::env::var("DB_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let db_timeout_short_secs: u64 = std::env::var("DB_TIMEOUT_SHORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);

    // Open DBLP database if configured
    let dblp_offline_db = if let Some(ref path) = dblp_offline_path {
        Some(backend::open_dblp_db(path)?)
    } else {
        None
    };

    let config = hallucinator_core::Config {
        openalex_key,
        s2_api_key,
        dblp_offline_path: dblp_offline_path.clone(),
        dblp_offline_db,
        max_concurrent_refs: 4,
        db_timeout_secs,
        db_timeout_short_secs,
        disabled_dbs: args.disable_dbs,
        check_openalex_authors: args.check_openalex_authors,
    };

    // Build filenames for display
    let filenames: Vec<String> = args
        .pdf_paths
        .iter()
        .map(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.display().to_string())
        })
        .collect();

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    // Install panic hook that restores terminal before printing panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Drain any stray input events (e.g. Enter keypress from launching the command)
    while event::poll(Duration::from_millis(50)).unwrap_or(false) {
        let _ = event::read();
    }

    let mut app = App::new(filenames);

    // Launch backend processing (only if PDFs were provided)
    let (tx, mut rx) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();

    if !args.pdf_paths.is_empty() {
        let cancel_clone = cancel.clone();
        let pdfs = args.pdf_paths.clone();
        tokio::spawn(async move {
            backend::run_batch(pdfs, config, tx, cancel_clone).await;
        });
    }

    // Also handle Ctrl+C at the OS level for clean shutdown
    let cancel_for_signal = cancel.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            cancel_for_signal.cancel();
        }
    });

    // Main event loop
    let tick_rate = Duration::from_millis(100);

    loop {
        // Draw
        terminal.draw(|f| app.view(f))?;

        // Poll for events with timeout for tick
        let timeout = tick_rate;

        tokio::select! {
            // Backend events (non-blocking drain)
            maybe_event = rx.recv() => {
                match maybe_event {
                    Some(backend_event) => {
                        app.handle_backend_event(backend_event);
                        // Drain any additional queued backend events
                        while let Ok(evt) = rx.try_recv() {
                            app.handle_backend_event(evt);
                        }
                    }
                    None => {
                        // Backend channel closed — processing done
                    }
                }
            }
            // Terminal input events
            _ = async {
                if event::poll(timeout).unwrap_or(false) {
                    if let Ok(evt) = event::read() {
                        let action = input::map_event(&evt);
                        if app.update(action) {
                            // Quit requested
                        }
                    }
                }
            } => {}
        }

        // Process tick
        app.update(action::Action::Tick);

        if app.should_quit {
            cancel.cancel();
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}
