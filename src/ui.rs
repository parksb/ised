use ised::{highlight_diff_lines, highlight_match};
use regex::Regex;
use std::fs;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, ConfirmState, Focus};

pub fn draw<B: Backend>(
    f: &mut Frame<B>,
    app: &App,
    filtered_files: &[String],
    filter_re: &Option<Regex>,
) {
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
    let mut offset = app.offset;
    if app.selected >= offset + list_height {
        offset = app.selected + 1 - list_height;
    } else if app.selected < offset {
        offset = app.selected;
    }

    let visible_files = filtered_files
        .iter()
        .skip(offset)
        .take(list_height)
        .enumerate()
        .map(|(i, fpath)| {
            let content = highlight_match(fpath, filter_re);
            let mut item = ListItem::new(content);
            if i + offset == app.selected {
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
            .title("File")
            .borders(Borders::ALL)
            .border_style(if app.focus == Focus::FileList {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );
    f.render_widget(file_list, left_rows[0]);

    let filter_input = Paragraph::new(Text::from(app.filter_input.as_str())).block(
        Block::default()
            .title("Filter")
            .borders(Borders::ALL)
            .border_style(if app.focus == Focus::FilePathFilter {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );
    f.render_widget(filter_input, left_rows[1]);

    let blank_text = match &app.confirm {
        ConfirmState::Confirming(path) => format!("Apply changes to {}? (y/n)", path),
        ConfirmState::ConfirmingAll(_) => "Apply changes to ALL files? (y/n)".to_string(),
        ConfirmState::None => "ised v0.1.0".to_string(),
    };
    let blank = Paragraph::new(Text::from(blank_text));
    f.render_widget(blank, left_rows[2]);

    let selected_file = filtered_files.get(app.selected).map(|s| s.to_string());
    let diff_output = if let Some(file_path) = selected_file {
        if let Ok(content) = fs::read_to_string(&file_path) {
            let from_re = Regex::new(&app.from_input).unwrap_or(Regex::new("$^").unwrap());
            let replaced = from_re
                .replace_all(&content, app.to_input.as_str())
                .to_string();
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
        .skip(app.diff_scroll)
        .take(height)
        .collect::<Vec<_>>();

    let diff_view = Paragraph::new(visible_diff).block(
        Block::default()
            .title("Diff")
            .borders(Borders::ALL)
            .border_style(if app.focus == Focus::DiffView {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );
    f.render_widget(diff_view, right_rows[0]);

    let from_paragraph = Paragraph::new(Text::from(app.from_input.as_str())).block(
        Block::default()
            .title("From")
            .borders(Borders::ALL)
            .border_style(if app.focus == Focus::From {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );
    f.render_widget(from_paragraph, right_rows[1]);

    let to_paragraph = Paragraph::new(Text::from(app.to_input.as_str())).block(
        Block::default()
            .title("To")
            .borders(Borders::ALL)
            .border_style(if app.focus == Focus::To {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );
    f.render_widget(to_paragraph, right_rows[2]);
}
