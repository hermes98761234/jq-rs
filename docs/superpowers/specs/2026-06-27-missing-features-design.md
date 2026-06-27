# jq-rs Missing Features Design

**Date:** 2026-06-27  
**Scope:** Implement the 6 unimplemented features from README.md  
**Approach:** Linear Hermes task chain (A) — strict sequence, no parallelism

---

## Features to Implement

| # | Feature | Complexity |
|---|---------|------------|
| 1 | `def` — user-defined functions with recursion | High |
| 2 | Regex — `test`, `match`, `capture` | Medium |
| 3 | Colors/ANSI output | Low |
| 4 | I/O — `inputs`, `input_filename`, multi-file CLI | Medium |
| 5 | Streaming parser — lazy stdin + `--stream` flag | Medium |
| 6 | Module system — `import`/`include` | High |

---

## Architecture

All changes extend the existing 4-file structure. No new files.

```
src/
├── main.rs        — CLI args, I/O loop, format_output (colors here)
├── parser.rs      — AST (Expr enum), recursive descent parser
├── interpreter.rs — tree-walking evaluator, Context (vars + fns)
└── value.rs       — JqValue type
```

New crate dependencies:
- `regex = "1"` — for feature 2
- No other new deps (`IsTerminal` from stdlib covers TTY detection)

---

## Feature 1: `def` with Recursion

### AST additions (parser.rs)

```rust
Def(String, Vec<String>, Box<Expr>),   // def name(p1; p2): body
Program(Vec<Expr>, Box<Expr>),          // def blocks + main expr
```

### Context additions (interpreter.rs)

```rust
pub fns: Vec<(String, Vec<String>, Expr)>,
```

Add `push_fn` and `get_fn` methods mirroring the existing `push_var`/`get_var` pattern.

### Parser

At the top level, before parsing the main expression:
1. Collect zero or more `def name(p1; p2; ...): body;` blocks
2. Wrap in `Expr::Program(defs, main_expr)`

Args are `;`-separated (jq convention). Zero-arg defs: `def double: . * 2;`

### Interpreter

On `Expr::Program`: register all defs into ctx, then evaluate main expr.

On `FunctionCall(name, args)`: check `ctx.fns` before built-ins. When found:
1. Evaluate each arg as a filter (not a value — jq passes filters as args)
2. Create a child context with params bound
3. Include the current function in the child context (enables recursion)
4. Evaluate body in child context

### Examples

```
def double: . * 2; [1,2,3] | map(double)
# => [2,4,6]

def fact: if . <= 1 then 1 else . * ((. - 1) | fact) end; 5 | fact
# => 120
```

---

## Feature 2: Regex

### Cargo.toml

```toml
regex = "1"
```

### Implementation

No new AST nodes — handle as `FunctionCall` in the interpreter's built-in dispatch.

**`test(re)` / `test(re; flags)`**
- Input: string
- Returns: `Bool` — true if input matches regex

**`match(re)` / `match(re; flags)`**
- Input: string
- Returns: object:
  ```json
  {"offset": 0, "length": 5, "string": "hello", "captures": [{"offset":0,"length":3,"string":"hel","name":null}]}
  ```

**`capture(re)` / `capture(re; flags)`**
- Input: string
- Returns: object of named capture groups: `{"name": "value", ...}`

**Flags:** `g` (global/all matches → array), `i` (case-insensitive), `x` (extended/ignore whitespace).

Flag handling: build `regex::RegexBuilder`, set `case_insensitive(true)` for `i`, `ignore_whitespace(true)` for `x`. For `g`, collect all matches instead of just the first.

---

## Feature 3: Colors/ANSI Output

### Detection

```rust
use std::io::IsTerminal;
let colored = std::io::stdout().is_terminal() && !monochrome;
```

No extra crate needed — `IsTerminal` is stable since Rust 1.70 (project minimum).

### Color scheme (matches original jq)

| Token | ANSI |
|-------|------|
| Object key | `\x1b[34;1m` (bold blue) |
| String value | `\x1b[0;32m` (green) |
| Number | `\x1b[0;39m` (default) |
| `null` | `\x1b[1;30m` (bold dark) |
| `true`/`false` | `\x1b[0;39m` (default) |
| Reset | `\x1b[0m` |

### Implementation

