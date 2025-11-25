//! Data structures for reporting scan outcomes.
//!
//! These types are serializable to JSON for machine-readable output and are also used
//! for human-readable printing in `main`.

use serde::Serialize;
use std::path::PathBuf;

/// A scan result for a single `.desktop` file.
///
/// Contains basic metadata extracted from `[Desktop Entry]` and a `status` field
/// describing whether it is OK, Broken, or Skipped.
#[derive(Debug, Serialize)]
pub struct Finding {
    /// Full path to the `.desktop` file inspected.
    pub desktop_file: PathBuf,

    /// Value of the `Name=` key (if present).
    pub name: Option<String>,

    /// Value of the `Exec=` key (if present).
    pub exec: Option<String>,

    /// Value of the `TryExec=` key (if present).
    pub try_exec: Option<String>,

    /// Value of the `Path=` key (if present).
    pub path_key: Option<String>,

    /// Whether `Hidden=true`.
    pub hidden: bool,

    /// Whether `NoDisplay=true`.
    pub no_display: bool,

    /// Inspection outcome.
    pub status: Status,
}

/// Outcome of inspecting a `.desktop` file.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Status {
    /// The entry appears healthy w.r.t. executable resolution.
    Ok {
        /// The resolved executable on disk, if one was found/resolved.
        ///
        /// Some valid entries (e.g. DBusActivatable) may not resolve an executable.
        resolved_executable: Option<PathBuf>,
    },

    /// The entry appears broken/stale with a human-readable explanation.
    Broken {
        /// Reason describing why the entry is considered broken.
        reason: String,
    },

    /// The entry was intentionally not checked.
    Skipped {
        /// Reason describing why the entry was skipped.
        reason: String,
    },
}
