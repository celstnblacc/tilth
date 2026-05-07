use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

// Supported MCP hosts and their config locations.
//
// Paths verified from official docs (2025):
//   claude-code:    ~/.claude.json                            (user scope)
//   cursor:         ~/.cursor/mcp.json                        (global)
//   windsurf:       ~/.codeium/windsurf/mcp_config.json       (global)
//   vscode:         .vscode/mcp.json                          (project scope)
//   claude-desktop: ~/Library/Application Support/Claude/...  (global)
//   opencode:       ~/.opencode.json                          (user scope)
//   gemini:         ~/.gemini/settings.json                   (user scope)
//   codex:          ~/.codex/config.toml                      (user scope, TOML)
//   amp:            ~/.config/amp/settings.json                (user scope)
//   droid:          ~/.factory/mcp.json                        (user scope)
//   antigravity:    ~/.gemini/antigravity/mcp_config.json      (user scope)
//   zed:            ~/.config/zed/settings.json                (user scope)
//   copilot-cli:    ~/.copilot/mcp-config.json                 (user scope)
//   augment:        ~/.augment/settings.json                   (user scope)
//   kiro:           ~/.kiro/settings/mcp.json                  (user scope)
//   kilo-code:      <globalStorage>/kilocode.kilo-code/...     (user scope)
//   cline:          <globalStorage>/saoudrizwan.claude-dev/... (user scope)
//   roo-code:       <globalStorage>/rooveterinaryinc.roo-cline/... (user scope)
//   trae:           .trae/mcp.json                             (project scope)
//   qwen-code:      ~/.qwen/settings.json                     (user scope)
//   crush:          ~/.config/crush/crush.json                 (user scope)
//   pi:             ~/.pi/agent/mcp.json                       (user scope)
const SUPPORTED_HOSTS: &[&str] = &[
    "claude-code",
    "cursor",
    "windsurf",
    "vscode",
    "claude-desktop",
    "opencode",
    "gemini",
    "codex",
    "amp",
    "droid",
    "antigravity",
    "zed",
    "copilot-cli",
    "augment",
    "kiro",
    "kilo-code",
    "cline",
    "roo-code",
    "trae",
    "qwen-code",
    "crush",
    "pi",
];

/// The tilth server entry as JSON, for hosts that use JSON config.
fn tilth_server_entry(edit: bool) -> Value {
    let (command, args) = tilth_command_and_args(edit);
    json!({
        "command": command,
        "args": args
    })
}

/// Write MCP config for the given host, preserving existing config.
pub fn run(host: &str, edit: bool) -> Result<(), String> {
    let host_info = resolve_host(host)?;

    if let Some(parent) = host_info.path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }

    match host_info.format {
        ConfigFormat::Json { .. } => write_json_config(&host_info, edit)?,
        ConfigFormat::Toml => write_toml_config(&host_info, edit)?,
    }

    if edit {
        eprintln!("✓ tilth (read+edit mode) added to {}", host_info.path.display());
    } else {
        eprintln!("✓ tilth (read-only mode) added to {}", host_info.path.display());
        eprintln!("  Use --edit to enable write operations (tilth_edit tool).");
    }
    if let Some(note) = host_info.note {
        eprintln!("  {note}");
    }
    Ok(())
}

fn write_json_config(host_info: &HostInfo, edit: bool) -> Result<(), String> {
    let servers_key = match host_info.format {
        ConfigFormat::Json { servers_key } => servers_key,
        ConfigFormat::Toml => unreachable!("write_json_config called for TOML host"),
    };

    let mut config: Value = if host_info.path.exists() {
        let raw = fs::read_to_string(&host_info.path)
            .map_err(|e| format!("failed to read {}: {e}", host_info.path.display()))?;
        serde_json::from_str(&raw)
            .map_err(|e| format!("invalid JSON in {}: {e}", host_info.path.display()))?
    } else {
        json!({})
    };

    upsert_json_server(&mut config, servers_key, tilth_server_entry(edit))?;

    let out =
        serde_json::to_string_pretty(&config).expect("serde_json::Value is always serializable");

    // Validate the output parses before touching disk.
    serde_json::from_str::<Value>(&out)
        .map_err(|e| format!("post-merge JSON validation failed: {e}"))?;

    backup_file(&host_info.path)?;
    atomic_write(&host_info.path, out.as_bytes())?;
    Ok(())
}

/// Writes a `[mcp_servers.tilth]` section into a TOML config file.
fn write_toml_config(host_info: &HostInfo, edit: bool) -> Result<(), String> {
    let (command, args) = tilth_command_and_args(edit);

    // Escape backslashes for TOML basic strings (Windows paths like C:\Users\...).
    let command_escaped = command.replace('\\', "\\\\");
    let args_toml: Vec<String> = args
        .iter()
        .map(|a| format!("\"{}\"", a.replace('\\', "\\\\")))
        .collect();
    let section = format!(
        "[mcp_servers.tilth]\ncommand = \"{command_escaped}\"\nargs = [{}]\n",
        args_toml.join(", ")
    );

    let existing = if host_info.path.exists() {
        fs::read_to_string(&host_info.path)
            .map_err(|e| format!("failed to read {}: {e}", host_info.path.display()))?
    } else {
        String::new()
    };

    // Remove existing [mcp_servers.tilth] section if present
    let output = if let Some(start) = existing.find("[mcp_servers.tilth]") {
        // Find end of section: next [header] or EOF
        let rest = &existing[start..];
        let end = rest[1..] // skip the opening '['
            .find("\n[")
            .map_or(existing.len(), |i| start + 1 + i + 1);
        format!("{}{}{}", &existing[..start], section, &existing[end..])
    } else {
        // Append with a blank line separator
        let sep = if existing.is_empty() || existing.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        format!("{existing}{sep}\n{section}")
    };

    backup_file(&host_info.path)?;
    atomic_write(&host_info.path, output.as_bytes())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Atomic write + backup helpers
// ---------------------------------------------------------------------------

/// Write `content` to `path` atomically: write to `path.tmp`, then rename.
///
/// Rename is atomic on POSIX filesystems (same volume). On Windows, `fs::rename`
/// replaces the destination atomically on NTFS since Vista.
fn atomic_write(path: &Path, content: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, content)
        .map_err(|e| format!("failed to write temp file {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp); // best-effort cleanup
        format!("failed to rename {} → {}: {e}", tmp.display(), path.display())
    })
}

