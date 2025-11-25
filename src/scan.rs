//! Async scanning and concurrent inspection.
//!
//! Responsibilities:
//! - Discover `.desktop` files under a set of directories (async directory walk).
//! - Inspect each file concurrently (bounded parallelism).
//! - Convert raw parsing/checking into `Finding` records.

use crate::{
    args::Args,
    check, desktop,
    report::{Finding, Status},
};
use anyhow::Result;
use futures::stream::{self, StreamExt};
use std::{env, path::PathBuf};
use tokio::{fs, sync::Semaphore};
use tracing::{debug, warn};

/// Recursively collect `.desktop` files from a list of root directories.
///
/// This function:
/// - walks directories using `tokio::fs::read_dir`
/// - skips symlinks to avoid recursion loops
/// - returns sorted, deduped paths
pub async fn collect_desktop_files(dirs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();

    for root in dirs {
        let mut stack = vec![root.clone()];

        while let Some(dir) = stack.pop() {
            let mut rd = match fs::read_dir(&dir).await {
                Ok(rd) => rd,
                Err(_) => continue, // skip missing/unreadable dirs
            };

            loop {
                let ent = match rd.next_entry().await {
                    Ok(Some(e)) => e,
                    Ok(None) => break,
                    Err(_) => break,
                };

                let ft = match ent.file_type().await {
                    Ok(ft) => ft,
                    Err(_) => continue,
                };

                if ft.is_symlink() {
                    continue; // avoid loops
                }
                let p = ent.path();
                if ft.is_dir() {
                    stack.push(p);
                } else if ft.is_file() && p.extension().and_then(|e| e.to_str()) == Some("desktop")
                {
                    out.push(p);
                }
            }
        }
    }

    out.sort();
    out.dedup();
    Ok(out)
}

/// Inspect a list of `.desktop` files concurrently with bounded parallelism.
///
/// - `args.jobs` controls max concurrency.
/// - Each file is read and checked independently.
/// - Any per-file errors are converted into a `Broken` finding.
pub async fn inspect_files_concurrently(files: Vec<PathBuf>, args: &Args) -> Vec<Finding> {
    let path_env = env::var("PATH").unwrap_or_default();
    let jobs = args
        .jobs
        .unwrap_or_else(|| num_cpus::get().saturating_mul(4).max(8));

    let sem = Semaphore::new(jobs);
    debug!(jobs, "Starting concurrent inspection");

    stream::iter(files)
        .map(|path| {
            let sem = &sem;
            let args = args.clone();
            let path_env = path_env.clone();

            async move {
                let _permit = sem.acquire().await.expect("semaphore closed");
                match inspect_one(&path, &path_env, &args).await {
                    Ok(f) => f,
                    Err(e) => {
                        warn!(file = %path.display(), error = %e, "Failed to inspect file");
                        Finding {
                            desktop_file: path,
                            name: None,
                            exec: None,
                            try_exec: None,
                            path_key: None,
                            hidden: false,
                            no_display: false,
                            status: Status::Broken {
                                reason: format!("Failed to read/parse file: {e:#}"),
                            },
                        }
                    }
                }
            }
        })
        .buffer_unordered(jobs)
        .collect()
        .await
}

