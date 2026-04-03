use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
};

use crate::app::App;

pub fn render(_app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" Threads ");

    let paragraph = Paragraph::new("(empty)").block(block);
    frame.render_widget(paragraph, area);
}
