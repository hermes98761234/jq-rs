# jq-rs Missing Features Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the 6 missing jq features: `def` (user-defined functions with recursion), regex (`test`/`match`/`capture`), ANSI color output, I/O builtins (`inputs`/`input_filename`), streaming parser + `--stream` flag, and module system (`import`/`include`).

**Architecture:** All changes extend the existing 4-file tree-walking interpreter. `parser.rs` gets new AST nodes and grammar; `interpreter.rs` gets new `Context` fields and builtin dispatch; `main.rs` gets CLI flags and I/O changes. Tasks chain strictly — all touch the same 4 files.

**Tech Stack:** Rust 1.95, `serde_json`, `clap 4`, `anyhow`, `thiserror`. Add `regex = "1"` in Task 2.

## Global Constraints

- Work dir: `/home/user/projects/jq-rs`
- Minimum Rust version: 1.70 (project requirement). `rustc --version` → 1.95 installed.
- Run tests with: `cargo test 2>&1`
- Build check: `cargo build 2>&1`
- Push after every task: `git push origin master`
- Never run the binary interactively. Verification is `cargo test` + `cargo build` + `echo '...' | cargo run -- '...'` one-liners.
- Each task ends with a git commit + push.
- Do NOT re-implement already-working features. Only add what the task specifies.

---

### Task 1: `def` — User-Defined Functions with Recursion

**Files:**
- Modify: `src/parser.rs` — add `Def`/`Program` AST nodes, `parse_def()`, update `parse()` and function-call arg parsing to support `;` separator
- Modify: `src/interpreter.rs` — add `fns`/`filter_args` to `Context`, handle `Program`/`Def` in `run()`, dispatch user-defined functions in `call_function()`

**Interfaces:**
- Produces: `Expr::Def(String, Vec<String>, Box<Expr>)` and `Expr::Program(Vec<Expr>, Box<Expr>)` AST nodes
- Produces: `Context::fns: Vec<(String, Vec<String>, Expr)>` and `Context::filter_args: Vec<(String, Expr)>`
- Produces: `Context::push_fn(name, params, body)` and `Context::get_fn(name)`

- [ ] **Step 1: Write failing tests in `src/interpreter.rs`**

Add this test module at the bottom of `src/interpreter.rs` (after all existing code):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    fn run_filter(filter: &str, input: &str) -> Result<Vec<JqValue>, InterpreterError> {
        let mut p = Parser::new(filter);
        let expr = p.parse().map_err(|e| InterpreterError::new(e.to_string()))?;
        let val: serde_json::Value = serde_json::from_str(input).unwrap();
        let jv = JqValue::from(val);
        let interp = Interpreter::new();
        let mut ctx = Context::new();
        interp.run(&expr, &jv, &mut ctx)
    }

    #[test]
    fn test_def_no_args() {
        let result = run_filter("def double: . * 2; 5 | double", "null").unwrap();
        assert_eq!(result, vec![JqValue::Number(10.0)]);
    }

    #[test]
    fn test_def_with_arg() {
        let result = run_filter("def add(x): . + x; 5 | add(3)", "null").unwrap();
        assert_eq!(result, vec![JqValue::Number(8.0)]);
    }

    #[test]
    fn test_def_recursive() {
        let result = run_filter(
            "def fact: if . <= 1 then 1 else . * ((. - 1) | fact) end; 5 | fact",
            "null",
        )
        .unwrap();
        assert_eq!(result, vec![JqValue::Number(120.0)]);
    }

    #[test]
    fn test_def_filter_arg() {
        // def map2(f): [.[] | f]
        let result = run_filter("def map2(f): [.[] | f]; [1,2,3] | map2(. * 2)", "null").unwrap();
        assert_eq!(
            result,
            vec![JqValue::Array(vec![
                JqValue::Number(2.0),
                JqValue::Number(4.0),
                JqValue::Number(6.0)
            ])]
        );
    }

    #[test]
    fn test_def_multiple() {
        let result =
            run_filter("def double: . * 2; def triple: . * 3; 4 | double | triple", "null")
                .unwrap();
        assert_eq!(result, vec![JqValue::Number(24.0)]);
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cd /home/user/projects/jq-rs && cargo test test_def 2>&1
```

Expected output contains: `error[E0282]` or `error` (tests won't compile yet).

- [ ] **Step 3: Add `Def` and `Program` variants to `Expr` enum in `src/parser.rs`**

In `src/parser.rs`, find the `Expr` enum (starts at line ~20). Add two new variants at the end of the enum, before the closing `}`:

```rust
    Def(String, Vec<String>, Box<Expr>),     // def name(p1; p2): body
    Program(Vec<Expr>, Box<Expr>),            // [def...] followed by main expr
```

The full enum after the change (only showing the tail — do not remove existing variants):
```rust
    // ... all existing variants unchanged ...
    BinaryOp(BinaryOp, Box<Expr>, Box<Expr>),            // e1 op e2
    Def(String, Vec<String>, Box<Expr>),                  // def name(p1; p2): body
    Program(Vec<Expr>, Box<Expr>),                        // [def...] followed by main expr
```

- [ ] **Step 4: Add `parse_def` and update function-call arg parsing to support `;` separator in `src/parser.rs`**

4a. Update the function call arg parsing in `parse_compound` (around line 314) to accept `;` as well as `,`. Find:

```rust
                        args.push(self.parse_pipe()?);
                        while self.match_char(',') {
                            args.push(self.parse_pipe()?);
                        }
```

Replace with:

```rust
                        args.push(self.parse_pipe()?);
                        while self.match_char(';') || self.match_char(',') {
                            args.push(self.parse_pipe()?);
                        }
```

Also do the same update in `parse_atom` around line 520:

```rust
                                args.push(self.parse_pipe()?);
                                while self.match_char(',') {
                                    args.push(self.parse_pipe()?);
                                }
```

Replace with:

```rust
                                args.push(self.parse_pipe()?);
                                while self.match_char(';') || self.match_char(',') {
                                    args.push(self.parse_pipe()?);
                                }
```

4b. Add `parse_def` method to `impl Parser` (add before the closing `}` of the `impl Parser` block, after `match_word`):

```rust
    fn parse_def(&mut self) -> Result<Expr, ParseError> {
        self.skip_whitespace();
        let name = self.parse_ident()?;
        self.skip_whitespace();
        let params = if self.pos < self.input.len() && self.char_at(self.pos) == '(' {
            self.pos += 1; // consume '('
            let mut params = Vec::new();
            self.skip_whitespace();
            if self.pos < self.input.len() && self.char_at(self.pos) != ')' {
                // optional leading $
                if self.pos < self.input.len() && self.char_at(self.pos) == '$' {
                    self.pos += 1;
                }
                params.push(self.parse_ident()?);
                while self.match_char(';') {
                    self.skip_whitespace();
                    if self.pos < self.input.len() && self.char_at(self.pos) == '$' {
                        self.pos += 1;
                    }
                    params.push(self.parse_ident()?);
                }
            }
            self.expect(')')?;
            params
        } else {
            vec![]
        };
        self.skip_whitespace();
        self.expect(':')?;
        let body = self.parse_pipe()?;
        self.skip_whitespace();
        self.expect(';')?;
        Ok(Expr::Def(name, params, Box::new(body)))
    }
