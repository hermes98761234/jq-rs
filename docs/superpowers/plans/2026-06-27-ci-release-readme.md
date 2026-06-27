# CI Matrix + Release + README Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Update CI to build on Linux, macOS, and Windows; cut a v0.2.0 release with all 3 platform binaries; refresh README.md to reflect the full feature set.

**Architecture:** Three sequential tasks — each commits and pushes to master before the next starts.

**Tech Stack:** GitHub Actions, Rust 1.95, `gh` CLI, `softprops/action-gh-release@v2`.

## Global Constraints

- Work dir: `/home/user/projects/jq-rs`
- Branch: `master` — push with `git push origin master`
- GitHub remote: `https://github.com/hermes98761234/jq-rs`
- Never run the binary interactively
- Each task ends with git commit + push + verification

---

### Task A: Update CI for Cross-Platform Matrix Builds

**Files:**
- Modify: `.github/workflows/ci.yml` — replace `build` and `release` jobs with a 3-platform matrix

**Interfaces:**
- Produces: CI that builds `jq-rs` (Linux/Mac) and `jq-rs.exe` (Windows) natively on each runner
- Produces: GitHub release with 3 binary artifacts when a `v*` tag is pushed

- [ ] **Step 1: Replace `.github/workflows/ci.yml` with this complete file**

Write the **entire** file (overwrite — do not merge):

```yaml
name: CI

on:
  push:
    branches: [master, main]
    tags: ['v*']
  pull_request:
    branches: [master, main]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    name: Test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache cargo dependencies
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-

      - name: Check formatting
        run: cargo fmt -- --check

      - name: Run clippy
        run: cargo clippy -- -D warnings

      - name: Run tests
        run: cargo test --verbose

  build:
    name: Build (${{ matrix.os }})
    needs: test
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-latest
            artifact_name: jq-rs
            artifact_upload_name: jq-rs-linux-x86_64
          - os: macos-latest
            artifact_name: jq-rs
            artifact_upload_name: jq-rs-macos-aarch64
          - os: windows-latest
            artifact_name: jq-rs.exe
            artifact_upload_name: jq-rs-windows-x86_64.exe
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo dependencies
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-

      - name: Build release
        run: cargo build --release --verbose

      - name: Upload build artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.artifact_upload_name }}
          path: target/release/${{ matrix.artifact_name }}

  release:
    name: Release
    runs-on: ubuntu-latest
    needs: build
    if: startsWith(github.ref, 'refs/tags/v')
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts

      - name: List artifacts
        run: ls -R artifacts

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          files: |
            artifacts/jq-rs-linux-x86_64/jq-rs
            artifacts/jq-rs-macos-aarch64/jq-rs
            artifacts/jq-rs-windows-x86_64.exe/jq-rs.exe
          generate_release_notes: true
          fail_on_unmatched_files: true
```

- [ ] **Step 2: Verify the file looks correct**

```bash
cat /home/user/projects/jq-rs/.github/workflows/ci.yml | head -20
```

Expected: starts with `name: CI` and has `on: push:`.

- [ ] **Step 3: Commit and push**

```bash
cd /home/user/projects/jq-rs
git add .github/workflows/ci.yml
git commit -m "ci: add cross-platform matrix build for Linux, macOS, Windows"
git push origin master
```

- [ ] **Step 4: Watch CI run**

```bash
gh run list --repo hermes98761234/jq-rs --limit 3 2>&1
```

Wait ~3-5 minutes for the build matrix to complete (3 parallel builds). Poll:

```bash
gh run watch --repo hermes98761234/jq-rs 2>&1
```

Expected: `test` job passes, then all 3 `build` jobs pass (Linux, macOS, Windows). No `release` job triggers on a branch push.

If any build job fails, read the log:

```bash
gh run view <run-id> --log-failed --repo hermes98761234/jq-rs 2>&1 | head -80
```

Fix the failure, commit, and push. Repeat until all 3 pass.

- [ ] **Step 5: Report**

Report: "CI updated. Matrix builds green for Linux, macOS, and Windows. Workflow file at `.github/workflows/ci.yml`."

