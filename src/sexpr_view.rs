use crate::{CrosstermFormatter, EventHandler, Item, PrettyExpr, PrettyFormatter};
use crossterm::style::Stylize;
use crossterm::{cursor, event, queue, style};
use std::io::Write;

#[derive(Clone)]
pub struct SexprView {
    expr: PrettyExpr<style::ContentStyle>,
    width: u16,
    height: u16,
    cursor: Option<Vec<usize>>,
}

impl SexprView {
    pub fn new(expr: PrettyExpr<style::ContentStyle>, width: u16, height: u16) -> Self {
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
                elements.insert(c_elem + 1, PrettyExpr::Inline(vec![]));
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

    fn draw(&self, buf: &mut impl Write, x: u16, y: u16) -> crossterm::Result<()> {
        queue!(buf, cursor::MoveTo(x, y))?;
        let mut pf = PrettyFormatter::default();
        pf.max_code_width = self.width as usize;
        let mut pe = pf.pretty(self.expr.clone());

        if let Some(path) = &self.cursor {
            pe = pe
                .with_style(path, style::ContentStyle::new().on_dark_green())
                .unwrap();
        }

        let mut cf = CrosstermFormatter::new(buf, x, y);
        pe.write(&mut cf)
    }
}

impl EventHandler for SexprView {
    fn handle_event(&mut self, event: &event::Event) -> bool {
        use crossterm::event::Event::*;
        use crossterm::event::KeyCode::*;
        use crossterm::event::KeyEvent;
        match event {
            Key(KeyEvent { code: Left, .. }) => self.move_cursor_out_of_list(),
            Key(KeyEvent { code: Right, .. }) => self.move_cursor_into_list(),
            Key(KeyEvent { code: Down, .. }) => self.move_cursor_in_list(1),
            Key(KeyEvent { code: Up, .. }) => self.move_cursor_in_list(-1),
            Key(KeyEvent { code: Delete, .. }) => self.delete_cursor_element(),
            Key(KeyEvent { code: PageUp, .. }) => self.wrap_cursor_in_list(),
            Key(KeyEvent { code: PageDown, .. }) => self.unwrap_unary_list_at_cursor(),
            Key(KeyEvent {
                code: Char('('), ..
            }) => self.wrap_cursor_in_list(),
            Key(KeyEvent {
                code: Char(')'), ..
            }) => {}
            Key(KeyEvent {
                code: Char(' '), ..
            }) => self.insert_element_after_cursor(),
            Key(KeyEvent { code: Char(ch), .. }) => self.append_at_cursor(&ch.to_string()),
            Key(KeyEvent {
                code: Backspace, ..
            }) => self.delete_at_cursor(),
            _ => return false,
        }
        true
    }
}
