//! Command-line argument definitions.
//!
//! This module defines the CLI surfaced by `desktop-scout`.

// -- std imports
use std::path::PathBuf;

// -- crate imports
use clap::Parser;

/// Command-line arguments for `desktop-scout`.
///
/// Use `--help` to see all options and defaults.
#[derive(Parser, Debug, Clone)]
#[command(
    name = "desktop-scout",
    about = "Detect broken/stale .desktop files by validating Exec/TryExec"
)]
pub struct Args {
    /// Print JSON output (machine readable)
    #[arg(long)]
    pub json: bool,

    /// Do not use default scan directories
    #[arg(long)]
    pub no_default: bool,

    /// Suppress all logging output
    #[arg(long)]
    pub no_log: bool,

    /// Include entries with Hidden=true or NoDisplay=true
    #[arg(long)]
    pub include_hidden: bool,

    /// Additional directory to scan (can be passed multiple times)
    #[arg(long = "dir")]
    pub extra_dirs: Vec<PathBuf>,

    /// Do not scan common extra dirs (Flatpak, Snap desktop exports)
    #[arg(long)]
    pub no_common_extras: bool,

    /// Heuristic checks for interpreter Exec lines (python/node/bash) where script path is an arg
    #[arg(long)]
    pub check_script_args: bool,

    /// Max concurrent inspections (defaults to CPU count * 4)
    #[arg(long)]
    pub jobs: Option<usize>,
}
