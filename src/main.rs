// -- crate imports
use anyhow::Result;
use clap::Parser;
use tracing::{debug, info, warn};

// -- module definitions
mod args;
mod check;
mod desktop;
mod linux_fs;
mod log;
mod report;
mod scan;

// -- module imports
use crate::args::Args;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.no_log {
        tracing::subscriber::set_global_default(tracing::subscriber::NoSubscriber::default())
            .expect("Failed to set no-op subscriber");
    } else {
        log::init_tracing()?;
        info!("desktop-scout started");
        debug!("Parsed args: {args:#?}");
    }

    let dirs = linux_fs::collect_application_dirs(&args);
    let files = scan::collect_desktop_files(&dirs).await?;
    let reports = scan::inspect_files_concurrently(files, &args).await;

    let broken: Vec<_> = reports
        .into_iter()
        .filter(|r| matches!(r.status, report::Status::Broken { .. }))
        .collect();

    if !args.json && broken.is_empty() {
        println!("No broken desktop entries found.");
        return Ok(());
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&broken)?);
        return Ok(());
    }

    println!("Broken .desktop entries ({}):\n", broken.len());
    for f in broken {
        println!("- {}", f.desktop_file.display());
        if let Some(name) = &f.name {
            println!("  Name: {name}");
        }
        if let Some(exec) = &f.exec {
            println!("  Exec: {exec}");
        }
        if let Some(tx) = &f.try_exec {
            println!("  TryExec: {tx}");
        }
        if let Some(p) = &f.path_key {
            println!("  Path: {p}");
        }
        println!("  Hidden: {} | NoDisplay: {}", f.hidden, f.no_display);

        if let report::Status::Broken { reason } = &f.status {
            println!("  Reason: {reason}");
        } else {
            warn!("Unexpected non-broken in broken list?");
        }
        println!();
    }

    info!("desktop-scout done!");
    Ok(())
}