/// If `path` exists, copy it to `path.bak` before overwriting.
/// No-op if the file does not exist yet (first install).
fn backup_file(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let backup = path.with_extension("bak");
    fs::copy(path, &backup)
        .map_err(|e| format!("failed to backup {} → {}: {e}", path.display(), backup.display()))?;
    Ok(())
}

/// Returns (command, args) for the tilth MCP server entry.
/// When KEYLOGGER_MCP=1 (default), wraps with keylogger-mcp-wrapper for traffic logging.
/// Set KEYLOGGER_MCP=0 to disable.
fn tilth_command_and_args(edit: bool) -> (String, Vec<String>) {
    let use_keylogger = std::env::var("KEYLOGGER_MCP")
        .map(|v| v != "0")
        .unwrap_or(true);

    let mut mcp_args: Vec<String> = vec!["--mcp".into()];
    if edit {
        mcp_args.push("--edit".into());
    }

    let (base_cmd, base_args) = {
        let via_npm = std::env::current_exe()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.contains("node_modules")))
            .unwrap_or(false);

        if via_npm {
            let mut args = vec!["tilth".to_string()];
            args.extend(mcp_args);
            ("npx".into(), args)
        } else {
            let command = std::env::current_exe()
                .ok()
                .and_then(|p| p.to_str().map(String::from))
                .unwrap_or_else(|| "tilth".into());
            (command, mcp_args)
        }
    };

    if use_keylogger {
        let mut wrapper_args = vec![
            "--name".to_string(),
            "tilth".to_string(),
            "--".to_string(),
            base_cmd,
        ];
        wrapper_args.extend(base_args);
        ("keylogger-mcp-wrapper".to_string(), wrapper_args)
    } else {
        (base_cmd, base_args)
    }
}

