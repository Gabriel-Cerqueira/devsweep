use crate::cleaner::{clean_project, DeletionMethod};
use crate::scanner::model::{
    format_bytes, Ecosystem, ProjectInfo, ScanMessage, SortDirection, SortField,
};
use crate::scanner::walker::scan_path;
use chrono::Local;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalState {
    ConfirmDelete,
    Help,
    FilterEcosystem,
}

pub struct App {
    pub root_path: PathBuf,
    pub projects: Vec<ProjectInfo>,
    pub filtered_indices: Vec<usize>,
    pub selected_ids: HashSet<String>,
    pub cursor_index: usize,
    pub scroll_offset: usize,

    pub search_query: String,
    pub is_searching: bool,

    pub sort_field: SortField,
    pub sort_direction: SortDirection,
    pub ecosystem_filter: Option<Ecosystem>,

    pub status_message: Option<(String, chrono::DateTime<chrono::Local>)>,
    pub is_scanning: bool,
    pub current_scanning_dir: Option<PathBuf>,
    pub scan_cancel: Arc<AtomicBool>,
    pub scan_rx: Receiver<ScanMessage>,
    pub scan_tx: Sender<ScanMessage>,

    pub active_modal: Option<ModalState>,
    pub delete_method: DeletionMethod,

    pub total_scanned_bytes: u64,
    pub total_freed_bytes: u64,
    pub elapsed_scan_millis: u128,
    pub should_quit: bool,
}

impl App {
    pub fn new(root_path: PathBuf) -> Self {
        let (scan_tx, scan_rx) = channel();
        let scan_cancel = Arc::new(AtomicBool::new(false));

        let mut app = Self {
            root_path: root_path.clone(),
            projects: Vec::new(),
            filtered_indices: Vec::new(),
            selected_ids: HashSet::new(),
            cursor_index: 0,
            scroll_offset: 0,
            search_query: String::new(),
            is_searching: false,
            sort_field: SortField::Size,
            sort_direction: SortDirection::Descending,
            ecosystem_filter: None,
            status_message: None,
            is_scanning: false,
            current_scanning_dir: None,
            scan_cancel,
            scan_rx,
            scan_tx,
            active_modal: None,
            delete_method: DeletionMethod::Trash,
            total_scanned_bytes: 0,
            total_freed_bytes: 0,
            elapsed_scan_millis: 0,
            should_quit: false,
        };

        app.start_scan();
        app
    }

    pub fn start_scan(&mut self) {
        if self.is_scanning {
            self.scan_cancel.store(true, Ordering::Relaxed);
        }

        self.projects.clear();
        self.filtered_indices.clear();
        self.selected_ids.clear();
        self.cursor_index = 0;
        self.scroll_offset = 0;
        self.total_scanned_bytes = 0;
        self.is_scanning = true;

        let (new_tx, new_rx) = channel();
        self.scan_tx = new_tx;
        self.scan_rx = new_rx;

        self.scan_cancel = Arc::new(AtomicBool::new(false));
        let cancel_flag = Arc::clone(&self.scan_cancel);
        let sender = self.scan_tx.clone();
        let scan_root = self.root_path.clone();

        thread::spawn(move || {
            scan_path(scan_root, sender, cancel_flag);
        });
    }

    pub fn process_scan_messages(&mut self) {
        while let Ok(msg) = self.scan_rx.try_recv() {
            match msg {
                ScanMessage::Started { root } => {
                    self.current_scanning_dir = Some(root);
                }
                ScanMessage::ScanningDirectory(path) => {
                    self.current_scanning_dir = Some(path);
                }
                ScanMessage::ProjectFound(project) => {
                    self.total_scanned_bytes += project.total_size_bytes;
                    self.projects.push(*project);
                    self.reapply_filter_and_sort();
                }
                ScanMessage::Finished {
                    total_projects,
                    total_bytes,
                    elapsed_millis,
                } => {
                    self.is_scanning = false;
                    self.current_scanning_dir = None;
                    self.total_scanned_bytes = total_bytes;
                    self.elapsed_scan_millis = elapsed_millis;
                    self.set_status(format!(
                        "Scan finished in {:.2}s. Found {} projects ({})",
                        elapsed_millis as f64 / 1000.0,
                        total_projects,
                        format_bytes(total_bytes)
                    ));
                    self.reapply_filter_and_sort();
                }
                ScanMessage::Error(err) => {
                    self.is_scanning = false;
                    self.set_status(format!("Scan error: {}", err));
                }
            }
        }
    }

    pub fn set_status<S: Into<String>>(&mut self, msg: S) {
        self.status_message = Some((msg.into(), Local::now()));
    }

    pub fn reapply_filter_and_sort(&mut self) {
        let query = self.search_query.trim().to_lowercase();
        let eco_filter = self.ecosystem_filter;

        let mut matched_indices: Vec<usize> = self
            .projects
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                if let Some(ref eco) = eco_filter {
                    if p.ecosystem != *eco {
                        return false;
                    }
                }

                if !query.is_empty() {
                    let matches_name = p.name.to_lowercase().contains(&query);
                    let matches_path = p
                        .root_path
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&query);
                    let matches_eco = p.ecosystem.as_str().to_lowercase().contains(&query);
                    let matches_art = p
                        .artifacts
                        .iter()
                        .any(|a| a.name.to_lowercase().contains(&query));

                    return matches_name || matches_path || matches_eco || matches_art;
                }

