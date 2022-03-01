use crate::{styles, textbuffer, RenderTarget};
use crossterm::event::KeyEvent;
use crossterm::style::Stylize;
use crossterm::{cursor, queue, style, style::ContentStyle};
use std::io::{Result, Stdout, Write};

pub type TextBuffer = textbuffer::TextBuffer<styles::Style>;

impl RenderTarget for Stdout {
    type Error = std::io::Error;
    type Style = styles::Style;

    fn prepare(&mut self) -> Result<()> {
        queue!(self, cursor::MoveTo(0, 0))
    }

    fn finalize(&mut self) -> Result<()> {
        self.flush()
    }

    fn write_char(&mut self, ch: char, s: &Self::Style) -> Result<()> {
        let s = adapt_style(s);
        queue!(self, style::PrintStyledContent(s.apply(ch)))
    }
}

fn adapt_style(s: &styles::Style) -> style::ContentStyle {
    use styles::Style::*;
    match s {
        Default => ContentStyle::new().white().on_dark_grey(),
        Background => ContentStyle::new().dark_green().on_dark_grey().bold(),
        Frame => ContentStyle::new().black().on_dark_grey(),
        Highlight => ContentStyle::new().black().on_dark_green(),
    }
}

pub fn adapt_event(e: crossterm::event::Event) -> crate::events::Event {
    use crate::events::Event as Y;
    use crossterm::event::Event as X;
    use crossterm::event::KeyCode::*;
    match e {
        X::Key(KeyEvent { code: Char(ch), .. }) => Y::Edit(ch),
        X::Key(KeyEvent {
            code: Backspace, ..
        }) => Y::EditBackspace,
        X::Key(KeyEvent { code: Delete, .. }) => Y::EditDelete,
        X::Key(KeyEvent { code: Left, .. }) => Y::NavLeft,
        X::Key(KeyEvent { code: PageDown, .. }) => Y::EditWrap,
        X::Key(KeyEvent { code: PageUp, .. }) => Y::EditUnwrap,
        X::Key(KeyEvent { code: Right, .. }) => Y::NavRight,
        X::Key(KeyEvent { code: Up, .. }) => Y::NavUp,
        X::Key(KeyEvent { code: Down, .. }) => Y::NavDown,
        _ => Y::Unknown,
    }
}
