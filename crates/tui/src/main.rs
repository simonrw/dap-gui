use std::{io, path::PathBuf, time::Duration};

use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use eyre::Context;
use ratatui::{Terminal, backend::CrosstermBackend};
use ui_core::bootstrap::{self, Args};

mod app;
mod async_bridge;
mod event;
mod input;
mod line_editor;
mod session;
mod syntax;
mod ui;

use app::App;
use event::EventHandler;

fn main() -> eyre::Result<()> {
    let _ = color_eyre::install();

    let args = Args::parse();

    // Tracing must go to a file -- stdout is the terminal.
    let log_dir = args.log_path.clone().unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("dapgui")
    });
    std::fs::create_dir_all(&log_dir).ok();

    let file_appender = tracing_appender::rolling::daily(&log_dir, "tui.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| project_log_filter(&args.log_level));

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_env_filter(env_filter)
        .init();

    let boot = bootstrap::bootstrap(&args)?;

    // Wakeup channel: the async bridge sends notifications here to unblock
    // the event handler when debugger events arrive.
    let (wakeup_tx, wakeup_rx) = crossbeam_channel::unbounded();

    let mut app = App::new(
        boot.configs,
        boot.config_names,
        boot.selected_config_index,
        args.config_path,
        boot.debug_root_dir,
        boot.state_manager,
        wakeup_tx,
        boot.initial_breakpoints,
        boot.keybindings,
    );

    // Install a panic hook that restores the terminal before printing.
    // This ensures the terminal is usable even after a panic.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Best-effort terminal restore: ignore errors since we're panicking anyway.
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            crossterm::cursor::Show
        );
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
    let (events, _event_tx) = EventHandler::new(Duration::from_millis(250), wakeup_rx);

    let result = run_loop(&mut terminal, &mut app, &events);

    // Clean shutdown: drop session before restoring terminal
    if app.session.is_some() {
        app.shutdown_session();
    }

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

/// Build an `EnvFilter` that sets the given level for all project crates
/// while keeping external crates at `warn`.
fn project_log_filter(level: &str) -> tracing_subscriber::EnvFilter {
    const PROJECT_CRATES: &[&str] = &[
        "dap_tui",
        "async_transport",
        "config",
        "dap_types",
        "debugger",
        "fuzzy",
        "launch_configuration",
        "server",
        "state",
        "ui_core",
    ];

    let directives: String = PROJECT_CRATES
        .iter()
        .map(|c| format!("{c}={level}"))
        .collect::<Vec<_>>()
        .join(",");

    tracing_subscriber::EnvFilter::new(format!("warn,{directives}"))
}

fn restore_terminal() -> eyre::Result<()> {
    disable_raw_mode().wrap_err("disabling raw mode")?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)
        .wrap_err("leaving alternate screen")?;
    Ok(())
}
