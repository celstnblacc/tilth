//! Validates `$PAGER` before spawning it, to prevent command injection via
//! a malicious environment variable. Ported from this fork's `security.rs`
//! (deleted upstream, along with the path-containment guard it also held —
//! that half is superseded by `mcp::tools`'s newer, more comprehensive
//! `resolve_scope`/`path_within_scope` system and intentionally not ported).

// Shell metacharacters that could enable command injection.
const SHELL_META: &[char] = &['|', '&', ';', '$', '`', '(', ')', '<', '>', '\n', '\r'];

/// Validate `$PAGER`'s value before it is passed to `Command::new`.
///
/// # Returns
/// A pager name/path safe to spawn directly. Falls back to `"less"` on any
/// suspicious input (shell metacharacters, missing/non-executable path).
pub fn validate_pager(pager: &str) -> String {
    // Allow list of known-safe pagers
    const SAFE_PAGERS: &[&str] = &["less", "more", "most", "bat", "cat"];

    let trimmed = pager.trim();

    // Empty or whitespace-only -> fallback
    if trimmed.is_empty() {
        return "less".to_string();
    }

    // Check if it's in the allow list (case-insensitive)
    let lower = trimmed.to_lowercase();
    if SAFE_PAGERS.iter().any(|&p| lower == p) {
        return trimmed.to_string();
    }

    // Check for shell metacharacters that could enable injection
    if trimmed.chars().any(|c| SHELL_META.contains(&c)) {
        eprintln!("warning: unsafe $PAGER value '{trimmed}', using 'less'");
        return "less".to_string();
    }

    // If it contains a path separator, verify it's an absolute path to an executable
    if trimmed.contains('/') {
        let path = std::path::Path::new(trimmed);
        if path.is_absolute() && path.exists() && is_executable(path) {
            return trimmed.to_string();
        }
        eprintln!("warning: $PAGER path '{trimmed}' not found or not executable, using 'less'");
        return "less".to_string();
    }

    // Simple command name without path — allow it (will be resolved via $PATH)
    // Already checked for shell metacharacters above
    trimmed.to_string()
}

/// Check if a path is executable (Unix-only check, no-op on Windows).
#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = std::fs::metadata(path) {
        let permissions = metadata.permissions();
        permissions.mode() & 0o111 != 0
    } else {
        false
    }
}

#[cfg(not(unix))]
fn is_executable(_path: &std::path::Path) -> bool {
    // On Windows, just check if file exists (all .exe are executable)
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_pager_accepted() {
        assert_eq!(validate_pager("less"), "less");
        assert_eq!(validate_pager("more"), "more");
    }

    #[test]
    fn test_empty_pager_falls_back() {
        assert_eq!(validate_pager(""), "less");
        assert_eq!(validate_pager("   "), "less");
    }

    #[test]
    fn test_pager_with_shell_metachars_rejected() {
        assert_eq!(validate_pager("less; rm -rf /tmp/x"), "less");
        assert_eq!(validate_pager("less && rm -rf /tmp/x"), "less");
        assert_eq!(validate_pager("$(rm -rf /tmp/x)"), "less");
        assert_eq!(validate_pager("less`rm -rf /tmp/x`"), "less");
    }

    #[test]
    fn test_pager_absolute_path_nonexistent_falls_back() {
        assert_eq!(validate_pager("/nonexistent/path/to/pager"), "less");
    }

    #[test]
    fn test_pager_relative_path_with_slash_falls_back() {
        // Non-absolute path containing '/' is rejected (must be absolute).
        assert_eq!(validate_pager("bin/pager"), "less");
    }

    #[test]
    fn test_simple_command_name_allowed_through_path() {
        // No metachars, no slash — resolved via $PATH at spawn time.
        assert_eq!(validate_pager("bat"), "bat");
        assert_eq!(validate_pager("moar"), "moar");
    }
}
