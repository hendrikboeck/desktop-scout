//! Directory discovery for `.desktop` files.
//!
//! This module collects application directories following XDG conventions plus
//! common extras (Flatpak/Snap exports) and any user-provided directories.

// -- std imports
use std::{collections::BTreeSet, path::PathBuf};

// -- crate imports
use tracing::debug;
use xdg::BaseDirectories;

// -- module imports
use crate::args::Args;

/// Collect a list of directories that may contain `.desktop` files.
///
/// Primary sources (unless `--no-default`):
/// - `$XDG_DATA_HOME/applications` (default: `~/.local/share/applications`)
/// - `$XDG_DATA_DIRS/applications` (default: `/usr/local/share/applications:/usr/share/applications`)
///
/// Extras (unless `--no-common-extras`):
/// - Flatpak exports (user + system)
/// - Snap desktop exports
///
/// Always includes `--dir` values verbatim.
pub fn collect_application_dirs(args: &Args) -> Vec<PathBuf> {
    let xdg = BaseDirectories::new();
    let mut set = BTreeSet::<PathBuf>::new();

    // Default dirs (can be disabled)
    if !args.no_default {
        if let Some(data_home) = xdg.get_data_home() {
            set.insert(data_home.join("applications"));

            if !args.no_common_extras {
                set.insert(data_home.join("flatpak/exports/share/applications"));
            }
        } else {
            debug!("XDG data home unavailable; skipping ~/.local/share candidates");
        }

        for dir in xdg.get_data_dirs() {
            set.insert(dir.join("applications"));
        }

        if !args.no_common_extras {
            set.insert(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
            set.insert(PathBuf::from("/var/lib/snapd/desktop/applications"));
        }
    }

    // User-provided extra dirs
    set.extend(args.extra_dirs.iter().cloned());

    debug!(
        count = set.len(),
        "Collected application dirs to scan: {set:#?}"
    );
    set.into_iter().collect::<Vec<_>>()
}
