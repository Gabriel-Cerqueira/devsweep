//! # Motor de Limpeza Segura de Artefatos
//!
//! Este módulo implementa a lógica de validação de segurança e deleção de diretórios de build.
//!
//! ### Princípios de Segurança Implementados:
//! 1. **Defesa em Profundidade (*Defense in Depth*)**: Nenhuma deleção é efetuada sem antes
//!    passar por 4 checagens atômicas em `validate_artifact_safety`.
//! 2. **Prevenção de Path Traversal & Root Deletion**: Garante que o caminho do artefato
//!    seja estritamente um descendente da raiz do projeto e nunca a própria raiz.
//! 3. **Integração com a Lixeira do Windows (`trash`)**: Por padrão, utiliza a API do Windows
//!    (`SHFileOperation` / `IFileOperation`) para que arquivos possam ser recuperados na Lixeira.
//! 4. **Progresso Concorrente**: Notifica cada etapa de limpeza através de canais `mpsc::Sender`.

use crate::scanner::model::{ArtifactFolder, ProjectInfo};
use crate::scanner::rules::is_known_artifact_name;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

/// Método escolhido para a exclusão dos arquivos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeletionMethod {
    /// Move os arquivos para a Lixeira do Sistema Operacional (Permite restauração)
    Trash,
    /// Remove os arquivos definitivamente do disco usando `std::fs::remove_dir_all`
    Permanent,
}

/// Resultado detalhado de uma operação de limpeza de artefato individual.
#[derive(Debug, Clone)]
pub struct CleanResult {
    /// Caminho do artefato processado
    pub artifact_path: PathBuf,
    /// Quantidade de bytes liberados com sucesso nesta operação
    pub bytes_freed: u64,
    /// Indica se a operação obteve êxito
    pub success: bool,
    /// Mensagem de erro caso a operação tenha falhado
    pub error: Option<String>,
}

/// Mensagens assíncronas emitidas durante a exclusão em segundo plano.
#[derive(Debug, Clone)]
pub enum CleanMessage {
    CleaningArtifact {
        project_id: String,
        project_name: String,
        artifact_name: String,
        current_step: usize,
        total_steps: usize,
    },
    ArtifactCleaned {
        project_id: String,
        result: CleanResult,
    },
    Finished {
        total_freed: u64,
        deleted_count: usize,
        failed_count: usize,
    },
}

/// Validador rigoroso de segurança contra deleções acidentais.
///
/// Retorna `Ok(())` se o caminho for comprovadamente um artefato seguro, ou `Err` com a violação encontrada.
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

/// Executa a limpeza de um artefato individual aplicando as validações de segurança.
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

/// Executa a limpeza de todos os artefatos vinculados a um determinado projeto de forma síncrona.
pub fn clean_project(project: &ProjectInfo, method: DeletionMethod) -> Vec<CleanResult> {
    project
        .artifacts
        .iter()
        .map(|artifact| clean_artifact(&project.root_path, artifact, method))
        .collect()
}

/// Executa a limpeza em lote em uma thread de background enviando progresso em tempo real.
pub fn clean_projects_threaded(
    projects_to_clean: Vec<ProjectInfo>,
    method: DeletionMethod,
    sender: Sender<CleanMessage>,
) {
    let total_steps: usize = projects_to_clean.iter().map(|p| p.artifacts.len()).sum();
    let mut current_step = 0;
    let mut total_freed = 0u64;
    let mut deleted_count = 0usize;
    let mut failed_count = 0usize;

    for project in projects_to_clean {
        let mut proj_has_failure = false;

        for artifact in &project.artifacts {
            current_step += 1;

            let _ = sender.send(CleanMessage::CleaningArtifact {
                project_id: project.id.clone(),
                project_name: project.name.clone(),
                artifact_name: artifact.name.clone(),
                current_step,
                total_steps,
            });

            let res = clean_artifact(&project.root_path, artifact, method);

            if res.success {
                total_freed += res.bytes_freed;
            } else {
                proj_has_failure = true;
            }

            let _ = sender.send(CleanMessage::ArtifactCleaned {
                project_id: project.id.clone(),
                result: res,
            });
        }

        if !proj_has_failure {
            deleted_count += 1;
        } else {
            failed_count += 1;
        }
    }

    let _ = sender.send(CleanMessage::Finished {
        total_freed,
        deleted_count,
        failed_count,
    });
}
