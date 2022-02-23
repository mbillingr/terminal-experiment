use std::marker::PhantomData;

macro_rules! pe {
    (($($xs:tt)*)) => {$crate::sxfmt::PrettyExpr::list(vec![$(pe!($xs)),*])};
    ($x:ident) => {$crate::sxfmt::PrettyExpr::Stat(stringify!{$x})};
    ($x:expr) => {$crate::sxfmt::PrettyExpr::Atom($x.to_string())};
}

#[derive(Debug, Clone)]
pub enum PrettyExpr<T = ()> {
    Atom(String),
    Stat(&'static str),
    Inline(Vec<PrettyExpr<T>>),
    Expand(Vec<PrettyExpr<T>>),
    Style(T, Box<PrettyExpr<T>>),
}

impl<T> PrettyExpr<T> {
    pub fn list(xs: Vec<PrettyExpr<T>>) -> Self {
        PrettyExpr::Inline(xs)
    }

    pub fn styled(style: impl Into<T>, exp: impl Into<PrettyExpr<T>>) -> Self {
        PrettyExpr::Style(style.into(), Box::new(exp.into()))
    }

    pub fn with_style(self, path: &[usize], style: impl Into<T>) -> Option<Self> {
        use PrettyExpr::*;
        match (path, self) {
            (_, Style(s, x)) => Some(Self::styled(s, x.with_style(path, style)?)),
            ([], x) => Some(Self::styled(style, x)),
            ([p, rest @ ..], Inline(xs)) => Self::list_with_style(xs, *p, rest, style).map(Inline),
            ([p, rest @ ..], Expand(xs)) => Self::list_with_style(xs, *p, rest, style).map(Expand),
            (_, Atom(_) | Stat(_)) => None,
        }
    }

    pub fn is_valid_path(&self, path: &[usize]) -> bool {
        self.get(path).is_some()
    }

    pub fn get(&self, path: &[usize]) -> Option<&Self> {
        use PrettyExpr::*;
        match (path, self) {
            (_, Style(_, x)) => x.get(path),
            ([], x) => Some(x),
            ([p, rest @ ..], Inline(xs) | Expand(xs)) => xs.get(*p).and_then(|x| x.get(rest)),
            (_, Atom(_) | Stat(_)) => None,
        }
    }

    fn list_with_style(
        xs: Vec<Self>,
        p: usize,
        rest_path: &[usize],
        style: impl Into<T>,
    ) -> Option<Vec<Self>> {
        let mut out = Vec::with_capacity(xs.len());
        let mut xs = xs.into_iter();

        for _ in 0..p {
            out.push(xs.next()?);
        }

        out.push(xs.next()?.with_style(rest_path, style)?);

        out.extend(xs);
        Some(out)
    }

    pub fn is_atom(&self) -> bool {
        match self {
            PrettyExpr::Atom(_) | PrettyExpr::Stat(_) => true,
            PrettyExpr::Inline(_) | PrettyExpr::Expand(_) => false,
            PrettyExpr::Style(_, x) => x.is_atom(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            PrettyExpr::Atom(_) | PrettyExpr::Stat(_) => 0,
            PrettyExpr::Inline(xs) | PrettyExpr::Expand(xs) => xs.len(),
            PrettyExpr::Style(_, x) => x.len(),
        }
    }

    fn inline_width(&self) -> usize {
        match self {
            PrettyExpr::Atom(x) => x.len(),
            PrettyExpr::Stat(x) => x.len(),
            PrettyExpr::Inline(xs) => {
                let n_spaces = if xs.len() < 2 { 0 } else { xs.len() - 1 };
                2 + xs.iter().map(PrettyExpr::inline_width).sum::<usize>() + n_spaces
            }
            PrettyExpr::Style(_, x) => x.inline_width(),
            _ => unimplemented!(),
        }
    }
}

#[derive(Copy, Clone)]
pub struct PrettyFormatter {
    pub max_code_width: usize,
    pub default_indent: usize,
}

impl Default for PrettyFormatter {
    fn default() -> Self {
        PrettyFormatter {
            max_code_width: 15,
            default_indent: 2,
        }
    }
}

pub struct Pretty<T> {
    pf: PrettyFormatter,
    pe: PrettyExpr<T>,
}

impl<T> Pretty<T> {
    pub fn write<F: Formatter<T>>(&self, f: &mut F) -> Result<(), F::Error> {
        self.pf.write(&self.pe, 0, f)
    }

    pub fn with_style(self, path: &[usize], style: impl Into<T>) -> Option<Self> {
        self.pe
            .with_style(path, style)
            .map(|pe| Pretty { pe, ..self })
    }
}

impl PrettyFormatter {
    pub fn new(max_code_width: usize, default_indent: usize) -> Self {
        PrettyFormatter {
            max_code_width,
            default_indent,
        }
    }

    pub fn prepare<T>(&self, pe: PrettyExpr<T>) -> PrettyExpr<T> {
        match pe {
            PrettyExpr::Atom(x) => PrettyExpr::Atom(x),
            PrettyExpr::Stat(x) => PrettyExpr::Stat(x),
            PrettyExpr::Inline(_) if pe.inline_width() <= self.max_code_width => pe,
            PrettyExpr::Inline(xs) => {
                PrettyExpr::Expand(xs.into_iter().map(|x| self.prepare(x)).collect())
            }
            PrettyExpr::Expand(_) => pe,
            PrettyExpr::Style(s, x) => PrettyExpr::styled(s, self.prepare(*x)),
        }
    }

