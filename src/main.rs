use clap::Parser;
use hive::app::AppState;
use hive::config::ClusterConfig;
use hive::tui;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
    }
}

#[derive(Parser)]
#[command(name = "hive")]
struct Cli {
    /// Path to cluster.yaml
    #[arg(long, default_value = "cluster.yaml")]
    config: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cfg = ClusterConfig::load(&cli.config)?;
    std::env::set_var("HIVE_CONFIG", cli.config.to_string_lossy().to_string());
    let cfg = Arc::new(Mutex::new(cfg));
    // In-memory password store (never persisted to disk).
    let secrets: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));

    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
    let _guard = TerminalGuard;

    let mut state = AppState::default();
    if let Err(e) = tui::run(&mut state, cfg, secrets) {
        eprintln!("tui error: {e}");
    }

    Ok(())
}
