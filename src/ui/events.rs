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

pub fn run_tui(mut app: App) -> Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;

    let tick_rate = Duration::from_millis(50);

    while !app.should_quit {
        app.process_scan_messages();

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

fn handle_key_event(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    if modifiers.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

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
