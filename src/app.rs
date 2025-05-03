use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use notify::{Event as NotifyEvent, RecursiveMode, Result as NotifyResult, Watcher};
use parking_lot::RwLock;
use ratatui::backend::Backend;
use ratatui::Terminal;
use rayon::prelude::*;
use std::path::Path;
use std::sync::Arc;
use std::{collections::HashMap, fs, io};
use tokio::{fs as tokio_fs, time};

use crate::config::find_and_load_config;
use crate::ui;
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
    pub from_input: String,
    pub from_cursor: usize,
    pub to_input: String,
    pub to_cursor: usize,
    pub focus: Focus,
    pub diff_scroll: usize,
    pub confirm: ConfirmState,
    file_cache: Arc<RwLock<FileCache>>,
    filtered_files_cache: Arc<RwLock<Option<FilterCache>>>,
    #[allow(dead_code)]
    file_watcher: Option<notify::RecommendedWatcher>,
    regex_cache: Arc<RwLock<HashMap<String, regex::Regex>>>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let files: Vec<String> = walkdir::WalkDir::new(".")
            .into_iter()
            .par_bridge()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| is_text_file(e.path()))
            .map(|e| e.path().display().to_string())
            .collect();

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
                        // Clear caches when files change
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

        Self {
            files,
            selected: 0,
            offset: 0,
            filter_input,
            filter_cursor: 0,
            from_input: String::new(),
            from_cursor: 0,
            to_input: String::new(),
            to_cursor: 0,
            focus: Focus::FileList,
            diff_scroll: 0,
            confirm: ConfirmState::None,
            file_cache,
            filtered_files_cache,
            file_watcher: watcher,
            regex_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        loop {
            let filtered_files = self.filter_files();

            if self.selected >= filtered_files.len() {
                self.selected = 0;
                self.offset = 0;
            }

            let file_content = if let Some(file) = filtered_files.get(self.selected) {
                self.read_file_content(file).await.ok()
            } else {
                None
            };

            terminal.draw(|f| ui::draw(f, self, &filtered_files, file_content))?;

            if event::poll(time::Duration::from_millis(200))? {
                if let Event::Key(key) = event::read()? {
                    if self.handle_key_event(key, &filtered_files)? {
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    fn filter_files(&self) -> Vec<String> {
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

    async fn read_file_content(&self, path: &str) -> io::Result<String> {
        {
            let cache = self.file_cache.read();
            if let Some(content) = cache.get(path) {
                return Ok(content.clone());
            }
        }

        let content = tokio_fs::read_to_string(path).await?;
        {
            let mut cache = self.file_cache.write();
            cache.insert(path.to_string(), content.clone());
        }
        Ok(content)
    }

    fn handle_key_event(&mut self, key: KeyEvent, filtered_files: &[String]) -> io::Result<bool> {
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
                    }
                }
                Focus::From => {
                    if self.from_cursor > 0 {
                        self.from_cursor -= 1;
                    }
                }
                Focus::To => {
                    if self.to_cursor > 0 {
                        self.to_cursor -= 1;
                    }
                }
                _ => {}
            },

            KeyEvent {
                code: KeyCode::Right,
                ..
            } => match self.focus {
                Focus::FilePathFilter => {
                    if self.filter_cursor < self.filter_input.len() {
                        self.filter_cursor += 1;
                    }
                }
                Focus::From => {
                    if self.from_cursor < self.from_input.len() {
                        self.from_cursor += 1;
                    }
                }
                Focus::To => {
                    if self.to_cursor < self.to_input.len() {
                        self.to_cursor += 1;
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
                        self.filter_input.remove(self.filter_cursor - 1);
                        self.filter_cursor -= 1;
                    }
                    self.selected = 0;
                    self.offset = 0;
                }
                Focus::From => {
                    if self.from_cursor > 0 {
                        self.from_input.remove(self.from_cursor - 1);
                        self.from_cursor -= 1;
                    }
                }
                Focus::To => {
                    if self.to_cursor > 0 {
                        self.to_input.remove(self.to_cursor - 1);
                        self.to_cursor -= 1;
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
                self.filter_input.insert(self.filter_cursor, c);
                self.filter_cursor += 1;
            }
            Focus::From => {
                self.from_input.insert(self.from_cursor, c);
                self.from_cursor += 1;
            }
            Focus::To => {
                self.to_input.insert(self.to_cursor, c);
                self.to_cursor += 1;
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
}
