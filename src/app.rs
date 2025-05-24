use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use notify::{Event as NotifyEvent, RecursiveMode, Result as NotifyResult, Watcher};
use parking_lot::RwLock;
use rayon::prelude::*;
use std::path::Path;
use std::sync::Arc;
use std::{collections::HashMap, fs, io};

use crate::config::find_and_load_config;
use crate::utils::{apply_substitution_partial, is_text_file};

type FilterCache = (String, String, Vec<String>);
type FileCache = HashMap<String, String>;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Focus {
    FileList,
    FilePathFilter,
    DiffView,
    From,
    To,
}

#[derive(Clone)]
pub enum ConfirmState {
    None,
    Confirming(String),
    ConfirmingAll(Vec<String>),
}

pub struct App {
    pub files: Vec<String>,
    pub selected: usize,
    pub offset: usize,
    pub filter_input: String,
    pub filter_cursor: usize,
    pub filter_view_offset: usize,
    pub filter_field_width: usize,
    pub from_input: String,
    pub from_cursor: usize,
    pub from_view_offset: usize,
    pub from_field_width: usize,
    pub to_input: String,
    pub to_cursor: usize,
    pub to_view_offset: usize,
    pub to_field_width: usize,
    pub focus: Focus,
    pub diff_scroll: usize,
    pub confirm: ConfirmState,
    pub is_loading: bool,
    pub spinner: char,
    file_cache: Arc<RwLock<FileCache>>,
    filtered_files_cache: Arc<RwLock<Option<FilterCache>>>,
    #[allow(dead_code)]
    file_watcher: Option<notify::RecommendedWatcher>,
    regex_cache: Arc<RwLock<HashMap<String, regex::Regex>>>,
}

