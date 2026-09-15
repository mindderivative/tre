//! §16.2's binding expression grammar and the `BindingResolver`
//! inversion: `{{ clicks }}`-style expressions parsed against "a small,
//! whitelisted grammar (attribute access, indexing, comparison,
//! arithmetic, boolean logic, zero-arg method calls like
//! `clicks.get()`) -- deliberately not a path to arbitrary code
//! execution."
//!
//! `engine-spec` can't evaluate an expression against a Python
//! `ViewModel` itself -- no `pyo3` dependency, per this crate's own
//! boundary rule (§16.1) -- so `BindingResolver` is the same
//! dependency-inversion shape as `AppHandler` (§4) and the completion-
//! queue mechanism (§5): a lower crate defines the capability, a higher
//! one (`engine-py`) supplies it. Tested here with a small fake
//! resolver over plain Rust primitives, proving the grammar/evaluator
//! standalone before `engine-py` ever implements the real one.
//!
//! A real Python value that `ident`/`attr`/`index`/`call` returns is
//! eagerly unwrapped into a primitive `Value` (`Int`/`Float`/`Str`/
//! `Bool`) whenever it actually is one -- which covers the large
//! majority of real bindings (`clicks.get()` returning a plain int,
//! `user.name` returning a plain string). `Value::Handle` exists only
//! for chaining through a *non*-primitive Python object (`user.profile.
//! name`, where `profile` is itself an object) -- `binary_op`/`truthy`
//! are the resolver's hooks for the rarer case a `Handle` needs
//! arithmetic/comparison/truthiness (e.g. a numeric-like Python object
//! that isn't already an `int`/`float`).

use std::fmt;

/// The value an expression evaluates to, or an intermediate value while
/// walking a chain of `.attr`/`[index]`/`.method()` postfix operations.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    /// An opaque, resolver-defined handle to a live non-primitive
    /// object (e.g. a nested Python object) -- `engine-spec` never
    /// inspects this itself, only threads it back into the resolver.
    Handle(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

/// The whitelisted expression AST -- every variant corresponds to one
/// of §16.2's named operations, nothing more (no assignment, no
/// arbitrary function calls, no argument lists).
#[derive(Clone, Debug, PartialEq)]
pub enum Expression {
    Ident(String),
    Literal(Value),
    Attr(Box<Expression>, String),
    Index(Box<Expression>, Box<Expression>),
    /// A zero-arg method call -- `Call(receiver, method_name)`.
    Call(Box<Expression>, String),
    Not(Box<Expression>),
    BinaryOp(Box<Expression>, BinOp, Box<Expression>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExpressionError {
    /// The raw binding string wasn't wrapped in `{{ ... }}` at all.
    NotABinding(String),
    Parse(String),
}

impl fmt::Display for ExpressionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotABinding(raw) => {
                write!(f, "binding {raw:?} is not wrapped in {{{{ ... }}}}")
            }
            Self::Parse(msg) => write!(f, "failed to parse binding expression: {msg}"),
        }
    }
}

impl std::error::Error for ExpressionError {}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolveError(pub String);

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ResolveError {}

/// The capability only `engine-py` can supply (real GIL access) --
/// `engine-spec`'s own evaluator drives an expression purely through
/// these methods, with zero knowledge of what's actually behind a
/// `Value::Handle`.
pub trait BindingResolver {
    /// Resolves a bare identifier against the active `ViewModel`.
    fn ident(&self, name: &str) -> Result<Value, ResolveError>;
    fn attr(&self, base: &Value, name: &str) -> Result<Value, ResolveError>;
    fn index(&self, base: &Value, index: &Value) -> Result<Value, ResolveError>;
    /// A zero-arg method call, per §16.2's own stated grammar limit.
    fn call(&self, base: &Value, method: &str) -> Result<Value, ResolveError>;
    /// Only invoked when at least one side is a `Value::Handle` --
    /// `evaluate` computes primitive-only arithmetic/comparisons
    /// directly itself (§16.2's grammar doesn't need Python for
    /// `1 + 2`).
    fn binary_op(&self, op: BinOp, left: &Value, right: &Value) -> Result<Value, ResolveError>;
    /// Truthiness of a `Value::Handle`, for `and`/`or`/`not`. Never
    /// called for a primitive -- `evaluate` already knows how to judge
    /// those itself.
    fn truthy(&self, value: &Value) -> Result<bool, ResolveError>;
}

