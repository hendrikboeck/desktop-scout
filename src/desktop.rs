//! Minimal `.desktop` parsing helpers.
//!
//! This project intentionally avoids a full spec-compliant parser and instead extracts
//! the essentials from `[Desktop Entry]` for the purpose of CLI health checks.

use std::collections::HashMap;

/// Parse only the `[Desktop Entry]` section into a key-value map.
///
/// - Ignores other sections.
/// - Ignores comments (`#` and `;` as a first non-whitespace char).
/// - Keeps keys exactly as written (no lowercasing).
///
/// This is sufficient for reading common keys like `Exec`, `TryExec`, `Name`, etc.
pub fn parse_desktop_entry_section(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut in_section = false;

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_section = line == "[Desktop Entry]";
            continue;
        }
        if !in_section {
            continue;
        }

        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }

    map
}

/// Parse a `.desktop` boolean string.
///
/// Accepts common truthy values:
/// - `true`, `1`, `yes` (case-insensitive)
pub fn parse_bool(v: Option<&String>) -> bool {
    matches!(
        v.map(|s| s.trim().to_ascii_lowercase()).as_deref(),
        Some("true") | Some("1") | Some("yes")
    )
}

/// Extract the executable token from `Exec=` after shell-splitting.
///
/// Handles typical patterns:
/// - `cmd arg1 arg2` → `cmd`
/// - `env VAR=1 VAR2=2 cmd arg` → `cmd`
///
/// Returns `None` if no plausible token exists.
pub fn extract_executable_from_tokens(tokens: &[String]) -> Option<String> {
    if tokens.is_empty() {
        return None;
    }

    let mut i = 0;

    // Handle `env ... cmd`
    if tokens.get(0).map(|s| s.as_str()) == Some("env") {
        i = 1;
        // Skip env options and assignments
        while i < tokens.len() {
            let t = &tokens[i];
            if t.starts_with('-') || t.contains('=') {
                i += 1;
                continue;
            }
            break;
        }
    }

    tokens.get(i).cloned()
}
