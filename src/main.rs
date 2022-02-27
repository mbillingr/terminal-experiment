#[macro_use]
mod sxfmt;
mod sexpr_view;
mod terminal_backend;
mod textbuffer;

use crate::backend::TextBuffer;
use crate::sxfmt::{Formatter, PrettyExpr, PrettyFormatter};
use crate::terminal_backend as backend;
use crate::textbuffer::RenderTarget;
use crossterm::style::{Attributes, Color};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::{
    cursor,
    event::{read, Event, KeyCode, KeyEvent},
    execute, queue, style,
    style::ContentStyle,
    terminal, ErrorKind, Result,
};
use sexpr_view::SexprView;
use std::io::{stdout, Write};

pub trait Item {
    fn size(&self) -> (usize, usize);
    fn resize(&mut self, width: usize, height: usize);
    fn draw(&self, buf: &mut TextBuffer, x: usize, y: usize) -> Result<()>;
}

pub trait EventHandler {
    fn handle_event(&mut self, event: &Event) -> bool;
}

const DEFAULT_FRAME: [char; 9] = ['╔', '═', '╗', '║', ' ', '║', '╚', '═', '╝'];

struct Framed<T: Item> {
    tiles: &'static [char],
    style: ContentStyle,
    inner: T,
}

impl<T: Item> Framed<T> {
    pub fn new(inner: T) -> Self {
        Framed {
            tiles: &DEFAULT_FRAME,
            style: ContentStyle {
                foreground_color: Some(Color::Blue),
                background_color: Some(Color::Cyan),
                attributes: Attributes::default(),
            },
            inner,
        }
    }
}

impl<T: Item> Item for Framed<T> {
    fn size(&self) -> (usize, usize) {
        let (w, h) = self.inner.size();
        (w + 2, h + 2)
    }

    fn resize(&mut self, width: usize, height: usize) {
        self.inner.resize(width - 2, height - 2)
    }

    fn draw(&self, buf: &mut TextBuffer, x: usize, y: usize) -> Result<()> {
        let (width, height) = self.size();

        // corners
        buf.set_char(x, y, self.tiles[0], self.style);
        buf.set_char(x + width, y, self.tiles[2], self.style);
        buf.set_char(x, y + height, self.tiles[6], self.style);
        buf.set_char(x + width, y + height, self.tiles[8], self.style);

        // edges
        buf.draw_hline(y, x + 1, x + width - 1, self.tiles[1], self.style);
        buf.draw_hline(y + height, x + 1, x + width - 1, self.tiles[7], self.style);
        buf.draw_vline(x, y + 1, y + height - 1, self.tiles[3], self.style);
        buf.draw_vline(x + width, y + 1, y + height - 1, self.tiles[5], self.style);

        //inside
        buf.fill_rect(
            x + 1,
            y + 1,
            x + width,
            y + height,
            self.tiles[4],
            self.style,
        );
        self.inner.draw(buf, x + 1, y + 1)
    }
}

struct CrosstermFormatter<'a, W: Write> {
    buf: &'a mut W,
    current_style: ContentStyle,
    saved_styles: Vec<ContentStyle>,
    start_column: u16,
    current_row: u16,
}

impl<'a, W: Write> CrosstermFormatter<'a, W> {
    pub fn new(buf: &'a mut W, x: u16, y: u16) -> Self {
        CrosstermFormatter {
            buf,
            current_style: Default::default(),
            saved_styles: vec![],
            start_column: x,
            current_row: y,
        }
    }
}

impl<'a, W: Write> Formatter<ContentStyle> for CrosstermFormatter<'a, W> {
    type Error = ErrorKind;

    fn write(&mut self, x: impl std::fmt::Display) -> std::result::Result<(), Self::Error> {
        queue!(self.buf, style::Print(x))
    }

    fn set_style(&mut self, style: &ContentStyle) {
        self.current_style = *style;

        if let Some(fg) = style.foreground_color {
            queue!(self.buf, style::SetForegroundColor(fg)).unwrap()
        }

        if let Some(bg) = style.background_color {
            queue!(self.buf, style::SetBackgroundColor(bg)).unwrap()
        }

        queue!(self.buf, style::SetAttributes(style.attributes)).unwrap();
    }

    fn save_style(&mut self) {
        self.saved_styles.push(self.current_style)
    }

    fn restore_style(&mut self) {
        let style = self.saved_styles.pop().unwrap();
        self.set_style(&style);
    }

    fn write_newline(&mut self) -> std::result::Result<(), Self::Error> {
        self.current_row += 1;
        queue!(
            self.buf,
            cursor::MoveTo(self.start_column, self.current_row)
        )
    }
}

struct TextBufferFormatter<'a> {
    buf: &'a mut TextBuffer,
    current_style: ContentStyle,
    saved_styles: Vec<ContentStyle>,
    start_column: usize,
    current_row: usize,
    cursor: (usize, usize),
}

impl<'a> TextBufferFormatter<'a> {
    pub fn new(buf: &'a mut TextBuffer, x: usize, y: usize) -> Self {
        TextBufferFormatter {
            buf,
            current_style: Default::default(),
            saved_styles: vec![],
            start_column: x,
            current_row: y,
            cursor: (x, y),
        }
    }
}

impl<'a> Formatter<ContentStyle> for TextBufferFormatter<'a> {
    type Error = ErrorKind;

    fn write(&mut self, x: impl std::fmt::Display) -> std::result::Result<(), Self::Error> {
        for ch in x.to_string().chars() {
            self.buf
                .set_char(self.cursor.0, self.cursor.1, ch, self.current_style);
            self.cursor.0 += 1;
        }
        Ok(())
    }

    fn set_style(&mut self, style: &ContentStyle) {
        self.current_style = *style;
    }

    fn save_style(&mut self) {
        self.saved_styles.push(self.current_style)
    }

    fn restore_style(&mut self) {
        let style = self.saved_styles.pop().unwrap();
        self.set_style(&style);
    }

    fn write_newline(&mut self) -> std::result::Result<(), Self::Error> {
        self.current_row += 1;
        self.cursor = (self.start_column, self.current_row);
        Ok(())
    }
}

fn main() -> Result<()> {
    let mut stdout = stdout();
    enable_raw_mode()?;

    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

    let (w, h) = terminal::size()?;
    let mut buffer: TextBuffer = TextBuffer::new(w as usize, h as usize);

    let exp = pe![(let ((a 1) (b 2) (c 3)) ("+" a b))];

    let mut sxv = SexprView::new(exp, 25, 10);

    loop {
        buffer.clear(
            '╳',
            ContentStyle {
                foreground_color: Some(Color::Green),
                background_color: Some(Color::Black),
                attributes: Attributes::default(),
            },
        );

        Framed::new(sxv.clone()).draw(&mut buffer, 2, 1)?;

        buffer.render(&mut stdout)?;

        let event = read()?;
        if !sxv.handle_event(&event) {
            match event {
                Event::Resize(w, h) => {
                    buffer.resize(w as usize, h as usize);
                    sxv.resize(w as usize - 7, h as usize - 5)
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Esc, ..
                }) => break,
                _ => {}
            }
        }
    }

    execute!(stdout, cursor::Show, terminal::LeaveAlternateScreen,)?;
    disable_raw_mode()?;

    Ok(())
}
