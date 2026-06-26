/// jq filter language parser.
///
/// Grammar (simplified):
///   filter       := pipe
///   pipe         := compound ('|' compound)*
///   compound     := atom ('.' accessor | '[' expr ']' | '(' args ')')*
///   accessor     := IDENT | STRING
///   atom         := '.' | STRING | NUMBER | 'null' | 'true' | 'false'
///                  | '[' filter? ']' | '{' obj_expr '}'
///                  | IDENT '(' args ')' | '(' filter ')' | 'if' filter 'then' filter ('else' filter)? 'end'
///                  | 'try' filter ('catch' filter)?
///                  | 'reduce' filter 'as' IDENT '(' filter ';' filter ')'
///                  | 'map' '(' filter ')'
///                  | 'select' '(' filter ')'
///   obj_expr     := (STRING ':' filter (',' STRING ':' filter)*)
///   args         := filter (',' filter)*

use crate::value::JqValue;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Identity,                                    // .
    FieldAccess(String),                         // .field
    IndexAccess(Box<Expr>),                      // .[expr]
    Literal(JqValue),                            // "string", 42, null, true, false
    Pipe(Box<Expr>, Box<Expr>),                  // e1 | e2
    ArrayLiteral(Vec<Expr>),                     // [e1, e2, ...]
    ObjectLiteral(Vec<(Expr, Expr)>),            // {k: v, ...}
    IfThenElse(Box<Expr>, Box<Expr>, Option<Box<Expr>>), // if e1 then e2 [else e3] end
    TryCatch(Box<Expr>, Option<Box<Expr>>),      // try e1 [catch e2]
    FunctionCall(String, Vec<Expr>),             // func(args)
    Variable(String),                            // $var
    Iterate,                                     // .[]
    Select(Box<Expr>),                           // select(expr)
    Map(Box<Expr>),                              // map(expr)
    Reduce(Box<Expr>, String, Box<Expr>, Box<Expr>), // reduce expr as $var (init; update)
    GroupBy(Box<Expr>),                          // group_by(expr)
    SortBy(Box<Expr>),                           // sort_by(expr)
    MinBy(Box<Expr>),                            // min_by(expr)
    MaxBy(Box<Expr>),                            // max_by(expr)
    UnaryMinus(Box<Expr>),                       // -expr
    BinaryOp(BinaryOp, Box<Expr>, Box<Expr>),    // e1 op e2
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Neq, Lt, Lte, Gt, Gte,
    And, Or,
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub pos: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parse error at position {}: {}", self.pos, self.message)
    }
}

impl std::error::Error for ParseError {}

pub struct Parser {
    input: String,
    pos: usize,
}

impl Parser {
    pub fn new(input: &str) -> Self {
        Parser {
            input: input.to_string(),
            pos: 0,
        }
    }

    pub fn parse(&mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_pipe()?;
        self.skip_whitespace();
        if self.pos < self.input.len() {
            return Err(ParseError {
                message: format!("Unexpected character at position {}: '{}'", self.pos, self.char_at(self.pos)),
                pos: self.pos,
            });
        }
        Ok(expr)
    }