    pub fn pretty<T>(&self, pe: PrettyExpr<T>) -> Pretty<T> {
        Pretty {
            pf: *self,
            pe: self.prepare(pe),
        }
    }

    fn write<T, F: Formatter<T>>(
        &self,
        pe: &PrettyExpr<T>,
        indent_level: usize,
        f: &mut F,
    ) -> Result<(), F::Error> {
        match pe {
            PrettyExpr::Atom(x) => f.write(x),
            PrettyExpr::Stat(x) => f.write(x),
            PrettyExpr::Inline(xs) => self.write_inline(xs, f),
            PrettyExpr::Expand(xs) => self.write_expanded(xs, indent_level, f),
            PrettyExpr::Style(s, x) => {
                f.save_style();
                f.set_style(s);
                self.write(x, indent_level, f)?;
                f.restore_style();
                Ok(())
            }
        }
    }

    fn write_inline<T, F: Formatter<T>>(
        &self,
        xs: &[PrettyExpr<T>],
        f: &mut F,
    ) -> Result<(), F::Error> {
        f.write("(")?;
        match &xs[..] {
            [] => {}
            [x] => self.write(x, 0, f)?,
            [x, ys @ ..] => {
                self.write(x, 0, f)?;
                for y in ys {
                    f.write(" ")?;
                    self.write(y, 0, f)?;
                }
            }
        }
        f.write(")")
    }

    fn write_expanded<T, F: Formatter<T>>(
        &self,
        xs: &[PrettyExpr<T>],
        mut indent_level: usize,
        f: &mut F,
    ) -> Result<(), F::Error> {
        f.write("(")?;
        match &xs[..] {
            [] => {}
            [x] => self.write(x, indent_level, f)?,
            [x, ys @ ..] => {
                if x.is_atom() {
                    indent_level += self.default_indent;
                } else {
                    indent_level += 1;
                }
                self.write(x, indent_level, f)?;
                for y in ys {
                    f.write_indent(indent_level)?;
                    self.write(y, indent_level, f)?;
                }
            }
        }
        f.write(")")
    }
}

impl<T: Clone> std::fmt::Display for PrettyExpr<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let pf = PrettyFormatter::default();
        let pe = pf.prepare(self.clone());
        let mut df = DisplayFormatter::new(f);
        pf.write(&pe, 0, &mut df)
    }
}

impl<T: Clone> std::fmt::Display for Pretty<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let pe = self.pf.prepare(self.pe.clone());
        let mut df = DisplayFormatter::new(f);
        self.pf.write(&pe, 0, &mut df)
    }
}

pub trait Formatter<S> {
    type Error;

    fn write(&mut self, x: impl std::fmt::Display) -> std::result::Result<(), Self::Error>;
    fn set_style(&mut self, style: &S);
    fn save_style(&mut self);
    fn restore_style(&mut self);

    fn write_newline(&mut self) -> std::result::Result<(), Self::Error> {
        self.write("\n")
    }

    fn write_indent(&mut self, level: usize) -> std::result::Result<(), Self::Error> {
        self.write_newline()?;
        self.write(" ".repeat(level))
    }
}

struct DisplayFormatter<'a, 'b, S> {
    f: &'a mut std::fmt::Formatter<'b>,
    _p: PhantomData<S>,
}

impl<'a, 'b, S> DisplayFormatter<'a, 'b, S> {
    pub fn new(f: &'a mut std::fmt::Formatter<'b>) -> Self {
        DisplayFormatter { f, _p: PhantomData }
    }
}

impl<S> Formatter<S> for DisplayFormatter<'_, '_, S> {
    type Error = std::fmt::Error;

    fn write(&mut self, x: impl std::fmt::Display) -> std::result::Result<(), Self::Error> {
        write!(self.f, "{}", x)
    }

    fn set_style(&mut self, _style: &S) {}
    fn save_style(&mut self) {}
    fn restore_style(&mut self) {}
}

#[test]
fn tests() {
    macro_rules! p {
        ($($x:tt)*) => {pe!($($x)*) as PrettyExpr::<()>};
    }

    assert_eq!(p!["abc"].inline_width(), 3);
    assert_eq!(p![abcde].inline_width(), 5);
    assert_eq!(p![()].inline_width(), 2);
    assert_eq!(p![(abc)].inline_width(), 5);
    assert_eq!(p![(a b c)].inline_width(), 7);
    assert_eq!(p![(let ((a 1) (b 2)) ("+" a b))].inline_width(), 27);

    let pf = PrettyFormatter::new(15, 2);

    assert_eq!(pf.pretty(p!["abc"]).to_string(), "abc");
    assert_eq!(pf.pretty(p![(if q a e)]).to_string(), "(if q a e)");
    assert_eq!(
        pf.pretty(p![(branchon question answer else)]).to_string(),
        "(branchon\n  question\n  answer\n  else)"
    );
    assert_eq!(
        pf.pretty(p![(branchon (a b) (c d) (e f))]).to_string(),
        "(branchon\n  (a b)\n  (c d)\n  (e f))"
    );
    assert_eq!(
        pf.pretty(p![(long_name (other_long_name (if q a e)))])
            .to_string(),
        "(long_name\n  (other_long_name\n    (if q a e)))"
    );
    assert_eq!(
        pf.pretty(p![(let ((a 1) (b 2) (c 3)) ("+" a b))])
            .to_string(),
        "(let\n  ((a 1)\n   (b 2)\n   (c 3))\n  (+ a b))"
    );

    println!("{}", p![(let ((a 1) (b 2) (c 3)) ("+" a b))]);
}