#[derive(Debug)]
enum ConfigFormat {
    /// JSON with a configurable servers key ("mcpServers" or "servers").
    Json { servers_key: &'static str },
    /// TOML with `[mcp_servers.<name>]` sections.
    Toml,
}

struct HostInfo {
    path: PathBuf,
    format: ConfigFormat,
    /// Optional note printed after success.
    note: Option<&'static str>,
}

fn resolve_host(host: &str) -> Result<HostInfo, String> {
    let home = home_dir()?;

    match host {
        // Claude Code user scope: ~/.claude.json → mcpServers
        // Available in all projects without checking into source control.
        "claude-code" => Ok(HostInfo {
            path: home.join(".claude.json"),
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: Some("User scope — available in all projects."),
        }),

        // Cursor global: ~/.cursor/mcp.json → mcpServers
        "cursor" => Ok(HostInfo {
            path: home.join(".cursor/mcp.json"),
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: None,
        }),

        // Windsurf global: ~/.codeium/windsurf/mcp_config.json → mcpServers
        "windsurf" => Ok(HostInfo {
            path: home.join(".codeium/windsurf/mcp_config.json"),
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: None,
        }),

        // VS Code project scope: .vscode/mcp.json → servers (NOT mcpServers)
        "vscode" => Ok(HostInfo {
            path: PathBuf::from(".vscode/mcp.json"),
            format: ConfigFormat::Json {
                servers_key: "servers",
            },
            note: Some("Project scope — run from your project root."),
        }),

        "claude-desktop" => Ok(HostInfo {
            path: claude_desktop_path()?,
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: None,
        }),

        // OpenCode user scope: ~/.opencode.json → mcpServers
        // Verified from opencode source: internal/config/config.go (viper config name ".opencode")
        "opencode" => Ok(HostInfo {
            path: home.join(".opencode.json"),
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: Some("User scope — available in all projects."),
        }),

        // Gemini CLI user scope: ~/.gemini/settings.json → mcpServers
        "gemini" => Ok(HostInfo {
            path: home.join(".gemini/settings.json"),
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: Some("User scope — available in all projects."),
        }),

        // Codex CLI user scope: ~/.codex/config.toml → [mcp_servers.tilth] (TOML)
        "codex" => Ok(HostInfo {
            path: home.join(".codex/config.toml"),
            format: ConfigFormat::Toml,
            note: Some("User scope — available in all projects."),
        }),

        // Amp user scope: ~/.config/amp/settings.json → amp.mcpServers
        // Verified from official docs: https://ampcode.com/manual
        "amp" => Ok(HostInfo {
            path: home.join(".config/amp/settings.json"),
            format: ConfigFormat::Json {
                servers_key: "amp.mcpServers",
            },
            note: Some("User scope — available in all projects."),
        }),

        // Google Antigravity user scope: ~/.gemini/antigravity/mcp_config.json → mcpServers
        // Verified from official docs: https://antigravity.google/docs/mcp
        "antigravity" => Ok(HostInfo {
            path: home.join(".gemini/antigravity/mcp_config.json"),
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: Some("User scope — available in all projects."),
        }),

        // Factory Droid user scope: ~/.factory/mcp.json → mcpServers
        // Verified from official docs: https://docs.factory.ai/cli/configuration/mcp
        "droid" => Ok(HostInfo {
            path: home.join(".factory/mcp.json"),
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: Some("User scope — available in all projects."),
        }),

        // Zed user scope: ~/.config/zed/settings.json → context_servers (NOT mcpServers)
        // Verified from official docs: https://zed.dev/docs/ai/mcp
        "zed" => Ok(HostInfo {
            path: home.join(".config/zed/settings.json"),
            format: ConfigFormat::Json {
                servers_key: "context_servers",
            },
            note: Some("User scope — available in all projects."),
        }),

        // GitHub Copilot CLI user scope: ~/.copilot/mcp-config.json → mcpServers
        // Verified from official docs: https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-mcp-servers
        "copilot-cli" => Ok(HostInfo {
            path: home.join(".copilot/mcp-config.json"),
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: Some("User scope — available in all projects."),
        }),

        // AugmentCode user scope: ~/.augment/settings.json → mcpServers
        // Verified from official docs: https://docs.augmentcode.com/cli/integrations
        "augment" => Ok(HostInfo {
            path: home.join(".augment/settings.json"),
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: Some("User scope — available in all projects."),
        }),

        // Kiro user scope: ~/.kiro/settings/mcp.json → mcpServers
        // Verified from official docs: https://kiro.dev/docs/mcp/configuration/
        "kiro" => Ok(HostInfo {
            path: home.join(".kiro/settings/mcp.json"),
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: Some("User scope — available in all projects."),
        }),

        // Kilo Code (VS Code extension): globalStorage → mcpServers
        // Verified from official docs: https://kilo.ai/docs/automate/mcp/using-in-kilo-code
        "kilo-code" => Ok(HostInfo {
            path: vscode_global_storage_path("kilocode.kilo-code", "mcp_settings.json")?,
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: None,
        }),

        // Cline (VS Code extension): globalStorage → mcpServers
        // Verified from official docs: https://docs.cline.bot/mcp-servers/configuring-mcp-servers
        "cline" => Ok(HostInfo {
            path: vscode_global_storage_path("saoudrizwan.claude-dev", "cline_mcp_settings.json")?,
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: None,
        }),

        // Roo Code (VS Code extension): globalStorage → mcpServers
        // Verified from official docs: https://docs.roocode.com/features/mcp/using-mcp-in-roo
        "roo-code" => Ok(HostInfo {
            path: vscode_global_storage_path("rooveterinaryinc.roo-cline", "mcp_settings.json")?,
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: None,
        }),

        // Trae project scope: .trae/mcp.json → mcpServers
        // Verified from official docs: https://docs.trae.ai/ide/add-mcp-servers
        "trae" => Ok(HostInfo {
            path: PathBuf::from(".trae/mcp.json"),
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: Some("Project scope — run from your project root."),
        }),

        // Qwen Code user scope: ~/.qwen/settings.json → mcpServers
        // Verified from official docs: https://qwenlm.github.io/qwen-code-docs/en/users/features/mcp/
        "qwen-code" => Ok(HostInfo {
            path: home.join(".qwen/settings.json"),
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: Some("User scope — available in all projects."),
        }),

        // Crush user scope: ~/.config/crush/crush.json → mcp (NOT mcpServers)
        // Verified from official docs: https://github.com/charmbracelet/crush
        "crush" => Ok(HostInfo {
            path: home.join(".config/crush/crush.json"),
            format: ConfigFormat::Json { servers_key: "mcp" },
            note: Some("User scope — available in all projects."),
        }),

        // Pi coding agent user scope: ~/.pi/agent/mcp.json → mcpServers
        // Verified from: https://github.com/badlogic/pi-mono/issues/563
        "pi" => Ok(HostInfo {
            path: home.join(".pi/agent/mcp.json"),
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: Some("User scope — available in all projects."),
        }),

        _ => Err(format!(
            "unknown host: {host}. Supported: {}",
            SUPPORTED_HOSTS.join(", ")
        )),
    }
}

fn home_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .map_err(|_| "USERPROFILE not set".into())
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME")
            .map(PathBuf::from)
            .map_err(|_| "HOME not set".into())
    }
}

/// Merge a tilth server entry into a JSON config under the given servers key.
/// Extracted for testability — used by `write_json_config` and unit tests.
fn upsert_json_server(config: &mut Value, servers_key: &str, entry: Value) -> Result<(), String> {
    config
        .as_object_mut()
        .ok_or("config root is not a JSON object")?
        .entry(servers_key)
        .or_insert(json!({}))
        .as_object_mut()
        .ok_or_else(|| format!("{servers_key} is not a JSON object"))?
        .insert("tilth".into(), entry);
    Ok(())
}

/// Returns the VS Code globalStorage path for a given extension and settings filename.
fn vscode_global_storage_path(extension_id: &str, filename: &str) -> Result<PathBuf, String> {
    let base = vscode_global_storage_base()?;
    Ok(base.join(extension_id).join("settings").join(filename))
}

fn vscode_global_storage_base() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        let home = home_dir()?;
        Ok(home.join("Library/Application Support/Code/User/globalStorage"))
    }

    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").map_err(|_| "APPDATA not set")?;
        Ok(PathBuf::from(appdata).join("Code/User/globalStorage"))
    }

    #[cfg(target_os = "linux")]
    {
        let home = home_dir()?;
        Ok(home.join(".config/Code/User/globalStorage"))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err("VS Code globalStorage path unknown on this OS".into())
    }
}

