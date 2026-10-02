//! # Gerenciador de Estado da Aplicação TUI
//!
//! Este módulo gerencia o estado completo da interface de terminal do DevSweep:
//! lista de projetos descobertos, filtros ativos, ordenação, seleção de itens,
//! controle do cursor e execução assíncrona de limpezas com feedback visual contínuo.
//!
//! ### Conceitos Rust Demonstrados:
//! 1. **Padrão State Machine & Single Source of Truth**:
//!    - Todos os estados da UI residem na struct `App`.
//!    - `filtered_indices: Vec<usize>` atua como uma visão projetada (índices que apontam para `projects`),
//!      evitando duplicação de instâncias de `ProjectInfo`.
//! 2. **`try_recv()` para I/O e Deleção Não-Bloqueante**:
//!    - As operações pesadas de I/O (varredura e exclusão na Lixeira) rodam em threads separadas,
//!      enviando mensagens por canais MPSC enquanto a TUI renderiza a 60 FPS com animação e barra de progresso.
//! 3. **`TableState` para Auto-Scroll**:
//!    - Gerencia automaticamente a janela visível da tabela conforme o cursor se move para baixo ou para cima.

use crate::cleaner::{clean_projects_threaded, CleanMessage, DeletionMethod};
use crate::scanner::model::{
    format_bytes, Ecosystem, ProjectInfo, ScanMessage, SortDirection, SortField,
};
use crate::scanner::walker::scan_path;
use chrono::Local;
use ratatui::widgets::TableState;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;

/// Estados possíveis de modais e caixas de diálogo sobrepostas na tela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalState {
    /// Modal de confirmação de exclusão dos projetos selecionados
    ConfirmDelete,
    /// Modal de progresso visual com spinner e barra de status durante a exclusão
    CleaningProgress,
    /// Modal de ajuda com atalhos de teclado
    Help,
    /// Modal de seleção rápida de filtro por ecossistema
    FilterEcosystem,
}

/// Informações de progresso em tempo real da exclusão de artefatos.
#[derive(Debug, Clone)]
pub struct CleanProgressInfo {
    pub project_name: String,
    pub artifact_name: String,
    pub current_step: usize,
    pub total_steps: usize,
    pub bytes_freed_so_far: u64,
    pub spinner_tick: usize,
}

/// Estrutura central que guarda todo o estado da interface gráfica do terminal.
pub struct App {
    /// Diretório raiz configurado para a varredura
    pub root_path: PathBuf,
    /// Lista master de todos os projetos identificados
    pub projects: Vec<ProjectInfo>,
    /// Índices dos projetos em `projects` que passam pelos critérios atuais de filtro e ordenação
    pub filtered_indices: Vec<usize>,
    /// Conjunto de IDs de projetos marcados pelo usuário com checkbox `[x]`
    pub selected_ids: HashSet<String>,
    /// Posição do cursor/linha destacada na visualização atual
    pub cursor_index: usize,
    /// Estado da tabela do Ratatui para auto-scroll e controle de offset de visualização
    pub table_state: TableState,

    /// Termo digitado na barra de busca ao vivo
    pub search_query: String,
    /// Indica se o modo de digitação de busca está ativo
    pub is_searching: bool,

    /// Campo atualmente usado para ordenação (Tamanho, Idade, Nome, Ecossistema)
    pub sort_field: SortField,
    /// Direção da ordenação (Crescente / Decrescente)
    pub sort_direction: SortDirection,
    /// Filtro restritivo de ecossistema (opcional)
    pub ecosystem_filter: Option<Ecosystem>,

    /// Mensagem temporária exibida na barra de status inferior (com timestamp de expiração)
    pub status_message: Option<(String, chrono::DateTime<chrono::Local>)>,
    /// Indica se uma thread de varredura está em execução em segundo plano
    pub is_scanning: bool,
    /// Último diretório visitado pela varredura (para indicador ao vivo)
    pub current_scanning_dir: Option<PathBuf>,
    /// Flag atômica compartilhada para solicitar cancelamento da varredura
    pub scan_cancel: Arc<AtomicBool>,
    /// Canal receptor de mensagens de varredura
    pub scan_rx: Receiver<ScanMessage>,
    /// Canal transmissor de mensagens de varredura
    pub scan_tx: Sender<ScanMessage>,

    /// Indica se uma operação de limpeza de arquivos está em andamento
    pub is_cleaning: bool,
    /// Dados de progresso da limpeza em execução
    pub clean_progress_info: Option<CleanProgressInfo>,
    /// Canal receptor de mensagens de limpeza
    pub clean_rx: Receiver<CleanMessage>,
    /// Canal transmissor de mensagens de limpeza
    pub clean_tx: Sender<CleanMessage>,

