#[macro_use]
mod sxfmt;

use crate::sxfmt::{Formatter, PrettyExpr, PrettyFormatter};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::{
    cursor,
    event::{read, Event, KeyCode, KeyEvent},
    execute, queue, style,
    style::{ContentStyle, Stylize},
    terminal, ErrorKind, Result,
};
use std::io::{stdout, Write};

pub trait Item {
    fn size(&self) -> (u16, u16);
    fn draw(&self, buf: &mut impl Write, x: u16, y: u16) -> Result<()>;
}

const DEFAULT_FRAME: [char; 8] = ['╔', '═', '╗', '║', '║', '╚', '═', '╝'];

struct Framed<T: Item> {
    tiles: &'static [char],
    inner: T,
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
    fn size(&self) -> (u16, u16) {
        let (w, h) = self.inner.size();
        (w + 2, h + 2)
    }

    fn draw(&self, buf: &mut impl Write, x: u16, y: u16) -> Result<()> {
        let (width, height) = self.inner.size();

        queue!(buf, cursor::MoveTo(x, y))?;
        queue!(buf, style::Print(self.tiles[0]))?;
        for _ in 0..width {
            queue!(buf, style::Print(self.tiles[1]))?;
        }
        queue!(buf, style::Print(self.tiles[2]))?;

        for i in 1..height + 1 {
            queue!(buf, cursor::MoveTo(x, y + i))?;
            queue!(
                buf,
                style::Print(self.tiles[3]),
                cursor::MoveRight(width),
                style::Print(self.tiles[4])
            )?;
        }

        queue!(buf, cursor::MoveTo(x, y + height + 1))?;
        queue!(buf, style::Print(self.tiles[5]))?;
        for _ in 0..width {
            queue!(buf, style::Print(self.tiles[6]))?;
        }
        queue!(buf, style::Print(self.tiles[7]))?;

        self.inner.draw(buf, x + 1, y + 1)
    }
}

#[derive(Clone)]
struct SexprView {
    expr: PrettyExpr<ContentStyle>,
    width: u16,
    height: u16,
    cursor: Option<Vec<usize>>,
}

impl SexprView {
    pub fn new(expr: PrettyExpr<ContentStyle>, width: u16, height: u16) -> Self {
        SexprView {
            expr,
            width,
            height,
            cursor: Some(vec![1]),
        }
    }

    pub fn move_cursor_out_of_list(&mut self) {
        match &mut self.cursor {
            None => {}
            Some(c) if c.is_empty() => self.cursor = None,
            Some(c) => {
                if self.expr.is_valid_path(c) {
                    c.pop().unwrap();
                }
            }
        }
    }

    pub fn move_cursor_into_list(&mut self) {
        match &mut self.cursor {
            None => self.cursor = Some(vec![]),
            Some(c) => {
                c.push(0);
                if !self.expr.is_valid_path(c) {
                    c.pop().unwrap();
                }
            }
        }
    }

    pub fn move_cursor_in_list(&mut self, dir: i8) {
        match &mut self.cursor {
            None => self.cursor = Some(vec![]),
            Some(c) if c.is_empty() => {}
            Some(c) => {
                let new_pos = c.pop().unwrap() as isize + dir as isize;
                let l = self.expr.get(c).unwrap().len() as isize;
                let new_pos = (new_pos + l) % l as isize;
                c.push(new_pos as usize);
                execute!(stdout(), cursor::MoveTo(0, 0)).unwrap();
            }
        }
    }

    pub fn append_at_cursor(&mut self, postfix: &str) {
        match &self.cursor {
            None => {}
            Some(c) => {
                let x = self.expr.get_mut(c).unwrap();
                if let Some(text) = x.get_text() {
                    let text = text.to_string() + postfix;
                    *x = PrettyExpr::Atom(text);
                } else if x.is_empty_list() {
                    x.elements_mut()
                        .unwrap()
                        .push(PrettyExpr::Atom(postfix.to_string()));
                    self.move_cursor_into_list();
                }
            }
        }
    }

    pub fn delete_at_cursor(&mut self) {
        match &self.cursor {
            None => {}
            Some(c) => {
                let x = self.expr.get_mut(c).unwrap();
                if let Some(text) = x.get_text() {
                    let mut text = text.to_string();
                    text.pop();
                    if text.is_empty() {
                        *x = PrettyExpr::Placeholder;
                    } else {
                        *x = PrettyExpr::Atom(text);
                    }
                }
            }
        }
    }

