//! # Loop de Eventos do Terminal e Captura de Teclado
//!
//! Este módulo gerencia o ciclo de vida do terminal (modo raw, tela alternativa)
//! e despacha os eventos de entrada do teclado usando a crate `crossterm`.
//!
//! ### Conceitos Rust e Terminal Demonstrados:
//! 1. **Modo Raw (*Raw Mode*) e Alternate Screen Buffer**:
//!    - `enable_raw_mode()`: Desativa o buffer de linha e eco automático do terminal para capturar teclas instantaneamente.
//!    - `EnterAlternateScreen`: Abre um buffer de tela secundário, garantindo que ao sair do DevSweep o terminal anterior
//!      permaneça intacto.
//! 2. **Garantia de Restauração de Recursos (Padrão RAII / Cleanup)**:
//!    - Restauração explícita do terminal ao encerrar o loop (`disable_raw_mode`, `LeaveAlternateScreen`, `show_cursor`).
//! 3. **Non-blocking Event Polling (`event::poll`)**:
//!    - O loop roda a cada 50ms para atualizar animações e mensagens de progresso da varredura e limpeza,
//!      lendo teclas apenas quando disponíveis sem travar a CPU em 100%.

use crate::cleaner::DeletionMethod;
use crate::scanner::model::Ecosystem;
use crate::ui::app::{App, ModalState};
use crate::ui::views::render;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::stdout;
use std::time::Duration;

/// Inicia o loop principal da TUI, configurando o terminal e tratando o ciclo de vida.
pub fn run_tui(mut app: App) -> Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;

    let tick_rate = Duration::from_millis(50);

    while !app.should_quit {
        // Processa mensagens de varredura e limpeza em segundo plano
        app.process_scan_messages();
        app.process_clean_messages();

        // Desenha o frame atual (com animações do spinner e barra de progresso)
        terminal.draw(|f| {
            render(f, &mut app);
        })?;

        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key_event(&mut app, key.code, key.modifiers);
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

/// Trata a tecla pressionada com base no contexto ativo (busca, modais ou navegação geral).
fn handle_key_event(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    // Atalho universal para abortar/sair (Ctrl + C)
    if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    // Se estiver em processo de exclusão de arquivos, bloqueia comandos para evitar race conditions
    if app.is_cleaning {
        return;
    }

    // Contexto 1: Usuário está digitando na barra de busca ao vivo
    if app.is_searching {
        match code {
            KeyCode::Enter | KeyCode::Esc => {
                app.is_searching = false;
            }
            KeyCode::Backspace => {
                app.search_query.pop();
                app.reapply_filter_and_sort();
            }
            KeyCode::Char(c) => {
                app.search_query.push(c);
                app.reapply_filter_and_sort();
            }
            _ => {}
        }
        return;
    }

    // Contexto 2: Um modal está sobreposto na tela
    if let Some(modal) = app.active_modal {
        match modal {
            ModalState::ConfirmDelete => match code {
                KeyCode::Esc => {
                    app.active_modal = None;
                }
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    app.delete_method = DeletionMethod::Trash;
                }
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    app.delete_method = DeletionMethod::Permanent;
                }
                KeyCode::Enter => {
                    app.active_modal = None;
                    app.execute_deletion();
                }
                _ => {}
            },
            ModalState::CleaningProgress => {
                // Bloqueado enquanto limpa
            }
            ModalState::Help => match code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Enter => {
                    app.active_modal = None;
                }
                _ => {}
            },
            ModalState::FilterEcosystem => match code {
                KeyCode::Esc => {
                    app.active_modal = None;
                }
                KeyCode::Char('0') => {
                    app.ecosystem_filter = None;
                    app.active_modal = None;
                    app.reapply_filter_and_sort();
                }
                KeyCode::Char('1') => {
                    app.ecosystem_filter = Some(Ecosystem::Rust);
                    app.active_modal = None;
                    app.reapply_filter_and_sort();
                }
                KeyCode::Char('2') => {
                    app.ecosystem_filter = Some(Ecosystem::Node);
                    app.active_modal = None;
                    app.reapply_filter_and_sort();
                }
                KeyCode::Char('3') => {
                    app.ecosystem_filter = Some(Ecosystem::Python);
                    app.active_modal = None;
                    app.reapply_filter_and_sort();
                }
                KeyCode::Char('4') => {
                    app.ecosystem_filter = Some(Ecosystem::GradleJava);
                    app.active_modal = None;
                    app.reapply_filter_and_sort();
                }
                KeyCode::Char('5') => {
                    app.ecosystem_filter = Some(Ecosystem::DotNet);
                    app.active_modal = None;
                    app.reapply_filter_and_sort();
                }
                KeyCode::Char('6') => {
                    app.ecosystem_filter = Some(Ecosystem::CppCmake);
                    app.active_modal = None;
                    app.reapply_filter_and_sort();
                }
                KeyCode::Char('7') => {
                    app.ecosystem_filter = Some(Ecosystem::FlutterDart);
                    app.active_modal = None;
                    app.reapply_filter_and_sort();
                }
                _ => {}
            },
        }
        return;
    }

    // Contexto 3: Modo de navegação normal da tabela
    match code {
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_cursor_up();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_cursor_down();
        }
        KeyCode::PageUp => {
            app.move_cursor_page_up(10);
        }
        KeyCode::PageDown => {
            app.move_cursor_page_down(10);
        }
        KeyCode::Char(' ') => {
            app.toggle_selection_current();
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            app.toggle_select_all();
        }
        KeyCode::Char('d') | KeyCode::Delete => {
            if app.selected_count() > 0 || app.current_selected_project().is_some() {
                app.active_modal = Some(ModalState::ConfirmDelete);
            }
        }
        KeyCode::Char('s') => {
            app.cycle_sort();
        }
        KeyCode::Char('f') => {
            app.active_modal = Some(ModalState::FilterEcosystem);
        }
        KeyCode::Char('/') => {
            app.is_searching = true;
        }
        KeyCode::Char('r') => {
            app.start_scan();
        }
        KeyCode::Char('?') | KeyCode::Char('h') => {
            app.active_modal = Some(ModalState::Help);
        }
        KeyCode::Esc => {
            if !app.search_query.is_empty() {
                app.search_query.clear();
                app.reapply_filter_and_sort();
            }
        }
        _ => {}
    }
}
