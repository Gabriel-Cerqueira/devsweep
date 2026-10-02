use crate::scanner::model::Ecosystem;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct DetectionRule {
    pub ecosystem: Ecosystem,
    pub indicator_files: &'static [&'static str],
    pub artifact_directories: &'static [&'static str],
}

pub static RULES: &[DetectionRule] = &[
    DetectionRule {
        ecosystem: Ecosystem::Rust,
        indicator_files: &["Cargo.toml"],
        artifact_directories: &["target"],
    },
    DetectionRule {
        ecosystem: Ecosystem::Node,
        indicator_files: &["package.json"],
        artifact_directories: &[
            "node_modules",
            ".next",
            ".nuxt",
            ".turbo",
            ".svelte-kit",
            ".astro",
            ".dist",
            ".output",
        ],
    },
    DetectionRule {
        ecosystem: Ecosystem::Python,
        indicator_files: &[
            "pyproject.toml",
            "requirements.txt",
            "Pipfile",
            "setup.py",
            "tox.ini",
            "poetry.lock",
        ],
        artifact_directories: &[
            ".venv",
            "venv",
            "env",
            "__pycache__",
            ".pytest_cache",
            ".mypy_cache",
            ".ruff_cache",
            ".coverage",
        ],
    },
    DetectionRule {
        ecosystem: Ecosystem::GradleJava,
        indicator_files: &[
            "build.gradle",
            "build.gradle.kts",
            "pom.xml",
            "settings.gradle",
            "settings.gradle.kts",
            "gradlew",
        ],
        artifact_directories: &["build", ".gradle", "out", "target"],
    },
    DetectionRule {
        ecosystem: Ecosystem::DotNet,
        indicator_files: &["*.csproj", "*.fsproj", "*.sln", "global.json"],
        artifact_directories: &["bin", "obj"],
    },
    DetectionRule {
        ecosystem: Ecosystem::CppCmake,
        indicator_files: &[
            "CMakeLists.txt",
            "Makefile",
            "vcpkg.json",
            "conanfile.txt",
            "meson.build",
        ],
        artifact_directories: &[
            "build",
            "cmake-build-debug",
            "cmake-build-release",
            "out",
            "bin",
        ],
    },
    DetectionRule {
        ecosystem: Ecosystem::FlutterDart,
        indicator_files: &["pubspec.yaml"],
        artifact_directories: &[".dart_tool", "build"],
    },
    DetectionRule {
        ecosystem: Ecosystem::Elixir,
        indicator_files: &["mix.exs"],
        artifact_directories: &["_build", "deps"],
    },
    DetectionRule {
        ecosystem: Ecosystem::PhpComposer,
        indicator_files: &["composer.json"],
        artifact_directories: &["vendor"],
    },
];

pub fn match_ecosystems(dir: &Path) -> Vec<&'static DetectionRule> {
    let mut matched = Vec::new();

    for rule in RULES {
        let is_match = rule.indicator_files.iter().any(|&pattern| {
            if pattern.starts_with("*.") {
                let ext = &pattern[2..];
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        if let Some(file_ext) = entry.path().extension() {
                            if file_ext == ext {
                                return true;
                            }
                        }
                    }
                }
                false
            } else {
                dir.join(pattern).exists()
            }
        });

        if is_match {
            matched.push(rule);
        }
    }

    matched
}

pub fn is_known_artifact_name(name: &str) -> bool {
    RULES
        .iter()
        .flat_map(|r| r.artifact_directories.iter())
        .any(|&art| art == name)
}