impl Clone for App {
    fn clone(&self) -> Self {
        Self {
            files: self.files.clone(),
            selected: self.selected,
            offset: self.offset,
            filter_input: self.filter_input.clone(),
            filter_cursor: self.filter_cursor,
            filter_view_offset: self.filter_view_offset,
            filter_field_width: self.filter_field_width,
            from_input: self.from_input.clone(),
            from_cursor: self.from_cursor,
            from_view_offset: self.from_view_offset,
            from_field_width: self.from_field_width,
            to_input: self.to_input.clone(),
            to_cursor: self.to_cursor,
            to_view_offset: self.to_view_offset,
            to_field_width: self.to_field_width,
            focus: self.focus,
            diff_scroll: self.diff_scroll,
            confirm: self.confirm.clone(),
            is_loading: self.is_loading,
            spinner: self.spinner,
            file_cache: self.file_cache.clone(),
            filtered_files_cache: self.filtered_files_cache.clone(),
            file_watcher: None,
            regex_cache: self.regex_cache.clone(),
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    fn update_view_offset_for_cursor(
        cursor: usize,
        view_offset: &mut usize,
        text_len: usize,
        field_width: usize,
    ) {
        if text_len <= field_width {
            *view_offset = 0;
            return;
        }

        if cursor < *view_offset {
            *view_offset = cursor;
        } else if cursor >= *view_offset + field_width {
            *view_offset = cursor + 1 - field_width.min(cursor + 1);
        }
    }

    fn scroll_view_left(view_offset: &mut usize, text_len: usize, field_width: usize) {
        if text_len <= field_width {
            return;
        }
        if *view_offset > 0 {
            *view_offset -= 1;
        }
    }

    fn scroll_view_right(view_offset: &mut usize, text_len: usize, field_width: usize) {
        if text_len <= field_width {
            return;
        }
        if *view_offset + field_width < text_len {
            *view_offset += 1;
        }
    }

    pub fn update_field_widths(&mut self, filter_width: usize, from_width: usize, to_width: usize) {
        self.filter_field_width = filter_width;
        self.from_field_width = from_width;
        self.to_field_width = to_width;
    }

    pub fn new() -> Self {
        let config = find_and_load_config();

        let filter_input = config
            .as_ref()
            .and_then(|c| c.files.as_ref())
            .and_then(|f| f.glob_filter.as_ref())
            .map(|patterns| patterns.join(","))
            .unwrap_or_default();

        let file_cache = Arc::new(RwLock::new(HashMap::new()));
        let filtered_files_cache = Arc::new(RwLock::new(None));

        let file_cache_clone = file_cache.clone();
        let filtered_files_cache_clone = filtered_files_cache.clone();

        let mut watcher = notify::recommended_watcher(move |res: NotifyResult<NotifyEvent>| {
            if let Ok(event) = res {
                match event.kind {
                    notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                        if let Some(path) = event.paths.first() {
                            if let Some(path_str) = path.to_str() {
                                let mut cache = file_cache_clone.write();
                                cache.remove(path_str);
                                let mut filtered_cache = filtered_files_cache_clone.write();
                                *filtered_cache = None;
                            }
                        }
                    }
                    _ => {}
                }
            }
        })
        .ok();

        if let Some(w) = &mut watcher {
            let _ = w.watch(Path::new("."), RecursiveMode::Recursive);
        }

        let spinner = '|';

        Self {
            files: Vec::new(),
            selected: 0,
            offset: 0,
            filter_input,
            filter_cursor: 0,
            filter_view_offset: 0,
            filter_field_width: 40,
            from_input: String::new(),
            from_cursor: 0,
            from_view_offset: 0,
            from_field_width: 40,
            to_input: String::new(),
            to_cursor: 0,
            to_view_offset: 0,
            to_field_width: 40,
            focus: Focus::FileList,
            diff_scroll: 0,
            confirm: ConfirmState::None,
            is_loading: true,
            spinner,
            file_cache,
            filtered_files_cache,
            file_watcher: watcher,
            regex_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn load_files(&mut self) {
        self.files = walkdir::WalkDir::new(".")
            .into_iter()
            .par_bridge()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| is_text_file(e.path()))
            .map(|e| e.path().display().to_string())
            .collect();
        self.is_loading = false;
        {
            let mut cache = self.filtered_files_cache.write();
            *cache = None;
        }
    }

    pub fn filter_files(&self) -> Vec<String> {
        use globset::{Glob, GlobSetBuilder};

        if self.filter_input.trim().is_empty() && self.from_input.trim().is_empty() {
            return self.files.clone();
        }

        {
            let cache = self.filtered_files_cache.read();
            if let Some((cached_filter, cached_from, cached_files)) = &*cache {
                if *cached_filter == self.filter_input && *cached_from == self.from_input {
                    return cached_files.clone();
                }
            }
        }

        let patterns: Vec<_> = self
            .filter_input
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect();

        let mut include_builder = GlobSetBuilder::new();
        let mut exclude_builder = GlobSetBuilder::new();
        let mut has_include = false;

        for pat in &patterns {
            if let Some(stripped) = pat.strip_prefix('!') {
                if let Ok(glob) = Glob::new(stripped) {
                    exclude_builder.add(glob);
                }
            } else {
                has_include = true;
                if let Ok(glob) = Glob::new(pat) {
                    include_builder.add(glob);
                }
            }
        }

        let include_set = include_builder.build().ok();
        let exclude_set = exclude_builder.build().ok();

        let from_re = if !self.from_input.is_empty() {
            {
                let cache = self.regex_cache.read();
                cache.get(&self.from_input).cloned()
            }
        } else {
            None
        };

        let from_re = from_re.or_else(|| {
            regex::Regex::new(&self.from_input).ok().inspect(|re| {
                self.regex_cache
                    .write()
                    .insert(self.from_input.clone(), re.clone());
            })
        });

        let filtered_files: Vec<String> = self
            .files
            .par_iter()
            .filter(|f| {
                let included = if has_include {
                    include_set
                        .as_ref()
                        .map(|set| set.is_match(f))
                        .unwrap_or(false)
                } else {
                    true
                };

                let excluded = exclude_set
                    .as_ref()
                    .map(|set| set.is_match(f))
                    .unwrap_or(false);

                let matches_from = if let Some(re) = &from_re {
                    let content = {
                        let cache = self.file_cache.read();
                        cache.get(*f).cloned()
                    };

                    if let Some(content) = content {
                        re.is_match(&content)
                    } else {
                        std::fs::read_to_string(f)
                            .map(|content| {
                                let mut cache = self.file_cache.write();
                                cache.insert(f.to_string(), content.clone());
                                re.is_match(&content)
                            })
                            .unwrap_or(false)
                    }
                } else {
                    true
                };

                included && !excluded && matches_from
            })
            .cloned()
            .collect();

        {
            let mut cache = self.filtered_files_cache.write();
            *cache = Some((
                self.filter_input.clone(),
                self.from_input.clone(),
                filtered_files.clone(),
            ));
        }

        filtered_files
    }

    pub fn handle_key_event(
        &mut self,
        key: KeyEvent,
        filtered_files: &[String],
    ) -> io::Result<bool> {
        match key {
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => return Ok(true),

            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.focus = Focus::FileList;
            }

            KeyEvent {
                code: KeyCode::Char('g'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.focus = Focus::FilePathFilter;
            }

            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.focus = Focus::DiffView;
            }

            KeyEvent {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.focus = Focus::From;
            }

            KeyEvent {
                code: KeyCode::Char('t'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.focus = Focus::To;
            }

            KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                self.focus = match self.focus {
                    Focus::FileList => Focus::FilePathFilter,
                    Focus::FilePathFilter => Focus::DiffView,
                    Focus::DiffView => Focus::From,
                    Focus::From => Focus::To,
                    Focus::To => Focus::FileList,
                };
            }

            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                if self.focus == Focus::FileList {
                    self.confirm = ConfirmState::ConfirmingAll(filtered_files.to_vec());
                }
            }

            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                if self.focus == Focus::FileList {
                    if let Some(file) = filtered_files.get(self.selected) {
                        self.confirm = ConfirmState::Confirming(file.clone());
                    }
                }
            }

            KeyEvent {
                code: KeyCode::Char('y'),
                ..
            } => match &self.confirm {
                ConfirmState::Confirming(path) => {
                    self.apply_substitution(path)?;
                    self.confirm = ConfirmState::None;
                }
                ConfirmState::ConfirmingAll(paths) => {
                    for path in paths {
                        let _ = self.apply_substitution(path);
                    }
                    self.confirm = ConfirmState::None;
                }
                ConfirmState::None => self.push_input('y'),
            },

            KeyEvent {
                code: KeyCode::Char('n'),
                ..
            } => {
                if !matches!(self.confirm, ConfirmState::None) {
                    self.confirm = ConfirmState::None;
                } else {
                    self.push_input('n');
                }
            }

            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.confirm = ConfirmState::None;
            }

