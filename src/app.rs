use std::io::{self, stdout};
use std::path::PathBuf;
use std::collections::HashMap;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::actions::{open_in_notepad, ClipboardItem};
use crate::explorer_fs::{available_volumes, list_entries, EntryItem};

mod file_actions;
mod navigation;
mod render;

pub fn run() -> io::Result<()> {
    terminal::enable_raw_mode()?;

    let mut out = stdout();
    execute!(out, EnterAlternateScreen, cursor::Hide)?;

    let result = run_app();

    execute!(out, LeaveAlternateScreen, cursor::Show)?;
    terminal::disable_raw_mode()?;

    result
}

fn run_app() -> io::Result<()> {
    let mut app = App::new()?;
    app.render()?;

    loop {
        match event::read()? {
            Event::Key(key) if key.kind == crossterm::event::KeyEventKind::Press => {
                if !app.handle_key(key)? {
                    break;
                }
                app.render()?;
            }
            Event::Resize(_, _) => {
                app.render()?;
            }
            _ => {}
        }
    }

    Ok(())
}

struct App {
    current_dir: PathBuf,
    parent_dir: Option<PathBuf>,
    parent_entries: Vec<EntryItem>,
    entries: Vec<EntryItem>,
    volumes: Vec<PathBuf>,
    selected: usize,
    status: String,
    preview: String,
    preview_scroll_offset: usize,
    clipboard: Option<ClipboardItem>,
    modal: Option<Modal>,
    scroll_offset: usize,
    navigation_history: HashMap<PathBuf, NavigationState>,
    shift_filter_query: String,
}

#[derive(Clone, Copy)]
struct NavigationState {
    selected: usize,
    scroll_offset: usize,
}

#[derive(Clone)]
enum Modal {
    Help,
    ConfirmDelete { path: PathBuf, label: String },
}

impl App {
    fn new() -> io::Result<Self> {
        let current_dir = std::env::current_dir()?;
        let volumes = available_volumes();
        let parent_dir = current_dir.parent().map(|path| path.to_path_buf());
        let parent_entries = match &parent_dir {
            Some(path) => list_entries(path)?,
            None => Vec::new(),
        };
        let entries = list_entries(&current_dir)?;

        let mut app = Self {
            current_dir,
            parent_dir,
            parent_entries,
            entries,
            volumes,
            selected: 0,
            scroll_offset: 0,
            status: String::from("Arrows move | 1-9 (layout-aware) switch volumes | Enter opens files in Notepad | H help | Q quit"),
            preview: String::from("Select a folder or file."),
            preview_scroll_offset: 0,
            clipboard: None,
            modal: None,
            navigation_history: HashMap::new(),
            shift_filter_query: String::new(),
        };

        app.update_preview();
        Ok(app)
    }

