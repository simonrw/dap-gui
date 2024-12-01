use color_eyre::eyre::{self, Context};
use crossbeam_channel::{select, Receiver};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Stylize;
use ratatui::{prelude::Backend, widgets::Paragraph, Frame, Terminal};

fn main() -> eyre::Result<()> {
    let mut terminal = ratatui::init();
    terminal.clear().wrap_err("clearing the terminal")?;
    let (_tx, rx) = crossbeam_channel::unbounded::<()>();
    let app_result = run(terminal, rx);
    ratatui::restore();
    app_result
}

fn run<T>(mut terminal: Terminal<T>, debugger_events: Receiver<()>) -> eyre::Result<()>
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
        terminal.draw(draw).wrap_err("failed to draw frame")?;

        // event handling
        select! {
            // terminal events
            recv(term_rx) -> msg => if let Event::Key(key) = msg? {
            if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                return Ok(());
            }
        },
        recv(debugger_events) -> msg => {
            if let Ok(msg) = msg {
                dbg!(msg);
            }
        },
        }
    }
}

fn draw(frame: &mut Frame) {
    let outer_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(frame.area());
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(outer_layout[0]);

    let greeting = Paragraph::new("Hello Ratatui! (press 'q' to quit)")
        .white()
        .on_black();
    let bottom = Paragraph::new("Bottom paragraph").white();
    let p = Paragraph::new("Foo").white().bold();

    frame.render_widget(greeting, layout[0]);
    frame.render_widget(bottom, layout[1]);
    frame.render_widget(p, outer_layout[1]);
}