Extend `format_output(value, compact, raw_output, colored)` with a recursive `colorize_pretty(value, indent, colored)` function that builds the pretty-printed string with ANSI codes interspersed. Only applies when `colored && !compact`.

---

## Feature 4: I/O — `inputs`, `input_filename`

### CLI change (main.rs)

Add positional file arguments after the filter:
```
jq-rs '.foo' a.json b.json c.json
```
Without files → read from stdin (existing behavior).

```rust
Arg::new("files")
    .help("Input JSON files")
    .num_args(0..)
```

### Context additions (interpreter.rs)

```rust
pub input_filename: Option<String>,       // current file name, None for stdin
pub remaining_inputs: Vec<(Option<String>, JqValue)>,  // (filename, value) pairs
```

### Builtins

**`input_filename`** → `ctx.input_filename.clone().map(JqValue::String).unwrap_or(JqValue::Null)`

**`inputs`** → drain `ctx.remaining_inputs`, returning each value as a stream element. Side-effectful: once consumed, values are gone.

### main.rs flow

1. Collect all `(filename, values)` pairs from files/stdin
2. For each input value being processed: set `ctx.input_filename`, set `ctx.remaining_inputs` to all subsequent values
3. Run filter on current value

---

## Feature 5: Streaming Parser

### Lazy stdin reading

Replace `io::stdin().read_to_string(...)` with:
```rust
let reader = io::BufReader::new(io::stdin());
let stream = serde_json::Deserializer::from_reader(reader).into_iter::<serde_json::Value>();
```
Processes values one at a time — no full stdin buffering. Same for file inputs.

### `--stream` flag

New CLI flag: `--stream` (no short form — avoids collision with `-s` slurp). When active, instead of outputting filter results normally, each input JSON value is decomposed into a stream of path/leaf events:

```
[[0],"a"]
[[1],"b"]
[[1],{"truncated":true}]
```

Implemented as `fn stream_value(path: &[JqValue], val: &JqValue) -> Vec<JqValue>` in interpreter.rs — recursively walks the value, emitting `[path, leaf]` pairs for scalars and `[path, {"truncated":true}]` after arrays/objects.

---

## Feature 6: Module System

### Syntax

```
import "path" as name;   # load defs, prefix with name::
include "path";          # load defs, no prefix
```

Both appear at the top of a program, before `def` blocks.

### Search path (in order)

1. `$JQLIB` environment variable (colon-separated paths)
2. `~/.jq/`
3. Current directory

Files must have `.jq` extension. `import "foo/bar"` resolves to `foo/bar.jq`.

### Implementation

Resolution happens at **parse time**, not runtime:

1. Parser encounters `import`/`include` — reads the `.jq` file from disk
2. Parses it recursively (supports nested imports)
3. Collects all `def` nodes from the module
4. For `import "x" as ns`: renames each def from `name` to `ns::name` in the AST
5. Splices the module's defs into the current `Program` node's def list before the local defs

This means no runtime lazy loading — all modules are resolved and merged into one flat def list before evaluation begins.

### Circular import guard

Track in-progress file paths during parse; if a file is encountered twice, skip it (like `include` guards in C).

---

## Task Sequence

```
T1: def + recursion
 └─ T2: regex (test, match, capture)
     └─ T3: colors/ANSI output
         └─ T4: I/O (inputs, input_filename, multi-file CLI)
             └─ T5: streaming parser + --stream flag
                 └─ T6: module system (import/include)
                     └─ T7: CI fix + tag v0.2.0
```

All tasks commit and push `origin master` on completion.

---

## Success Criteria

After all 6 tasks:

```bash
# def
echo 'null' | jq-rs 'def double: . * 2; [1,2,3] | map(double)'
# => [2,4,6]

# regex
echo '"hello world"' | jq-rs 'test("world")'
# => true

# colors (visual check — output is colored when TTY)
echo '{"a":1}' | jq-rs '.'

# inputs
echo '1\n2\n3' | jq-rs -n '[inputs]'
# => [1,2,3]

# streaming
echo '{"a":1}' | jq-rs --stream '.'
# => [["a"],1]  [[],{"truncated":true}]  (path/leaf events)

# modules
echo 'null' | jq-rs 'import "mylib" as lib; lib::double(5)'
```
