use color_eyre::eyre::{self, Context};
use crossbeam_channel::{select, Receiver, Select, SelectTimeoutError};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::{prelude::*, widgets::Paragraph, Frame, Terminal};
use std::time::Duration;
use widgets::breakpoints::BreakpointsView;

mod widgets;

struct App {
    should_quit: bool,
}

fn main() -> eyre::Result<()> {
    let mut terminal = ratatui::init();
    terminal.clear().wrap_err("clearing the terminal")?;
    let (tx, rx) = crossbeam_channel::unbounded::<()>();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        let _ = tx.send(());
    });
    let mut app = App { should_quit: false };
    let app_result = run(&mut app, terminal, rx);
    ratatui::restore();
    app_result
}

fn run<T>(
    app: &mut App,
    mut terminal: Terminal<T>,
    debugger_events: Receiver<()>,
) -> eyre::Result<()>
where
    T: Backend,
{
    // set up background thread for terminal events
    let (term_tx, term_rx) = crossbeam_channel::unbounded();
    std::thread::spawn(move || {
        loop {
            let event = event::read()?;
            let _ = term_tx.send(event);
        }

        // this return value is needed for function typing
        #[allow(unreachable_code)]
        Ok::<_, std::io::Error>(())
    });

    loop {
        // rendering
        terminal
            .draw(|frame| draw(app, frame))
            .wrap_err("failed to draw frame")?;

        // event handling
        select! {
            // terminal events
            recv(term_rx) -> msg => if let Event::Key(key) = msg? {
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                    return Ok(());
                }
            },
            recv(debugger_events) -> msg => {
                if let Ok(_) = msg {
                }
            },
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn draw(app: &App, frame: &mut Frame) {
    let outer_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(frame.area());
    let left_panel = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(outer_layout[0]);
    let variables_frame = left_panel[0];
    let stack_frame = left_panel[1];

    let right_panel = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(outer_layout[1]);
    let code_view = right_panel[0];
    let breakpoints_area = right_panel[1];

    let greeting = Paragraph::new("Hello Ratatui! (press 'q' to quit)")
        .white()
        .on_black();
    let bottom = Paragraph::new("Bottom paragraph").white();
    let p = Paragraph::new(format!("App value")).white().bold();

    frame.render_widget(greeting, variables_frame);
    frame.render_widget(bottom, stack_frame);
    frame.render_widget(p, code_view);

    frame.render_widget(BreakpointsView::default(), breakpoints_area);
}
