# desktop-scout

`desktop-scout` is a command-line utility designed to detect broken or stale `.desktop` files on Linux systems. It scans standard application directories (XDG data directories, Flatpak exports, Snap exports) and validates the `Exec` and `TryExec` keys within each entry to ensure they resolve to executable files on the current system.

## Features

- **Automated Discovery**: Recursively collects `.desktop` files from standard XDG locations (`~/.local/share/applications`, `/usr/share/applications`) and common package manager export paths.
- **Concurrent Inspection**: Utilizes asynchronous I/O and bounded parallelism to inspect files efficiently.
- **Validation Logic**:
  - Parses `[Desktop Entry]` sections.
  - Resolves `TryExec` and `Exec` commands against the system `PATH` or absolute paths.
  - Handles `env` variables and shell quoting in command lines.
  - Optionally checks for missing script arguments when the executable is an interpreter (e.g., Python, Node, Bash).
- **Filtering**: Automatically skips entries marked as `Hidden=true` or `NoDisplay=true` unless configured otherwise.
- **Reporting**: Outputs findings in human-readable text or machine-readable JSON format.

## Installation

### From Source

Ensure you have a recent version of Rust installed.

```sh
git clone https://github.com/hendrikboeck/desktop-scout.git
cd desktop-scout
cargo install --path .
```

## Usage

Run the tool without arguments to scan default directories and print broken entries to standard output:

```sh
desktop-scout
```

### Command Line Options

- `--json`: Output results in JSON format for integration with other tools.
- `--no-default`: Disable scanning of standard XDG directories.
- `--dir <PATH>`: Add a custom directory to the scan list. Can be specified multiple times.
- `--include-hidden`: Include entries marked as `Hidden` or `NoDisplay` in the scan.
- `--check-script-args`: Enable heuristic checks for missing script files when the `Exec` line invokes an interpreter.
- `--jobs <N>`: Set the maximum number of concurrent file inspections (defaults to 4x CPU count).
- `--no-log`: Suppress logging output.

### Examples

**Scan default directories and pipe to `jq`:**

```sh
desktop-scout --no-log --json | jq "."
```

**Scan specific directories and output JSON:**

```sh
desktop-scout --no-default --dir ~/custom-apps --json
```

**Enable strict checking for interpreter scripts:**

```sh
desktop-scout --check-script-args
```

## Logging

By default, logs are written to:

- **Debug builds**: `./desktop-scout.log`
- **Release builds**: `$XDG_DATA_HOME/desktop-scout/desktop-scout.log`

Logging levels can be controlled via the `RUST_LOG` environment variable.

## Development

This project uses `just` for command management.

- **Build**: `just build`
- **Run**: `just run`
- **Test**: `just test`

To enable `tokio