```

4c. Update the `parse()` method to collect `def` blocks before the main expression. Replace the entire `pub fn parse` method:

```rust
    pub fn parse(&mut self) -> Result<Expr, ParseError> {
        self.skip_whitespace();
        let mut defs = Vec::new();
        while self.match_word("def") {
            defs.push(self.parse_def()?);
            self.skip_whitespace();
        }
        let expr = self.parse_pipe()?;
        self.skip_whitespace();
        if self.pos < self.input.len() {
            return Err(ParseError {
                message: format!(
                    "Unexpected character at position {}: '{}'",
                    self.pos,
                    self.char_at(self.pos)
                ),
                pos: self.pos,
            });
        }
        if defs.is_empty() {
            Ok(expr)
        } else {
            Ok(Expr::Program(defs, Box::new(expr)))
        }
    }
```

- [ ] **Step 5: Update `Context` and add `Program`/`Def` evaluation in `src/interpreter.rs`**

5a. Replace the existing `Context` struct and its `impl` block with:

```rust
/// A context for variable bindings and function definitions during interpretation.
#[derive(Clone)]
pub struct Context {
    pub vars: Vec<(String, JqValue)>,
    pub fns: Vec<(String, Vec<String>, Expr)>,
    pub filter_args: Vec<(String, Expr)>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            vars: Vec::new(),
            fns: Vec::new(),
            filter_args: Vec::new(),
        }
    }

    pub fn push_var(&mut self, name: &str, value: JqValue) {
        self.vars.push((name.to_string(), value));
    }

    pub fn get_var(&self, name: &str) -> Option<JqValue> {
        self.vars
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    }

    pub fn push_fn(&mut self, name: String, params: Vec<String>, body: Expr) {
        self.fns.push((name, params, body));
    }

    pub fn get_fn(&self, name: &str) -> Option<(Vec<String>, Expr)> {
        self.fns
            .iter()
            .rev()
            .find(|(n, _, _)| n == name)
            .map(|(_, p, b)| (p.clone(), b.clone()))
    }
}
```

5b. In `interpreter.rs`, the `run()` method has a `match expr { ... }` block. After the last existing match arm (`Expr::BinaryOp` → around line 298), add these two arms before the closing `}` of the match:

```rust
            Expr::Program(defs, body) => {
                for def in defs {
                    if let Expr::Def(name, params, def_body) = def {
                        ctx.push_fn(name.clone(), params.clone(), (**def_body).clone());
                    }
                }
                self.run(body, input, ctx)
            }

            Expr::Def(_, _, _) => {
                // standalone def — no-op, already registered by Program
                Ok(vec![input.clone()])
            }
