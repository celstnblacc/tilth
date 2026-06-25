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
- 2026-05-08: detect MCP-host Stop-hook kill regressions — log clear diagnostic on SIGTERM/SIGHUP within 60s of startup (points at `verify-mcp-stop-hook` and ~/.claude/settings.json hooks.Stop)

## [0.6.2] - 2026-05-08

### Added
- MCP-host Stop-hook kill diagnostic: on SIGTERM/SIGHUP within 60s of startup, log a clear warning pointing at `verify-mcp-stop-hook` and the offending `~/.claude/settings.json` `hooks.Stop` entry. Helps detect regressions where the MCP host (e.g. Claude Code) `Stop` hook pkills MCP children every assistant turn.
- `signal-hook = "0.3"` dependency for SIGTERM/SIGHUP handling.

## [0.7.0] - 2026-05-08

### Removed (BREAKING)
- `KEYLOGGER_MCP` env var support in `src/install.rs`. `tilth install <host>` no longer wraps tilth's command with `keylogger-mcp-wrapper`. The wrapping responsibility now lives in keylogger-mcp itself (v0.2.0+), where it belongs.

### Migration
If you previously relied on the default-on wrapping (i.e. you ran `tilth install <host>` with `KEYLOGGER_MCP` unset or `=1` and expected MCP traffic to be logged), the replacement is one command:

    keylogger-mcp wrap <host> tilth

Reverse with:

    keylogger-mcp unwrap <host> tilth

See `keylogger-mcp status` for current state across all hosts.

### Why
Tilth had no business knowing keylogger existed. The coupling (env-var read in tilth's installer, hardcoded `keylogger-mcp-wrapper` command path) made `tilth install` non-idempotent and silently broken on machines without keylogger on PATH. With v0.7.0 tilth installs only tilth, and users who want logging point keylogger-mcp at the servers they care about.

## [0.8.0] - 2026-05-08

### Changed (BREAKING — JSON output shape)
- `tilth doctor` reimplemented as a typed report. The merged design unifies the previous ad-hoc `install::doctor` (host registration walker) with the parked `doctor.rs` redesign (binary/edit-mode/scope checks). One command, six checks: `binary`, `mcp_hosts`, `command_ok`, `trust_level`, `scope`, `edit_mode`.
- JSON output shape changed from `{tilth_version, healthy, registered_hosts, hosts: {<host>: {...}}}` to `{overall: "pass|warn|fail", checks: [{name, status, detail}, ...]}`. Anyone scripting against the old shape needs to update.
- New module: `src/doctor.rs`. Public API: `tilth::doctor::run(json)`, `build_report()`, `DoctorReport`, `DoctorCheck`, `CheckStatus`.
- `pub fn doctor` removed from `src/install.rs`. Trust-level / host-registration helpers (`resolve_host`, `check_registration`, `SUPPORTED_HOSTS`, `HostInfo`, `ConfigFormat`) bumped to `pub(crate)` so the new doctor module can use them.
- `main.rs` `Command::Doctor` route now calls `tilth::doctor::run(json)`. Exit code 1 on overall=fail.
- 9 new unit tests in `src/doctor.rs::tests` covering CheckStatus serialization, DoctorCheck JSON shape, overall aggregation rules, and stable JSON schema.

### Migration
The human-readable text output looks similar but with a different layout. JSON consumers must switch:

```diff
- jq '.healthy'
+ jq '.overall == "pass"'

- jq '.registered_hosts'
+ jq '[.checks[] | select(.name == "mcp_hosts").detail]'
```

### Why
The merged design closes a months-old design gap. Two parallel doctor implementations existed — the shipped `install::doctor` and a 444-line untracked `doctor.rs` — that checked different things. Users got whichever one was wired into the CLI, with no way to discover the other's checks. v0.8.0 unifies them behind one stable typed shape so future checks slot in cleanly.

- 2026-06-25: chore: remove personal workspace path from tracked files
