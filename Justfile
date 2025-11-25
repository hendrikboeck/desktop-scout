set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

tokio_console_rustflags := '["--cfg","tokio_unstable"]'

default:
  @just --list

# Build (normal mode). Extra ARGS are forwarded to `cargo build`.
build *ARGS:
  cargo build {{ARGS}}

# Run (normal mode). Extra ARGS are forwarded to `cargo run` (use `--` for program args).
run *ARGS:
  cargo run {{ARGS}}

# Test (normal mode). Extra ARGS are forwarded to `cargo test`.
test *ARGS:
  cargo test {{ARGS}}

# Build with `tokio-console` feature + `tokio_unstable` cfg enabled.
build-console *ARGS:
  cargo build \
    --features tokio-console \
    --config 'build.rustflags={{tokio_console_rustflags}}' \
    {{ARGS}}

# Run with `tokio-console` feature + `tokio_unstable` cfg enabled.
run-console *ARGS:
  cargo run \
    --features tokio-console \
    --config 'build.rustflags={{tokio_console_rustflags}}' \
    {{ARGS}}

# Test with `tokio-console` feature + `tokio_unstable` cfg enabled.
test-console *ARGS:
  cargo test \
    --features tokio-console \
    --config 'build.rustflags={{tokio_console_rustflags}}' \
    {{ARGS}}
