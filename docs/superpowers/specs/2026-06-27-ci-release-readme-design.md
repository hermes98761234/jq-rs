# CI Matrix + Release + README Design

**Date:** 2026-06-27  
**Scope:** Three sequential tasks — cross-platform CI, v0.2.0 release, README refresh

---

## Context

Current state:
- CI (`ci.yml`) builds and tests on Linux only
- Cargo.toml version: `0.0.1`
- GitHub remote: `https://github.com/hermes98761234/jq-rs`
- Branch: `master`
- 6 new features in progress (def, regex, colors, I/O, streaming, modules) — these tasks run after features land

---

## Task A: Update CI for Cross-Platform Matrix Builds

Replace the existing `build` and `release` jobs with a matrix strategy covering three native runners:

| Target | Runner | Binary name |
|--------|--------|-------------|
| Linux x86_64 | `ubuntu-latest` | `jq-rs-linux-x86_64` |
| macOS (Apple Silicon + x86) | `macos-latest` | `jq-rs-macos-aarch64` |
| Windows x86_64 | `windows-latest` | `jq-rs-windows-x86_64.exe` |

**Test job** stays Linux-only (fmt, clippy, tests). The `build` job becomes a matrix across all 3 runners. The `release` job downloads all 3 artifacts and attaches them to the GitHub release.

**Windows note:** binary name is `jq-rs.exe`, artifact path is `target/release/jq-rs.exe`. The upload step must handle this with a conditional or by using the `${{ matrix.artifact_name }}` variable.

---

## Task B: Create v0.2.0 Release

1. Update `Cargo.toml` version from `0.0.1` to `0.2.0`
2. Update `Cargo.lock` (`cargo build`)
3. Commit `chore: bump version to v0.2.0`
4. Push to master
5. Wait for CI green
6. Tag `v0.2.0` and push tag
7. Wait for release workflow; verify `gh release view v0.2.0` shows all 3 binaries

---

## Task C: Update README.md

Refresh the README to reflect the v0.2.0 feature set:
- Update the feature table: all 6 new features should show ✅
- Add install instructions for Linux, macOS, Windows (download from releases)
- Update the "Not Implemented" section to reflect what's left (module system edge cases, etc.)
- Ensure examples in usage section cover new features (def, regex, colors)
- Commit and push to master

---

## Task Sequence

```
T_ci: Update CI matrix (no parent — ready immediately)
 └─ T_release: Bump to v0.2.0 and create release
     └─ T_readme: Update README.md
```