---

### Task B: Bump Version to v0.2.0 and Create Release

**Files:**
- Modify: `Cargo.toml` — version `0.0.1` → `0.2.0`
- Modify: `Cargo.lock` — regenerated by `cargo build`

**Interfaces:**
- Consumes: green CI from Task A (3-platform matrix builds)
- Produces: `v0.2.0` GitHub release with 3 binary artifacts

- [ ] **Step 1: Bump version in `Cargo.toml`**

In `/home/user/projects/jq-rs/Cargo.toml`, find:

```toml
version = "0.0.1"
```

Replace with:

```toml
version = "0.2.0"
```

- [ ] **Step 2: Regenerate Cargo.lock**

```bash
cd /home/user/projects/jq-rs && cargo build 2>&1 | tail -5
```

Expected: `Compiling jq-rs v0.2.0` and `Finished` with no errors.

- [ ] **Step 3: Commit and push**

```bash
cd /home/user/projects/jq-rs
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to v0.2.0"
git push origin master
```

- [ ] **Step 4: Wait for CI to go green on master**

```bash
gh run watch --repo hermes98761234/jq-rs 2>&1
```

Expected: all 3 build matrix jobs pass.

- [ ] **Step 5: Tag and push v0.2.0**

```bash
cd /home/user/projects/jq-rs
git tag v0.2.0
git push origin v0.2.0
```

- [ ] **Step 6: Watch the release workflow**

```bash
gh run list --repo hermes98761234/jq-rs --limit 5 2>&1
gh run watch --repo hermes98761234/jq-rs 2>&1
```

Expected: `build` matrix completes for all 3 platforms, then `release` job runs and creates the GitHub release.

- [ ] **Step 7: Verify the release**

```bash
gh release view v0.2.0 --repo hermes98761234/jq-rs 2>&1
```

Expected output contains:
- `jq-rs-linux-x86_64`
- `jq-rs-macos-aarch64`
- `jq-rs-windows-x86_64.exe`

If the release job failed, read the log:

```bash
gh run view <run-id> --log-failed --repo hermes98761234/jq-rs 2>&1 | head -80
```

Fix, delete the tag, re-tag, and push:

```bash
git tag -d v0.2.0
git push origin :refs/tags/v0.2.0
# fix the issue, commit, push, then re-tag:
git tag v0.2.0
git push origin v0.2.0
```

- [ ] **Step 8: Report**

Report: "v0.2.0 released at <GitHub release URL>. Artifacts: jq-rs-linux-x86_64, jq-rs-macos-aarch64, jq-rs-windows-x86_64.exe."

---

### Task C: Update README.md

**Files:**
- Modify: `README.md`

**Interfaces:**
- Consumes: v0.2.0 release URL from Task B

- [ ] **Step 1: Inspect current project state**

```bash
cat /home/user/projects/jq-rs/README.md
cat /home/user/projects/jq-rs/Cargo.toml | head -10
gh release view v0.2.0 --repo hermes98761234/jq-rs --json assets --jq '.assets[].name' 2>&1
```

- [ ] **Step 2: Write the updated README.md**

Write the **entire** `README.md` (full overwrite). Use this as the template, filling in the release URL from Task B:

```markdown
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
```

**Important:** After writing the file, verify it looks correct:

```bash
head -5 /home/user/projects/jq-rs/README.md
wc -l /home/user/projects/jq-rs/README.md
```

Expected: starts with `# jq-rs 🦀`, at least 150 lines.

- [ ] **Step 3: Commit and push**

```bash
cd /home/user/projects/jq-rs
git add README.md
git commit -m "docs: update README for v0.2.0 — all features, cross-platform install"
git push origin master
```

- [ ] **Step 4: Verify on GitHub**

```bash
gh repo view hermes98761234/jq-rs --web 2>/dev/null || echo "https://github.com/hermes98761234/jq-rs"
```

Open the URL and confirm the README renders correctly.

- [ ] **Step 5: Report**

Report: "README.md updated and pushed. All 6 new features documented. Cross-platform install instructions added for Linux, macOS, and Windows."
