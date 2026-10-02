use crate::cleaner::{clean_project, DeletionMethod};
use crate::cli::args::{CleanArgs, ScanArgs};
use crate::scanner::model::{format_age, format_bytes, ProjectInfo, ScanMessage};
use crate::scanner::walker::scan_path;
use anyhow::Result;
use chrono::Local;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;

pub fn run_scan_command(args: ScanArgs) -> Result<()> {
    let target_path = canonicalize_path(args.path);
    println!("Scanning for dev build caches in: {}", target_path.display());

    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));

    let scan_root = target_path.clone();
    thread::spawn(move || {
        scan_path(scan_root, tx, cancel);
    });

    let mut projects = Vec::new();
    let filter_eco = args.ecosystem.map(|e| e.to_scanner_ecosystem());

    while let Ok(msg) = rx.recv() {
        match msg {
            ScanMessage::ProjectFound(p) => {
                if let Some(ref eco) = filter_eco {
                    if p.ecosystem != *eco {
                        continue;
                    }
                }

                if let Some(min_days) = args.older_than {
                    if let Some(last_mod) = p.last_modified {
                        let age_days = Local::now().signed_duration_since(last_mod).num_days();
                        if age_days < min_days {
                            continue;
                        }
                    }
                }

                projects.push(*p);
            }
            ScanMessage::Finished {
                total_bytes,
                elapsed_millis,
                ..
            } => {
                println!();
                render_scan_table(&projects);
                println!(
                    "\nTotal Reclaimable Space: {} across {} project(s) (Scanned in {:.2}s)",
                    format_bytes(total_bytes),
                    projects.len(),
                    elapsed_millis as f64 / 1000.0
                );
                break;
            }
            ScanMessage::Error(err) => {
                eprintln!("Scan error: {}", err);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

pub fn run_clean_command(args: CleanArgs) -> Result<()> {
    let target_path = canonicalize_path(args.path);
    println!("Analyzing projects in: {}", target_path.display());

    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));

    let scan_root = target_path.clone();
    thread::spawn(move || {
        scan_path(scan_root, tx, cancel);
    });

    let mut matching_projects = Vec::new();
    let filter_eco = args.ecosystem.map(|e| e.to_scanner_ecosystem());

    while let Ok(msg) = rx.recv() {
        match msg {
            ScanMessage::ProjectFound(p) => {
                if let Some(ref eco) = filter_eco {
                    if p.ecosystem != *eco {
                        continue;
                    }
                }

                if let Some(min_days) = args.older_than {
                    if let Some(last_mod) = p.last_modified {
                        let age_days = Local::now().signed_duration_since(last_mod).num_days();
                        if age_days < min_days {
                            continue;
                        }
                    }
                }

                matching_projects.push(*p);
            }
            ScanMessage::Finished { .. } => {
                break;
            }
            _ => {}
        }
    }

    if matching_projects.is_empty() {
        println!("No matching projects or build caches found to clean.");
        return Ok(());
    }

    render_scan_table(&matching_projects);

    let total_reclaimable: u64 = matching_projects.iter().map(|p| p.total_size_bytes).sum();
    let method = if args.permanent {
        DeletionMethod::Permanent
    } else {
        DeletionMethod::Trash
    };

    println!(
        "\nTarget: {} project(s), reclaiming {}",
        matching_projects.len(),
        format_bytes(total_reclaimable)
    );
    println!(
        "Mode: {}",
        if args.permanent {
            "Permanent Deletion"
        } else {
            "Recycle Bin (Safe)"
        }
    );

    if args.dry_run {
        println!("\n[DRY RUN] Simulation complete. No files were modified.");
        return Ok(());
    }

    if !args.yes {
        print!("\nProceed with deletion? [y/N]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim().to_lowercase();
        if trimmed != "y" && trimmed != "yes" {
            println!("Operation cancelled by user.");
            return Ok(());
        }
    }

    println!("\nCleaning build artifacts...");
    let mut total_freed = 0u64;

    for proj in &matching_projects {
        let results = clean_project(proj, method);
        for res in results {
            if res.success {
                total_freed += res.bytes_freed;
                println!(
                    "  [CLEANED] {} -> {} ({})",
                    proj.name,
                    res.artifact_path.display(),
                    format_bytes(res.bytes_freed)
                );
            } else {
                eprintln!(
                    "  [FAILED]  {} -> {}: {}",
                    proj.name,
                    res.artifact_path.display(),
                    res.error.unwrap_or_else(|| "Unknown error".to_string())
                );
            }
        }
    }

    println!(
        "\nCleanup complete. Total freed space: {}",
        format_bytes(total_freed)
    );

    Ok(())
}

fn canonicalize_path(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

fn render_scan_table(projects: &[ProjectInfo]) {
    println!(
        "{:<12} {:<24} {:<20} {:<12} {:<12}",
        "ECOSYSTEM", "PROJECT", "ARTIFACTS", "SIZE", "LAST ACTIVE"
    );
    println!("{}", "-".repeat(84));

    for p in projects {
        let arts: Vec<&str> = p.artifacts.iter().map(|a| a.name.as_str()).collect();
        let arts_str = arts.join(", ");
        let name_truncated = if p.name.len() > 22 {
            format!("{}...", &p.name[..19])
        } else {
            p.name.clone()
        };

        println!(
            "{:<12} {:<24} {:<20} {:<12} {:<12}",
            p.ecosystem.as_str(),
            name_truncated,
            arts_str,
            format_bytes(p.total_size_bytes),
            format_age(p.last_modified)
        );
    }
}
