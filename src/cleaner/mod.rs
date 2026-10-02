use crate::scanner::model::{ArtifactFolder, ProjectInfo};
use crate::scanner::rules::is_known_artifact_name;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeletionMethod {
    Trash,
    Permanent,
}

#[derive(Debug, Clone)]
pub struct CleanResult {
    pub artifact_path: PathBuf,
    pub bytes_freed: u64,
    pub success: bool,
    pub error: Option<String>,
}

pub fn validate_artifact_safety(project_root: &Path, artifact_path: &Path) -> Result<()> {
    if !artifact_path.exists() {
        bail!("Artifact directory does not exist: {}", artifact_path.display());
    }

    if !artifact_path.starts_with(project_root) {
        bail!(
            "Security violation: artifact '{}' is outside project root '{}'",
            artifact_path.display(),
            project_root.display()
        );
    }

    if artifact_path == project_root {
        bail!("Security violation: cannot delete the project root directory itself");
    }

    let folder_name = artifact_path
        .file_name()
        .and_then(|n| n.to_str())
        .context("Invalid folder name")?;

    if !is_known_artifact_name(folder_name) {
        bail!(
            "Security violation: '{}' is not recognized as a disposable build artifact folder",
            folder_name
        );
    }

    Ok(())
}

pub fn clean_artifact(
    project_root: &Path,
    artifact: &ArtifactFolder,
    method: DeletionMethod,
) -> CleanResult {
    let mut result = CleanResult {
        artifact_path: artifact.full_path.clone(),
        bytes_freed: 0,
        success: false,
        error: None,
    };

    if let Err(err) = validate_artifact_safety(project_root, &artifact.full_path) {
        result.error = Some(err.to_string());
        return result;
    }

    let delete_op = match method {
        DeletionMethod::Trash => trash::delete(&artifact.full_path)
            .map_err(|e| anyhow::anyhow!("Trash operation failed: {}", e)),
        DeletionMethod::Permanent => fs::remove_dir_all(&artifact.full_path)
            .map_err(|e| anyhow::anyhow!("Permanent deletion failed: {}", e)),
    };

    match delete_op {
        Ok(_) => {
            result.bytes_freed = artifact.size_bytes;
            result.success = true;
        }
        Err(err) => {
            result.error = Some(err.to_string());
        }
    }

    result
}

pub fn clean_project(project: &ProjectInfo, method: DeletionMethod) -> Vec<CleanResult> {
    project
        .artifacts
        .iter()
        .map(|artifact| clean_artifact(&project.root_path, artifact, method))
        .collect()
}