                true
            })
            .map(|(idx, _)| idx)
            .collect();

        // Sort
        let sort_field = self.sort_field;
        let sort_dir = self.sort_direction;
        let projects_ref = &self.projects;

        matched_indices.sort_by(|&a_idx, &b_idx| {
            let a = &projects_ref[a_idx];
            let b = &projects_ref[b_idx];

            let ord = match sort_field {
                SortField::Size => a.total_size_bytes.cmp(&b.total_size_bytes),
                SortField::Age => {
                    let a_age = a.last_modified;
                    let b_age = b.last_modified;
                    // None (unknown) considered older/newer
                    a_age.cmp(&b_age)
                }
                SortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortField::Ecosystem => a.ecosystem.as_str().cmp(b.ecosystem.as_str()),
            };

            match sort_dir {
                SortDirection::Ascending => ord,
                SortDirection::Descending => ord.reverse(),
            }
        });

        self.filtered_indices = matched_indices;

        if self.cursor_index >= self.filtered_indices.len() {
            self.cursor_index = self.filtered_indices.len().saturating_sub(1);
        }
    }

    pub fn toggle_selection_current(&mut self) {
        if let Some(&proj_idx) = self.filtered_indices.get(self.cursor_index) {
            let proj = &self.projects[proj_idx];
            if self.selected_ids.contains(&proj.id) {
                self.selected_ids.remove(&proj.id);
            } else {
                self.selected_ids.insert(proj.id.clone());
            }
        }
    }

    pub fn toggle_select_all(&mut self) {
        let all_selected = self
            .filtered_indices
            .iter()
            .all(|&idx| self.selected_ids.contains(&self.projects[idx].id));

        if all_selected {
            for &idx in &self.filtered_indices {
                self.selected_ids.remove(&self.projects[idx].id);
            }
        } else {
            for &idx in &self.filtered_indices {
                self.selected_ids.insert(self.projects[idx].id.clone());
            }
        }
    }

    pub fn selected_bytes(&self) -> u64 {
        self.projects
            .iter()
            .filter(|p| self.selected_ids.contains(&p.id))
            .map(|p| p.total_size_bytes)
            .sum()
    }

    pub fn selected_count(&self) -> usize {
        self.selected_ids.len()
    }

    pub fn current_selected_project(&self) -> Option<&ProjectInfo> {
        self.filtered_indices
            .get(self.cursor_index)
            .and_then(|&idx| self.projects.get(idx))
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor_index > 0 {
            self.cursor_index -= 1;
        }
    }

    pub fn move_cursor_down(&mut self) {
        if !self.filtered_indices.is_empty() && self.cursor_index + 1 < self.filtered_indices.len() {
            self.cursor_index += 1;
        }
    }

    pub fn move_cursor_page_up(&mut self, page_size: usize) {
        self.cursor_index = self.cursor_index.saturating_sub(page_size);
    }

    pub fn move_cursor_page_down(&mut self, page_size: usize) {
        if !self.filtered_indices.is_empty() {
            self.cursor_index = (self.cursor_index + page_size).min(self.filtered_indices.len() - 1);
        }
    }

    pub fn cycle_sort(&mut self) {
        match (self.sort_field, self.sort_direction) {
            (SortField::Size, SortDirection::Descending) => {
                self.sort_direction = SortDirection::Ascending;
            }
            (SortField::Size, SortDirection::Ascending) => {
                self.sort_field = SortField::Age;
                self.sort_direction = SortDirection::Ascending; // oldest first
            }
            (SortField::Age, SortDirection::Ascending) => {
                self.sort_direction = SortDirection::Descending;
            }
            (SortField::Age, SortDirection::Descending) => {
                self.sort_field = SortField::Name;
                self.sort_direction = SortDirection::Ascending;
            }
            (SortField::Name, _) => {
                self.sort_field = SortField::Ecosystem;
                self.sort_direction = SortDirection::Ascending;
            }
            (SortField::Ecosystem, _) => {
                self.sort_field = SortField::Size;
                self.sort_direction = SortDirection::Descending;
            }
        }
        self.reapply_filter_and_sort();
    }

    pub fn execute_deletion(&mut self) {
        let to_delete_ids: Vec<String> = if self.selected_ids.is_empty() {
            if let Some(current) = self.current_selected_project() {
                vec![current.id.clone()]
            } else {
                return;
            }
        } else {
            self.selected_ids.iter().cloned().collect()
        };

        let mut freed_in_batch = 0u64;
        let mut deleted_count = 0usize;
        let mut failed_count = 0usize;

        let method = self.delete_method;

        self.projects.retain_mut(|project| {
            if to_delete_ids.contains(&project.id) {
                let results = clean_project(project, method);
                let all_success = results.iter().all(|r| r.success);
                let bytes_freed: u64 = results.iter().map(|r| r.bytes_freed).sum();

                if all_success {
                    freed_in_batch += bytes_freed;
                    deleted_count += 1;
                    false // Remove from list
                } else {
                    failed_count += 1;
                    // Recalculate remaining
                    project.artifacts.retain(|a| {
                        !results
                            .iter()
                            .any(|r| r.artifact_path == a.full_path && r.success)
                    });
                    project.recalculate_totals();
                    true // Keep remaining
                }
            } else {
                true
            }
        });

        self.total_freed_bytes += freed_in_batch;
        self.total_scanned_bytes = self.total_scanned_bytes.saturating_sub(freed_in_batch);
        self.selected_ids.clear();
        self.reapply_filter_and_sort();

        let target_name = match method {
            DeletionMethod::Trash => "moved to Recycle Bin",
            DeletionMethod::Permanent => "permanently deleted",
        };

        if failed_count > 0 {
            self.set_status(format!(
                "Cleaned {} projects ({}), but {} had errors ({})",
                deleted_count,
                format_bytes(freed_in_batch),
                failed_count,
                target_name
            ));
        } else {
            self.set_status(format!(
                "Successfully cleaned {} projects. Freed {} ({})",
                deleted_count,
                format_bytes(freed_in_batch),
                target_name
            ));
        }
    }
}
