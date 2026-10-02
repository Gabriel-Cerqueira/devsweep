use chrono::{DateTime, Local};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ecosystem {
    Rust,
    Node,
    Python,
    GradleJava,
    DotNet,
    CppCmake,
    FlutterDart,
    Elixir,
    PhpComposer,
}

impl Ecosystem {
    pub fn as_str(&self) -> &'static str {
        match self {
            Ecosystem::Rust => "Rust",
            Ecosystem::Node => "Node / JS",
            Ecosystem::Python => "Python",
            Ecosystem::GradleJava => "Java / Gradle",
            Ecosystem::DotNet => ".NET / C#",
            Ecosystem::CppCmake => "C / C++",
            Ecosystem::FlutterDart => "Flutter / Dart",
            Ecosystem::Elixir => "Elixir",
            Ecosystem::PhpComposer => "PHP / Composer",
        }
    }

    pub fn badge(&self) -> &'static str {
        match self {
            Ecosystem::Rust => "[Rust]",
            Ecosystem::Node => "[Node]",
            Ecosystem::Python => "[Py]",
            Ecosystem::GradleJava => "[Java]",
            Ecosystem::DotNet => "[.NET]",
            Ecosystem::CppCmake => "[C/C++]",
            Ecosystem::FlutterDart => "[Dart]",
            Ecosystem::Elixir => "[Elixir]",
            Ecosystem::PhpComposer => "[PHP]",
        }
    }
}

impl fmt::Display for Ecosystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct ArtifactFolder {
    pub name: String,
    pub full_path: PathBuf,
    pub size_bytes: u64,
    pub file_count: usize,
    pub last_modified: Option<DateTime<Local>>,
}

#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
    pub root_path: PathBuf,
    pub ecosystem: Ecosystem,
    pub artifacts: Vec<ArtifactFolder>,
    pub total_size_bytes: u64,
    pub last_modified: Option<DateTime<Local>>,
}

impl ProjectInfo {
    pub fn new(
        name: String,
        root_path: PathBuf,
        ecosystem: Ecosystem,
        artifacts: Vec<ArtifactFolder>,
    ) -> Self {
        let total_size_bytes = artifacts.iter().map(|a| a.size_bytes).sum();
        let last_modified = artifacts
            .iter()
            .filter_map(|a| a.last_modified)
            .max();

        let id = format!("{}:{}", ecosystem.as_str(), root_path.display());

        Self {
            id,
            name,
            root_path,
            ecosystem,
            artifacts,
            total_size_bytes,
            last_modified,
        }
    }

    pub fn recalculate_totals(&mut self) {
        self.total_size_bytes = self.artifacts.iter().map(|a| a.size_bytes).sum();
        self.last_modified = self.artifacts.iter().filter_map(|a| a.last_modified).max();
    }
}

#[derive(Debug, Clone)]
pub enum ScanMessage {
    Started { root: PathBuf },
    ScanningDirectory(PathBuf),
    ProjectFound(Box<ProjectInfo>),
    Finished {
        total_projects: usize,
        total_bytes: u64,
        elapsed_millis: u128,
    },
    #[allow(dead_code)]
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Size,
    Age,
    Name,
    Ecosystem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_age(date: Option<DateTime<Local>>) -> String {
    match date {
        Some(d) => {
            let now = Local::now();
            let duration = now.signed_duration_since(d);
            let days = duration.num_days();

            if days <= 0 {
                let hours = duration.num_hours();
                if hours <= 0 {
                    let mins = duration.num_minutes();
                    if mins <= 0 {
                        "just now".to_string()
                    } else {
                        format!("{}m ago", mins)
                    }
                } else {
                    format!("{}h ago", hours)
                }
            } else if days == 1 {
                "yesterday".to_string()
            } else if days < 30 {
                format!("{}d ago", days)
            } else if days < 365 {
                format!("{}mo ago", days / 30)
            } else {
                format!("{}y ago", days / 365)
            }
        }
        None => "unknown".to_string(),
    }
}