            KeyEvent {
                code: KeyCode::Up, ..
            } => match self.focus {
                Focus::FileList => {
                    if self.selected > 0 {
                        self.selected -= 1;
                    }
                }
                Focus::DiffView => {
                    self.diff_scroll = self.diff_scroll.saturating_sub(1);
                }
                _ => {}
            },

            KeyEvent {
                code: KeyCode::Down,
                ..
            } => match self.focus {
                Focus::FileList => {
                    if self.selected + 1 < filtered_files.len() {
                        self.selected += 1;
                    }
                }
                Focus::DiffView => {
                    self.diff_scroll += 1;
                }
                _ => {}
            },

            KeyEvent {
                code: KeyCode::Left,
                ..
            } => match self.focus {
                Focus::FilePathFilter => {
                    if self.filter_cursor > 0 {
                        self.filter_cursor -= 1;
                        Self::update_view_offset_for_cursor(
                            self.filter_cursor,
                            &mut self.filter_view_offset,
                            self.filter_input.chars().count(),
                            self.filter_field_width,
                        );
                    } else {
                        Self::scroll_view_left(
                            &mut self.filter_view_offset,
                            self.filter_input.chars().count(),
                            self.filter_field_width,
                        );
                    }
                }
                Focus::From => {
                    if self.from_cursor > 0 {
                        self.from_cursor -= 1;
                        Self::update_view_offset_for_cursor(
                            self.from_cursor,
                            &mut self.from_view_offset,
                            self.from_input.chars().count(),
                            self.from_field_width,
                        );
                    } else {
                        Self::scroll_view_left(
                            &mut self.from_view_offset,
                            self.from_input.chars().count(),
                            self.from_field_width,
                        );
                    }
                }
                Focus::To => {
                    if self.to_cursor > 0 {
                        self.to_cursor -= 1;
                        Self::update_view_offset_for_cursor(
                            self.to_cursor,
                            &mut self.to_view_offset,
                            self.to_input.chars().count(),
                            self.to_field_width,
                        );
                    } else {
                        Self::scroll_view_left(
                            &mut self.to_view_offset,
                            self.to_input.chars().count(),
                            self.to_field_width,
                        );
                    }
                }
                _ => {}
            },