    pub fn delete_cursor_element(&mut self) {
        match self.cursor.as_ref().map(Vec::as_slice) {
            Some([c_list @ .., c_elem]) => {
                let c_elem = *c_elem;
                let elements = self
                    .expr
                    .get_mut(c_list)
                    .and_then(PrettyExpr::elements_mut)
                    .unwrap();
                elements.remove(c_elem);
                if elements.is_empty() {
                    self.cursor.as_mut().unwrap().pop();
                } else {
                    let last = self.cursor.as_mut().and_then(|c| c.last_mut()).unwrap();
                    *last = usize::min(c_elem, elements.len() - 1)
                }
            }
            _ => {}
        }
    }

    pub fn insert_element_after_cursor(&mut self) {
        match self.cursor.as_ref().map(Vec::as_slice) {
            Some([c_list @ .., c_elem]) => {
                let c_elem = *c_elem;
                let elements = self
                    .expr
                    .get_mut(c_list)
                    .and_then(PrettyExpr::elements_mut)
                    .unwrap();
                elements.insert(c_elem + 1, PrettyExpr::Placeholder);
                self.move_cursor_in_list(1);
            }
            _ => {}
        }
    }

    pub fn wrap_cursor_in_list(&mut self) {
        match &self.cursor {
            None => {}
            Some(c) => {
                let x = self.expr.get_mut(c).unwrap();
                let y = x.clone();
                *x = PrettyExpr::list(vec![y]);
            }
        }
    }

    pub fn unwrap_unary_list_at_cursor(&mut self) {
        match &self.cursor {
            None => {}
            Some(c) => {
                let x = self.expr.get_mut(c).unwrap();
                if let Some([y]) = x.elements() {
                    *x = y.clone();
                }
            }
        }
    }
}

impl Item for SexprView {
    fn size(&self) -> (u16, u16) {
        return (self.width, self.height);
    }

    fn draw(&self, buf: &mut impl Write, x: u16, y: u16) -> Result<()> {
        queue!(buf, cursor::MoveTo(x, y))?;
        let mut pf = PrettyFormatter::default();
        pf.max_code_width = self.width as usize;
        let mut pe = pf.pretty(self.expr.clone());

        if let Some(path) = &self.cursor {
            pe = pe
                .with_style(path, ContentStyle::new().on_dark_green())
                .unwrap();
        }

        let mut cf = CrosstermFormatter::new(buf, x, y);
        pe.write(&mut cf)
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
    //enable_raw_mode()?;

    //execute!(stdout, /*terminal::EnterAlternateScreen,*/ cursor::Hide)?;

    let exp = pe![(let ((a 1) (b 2) (c 3)) ("+" a b))];
    let exp = exp
        .with_style(&[], ContentStyle::default().white().on_black())
        .unwrap();

    let mut sxv = SexprView::new(exp, 25, 10);

    loop {
        queue!(
            &mut stdout,
            terminal::Clear(terminal::ClearType::All),
            style::ResetColor
        )?;

        Framed::new(sxv.clone()).draw(&mut stdout, 7, 20)?;

        stdout.flush()?;
        match read()? {
            Event::Key(KeyEvent {
                code: KeyCode::Esc, ..
            }) => break,
            Event::Key(KeyEvent {
                code: KeyCode::Left,
                ..
            }) => sxv.move_cursor_out_of_list(),
            Event::Key(KeyEvent {
                code: KeyCode::Right,
                ..
            }) => sxv.move_cursor_into_list(),
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                ..
            }) => sxv.move_cursor_in_list(1),
            Event::Key(KeyEvent {
                code: KeyCode::Up, ..
            }) => sxv.move_cursor_in_list(-1),
            Event::Key(KeyEvent {
                code: KeyCode::Delete,
                ..
            }) => sxv.delete_cursor_element(),
            Event::Key(KeyEvent {
                code: KeyCode::PageUp,
                ..
            }) => sxv.wrap_cursor_in_list(),
            Event::Key(KeyEvent {
                code: KeyCode::PageDown,
                ..
            }) => sxv.unwrap_unary_list_at_cursor(),
            Event::Key(KeyEvent {
                code: KeyCode::Char('('),
                ..
            }) => sxv.wrap_cursor_in_list(),
            Event::Key(KeyEvent {
                code: KeyCode::Char(')'),
                ..
            }) => {}
            Event::Key(KeyEvent {
                code: KeyCode::Char(' '),
                ..
            }) => sxv.insert_element_after_cursor(),
            Event::Key(KeyEvent {
                code: KeyCode::Char(ch),
                ..
            }) => sxv.append_at_cursor(&ch.to_string()),
            Event::Key(KeyEvent {
                code: KeyCode::Backspace,
                ..
            }) => sxv.delete_at_cursor(),
            _ => {}
        }
    }

    //execute!(stdout, cursor::Show, /*terminal::LeaveAlternateScreen*/)?;
    //disable_raw_mode()?;

    Ok(())
}
