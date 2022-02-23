#[macro_use]
mod sxfmt;

use crate::sxfmt::{Formatter, PrettyExpr, PrettyFormatter};
use crossterm::style::Color;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::{
    cursor,
    event::{poll, read, Event},
    execute, queue, style,
    style::{ContentStyle, Stylize},
    terminal, ErrorKind, Result,
};
use std::fmt::Display;
use std::io::{stdout, Write};
use std::time::Duration;

pub trait Item {
    fn size(&self) -> (u16, u16);
    fn draw(&self, buf: &mut impl Write, x: u16, y: u16) -> Result<()>;
}

const DEFAULT_FRAME: [char;8] = ['╔', '═', '╗', '║', '║', '╚', '═', '╝'];

struct Framed<T: Item> {
    tiles: &'static[char],
    inner: T
}

impl<T: Item> Framed<T> {
    pub fn new(inner: T) -> Self {
        Framed {
            tiles: &DEFAULT_FRAME,
            inner,
        }
    }
}

impl<T: Item> Item for Framed<T> {
    fn draw(&self, buf: &mut impl Write, x: u16, y:u16) -> Result<()> {
        let (width, height) = self.inner.size();

        queue!(buf, cursor::MoveTo(x, y))?;
        queue!(buf, style::Print(self.tiles[0]))?;
        for _ in 0..width {
            queue!(buf, style::Print(self.tiles[1]))?;
        }
        queue!(buf, style::Print(self.tiles[2]))?;

        for i in 1..height+1 {
            queue!(buf, cursor::MoveTo(x, y+i))?;
            queue!(buf, style::Print(self.tiles[3]), cursor::MoveRight(width), style::Print(self.tiles[4]))?;
        }

        queue!(buf, cursor::MoveTo(x, y+height+1))?;
        queue!(buf, style::Print(self.tiles[5]))?;
        for _ in 0..width {
            queue!(buf, style::Print(self.tiles[6]))?;
        }
        queue!(buf, style::Print(self.tiles[7]))?;

        self.inner.draw(buf, x+1, y+1)
    }

    fn size(&self) -> (u16, u16) {
        let (w, h) = self.inner.size();
        (w+2, h+2)
    }
}

struct Hello;

impl Item for Hello {
    fn size(&self) -> (u16, u16) {
        return (19, 3);
    }

    fn draw(&self, buf: &mut impl Write, x: u16, y: u16) -> Result<()> {
        queue!(
            buf,
            style::SetForegroundColor(Color::Yellow),
            style::SetBackgroundColor(Color::DarkBlue)
        )?;
        queue!(buf, cursor::MoveTo(x, y), style::Print("+-----------------+"))?;
        queue!(
                buf,
                cursor::MoveTo(x, y+1),
                style::Print("|"),
                style::Print("  Hello, World!  "),
                style::Print("|")
            )?;
        queue!(buf, cursor::MoveTo(x, y+2), style::Print("+-----------------+"))
    }
}

struct CrosstermFormatter<'a, W: Write> {
    buf: &'a mut W,
    current_style: ContentStyle,
    saved_styles: Vec<ContentStyle>,
}

impl<'a, W: Write> CrosstermFormatter<'a, W> {
    pub fn new(buf: &'a mut W) -> Self {
        CrosstermFormatter {
            buf,
            current_style: Default::default(),
            saved_styles: vec![],
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
}

fn main() -> Result<()> {
    let mut stdout = stdout();
    enable_raw_mode()?;

    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

    let mut pos: (i16, i16) = (12, 9);
    let mut dir: (i16, i16) = (1, 1);

    loop {
        if poll(Duration::from_millis(0))? {
            match read()? {
                Event::Key(_) => break,
                _ => {}
            }
        }

        queue!(&mut stdout, style::ResetColor)?;

        let hi = Framed::new(Hello);
        hi.draw(&mut stdout, 25, 20)?;

        Hello.draw(&mut stdout, 25, 25)?;
        Hello.draw(&mut stdout, 20, 26)?;

        queue!(
            stdout,
            cursor::MoveTo(5, 5),
            style::Print("+---------------+")
        )?;
        for y in 6..15 {
            queue!(
                stdout,
                cursor::MoveTo(5, y),
                style::Print("|               |")
            )?;
        }
        queue!(
            stdout,
            cursor::MoveTo(5, 15),
            style::Print("+---------------+")
        )?;

        if pos.0 <= 6 || pos.0 >= 20 {
            dir.0 = -dir.0;
        }

        if pos.1 <= 6 || pos.1 >= 14 {
            dir.1 = -dir.1;
        }

        pos = (pos.0 + dir.0, pos.1 + dir.1);
        queue!(
            stdout,
            cursor::MoveTo(pos.0 as u16, pos.1 as u16),
            style::PrintStyledContent("*".green())
        )?;

        queue!(stdout, cursor::MoveTo(0, 30))?;
        let mut cf = CrosstermFormatter::new(&mut stdout);
        let pf = PrettyFormatter::default();
        let exp = pe![(let ((a 1) (b 2) (c 3)) ("+" a b))];
        let exp = PrettyExpr::styled(ContentStyle::default().white().on_black(), exp);
        let exp = exp
            .with_style(&[1, 1, 0], ContentStyle::default().on_dark_green())
            .unwrap();
        println!("{:?}", exp);
        let pe = pf.pretty(exp);
        pe.write(&mut cf)?;

        stdout.flush()?;
        std::thread::sleep(Duration::from_millis(50));
    }

    execute!(stdout, terminal::LeaveAlternateScreen)?;
    disable_raw_mode()?;

    Ok(())
}