/// Parses a full binding string (`"{{ clicks.get() }}"`) into an
/// [`Expression`] -- the `{{`/`}}` delimiters are part of this
/// function's own contract, not the grammar itself, matching every
/// binding value's real shape in a `view.yaml`.
pub fn parse_binding(raw: &str) -> Result<Expression, ExpressionError> {
    let trimmed = raw.trim();
    let inner = trimmed
        .strip_prefix("{{")
        .and_then(|s| s.strip_suffix("}}"))
        .ok_or_else(|| ExpressionError::NotABinding(raw.to_string()))?;

    let tokens = lex(inner).map_err(ExpressionError::Parse)?;
    let mut parser = Parser {
        tokens: &tokens,
        pos: 0,
    };
    let expr = parser.parse_or().map_err(ExpressionError::Parse)?;
    if parser.pos != tokens.len() {
        return Err(ExpressionError::Parse(format!(
            "unexpected trailing input at token {}",
            parser.pos
        )));
    }
    Ok(expr)
}

/// Evaluates `expr` against `resolver`. Primitive-only binary
/// operations (both operands already `Int`/`Float`/`Str`/`Bool`) are
/// computed directly here, without asking the resolver anything --
/// only an operation touching a `Value::Handle` delegates to
/// [`BindingResolver::binary_op`].
pub fn evaluate(expr: &Expression, resolver: &dyn BindingResolver) -> Result<Value, ResolveError> {
    match expr {
        Expression::Ident(name) => resolver.ident(name),
        Expression::Literal(value) => Ok(value.clone()),
        Expression::Attr(base, name) => {
            let base = evaluate(base, resolver)?;
            resolver.attr(&base, name)
        }
        Expression::Index(base, index) => {
            let base = evaluate(base, resolver)?;
            let index = evaluate(index, resolver)?;
            resolver.index(&base, &index)
        }
        Expression::Call(base, method) => {
            let base = evaluate(base, resolver)?;
            resolver.call(&base, method)
        }
        Expression::Not(inner) => {
            let value = evaluate(inner, resolver)?;
            Ok(Value::Bool(!truthy(&value, resolver)?))
        }
        Expression::BinaryOp(left, BinOp::And, right) => {
            let left_value = evaluate(left, resolver)?;
            if !truthy(&left_value, resolver)? {
                return Ok(left_value);
            }
            evaluate(right, resolver)
        }
        Expression::BinaryOp(left, BinOp::Or, right) => {
            let left_value = evaluate(left, resolver)?;
            if truthy(&left_value, resolver)? {
                return Ok(left_value);
            }
            evaluate(right, resolver)
        }
        Expression::BinaryOp(left, op, right) => {
            let left = evaluate(left, resolver)?;
            let right = evaluate(right, resolver)?;
            binary_op(*op, &left, &right, resolver)
        }
    }
}

fn truthy(value: &Value, resolver: &dyn BindingResolver) -> Result<bool, ResolveError> {
    match value {
        Value::Bool(b) => Ok(*b),
        Value::Int(i) => Ok(*i != 0),
        Value::Float(f) => Ok(*f != 0.0),
        Value::Str(s) => Ok(!s.is_empty()),
        Value::Handle(_) => resolver.truthy(value),
    }
}