```

5c. In `call_function`, add user-defined function dispatch at the very top, before the `match name {` line. Find:

```rust
    fn call_function(
        &self,
        name: &str,
        args: &[Expr],
        input: &JqValue,
        ctx: &mut Context,
    ) -> Result<Vec<JqValue>, InterpreterError> {
        match name {
```

Replace with:

```rust
    fn call_function(
        &self,
        name: &str,
        args: &[Expr],
        input: &JqValue,
        ctx: &mut Context,
    ) -> Result<Vec<JqValue>, InterpreterError> {
        // Check filter args first (for function parameters passed as filters)
        if args.is_empty() {
            if let Some((_, filter_expr)) = ctx
                .filter_args
                .iter()
                .rev()
                .find(|(n, _)| n.as_str() == name)
                .map(|(n, e)| (n.clone(), e.clone()))
            {
                return self.run(&filter_expr, input, ctx);
            }
        }

        // Check user-defined functions
        if let Some((params, body)) = ctx.get_fn(name) {
            let mut child_ctx = ctx.clone();
            for (param, arg_expr) in params.iter().zip(args.iter()) {
                child_ctx.filter_args.push((param.clone(), arg_expr.clone()));
            }
            // Enable recursion: re-register the function in the child context
            child_ctx.push_fn(name.to_string(), params, body.clone());
            return self.run(&body, input, &mut child_ctx);
        }

        match name {
```

- [ ] **Step 6: Run tests**

```bash
cd /home/user/projects/jq-rs && cargo test test_def 2>&1
```

Expected: all 5 `test_def_*` tests pass. Fix any compile errors before proceeding.

Also run full test suite to check nothing regressed:

```bash
cd /home/user/projects/jq-rs && cargo test 2>&1
```

Expected: all tests pass.

- [ ] **Step 7: Smoke test the binary**

```bash
cd /home/user/projects/jq-rs && echo 'null' | cargo run -- 'def double: . * 2; [1,2,3] | map(double)' 2>&1
```

Expected output:
```
[
  2,
  4,
  6
]
```

```bash
cd /home/user/projects/jq-rs && echo 'null' | cargo run -- 'def fact: if . <= 1 then 1 else . * ((. - 1) | fact) end; 5 | fact' 2>&1
```

Expected output: `120`

- [ ] **Step 8: Commit and push**

```bash
cd /home/user/projects/jq-rs
git add src/parser.rs src/interpreter.rs
git commit -m "feat: add def user-defined functions with recursion and filter args"
git push origin master
```

---

### Task 2: Regex — `test`, `match`, `capture`

**Files:**
- Modify: `Cargo.toml` — add `regex = "1"`
- Modify: `src/interpreter.rs` — add `test`, `match_re`, `capture` to `call_function()`; import `regex` crate

**Interfaces:**
- Consumes: `Context::filter_args` and user-fn dispatch from Task 1 (Task 2 adds to `call_function` after the user-fn block)
- Produces: `"test"`, `"match"`, `"capture"` dispatch in `call_function`

- [ ] **Step 1: Write failing tests — add to the test module in `src/interpreter.rs`**

Add these tests inside the `mod tests { ... }` block at the bottom of `src/interpreter.rs` (inside the existing `#[cfg(test)] mod tests` that was added in Task 1):

```rust
    #[test]
    fn test_regex_test_basic() {
        let result = run_filter(r#"test("wor")"#, r#""hello world""#).unwrap();
        assert_eq!(result, vec![JqValue::Bool(true)]);
    }

    #[test]
    fn test_regex_test_no_match() {
        let result = run_filter(r#"test("xyz")"#, r#""hello world""#).unwrap();
        assert_eq!(result, vec![JqValue::Bool(false)]);
    }

    #[test]
    fn test_regex_test_flags_case_insensitive() {
        let result = run_filter(r#"test("HELLO"; "i")"#, r#""hello world""#).unwrap();
        assert_eq!(result, vec![JqValue::Bool(true)]);
    }

    #[test]
    fn test_regex_match_basic() {
        let result = run_filter(r#"match("wor")"#, r#""hello world""#).unwrap();
        assert_eq!(result.len(), 1);
        if let JqValue::Object(map) = &result[0] {
            assert_eq!(map.get("string"), Some(&JqValue::String("wor".to_string())));
            assert_eq!(map.get("offset"), Some(&JqValue::Number(6.0)));
            assert_eq!(map.get("length"), Some(&JqValue::Number(3.0)));
        } else {
            panic!("Expected object from match");
        }
    }

    #[test]
    fn test_regex_capture_named() {
        let result = run_filter(r#"capture("(?P<first>\\w+) (?P<second>\\w+)")"#, r#""hello world""#).unwrap();
        assert_eq!(result.len(), 1);
        if let JqValue::Object(map) = &result[0] {
            assert_eq!(map.get("first"), Some(&JqValue::String("hello".to_string())));
            assert_eq!(map.get("second"), Some(&JqValue::String("world".to_string())));
        } else {
            panic!("Expected object from capture");
        }
    }
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cd /home/user/projects/jq-rs && cargo test test_regex 2>&1
```

Expected: compile error — `test`, `match`, `capture` not yet implemented.

- [ ] **Step 3: Add `regex` dependency to `Cargo.toml`**

In `Cargo.toml`, add after the existing dependencies:

```toml
regex = "1"
```

The `[dependencies]` section should now read:

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
anyhow = "1"
thiserror = "1"
regex = "1"
```

- [ ] **Step 4: Add regex import and helpers in `src/interpreter.rs`**

At the very top of `src/interpreter.rs` (before `use std::collections::BTreeMap;`), add:

```rust
use regex::{Regex, RegexBuilder};
```

Add this helper function at the bottom of `src/interpreter.rs` (after `del_path`, before `#[cfg(test)]`):

```rust
fn build_regex(pattern: &str, flags: &str) -> Result<Regex, InterpreterError> {
    let mut builder = RegexBuilder::new(pattern);
    for flag in flags.chars() {
        match flag {
            'i' => { builder.case_insensitive(true); }
            'x' => { builder.ignore_whitespace(true); }
            's' => { builder.dot_matches_new_line(true); }
            'm' => { builder.multi_line(true); }
            _ => {}
        }
    }
    builder.build().map_err(|e| InterpreterError::new(format!("Invalid regex: {}", e)))
}

fn regex_match_to_object(m: regex::Match, captures: &regex::Captures, re: &Regex) -> JqValue {
    let mut map = std::collections::BTreeMap::new();
    map.insert("offset".to_string(), JqValue::Number(m.start() as f64));
    map.insert("length".to_string(), JqValue::Number(m.as_str().len() as f64));
    map.insert("string".to_string(), JqValue::String(m.as_str().to_string()));
    let caps: Vec<JqValue> = re.capture_names().enumerate().skip(1).filter_map(|(i, name)| {
        captures.get(i).map(|cm| {
            let mut cmap = std::collections::BTreeMap::new();
            cmap.insert("offset".to_string(), JqValue::Number(cm.start() as f64));
            cmap.insert("length".to_string(), JqValue::Number(cm.as_str().len() as f64));
            cmap.insert("string".to_string(), JqValue::String(cm.as_str().to_string()));
            cmap.insert("name".to_string(), name.map(|n| JqValue::String(n.to_string())).unwrap_or(JqValue::Null));
            JqValue::Object(cmap)
        })
    }).collect();
    map.insert("captures".to_string(), JqValue::Array(caps));
    JqValue::Object(map)
}
```

- [ ] **Step 5: Add `test`, `match`, `capture` dispatch in `call_function`**

In `src/interpreter.rs`, in `call_function`, find the line:

```rust
            _ => Err(InterpreterError::new(format!("Unknown function: {}", name))),
```

Replace it with:

```rust
            "test" => {
                if args.is_empty() || args.len() > 2 {
                    return Err(InterpreterError::new("test takes 1 or 2 arguments"));
                }
                let s = match input {
                    JqValue::String(s) => s.clone(),
                    _ => return Err(InterpreterError::new("test requires string input")),
                };
                let pattern = match self.eval_to_single(&args[0], input, ctx)? {
                    JqValue::String(p) => p,
                    _ => return Err(InterpreterError::new("test regex must be a string")),
                };
                let flags = if args.len() == 2 {
                    match self.eval_to_single(&args[1], input, ctx)? {
                        JqValue::String(f) => f,
                        _ => String::new(),
                    }
                } else {
                    String::new()
                };
                let re = build_regex(&pattern, &flags)?;
                Ok(vec![JqValue::Bool(re.is_match(&s))])
            }

            "match" => {
                if args.is_empty() || args.len() > 2 {
                    return Err(InterpreterError::new("match takes 1 or 2 arguments"));
                }
                let s = match input {
                    JqValue::String(s) => s.clone(),
                    _ => return Err(InterpreterError::new("match requires string input")),
                };
                let pattern = match self.eval_to_single(&args[0], input, ctx)? {
                    JqValue::String(p) => p,
                    _ => return Err(InterpreterError::new("match regex must be a string")),
                };
                let flags = if args.len() == 2 {
                    match self.eval_to_single(&args[1], input, ctx)? {
                        JqValue::String(f) => f,
                        _ => String::new(),
                    }
                } else {
                    String::new()
                };
                let global = flags.contains('g');
                let re = build_regex(&pattern, &flags.replace('g', ""))?;
                if global {
                    let results: Vec<JqValue> = re.captures_iter(&s)
                        .map(|caps| {
                            let m = caps.get(0).unwrap();
                            regex_match_to_object(m, &caps, &re)
                        })
                        .collect();
                    Ok(vec![JqValue::Array(results)])
                } else {
                    match re.captures(&s) {
                        Some(caps) => {
                            let m = caps.get(0).unwrap();
                            Ok(vec![regex_match_to_object(m, &caps, &re)])
                        }
                        None => Err(InterpreterError::new("no match")),
                    }
                }
            }

            "capture" => {
                if args.is_empty() || args.len() > 2 {
                    return Err(InterpreterError::new("capture takes 1 or 2 arguments"));
                }
                let s = match input {
                    JqValue::String(s) => s.clone(),
                    _ => return Err(InterpreterError::new("capture requires string input")),
                };
                let pattern = match self.eval_to_single(&args[0], input, ctx)? {
                    JqValue::String(p) => p,
                    _ => return Err(InterpreterError::new("capture regex must be a string")),
                };
                let flags = if args.len() == 2 {
                    match self.eval_to_single(&args[1], input, ctx)? {
                        JqValue::String(f) => f,
                        _ => String::new(),
                    }
                } else {
                    String::new()
                };
                let re = build_regex(&pattern, &flags)?;
                match re.captures(&s) {
                    Some(caps) => {
                        let mut map = std::collections::BTreeMap::new();
                        for name in re.capture_names().flatten() {
                            if let Some(m) = caps.name(name) {
                                map.insert(name.to_string(), JqValue::String(m.as_str().to_string()));
                            }
                        }
                        Ok(vec![JqValue::Object(map)])
                    }
                    None => Err(InterpreterError::new("no match")),
                }
            }

            _ => Err(InterpreterError::new(format!("Unknown function: {}", name))),
```

- [ ] **Step 6: Run tests**

```bash
cd /home/user/projects/jq-rs && cargo test test_regex 2>&1
```

Expected: all 5 `test_regex_*` tests pass.

```bash
cd /home/user/projects/jq-rs && cargo test 2>&1
```

Expected: all tests pass.

- [ ] **Step 7: Smoke test**

```bash
cd /home/user/projects/jq-rs && echo '"hello world"' | cargo run -- 'test("wor")' 2>&1
```
Expected: `true`

```bash
cd /home/user/projects/jq-rs && echo '"hello world"' | cargo run -- 'match("(\\w+) (\\w+)")' 2>&1
```
Expected: JSON object with `offset`, `length`, `string`, `captures`.

- [ ] **Step 8: Commit and push**

```bash
cd /home/user/projects/jq-rs
git add Cargo.toml Cargo.lock src/interpreter.rs
git commit -m "feat: add regex builtins test, match, capture"
git push origin master
```

---

### Task 3: Colors / ANSI Output

**Files:**
- Modify: `src/main.rs` — update `format_output` signature to accept `colored: bool`; add recursive `colorize_json` function; detect TTY with `std::io::IsTerminal`

**Interfaces:**
- Consumes: nothing from previous tasks (isolated to `main.rs`)
- Produces: colored pretty-printed output when stdout is a TTY and `-M` is not passed

- [ ] **Step 1: Write failing test — add to `src/main.rs`**

Add this test module at the bottom of `src/main.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::JqValue;

    #[test]
    fn test_colorize_string_contains_ansi() {
        let val = JqValue::String("hello".to_string());
        let out = format_output(val, false, false, true);
        assert!(out.contains("\x1b["), "Expected ANSI codes in colored output");
        assert!(out.contains("hello"), "Expected string content");
    }

    #[test]
    fn test_no_color_when_disabled() {
        let val = JqValue::String("hello".to_string());
        let out = format_output(val, false, false, false);
        assert!(!out.contains("\x1b["), "Expected no ANSI codes");
    }

    #[test]
    fn test_colorize_number() {
        let val = JqValue::Number(42.0);
        let out = format_output(val, false, false, true);
        assert!(out.contains("42"), "Expected number in output");
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cd /home/user/projects/jq-rs && cargo test test_colorize 2>&1
```

Expected: compile error — `format_output` doesn't take 4 args yet.

- [ ] **Step 3: Update `format_output` and add `colorize_json` in `src/main.rs`**

Replace the entire `format_output` function with:

```rust
fn format_output(value: JqValue, compact: bool, raw_output: bool, colored: bool) -> String {
    if raw_output {
        return match &value {
            JqValue::String(s) => s.clone(),
            JqValue::Null => "null".to_string(),
            _ => colorize_json(&value, 0, false),
        };
    }
    if compact {
        let sval: serde_json::Value = value.into();
        return serde_json::to_string(&sval).unwrap_or_default();
    }
    colorize_json(&value, 0, colored)
}

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const GREEN: &str = "\x1b[0;32m";
const BLUE: &str = "\x1b[0;34m";
const DARK: &str = "\x1b[1;30m";

fn colorize_json(val: &JqValue, indent: usize, colored: bool) -> String {
    let pad = "  ".repeat(indent);
    let pad1 = "  ".repeat(indent + 1);
    match val {
        JqValue::Null => {
            if colored {
                format!("{BOLD}null{RESET}")
            } else {
                "null".to_string()
            }
        }
        JqValue::Bool(b) => {
            let s = if *b { "true" } else { "false" };
            if colored {
                format!("{BOLD}{s}{RESET}")
            } else {
                s.to_string()
            }
        }
        JqValue::Number(n) => {
            let s = if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                format!("{}", n)
            };
            if colored {
                format!("{BLUE}{s}{RESET}")
            } else {
                s
            }
        }
        JqValue::String(s) => {
            let escaped = serde_json::to_string(s).unwrap_or_else(|_| format!("\"{}\"", s));
            if colored {
                format!("{GREEN}{escaped}{RESET}")
            } else {
                escaped
            }
        }
        JqValue::Array(arr) => {
            if arr.is_empty() {
                return "[]".to_string();
            }
            let mut out = "[\n".to_string();
            for (i, item) in arr.iter().enumerate() {
                out.push_str(&pad1);
                out.push_str(&colorize_json(item, indent + 1, colored));
                if i + 1 < arr.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&pad);
            out.push(']');
            out
        }
        JqValue::Object(map) => {
            if map.is_empty() {
                return "{}".to_string();
            }
            let mut out = "{\n".to_string();
            let entries: Vec<_> = map.iter().collect();
            for (i, (k, v)) in entries.iter().enumerate() {
                let key_str = serde_json::to_string(k).unwrap_or_else(|_| format!("\"{}\"", k));
                out.push_str(&pad1);
                if colored {
                    out.push_str(&format!("{BOLD}{BLUE}{key_str}{RESET}"));
                } else {
                    out.push_str(&key_str);
                }
                out.push_str(": ");
                out.push_str(&colorize_json(v, indent + 1, colored));
                if i + 1 < entries.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&pad);
            out.push('}');
            out
        }
    }
}
```

- [ ] **Step 4: Update all callers of `format_output` in `src/main.rs` to pass `colored`**

In `main.rs`, find where `format_output` is called (currently looks like `format_output(result, compact, raw_output)`). Replace the relevant lines.

First, add TTY detection after the `null_input` flag extraction. After:

```rust
    let null_input = matches.get_flag("null-input");
```

Add:

```rust
    let monochrome = matches.get_flag("monochrome");
    use std::io::IsTerminal;
    let colored = std::io::stdout().is_terminal() && !monochrome;
```

Then find the line:

```rust
            let output_str = format_output(result, compact, raw_output);
```

Replace with:

```rust
            let output_str = format_output(result, compact, raw_output, colored);
```

- [ ] **Step 5: Run tests**

```bash
cd /home/user/projects/jq-rs && cargo test test_colorize 2>&1
```

Expected: all 3 `test_colorize*` tests pass.

```bash
cd /home/user/projects/jq-rs && cargo test 2>&1
```

Expected: all tests pass.

- [ ] **Step 6: Smoke test**

```bash
cd /home/user/projects/jq-rs && echo '{"name":"alice","age":30}' | cargo run -- '.' 2>&1
```

Expected: pretty-printed JSON (no ANSI in CI/pipe, but no crash). With a real terminal the output would be colored.

- [ ] **Step 7: Commit and push**

```bash
cd /home/user/projects/jq-rs
git add src/main.rs
git commit -m "feat: add ANSI color output with TTY detection"
git push origin master
```

---

### Task 4: I/O — `inputs`, `input_filename`, Multi-File CLI

**Files:**
- Modify: `src/main.rs` — add `files` CLI arg, `--stream`-less multi-file reading, set `ctx.input_filename` and `ctx.remaining_inputs`
- Modify: `src/interpreter.rs` — add `input_filename: Option<String>` and `remaining_inputs: Vec<JqValue>` to `Context`; add `inputs`/`input_filename` dispatch in `call_function`

**Interfaces:**
- Consumes: `Context` struct from Task 1
- Produces: `ctx.input_filename`, `ctx.remaining_inputs`, `"inputs"` and `"input_filename"` builtins

- [ ] **Step 1: Write failing tests in `src/interpreter.rs`**

Add to the `mod tests` block in `src/interpreter.rs`:

```rust
    #[test]
    fn test_input_filename_null() {
        let mut p = Parser::new("input_filename");
        let expr = p.parse().map_err(|e| InterpreterError::new(e.to_string())).unwrap();
        let interp = Interpreter::new();
        let mut ctx = Context::new();
        ctx.input_filename = None;
        let result = interp.run(&expr, &JqValue::Null, &mut ctx).unwrap();
        assert_eq!(result, vec![JqValue::Null]);
    }

    #[test]
    fn test_input_filename_set() {
        let mut p = Parser::new("input_filename");
        let expr = p.parse().map_err(|e| InterpreterError::new(e.to_string())).unwrap();
        let interp = Interpreter::new();
        let mut ctx = Context::new();
        ctx.input_filename = Some("test.json".to_string());
        let result = interp.run(&expr, &JqValue::Null, &mut ctx).unwrap();
        assert_eq!(result, vec![JqValue::String("test.json".to_string())]);
    }

    #[test]
    fn test_inputs_drains_remaining() {
        let mut p = Parser::new("[inputs]");
        let expr = p.parse().map_err(|e| InterpreterError::new(e.to_string())).unwrap();
        let interp = Interpreter::new();
        let mut ctx = Context::new();
        ctx.remaining_inputs = vec![JqValue::Number(1.0), JqValue::Number(2.0), JqValue::Number(3.0)];
        let result = interp.run(&expr, &JqValue::Null, &mut ctx).unwrap();
        assert_eq!(result, vec![JqValue::Array(vec![
            JqValue::Number(1.0),
            JqValue::Number(2.0),
            JqValue::Number(3.0),
        ])]);
        assert!(ctx.remaining_inputs.is_empty(), "inputs should drain the list");
    }
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cd /home/user/projects/jq-rs && cargo test test_input 2>&1
```

Expected: compile error — `input_filename` and `remaining_inputs` fields don't exist on `Context` yet.

- [ ] **Step 3: Update `Context` in `src/interpreter.rs`**

Replace the `Context` struct definition (added in Task 1) with:

```rust
#[derive(Clone)]
pub struct Context {
    pub vars: Vec<(String, JqValue)>,
    pub fns: Vec<(String, Vec<String>, Expr)>,
    pub filter_args: Vec<(String, Expr)>,
    pub input_filename: Option<String>,
    pub remaining_inputs: Vec<JqValue>,
}

impl Context {
    pub fn new() -> Self {
        Context {
            vars: Vec::new(),
            fns: Vec::new(),
            filter_args: Vec::new(),
            input_filename: None,
            remaining_inputs: Vec::new(),
        }
    }

    pub fn push_var(&mut self, name: &str, value: JqValue) {
        self.vars.push((name.to_string(), value));
    }

    pub fn get_var(&self, name: &str) -> Option<JqValue> {
        self.vars
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    }

    pub fn push_fn(&mut self, name: String, params: Vec<String>, body: Expr) {
        self.fns.push((name, params, body));
    }

    pub fn get_fn(&self, name: &str) -> Option<(Vec<String>, Expr)> {
        self.fns
            .iter()
            .rev()
            .find(|(n, _, _)| n == name)
            .map(|(_, p, b)| (p.clone(), b.clone()))
    }
}
```

- [ ] **Step 4: Add `inputs` and `input_filename` to `call_function` in `src/interpreter.rs`**

In `call_function`, find the line `"error" => {` and add these two arms before it:

```rust
            "inputs" => {
                if !args.is_empty() {
                    return Err(InterpreterError::new("inputs takes no arguments"));
                }
                let values = std::mem::take(&mut ctx.remaining_inputs);
                Ok(values)
            }

            "input_filename" => {
                if !args.is_empty() {
                    return Err(InterpreterError::new("input_filename takes no arguments"));
                }
                Ok(vec![ctx.input_filename.clone()
                    .map(JqValue::String)
                    .unwrap_or(JqValue::Null)])
            }
```

- [ ] **Step 5: Add `files` CLI argument and multi-file support in `src/main.rs`**

5a. Add a `files` arg to the `Command::new("jq-rs")` builder in `main()`. After the `monochrome` arg definition (`.arg(Arg::new("monochrome")...)`), add:

```rust
        .arg(
            Arg::new("files")
                .help("Input JSON files (reads stdin if none given)")
                .num_args(0..)
                .value_name("FILE"),
        )
```

5b. Replace the stdin-reading section in `main()`. Find the block starting with:

```rust
    // Read all input
    let mut input_text = String::new();
    io::stdin()
        .read_to_string(&mut input_text)
        .with_context(|| "Failed to read from stdin")?;

    // Parse input JSON(s)
    let input_values: Vec<JqValue> = if null_input {
        vec![JqValue::Null]
    } else {
        let trimmed = input_text.trim();
        if trimmed.is_empty() {
            vec![]
        } else if slurp {
            let values = parse_json_stream(trimmed);
            if values.is_empty() {
                vec![]
            } else {
                vec![JqValue::Array(values)]
            }
        } else {
            parse_json_stream(trimmed)
        }
    };
```

Replace it entirely with:

```rust
    // Always collect stdin/file values (needed for `inputs` builtin even in -n mode)
    let file_paths: Vec<String> = matches
        .get_many::<String>("files")
        .unwrap_or_default()
        .cloned()
        .collect();

    let mut raw_inputs: Vec<(Option<String>, JqValue)> = Vec::new();
    if file_paths.is_empty() {
        let mut text = String::new();
        io::stdin()
            .read_to_string(&mut text)
            .with_context(|| "Failed to read from stdin")?;
        for v in parse_json_stream(text.trim()) {
            raw_inputs.push((None, v));
        }
    } else {
        for path in &file_paths {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read file: {}", path))?;
            for v in parse_json_stream(text.trim()) {
                raw_inputs.push((Some(path.clone()), v));
            }
        }
    }
    let all_raw_values: Vec<JqValue> = raw_inputs.iter().map(|(_, v)| v.clone()).collect();

    // Build all_inputs: what gets processed as main inputs
    let all_inputs: Vec<(Option<String>, JqValue)> = if null_input {
        vec![(None, JqValue::Null)]
    } else if slurp {
        let values: Vec<JqValue> = raw_inputs.iter().map(|(_, v)| v.clone()).collect();
        vec![(None, JqValue::Array(values))]
    } else {
        raw_inputs
    };
```

5c. Replace the processing loop. Find:

```rust
    let interpreter = Interpreter::new();
    let mut ctx = Context::new();

    // Process each input
    let mut outputs: Vec<String> = Vec::new();

    for input in &input_values {
        let results = interpreter
            .run(&expr, input, &mut ctx)
            .map_err(|e| anyhow!("{}", e))?;
        for result in results {
            let output_str = format_output(result, compact, raw_output, colored);
            outputs.push(output_str);
        }
    }
```

Replace with:

```rust
    let interpreter = Interpreter::new();

    // Process each input
    let mut outputs: Vec<String> = Vec::new();

    for (i, (filename, input)) in all_inputs.iter().enumerate() {
        let mut ctx = Context::new();
        ctx.input_filename = filename.clone();
        // In null-input mode all stdin values are available via `inputs`;
        // otherwise, remaining inputs are the ones after the current index.
        ctx.remaining_inputs = if null_input {
            all_raw_values.clone()
        } else {
            all_inputs[i + 1..].iter().map(|(_, v)| v.clone()).collect()
        };

        let results = interpreter
            .run(&expr, input, &mut ctx)
            .map_err(|e| anyhow!("{}", e))?;
        for result in results {
            let output_str = format_output(result, compact, raw_output, colored);
            outputs.push(output_str);
        }
    }
```

- [ ] **Step 6: Run tests**

```bash
cd /home/user/projects/jq-rs && cargo test test_input 2>&1
```

Expected: all 3 `test_input*` tests pass.

```bash
cd /home/user/projects/jq-rs && cargo test 2>&1
```

Expected: all tests pass.

- [ ] **Step 7: Smoke test**

```bash
cd /home/user/projects/jq-rs && printf '1\n2\n3\n' | cargo run -- -n '[inputs]' 2>&1
```

Expected: `[1, 2, 3]`

```bash
cd /home/user/projects/jq-rs && echo '{}' | cargo run -- 'input_filename' 2>&1
```

Expected: `null`

- [ ] **Step 8: Commit and push**

```bash
cd /home/user/projects/jq-rs
git add src/main.rs src/interpreter.rs
git commit -m "feat: add inputs, input_filename builtins and multi-file CLI support"
git push origin master
```

---

### Task 5: Streaming Parser + `--stream` Flag

**Files:**
- Modify: `src/main.rs` — replace `read_to_string` → `BufReader` + `serde_json` iterator for lazy stdin/file parsing; add `--stream` flag and streaming output mode

**Interfaces:**
- Consumes: `all_inputs` collection from Task 4
- Produces: lazy JSON parsing via `serde_json::Deserializer::from_reader`; `--stream` flag producing `[[path], leaf]` events

- [ ] **Step 1: Write failing test in `src/main.rs`**

Add to the `mod tests` block in `src/main.rs`:

```rust
    #[test]
    fn test_stream_value_scalar() {
        let val = JqValue::Number(42.0);
        let events = stream_value(&[], &val);
        assert_eq!(events.len(), 1);
        // [[], 42]
        if let JqValue::Array(pair) = &events[0] {
            assert_eq!(&pair[0], &JqValue::Array(vec![]));
            assert_eq!(&pair[1], &JqValue::Number(42.0));
        } else {
            panic!("Expected array pair");
        }
    }

    #[test]
    fn test_stream_value_object() {
        let mut map = std::collections::BTreeMap::new();
        map.insert("a".to_string(), JqValue::Number(1.0));
        let val = JqValue::Object(map);
        let events = stream_value(&[], &val);
        // [[["a"],1], [["a"],{"truncated":true}]]  — actually [[["a"],1], [["a"],{"truncated":true}]]
        // For a leaf object entry: [["a"], 1] then truncated at end
        // The exact format: [path_to_key, value] for each leaf, then [path, {"truncated":true}]
        assert!(events.len() >= 2, "Expected at least 2 events for object");
    }
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cd /home/user/projects/jq-rs && cargo test test_stream 2>&1
```

Expected: compile error — `stream_value` function doesn't exist yet.

- [ ] **Step 3: Add `stream_value` function to `src/main.rs`**

Add this function before the `#[cfg(test)]` block at the bottom of `src/main.rs`:

```rust
pub fn stream_value(path: &[JqValue], val: &JqValue) -> Vec<JqValue> {
    let mut events = Vec::new();
    match val {
        JqValue::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                let mut child_path = path.to_vec();
                child_path.push(JqValue::Number(i as f64));
                events.extend(stream_value(&child_path, item));
            }
            // truncated marker
            let mut trunc = std::collections::BTreeMap::new();
            trunc.insert("truncated".to_string(), JqValue::Bool(true));
            events.push(JqValue::Array(vec![
                JqValue::Array(path.to_vec()),
                JqValue::Object(trunc),
            ]));
        }
        JqValue::Object(map) => {
            for (k, v) in map {
                let mut child_path = path.to_vec();
                child_path.push(JqValue::String(k.clone()));
                events.extend(stream_value(&child_path, v));
            }
            let mut trunc = std::collections::BTreeMap::new();
            trunc.insert("truncated".to_string(), JqValue::Bool(true));
            events.push(JqValue::Array(vec![
                JqValue::Array(path.to_vec()),
                JqValue::Object(trunc),
            ]));
        }
        _ => {
            events.push(JqValue::Array(vec![
                JqValue::Array(path.to_vec()),
                val.clone(),
            ]));
        }
    }
    events
}
```

- [ ] **Step 4: Replace `parse_json_stream` (read-all) with lazy reader in `src/main.rs`**

Replace the existing `parse_json_stream`, `try_parse_json_prefix`, and `json_consumed_length` functions with a single lazy-reader version:

```rust
fn parse_json_from_reader<R: std::io::Read>(reader: R) -> Vec<JqValue> {
    serde_json::Deserializer::from_reader(reader)
        .into_iter::<serde_json::Value>()
        .filter_map(|r| r.ok())
        .map(JqValue::from)
        .collect()
}

fn parse_json_stream(text: &str) -> Vec<JqValue> {
    parse_json_from_reader(text.as_bytes())
}
```

This keeps the `parse_json_stream(text.trim())` call sites intact while switching to lazy reading internally.

Also update the stdin reading path in `main()` to use the reader directly (avoiding `read_to_string` for stdin). Find the stdin reading block added in Task 4:

```rust
        if file_paths.is_empty() {
            let mut text = String::new();
            io::stdin()
                .read_to_string(&mut text)
                .with_context(|| "Failed to read from stdin")?;
            let vals = parse_json_stream(text.trim());
            for v in vals {
                all.push((None, v));
            }
        }
```

Replace with:

```rust
        if file_paths.is_empty() {
            let reader = io::BufReader::new(io::stdin());
            let vals = parse_json_from_reader(reader);
            for v in vals {
                all.push((None, v));
            }
        }
```

Similarly update the file reading path. Find:

```rust
            for path in &file_paths {
                let text = std::fs::read_to_string(path)
                    .with_context(|| format!("Failed to read file: {}", path))?;
                let vals = parse_json_stream(text.trim());
```

Replace with:

```rust
            for path in &file_paths {
                let file = std::fs::File::open(path)
                    .with_context(|| format!("Failed to open file: {}", path))?;
                let reader = io::BufReader::new(file);
                let vals = parse_json_from_reader(reader);
```

- [ ] **Step 5: Add `--stream` CLI flag and streaming output in `src/main.rs`**

5a. Add the `--stream` arg to `Command::new("jq-rs")`. After the `files` arg, add:

```rust
        .arg(
            Arg::new("stream")
                .long("stream")
                .action(clap::ArgAction::SetTrue)
                .help("Output streaming path/value events instead of filter results"),
        )
```

5b. After the `colored` variable is set, add:

```rust
    let stream_mode = matches.get_flag("stream");
```

5c. Replace the processing loop section to support streaming mode. Find the loop:

```rust
    for (i, (filename, input)) in all_inputs.iter().enumerate() {
        let mut ctx = Context::new();
        ctx.input_filename = filename.clone();
        ctx.remaining_inputs = all_inputs[i + 1..].iter().map(|(_, v)| v.clone()).collect();

        let results = interpreter
            .run(&expr, input, &mut ctx)
            .map_err(|e| anyhow!("{}", e))?;
        for result in results {
            let output_str = format_output(result, compact, raw_output, colored);
            outputs.push(output_str);
        }
        let _ = total;
    }
```

Replace with:

```rust
    for (i, (filename, input)) in all_inputs.iter().enumerate() {
        let mut ctx = Context::new();
        ctx.input_filename = filename.clone();
        ctx.remaining_inputs = all_inputs[i + 1..].iter().map(|(_, v)| v.clone()).collect();

        if stream_mode {
            let events = stream_value(&[], input);
            for event in events {
                let output_str = format_output(event, compact, raw_output, false);
                outputs.push(output_str);
            }
        } else {
            let results = interpreter
                .run(&expr, input, &mut ctx)
                .map_err(|e| anyhow!("{}", e))?;
            for result in results {
                let output_str = format_output(result, compact, raw_output, colored);
                outputs.push(output_str);
            }
        }
        let _ = total;
    }
```

- [ ] **Step 6: Run tests**

```bash
cd /home/user/projects/jq-rs && cargo test test_stream 2>&1
```

Expected: both `test_stream_*` tests pass.

```bash
cd /home/user/projects/jq-rs && cargo test 2>&1
```

Expected: all tests pass.

- [ ] **Step 7: Smoke test**

```bash
cd /home/user/projects/jq-rs && echo '{"a":1,"b":2}' | cargo run -- --stream '.' 2>&1
```

Expected output (approximate — order matches BTreeMap):
```
[
  [
    "a"
  ],
  1
]
[
  [
    "b"
  ],
  2
]
[
  [],
  {
    "truncated": true
  }
]
```

- [ ] **Step 8: Commit and push**

```bash
cd /home/user/projects/jq-rs
git add src/main.rs
git commit -m "feat: lazy streaming parser and --stream flag for path/leaf event output"
git push origin master
```

---

### Task 6: Module System — `import` / `include`

**Files:**
- Modify: `src/parser.rs` — update `parse_ident` to support `::` in names; add `parse_import`/`parse_include` at top of `parse()`; add file loading and recursive parsing

**Interfaces:**
- Consumes: `Expr::Def`, `Expr::Program` from Task 1; `parse_def()` from Task 1
- Produces: `import "path" as ns;` and `include "path";` support; `::` namespace separator in identifiers

- [ ] **Step 1: Write failing tests — add to `src/parser.rs`**

Find the `#[cfg(test)] mod tests` block at the bottom of `src/parser.rs` and add:

```rust
    #[test]
    fn test_namespaced_ident() {
        // lib::double should parse as a function call named "lib::double"
        let mut p = Parser::new("lib::double");
        let expr = p.parse().unwrap();
        match expr {
            Expr::FunctionCall(name, args) => {
                assert_eq!(name, "lib::double");
                assert!(args.is_empty());
            }
            other => panic!("Expected FunctionCall, got {:?}", other),
        }
    }

    #[test]
    fn test_def_then_namespaced_call() {
        // Simulate what include does: def lib::double: . * 2; 5 | lib::double
        let mut p = Parser::new("def lib::double: . * 2; 5 | lib::double");
        let expr = p.parse().unwrap();
        match expr {
            Expr::Program(defs, _body) => {
                assert_eq!(defs.len(), 1);
                if let Expr::Def(name, _, _) = &defs[0] {
                    assert_eq!(name, "lib::double");
                } else {
                    panic!("Expected Def node");
                }
            }
            other => panic!("Expected Program, got {:?}", other),
        }
    }
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cd /home/user/projects/jq-rs && cargo test test_namespaced 2>&1
```

Expected: fail — `lib::double` currently parses as `FunctionCall("lib", [])` and stops at `::`.

- [ ] **Step 3: Update `parse_ident` to support `::` namespace separator in `src/parser.rs`**

Replace the existing `parse_ident` method:

```rust
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
```

With:

```rust
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
        let mut name = self.input[start..self.pos].to_string();
        // Support namespace::name syntax (two colons, not one)
        while self.pos + 1 < self.input.len()
            && self.char_at(self.pos) == ':'
            && self.char_at(self.pos + 1) == ':'
        {
            self.pos += 2;
            let ns_start = self.pos;
            while self.pos < self.input.len() && is_ident_part(self.char_at(self.pos)) {
                self.pos += 1;
            }
            if self.pos == ns_start {
                // bare `::` with no following ident — put back and stop
                self.pos -= 2;
                break;
            }
            name.push_str("::");
            name.push_str(&self.input[ns_start..self.pos]);
        }
        Ok(name)
    }
```

- [ ] **Step 4: Add `parse_import`/`parse_include` and module loading in `src/parser.rs`**

4a. Add these helper methods to `impl Parser` (before the closing `}` of the impl block):

```rust
    fn parse_module_path(&mut self) -> Result<String, ParseError> {
        // Module path is a quoted string
        self.skip_whitespace();
        if self.pos >= self.input.len() || self.char_at(self.pos) != '"' {
            return Err(ParseError {
                message: "Expected quoted module path".to_string(),
                pos: self.pos,
            });
        }
        self.parse_string()
    }

    fn resolve_module_path(path: &str, search_paths: &[std::path::PathBuf]) -> Option<std::path::PathBuf> {
        let with_ext = if path.ends_with(".jq") {
            path.to_string()
        } else {
            format!("{}.jq", path)
        };
        for dir in search_paths {
            let candidate = dir.join(&with_ext);
            if candidate.exists() {
                return Some(candidate);
        }
        }
        None
    }

    fn module_search_paths() -> Vec<std::path::PathBuf> {
        let mut paths = Vec::new();
        // $JQLIB env var (colon-separated)
        if let Ok(jqlib) = std::env::var("JQLIB") {
            for p in jqlib.split(':') {
                paths.push(std::path::PathBuf::from(p));
            }
        }
        // ~/.jq/
        if let Some(home) = std::env::var("HOME").ok() {
            paths.push(std::path::PathBuf::from(home).join(".jq"));
        }
        // current directory
        paths.push(std::path::PathBuf::from("."));
        paths
    }

    fn load_module_defs(
        module_path: &str,
        namespace: Option<&str>,
        visited: &mut Vec<String>,
    ) -> Result<Vec<Expr>, ParseError> {
        if visited.contains(&module_path.to_string()) {
            return Ok(vec![]); // circular import guard
        }
        let search_paths = Parser::module_search_paths();
        let file_path = Parser::resolve_module_path(module_path, &search_paths).ok_or_else(|| {
            ParseError {
                message: format!("Module not found: {}", module_path),
                pos: 0,
            }
        })?;
        let source = std::fs::read_to_string(&file_path).map_err(|e| ParseError {
            message: format!("Failed to read module {}: {}", file_path.display(), e),
            pos: 0,
        })?;
        visited.push(module_path.to_string());
        let mut sub_parser = Parser::new(&source);
        // Parse the module as a sequence of defs (no main expression required)
        let mut defs = Vec::new();
        sub_parser.skip_whitespace();
        while sub_parser.match_word("def") {
            let def = sub_parser.parse_def()?;
            defs.push(def);
            sub_parser.skip_whitespace();
        }
        // Apply namespace prefix if given
        if let Some(ns) = namespace {
            defs = defs.into_iter().map(|d| {
                if let Expr::Def(name, params, body) = d {
                    Expr::Def(format!("{}::{}", ns, name), params, body)
                } else {
                    d
                }
            }).collect();
        }
        Ok(defs)
    }
```

4b. Update the `parse()` method to handle `import` and `include` before `def` blocks:

```rust
    pub fn parse(&mut self) -> Result<Expr, ParseError> {
        self.skip_whitespace();
        let mut defs = Vec::new();
        let mut visited = Vec::new();

        // Handle import/include at the top
        loop {
            if self.match_word("import") {
                let path = self.parse_module_path()?;
                self.skip_whitespace();
                self.expect_word("as")?;
                self.skip_whitespace();
                let ns = self.parse_ident()?;
                self.skip_whitespace();
                self.expect(';')?;
                let module_defs = Parser::load_module_defs(&path, Some(&ns), &mut visited)?;
                defs.extend(module_defs);
            } else if self.match_word("include") {
                let path = self.parse_module_path()?;
                self.skip_whitespace();
                self.expect(';')?;
                let module_defs = Parser::load_module_defs(&path, None, &mut visited)?;
                defs.extend(module_defs);
            } else {
                break;
            }
            self.skip_whitespace();
        }

        // Collect local def blocks
        while self.match_word("def") {
            defs.push(self.parse_def()?);
            self.skip_whitespace();
        }

        let expr = self.parse_pipe()?;
        self.skip_whitespace();
        if self.pos < self.input.len() {
            return Err(ParseError {
                message: format!(
                    "Unexpected character at position {}: '{}'",
                    self.pos,
                    self.char_at(self.pos)
                ),
                pos: self.pos,
            });
        }
        if defs.is_empty() {
            Ok(expr)
        } else {
            Ok(Expr::Program(defs, Box::new(expr)))
        }
    }
```

- [ ] **Step 5: Run tests**

```bash
cd /home/user/projects/jq-rs && cargo test test_namespaced 2>&1
```

Expected: both `test_namespaced_*` tests pass.

```bash
cd /home/user/projects/jq-rs && cargo test 2>&1
```

Expected: all tests pass.

- [ ] **Step 6: Smoke test with a module file**

Create a test module file and exercise it:

```bash
echo 'def double: . * 2;' > /tmp/mylib.jq
cd /home/user/projects/jq-rs && JQLIB=/tmp echo 'null' | cargo run -- 'include "mylib"; [1,2,3] | map(double)' 2>&1
```

Expected: `[2, 4, 6]`

```bash
echo 'def triple: . * 3;' > /tmp/mathlib.jq
cd /home/user/projects/jq-rs && JQLIB=/tmp echo 'null' | cargo run -- 'import "mathlib" as m; 5 | m::triple' 2>&1
```

Expected: `15`

Clean up:
```bash
rm /tmp/mylib.jq /tmp/mathlib.jq
```

- [ ] **Step 7: Commit and push**

```bash
cd /home/user/projects/jq-rs
git add src/parser.rs
git commit -m "feat: module system with import/include and :: namespace support"
git push origin master
```

---

### Task 7: CI Verification + Tag v0.2.0

**Files:**
- Read: `.github/workflows/` — check existing CI workflows
- Possibly fix: any CI failures from the new features

**Interfaces:**
- Consumes: all 6 feature tasks completed and pushed

- [ ] **Step 1: Check existing CI workflows**

```bash
ls /home/user/projects/jq-rs/.github/workflows/ 2>&1
```

Read each workflow file to understand what CI runs.

- [ ] **Step 2: Run the full test suite locally**

```bash
cd /home/user/projects/jq-rs && cargo test 2>&1
```

Expected: all tests pass. If any fail, fix them before proceeding.

```bash
cd /home/user/projects/jq-rs && cargo clippy 2>&1 | grep "^error" | head -20
```

Fix any clippy errors (warnings are OK to skip).

- [ ] **Step 3: Check CI status on GitHub**

```bash
gh run list --repo "$(git remote get-url origin | sed 's/.*github.com[:/]//' | sed 's/.git$//')" --limit 5 2>&1
```

If CI is failing, read the failure log:

```bash
gh run view <run-id> --log-failed 2>&1 | head -100
```

Fix any failures and push:

```bash
cd /home/user/projects/jq-rs
git add -u
git commit -m "fix: CI issues after feature additions"
git push origin master
```

Then poll until CI is green:

```bash
gh run watch 2>&1
```

- [ ] **Step 4: Tag v0.2.0**

Once CI is green:

```bash
cd /home/user/projects/jq-rs
git tag v0.2.0
git push origin v0.2.0
```

- [ ] **Step 5: Verify release (if release workflow exists)**

```bash
gh run list --limit 5 2>&1
```

If a release workflow was triggered by the tag, watch it:

```bash
gh run watch 2>&1
```

Verify the release:

```bash
gh release view v0.2.0 2>&1
```

- [ ] **Step 6: Report completion**

Report: "All 6 features implemented and verified. Tagged v0.2.0. Features added: `def` with recursion, regex (`test`/`match`/`capture`), ANSI color output, `inputs`/`input_filename` I/O builtins, lazy streaming parser + `--stream` flag, module system (`import`/`include` with `::` namespacing)."
