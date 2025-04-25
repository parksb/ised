use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ised::{highlight_diff_lines, highlight_match};
use regex::Regex;
use std::{error::Error, fs, io};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use walkdir::WalkDir;

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

#[derive(PartialEq, Eq)]
enum Focus {
    FileList,
    FilePathFilter,
    DiffView,
    From,
    To,
}

enum ConfirmState {
    None,
    Confirming(String),
    ConfirmingAll(Vec<String>),
}

fn run_app<B: tui::backend::Backend>(terminal: &mut Terminal<B>) -> io::Result<()> {
    let files: Vec<String> = WalkDir::new(".")
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().display().to_string())
        .collect();

    let mut selected = 0;
    let mut offset = 0;
    let mut filter_input = String::new();
    let mut from_input = String::new();
    let mut to_input = String::new();
    let mut focus = Focus::FileList;
    let mut diff_scroll = 0;
    let mut confirm = ConfirmState::None;

    loop {
        let filter_regex = Regex::new(&filter_input).ok();
        let filtered_files: Vec<String> = files
            .iter()
            .filter(|f| filter_regex.as_ref().map_or(true, |re| re.is_match(f)))
            .cloned()
            .collect();

        terminal.draw(|f| {
            let size = f.size();

            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(size);

            let left_rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(10),
                    Constraint::Length(3),
                    Constraint::Length(3),
                ])
                .split(columns[0]);

            let right_rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(10),
                    Constraint::Length(3),
                    Constraint::Length(3),
                ])
                .split(columns[1]);

            let list_height = left_rows[0].height as usize - 2;
            if selected >= offset + list_height {
                offset = selected + 1 - list_height;
            } else if selected < offset {
                offset = selected;
            }

            let visible_files = filtered_files
                .iter()
                .skip(offset)
                .take(list_height)
                .enumerate()
                .map(|(i, fpath)| {
                    let content = highlight_match(fpath, &filter_regex);
                    let mut item = ListItem::new(content);
                    if i + offset == selected {
                        item = item.style(
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        );
                    }
                    item
                })
                .collect::<Vec<_>>();

            let file_list = List::new(visible_files).block(
                Block::default()
                    .title("File List")
                    .borders(Borders::ALL)
                    .border_style(if focus == Focus::FileList {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default()
                    }),
            );
            f.render_widget(file_list, left_rows[0]);

            let path_input = Paragraph::new(Text::from(filter_input.as_str())).block(
                Block::default()
                    .title("File Path Filter")
                    .borders(Borders::ALL)
                    .border_style(if focus == Focus::FilePathFilter {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default()
                    }),
            );
            f.render_widget(path_input, left_rows[1]);

            let blank_text = match &confirm {
                ConfirmState::Confirming(path) => format!("Apply changes to {}? (y/n)", path),
                ConfirmState::ConfirmingAll(_) => "Apply changes to ALL files? (y/n)".to_string(),
                ConfirmState::None => "ised v0.1.0".to_string(),
            };

            let blank = Paragraph::new(Text::from(blank_text));
            f.render_widget(blank, left_rows[2]);

            let selected_file = filtered_files.get(selected).map(|s| s.to_string());
            let diff_output = if let Some(file_path) = selected_file {
                if let Ok(content) = fs::read_to_string(&file_path) {
                    let from_re = Regex::new(&from_input).unwrap_or(Regex::new("$^").unwrap());
                    let replaced = from_re.replace_all(&content, to_input.as_str()).to_string();
                    highlight_diff_lines(content, replaced)
                } else {
                    vec![Spans::from(Span::styled(
                        "Failed to read file.",
                        Style::default().fg(Color::Red),
                    ))]
                }
            } else {
                vec![Spans::from("No file selected.")]
            };

            let height = right_rows[0].height as usize - 2;
            let visible_diff = diff_output
                .into_iter()
                .skip(diff_scroll)
                .take(height)
                .collect::<Vec<_>>();

            let diff_view = Paragraph::new(visible_diff).block(
                Block::default()
                    .title("Diff View")
                    .borders(Borders::ALL)
                    .border_style(if focus == Focus::DiffView {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default()
                    }),
            );
            f.render_widget(diff_view, right_rows[0]);

            let from_paragraph = Paragraph::new(Text::from(from_input.as_str())).block(
                Block::default()
                    .title("From (Regex)")
                    .borders(Borders::ALL)
                    .border_style(if focus == Focus::From {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default()
                    }),
            );
            f.render_widget(from_paragraph, right_rows[1]);

            let to_paragraph = Paragraph::new(Text::from(to_input.as_str())).block(
                Block::default()
                    .title("To")
                    .borders(Borders::ALL)
                    .border_style(if focus == Focus::To {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default()
                    }),
            );
            f.render_widget(to_paragraph, right_rows[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                match key {
                    event::KeyEvent {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => break,
                    event::KeyEvent {
                        code: KeyCode::Tab, ..
                    } => {
                        focus = match focus {
                            Focus::FileList => Focus::FilePathFilter,
                            Focus::FilePathFilter => Focus::DiffView,
                            Focus::DiffView => Focus::From,
                            Focus::From => Focus::To,
                            Focus::To => Focus::FileList,
                        };
                    }
                    event::KeyEvent {
                        code: KeyCode::Enter,
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        if let Focus::FileList = focus {
                            confirm = ConfirmState::ConfirmingAll(filtered_files.clone());
                        }
                    }
                    event::KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    } => {
                        if let Focus::FileList = focus {
                            if let Some(file_path) = filtered_files.get(selected) {
                                confirm = ConfirmState::Confirming(file_path.to_string());
                            }
                        }
                    }
                    event::KeyEvent {
                        code: KeyCode::Char('y'),
                        ..
                    } => match &confirm {
                        ConfirmState::Confirming(path) => {
                            if let Ok(content) = fs::read_to_string(path) {
                                let from_re =
                                    Regex::new(&from_input).unwrap_or(Regex::new("$^").unwrap());
                                let replaced =
                                    from_re.replace_all(&content, to_input.as_str()).to_string();
                                fs::write(path, replaced)?;
                            }
                            confirm = ConfirmState::None;
                        }
                        ConfirmState::ConfirmingAll(paths) => {
                            let from_re =
                                Regex::new(&from_input).unwrap_or(Regex::new("$^").unwrap());
                            for path in paths {
                                if let Ok(content) = fs::read_to_string(path) {
                                    let replaced = from_re
                                        .replace_all(&content, to_input.as_str())
                                        .to_string();
                                    let _ = fs::write(path, replaced);
                                }
                            }
                            confirm = ConfirmState::None;
                        }
                        ConfirmState::None => match focus {
                            Focus::FilePathFilter => filter_input.push('y'),
                            Focus::From => from_input.push('y'),
                            Focus::To => to_input.push('y'),
                            _ => {}
                        },
                    },
                    event::KeyEvent {
                        code: KeyCode::Char('n'),
                        ..
                    } => {
                        if let ConfirmState::Confirming(_) = &confirm {
                            confirm = ConfirmState::None;
                        } else {
                            match focus {
                                Focus::FilePathFilter => filter_input.push('n'),
                                Focus::From => from_input.push('n'),
                                Focus::To => to_input.push('n'),
                                _ => {}
                            }
                        }
                    }
                    event::KeyEvent {
                        code: KeyCode::Esc, ..
                    } => {
                        if let ConfirmState::Confirming(_) = &confirm {
                            confirm = ConfirmState::None;
                        }
                    }
                    event::KeyEvent {
                        code: KeyCode::Up, ..
                    } => {
                        if focus == Focus::FileList && selected > 0 {
                            selected -= 1;
                        } else if focus == Focus::DiffView && diff_scroll > 0 {
                            diff_scroll -= 1;
                        }
                    }
                    event::KeyEvent {
                        code: KeyCode::Down,
                        ..
                    } => {
                        if focus == Focus::FileList && selected + 1 < files.len() {
                            selected += 1;
                        } else if focus == Focus::DiffView {
                            diff_scroll += 1;
                        }
                    }
                    event::KeyEvent {
                        code: KeyCode::Char('j'),
                        ..
                    } => match focus {
                        Focus::FileList => {
                            if selected + 1 < files.len() {
                                selected += 1;
                            }
                        }
                        Focus::DiffView => {
                            diff_scroll += 1;
                        }
                        Focus::FilePathFilter => filter_input.push('j'),
                        Focus::From => from_input.push('j'),
                        Focus::To => to_input.push('j'),
                    },
                    event::KeyEvent {
                        code: KeyCode::Char('k'),
                        ..
                    } => match focus {
                        Focus::FileList => {
                            selected = selected.saturating_sub(1);
                        }
                        Focus::DiffView => {
                            diff_scroll = diff_scroll.saturating_sub(1);
                        }
                        Focus::FilePathFilter => filter_input.push('k'),
                        Focus::From => from_input.push('k'),
                        Focus::To => to_input.push('k'),
                    },
                    event::KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    } => match focus {
                        Focus::FilePathFilter => {
                            filter_input.pop();
                            selected = 0;
                            offset = 0;
                        }
                        Focus::From => {
                            from_input.pop();
                        }
                        Focus::To => {
                            to_input.pop();
                        }
                        _ => {}
                    },
                    event::KeyEvent {
                        code: KeyCode::Char(c),
                        ..
                    } => match focus {
                        Focus::FilePathFilter => filter_input.push(c),
                        Focus::From => from_input.push(c),
                        Focus::To => to_input.push(c),
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
