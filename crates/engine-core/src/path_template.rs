//! #272 (TRE-REQ-013 step 4): a path written once with named parameters, which the engine
//! evaluates into plain path data each time a parameter moves.
//!
//! A template is SVG path data in which any number may be a hole, `{expression}`:
//!
//! ```text
//! M {r},0 H {w - r} A {r} {r} 0 0 1 {w},{r} ...
//! ```
//!
//! An expression is numbers, parameter names, `+`, `-`, `*`, unary signs and parentheses, and
//! nothing else: no division, no functions, no conditions. Evaluating it substitutes the numbers
//! into the text and parses that as path data, so the result is exactly what the same data
//! written out would be.

use std::fmt::Write as _;

use crate::path::PathData;

#[derive(Clone, Debug, PartialEq)]
enum Expr {
    Number(f64),
    Param(String),
    Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}

impl Expr {
    fn eval(&self, lookup: &dyn Fn(&str) -> Option<f64>) -> Result<f64, String> {
        Ok(match self {
            Self::Number(n) => *n,
            Self::Param(name) => lookup(name).ok_or_else(|| format!("no parameter `{name}`"))?,
            Self::Neg(e) => -e.eval(lookup)?,
            Self::Add(a, b) => a.eval(lookup)? + b.eval(lookup)?,
            Self::Sub(a, b) => a.eval(lookup)? - b.eval(lookup)?,
            Self::Mul(a, b) => a.eval(lookup)? * b.eval(lookup)?,
        })
    }

    fn names<'a>(&'a self, out: &mut Vec<&'a str>) {
        match self {
            Self::Number(_) => {}
            Self::Param(name) => out.push(name),
            Self::Neg(e) => e.names(out),
            Self::Add(a, b) | Self::Sub(a, b) | Self::Mul(a, b) => {
                a.names(out);
                b.names(out);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Part {
    Text(String),
    Hole(Expr),
}

/// Path data with holes. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub struct PathTemplate {
    source: String,
    parts: Vec<Part>,
}

impl PathTemplate {
    /// Whether `text` has a hole, so is meant as a template and not as plain path data.
    pub fn is_template(text: &str) -> bool {
        text.contains('{')
    }

    /// Parses a template: every `{` closes with a `}` and holds an expression. Whether the text
    /// around the holes is path data is checked by [`PathTemplate::evaluate`].
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut parts = Vec::new();
        let mut rest = text;
        while let Some(open) = rest.find('{') {
            if open > 0 {
                parts.push(Part::Text(rest[..open].to_owned()));
            }
            let after = &rest[open + 1..];
            let close = after
                .find('}')
                .ok_or_else(|| "a `{` is never closed with a `}`".to_owned())?;
            if after[..close].contains('{') {
                return Err("holes do not nest: a `{` inside a `{...}`".into());
            }
            parts.push(Part::Hole(parse_expr(&after[..close])?));
            rest = &after[close + 1..];
        }
        if rest.contains('}') {
            return Err("a `}` with no `{` before it".into());
        }
        if !rest.is_empty() {
            parts.push(Part::Text(rest.to_owned()));
        }
        Ok(Self {
            source: text.to_owned(),
            parts,
        })
    }

    /// The template as written.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The parameter names it uses, each once, in order of first use.
    pub fn names(&self) -> Vec<&str> {
        let mut all = Vec::new();
        for part in &self.parts {
            if let Part::Hole(expr) = part {
                expr.names(&mut all);
            }
        }
        let mut seen = Vec::new();
        for name in all {
            if !seen.contains(&name) {
                seen.push(name);
            }
        }
        seen
    }

    /// The path data with every hole replaced by its value under `lookup`. A name `lookup` does
    /// not know, a value that is not finite, or text that is not path data is an error.
    pub fn evaluate(&self, lookup: &dyn Fn(&str) -> Option<f64>) -> Result<PathData, String> {
        let mut text = String::with_capacity(self.source.len());
        for part in &self.parts {
            match part {
                Part::Text(t) => text.push_str(t),
                Part::Hole(expr) => {
                    let value = expr.eval(lookup)?;
                    if !value.is_finite() {
                        return Err("an expression is not a finite number".into());
                    }
                    // Spaces keep a negative value from fusing with what is next to it.
                    let _ = write!(text, " {value} ");
                }
            }
        }
        PathData::from_svg(&text).map_err(|e| format!("isn't valid path data once filled in: {e}"))
    }
}

struct Parser<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

fn parse_expr(text: &str) -> Result<Expr, String> {
    let mut parser = Parser {
        chars: text.chars().peekable(),
    };
    let expr = parser.sum()?;
    parser.skip_space();
    match parser.chars.next() {
        None => Ok(expr),
        Some(c) => Err(format!(
            "`{c}` is not allowed in `{{{text}}}`: only numbers, names, + - * and parentheses"
        )),
    }
}

impl Parser<'_> {
    fn skip_space(&mut self) {
        while self.chars.peek().is_some_and(|c| c.is_whitespace()) {
            self.chars.next();
        }
    }