            KeyEvent {
                code: KeyCode::Right,
                ..
            } => match self.focus {
                Focus::FilePathFilter => {
                    if self.filter_cursor < self.filter_input.chars().count() {
                        self.filter_cursor += 1;
                        Self::update_view_offset_for_cursor(
                            self.filter_cursor,
                            &mut self.filter_view_offset,
                            self.filter_input.chars().count(),
                            self.filter_field_width,
                        );
                    } else {
                        Self::scroll_view_right(
                            &mut self.filter_view_offset,
                            self.filter_input.len(),
                            self.filter_field_width,
                        );
                    }
                }
                Focus::From => {
                    if self.from_cursor < self.from_input.chars().count() {
                        self.from_cursor += 1;
                        Self::update_view_offset_for_cursor(
                            self.from_cursor,
                            &mut self.from_view_offset,
                            self.from_input.chars().count(),
                            self.from_field_width,
                        );
                    } else {
                        Self::scroll_view_right(
                            &mut self.from_view_offset,
                            self.from_input.len(),
                            self.from_field_width,
                        );
                    }
                }
                Focus::To => {
                    if self.to_cursor < self.to_input.chars().count() {
                        self.to_cursor += 1;
                        Self::update_view_offset_for_cursor(
                            self.to_cursor,
                            &mut self.to_view_offset,
                            self.to_input.chars().count(),
                            self.to_field_width,
                        );
                    } else {
                        Self::scroll_view_right(
                            &mut self.to_view_offset,
                            self.to_input.len(),
                            self.to_field_width,
                        );
                    }
                }
                _ => {}
            },

