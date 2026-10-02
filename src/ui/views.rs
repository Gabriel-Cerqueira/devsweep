use crate::cleaner::DeletionMethod;
use crate::scanner::model::{format_age, format_bytes, Ecosystem, SortDirection, SortField};
use crate::ui::app::{App, ModalState};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table, Wrap,
    },
    Frame,
};

pub fn render(f: &mut Frame, app: &App) {
    let size = f.area();

    // Main layout: Header (3 lines), Content (min 10), Footer (2 lines)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(size);

    render_header(f, app, chunks[0]);
    render_body(f, app, chunks[1]);
    render_footer(f, app, chunks[2]);

    if let Some(modal) = app.active_modal {
        match modal {
            ModalState::ConfirmDelete => render_confirm_modal(f, app, size),
            ModalState::Help => render_help_modal(f, size),
            ModalState::FilterEcosystem => render_filter_modal(f, app, size),
        }
    }
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let scan_status_span = if app.is_scanning {
        let path_text = app
            .current_scanning_dir
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "scanning...".to_string());
        let truncated = if path_text.len() > 35 {
            format!("...{}", &path_text[path_text.len().saturating_sub(32)..])
        } else {
            path_text
        };
        Span::styled(
            format!(" [SCANNING: {}] ", truncated),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            format!(" [SCAN READY ({:.2}s)] ", app.elapsed_scan_millis as f64 / 1000.0),
            Style::default().fg(Color::Green),
        )
    };

    let total_found_str = format_bytes(app.total_scanned_bytes);
    let selected_str = format_bytes(app.selected_bytes());
    let freed_str = format_bytes(app.total_freed_bytes);

    let stats_line = Line::from(vec![
        Span::styled(
            " DevSweep ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        scan_status_span,
        Span::raw(" | Projects: "),
        Span::styled(
            format!("{}", app.projects.len()),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw(" | Total Cache: "),
        Span::styled(total_found_str, Style::default().fg(Color::Yellow)),
        Span::raw(" | Selected: "),
        Span::styled(
            format!("{} ({})", app.selected_count(), selected_str),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | Freed: "),
        Span::styled(freed_str, Style::default().fg(Color::Green)),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let header_para = Paragraph::new(stats_line).block(block);
    f.render_widget(header_para, area);
}

fn render_body(f: &mut Frame, app: &App, area: Rect) {
    // Split into Left Table (65%) and Right Inspector (35%)
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(66), Constraint::Percentage(34)])
        .split(area);

    render_projects_table(f, app, body_chunks[0]);
    render_inspector(f, app, body_chunks[1]);
}

fn get_ecosystem_color(eco: Ecosystem) -> Color {
    match eco {
        Ecosystem::Rust => Color::Rgb(222, 100, 48),
        Ecosystem::Node => Color::Rgb(104, 187, 89),
        Ecosystem::Python => Color::Rgb(255, 212, 59),
        Ecosystem::GradleJava => Color::Rgb(231, 111, 81),
        Ecosystem::DotNet => Color::Rgb(81, 43, 212),
        Ecosystem::CppCmake => Color::Rgb(97, 175, 239),
        Ecosystem::FlutterDart => Color::Rgb(41, 182, 246),
        Ecosystem::Elixir => Color::Rgb(163, 112, 247),
        Ecosystem::PhpComposer => Color::Rgb(136, 146, 191),
    }
}

fn render_projects_table(f: &mut Frame, app: &App, area: Rect) {
    let sort_indicator = match app.sort_direction {
        SortDirection::Ascending => "^",
        SortDirection::Descending => "v",
    };

    let sort_label = match app.sort_field {
        SortField::Size => format!("Size {}", sort_indicator),
        SortField::Age => format!("Age {}", sort_indicator),
        SortField::Name => format!("Name {}", sort_indicator),
        SortField::Ecosystem => format!("Type {}", sort_indicator),
    };

    let filter_label = if let Some(eco) = app.ecosystem_filter {
        format!(" [Filter: {}]", eco.as_str())
    } else {
        String::new()
    };

    let search_label = if app.is_searching {
        format!(" [Search: {}_]", app.search_query)
    } else if !app.search_query.is_empty() {
        format!(" [Search: {}]", app.search_query)
    } else {
        String::new()
    };

    let title = format!(
        " Projects ({}/{}) - Sort: {}{} ",
        app.filtered_indices.len(),
        app.projects.len(),
        sort_label,
        if !search_label.is_empty() {
            &search_label
        } else {
            &filter_label
        }
    );

    let header_cells = ["  ", "Type", "Project", "Artifacts", "Size", "Modified"]
        .iter()
        .map(|h| {
            Cell::from(*h).style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
        });
    let header_row = Row::new(header_cells)
        .style(Style::default().bg(Color::Rgb(24, 28, 36)))
        .height(1);

    let rows: Vec<Row> = app
        .filtered_indices
        .iter()
        .enumerate()
        .map(|(view_idx, &proj_idx)| {
            let proj = &app.projects[proj_idx];
            let is_selected = app.selected_ids.contains(&proj.id);
            let is_cursor = view_idx == app.cursor_index;

            let check_mark = if is_selected { "[x]" } else { "[ ]" };
            let eco_badge = proj.ecosystem.badge();
            let eco_color = get_ecosystem_color(proj.ecosystem);

            let artifact_names: Vec<&str> = proj.artifacts.iter().map(|a| a.name.as_str()).collect();
            let artifacts_str = artifact_names.join(", ");

            let size_str = format_bytes(proj.total_size_bytes);
            let age_str = format_age(proj.last_modified);

            let cells = vec![
                Cell::from(Span::styled(
                    check_mark,
                    if is_selected {
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                )),
                Cell::from(Span::styled(
                    eco_badge,
                    Style::default().fg(eco_color).add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::raw(&proj.name)),
                Cell::from(Span::styled(
                    artifacts_str,
                    Style::default().fg(Color::DarkGray),
                )),
                Cell::from(Span::styled(
                    size_str,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::styled(age_str, Style::default().fg(Color::Gray))),
            ];

            let mut row = Row::new(cells).height(1);
            if is_cursor {
                row = row.style(
                    Style::default()
                        .bg(Color::Rgb(40, 50, 75))
                        .add_modifier(Modifier::BOLD),
                );
            }
            row
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Length(8),
        Constraint::Percentage(32),
        Constraint::Percentage(26),
        Constraint::Length(10),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths)
        .header(header_row)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(if app.is_searching {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::DarkGray)
                }),
        );

    f.render_widget(table, area);
}

fn render_inspector(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Project Details ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let content: Vec<Line> = if let Some(proj) = app.current_selected_project() {
        let eco_color = get_ecosystem_color(proj.ecosystem);

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Name: ", Style::default().fg(Color::Cyan)),
                Span::styled(&proj.name, Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("Ecosystem: ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    proj.ecosystem.as_str(),
                    Style::default().fg(eco_color).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Root Path: ", Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![Span::styled(
                proj.root_path.to_string_lossy().to_string(),
                Style::default().fg(Color::Gray),
            )]),
            Line::from(vec![
                Span::styled("Total Cache: ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    format_bytes(proj.total_size_bytes),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Last Inactive: ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    format_age(proj.last_modified),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "--- Artifact Breakdown ---",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        for art in &proj.artifacts {
            let art_age = format_age(art.last_modified);
            lines.push(Line::from(vec![
                Span::styled(format!(" - {}: ", art.name), Style::default().fg(Color::Green)),
                Span::styled(
                    format_bytes(art.size_bytes),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    format!(" ({} files, {})", art.file_count, art_age),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        lines
    } else {
        vec![
            Line::from("No project selected."),
            Line::from(""),
            Line::from("Use [Up/Down] to navigate."),
        ]
    };

    let paragraph = Paragraph::new(content).block(block).wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let status_text = if let Some((msg, _)) = &app.status_message {
        msg.as_str()
    } else {
        "[Space] Select | [A] All | [D] Clean | [S] Sort | [F] Filter | [/] Search | [R] Rescan | [?] Help | [Q] Quit"
    };

    let p = Paragraph::new(status_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Left);

    f.render_widget(p, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn render_confirm_modal(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(60, 45, area);
    f.render_widget(Clear, popup_area);

    let count = if app.selected_count() > 0 {
        app.selected_count()
    } else {
        1
    };

    let bytes = if app.selected_count() > 0 {
        app.selected_bytes()
    } else if let Some(p) = app.current_selected_project() {
        p.total_size_bytes
    } else {
        0
    };

    let method_text = match app.delete_method {
        DeletionMethod::Trash => "Recycle Bin (Safe & Recoverable)",
        DeletionMethod::Permanent => "Permanent Deletion (Non-recoverable)",
    };

    let method_color = match app.delete_method {
        DeletionMethod::Trash => Color::Green,
        DeletionMethod::Permanent => Color::Red,
    };

    let text = vec![
        Line::from(vec![Span::styled(
            "Are you sure you want to clean the selected caches?",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Projects to clean: "),
            Span::styled(
                format!("{}", count),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("Total Space to Reclaim: "),
            Span::styled(
                format_bytes(bytes),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("Deletion Method: "),
            Span::styled(
                method_text,
                Style::default().fg(method_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("[T]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" Recycle Bin (Default)   "),
            Span::styled("[P]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(" Permanent Deletion"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "[Enter] Confirm Execution",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("     "),
            Span::styled("[Esc] Cancel", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let block = Block::default()
        .title(" Confirm Cleanup ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Red));

    let para = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false });

    f.render_widget(para, popup_area);
}

fn render_help_modal(f: &mut Frame, area: Rect) {
    let popup_area = centered_rect(65, 60, area);
    f.render_widget(Clear, popup_area);

    let shortcuts = vec![
        ("Up / Down / j / k", "Navigate project list"),
        ("PgUp / PgDown", "Jump page up/down"),
        ("Space", "Toggle selection of highlighted project"),
        ("a", "Toggle select all / deselect all"),
        ("d / Delete", "Open clean confirmation dialog"),
        ("s", "Cycle sort (Size -> Age -> Name -> Type)"),
        ("f", "Filter by ecosystem"),
        ("/", "Live search query"),
        ("r", "Restart directory scan"),
        ("Esc", "Close modal / Clear search"),
        ("q", "Quit DevSweep"),
    ];

    let rows: Vec<Row> = shortcuts
        .into_iter()
        .map(|(key, desc)| {
            Row::new(vec![
                Cell::from(Span::styled(
                    key,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::raw(desc)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [Constraint::Percentage(35), Constraint::Percentage(65)],
    )
    .block(
        Block::default()
            .title(" Keyboard Shortcuts & Help ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(table, popup_area);
}

fn render_filter_modal(f: &mut Frame, _app: &App, area: Rect) {
    let popup_area = centered_rect(50, 45, area);
    f.render_widget(Clear, popup_area);

    let options = vec![
        ("0", "All Ecosystems (Clear Filter)"),
        ("1", "Rust (Cargo target)"),
        ("2", "Node.js (node_modules, .next, etc.)"),
        ("3", "Python (.venv, __pycache__, etc.)"),
        ("4", "Java / Gradle (build, .gradle, out)"),
        ("5", ".NET / C# (bin, obj)"),
        ("6", "C / C++ / CMake (build, cmake-build-*)"),
        ("7", "Flutter / Dart (.dart_tool, build)"),
    ];

    let rows: Vec<Row> = options
        .into_iter()
        .map(|(num, label)| {
            Row::new(vec![
                Cell::from(Span::styled(
                    format!("[{}]", num),
                    Style::default().fg(Color::Yellow),
                )),
                Cell::from(Span::raw(label)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [Constraint::Length(6), Constraint::Percentage(80)],
    )
    .block(
        Block::default()
            .title(" Filter by Ecosystem (Press 0-7 or Esc) ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Magenta)),
    );

    f.render_widget(table, popup_area);
}