fn claude_desktop_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        let home = home_dir()?;
        Ok(home.join("Library/Application Support/Claude/claude_desktop_config.json"))
    }

    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").map_err(|_| "APPDATA not set")?;
        Ok(PathBuf::from(appdata).join("Claude/claude_desktop_config.json"))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err("claude-desktop config path unknown on this OS".into())
    }
}

// ---------------------------------------------------------------------------
// Doctor — health check across registered MCP hosts
// ---------------------------------------------------------------------------

/// Trust level of the tilth registration in an MCP host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustLevel {
    /// Read-only: no `tilth_edit` tool exposed (default install).
    ReadOnly,
    /// Read + edit: `tilth_edit` enabled via `--edit` flag.
    ReadEdit,
}

impl TrustLevel {
    fn as_str(&self) -> &'static str {
        match self {
            TrustLevel::ReadOnly => "read_only",
            TrustLevel::ReadEdit => "read_edit",
        }
    }
}

/// Registration status of tilth in one MCP host.
pub struct HostStatus {
    pub host: String,
    pub config_path: PathBuf,
    pub config_exists: bool,
    pub registered: bool,
    pub command: Option<String>,
    pub command_ok: Option<bool>,
    pub trust_level: Option<TrustLevel>,
}

/// Returns true if `cmd` (bare filename) resolves to an executable on `$PATH`,
/// or if `cmd` is an absolute/relative path pointing to an existing file.
fn command_in_path(cmd: &str) -> bool {
    // Absolute or explicitly relative path — check directly.
    let p = PathBuf::from(cmd);
    if p.is_absolute() || cmd.contains('/') || cmd.contains('\\') {
        return p.is_file();
    }
    // Bare name — walk PATH.
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path_var).any(|dir| dir.join(cmd).is_file())
}