    fn sum(&mut self) -> Result<Expr, String> {
        let mut left = self.product()?;
        loop {
            self.skip_space();
            match self.chars.peek() {
                Some('+') => {
                    self.chars.next();
                    left = Expr::Add(Box::new(left), Box::new(self.product()?));
                }
                Some('-') => {
                    self.chars.next();
                    left = Expr::Sub(Box::new(left), Box::new(self.product()?));
                }
                _ => return Ok(left),
            }
        }
    }

    fn product(&mut self) -> Result<Expr, String> {
        let mut left = self.factor()?;
        loop {
            self.skip_space();
            if self.chars.peek() == Some(&'*') {
                self.chars.next();
                left = Expr::Mul(Box::new(left), Box::new(self.factor()?));
            } else {
                return Ok(left);
            }
        }
    }

    fn factor(&mut self) -> Result<Expr, String> {
        self.skip_space();
        match self.chars.peek().copied() {
            Some('-') => {
                self.chars.next();
                Ok(Expr::Neg(Box::new(self.factor()?)))
            }
            Some('+') => {
                self.chars.next();
                self.factor()
            }
            Some('(') => {
                self.chars.next();
                let inner = self.sum()?;
                self.skip_space();
                if self.chars.next() != Some(')') {
                    return Err("a `(` is never closed".into());
                }
                Ok(inner)
            }
            Some(c) if c.is_ascii_digit() || c == '.' => {
                let mut number = String::new();
                while let Some(&c) = self.chars.peek() {
                    if c.is_ascii_digit() || c == '.' {
                        number.push(c);
                        self.chars.next();
                    } else {
                        break;
                    }
                }
                number
                    .parse()
                    .map(Expr::Number)
                    .map_err(|_| format!("`{number}` is not a number"))
            }
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                let mut name = String::new();
                while let Some(&c) = self.chars.peek() {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        name.push(c);
                        self.chars.next();
                    } else {
                        break;
                    }
                }
                Ok(Expr::Param(name))
            }
            Some(c) => Err(format!("unexpected `{c}` in an expression")),
            None => Err("an expression is empty or ends early".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(values: &'static [(&'static str, f64)]) -> impl Fn(&str) -> Option<f64> {
        move |name| values.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
    }

    #[test]
    fn a_filled_in_template_is_the_same_data_written_out() {
        let t = PathTemplate::parse("M0,0 H {w} V {h - 2 * r} L {r},{-r} Z").unwrap();
        let got = t
            .evaluate(&lookup(&[("w", 40.0), ("h", 30.0), ("r", 5.0)]))
            .unwrap();
        let want = PathData::from_svg("M0,0 H 40 V 20 L 5,-5 Z").unwrap();
        assert_eq!(got.0, want.0);
        assert_eq!(t.names(), ["w", "h", "r"]);
    }

    #[test]
    fn precedence_signs_and_parentheses() {
        let eval = |expr: &str| {
            let t = PathTemplate::parse(&format!("M0,0 H {{{expr}}}")).unwrap();
            let data = t.evaluate(&lookup(&[("a", 3.0), ("b", 4.0)])).unwrap();
            data.to_svg()
        };
        assert_eq!(eval("1 + 2 * 3"), "M0,0 L7,0");
        assert_eq!(eval("(1 + 2) * 3"), "M0,0 L9,0");
        assert_eq!(eval("-a - -b"), "M0,0 L1,0");
        assert_eq!(eval("a*b+.5"), "M0,0 L12.5,0");
    }

    #[test]
    fn a_negative_value_does_not_fuse_with_its_neighbour() {
        let t = PathTemplate::parse("M{x}{y} L{y}{x}").unwrap();
        let data = t.evaluate(&lookup(&[("x", -3.0), ("y", -4.0)])).unwrap();
        assert_eq!(data.0, PathData::from_svg("M-3 -4 L-4 -3").unwrap().0);
    }

    #[test]
    fn only_sums_products_numbers_and_names_are_evaluated() {
        for bad in [
            "M0,0 H {a / 2}",
            "M0,0 H {sqrt(a)}",
            "M0,0 H {a ? 1}",
            "M0,0 H {}",
            "M0,0 H {(a}",
        ] {
            assert!(PathTemplate::parse(bad).is_err(), "{bad}");
        }
        assert!(PathTemplate::parse("M0,0 H {a").is_err());
        assert!(PathTemplate::parse("M0,0 H a}").is_err());
        assert!(PathTemplate::parse("M0,0 H {{a}}").is_err());
    }

    #[test]
    fn a_missing_name_or_a_nonfinite_value_or_bad_data_is_an_error() {
        let t = PathTemplate::parse("M0,0 H {a}").unwrap();
        assert!(
            t.evaluate(&lookup(&[]))
                .unwrap_err()
                .contains("no parameter `a`")
        );
        assert!(t.evaluate(&lookup(&[("a", f64::INFINITY)])).is_err());
        let bad = PathTemplate::parse("X {a}").unwrap();
        assert!(bad.evaluate(&lookup(&[("a", 1.0)])).is_err());
    }
}