    fn handle_key(&mut self, key: KeyEvent) -> io::Result<bool> {
        if let Some(modal) = self.modal.clone() {
            return self.handle_modal_key(key, modal);
        }

        let shift_pressed = key.modifiers.contains(KeyModifiers::SHIFT);
        if !shift_pressed {
            self.shift_filter_query.clear();
        }

        if shift_pressed {
            match key.code {
                KeyCode::Up => {
                    self.scroll_preview_by(-1);
                    return Ok(true);
                }
                KeyCode::Down => {
                    self.scroll_preview_by(1);
                    return Ok(true);
                }
                KeyCode::Backspace => {
                    self.shift_filter_query.pop();
                    self.jump_to_shift_filter_match();
                    return Ok(true);
                }
                KeyCode::Esc => {
                    self.shift_filter_query.clear();
                    self.status = String::from("Shift filter cleared.");
                    return Ok(true);
                }
                KeyCode::Char(ch) if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') => {
                    self.shift_filter_query.push(ch.to_ascii_lowercase());
                    self.jump_to_shift_filter_match();
                    return Ok(true);
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char(ch) => match ch.to_ascii_lowercase() {
                'q' => return Ok(false),
                'h' => {
                    self.modal = Some(Modal::Help);
                }
                'r' => {
                    self.status = String::from("Refreshing folder contents.");
                    self.reload()?;
                }
                'c' => self.copy_selected(),
                'x' => self.cut_selected(),
                'v' => self.paste_selected()?,
                'd' => self.request_delete(),
                
                _ => {
                    if let Some(index) = volume_shortcut_index(ch) {
                        self.switch_root(index)?;
                    }
                }
            },
            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                    let rows = self.current_list_rows().unwrap_or(1).max(1);
                    self.ensure_selected_visible(rows);
                    self.update_preview();
                }
            }
            KeyCode::Down => {
                if self.selected + 1 < self.entries.len() {
                    self.selected += 1;
                    let rows = self.current_list_rows().unwrap_or(1).max(1);
                    self.ensure_selected_visible(rows);
                    self.update_preview();
                }
            }
            KeyCode::PageUp => {
                let rows = self.current_list_rows().unwrap_or(1).max(1);
                let step = rows.saturating_sub(2).max(1);
                self.scroll_offset = self.scroll_offset.saturating_sub(step);
                self.selected = self.selected.saturating_sub(step);
                self.ensure_selected_visible(rows);
                self.update_preview();
            }
            KeyCode::PageDown => {
                let rows = self.current_list_rows().unwrap_or(1).max(1);
                let step = rows.saturating_sub(2).max(1);
                self.selected = (self.selected + step).min(self.entries.len().saturating_sub(1));
                self.ensure_selected_visible(rows);
                self.update_preview();
            }
            KeyCode::Home => {
                self.selected = 0;
                self.scroll_offset = 0;
                self.update_preview();
            }
            KeyCode::End => {
                if !self.entries.is_empty() {
                    let rows = self.current_list_rows().unwrap_or(1).max(1);
                    let visible_rows = rows.saturating_sub(2).max(1);
                    self.selected = self.entries.len() - 1;
                    self.scroll_offset = self.entries.len().saturating_sub(visible_rows);
                    self.scroll_offset = self.scroll_offset.min(self.max_scroll_start(rows));
                    self.update_preview();
                }
            }
            KeyCode::Right => {
                if let Some(item) = self.selected_item().cloned() {
                    if item.is_dir {
                        self.enter_directory(item.path)?;
                    }
                }
            }
            KeyCode::Enter => {
                if let Some(item) = self.selected_item().cloned() {
                    if item.is_dir {
                        self.enter_directory(item.path)?;
                    } else if item.is_file {
                        open_in_notepad(&item.path)?;
                        self.status = format!("Opened {} in Notepad.", item.name);
                    }
                }
            }
            KeyCode::Backspace | KeyCode::Left => {
                self.go_parent()?;
            }
            _ => {}
        }

        Ok(true)
    }

    fn handle_modal_key(&mut self, key: KeyEvent, modal: Modal) -> io::Result<bool> {
        match modal {
            Modal::Help => match key.code {
                KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Enter => {
                    self.modal = None;
                }
                _ => {}
            },
            Modal::ConfirmDelete { path, label } => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.delete_confirmed(path, label)?;
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.modal = None;
                    self.status = String::from("Delete cancelled.");
                }
                _ => {}
            },
        }

        Ok(true)
    }
}

fn volume_shortcut_index(ch: char) -> Option<usize> {
    match ch {
        '1' | '+' | '!' => Some(0),
        '2' | 'ě' | 'Ě' | '@' => Some(1),
        '3' | 'š' | 'Š' | '#' => Some(2),
        '4' | 'č' | 'Č' | '$' => Some(3),
        '5' | 'ř' | 'Ř' | '%' => Some(4),
        '6' | 'ž' | 'Ž' | '^' => Some(5),
        '7' | 'ý' | 'Ý' | '&' => Some(6),
        '8' | 'á' | 'Á' | '*' => Some(7),
        '9' | 'í' | 'Í' | '(' => Some(8),
        _ => None,
    }
}
