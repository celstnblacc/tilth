//! `tilth doctor` — health check for the tilth installation.
//!
//! Single command, typed report. Replaces the ad-hoc print-only doctor that
//! previously lived in install.rs and the parked-branch alternative that
//! checked PATH/edit-mode/scope but lacked host registration. Both check sets
//! now live behind one `DoctorReport` shape.
//!
//! Checks (in order):
//!   binary       — tilth binary on PATH
//!   mcp_hosts    — registered MCP host configs and their commands
//!   trust_level  — read-only vs read+edit per registered host
//!   command_ok   — registered command actually exists on disk / PATH
//!   scope        — current working directory is a readable directory
//!   edit_mode    — at least one host has --edit (informational)
//!
//! Overall = Fail if any Fail; else Warn if any Warn; else Pass.

use serde::Serialize;

use crate::install::{check_registration, resolve_host, TrustLevel, SUPPORTED_HOSTS};

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorCheck {
    pub name: &'static str,
    pub status: CheckStatus,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct DoctorReport {
    pub overall: CheckStatus,
    pub checks: Vec<DoctorCheck>,
}

// ── Individual checks ──────────────────────────────────────────────────────

/// Check that `tilth` is findable on PATH.
pub fn check_binary() -> DoctorCheck {
    let found = std::env::var_os("PATH")
        .map(|path_var| {
            std::env::split_paths(&path_var).any(|dir| {
                dir.join("tilth").is_file()
                    || dir.join("tilth.exe").is_file()
                    || dir.join("tilth.cmd").is_file()
            })
        })
        .unwrap_or(false);

    let exe = std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_string());

    if found {
        DoctorCheck {
            name: "binary",
            status: CheckStatus::Pass,
            detail: exe
                .map(|p| format!("`tilth` found — running from {p}"))
                .unwrap_or_else(|| "`tilth` found on PATH".to_string()),
        }
    } else {
        DoctorCheck {
            name: "binary",
            status: CheckStatus::Warn,
            detail:
                "`tilth` not found on PATH — add install dir to PATH or run `tilth install <host>`"
                    .to_string(),
        }
    }
}

/// Probe each known MCP host. Returns one row per host whose `resolve_host` succeeds.
fn probe_hosts() -> Vec<HostProbe> {
    let mut out = Vec::new();
    for &host in SUPPORTED_HOSTS {
        let Ok(info) = resolve_host(host) else {
            continue;
        };
        let registration = if info.path.exists() {
            check_registration(&info)
        } else {
            None
        };
        out.push(HostProbe { host, registration });
    }
    out
}

struct HostProbe {
    host: &'static str,
    registration: Option<(String, bool, TrustLevel)>,
}

/// Check which MCP hosts have tilth registered.
pub fn check_mcp_hosts() -> DoctorCheck {
    let probes = probe_hosts();
    let registered: Vec<&str> = probes
        .iter()
        .filter(|p| p.registration.is_some())
        .map(|p| p.host)
        .collect();
    if registered.is_empty() {
        DoctorCheck {
            name: "mcp_hosts",
            status: CheckStatus::Warn,
            detail: "tilth not registered in any MCP host — run `tilth install <host>`"
                .to_string(),
        }
    } else {
        DoctorCheck {
            name: "mcp_hosts",
            status: CheckStatus::Pass,
            detail: format!("registered in: {}", registered.join(", ")),
        }
    }
}

/// Check whether the registered command for each host actually exists on disk / PATH.
pub fn check_command_ok() -> DoctorCheck {
    let probes = probe_hosts();
    let bad: Vec<String> = probes
        .iter()
        .filter_map(|p| match &p.registration {
            Some((cmd, false, _)) => Some(format!("{} → {}", p.host, cmd)),
            _ => None,
        })
        .collect();
    if bad.is_empty() {
        DoctorCheck {
            name: "command_ok",
            status: CheckStatus::Pass,
            detail: "all registered commands resolvable".to_string(),
        }
    } else {
        DoctorCheck {
            name: "command_ok",
            status: CheckStatus::Fail,
            detail: format!("registered command not found: {}", bad.join("; ")),
        }
    }
}

