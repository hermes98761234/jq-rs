# jq-rs 🦀

> A Rust reimplementation of [jq](https://github.com/jqlang/jq) — the lightweight and flexible command-line JSON processor.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![Build](https://img.shields.io/badge/build-passing-brightgreen.svg)]()
[![Release](https://img.shields.io/github/v/release/hermes98761234/jq-rs)](https://github.com/hermes98761234/jq-rs/releases)

## Overview

**jq-rs** is a from-scratch Rust implementation of the jq JSON query language. It provides a fast, safe, and modern alternative to the original C implementation, leveraging Rust's memory safety guarantees and zero-cost abstractions.

## Features

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
| `sort` / `sort_by(expr)` | ✅ | ✅ |
| `unique` | ✅ | ✅ |
| `min` / `max` / `min_by` / `max_by` | ✅ | ✅ |
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
| `def` (user-defined functions) | ✅ | ✅ |
| Regex (`test`, `match`, `capture`) | ✅ | ✅ |
| Colors/ANSI output | ✅ | ✅ |
| `inputs` / `input_filename` | ✅ | ✅ |
| Streaming parser / `--stream` | ✅ | ✅ |
| Module system (`import`/`include`) | ✅ | ✅ |

## Installation

### Download Binary (Recommended)

Download the pre-built binary for your platform from the [latest release](https://github.com/hermes98761234/jq-rs/releases/latest):

**Linux:**
```bash
curl -L https://github.com/hermes98761234/jq-rs/releases/latest/download/jq-rs-linux-x86_64 -o jq-rs
chmod +x jq-rs
sudo mv jq-rs /usr/local/bin/
```

**macOS:**
```bash
curl -L https://github.com/hermes98761234/jq-rs/releases/latest/download/jq-rs-macos-aarch64 -o jq-rs
chmod +x jq-rs
sudo mv jq-rs /usr/local/bin/
```

**Windows:**
Download `jq-rs-windows-x86_64.exe` from the [releases page](https://github.com/hermes98761234/jq-rs/releases/latest) and add it to your PATH.

### From Source

```bash
git clone https://github.com/hermes98761234/jq-rs.git
cd jq-rs
cargo build --release
# Binary at target/release/jq-rs (or jq-rs.exe on Windows)
```

Requirements: Rust 1.70+

## Usage

### Basic Examples

```bash
# Extract a field
echo '{"name":"john","age":30}' | jq-rs '.name'
# => "john"

# Array iteration
echo '[1,2,3]' | jq-rs '.[]'
# => 1
# => 2
# => 3

# Map transformation
echo '[1,2,3,4,5]' | jq-rs 'map(. * 2)'
# => [2, 4, 6, 8, 10]

# Filter with select
echo '[1,2,3,4,5]' | jq-rs '[.[] | select(. > 3)]'
# => [4, 5]

# Pipe operations
echo '{"items":["a","b","c"]}' | jq-rs '.items | length'
# => 3

# Raw string output
echo '{"name":"hello"}' | jq-rs -r '.name'
# => hello

# Compact output
echo '{"a":1,"b":2}' | jq-rs -c '.'
# => {"a":1,"b":2}
```

### User-Defined Functions

```bash
# Simple def
echo 'null' | jq-rs 'def double: . * 2; [1,2,3] | map(double)'
# => [2, 4, 6]

# Recursive function
echo 'null' | jq-rs 'def fact: if . <= 1 then 1 else . * ((. - 1) | fact) end; 5 | fact'
# => 120

# Function with filter argument
echo 'null' | jq-rs 'def apply(f): . | f; 5 | apply(. * 2)'
# => 10
```

### Regex

```bash
echo '"hello world"' | jq-rs 'test("wor")'
# => true

echo '"hello world"' | jq-rs 'match("(\\w+) (\\w+)")'
# => {"offset":0,"length":11,"string":"hello world","captures":[...]}

echo '"2024-01-15"' | jq-rs 'capture("(?P<year>\\d{4})-(?P<month>\\d{2})-(?P<day>\\d{2})")'
# => {"year":"2024","month":"01","day":"15"}
```

### I/O

```bash
# Collect all stdin values into array
printf '1\n2\n3\n' | jq-rs -n '[inputs]'
# => [1, 2, 3]

# Process multiple files
jq-rs '.name' a.json b.json c.json

# Get current filename
jq-rs 'input_filename' a.json b.json
```

### Streaming

```bash
# Output path/value events
echo '{"a":1,"b":2}' | jq-rs --stream '.'
# => [["a"],1]
# => [["b"],2]
# => [[],{"truncated":true}]
```

### Modules

```bash
# ~/.jq/mylib.jq:  def double: . * 2;
echo 'null' | jq-rs 'include "mylib"; [1,2,3] | map(double)'
# => [2, 4, 6]

# With namespace
# ~/.jq/math.jq:  def square: . * .;
echo 'null' | jq-rs 'import "math" as m; 5 | m::square'
# => 25
```

### Advanced Examples

```bash
# Reduce
echo '[1,2,3,4,5]' | jq-rs 'reduce .[] as $x (0; . + $x)'
# => 15

# Group by
echo '[{"t":"a"},{"t":"b"},{"t":"a"}]' | jq-rs 'group_by(.t)'

# Sort by field
echo '[{"n":3},{"n":1},{"n":2}]' | jq-rs 'sort_by(.n)'

# Object construction
echo '{"first":"John","last":"Doe"}' | jq-rs '{name: (.first + " " + .last)}'

# Conditional
echo '5' | jq-rs 'if . > 3 then "big" else "small" end'
```

## CLI Flags

| Flag | Description |
|------|-------------|
| `-c` | Compact output (no pretty-print) |
| `-r` | Raw output (strings without quotes) |
| `-s` | Slurp all inputs into an array |
| `-n` | Null input (use `inputs` to read stdin) |
| `-f <file>` | Read filter from a file |
| `-M` | Monochrome output (disable colors) |
| `--stream` | Output streaming path/value events |

## Architecture

```
src/
├── main.rs        # CLI entry point, argument parsing, I/O, color output
├── parser.rs      # Recursive descent parser for jq filter language
├── interpreter.rs # Tree-walking interpreter, Context (vars, fns, I/O state)
└── value.rs       # JSON value type with serde integration
```

## Building

```bash
cargo build           # debug
cargo build --release # optimized
cargo test            # run tests
```

## License

MIT — see [LICENSE](LICENSE).

## Acknowledgments

- [jqlang/jq](https://github.com/jqlang/jq) — the original jq implementation
- [serde-rs/json](https://github.com/serde-rs/json) — JSON parsing infrastructure