    /// Modal ativo na tela (se houver)
    pub active_modal: Option<ModalState>,
    /// Método de exclusão configurado (Lixeira vs Permanente)
    pub delete_method: DeletionMethod,

    /// Total de bytes de cache encontrados na varredura
    pub total_scanned_bytes: u64,
    /// Total de bytes de cache liberados durante a sessão
    pub total_freed_bytes: u64,
    /// Duração da última varredura em milissegundos
    pub elapsed_scan_millis: u128,
    /// Flag para sinalizar encerramento gracioso do programa
    pub should_quit: bool,
}

impl App {
    /// Inicializa uma nova sessão da TUI e dispara a primeira varredura em segundo plano.
    pub fn new(root_path: PathBuf) -> Self {
        let (scan_tx, scan_rx) = channel();
        let (clean_tx, clean_rx) = channel();
        let scan_cancel = Arc::new(AtomicBool::new(false));
        let mut table_state = TableState::default();
        table_state.select(Some(0));

        let mut app = Self {
            root_path: root_path.clone(),
            projects: Vec::new(),
            filtered_indices: Vec::new(),
            selected_ids: HashSet::new(),
            cursor_index: 0,
            table_state,
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
            is_cleaning: false,
            clean_progress_info: None,
            clean_rx,
            clean_tx,
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

    /// Dispara uma nova varredura assíncrona da árvore de diretórios.
    pub fn start_scan(&mut self) {
        if self.is_scanning {
            self.scan_cancel.store(true, Ordering::Relaxed);
        }

        self.projects.clear();
        self.filtered_indices.clear();
        self.selected_ids.clear();
        self.cursor_index = 0;
        self.table_state.select(Some(0));
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

    /// Processa mensagens pendentes enviadas pela thread de varredura.
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

    /// Processa mensagens pendentes emitidas pela thread assíncrona de limpeza.
    pub fn process_clean_messages(&mut self) {
        if let Some(ref mut progress) = self.clean_progress_info {
            progress.spinner_tick = progress.spinner_tick.wrapping_add(1);
        }

        while let Ok(msg) = self.clean_rx.try_recv() {
            match msg {
                CleanMessage::CleaningArtifact {
                    project_name,
                    artifact_name,
                    current_step,
                    total_steps,
                    ..
                } => {
                    let prev_freed = self
                        .clean_progress_info
                        .as_ref()
                        .map(|p| p.bytes_freed_so_far)
                        .unwrap_or(0);
                    let tick = self
                        .clean_progress_info
                        .as_ref()
                        .map(|p| p.spinner_tick)
                        .unwrap_or(0);

                    self.clean_progress_info = Some(CleanProgressInfo {
                        project_name,
                        artifact_name,
                        current_step,
                        total_steps,
                        bytes_freed_so_far: prev_freed,
                        spinner_tick: tick,
                    });
                }
                CleanMessage::ArtifactCleaned {
                    project_id,
                    result,
                } => {
                    if result.success {
                        self.total_freed_bytes += result.bytes_freed;
                        self.total_scanned_bytes =
                            self.total_scanned_bytes.saturating_sub(result.bytes_freed);

                        if let Some(ref mut progress) = self.clean_progress_info {
                            progress.bytes_freed_so_far += result.bytes_freed;
                        }

                        // Remove artefato do projeto em memória
                        if let Some(proj) = self.projects.iter_mut().find(|p| p.id == project_id) {
                            proj.artifacts.retain(|a| a.full_path != result.artifact_path);
                            proj.recalculate_totals();
                        }
                    }
                }
                CleanMessage::Finished {
                    total_freed,
                    deleted_count,
                    failed_count,
                } => {
                    self.is_cleaning = false;
                    self.clean_progress_info = None;
                    self.active_modal = None;

                    // Remove projetos que ficaram sem nenhum artefato
                    self.projects.retain(|p| !p.artifacts.is_empty());
                    self.selected_ids.clear();
                    self.reapply_filter_and_sort();

                    let target_name = match self.delete_method {
                        DeletionMethod::Trash => "moved to Recycle Bin",
                        DeletionMethod::Permanent => "permanently deleted",
                    };

                    if failed_count > 0 {
                        self.set_status(format!(
                            "Cleaned {} projects ({}), but {} had errors ({})",
                            deleted_count,
                            format_bytes(total_freed),
                            failed_count,
                            target_name
                        ));
                    } else {
                        self.set_status(format!(
                            "Successfully cleaned {} projects. Freed {} ({})",
                            deleted_count,
                            format_bytes(total_freed),
                            target_name
                        ));
                    }
                }
            }
        }
    }

    /// Atualiza a mensagem na barra de status inferior.
    pub fn set_status<S: Into<String>>(&mut self, msg: S) {
        self.status_message = Some((msg.into(), Local::now()));
    }

    /// Filtra e reordena a lista de visualização com base na busca, filtro de ecossistema e ordenação ativa.
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

        // Ordenação
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

        if self.filtered_indices.is_empty() {
            self.cursor_index = 0;
            self.table_state.select(None);
        } else {
            if self.cursor_index >= self.filtered_indices.len() {
                self.cursor_index = self.filtered_indices.len().saturating_sub(1);
            }
            self.table_state.select(Some(self.cursor_index));
        }
    }

    /// Alterna a seleção do projeto atualmente sob o cursor.
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

    /// Seleciona ou desmarca todos os projetos atualmente visíveis no filtro.
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

    /// Retorna a soma em bytes dos projetos selecionados.
    pub fn selected_bytes(&self) -> u64 {
        self.projects
            .iter()
            .filter(|p| self.selected_ids.contains(&p.id))
            .map(|p| p.total_size_bytes)
            .sum()
    }

    /// Retorna a quantidade de projetos selecionados.
    pub fn selected_count(&self) -> usize {
        self.selected_ids.len()
    }

    /// Retorna a referência ao projeto sob o cursor.
    pub fn current_selected_project(&self) -> Option<&ProjectInfo> {
        self.filtered_indices
            .get(self.cursor_index)
            .and_then(|&idx| self.projects.get(idx))
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor_index > 0 {
            self.cursor_index -= 1;
            self.table_state.select(Some(self.cursor_index));
        }
    }

    pub fn move_cursor_down(&mut self) {
        if !self.filtered_indices.is_empty() && self.cursor_index + 1 < self.filtered_indices.len() {
            self.cursor_index += 1;
            self.table_state.select(Some(self.cursor_index));
        }
    }

    pub fn move_cursor_page_up(&mut self, page_size: usize) {
        self.cursor_index = self.cursor_index.saturating_sub(page_size);
        if !self.filtered_indices.is_empty() {
            self.table_state.select(Some(self.cursor_index));
        }
    }

    pub fn move_cursor_page_down(&mut self, page_size: usize) {
        if !self.filtered_indices.is_empty() {
            self.cursor_index = (self.cursor_index + page_size).min(self.filtered_indices.len() - 1);
            self.table_state.select(Some(self.cursor_index));
        }
    }

    /// Alterna ciclicamente o modo de ordenação (Tamanho -> Idade -> Nome -> Ecossistema).
    pub fn cycle_sort(&mut self) {
        match (self.sort_field, self.sort_direction) {
            (SortField::Size, SortDirection::Descending) => {
                self.sort_direction = SortDirection::Ascending;
            }
            (SortField::Size, SortDirection::Ascending) => {
                self.sort_field = SortField::Age;
                self.sort_direction = SortDirection::Ascending;
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

    /// Dispara a exclusão física ou envio para a lixeira dos projetos selecionados em segundo plano.
    pub fn execute_deletion(&mut self) {
        if self.is_cleaning {
            return;
        }

        let to_delete_ids: Vec<String> = if self.selected_ids.is_empty() {
            if let Some(current) = self.current_selected_project() {
                vec![current.id.clone()]
            } else {
                return;
            }
        } else {
            self.selected_ids.iter().cloned().collect()
        };

        let projects_to_clean: Vec<ProjectInfo> = self
            .projects
            .iter()
            .filter(|p| to_delete_ids.contains(&p.id))
            .cloned()
            .collect();

        if projects_to_clean.is_empty() {
            return;
        }

        self.is_cleaning = true;
        self.active_modal = Some(ModalState::CleaningProgress);

        let total_steps: usize = projects_to_clean.iter().map(|p| p.artifacts.len()).sum();
        self.clean_progress_info = Some(CleanProgressInfo {
            project_name: projects_to_clean[0].name.clone(),
            artifact_name: "preparing...".to_string(),
            current_step: 0,
            total_steps,
            bytes_freed_so_far: 0,
            spinner_tick: 0,
        });

        let (new_clean_tx, new_clean_rx) = channel();
        self.clean_tx = new_clean_tx;
        self.clean_rx = new_clean_rx;

        let sender = self.clean_tx.clone();
        let method = self.delete_method;

        thread::spawn(move || {
            clean_projects_threaded(projects_to_clean, method, sender);
        });
    }
}