/// Check trust level (read-only vs read+edit) per registered host.
pub fn check_trust_level() -> DoctorCheck {
    let probes = probe_hosts();
    let registered: Vec<&HostProbe> =
        probes.iter().filter(|p| p.registration.is_some()).collect();
    if registered.is_empty() {
        return DoctorCheck {
            name: "trust_level",
            status: CheckStatus::Warn,
            detail: "no hosts registered".to_string(),
        };
    }
    let with_edit: Vec<&str> = registered
        .iter()
        .filter(|p| matches!(p.registration.as_ref().map(|r| &r.2), Some(TrustLevel::ReadEdit)))
        .map(|p| p.host)
        .collect();
    let without_edit: Vec<&str> = registered
        .iter()
        .filter(|p| matches!(p.registration.as_ref().map(|r| &r.2), Some(TrustLevel::ReadOnly)))
        .map(|p| p.host)
        .collect();

    let detail = match (with_edit.is_empty(), without_edit.is_empty()) {
        (true, false) => format!(
            "read-only in: {}. Run `tilth install <host> --edit` to enable edits.",
            without_edit.join(", ")
        ),
        (false, true) => format!("read+edit in: {}", with_edit.join(", ")),
        (false, false) => format!(
            "read+edit: {}; read-only: {}",
            with_edit.join(", "),
            without_edit.join(", ")
        ),
        (true, true) => "no registered hosts".to_string(),
    };

    DoctorCheck {
        name: "trust_level",
        status: CheckStatus::Pass,
        detail,
    }
}

/// Check whether at least one host has edit mode enabled.
pub fn check_edit_mode() -> DoctorCheck {
    let probes = probe_hosts();
    let any_edit = probes.iter().any(|p| {
        matches!(p.registration.as_ref().map(|r| &r.2), Some(TrustLevel::ReadEdit))
    });
    if any_edit {
        DoctorCheck {
            name: "edit_mode",
            status: CheckStatus::Pass,
            detail: "edit mode active in at least one host".to_string(),
        }
    } else {
        DoctorCheck {
            name: "edit_mode",
            status: CheckStatus::Pass,
            detail: "edit mode not active anywhere (informational)".to_string(),
        }
    }
}

/// Check that the current working directory is readable.
pub fn check_scope() -> DoctorCheck {
    match std::env::current_dir() {
        Ok(cwd) if cwd.is_dir() => DoctorCheck {
            name: "scope",
            status: CheckStatus::Pass,
            detail: format!("current scope: {}", cwd.display()),
        },
        Ok(_) => DoctorCheck {
            name: "scope",
            status: CheckStatus::Fail,
            detail: "current directory is not a readable directory".to_string(),
        },
        Err(e) => DoctorCheck {
            name: "scope",
            status: CheckStatus::Fail,
            detail: format!("cannot determine current directory: {e}"),
        },
    }
}

// ── Report assembly ────────────────────────────────────────────────────────

pub fn build_report() -> DoctorReport {
    let checks = vec![
        check_binary(),
        check_mcp_hosts(),
        check_command_ok(),
        check_trust_level(),
        check_scope(),
        check_edit_mode(),
    ];

    let overall = compute_overall(&checks);
    DoctorReport { overall, checks }
}

fn compute_overall(checks: &[DoctorCheck]) -> CheckStatus {
    if checks.iter().any(|c| c.status == CheckStatus::Fail) {
        CheckStatus::Fail
    } else if checks.iter().any(|c| c.status == CheckStatus::Warn) {
        CheckStatus::Warn
    } else {
        CheckStatus::Pass
    }
}

// ── Entry point ────────────────────────────────────────────────────────────

