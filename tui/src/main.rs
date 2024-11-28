use color_eyre::eyre::{self, Context};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Stylize;
use ratatui::{prelude::Backend, widgets::Paragraph, Frame, Terminal};

fn main() -> eyre::Result<()> {
    let mut terminal = ratatui::init();
    terminal.clear().wrap_err("clearing the terminal")?;
    let app_result = run(terminal);
    ratatui::restore();
    app_result
}

fn run<T>(mut terminal: Terminal<T>) -> eyre::Result<()>
where
    T: Backend,
{
    loop {
        terminal.draw(draw).wrap_err("failed to draw frame")?;

        // event handling
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                return Ok(());
            }
        }
    }
    Ok(())
}

fn draw(frame: &mut Frame) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(frame.area());

    let greeting = Paragraph::new("Hello Ratatui! (press 'q' to quit)")
        .white()
        .on_black();
    let bottom = Paragraph::new("Bottom paragraph").white();
    frame.render_widget(greeting, layout[0]);
    frame.render_widget(bottom, layout[1]);
}