            KeyEvent {
                code: KeyCode::Char(c),
                ..
            } => match c {
                'j' => match self.focus {
                    Focus::FileList => {
                        if self.selected + 1 < filtered_files.len() {
                            self.selected += 1;
                        }
                    }
                    Focus::DiffView => self.diff_scroll += 1,
                    _ => self.push_input('j'),
                },
                'k' => match self.focus {
                    Focus::FileList => self.selected = self.selected.saturating_sub(1),
                    Focus::DiffView => self.diff_scroll = self.diff_scroll.saturating_sub(1),
                    _ => self.push_input('k'),
                },
                _ => self.push_input(c),
            },

            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => match self.focus {
                Focus::FilePathFilter => {
                    if self.filter_cursor > 0 {
                        let char_indices: Vec<(usize, char)> =
                            self.filter_input.char_indices().collect();
                        if let Some(&(byte_pos, _)) = char_indices.get(self.filter_cursor - 1) {
                            self.filter_input.remove(byte_pos);
                        }
                        self.filter_cursor -= 1;
                        Self::update_view_offset_for_cursor(
                            self.filter_cursor,
                            &mut self.filter_view_offset,
                            self.filter_input.chars().count(),
                            self.filter_field_width,
                        );
                    }
                    self.selected = 0;
                    self.offset = 0;
                }
                Focus::From => {
                    if self.from_cursor > 0 {
                        let char_indices: Vec<(usize, char)> =
                            self.from_input.char_indices().collect();
                        if let Some(&(byte_pos, _)) = char_indices.get(self.from_cursor - 1) {
                            self.from_input.remove(byte_pos);
                        }
                        self.from_cursor -= 1;
                        Self::update_view_offset_for_cursor(
                            self.from_cursor,
                            &mut self.from_view_offset,
                            self.from_input.chars().count(),
                            self.from_field_width,
                        );
                    }
                }
                Focus::To => {
                    if self.to_cursor > 0 {
                        let char_indices: Vec<(usize, char)> =
                            self.to_input.char_indices().collect();
                        if let Some(&(byte_pos, _)) = char_indices.get(self.to_cursor - 1) {
                            self.to_input.remove(byte_pos);
                        }
                        self.to_cursor -= 1;
                        Self::update_view_offset_for_cursor(
                            self.to_cursor,
                            &mut self.to_view_offset,
                            self.to_input.chars().count(),
                            self.to_field_width,
                        );
                    }
                }
                _ => {}
            },

            _ => {}
        }
        Ok(false)
    }

    fn push_input(&mut self, c: char) {
        match self.focus {
            Focus::FilePathFilter => {
                let char_indices: Vec<(usize, char)> = self.filter_input.char_indices().collect();
                let byte_pos = if self.filter_cursor >= char_indices.len() {
                    self.filter_input.len()
                } else {
                    char_indices[self.filter_cursor].0
                };
                self.filter_input.insert(byte_pos, c);
                self.filter_cursor += 1;
                Self::update_view_offset_for_cursor(
                    self.filter_cursor,
                    &mut self.filter_view_offset,
                    self.filter_input.chars().count(),
                    self.filter_field_width,
                );
                self.selected = 0;
                self.offset = 0;
            }
            Focus::From => {
                let char_indices: Vec<(usize, char)> = self.from_input.char_indices().collect();
                let byte_pos = if self.from_cursor >= char_indices.len() {
                    self.from_input.len()
                } else {
                    char_indices[self.from_cursor].0
                };
                self.from_input.insert(byte_pos, c);
                self.from_cursor += 1;
                Self::update_view_offset_for_cursor(
                    self.from_cursor,
                    &mut self.from_view_offset,
                    self.from_input.chars().count(),
                    self.from_field_width,
                );
            }
            Focus::To => {
                let char_indices: Vec<(usize, char)> = self.to_input.char_indices().collect();
                let byte_pos = if self.to_cursor >= char_indices.len() {
                    self.to_input.len()
                } else {
                    char_indices[self.to_cursor].0
                };
                self.to_input.insert(byte_pos, c);
                self.to_cursor += 1;
                Self::update_view_offset_for_cursor(
                    self.to_cursor,
                    &mut self.to_view_offset,
                    self.to_input.chars().count(),
                    self.to_field_width,
                );
            }
            _ => {}
        }
    }

    fn apply_substitution(&self, path: &str) -> io::Result<()> {
        let content = fs::read_to_string(path)?;
        let replaced = apply_substitution_partial(&content, &self.from_input, &self.to_input);
        fs::write(path, replaced)?;

        {
            let mut cache = self.file_cache.write();
            cache.remove(path);
        }

        {
            let mut cache = self.filtered_files_cache.write();
            *cache = None;
        }

        Ok(())
    }

    pub fn spin(&mut self) {
        self.spinner = match self.spinner {
            '|' => '/',
            '/' => '-',
            '-' => '\\',
            '\\' => '|',
            _ => '|',
        };
    }
}
