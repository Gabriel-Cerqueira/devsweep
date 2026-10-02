use chrono::{DateTime, Local};
use std::fs;
use std::path::Path;
use std::time::SystemTime;

#[derive(Debug, Default)]
pub struct FolderStats {
    pub size_bytes: u64,
    pub file_count: usize,
    pub latest_modified: Option<DateTime<Local>>,
}

pub fn calculate_folder_stats(path: &Path) -> FolderStats {
    let mut stats = FolderStats::default();
    let mut latest_sys_time: Option<SystemTime> = None;

    let walker = jwalk::WalkDir::new(path)
        .skip_hidden(false)
        .follow_links(false);

    for entry_result in walker {
        let entry = match entry_result {
            Ok(e) => e,
            Err(_) => continue,
        };

        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                stats.size_bytes += meta.len();
                stats.file_count += 1;

                if let Ok(mod_time) = meta.modified() {
                    match latest_sys_time {
                        Some(prev) if mod_time > prev => {
                            latest_sys_time = Some(mod_time);
                        }
                        None => {
                            latest_sys_time = Some(mod_time);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    if let Some(sys_time) = latest_sys_time {
        let dt: DateTime<Local> = sys_time.into();
        stats.latest_modified = Some(dt);
    } else if let Ok(meta) = fs::metadata(path) {
        if let Ok(mod_time) = meta.modified() {
            let dt: DateTime<Local> = mod_time.into();
            stats.latest_modified = Some(dt);
        }
    }

    stats
}
