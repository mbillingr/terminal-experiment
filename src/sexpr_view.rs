use crate::backend::TextBuffer;
use crate::styles::Style;
use crate::{EventHandler, Item, PrettyExpr, PrettyFormatter, TextBufferFormatter};
use crossterm::event;

#[derive(Clone)]
pub struct SexprView {
    expr: PrettyExpr<Style>,
    width: usize,
    height: usize,
    cursor: Vec<usize>,
}

impl SexprView {
    pub fn new(expr: PrettyExpr<Style>, width: usize, height: usize) -> Self {
        SexprView {
            expr,
            width,
            height,
            cursor: vec![],
        }
    }

    pub fn move_cursor_out_of_list(&mut self) {
        self.cursor.pop();
    }

    pub fn move_cursor_into_list(&mut self) {
        self.cursor.push(0);
        if !self.expr.is_valid_path(&self.cursor) {
            self.cursor.pop().unwrap();
        }
    }

    pub fn move_cursor_in_list(&mut self, dir: i8) {
        if self.cursor.is_empty() {
            return;
        }
        let new_pos = self.cursor.pop().unwrap() as isize + dir as isize;
        let l = self.expr.get(&self.cursor).unwrap().len() as isize;
        let new_pos = (new_pos + l) % l as isize;
        self.cursor.push(new_pos as usize);
    }

    pub fn append_at_cursor(&mut self, postfix: &str) {
        let x = self.expr.get_mut(&self.cursor).unwrap();
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

    pub fn delete_at_cursor(&mut self) {
        let x = self.expr.get_mut(&self.cursor).unwrap();
        if let Some(text) = x.get_text() {
            let mut text = text.to_string();
            text.pop();
            if text.is_empty() {
                *x = PrettyExpr::list(vec![]);
            } else {
                *x = PrettyExpr::Atom(text);
            }
        }
    }

    pub fn delete_cursor_element(&mut self) {
        match self.cursor.as_slice() {
            [c_list @ .., c_elem] => {
                let c_elem = *c_elem;
                let x = self.expr.get_mut(c_list).unwrap();
                x.remove_item(c_elem);
                if x.is_empty_list() {
                    self.cursor.pop();
                } else {
                    let last = self.cursor.last_mut().unwrap();
                    *last = usize::min(c_elem, x.len() - 1)
                }
            }
            [] => {} //self.expr = PrettyExpr::empty_list(),
        }
    }

    pub fn insert_element_after_cursor(&mut self) {
        match self.cursor.as_slice() {
            [c_list @ .., c_elem] => {
                let c_elem = *c_elem;
                let x = self.expr.get_mut(c_list).unwrap();
                if x.is_quotation() {
                    self.move_cursor_out_of_list();
                    self.insert_element_after_cursor();
                } else {
                    let elements = x.elements_mut().unwrap();
                    elements.insert(c_elem + 1, PrettyExpr::empty_list());
                    self.move_cursor_in_list(1);
                }
            }
            _ => {}
        }
    }

    pub fn quote_cursor(&mut self) {
        let x = self.expr.get_mut(&self.cursor).unwrap();
        let y = x.clone();
        *x = PrettyExpr::quote(y);
    }

    pub fn wrap_cursor_in_list(&mut self) {
        let x = self.expr.get_mut(&self.cursor).unwrap();
        let y = x.clone();
        *x = PrettyExpr::list(vec![y]);
    }

    pub fn unwrap_unary_list_at_cursor(&mut self) {
        let x = self.expr.get_mut(&self.cursor).unwrap();
        if let Some([y]) = x.elements() {
            *x = y.clone();
        } else if let Some(y) = x.quoted_value() {
            *x = y.clone();
        }
    }
}

impl Item for SexprView {
    fn size(&self) -> (usize, usize) {
        return (self.width, self.height);
    }

    fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
    }

    fn draw(&self, buf: &mut TextBuffer, x: usize, y: usize) -> crossterm::Result<()> {
        let mut pf = PrettyFormatter::default();
        pf.max_code_width = self.width as usize;
        let mut pe = pf.pretty(self.expr.clone());

        pe = pe
            .with_style(&[], Style::Default)
            .unwrap()
            .with_style(&self.cursor, Style::Highlight)
            .unwrap();

        let mut cf = TextBufferFormatter::new(buf, x, y);
        pe.write(&mut cf)
    }
}

impl EventHandler<event::Event> for SexprView {
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
                code: Char('\''), ..
            }) => {
                self.quote_cursor();
                self.move_cursor_into_list();
            }
            Key(KeyEvent {
                code: Char('('), ..
            }) => {
                self.wrap_cursor_in_list();
                self.move_cursor_into_list();
            }
            Key(KeyEvent {
                code: Char(')'), ..
            }) => self.move_cursor_out_of_list(),
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
