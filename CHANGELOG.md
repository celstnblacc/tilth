# Changelog

All notable changes to tilth will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Changed
- Added `.serena/` to `.gitignore` (Serena MCP cache directory)

## [0.5.7-security.1] (2026-04-01) — celstnblacc/tilth fork

### Security

* **path-traversal (P-1):** add `security::validate_path_mcp()` using `canonicalize()` + prefix check at all three MCP entry points (`tool_read` single, `tool_read` batch, `tool_edit`) — prevents reads/writes outside the project root
* **command-injection (P-2):** add `security::validate_pager()` with allow-list (`less`, `more`, `cat`, `bat`, `most`) + shell-metacharacter filter in `emit_output()` — malicious `$PAGER` values are rejected and fall back to `less`

### Added

* `src/security.rs` — new security module with 13 unit tests covering path traversal and pager injection scenarios

## [0.6.1] - 2026-04-13

### Fixed
- **clippy (Rust 1.94):** resolve 9 lint warnings introduced by new clippy lints
  - `needless_raw_string_hashes`: drop `r#"…"#` → `r"…"` in 5 test string literals (none contain `"`)
  - `cast_lossless`: replace `as f64` with `f64::from()` in `bloom.rs`
  - `doc_markdown`: add backticks around `validate_path_in_scope` and `set_current_dir` in `security.rs`
- 2026-05-07: feat(install): add keylogger-mcp-wrapper support in tilth_command_and_args() — transparent MCP proxy logging controlled by KEYLOGGER_MCP env var
