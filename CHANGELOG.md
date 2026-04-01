# Changelog

All notable changes to tilth will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [0.5.7-security.1] (2026-04-01) — celstnblacc/tilth fork

### Security

* **path-traversal (P-1):** add `security::validate_path_mcp()` using `canonicalize()` + prefix check at all three MCP entry points (`tool_read` single, `tool_read` batch, `tool_edit`) — prevents reads/writes outside the project root
* **command-injection (P-2):** add `security::validate_pager()` with allow-list (`less`, `more`, `cat`, `bat`, `most`) + shell-metacharacter filter in `emit_output()` — malicious `$PAGER` values are rejected and fall back to `less`

### Added

* `src/security.rs` — new security module with 13 unit tests covering path traversal and pager injection scenarios
