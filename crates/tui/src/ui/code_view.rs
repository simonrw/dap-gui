use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::App;

use super::border_style;
use crate::app::Focus;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style(app, Focus::CodeView))
        .title(" Code ");

    let paragraph = Paragraph::new("(no file open)")
        .alignment(Alignment::Center)
        .block(block);

    frame.render_widget(paragraph, area);
}
