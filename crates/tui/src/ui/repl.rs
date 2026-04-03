use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
};

use crate::app::App;

use super::border_style;
use crate::app::Focus;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style(app, Focus::Repl))
        .title(" REPL ");

    let paragraph = Paragraph::new("(empty)").block(block);
    frame.render_widget(paragraph, area);
}