fn binary_op(
    op: BinOp,
    left: &Value,
    right: &Value,
    resolver: &dyn BindingResolver,
) -> Result<Value, ResolveError> {
    use Value::{Bool, Float, Handle, Int, Str};

    if matches!(left, Handle(_)) || matches!(right, Handle(_)) {
        return resolver.binary_op(op, left, right);
    }

    match (op, left, right) {
        (BinOp::Add, Int(a), Int(b)) => Ok(Int(a + b)),
        (BinOp::Add, Float(a), Float(b)) => Ok(Float(a + b)),
        (BinOp::Add, Int(a), Float(b)) | (BinOp::Add, Float(b), Int(a)) => Ok(Float(*a as f64 + b)),
        (BinOp::Add, Str(a), Str(b)) => Ok(Str(format!("{a}{b}"))),
        (BinOp::Sub, Int(a), Int(b)) => Ok(Int(a - b)),
        (BinOp::Sub, Float(a), Float(b)) => Ok(Float(a - b)),
        (BinOp::Mul, Int(a), Int(b)) => Ok(Int(a * b)),
        (BinOp::Mul, Float(a), Float(b)) => Ok(Float(a * b)),
        (BinOp::Div, Int(a), Int(b)) if *b != 0 => Ok(Float(*a as f64 / *b as f64)),
        (BinOp::Div, Float(a), Float(b)) => Ok(Float(a / b)),
        (BinOp::Eq, a, b) => Ok(Bool(a == b)),
        (BinOp::Ne, a, b) => Ok(Bool(a != b)),
        (BinOp::Lt, Int(a), Int(b)) => Ok(Bool(a < b)),
        (BinOp::Le, Int(a), Int(b)) => Ok(Bool(a <= b)),
        (BinOp::Gt, Int(a), Int(b)) => Ok(Bool(a > b)),
        (BinOp::Ge, Int(a), Int(b)) => Ok(Bool(a >= b)),
        (BinOp::Lt, Float(a), Float(b)) => Ok(Bool(a < b)),
        (BinOp::Le, Float(a), Float(b)) => Ok(Bool(a <= b)),
        (BinOp::Gt, Float(a), Float(b)) => Ok(Bool(a > b)),
        (BinOp::Ge, Float(a), Float(b)) => Ok(Bool(a >= b)),
        _ => Err(ResolveError(format!(
            "unsupported operation {op:?} between {left:?} and {right:?}"
        ))),
    }
}

// --- Lexer -----------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Ident(String),
    Int(i64),
    Float(f64),
    Str(String),
    Dot,
    Comma,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Plus,
    Minus,
    Star,
    Slash,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
    True,
    False,
}

fn lex(input: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            if text.contains('.') {
                tokens.push(Token::Float(
                    text.parse()
                        .map_err(|_| format!("invalid number {text:?}"))?,
                ));
            } else {
                tokens.push(Token::Int(
                    text.parse()
                        .map_err(|_| format!("invalid number {text:?}"))?,
                ));
            }
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            tokens.push(match word.as_str() {
                "and" => Token::And,
                "or" => Token::Or,
                "not" => Token::Not,
                "True" => Token::True,
                "False" => Token::False,
                _ => Token::Ident(word),
            });
        } else if c == '"' || c == '\'' {
            let quote = c;
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != quote {
                i += 1;
            }
            if i >= chars.len() {
                return Err("unterminated string literal".to_string());
            }
            tokens.push(Token::Str(chars[start..i].iter().collect()));
            i += 1; // consume closing quote
        } else {
            let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
            match two.as_str() {
                "==" => {
                    tokens.push(Token::Eq);
                    i += 2;
                    continue;
                }
                "!=" => {
                    tokens.push(Token::Ne);
                    i += 2;
                    continue;
                }
                "<=" => {
                    tokens.push(Token::Le);
                    i += 2;
                    continue;
                }
                ">=" => {
                    tokens.push(Token::Ge);
                    i += 2;
                    continue;
                }
                _ => {}
            }
            let token = match c {
                '.' => Token::Dot,
                ',' => Token::Comma,
                '(' => Token::LParen,
                ')' => Token::RParen,
                '[' => Token::LBracket,
                ']' => Token::RBracket,
                '+' => Token::Plus,
                '-' => Token::Minus,
                '*' => Token::Star,
                '/' => Token::Slash,
                '<' => Token::Lt,
                '>' => Token::Gt,
                other => return Err(format!("unexpected character {other:?}")),
            };
            tokens.push(token);
            i += 1;
        }
    }

    Ok(tokens)
}