/// Inspect a single `.desktop` file and return a `Finding`.
///
/// This function:
/// - reads the file asynchronously
/// - parses `[Desktop Entry]`
/// - applies skip rules (`Hidden`, `NoDisplay`, `Type!=Application`)
/// - validates `TryExec` (preferred) and/or `Exec`
/// - returns `Ok`, `Broken`, or `Skipped`
async fn inspect_one(path: &PathBuf, path_env: &str, args: &Args) -> Result<Finding> {
    let content = fs::read_to_string(path).await?;
    let kv = desktop::parse_desktop_entry_section(&content);

    let name = kv.get("Name").cloned();
    let exec = kv.get("Exec").cloned();
    let try_exec = kv.get("TryExec").cloned();
    let hidden = desktop::parse_bool(kv.get("Hidden"));
    let no_display = desktop::parse_bool(kv.get("NoDisplay"));
    let typ = kv.get("Type").map(|s| s.trim().to_string());
    let dbus_activatable = desktop::parse_bool(kv.get("DBusActivatable"));
    let path_key = kv.get("Path").cloned();

    if !args.include_hidden && (hidden || no_display) {
        return Ok(Finding {
            desktop_file: path.clone(),
            name,
            exec,
            try_exec,
            path_key,
            hidden,
            no_display,
            status: Status::Skipped {
                reason: "Hidden=true or NoDisplay=true (use --include-hidden to scan these)".into(),
            },
        });
    }

    if let Some(t) = typ.as_deref() {
        if t != "Application" {
            return Ok(Finding {
                desktop_file: path.clone(),
                name,
                exec,
                try_exec,
                path_key,
                hidden,
                no_display,
                status: Status::Skipped {
                    reason: format!("Type={t} (only Type=Application is checked)"),
                },
            });
        }
    }

    // DBus activatable entries may legitimately omit Exec.
    if dbus_activatable && exec.is_none() {
        return Ok(Finding {
            desktop_file: path.clone(),
            name,
            exec,
            try_exec,
            path_key,
            hidden,
            no_display,
            status: Status::Ok {
                resolved_executable: None,
            },
        });
    }

    let ctx = check::CheckContext {
        path_env,
        path_key: path_key.as_deref(),
        check_script_args: args.check_script_args,
    };

    // Prefer TryExec if present.
    if let Some(tx) = try_exec.clone().as_deref() {
        match check::validate_tryexec(tx, &ctx).await? {
            Some(resolved_tx) => {
                // Still validate Exec if present.
                if let Some(exec_line) = exec.as_deref() {
                    match check::validate_exec(exec_line, &ctx).await {
                        Ok(Some(resolved_exec)) => {
                            return Ok(Finding {
                                desktop_file: path.clone(),
                                name,
                                exec,
                                try_exec,
                                path_key,
                                hidden,
                                no_display,
                                status: Status::Ok {
                                    resolved_executable: Some(resolved_exec),
                                },
                            });
                        }
                        Ok(None) => {
                            return Ok(Finding {
                                desktop_file: path.clone(),
                                name,
                                exec,
                                try_exec,
                                path_key,
                                hidden,
                                no_display,
                                status: Status::Broken {
                                    reason: "Exec does not resolve (even though TryExec does)"
                                        .into(),
                                },
                            });
                        }
                        Err(e) => {
                            return Ok(Finding {
                                desktop_file: path.clone(),
                                name,
                                exec,
                                try_exec,
                                path_key,
                                hidden,
                                no_display,
                                status: Status::Broken {
                                    reason: format!("Exec check failed: {e:#}"),
                                },
                            });
                        }
                    }
                }

                return Ok(Finding {
                    desktop_file: path.clone(),
                    name,
                    exec,
                    try_exec,
                    path_key,
                    hidden,
                    no_display,
                    status: Status::Ok {
                        resolved_executable: Some(resolved_tx),
                    },
                });
            }
            None => {
                return Ok(Finding {
                    desktop_file: path.clone(),
                    name,
                    exec,
                    try_exec,
                    path_key,
                    hidden,
                    no_display,
                    status: Status::Broken {
                        reason: format!("TryExec does not resolve: {tx}"),
                    },
                });
            }
        }
    }

    // Otherwise validate Exec.
    if let Some(exec_line) = exec.as_deref() {
        match check::validate_exec(exec_line, &ctx).await {
            Ok(Some(resolved)) => Ok(Finding {
                desktop_file: path.clone(),
                name,
                exec,
                try_exec,
                path_key,
                hidden,
                no_display,
                status: Status::Ok {
                    resolved_executable: Some(resolved),
                },
            }),
            Ok(None) => Ok(Finding {
                desktop_file: path.clone(),
                name,
                exec,
                try_exec,
                path_key,
                hidden,
                no_display,
                status: Status::Broken {
                    reason: "Exec does not resolve".into(),
                },
            }),
            Err(e) => Ok(Finding {
                desktop_file: path.clone(),
                name,
                exec,
                try_exec,
                path_key,
                hidden,
                no_display,
                status: Status::Broken {
                    reason: format!("Exec check failed: {e:#}"),
                },
            }),
        }
    } else {
        Ok(Finding {
            desktop_file: path.clone(),
            name,
            exec,
            try_exec,
            path_key,
            hidden,
            no_display,
            status: Status::Broken {
                reason: "No Exec key found (and not DBusActivatable)".into(),
            },
        })
    }
}
