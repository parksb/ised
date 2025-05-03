use ratatui::{
    layout::{Constraint, Direction, Layout, Position},
    style::{Color, Modifier, Style},
    text::{Line, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, ConfirmState, Focus};
use crate::utils::apply_substitution_partial;
use crate::utils::highlight_diff_lines;
use crate::utils::highlight_match;

pub fn draw(f: &mut Frame, app: &App, filtered_files: &[String], file_content: Option<String>) {
    let size = f.area();
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
            let content = highlight_match(fpath, &app.filter_input);
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
            .title("File [L]ist")
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
            .title("[G]lob Filter")
            .borders(Borders::ALL)
            .border_style(if app.focus == Focus::FilePathFilter {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );
    f.render_widget(filter_input, left_rows[1]);
    if app.focus == Focus::FilePathFilter {
        f.set_cursor_position(Position::new(
            left_rows[1].x + 1 + app.filter_cursor as u16,
            left_rows[1].y + 1,
        ));
    }

    let blank_text = match &app.confirm {
        ConfirmState::Confirming(path) => format!("Apply changes to {}? (y/n)", path),
        ConfirmState::ConfirmingAll(_) => "Apply changes to ALL files? (y/n)".to_string(),
        ConfirmState::None => "".to_string(),
    };
    let blank = Paragraph::new(Text::from(blank_text));
    f.render_widget(blank, left_rows[2]);

    let diff_output = if let Some(content) = file_content {
        let replaced = apply_substitution_partial(&content, &app.from_input, &app.to_input);
        highlight_diff_lines(content, replaced)
    } else {
        vec![Line::from("No file selected.")]
    };

    let height = right_rows[0].height as usize - 2;
    let visible_diff = diff_output
        .into_iter()
        .skip(app.diff_scroll)
        .take(height)
        .collect::<Vec<_>>();

    let diff_view = Paragraph::new(visible_diff).block(
        Block::default()
            .title("[D]iff")
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
            .title("[F]rom")
            .borders(Borders::ALL)
            .border_style(if app.focus == Focus::From {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );
    f.render_widget(from_paragraph, right_rows[1]);
    if app.focus == Focus::From {
        f.set_cursor_position(Position::new(
            right_rows[1].x + 1 + app.from_cursor as u16,
            right_rows[1].y + 1,
        ));
    }

    let to_paragraph = Paragraph::new(Text::from(app.to_input.as_str())).block(
        Block::default()
            .title("[T]o")
            .borders(Borders::ALL)
            .border_style(if app.focus == Focus::To {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );
    f.render_widget(to_paragraph, right_rows[2]);
    if app.focus == Focus::To {
        f.set_cursor_position(Position::new(
            right_rows[2].x + 1 + app.to_cursor as u16,
            right_rows[2].y + 1,
        ));
    }
}
