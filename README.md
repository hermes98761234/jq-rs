# jq-rs 🦀

> A Rust reimplementation of [jq](https://github.com/jqlang/jq) — the lightweight and flexible command-line JSON processor.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![Build](https://img.shields.io/badge/build-passing-brightgreen.svg)]()

## Overview

**jq-rs** is a from-scratch Rust implementation of the jq JSON query language. It provides a fast, safe, and modern alternative to the original C implementation, leveraging Rust's memory safety guarantees and zero-cost abstractions.

This project serves as both a learning resource for building interpreters in Rust and a practical tool for processing JSON on the command line.

## Features

### Implemented

| Feature | jq-rs | Original jq |
|---------|:-----:|:-----------:|
| Identity (`.`) | ✅ | ✅ |
| Field access (`.field`) | ✅ | ✅ |
| Array iteration (`.[]`) | ✅ | ✅ |
| Array index (`.["key"]`, `.[0]`) | ✅ | ✅ |
| Pipe (`\|`) | ✅ | ✅ |
| `select(expr)` | ✅ | ✅ |
| `map(expr)` | ✅ | ✅ |
| `reduce expr as $var (init; update)` | ✅ | ✅ |
| `if-then-else-end` | ✅ | ✅ |
| `try-catch` | ✅ | ✅ |
| `length` | ✅ | ✅ |
| `keys` | ✅ | ✅ |
| `type` | ✅ | ✅ |
| `has(key)` | ✅ | ✅ |
| `contains` | ✅ | ✅ |
| `in` | ✅ | ✅ |
| `sort` | ✅ | ✅ |
| `unique` | ✅ | ✅ |
| `min` / `max` | ✅ | ✅ |
| `add` | ✅ | ✅ |
| `tonumber` / `tostring` | ✅ | ✅ |
| `startswith` / `endswith` | ✅ | ✅ |
| `split` / `join` | ✅ | ✅ |
| `flatten` | ✅ | ✅ |
| `reverse` | ✅ | ✅ |
| `first` / `last` / `nth(n)` | ✅ | ✅ |
| `to_entries` / `from_entries` | ✅ | ✅ |
| `getpath` / `setpath` / `delpaths` | ✅ | ✅ |
| `all` / `any` | ✅ | ✅ |
| `group_by(expr)` | ✅ | ✅ |
| `sort_by(expr)` | ✅ | ✅ |
| `min_by` / `max_by` | ✅ | ✅ |
| Arithmetic (`+`, `-`, `*`, `/`, `%`) | ✅ | ✅ |
| Comparison (`==`, `!=`, `<`, `>`, `<=`, `>=`) | ✅ | ✅ |
| Boolean (`and`, `or`) | ✅ | ✅ |
| Array/Object literals | ✅ | ✅ |
| Compact output (`-c`) | ✅ | ✅ |
| Raw output (`-r`) | ✅ | ✅ |
| Slurp (`-s`) | ✅ | ✅ |
| Null input (`-n`) | ✅ | ✅ |
| Program from file (`-f`) | ✅ | ✅ |
| Monochrome (`-M`) | ✅ | ✅ |
| Regex (`test`, `match`, `capture`) | ❌ | ✅ |
| I/O (`inputs`, `input_filename`) | ❌ | ✅ |
| Module system | ❌ | ✅ |
| Streaming parser | ❌ | ✅ |
| `def` (user-defined functions) | ❌ | ✅ |
| Colors/ANSI output | ❌ | ✅ |

## Installation

### From Source

```bash
git clone https://github.com/hermes98761234/jq-rs.git
cd jq-rs
cargo build --release
```

The binary will be at `target/release/jq-rs`.

### Quick Start

```bash
# Add to your PATH
sudo cp target/release/jq-rs /usr/local/bin/
```

## Usage

### Basic Examples

```bash
# Extract a field
echo '{"name":"john","age":30}' | jq-rs '.name'
# Output: "john"

# Array iteration
echo '[1,2,3]' | jq-rs '.[]'
# Output: 1 2 3 (each on its own line)

# Map transformation
echo '[1,2,3,4,5]' | jq-rs 'map(. * 2)'
# Output: [2, 4, 6, 8, 10]

# Filter with select
echo '[1,2,3,4,5]' | jq-rs 'select(. > 3)'
# Output: 4 5

# Pipe operations
echo '{"items":["a","b","c"]}' | jq-rs '.items | length'
# Output: 3

# Raw string output
echo '{"name":"hello"}' | jq-rs -r '.name'
# Output: hello (without quotes)

# Compact output
echo '{"a":1,"b":2}' | jq-rs -c '.'
# Output: {"a":1,"b":2}
```

### Advanced Examples

```bash
# Reduce
echo '[1,2,3,4,5]' | jq-rs 'reduce .[] as $x (0; . + $x)'
# Output: 15

# Group by
echo '[{"t":"a"},{"t":"b"},{"t":"a"}]' | jq-rs 'group_by(.t)'

# Sort
echo '[3,1,4,1,5,9,2,6]' | jq-rs 'sort'

# Object construction
echo '{"first":"John","last":"Doe"}' | jq-rs '{name: (.first + " " + .last)}'

# Conditional
echo '5' | jq-rs 'if . > 3 then "big" else "small" end'

# Keys and length
echo '{"a":1,"b":2,"c":3}' | jq-rs 'keys | length'
# Output: 3
```

## Building

### Requirements

- Rust 1.70+ (2021 edition)
- Cargo

### Development

```bash
# Debug build
cargo build

# Run tests
cargo test

# Release build (optimized)
cargo build --release

# Run with cargo
cargo run -- '.name' <<< '{"name":"test"}'
```

## Architecture

```
src/
├── main.rs        # CLI entry point, argument parsing, I/O
├── parser.rs      # Recursive descent parser for jq filter language
├── interpreter.rs # Tree-walking interpreter
└── value.rs       # JSON value type with serde integration
```

The implementation follows a classic interpreter pipeline:

1. **Lexing/Parsing** — `parser.rs` implements a recursive descent parser that produces an AST (`Expr` enum)
2. **Interpretation** — `interpreter.rs` walks the AST, evaluating expressions against input JSON values
3. **Value System** — `value.rs` defines `JqValue`, a JSON-compatible value type with conversions to/from `serde_json::Value`

## Performance

While not yet optimized for raw speed, jq-rs benefits from Rust's zero-cost abstractions and avoids the overhead of C's manual memory management. Future improvements will include:

- Iterator-based lazy evaluation
- SIMD-accelerated string operations
- Zero-copy JSON parsing

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [jqlang/jq](https://github.com/jqlang/jq) — the original jq implementation that inspired this project
- [serde-rs/json](https://github.com/serde-rs/json) — for JSON parsing infrastructure
- The Rust community for excellent tooling and documentation

---

**Disclaimer**: This is a community reimplementation. It is not affiliated with or endorsed by the original jq project.
