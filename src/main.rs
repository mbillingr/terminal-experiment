use std::fmt::Display;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::{
    cursor,
    event::{poll, read, Event},
    execute, queue, style,
    style::Stylize,
    terminal, Result,
};
use std::io::{stdout, Write};
use std::time::Duration;
use crossterm::style::Color;


pub trait Item {
    fn draw_line(&self, buf: &mut impl Write, line: u16) -> Result<()>;

    fn size(&self) -> (u16, u16);

    fn draw(&self, buf: &mut impl Write, x: u16, y:u16) -> Result<()> {
        for i in 0..Hello.size().1 {
            queue!(buf, cursor::MoveTo(x, y+i))?;
            Hello.draw_line(buf, i)?;
        }
        Ok(())
    }
}


struct Hello;

impl Item for Hello {
    fn draw_line(&self, buf: &mut impl Write, line: u16) -> Result<()> {
        queue!(buf, style::SetForegroundColor(Color::Yellow), style::SetBackgroundColor(Color::DarkBlue))?;
        match line {
            0 => queue!(buf, style::Print("+-----------------+")),
            1 => queue!(buf, style::Print("|"), style::PrintStyledContent("  Hello, World!  ".yellow().on_dark_blue()), style::Print("|")),
            2 => queue!(buf, style::Print("+-----------------+")),
            _ => Ok(()),
        }
    }

    fn size(&self) -> (u16, u16) {
        return (19, 3)
    }
}


pub enum PrintExp {
    Atom(String),
    Static(&'static str),
    List(Vec<PrintExp>),
}

const MAX_WIDTH: usize = 15;
const MAX_OPERATOR_MIX_LENGTH: usize = 4;

impl PrintExp {
    fn print(&self) {
        if let Some(x) = self.print_inline(0) {
            print!("{}", x)
        }

        if let Some(x) = self.print_mixed(0, 0) {
            print!("{}", x)
        }

        if let Some(x) = self.print_long(0, 0) {
            print!("{}", x)
        }
    }

    fn print_inline(&self, current_column: usize) -> Option<String> {
        // (if q a e)
        match self {
            PrintExp::Atom(s) => Some(format!("{}", s)),
            PrintExp::Static(s) => Some(format!("{}", s)),
            PrintExp::List(xs) => {
                let mut items = vec![];
                for x in xs {
                    items.push(x.print_inline(current_column)?);
                }
                let out = format!("({})", items.join(" "));
                if current_column + out.len() > MAX_WIDTH {
                    None
                } else {
                    Some(out)
                }
            }
        }
    }

    fn print_mixed(&self, current_column: usize, next_indent: usize) -> Option<String> {
        // (if q
        //     a
        //     e)
        todo!()
    }

    fn print_long(&self, current_column: usize, next_indent: usize) -> Option<String> {
        // (if
        //   q
        //   a
        //   e)
        todo!()
    }
}


pub trait PrettyPrint {
    fn prepare(&self) -> PrintExp;
}

impl PrettyPrint for () {
    fn prepare(&self) -> PrintExp {
        PrintExp::Static("()")
    }
}

impl<T: PrettyPrint> PrettyPrint for Vec<T> {
    fn prepare(&self) -> PrintExp {
        PrintExp::List(self.iter().map(PrettyPrint::prepare).collect())
    }
}


fn pretty_print<T:PrettyPrint>(x: &T) {
    x.prepare().print();
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

        stdout.flush()?;
        std::thread::sleep(Duration::from_millis(50));
    }

    execute!(stdout, terminal::LeaveAlternateScreen)?;
    disable_raw_mode()?;

    Ok(())
}