    fn parse_pipe(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_comparison()?;
        self.skip_whitespace();
        while self.pos < self.input.len() && self.char_at(self.pos) == '|' {
            self.pos += 1;
            let right = self.parse_comparison()?;
            left = Expr::Pipe(Box::new(left), Box::new(right));
            self.skip_whitespace();
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_additive()?;
        self.skip_whitespace();
        if self.pos < self.input.len() {
            let ch = self.char_at(self.pos);
            let op = match ch {
                '=' if self.pos + 1 < self.input.len() && self.char_at(self.pos + 1) == '=' => {
                    self.pos += 2;
                    BinaryOp::Eq
                }
                '!' if self.pos + 1 < self.input.len() && self.char_at(self.pos + 1) == '=' => {
                    self.pos += 2;
                    BinaryOp::Neq
                }
                '>' if self.pos + 1 < self.input.len() && self.char_at(self.pos + 1) == '=' => {
                    self.pos += 2;
                    BinaryOp::Gte
                }
                '<' if self.pos + 1 < self.input.len() && self.char_at(self.pos + 1) == '=' => {
                    self.pos += 2;
                    BinaryOp::Lte
                }
                '>' => {
                    self.pos += 1;
                    BinaryOp::Gt
                }
                '<' => {
                    self.pos += 1;
                    BinaryOp::Lt
                }
                _ => return Ok(left),
            };
            let right = self.parse_additive()?;
            left = Expr::BinaryOp(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_multiplicative()?;
        loop {
            self.skip_whitespace();
            if self.pos >= self.input.len() {
                break;
            }
            let ch = self.char_at(self.pos);
            let op = match ch {
                '+' => {
                    self.pos += 1;
                    BinaryOp::Add
                }
                '-' => {
                    self.pos += 1;
                    BinaryOp::Sub
                }
                _ => break,
            };
            let right = self.parse_multiplicative()?;
            left = Expr::BinaryOp(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and()?;
        loop {
            self.skip_whitespace();
            if self.pos >= self.input.len() {
                break;
            }
            let ch = self.char_at(self.pos);
            let op = match ch {
                '*' => {
                    self.pos += 1;
                    BinaryOp::Mul
                }
                '/' => {
                    self.pos += 1;
                    BinaryOp::Div
                }
                '%' => {
                    self.pos += 1;
                    BinaryOp::Mod
                }
                _ => break,
            };
            let right = self.parse_and()?;
            left = Expr::BinaryOp(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_or()?;
        loop {
            self.skip_whitespace();
            if self.match_word("and") {
                let right = self.parse_or()?;
                left = Expr::BinaryOp(BinaryOp::And, Box::new(left), Box::new(right));
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_compound()?;
        loop {
            self.skip_whitespace();
            if self.match_word("or") {
                let right = self.parse_compound()?;
                left = Expr::BinaryOp(BinaryOp::Or, Box::new(left), Box::new(right));
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_compound(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_atom()?;
        loop {
            self.skip_whitespace();
            if self.pos >= self.input.len() {
                break;
            }
            match self.char_at(self.pos) {
                '.' => {
                    self.pos += 1;
                    if self.pos >= self.input.len() {
                        // trailing dot means iterate
                        expr = Expr::Iterate;
                        break;
                    }
                    let ch = self.char_at(self.pos);
                    if ch == '[' {
                        // .[expr] or .[]
                        self.pos += 1;
                        if self.pos < self.input.len() && self.char_at(self.pos) == ']' {
                            self.pos += 1;
                            expr = Expr::Iterate;
                        } else {
                            let idx = self.parse_pipe()?;
                            self.expect(']')?;
                            let next = Expr::IndexAccess(Box::new(idx));
                            expr = if expr == Expr::Identity {
                                next
                            } else {
                                Expr::Pipe(Box::new(expr), Box::new(next))
                            };
                        }
                    } else if ch == '(' {
                        // skip, handled elsewhere
                        break;
                    } else if is_ident_start(ch) {
                        let name = self.parse_ident()?;
                        let next = Expr::FieldAccess(name);
                        expr = if expr == Expr::Identity {
                            next
                        } else {
                            Expr::Pipe(Box::new(expr), Box::new(next))
                        };
                    } else if ch == '.' {
                        // .. is identity of identity = identity
                        expr = Expr::Identity;
                    } else {
                        // trailing dot after expression, treat as field access with empty name
                        expr = Expr::FieldAccess(String::new());
                    }
                }
                '[' => {
                    self.pos += 1;
                    self.skip_whitespace();
                    if self.pos < self.input.len() && self.char_at(self.pos) == ']' {
                        self.pos += 1;
                        let next = Expr::Iterate;
                        expr = if expr == Expr::Identity {
                            next
                        } else {
                            Expr::Pipe(Box::new(expr), Box::new(next))
                        };
                    } else {
                        let idx = self.parse_pipe()?;
                        self.expect(']')?;
                        let next = Expr::IndexAccess(Box::new(idx));
                        expr = if expr == Expr::Identity {
                            next
                        } else {
                            Expr::Pipe(Box::new(expr), Box::new(next))
                        };
                    }
                }
                '(' => {
                    // function call
                    self.pos += 1;
                    let mut args = Vec::new();
                    self.skip_whitespace();
                    if self.pos < self.input.len() && self.char_at(self.pos) != ')' {
                        args.push(self.parse_pipe()?);
                        while self.match_char(',') {
                            args.push(self.parse_pipe()?);
                        }
                    }
                    self.expect(')')?;
                    if let Expr::FieldAccess(name) = &expr {
                        expr = Expr::FunctionCall(name.clone(), args);
                    } else if let Expr::Identity = &expr {
                        // (filter) - just parenthesized expression
                        expr = args.into_iter().next().unwrap_or(Expr::Identity);
                    } else {
                        // parenthesized expression
                        if args.is_empty() {
                            expr = Expr::Identity;
                        }
                    }
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_atom(&mut self) -> Result<Expr, ParseError> {
        self.skip_whitespace();
        if self.pos >= self.input.len() {
            return Err(ParseError {
                message: "Unexpected end of input".to_string(),
                pos: self.pos,
            });
        }

        let ch = self.char_at(self.pos);

        match ch {
            '.' => {
                self.pos += 1;
                // Check what follows
                if self.pos >= self.input.len() {
                    return Ok(Expr::Identity);
                }
                let next = self.char_at(self.pos);
                if next == '[' {
                    self.pos += 1;
                    // Handle .[] (empty brackets = iterate)
                    if self.pos < self.input.len() && self.char_at(self.pos) == ']' {
                        self.pos += 1;
                        return Ok(Expr::Iterate);
                    }
                    let idx = self.parse_pipe()?;
                    self.expect(']')?;
                    Ok(Expr::IndexAccess(Box::new(idx)))
                } else if next == '(' {
                    // It's .(expr) which is just (expr)
                    self.pos += 1;
                    let inner = self.parse_pipe()?;
                    self.expect(')')?;
                    Ok(inner)
                } else if is_ident_start(next) {
                    let name = self.parse_ident()?;
                    Ok(Expr::FieldAccess(name))
                } else if next == '.' {
                    self.pos += 1;
                    Ok(Expr::Identity)
                } else {
                    Ok(Expr::Identity)
                }
            }
            '"' | '\'' => {
                let s = self.parse_string()?;
                Ok(Expr::Literal(JqValue::String(s)))
            }
            '[' => self.parse_array_literal(),
            '{' => self.parse_object_literal(),
            '(' => {
                self.pos += 1;
                let expr = self.parse_pipe()?;
                self.expect(')')?;
                Ok(expr)
            }
            '-' if self.pos + 1 < self.input.len() && self.char_at(self.pos + 1).is_ascii_digit() => {
                self.pos += 1;
                let n = self.parse_number(false)?;
                Ok(Expr::Literal(JqValue::Number(n)))
            }
            '-' => {
                self.pos += 1;
                let expr = self.parse_atom()?;
                Ok(Expr::UnaryMinus(Box::new(expr)))
            }
            c if c.is_ascii_digit() => {
                let n = self.parse_number(true)?;
                Ok(Expr::Literal(JqValue::Number(n)))
            }
            '$' => {
                self.pos += 1;
                let name = self.parse_ident()?;
                Ok(Expr::Variable(name))
            }
            _ if is_ident_start(ch) => {
                let word = self.parse_ident()?;
                match word.as_str() {
                    "null" => Ok(Expr::Literal(JqValue::Null)),
                    "true" => Ok(Expr::Literal(JqValue::Bool(true))),
                    "false" => Ok(Expr::Literal(JqValue::Bool(false))),
                    "if" => {
                        let cond = self.parse_pipe()?;
                        self.skip_whitespace();
                        self.expect_word("then")?;
                        let then_branch = self.parse_pipe()?;
                        self.skip_whitespace();
                        let else_branch = if self.match_word("else") {
                            let e = self.parse_pipe()?;
                            Some(Box::new(e))
                        } else {
                            None
                        };
                        self.skip_whitespace();
                        self.expect_word("end")?;
                        Ok(Expr::IfThenElse(Box::new(cond), Box::new(then_branch), else_branch))
                    }
                    "try" => {
                        let body = self.parse_pipe()?;
                        self.skip_whitespace();
                        let catch = if self.match_word("catch") {
                            let c = self.parse_pipe()?;
                            Some(Box::new(c))
                        } else {
                            None
                        };
                        Ok(Expr::TryCatch(Box::new(body), catch))
                    }
                    "reduce" => {
                        let expr = self.parse_pipe()?;
                        self.skip_whitespace();
                        self.expect_word("as")?;
                        self.skip_whitespace();
                        // Variable name: skip leading $ if present
                        if self.pos < self.input.len() && self.char_at(self.pos) == '$' {
                            self.pos += 1;
                        }
                        let var = self.parse_ident()?;
                        self.skip_whitespace();
                        self.expect('(')?;
                        let init = self.parse_pipe()?;
                        self.expect(';')?;
                        let update = self.parse_pipe()?;
                        self.expect(')')?;
                        Ok(Expr::Reduce(Box::new(expr), var, Box::new(init), Box::new(update)))
                    }
                    "map" => {
                        self.expect('(')?;
                        let expr = self.parse_pipe()?;
                        self.expect(')')?;
                        Ok(Expr::Map(Box::new(expr)))
                    }
                    "select" => {
                        self.expect('(')?;
                        let expr = self.parse_pipe()?;
                        self.expect(')')?;
                        Ok(Expr::Select(Box::new(expr)))
                    }
                    "group_by" | "groupby" => {
                        self.expect('(')?;
                        let expr = self.parse_pipe()?;
                        self.expect(')')?;
                        Ok(Expr::GroupBy(Box::new(expr)))
                    }
                    "sort_by" | "sortby" => {
                        self.expect('(')?;
                        let expr = self.parse_pipe()?;
                        self.expect(')')?;
                        Ok(Expr::SortBy(Box::new(expr)))
                    }
                    "min_by" | "minby" => {
                        self.expect('(')?;
                        let expr = self.parse_pipe()?;
                        self.expect(')')?;
                        Ok(Expr::MinBy(Box::new(expr)))
                    }
                    "max_by" | "maxby" => {
                        self.expect('(')?;
                        let expr = self.parse_pipe()?;
                        self.expect(')')?;
                        Ok(Expr::MaxBy(Box::new(expr)))
                    }
                    _ => {
                        // Check if followed by ( for function call
                        self.skip_whitespace();
                        if self.pos < self.input.len() && self.char_at(self.pos) == '(' {
                            self.pos += 1;
                            let mut args = Vec::new();
                            self.skip_whitespace();
                            if self.pos < self.input.len() && self.char_at(self.pos) != ')' {
                                args.push(self.parse_pipe()?);
                                while self.match_char(',') {
                                    args.push(self.parse_pipe()?);
                                }
                            }
                            self.expect(')')?;
                            Ok(Expr::FunctionCall(word, args))
                        } else {
                            // Could be a variable reference or just identity
                            Ok(Expr::FunctionCall(word, vec![]))
                        }
                    }
                }
            }
            _ => Err(ParseError {
                message: format!("Unexpected character: '{}'", ch),
                pos: self.pos,
            }),
        }
    }

    fn parse_array_literal(&mut self) -> Result<Expr, ParseError> {
        self.pos += 1; // skip [
        self.skip_whitespace();
        let mut elements = Vec::new();
        if self.pos < self.input.len() && self.char_at(self.pos) != ']' {
            elements.push(self.parse_pipe()?);
            while self.match_char(',') {
                elements.push(self.parse_pipe()?);
            }
        }
        self.expect(']')?;
        Ok(Expr::ArrayLiteral(elements))
    }

    fn parse_object_key_expr(&mut self) -> Result<Expr, ParseError> {
        self.skip_whitespace();
        let ch = self.char_at(self.pos);
        if ch == '"' || ch == '\'' {
            let s = self.parse_string()?;
            Ok(Expr::Literal(JqValue::String(s)))
        } else if is_ident_start(ch) {
            let name = self.parse_ident()?;
            Ok(Expr::Literal(JqValue::String(name)))
        } else {
            self.parse_pipe()
        }
    }

    fn parse_object_literal(&mut self) -> Result<Expr, ParseError> {
        self.pos += 1; // skip {
        self.skip_whitespace();
        let mut pairs = Vec::new();
        if self.pos < self.input.len() && self.char_at(self.pos) != '}' {
            let key = self.parse_object_key_expr()?;
            self.expect(':')?;
            let val = self.parse_pipe()?;
            pairs.push((key, val));
            while self.match_char(',') {
                let key = self.parse_object_key_expr()?;
                self.expect(':')?;
                let val = self.parse_pipe()?;
                pairs.push((key, val));
            }
        }
        self.expect('}')?;
        Ok(Expr::ObjectLiteral(pairs))
    }

    fn parse_string(&mut self) -> Result<String, ParseError> {
        let quote = self.char_at(self.pos);
        self.pos += 1;
        let start = self.pos;
        let mut result = String::new();
        while self.pos < self.input.len() {
            let ch = self.char_at(self.pos);
            if ch == quote {
                result = self.input[start..self.pos].to_string();
                self.pos += 1;
                return Ok(result);
            }
            if ch == '\\' && quote == '"' {
                self.pos += 1;
                if self.pos < self.input.len() {
                    let escaped = self.char_at(self.pos);
                    match escaped {
                        'n' => result.push('\n'),
                        't' => result.push('\t'),
                        'r' => result.push('\r'),
                        '\\' => result.push('\\'),
                        '"' => result.push('"'),
                        _ => {
                            result.push('\\');
                            result.push(escaped);
                        }
                    }
                }
            } else {
                result.push(ch);
            }
            self.pos += 1;
        }
        Err(ParseError {
            message: "Unterminated string".to_string(),
            pos: start,
        })
    }

    fn parse_number(&mut self, positive: bool) -> Result<f64, ParseError> {
        let start = if positive { self.pos } else { self.pos - 1 };
        if positive {
            // start from current position
        } else {
            // already consumed '-', include it in the string
        }
        while self.pos < self.input.len() && self.char_at(self.pos).is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos < self.input.len() && self.char_at(self.pos) == '.' {
            self.pos += 1;
            while self.pos < self.input.len() && self.char_at(self.pos).is_ascii_digit() {
                self.pos += 1;
            }
        }
        if self.pos < self.input.len() && (self.char_at(self.pos) == 'e' || self.char_at(self.pos) == 'E') {
            self.pos += 1;
            if self.pos < self.input.len() && (self.char_at(self.pos) == '+' || self.char_at(self.pos) == '-') {
                self.pos += 1;
            }
            while self.pos < self.input.len() && self.char_at(self.pos).is_ascii_digit() {
                self.pos += 1;
            }
        }
        let num_str = &self.input[start..self.pos];
        num_str.parse::<f64>().map_err(|_| ParseError {
            message: format!("Invalid number: {}", num_str),
            pos: start,
        })
    }

    fn parse_ident(&mut self) -> Result<String, ParseError> {
        let start = self.pos;
        while self.pos < self.input.len() && is_ident_part(self.char_at(self.pos)) {
            self.pos += 1;
        }
        if self.pos == start {
            return Err(ParseError {
                message: "Expected identifier".to_string(),
                pos: self.pos,
            });
        }
        Ok(self.input[start..self.pos].to_string())
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() && self.char_at(self.pos).is_whitespace() {
            self.pos += 1;
        }
    }

    fn char_at(&self, pos: usize) -> char {
        self.input.chars().nth(pos).unwrap_or('\0')
    }

    fn match_char(&mut self, c: char) -> bool {
        self.skip_whitespace();
        if self.pos < self.input.len() && self.char_at(self.pos) == c {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, c: char) -> Result<(), ParseError> {
        if self.pos < self.input.len() && self.char_at(self.pos) == c {
            self.pos += 1;
            Ok(())
        } else {
            Err(ParseError {
                message: format!("Expected '{}'", c),
                pos: self.pos,
            })
        }
    }

    fn expect_word(&mut self, word: &str) -> Result<(), ParseError> {
        self.skip_whitespace();
        let after = self.pos + word.len();
        if self.input[self.pos..].starts_with(word) {
            self.pos = after;
            Ok(())
        } else {
            Err(ParseError {
                message: format!("Expected '{}'", word),
                pos: self.pos,
            })
        }
    }

    fn match_word(&mut self, word: &str) -> bool {
        self.skip_whitespace();
        let after = self.pos + word.len();
        if self.input[self.pos..].starts_with(word) {
            if after >= self.input.len() || !is_ident_part(self.char_at(after)) {
                self.pos = after;
                return true;
            }
        }
        false
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_part(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_identity() {
        let mut p = Parser::new(".");
        assert_eq!(p.parse().unwrap(), Expr::Identity);
    }

    #[test]
    fn test_parse_field_access() {
        let mut p = Parser::new(".foo");
        assert_eq!(p.parse().unwrap(), Expr::FieldAccess("foo".to_string()));
    }

    #[test]
    fn test_parse_pipe() {
        let mut p = Parser::new(".foo | .bar");
        let expr = p.parse().unwrap();
        match expr {
            Expr::Pipe(left, right) => {
                assert_eq!(*left, Expr::FieldAccess("foo".to_string()));
                assert_eq!(*right, Expr::FieldAccess("bar".to_string()));
            }
            _ => panic!("Expected Pipe expression"),
        }
    }

    #[test]
    fn test_parse_string() {
        let mut p = Parser::new("\"hello\"");
        assert_eq!(p.parse().unwrap(), Expr::Literal(JqValue::String("hello".to_string())));
    }

    #[test]
    fn test_parse_number() {
        let mut p = Parser::new("42");
        assert_eq!(p.parse().unwrap(), Expr::Literal(JqValue::Number(42.0)));
    }

    #[test]
    fn test_parse_array() {
        let mut p = Parser::new("[1, 2, 3]");
        let expr = p.parse().unwrap();
        match expr {
            Expr::ArrayLiteral(elements) => {
                assert_eq!(elements.len(), 3);
            }
            _ => panic!("Expected ArrayLiteral"),
        }
    }

    #[test]
    fn test_parse_select() {
        let mut p = Parser::new("select(. > 5)");
        let expr = p.parse().unwrap();
        match expr {
            Expr::Select(_) => {}
            _ => panic!("Expected Select expression"),
        }
    }

    #[test]
    fn test_parse_if_then_else() {
        let mut p = Parser::new("if . > 5 then \"big\" else \"small\" end");
        let expr = p.parse().unwrap();
        match expr {
            Expr::IfThenElse(_, _, Some(_)) => {}
            _ => panic!("Expected IfThenElse with else branch"),
        }
    }
}
