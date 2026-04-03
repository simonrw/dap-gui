use std::{io, path::PathBuf, time::Duration};

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use eyre::Context;
use ratatui::{Terminal, backend::CrosstermBackend};

mod app;
mod event;
mod ui;

use app::App;
use event::EventHandler;

#[derive(Parser)]
struct Args {
    /// Path to a launch.json or VS Code workspace file.
    config_path: PathBuf,

    /// Select a specific configuration by name.
    #[clap(short, long)]
    name: Option<String>,

    /// Initial breakpoints in `file:line` format.
    #[clap(short, long)]
    breakpoints: Vec<String>,
}

fn main() -> eyre::Result<()> {
    let _ = color_eyre::install();

    let args = Args::parse();

    // Tracing must go to a file — stdout is the terminal.
    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("dapgui");
    std::fs::create_dir_all(&log_dir).ok();
    let log_file = std::fs::File::create(log_dir.join("tui.log")).wrap_err("creating log file")?;
    tracing_subscriber::fmt()
        .with_writer(log_file)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Load configurations
    let configs = launch_configuration::load_all_from_path(&args.config_path)
        .wrap_err("loading launch configurations")?;
    if configs.is_empty() {
        eyre::bail!("no configurations found in {}", args.config_path.display());
    }
    let config_names: Vec<String> = configs.iter().map(|c| c.name().to_string()).collect();

    let selected_config_index = if let Some(ref name) = args.name {
        config_names
            .iter()
            .position(|n| n == name)
            .ok_or_else(|| eyre::eyre!("no configuration named '{name}' found"))?
    } else {
        0
    };

    let debug_root_dir = std::env::current_dir()
        .and_then(|p| std::fs::canonicalize(&p))
        .wrap_err("resolving current directory")?;

    let mut app = App::new(
        configs,
        config_names,
        selected_config_index,
        args.config_path,
        debug_root_dir,
    );

    // Install a panic hook that restores the terminal before printing.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        default_hook(info);
    }));

    // Set up terminal
    enable_raw_mode().wrap_err("enabling raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .wrap_err("entering alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).wrap_err("creating terminal")?;

    // Event loop
    let (events, _event_tx) = EventHandler::new(Duration::from_millis(250));

    let result = run_loop(&mut terminal, &mut app, &events);

    // Restore terminal
    restore_terminal()?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    events: &EventHandler,
) -> eyre::Result<()> {
    loop {
        terminal.draw(|frame| ui::render(app, frame))?;

        let event = events.recv()?;
        app.handle_event(event);

        if app.should_quit {
            return Ok(());
        }
    }
}

fn restore_terminal() -> eyre::Result<()> {
    disable_raw_mode().wrap_err("disabling raw mode")?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)
        .wrap_err("leaving alternate screen")?;
    Ok(())
}
