use ratatui::prelude::Stylize;
use ratatui::widgets::{Paragraph, Widget};

#[derive(Default)]
pub struct BreakpointsView;

impl Widget for BreakpointsView {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let breakpoints = Paragraph::new(format!("breakpoints")).white().bold();
        breakpoints.render(area, buf);
    }
}