pub fn run(json: bool) -> Result<(), String> {
    let report = build_report();

    if json {
        let out = serde_json::to_string_pretty(&report)
            .map_err(|e| format!("JSON serialization error: {e}"))?;
        println!("{out}");
        return if matches!(report.overall, CheckStatus::Fail) {
            Err(String::new()) // exit code 1, no extra output
        } else {
            Ok(())
        };
    }

    let icon = |s: &CheckStatus| match s {
        CheckStatus::Pass => "✓",
        CheckStatus::Warn => "!",
        CheckStatus::Fail => "✗",
    };

    println!("tilth doctor [{}]", icon(&report.overall));
    println!();
    for check in &report.checks {
        println!(
            "  [{}] {:<12} {}",
            icon(&check.status),
            check.name,
            check.detail
        );
    }
    println!();

    match report.overall {
        CheckStatus::Pass => {
            println!("All checks passed.");
            Ok(())
        }
        CheckStatus::Warn => {
            println!("Warnings found — see above.");
            Ok(())
        }
        CheckStatus::Fail => {
            println!("Failures found — see above.");
            Err(String::new())
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_status_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&CheckStatus::Pass).unwrap(), "\"pass\"");
        assert_eq!(serde_json::to_string(&CheckStatus::Warn).unwrap(), "\"warn\"");
        assert_eq!(serde_json::to_string(&CheckStatus::Fail).unwrap(), "\"fail\"");
    }

    #[test]
    fn doctor_check_serializes_correctly() {
        let c = DoctorCheck {
            name: "binary",
            status: CheckStatus::Pass,
            detail: "found".to_string(),
        };
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("\"binary\""));
        assert!(json.contains("\"pass\""));
        assert!(json.contains("\"found\""));
    }

    #[test]
    fn overall_fail_when_any_fail() {
        let checks = vec![
            DoctorCheck { name: "a", status: CheckStatus::Pass, detail: String::new() },
            DoctorCheck { name: "b", status: CheckStatus::Fail, detail: String::new() },
        ];
        assert_eq!(compute_overall(&checks), CheckStatus::Fail);
    }

    #[test]
    fn overall_warn_when_no_fail_but_warn() {
        let checks = vec![
            DoctorCheck { name: "a", status: CheckStatus::Pass, detail: String::new() },
            DoctorCheck { name: "b", status: CheckStatus::Warn, detail: String::new() },
        ];
        assert_eq!(compute_overall(&checks), CheckStatus::Warn);
    }

    #[test]
    fn overall_pass_when_all_pass() {
        let checks = vec![
            DoctorCheck { name: "a", status: CheckStatus::Pass, detail: String::new() },
        ];
        assert_eq!(compute_overall(&checks), CheckStatus::Pass);
    }

    #[test]
    fn build_report_has_six_checks() {
        let report = build_report();
        assert_eq!(report.checks.len(), 6);
    }

    #[test]
    fn report_json_roundtrip_has_stable_shape() {
        let report = build_report();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"overall\""));
        assert!(json.contains("\"checks\""));
        assert!(json.contains("\"binary\""));
        assert!(json.contains("\"mcp_hosts\""));
        assert!(json.contains("\"command_ok\""));
        assert!(json.contains("\"trust_level\""));
        assert!(json.contains("\"scope\""));
        assert!(json.contains("\"edit_mode\""));
    }

    #[test]
    fn check_scope_pass_in_normal_env() {
        let c = check_scope();
        assert_eq!(c.name, "scope");
        assert_eq!(c.status, CheckStatus::Pass);
    }

    #[test]
    fn each_check_returns_its_own_name() {
        assert_eq!(check_binary().name, "binary");
        assert_eq!(check_mcp_hosts().name, "mcp_hosts");
        assert_eq!(check_command_ok().name, "command_ok");
        assert_eq!(check_trust_level().name, "trust_level");
        assert_eq!(check_edit_mode().name, "edit_mode");
        assert_eq!(check_scope().name, "scope");
    }
}
