use crate::{textbuffer, RenderTarget};
use crossterm::{cursor, queue, style, style::ContentStyle};
use std::io::{Result, Stdout, Write};

pub type TextBuffer = textbuffer::TextBuffer<ContentStyle>;

impl RenderTarget for Stdout {
    type Error = std::io::Error;
    type Style = ContentStyle;

    fn prepare(&mut self) -> Result<()> {
        queue!(self, cursor::MoveTo(0, 0))
    }

    fn finalize(&mut self) -> Result<()> {
        self.flush()
    }

    fn write_char(&mut self, ch: char, s: &ContentStyle) -> Result<()> {
        queue!(self, style::PrintStyledContent(s.apply(ch)))
    }
}
