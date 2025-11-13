use std::ops::Range;

use iced::{advanced, highlighter::Highlighter as IcedHighlighter};

pub(crate) struct Highlighter {
    inner: IcedHighlighter,
}

impl advanced::text::Highlighter for Highlighter {
    type Settings = iced::highlighter::Settings;
    type Highlight = iced::highlighter::Highlight;
    type Iterator<'a>
        = Box<dyn Iterator<Item = (Range<usize>, iced::highlighter::Highlight)> + 'a>
    where
        Self: 'a;

    fn new(settings: &Self::Settings) -> Self {
        let inner = IcedHighlighter::new(settings);
        Self { inner }
    }

    fn update(&mut self, new_settings: &Self::Settings) {
        self.inner.update(new_settings)
    }

    fn change_line(&mut self, line: usize) {
        self.inner.change_line(line)
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        self.inner.highlight_line(line)
    }

    fn current_line(&self) -> usize {
        self.inner.current_line()
    }
}
