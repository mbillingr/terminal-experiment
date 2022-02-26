#[macro_use]
mod sxfmt;
mod sexpr_view;

use crate::sxfmt::{Formatter, PrettyExpr, PrettyFormatter};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::{
    cursor,
    event::{read, Event, KeyCode, KeyEvent},
    execute, queue, style,
    style::{ContentStyle, Stylize},
    terminal, ErrorKind, Result,
};
use sexpr_view::SexprView;
use std::io::{stdout, Write};

pub trait Item {
    fn size(&self) -> (u16, u16);
    fn draw(&self, buf: &mut impl Write, x: u16, y: u16) -> Result<()>;
}

pub trait EventHandler {
    fn handle_event(&mut self, event: &Event) -> bool;
}

const DEFAULT_FRAME: [char; 9] = ['╔', '═', '╗', '║', ' ', '║', '╚', '═', '╝'];
const DEFAULT_SHADOW: [char; 5] = ['▖', '▌', '▝', '▀', '▘'];

struct Framed<T: Item> {
    tiles: &'static [char],
    shadow_tiles: &'static [char],
    inner: T,
}

impl<T: Item> Framed<T> {
    pub fn new(inner: T) -> Self {
        Framed {
            tiles: &DEFAULT_FRAME,
            shadow_tiles: &DEFAULT_SHADOW,
            inner,
        }
    }
}

impl<T: Item> Item for Framed<T> {
    fn size(&self) -> (u16, u16) {
        let (w, h) = self.inner.size();
        (w + 2, h + 2)
    }

    fn draw(&self, buf: &mut impl Write, x: u16, y: u16) -> Result<()> {
        let (width, height) = self.inner.size();

        queue!(
            buf,
            cursor::MoveTo(x, y),
            style::SetForegroundColor(style::Color::DarkGrey),
            style::SetBackgroundColor(style::Color::Grey)
        )?;
        queue!(buf, style::Print(self.tiles[0]))?;
        for _ in 0..width {
            queue!(buf, style::Print(self.tiles[1]))?;
        }
        queue!(
            buf,
            style::Print(self.tiles[2]),
            style::Print(self.shadow_tiles[0].dark_yellow().on_yellow())
        )?;

        for i in 1..height + 1 {
            queue!(
                buf,
                cursor::MoveTo(x, y + i),
                style::SetForegroundColor(style::Color::DarkGrey),
                style::SetBackgroundColor(style::Color::Grey)
            )?;
            queue!(buf, style::Print(self.tiles[3]))?;
            for _ in 0..width {
                queue!(buf, style::Print(self.tiles[4]))?;
            }
            queue!(buf, style::Print(self.tiles[5]))?;
            queue!(
                buf,
                style::Print(self.shadow_tiles[1].dark_yellow().on_yellow())
            )?;
        }

        queue!(
            buf,
            cursor::MoveTo(x, y + height + 1),
            style::SetForegroundColor(style::Color::DarkGrey),
            style::SetBackgroundColor(style::Color::Grey)
        )?;
        queue!(buf, style::Print(self.tiles[6]))?;
        for _ in 0..width {
            queue!(buf, style::Print(self.tiles[7]))?;
        }
        queue!(
            buf,
            style::Print(self.tiles[8]),
            style::Print(self.shadow_tiles[1].dark_yellow().on_yellow())
        )?;

        queue!(
            buf,
            cursor::MoveTo(x, y + height + 2),
            style::SetForegroundColor(style::Color::DarkYellow),
            style::SetBackgroundColor(style::Color::Yellow)
        )?;
        queue!(buf, style::Print(self.shadow_tiles[2]))?;
        for _ in 0..width + 1 {
            queue!(buf, style::Print(self.shadow_tiles[3]))?;
        }
        queue!(buf, style::Print(self.shadow_tiles[4]))?;

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

fn main() -> Result<()> {
    let mut stdout = stdout();
    enable_raw_mode()?;

    execute!(
        stdout,
        /*terminal::EnterAlternateScreen,*/ cursor::Hide
    )?;

    let exp = pe![(let ((a 1) (b 2) (c 3)) ("+" a b))];

    let mut sxv = SexprView::new(exp, 25, 10);

    loop {
        queue!(
            &mut stdout,
            style::ResetColor,
            style::SetBackgroundColor(style::Color::Yellow),
            terminal::Clear(terminal::ClearType::All),
        )?;

        Framed::new(sxv.clone()).draw(&mut stdout, 7, 20)?;

        stdout.flush()?;
        let event = read()?;
        if !sxv.handle_event(&event) {
            match event {
                Event::Key(KeyEvent {
                    code: KeyCode::Esc, ..
                }) => break,
                _ => {}
            }
        }
    }

    execute!(stdout, cursor::Show /*terminal::LeaveAlternateScreen*/,)?;
    disable_raw_mode()?;

    Ok(())
}