// --- Recursive-descent parser -----------------------------------------

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        self.pos += 1;
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        match self.advance() {
            Some(tok) if tok == expected => Ok(()),
            other => Err(format!("expected {expected:?}, got {other:?}")),
        }
    }

    fn parse_or(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_and()?;
        while self.peek() == Some(&Token::Or) {
            self.advance();
            let right = self.parse_and()?;
            left = Expression::BinaryOp(Box::new(left), BinOp::Or, Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_not()?;
        while self.peek() == Some(&Token::And) {
            self.advance();
            let right = self.parse_not()?;
            left = Expression::BinaryOp(Box::new(left), BinOp::And, Box::new(right));
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Expression, String> {
        if self.peek() == Some(&Token::Not) {
            self.advance();
            return Ok(Expression::Not(Box::new(self.parse_not()?)));
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expression, String> {
        let left = self.parse_additive()?;
        let op = match self.peek() {
            Some(Token::Eq) => Some(BinOp::Eq),
            Some(Token::Ne) => Some(BinOp::Ne),
            Some(Token::Lt) => Some(BinOp::Lt),
            Some(Token::Le) => Some(BinOp::Le),
            Some(Token::Gt) => Some(BinOp::Gt),
            Some(Token::Ge) => Some(BinOp::Ge),
            _ => None,
        };
        let Some(op) = op else {
            return Ok(left);
        };
        self.advance();
        let right = self.parse_additive()?;
        Ok(Expression::BinaryOp(Box::new(left), op, Box::new(right)))
    }

    fn parse_additive(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinOp::Add,
                Some(Token::Minus) => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Expression::BinaryOp(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_postfix()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => BinOp::Mul,
                Some(Token::Slash) => BinOp::Div,
                _ => break,
            };
            self.advance();
            let right = self.parse_postfix()?;
            left = Expression::BinaryOp(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn parse_postfix(&mut self) -> Result<Expression, String> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek() {
                Some(Token::Dot) => {
                    self.advance();
                    let name = match self.advance() {
                        Some(Token::Ident(name)) => name.clone(),
                        other => {
                            return Err(format!("expected identifier after '.', got {other:?}"));
                        }
                    };
                    if self.peek() == Some(&Token::LParen) {
                        self.advance();
                        self.expect(&Token::RParen)?;
                        expr = Expression::Call(Box::new(expr), name);
                    } else {
                        expr = Expression::Attr(Box::new(expr), name);
                    }
                }
                Some(Token::LBracket) => {
                    self.advance();
                    let index = self.parse_or()?;
                    self.expect(&Token::RBracket)?;
                    expr = Expression::Index(Box::new(expr), Box::new(index));
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expression, String> {
        match self.advance() {
            Some(Token::Ident(name)) => Ok(Expression::Ident(name.clone())),
            Some(Token::Int(n)) => Ok(Expression::Literal(Value::Int(*n))),
            Some(Token::Float(n)) => Ok(Expression::Literal(Value::Float(*n))),
            Some(Token::Str(s)) => Ok(Expression::Literal(Value::Str(s.clone()))),
            Some(Token::True) => Ok(Expression::Literal(Value::Bool(true))),
            Some(Token::False) => Ok(Expression::Literal(Value::Bool(false))),
            Some(Token::LParen) => {
                let expr = self.parse_or()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            other => Err(format!("unexpected token {other:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// A small fake resolver over a flat `HashMap<String, Value>` --
    /// proves the grammar/evaluator standalone, with zero `pyo3`
    /// involvement, per this module's own doc comment.
    struct FakeResolver {
        vars: HashMap<String, Value>,
        calls: RefCell<Vec<String>>,
    }

    impl BindingResolver for FakeResolver {
        fn ident(&self, name: &str) -> Result<Value, ResolveError> {
            self.vars
                .get(name)
                .cloned()
                .ok_or_else(|| ResolveError(format!("unknown identifier {name:?}")))
        }
        fn attr(&self, _base: &Value, name: &str) -> Result<Value, ResolveError> {
            Err(ResolveError(format!(
                "FakeResolver has no attrs, got {name:?}"
            )))
        }
        fn index(&self, base: &Value, index: &Value) -> Result<Value, ResolveError> {
            if let (Value::Str(s), Value::Int(i)) = (base, index) {
                let ch = s
                    .chars()
                    .nth(*i as usize)
                    .ok_or_else(|| ResolveError("index out of range".to_string()))?;
                return Ok(Value::Str(ch.to_string()));
            }
            Err(ResolveError("unsupported index".to_string()))
        }
        fn call(&self, base: &Value, method: &str) -> Result<Value, ResolveError> {
            self.calls.borrow_mut().push(method.to_string());
            match (base, method) {
                (Value::Int(n), "double") => Ok(Value::Int(n * 2)),
                _ => Err(ResolveError(format!("no method {method:?}"))),
            }
        }
        fn binary_op(&self, op: BinOp, left: &Value, right: &Value) -> Result<Value, ResolveError> {
            Err(ResolveError(format!(
                "no Handle binary_op needed: {op:?} {left:?} {right:?}"
            )))
        }
        fn truthy(&self, value: &Value) -> Result<bool, ResolveError> {
            Err(ResolveError(format!("no Handle truthy needed: {value:?}")))
        }
    }

    fn eval(raw: &str, resolver: &FakeResolver) -> Value {
        let expr = parse_binding(raw).expect("valid binding must parse");
        evaluate(&expr, resolver).expect("evaluation must succeed")
    }

    #[test]
    fn bare_identifier_resolves_through_the_resolver() {
        let resolver = FakeResolver {
            vars: HashMap::from([("clicks".to_string(), Value::Int(3))]),
            calls: RefCell::new(Vec::new()),
        };
        assert_eq!(eval("{{ clicks }}", &resolver), Value::Int(3));
    }

    #[test]
    fn zero_arg_method_call_reaches_the_resolver() {
        let resolver = FakeResolver {
            vars: HashMap::from([("clicks".to_string(), Value::Int(21))]),
            calls: RefCell::new(Vec::new()),
        };
        assert_eq!(eval("{{ clicks.double() }}", &resolver), Value::Int(42));
        assert_eq!(resolver.calls.borrow().as_slice(), ["double"]);
    }

    #[test]
    fn arithmetic_and_comparison_on_primitives_need_no_resolver_call() {
        let resolver = FakeResolver {
            vars: HashMap::from([("clicks".to_string(), Value::Int(5))]),
            calls: RefCell::new(Vec::new()),
        };
        assert_eq!(eval("{{ clicks + 1 }}", &resolver), Value::Int(6));
        assert_eq!(eval("{{ clicks * 2 - 3 }}", &resolver), Value::Int(7));
        assert_eq!(eval("{{ clicks > 3 }}", &resolver), Value::Bool(true));
        assert_eq!(eval("{{ clicks == 5 }}", &resolver), Value::Bool(true));
    }

    #[test]
    fn boolean_logic_short_circuits() {
        let resolver = FakeResolver {
            vars: HashMap::from([
                ("a".to_string(), Value::Bool(false)),
                ("b".to_string(), Value::Bool(true)),
            ]),
            calls: RefCell::new(Vec::new()),
        };
        assert_eq!(eval("{{ a and b }}", &resolver), Value::Bool(false));
        assert_eq!(eval("{{ a or b }}", &resolver), Value::Bool(true));
        assert_eq!(eval("{{ not a }}", &resolver), Value::Bool(true));
    }

    #[test]
    fn indexing_reaches_the_resolver() {
        let resolver = FakeResolver {
            vars: HashMap::from([("name".to_string(), Value::Str("Ada".to_string()))]),
            calls: RefCell::new(Vec::new()),
        };
        assert_eq!(
            eval("{{ name[0] }}", &resolver),
            Value::Str("A".to_string())
        );
    }

    #[test]
    fn a_string_not_wrapped_in_double_braces_is_rejected() {
        let err = parse_binding("clicks").expect_err("a bare string isn't a binding");
        assert!(matches!(err, ExpressionError::NotABinding(_)));
    }

    #[test]
    fn a_malformed_expression_is_a_clear_parse_error_not_a_panic() {
        let err =
            parse_binding("{{ clicks + }}").expect_err("dangling operator must fail to parse");
        assert!(matches!(err, ExpressionError::Parse(_)));
    }

    #[test]
    fn unknown_identifier_is_a_clear_resolve_error() {
        let resolver = FakeResolver {
            vars: HashMap::new(),
            calls: RefCell::new(Vec::new()),
        };
        let expr = parse_binding("{{ nope }}").unwrap();
        let err = evaluate(&expr, &resolver).expect_err("an unknown identifier must fail cleanly");
        assert!(err.0.contains("nope"));
    }
}
