//! Executable resolution and validity checks.
//!
//! This module contains logic that takes a `TryExec` or `Exec` string and decides whether
//! it resolves to a runnable executable on the current machine. Checks are async
//! (Tokio filesystem operations).

use crate::desktop::extract_executable_from_tokens;
use anyhow::Result;
use shlex;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Context required to validate a `.desktop` entry.
///
/// This groups together environment-derived and `.desktop`-derived values used during resolution.
pub struct CheckContext<'a> {
    /// The PATH string used to resolve bare commands (e.g., `firefox`).
    pub path_env: &'a str,

    /// Optional working directory from `.desktop` `Path=`.
    ///
    /// This is used for resolving relative executable tokens like `./bin/myapp`.
    pub path_key: Option<&'a str>,

    /// If true, run a conservative heuristic that flags missing scripts when the
    /// executable is an interpreter (python/node/bash/etc).
    pub check_script_args: bool,
}

/// Validate a `TryExec=` value.
///
/// `TryExec` is specifically meant to test program presence. We try to resolve it
/// either as a filesystem path (if it contains `/`) or by searching `PATH`.
pub async fn validate_tryexec(try_exec: &str, ctx: &CheckContext<'_>) -> Result<Option<PathBuf>> {
    resolve_executable(try_exec, ctx.path_env, ctx.path_key).await
}

/// Validate an `Exec=` command line.
///
/// Steps:
/// 1. Shell-split `Exec`
/// 2. Extract the executable token (with `env VAR=...` handling)
/// 3. Resolve it as a path or via `PATH`
/// 4. (Optional) run script-argument heuristic for interpreters.
///
/// Returns `Ok(Some(path))` if the executable resolves and is runnable,
/// `Ok(None)` if it does not resolve, and `Err` for parse/heuristic failures.
pub async fn validate_exec(exec_line: &str, ctx: &CheckContext<'_>) -> Result<Option<PathBuf>> {
    let tokens =
        shlex::split(exec_line).ok_or_else(|| anyhow::anyhow!("Failed to shell-split Exec"))?;
    let extracted = extract_executable_from_tokens(&tokens)
        .ok_or_else(|| anyhow::anyhow!("Could not extract executable from Exec"))?;

    // If the "executable" is actually a field code marker, it's not meaningful.
    if extracted.starts_with('%') {
        return Ok(None);
    }

    let resolved = resolve_executable(&extracted, ctx.path_env, ctx.path_key).await?;

    // Optional: check missing script arguments for interpreter launchers.
    if ctx.check_script_args {
        if let Some(resolved_exe) = &resolved {
            if let Some(reason) =
                heuristic_script_missing(resolved_exe, &tokens, ctx.path_key).await?
            {
                return Err(anyhow::anyhow!(reason));
            }
        }
    }

    Ok(resolved)
}

/// Resolve an executable token to an on-disk executable path, if possible.
///
/// Rules:
/// - If `token` contains `/`, treat it as a path.
///   - Absolute: validate directly.
///   - Relative: if `Path=` exists, resolve relative to that working dir.
/// - Otherwise (no `/`), search `PATH`.
pub async fn resolve_executable(
    token: &str,
    path_env: &str,
    path_key: Option<&str>,
) -> Result<Option<PathBuf>> {
    // If token includes a '/', treat it as a path.
    if token.contains('/') {
        let p = Path::new(token);

        if p.is_absolute() {
            return Ok(if is_executable_file(p).await {
                Some(p.to_path_buf())
            } else {
                None
            });
        }

        // Relative path: try resolve via Path= (working dir)
        if let Some(wd) = path_key {
            let candidate = Path::new(wd).join(p);
            return Ok(if is_executable_file(&candidate).await {
                Some(candidate)
            } else {
                None
            });
        }

        // Relative without Path= is ambiguous in `.desktop`; we treat as unresolved.
        return Ok(None);
    }

    // Bare cmd: search PATH
    Ok(which_in_path(token, path_env).await)
}

/// Search for `cmd` in the given PATH string.
///
/// Returns the first match that is an executable file.
async fn which_in_path(cmd: &str, path_env: &str) -> Option<PathBuf> {
    for dir in path_env.split(':').filter(|s| !s.is_empty()) {
        let candidate = Path::new(dir).join(cmd);
        if is_executable_file(&candidate).await {
            return Some(candidate);
        }
    }
    None
}

/// Check whether `p` exists, is a regular file, and has any executable bit set.
async fn is_executable_file(p: &Path) -> bool {
    let md = match fs::metadata(p).await {
        Ok(m) => m,
        Err(_) => return false,
    };

    if !md.is_file() {
        return false;
    }

    // NOTE: this is Unix-specific, which is fine for Linux `.desktop` scanning.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = md.permissions().mode();
        (mode & 0o111) != 0
    }

    #[cfg(not(unix))]
    {
        // Best-effort fallback: existence and file-ness only.
        true
    }
}

/// (Optional) heuristic: flag missing scripts when `Exec` uses an interpreter.
///
/// Example it catches:
/// - `python3 /home/user/bin/foo.py` (script missing)
///
/// This is intentionally conservative and does not attempt to parse all interpreter flags.
/// It tries to find the first "non-option" argument and verifies it exists if it looks like a path.
async fn heuristic_script_missing(
    resolved_exe: &Path,
    tokens: &[String],
    path_key: Option<&str>,
) -> Result<Option<String>> {
    let exe_name = resolved_exe
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let is_interpreter = matches!(
        exe_name.as_str(),
        "python" | "python3" | "node" | "bash" | "sh" | "ruby" | "perl"
    );
    if !is_interpreter || tokens.is_empty() {
        return Ok(None);
    }

    // Find first non-option argument after interpreter.
    let mut i = 1;
    while i < tokens.len() {
        let t = &tokens[i];
        if t.starts_with('%') {
            i += 1;
            continue;
        }
        if t.starts_with('-') {
            i += 1;
            continue;
        }
        break;
    }

    let arg = match tokens.get(i) {
        Some(a) => a,
        None => return Ok(None),
    };

    // Only care if it looks path-like.
    if !arg.contains('/') {
        return Ok(None);
    }

    let p = Path::new(arg);
    let candidate = if p.is_absolute() {
        p.to_path_buf()
    } else if let Some(wd) = path_key {
        Path::new(wd).join(p)
    } else {
        // Relative without Path= is ambiguous.
        return Ok(None);
    };

    if fs::metadata(&candidate).await.is_err() {
        return Ok(Some(format!(
            "Interpreter {exe_name} exists, but script/path argument is missing: {}",
            candidate.display()
        )));
    }

    Ok(None)
}
