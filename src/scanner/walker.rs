use crate::scanner::model::{ArtifactFolder, ProjectInfo, ScanMessage};
use crate::scanner::rules::match_ecosystems;
use crate::scanner::size::calculate_folder_stats;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Instant;

const IGNORED_DIR_NAMES: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    "$Recycle.Bin",
    "$RECYCLE.BIN",
    "System Volume Information",
    "Recovery",
    "Windows",
    "Program Files",
    "Program Files (x86)",
    "ProgramData",
];

pub fn scan_path(
    root: PathBuf,
    sender: Sender<ScanMessage>,
    cancel_flag: Arc<AtomicBool>,
) {
    let start_time = Instant::now();
    let _ = sender.send(ScanMessage::Started { root: root.clone() });

    let mut total_projects = 0;
    let mut total_bytes = 0u64;

    let mut visited_projects = HashSet::new();

    let walker = jwalk::WalkDir::new(&root)
        .skip_hidden(false)
        .follow_links(false)
        .process_read_dir(move |_depth, _path, _read_dir_state, children| {
            children.retain(|dir_entry_result| {
                if let Ok(entry) = dir_entry_result {
                    if let Some(name_str) = entry.file_name.to_str() {
                        if IGNORED_DIR_NAMES.iter().any(|&ign| ign.eq_ignore_ascii_case(name_str)) {
                            return false;
                        }
                    }
                }
                true
            });
        });

    let mut last_progress_report = Instant::now();

    for entry_result in walker.into_iter() {
        if cancel_flag.load(Ordering::Relaxed) {
            break;
        }

        let entry = match entry_result {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_dir() {
            continue;
        }

        let current_dir = entry.path();

        if last_progress_report.elapsed().as_millis() > 150 {
            let _ = sender.send(ScanMessage::ScanningDirectory(current_dir.clone()));
            last_progress_report = Instant::now();
        }

        let matched_rules = match_ecosystems(&current_dir);
        if matched_rules.is_empty() {
            continue;
        }

        let dir_canonical = current_dir.canonicalize().unwrap_or_else(|_| current_dir.clone());
        if visited_projects.contains(&dir_canonical) {
            continue;
        }
        visited_projects.insert(dir_canonical);

        let project_name = current_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        for rule in matched_rules {
            let mut artifacts = Vec::new();

            for &art_name in rule.artifact_directories {
                let art_path = current_dir.join(art_name);
                if art_path.exists() && art_path.is_dir() {
                    let stats = calculate_folder_stats(&art_path);
                    if stats.file_count > 0 || stats.size_bytes > 0 {
                        artifacts.push(ArtifactFolder {
                            name: art_name.to_string(),
                            full_path: art_path,
                            size_bytes: stats.size_bytes,
                            file_count: stats.file_count,
                            last_modified: stats.latest_modified,
                        });
                    }
                }
            }

            if !artifacts.is_empty() {
                let project = ProjectInfo::new(
                    project_name.clone(),
                    current_dir.clone(),
                    rule.ecosystem,
                    artifacts,
                );

                total_projects += 1;
                total_bytes += project.total_size_bytes;

                let _ = sender.send(ScanMessage::ProjectFound(Box::new(project)));
            }
        }
    }

    let elapsed_millis = start_time.elapsed().as_millis();
    let _ = sender.send(ScanMessage::Finished {
        total_projects,
        total_bytes,
        elapsed_millis,
    });
}
