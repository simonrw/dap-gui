use color_eyre::eyre::{self, Context};
use crossterm::event::{self, Event};
use ratatui::{prelude::Backend, Frame, Terminal};

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
        if matches!(
            event::read().wrap_err("failed to read event")?,
            Event::Key(_)
        ) {
            break;
        }
    }
    Ok(())
}

fn draw(_frame: &mut Frame) {}