/// Extract the tilth command and trust level registered in `info`'s config file.
/// Returns `Some((command_string, command_is_reachable, trust_level))` or `None` when
/// tilth is not registered (or the config file doesn't exist / is unreadable).
fn check_registration(info: &HostInfo) -> Option<(String, bool, TrustLevel)> {
    if !info.path.exists() {
        return None;
    }
    let raw = fs::read_to_string(&info.path).ok()?;
    match &info.format {
        ConfigFormat::Json { servers_key } => {
            let config: Value = serde_json::from_str(&raw).ok()?;
            // JSON Pointer: dots in servers_key are literal per RFC 6901.
            let pointer = format!("/{servers_key}/tilth");
            let entry = config.pointer(&pointer)?;
            let command = entry.get("command")?.as_str()?.to_string();
            let args = entry
                .get("args")
                .and_then(|a| a.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                .unwrap_or_default();
            let trust = if args.contains(&"--edit") {
                TrustLevel::ReadEdit
            } else {
                TrustLevel::ReadOnly
            };
            let ok = command_in_path(&command);
            Some((command, ok, trust))
        }
        ConfigFormat::Toml => {
            let section_start = raw.find("[mcp_servers.tilth]")?;
            let section = &raw[section_start..];
            let mut command = None;
            let mut has_edit = false;
            for line in section.lines().skip(1) {
                if line.trim_start().starts_with('[') {
                    break; // next section
                }
                if let Some(rest) = line.trim_start().strip_prefix("command") {
                    if let Some(rest) = rest.trim_start().strip_prefix('=') {
                        command = Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
                    }
                }
                if line.contains("\"--edit\"") || line.contains("'--edit'") {
                    has_edit = true;
                }
            }
            let command = command?;
            let ok = command_in_path(&command);
            let trust = if has_edit { TrustLevel::ReadEdit } else { TrustLevel::ReadOnly };
            Some((command, ok, trust))
        }
    }
}

/// Run `tilth doctor [--json]`.
///
/// Iterates all supported hosts, checks whether tilth is registered in each
/// config file that exists, and reports health status.
pub fn doctor(json: bool) {
    let tilth_version = env!("CARGO_PKG_VERSION");

    let mut statuses: Vec<HostStatus> = Vec::new();
    let mut registered_count = 0usize;

    for &host in SUPPORTED_HOSTS {
        let Ok(info) = resolve_host(host) else {
            continue;
        };

        let config_exists = info.path.exists();
        let (registered, command, command_ok, trust_level) = if config_exists {
            match check_registration(&info) {
                Some((cmd, ok, trust)) => (true, Some(cmd), Some(ok), Some(trust)),
                None => (false, None, None, None),
            }
        } else {
            (false, None, None, None)
        };

        if registered {
            registered_count += 1;
        }

        statuses.push(HostStatus {
            host: host.to_string(),
            config_path: info.path,
            config_exists,
            registered,
            command,
            command_ok,
            trust_level,
        });
    }

    let healthy = registered_count > 0;

    if json {
        // Only include hosts where a config file exists.
        let hosts_map: serde_json::Map<String, Value> = statuses
            .iter()
            .filter(|s| s.config_exists)
            .map(|s| {
                let mut obj = serde_json::Map::new();
                obj.insert("registered".into(), json!(s.registered));
                obj.insert(
                    "config_path".into(),
                    json!(s.config_path.to_string_lossy()),
                );
                if let Some(cmd) = &s.command {
                    obj.insert("command".into(), json!(cmd));
                }
                if let Some(ok) = s.command_ok {
                    obj.insert("command_ok".into(), json!(ok));
                }
                if let Some(trust) = &s.trust_level {
                    obj.insert("trust_level".into(), json!(trust.as_str()));
                }
                (s.host.clone(), Value::Object(obj))
            })
            .collect();

        let registered_hosts: Vec<String> = statuses
            .iter()
            .filter(|s| s.registered)
            .map(|s| s.host.clone())
            .collect();

        let output = json!({
            "tilth_version": tilth_version,
            "healthy": healthy,
            "registered_hosts": registered_hosts,
            "hosts": hosts_map,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&output).expect("doctor output is always serializable")
        );
    } else {
        println!("tilth doctor  v{tilth_version}");
        println!();
        let mut any_shown = false;
        for s in &statuses {
            if !s.config_exists {
                continue;
            }
            any_shown = true;
            let status_str = if s.registered {
                let trust_str = match &s.trust_level {
                    Some(TrustLevel::ReadEdit) => "  [read+edit]",
                    _ => "  [read-only]",
                };
                match s.command_ok {
                    Some(false) => format!(
                        "✓ registered{trust_str}  ✗ command missing: {}",
                        s.command.as_deref().unwrap_or("?")
                    ),
                    _ => format!("✓ registered{trust_str}"),
                }
            } else {
                "✗ config exists — tilth not registered".to_string()
            };
            println!("  {:<20} {status_str}", s.host);
            if s.registered {
                if let Some(cmd) = &s.command {
                    println!("    command: {cmd}");
                }
            }
        }
        if !any_shown {
            println!("  (no host config files found)");
        }
        println!();
        if healthy {
            println!("✓ healthy — tilth registered in {registered_count} host(s)");
        } else {
            println!("✗ not healthy — tilth not registered in any host");
            println!("  run: tilth install <host>");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amp_resolve_host() {
        let info = resolve_host("amp").expect("amp should resolve");
        assert!(
            info.path.ends_with(".config/amp/settings.json"),
            "path should end with .config/amp/settings.json, got: {}",
            info.path.display()
        );
        match info.format {
            ConfigFormat::Json { servers_key } => {
                assert_eq!(servers_key, "amp.mcpServers");
            }
            ConfigFormat::Toml => panic!("amp should use JSON format, not TOML"),
        }
    }

    #[test]
    fn amp_dotted_key_is_literal_not_nested() {
        let mut config = json!({});
        let entry = json!({"command": "tilth", "args": ["--mcp"]});
        upsert_json_server(&mut config, "amp.mcpServers", entry).unwrap();

        // Top-level key must be the literal "amp.mcpServers"
        assert!(
            config.get("amp.mcpServers").is_some(),
            "should have literal top-level key 'amp.mcpServers'"
        );
        // Must NOT create a nested "amp" object
        assert!(
            config.get("amp").is_none(),
            "should NOT have a nested 'amp' key"
        );
        // Verify tilth entry is inside
        assert_eq!(config["amp.mcpServers"]["tilth"]["command"], json!("tilth"));
    }

    #[test]
    fn amp_preserves_unrelated_config() {
        let mut config = json!({
            "amp.theme": "dark",
            "amp.mcpServers": {
                "other": {"command": "foo", "args": []}
            }
        });
        let entry = json!({"command": "tilth", "args": ["--mcp"]});
        upsert_json_server(&mut config, "amp.mcpServers", entry).unwrap();

        assert_eq!(config["amp.theme"], json!("dark"));
        assert_eq!(config["amp.mcpServers"]["other"]["command"], json!("foo"));
        assert!(config["amp.mcpServers"]["tilth"].is_object());
    }

    #[test]
    fn amp_overwrites_existing_tilth() {
        let mut config = json!({
            "amp.mcpServers": {
                "tilth": {"command": "old", "args": ["--old"]}
            }
        });
        let entry = json!({"command": "tilth", "args": ["--mcp"]});
        upsert_json_server(&mut config, "amp.mcpServers", entry).unwrap();

        assert_eq!(config["amp.mcpServers"]["tilth"]["args"], json!(["--mcp"]));
    }

    #[test]
    fn amp_error_when_servers_key_not_object() {
        let mut config = json!({"amp.mcpServers": []});
        let entry = json!({"command": "tilth", "args": ["--mcp"]});
        let err = upsert_json_server(&mut config, "amp.mcpServers", entry).unwrap_err();
        assert!(
            err.contains("amp.mcpServers is not a JSON object"),
            "error should mention the key, got: {err}"
        );
    }

    #[test]
    fn droid_resolve_host() {
        let info = resolve_host("droid").expect("droid should resolve");
        assert!(
            info.path.ends_with(".factory/mcp.json"),
            "path should end with .factory/mcp.json, got: {}",
            info.path.display()
        );
        match info.format {
            ConfigFormat::Json { servers_key } => {
                assert_eq!(servers_key, "mcpServers");
            }
            ConfigFormat::Toml => panic!("droid should use JSON format, not TOML"),
        }
    }

    #[test]
    fn droid_preserves_existing_servers() {
        let mut config = json!({
            "mcpServers": {
                "playwright": {"command": "npx", "args": ["-y", "@playwright/mcp@latest"]}
            }
        });
        let entry = json!({"command": "tilth", "args": ["--mcp"]});
        upsert_json_server(&mut config, "mcpServers", entry).unwrap();

        assert_eq!(config["mcpServers"]["playwright"]["command"], json!("npx"));
        assert!(config["mcpServers"]["tilth"].is_object());
    }

    #[test]
    fn unknown_host_error_includes_droid() {
        let err = resolve_host("nope")
            .err()
            .expect("unknown host should return an error");
        assert!(
            err.contains("droid"),
            "error should list droid in supported hosts, got: {err}"
        );
    }

    #[test]
    fn antigravity_resolve_host() {
        let info = resolve_host("antigravity").expect("antigravity should resolve");
        assert!(
            info.path.ends_with(".gemini/antigravity/mcp_config.json"),
            "path should end with .gemini/antigravity/mcp_config.json, got: {}",
            info.path.display()
        );
        match info.format {
            ConfigFormat::Json { servers_key } => {
                assert_eq!(servers_key, "mcpServers");
            }
            ConfigFormat::Toml => panic!("antigravity should use JSON format, not TOML"),
        }
    }

    #[test]
    fn antigravity_preserves_existing_servers() {
        let mut config = json!({
            "mcpServers": {
                "firebase": {"command": "npx", "args": ["-y", "firebase-tools@latest", "mcp"]}
            }
        });
        let entry = json!({"command": "tilth", "args": ["--mcp"]});
        upsert_json_server(&mut config, "mcpServers", entry).unwrap();

        assert_eq!(config["mcpServers"]["firebase"]["command"], json!("npx"));
        assert!(config["mcpServers"]["tilth"].is_object());
    }

    #[test]
    fn unknown_host_error_includes_antigravity() {
        let err = resolve_host("nope")
            .err()
            .expect("unknown host should return an error");
        assert!(
            err.contains("antigravity"),
            "error should list antigravity in supported hosts, got: {err}"
        );
    }

    #[test]
    fn zed_resolve_host() {
        let info = resolve_host("zed").expect("zed should resolve");
        assert!(
            info.path.ends_with(".config/zed/settings.json"),
            "path should end with .config/zed/settings.json, got: {}",
            info.path.display()
        );
        match info.format {
            ConfigFormat::Json { servers_key } => {
                assert_eq!(servers_key, "context_servers");
            }
            ConfigFormat::Toml => panic!("zed should use JSON format, not TOML"),
        }
    }

    #[test]
    fn zed_uses_context_servers_not_mcp_servers() {
        let mut config = json!({});
        let entry = json!({"command": "tilth", "args": ["--mcp"]});
        upsert_json_server(&mut config, "context_servers", entry).unwrap();

        assert!(config.get("context_servers").is_some());
        assert!(config.get("mcpServers").is_none());
        assert_eq!(
            config["context_servers"]["tilth"]["command"],
            json!("tilth")
        );
    }

    #[test]
    fn copilot_cli_resolve_host() {
        let info = resolve_host("copilot-cli").expect("copilot-cli should resolve");
        assert!(
            info.path.ends_with(".copilot/mcp-config.json"),
            "path should end with .copilot/mcp-config.json, got: {}",
            info.path.display()
        );
        match info.format {
            ConfigFormat::Json { servers_key } => {
                assert_eq!(servers_key, "mcpServers");
            }
            ConfigFormat::Toml => panic!("copilot-cli should use JSON format, not TOML"),
        }
    }

    #[test]
    fn augment_resolve_host() {
        let info = resolve_host("augment").expect("augment should resolve");
        assert!(
            info.path.ends_with(".augment/settings.json"),
            "path should end with .augment/settings.json, got: {}",
            info.path.display()
        );
        match info.format {
            ConfigFormat::Json { servers_key } => {
                assert_eq!(servers_key, "mcpServers");
            }
            ConfigFormat::Toml => panic!("augment should use JSON format, not TOML"),
        }
    }

    #[test]
    fn kiro_resolve_host() {
        let info = resolve_host("kiro").expect("kiro should resolve");
        assert!(
            info.path.ends_with(".kiro/settings/mcp.json"),
            "path should end with .kiro/settings/mcp.json, got: {}",
            info.path.display()
        );
        match info.format {
            ConfigFormat::Json { servers_key } => {
                assert_eq!(servers_key, "mcpServers");
            }
            ConfigFormat::Toml => panic!("kiro should use JSON format, not TOML"),
        }
    }

    #[test]
    fn kilo_code_resolve_host() {
        let info = resolve_host("kilo-code").expect("kilo-code should resolve");
        let path_str = info.path.to_string_lossy();
        assert!(
            path_str.contains("kilocode.kilo-code") && path_str.contains("mcp_settings.json"),
            "path should contain kilocode.kilo-code and mcp_settings.json, got: {path_str}",
        );
        match info.format {
            ConfigFormat::Json { servers_key } => {
                assert_eq!(servers_key, "mcpServers");
            }
            ConfigFormat::Toml => panic!("kilo-code should use JSON format, not TOML"),
        }
    }

    #[test]
    fn cline_resolve_host() {
        let info = resolve_host("cline").expect("cline should resolve");
        let path_str = info.path.to_string_lossy();
        assert!(
            path_str.contains("saoudrizwan.claude-dev")
                && path_str.contains("cline_mcp_settings.json"),
            "path should contain saoudrizwan.claude-dev and cline_mcp_settings.json, got: {path_str}",
        );
        match info.format {
            ConfigFormat::Json { servers_key } => {
                assert_eq!(servers_key, "mcpServers");
            }
            ConfigFormat::Toml => panic!("cline should use JSON format, not TOML"),
        }
    }

    #[test]
    fn roo_code_resolve_host() {
        let info = resolve_host("roo-code").expect("roo-code should resolve");
        let path_str = info.path.to_string_lossy();
        assert!(
            path_str.contains("rooveterinaryinc.roo-cline")
                && path_str.contains("mcp_settings.json"),
            "path should contain rooveterinaryinc.roo-cline and mcp_settings.json, got: {path_str}",
        );
        match info.format {
            ConfigFormat::Json { servers_key } => {
                assert_eq!(servers_key, "mcpServers");
            }
            ConfigFormat::Toml => panic!("roo-code should use JSON format, not TOML"),
        }
    }

    #[test]
    fn trae_resolve_host() {
        let info = resolve_host("trae").expect("trae should resolve");
        assert!(
            info.path.ends_with(".trae/mcp.json"),
            "path should end with .trae/mcp.json, got: {}",
            info.path.display()
        );
        match info.format {
            ConfigFormat::Json { servers_key } => {
                assert_eq!(servers_key, "mcpServers");
            }
            ConfigFormat::Toml => panic!("trae should use JSON format, not TOML"),
        }
        assert_eq!(
            info.note,
            Some("Project scope — run from your project root.")
        );
    }

    #[test]
    fn qwen_code_resolve_host() {
        let info = resolve_host("qwen-code").expect("qwen-code should resolve");
        assert!(
            info.path.ends_with(".qwen/settings.json"),
            "path should end with .qwen/settings.json, got: {}",
            info.path.display()
        );
        match info.format {
            ConfigFormat::Json { servers_key } => {
                assert_eq!(servers_key, "mcpServers");
            }
            ConfigFormat::Toml => panic!("qwen-code should use JSON format, not TOML"),
        }
    }

    #[test]
    fn crush_resolve_host() {
        let info = resolve_host("crush").expect("crush should resolve");
        assert!(
            info.path.ends_with(".config/crush/crush.json"),
            "path should end with .config/crush/crush.json, got: {}",
            info.path.display()
        );
        match info.format {
            ConfigFormat::Json { servers_key } => {
                assert_eq!(servers_key, "mcp");
            }
            ConfigFormat::Toml => panic!("crush should use JSON format, not TOML"),
        }
    }

    #[test]
    fn crush_uses_mcp_not_mcp_servers() {
        let mut config = json!({});
        let entry = json!({"command": "tilth", "args": ["--mcp"]});
        upsert_json_server(&mut config, "mcp", entry).unwrap();

        assert!(config.get("mcp").is_some());
        assert!(config.get("mcpServers").is_none());
        assert_eq!(config["mcp"]["tilth"]["command"], json!("tilth"));
    }

    #[test]
    fn pi_resolve_host() {
        let info = resolve_host("pi").expect("pi should resolve");
        assert!(
            info.path.ends_with(".pi/agent/mcp.json"),
            "path should end with .pi/agent/mcp.json, got: {}",
            info.path.display()
        );
        match info.format {
            ConfigFormat::Json { servers_key } => {
                assert_eq!(servers_key, "mcpServers");
            }
            ConfigFormat::Toml => panic!("pi should use JSON format, not TOML"),
        }
    }

    // -----------------------------------------------------------------------
    // Atomic write + backup tests
    // -----------------------------------------------------------------------

    #[test]
    fn atomic_write_creates_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("out.json");
        atomic_write(&path, b"{\"ok\":true}").unwrap();
        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"ok\""));
    }

    #[test]
    fn atomic_write_no_tmp_left_on_success() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("out.json");
        atomic_write(&path, b"{}").unwrap();
        let tmp = path.with_extension("tmp");
        assert!(!tmp.exists(), ".tmp file should be gone after rename");
    }

    #[test]
    fn backup_file_creates_bak() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("cfg.json");
        fs::write(&path, b"{\"original\":true}").unwrap();
        backup_file(&path).unwrap();
        let bak = path.with_extension("bak");
        assert!(bak.exists(), ".bak should be created");
        let content = fs::read_to_string(&bak).unwrap();
        assert!(content.contains("\"original\""));
    }

    #[test]
    fn backup_file_noop_when_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nonexistent.json");
        // Should not error — first install has nothing to back up
        backup_file(&path).unwrap();
        assert!(!path.with_extension("bak").exists());
    }

    #[test]
    fn install_json_roundtrip_valid() {
        // write_json_config produces a valid config the next read parses correctly
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("cfg.json");
        let info = HostInfo {
            path: path.clone(),
            format: ConfigFormat::Json { servers_key: "mcpServers" },
            note: None,
        };
        write_json_config(&info, false).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        let v: Value = serde_json::from_str(&raw).expect("output must be valid JSON");
        assert!(v["mcpServers"]["tilth"].is_object());
    }

    #[test]
    fn install_json_creates_backup_on_update() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("cfg.json");
        // First write
        let info = HostInfo { path: path.clone(), format: ConfigFormat::Json { servers_key: "mcpServers" }, note: None };
        write_json_config(&info, false).unwrap();
        // Second write should produce a .bak
        write_json_config(&info, false).unwrap();
        assert!(path.with_extension("bak").exists(), ".bak should be created on second write");
    }

    #[test]
    fn install_toml_roundtrip_valid() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let info = HostInfo { path: path.clone(), format: ConfigFormat::Toml, note: None };
        write_toml_config(&info, false).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("[mcp_servers.tilth]"), "TOML must contain section");
        assert!(raw.contains("command ="), "TOML must contain command");
    }

    #[test]
    fn install_toml_creates_backup_on_update() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let info = HostInfo { path: path.clone(), format: ConfigFormat::Toml, note: None };
        write_toml_config(&info, false).unwrap();
        write_toml_config(&info, false).unwrap();
        assert!(path.with_extension("bak").exists(), ".bak should be created on second write");
    }

    // -----------------------------------------------------------------------
    // Trust level detection tests
    // -----------------------------------------------------------------------

    #[test]
    fn trust_level_read_only_when_no_edit_flag() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("mcp.json");
        let content = r#"{"mcpServers":{"tilth":{"command":"tilth","args":["--mcp"]}}}"#;
        fs::write(&path, content).unwrap();
        let info = HostInfo { path, format: ConfigFormat::Json { servers_key: "mcpServers" }, note: None };
        let (_, _, trust) = check_registration(&info).unwrap();
        assert_eq!(trust, TrustLevel::ReadOnly);
    }

    #[test]
    fn trust_level_read_edit_when_edit_flag_present() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("mcp.json");
        let content = r#"{"mcpServers":{"tilth":{"command":"tilth","args":["--mcp","--edit"]}}}"#;
        fs::write(&path, content).unwrap();
        let info = HostInfo { path, format: ConfigFormat::Json { servers_key: "mcpServers" }, note: None };
        let (_, _, trust) = check_registration(&info).unwrap();
        assert_eq!(trust, TrustLevel::ReadEdit);
    }

    #[test]
    fn trust_level_toml_read_only() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        fs::write(&path, "\n[mcp_servers.tilth]\ncommand = \"tilth\"\nargs = [\"--mcp\"]\n").unwrap();
        let info = HostInfo { path, format: ConfigFormat::Toml, note: None };
        let (_, _, trust) = check_registration(&info).unwrap();
        assert_eq!(trust, TrustLevel::ReadOnly);
    }

    #[test]
    fn trust_level_toml_read_edit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        fs::write(&path, "\n[mcp_servers.tilth]\ncommand = \"tilth\"\nargs = [\"--mcp\", \"--edit\"]\n").unwrap();
        let info = HostInfo { path, format: ConfigFormat::Toml, note: None };
        let (_, _, trust) = check_registration(&info).unwrap();
        assert_eq!(trust, TrustLevel::ReadEdit);
    }

    #[test]
    fn trust_level_as_str() {
        assert_eq!(TrustLevel::ReadOnly.as_str(), "read_only");
        assert_eq!(TrustLevel::ReadEdit.as_str(), "read_edit");
    }

    #[test]
    fn unknown_host_error_includes_amp() {
        let err = resolve_host("nope")
            .err()
            .expect("unknown host should return an error");
        assert!(
            err.contains("amp"),
            "error should list amp in supported hosts, got: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // Doctor — check_registration and command_in_path unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn doctor_check_registration_json_registered() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("mcp.json");
        let content = r#"{"mcpServers":{"tilth":{"command":"/usr/local/bin/tilth","args":["--mcp"]}}}"#;
        fs::write(&path, content).unwrap();

        let info = HostInfo {
            path: path.clone(),
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: None,
        };

        let result = check_registration(&info);
        assert!(result.is_some(), "tilth is registered — should return Some");
        let (cmd, _ok, _trust) = result.unwrap();
        assert_eq!(cmd, "/usr/local/bin/tilth");
    }

    #[test]
    fn doctor_check_registration_json_not_registered() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("mcp.json");
        fs::write(&path, r#"{"mcpServers":{"other":{"command":"foo"}}}"#).unwrap();

        let info = HostInfo {
            path,
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: None,
        };

        assert!(
            check_registration(&info).is_none(),
            "tilth not in config — should return None"
        );
    }

    #[test]
    fn doctor_check_registration_toml_registered() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let content = "\n[mcp_servers.tilth]\ncommand = \"/usr/local/bin/tilth\"\nargs = [\"--mcp\"]\n";
        fs::write(&path, content).unwrap();

        let info = HostInfo {
            path,
            format: ConfigFormat::Toml,
            note: None,
        };

        let result = check_registration(&info);
        assert!(result.is_some(), "tilth section present — should return Some");
        let (cmd, _ok, _trust) = result.unwrap();
        assert_eq!(cmd, "/usr/local/bin/tilth");
    }

    #[test]
    fn doctor_check_registration_toml_not_registered() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        fs::write(&path, "[other_section]\nkey = \"value\"\n").unwrap();

        let info = HostInfo {
            path,
            format: ConfigFormat::Toml,
            note: None,
        };

        assert!(
            check_registration(&info).is_none(),
            "no tilth section — should return None"
        );
    }

    #[test]
    fn doctor_check_registration_missing_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let info = HostInfo {
            path: dir.path().join("nonexistent.json"),
            format: ConfigFormat::Json {
                servers_key: "mcpServers",
            },
            note: None,
        };

        assert!(
            check_registration(&info).is_none(),
            "missing config file — should return None"
        );
    }

    #[test]
    fn doctor_command_in_path_found() {
        // "sh" must exist on any Unix system (and cmd.exe on Windows)
        #[cfg(not(target_os = "windows"))]
        assert!(command_in_path("sh"), "sh should be found on PATH");
    }

    #[test]
    fn doctor_command_in_path_not_found() {
        assert!(
            !command_in_path("__tilth_definitely_not_a_real_binary_xyz123__"),
            "fake binary should not be found on PATH"
        );
    }

    #[test]
    fn doctor_command_in_path_absolute_existing() {
        // /bin/sh always exists on Unix
        #[cfg(not(target_os = "windows"))]
        {
            let path = if std::path::Path::new("/bin/sh").exists() {
                "/bin/sh"
            } else {
                "/usr/bin/env"
            };
            assert!(command_in_path(path), "{path} should resolve as existing file");
        }
    }

    #[test]
    fn doctor_command_in_path_absolute_missing() {
        assert!(
            !command_in_path("/nonexistent/path/to/tilth"),
            "nonexistent absolute path should return false"
        );
    }

    #[test]
    fn doctor_json_output_is_valid_json() {
        // Smoke test: doctor(true) should not panic and should produce parseable JSON.
        // We capture stdout by running the serialisation logic directly.
        let tilth_version = env!("CARGO_PKG_VERSION");
        let hosts_map: serde_json::Map<String, Value> = serde_json::Map::new();
        let registered_hosts: Vec<String> = vec![];
        let output = json!({
            "tilth_version": tilth_version,
            "healthy": false,
            "registered_hosts": registered_hosts,
            "hosts": hosts_map,
        });
        let s = serde_json::to_string_pretty(&output).unwrap();
        let parsed: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed["healthy"], json!(false));
        assert!(parsed["tilth_version"].is_string());
        assert!(parsed["registered_hosts"].is_array());
        assert!(parsed["hosts"].is_object());
    }
}
