use std::io::{self, stdout, Write};
use std::path::PathBuf;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::actions::{delete_path, open_in_notepad, paste_clipboard, ClipboardItem, ClipboardMode};
use crate::explorer_fs::{list_entries, read_preview, EntryItem};
use crate::help::help_lines;

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
    selected: usize,
    status: String,
    preview: String,
    clipboard: Option<ClipboardItem>,
    modal: Option<Modal>,
}

#[derive(Clone)]
enum Modal {
    Help,
    ConfirmDelete { path: PathBuf, label: String },
}

impl App {
    fn new() -> io::Result<Self> {
        let current_dir = std::env::current_dir()?;
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
            selected: 0,
            status: String::from("Arrow keys move | Enter opens files in Notepad | H help | Q quit"),
            preview: String::from("Select a folder or file."),
            clipboard: None,
            modal: None,
        };

        app.update_preview();
        Ok(app)
    }

    fn reload(&mut self) -> io::Result<()> {
        self.parent_dir = self.current_dir.parent().map(|path| path.to_path_buf());
        self.parent_entries = match &self.parent_dir {
            Some(path) => list_entries(path)?,
            None => Vec::new(),
        };
        self.entries = list_entries(&self.current_dir)?;

        if self.selected >= self.entries.len() {
            self.selected = self.entries.len().saturating_sub(1);
        }

        self.update_preview();
        Ok(())
    }

    fn selected_item(&self) -> Option<&EntryItem> {
        self.entries.get(self.selected)
    }

    fn enter_directory(&mut self, path: PathBuf) -> io::Result<()> {
        self.current_dir = path;
        self.selected = 0;
        self.reload()?;
        self.status = String::from("Folder opened.");
        Ok(())
    }

    fn go_parent(&mut self) -> io::Result<()> {
        if let Some(parent) = self.parent_dir.clone() {
            self.current_dir = parent;
            self.selected = 0;
            self.reload()?;
            self.status = String::from("Moved to parent folder.");
        } else {
            self.status = String::from("You are already at a root folder.");
        }

        Ok(())
    }

    fn switch_root(&mut self, key: char) -> io::Result<()> {
        let target = match key {
            '1' => Some('C'),
            '2' => Some('D'),
            '3' => Some('E'),
            '4' => Some('F'),
            _ => None,
        };

        if let Some(letter) = target {
            let root = crate::explorer_fs::drive_root(letter);
            if root.exists() {
                self.current_dir = root;
                self.selected = 0;
                self.reload()?;
                self.status = format!("Switched to {} drive.", letter);
            } else {
                self.status = format!("Drive {}: not found.", letter);
            }
        }

        Ok(())
    }

    fn copy_selected(&mut self) {
        if let Some(item) = self.selected_item().cloned() {
            self.clipboard = Some(ClipboardItem {
                source: item.path.clone(),
                mode: ClipboardMode::Copy,
            });
            self.status = format!("Copied {}.", item.name);
        }
    }

    fn cut_selected(&mut self) {
        if let Some(item) = self.selected_item().cloned() {
            self.clipboard = Some(ClipboardItem {
                source: item.path.clone(),
                mode: ClipboardMode::Cut,
            });
            self.status = format!("Cut {}.", item.name);
        }
    }

    fn paste_selected(&mut self) -> io::Result<()> {
        match self.clipboard.clone() {
            Some(item) => {
                let target = paste_clipboard(&item, &self.current_dir)?;
                if matches!(item.mode, ClipboardMode::Cut) {
                    self.clipboard = None;
                }
                self.reload()?;
                self.status = format!("Pasted to {}.", target.display());
            }
            None => {
                self.status = String::from("Clipboard is empty.");
            }
        }

        Ok(())
    }

    fn request_delete(&mut self) {
        if let Some(item) = self.selected_item() {
            self.modal = Some(Modal::ConfirmDelete {
                path: item.path.clone(),
                label: item.name.clone(),
            });
        }
    }

    fn delete_confirmed(&mut self, path: PathBuf, label: String) -> io::Result<()> {
        delete_path(&path)?;
        self.modal = None;
        self.reload()?;
        self.status = format!("Deleted {}.", label);
        Ok(())
    }

    fn update_preview(&mut self) {
        if let Some(item) = self.entries.get(self.selected) {
            if item.is_dir {
                self.preview = format!(
                    "Folder: {}\nPath: {}\n\nEnter opens the folder. Right arrow does the same.",
                    item.name,
                    item.path.display()
                );
            } else if item.is_file {
                match read_preview(&item.path, 8000) {
                    Ok(contents) => {
                        self.preview = format!(
                            "File: {}\nPath: {}\n\n{}",
                            item.name,
                            item.path.display(),
                            contents
                        );
                    }
                    Err(error) => {
                        self.preview = format!("File: {}\n\nUnable to load preview: {error}", item.name);
                    }
                }
            } else {
                self.preview = format!("Item: {}\n\nUnknown item type.", item.name);
            }
        } else {
            self.preview = String::from("This folder is empty.");
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> io::Result<bool> {
        if let Some(modal) = self.modal.clone() {
            return self.handle_modal_key(key, modal);
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(false),
            KeyCode::Char('h') | KeyCode::Char('H') => {
                self.modal = Some(Modal::Help);
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.status = String::from("Refreshing folder contents.");
                self.reload()?;
            }
            KeyCode::Char('c') | KeyCode::Char('C') => self.copy_selected(),
            KeyCode::Char('x') | KeyCode::Char('X') => self.cut_selected(),
            KeyCode::Char('v') | KeyCode::Char('V') => {
                self.paste_selected()?;
            }
            KeyCode::Char('d') | KeyCode::Char('D') => self.request_delete(),
            KeyCode::Char('1') | KeyCode::Char('2') | KeyCode::Char('3') | KeyCode::Char('4') => {
                if let KeyCode::Char(key_char) = key.code {
                    self.switch_root(key_char)?;
                }
            }
            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                    self.update_preview();
                }
            }
            KeyCode::Down => {
                if self.selected + 1 < self.entries.len() {
                    self.selected += 1;
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

    fn render(&self) -> io::Result<()> {
        let mut out = stdout();
        execute!(out, Clear(ClearType::All), cursor::MoveTo(0, 0))?;

        if matches!(self.modal, Some(Modal::Help)) {
            return self.render_help_overlay(&mut out);
        }

        let (width, height) = terminal::size().unwrap_or((80, 24));
        let (left_width, middle_width, right_width) = three_column_widths(width);
        let content_height = height.saturating_sub(6) as usize;
        let preview_width = right_width as usize;

        execute!(
            out,
            SetForegroundColor(Color::Cyan),
            Print(format!("File Explorer - {}\n", self.current_dir.display())),
            ResetColor,
            Print("Controls: arrows move | Right enters folders | Enter opens files in Notepad | H help | Q quit\n\n"),
        )?;

        execute!(out, SetForegroundColor(Color::Yellow), Print(self.panel_title("Parent", self.parent_dir.as_ref())), ResetColor)?;
        execute!(out, Print(" | "))?;
        execute!(out, SetForegroundColor(Color::Yellow), Print(self.panel_title("Current", Some(&self.current_dir))), ResetColor)?;
        execute!(out, Print(" | "))?;
        execute!(out, SetForegroundColor(Color::Yellow), Print(self.panel_title("Preview", None)), ResetColor, Print("\n"))?;

        let left_lines = self.render_directory_lines(&self.parent_entries, left_width as usize, Some(&self.current_dir), None);
        let middle_lines = self.render_directory_lines(&self.entries, middle_width as usize, None, Some(self.selected));
        let right_lines = wrap_text(&self.preview, preview_width.max(20));

        for row in 0..content_height {
            let left = left_lines.get(row).cloned().unwrap_or_default();
            let middle = middle_lines.get(row).cloned().unwrap_or_default();
            let right = right_lines.get(row).cloned().unwrap_or_default();

            execute!(
                out,
                Print(fit_text(&left, left_width as usize)),
                Print(" | "),
                Print(fit_text(&middle, middle_width as usize)),
                Print(" | "),
                Print(clip_text(&right, preview_width.max(20))),
                Print("\n"),
            )?;
        }

        let footer_y = height.saturating_sub(2);
        let mut footer = self.status.clone();
        if let Some(clipboard) = &self.clipboard {
            let mode = match clipboard.mode {
                ClipboardMode::Copy => "Copy",
                ClipboardMode::Cut => "Cut",
            };
            footer.push_str(&format!(" | Clipboard: {}", mode));
        }

        execute!(
            out,
            cursor::MoveTo(0, footer_y),
            SetForegroundColor(Color::DarkGrey),
            Print(clip_text(&footer, width.saturating_sub(1) as usize)),
            ResetColor,
        )?;

        if let Some(Modal::ConfirmDelete { label, .. }) = &self.modal {
            let prompt = format!("Delete '{}' ? Press Y to confirm, N to cancel.", label);
            execute!(
                out,
                cursor::MoveTo(0, footer_y.saturating_sub(1)),
                SetForegroundColor(Color::Red),
                Print(clip_text(&prompt, width.saturating_sub(1) as usize)),
                ResetColor,
            )?;
        }

        out.flush()?;
        Ok(())
    }

    fn render_help_overlay(&self, out: &mut std::io::Stdout) -> io::Result<()> {
        let (width, height) = terminal::size().unwrap_or((80, 24));
        let lines = help_lines();
        let box_width = width.saturating_sub(4).max(40) as usize;
        let mut content = Vec::new();

        for line in lines {
            content.extend(wrap_text(line, box_width.saturating_sub(4)));
            if line.is_empty() {
                content.push(String::new());
            }
        }

        let available = height.saturating_sub(4) as usize;
        let visible = content.len().min(available);

        execute!(
            out,
            SetForegroundColor(Color::Cyan),
            Print(fit_text("Help", width as usize)),
            ResetColor,
            Print("\n\n"),
        )?;

        for line in content.iter().take(visible) {
            execute!(out, Print(fit_text(line, width.saturating_sub(4) as usize)), Print("\n"))?;
        }

        let footer = "Press H, Esc, or Enter to close help.";
        execute!(
            out,
            cursor::MoveTo(0, height.saturating_sub(2)),
            SetForegroundColor(Color::DarkGrey),
            Print(fit_text(footer, width as usize)),
            ResetColor,
        )?;

        out.flush()?;
        Ok(())
    }

    fn panel_title(&self, label: &str, path: Option<&PathBuf>) -> String {
        match path {
            Some(path) => format!("{} [{}]", label, path.display()),
            None => label.to_string(),
        }
    }

    fn render_directory_lines(
        &self,
        entries: &[EntryItem],
        width: usize,
        highlight_path: Option<&PathBuf>,
        selected_index: Option<usize>,
    ) -> Vec<String> {
        if entries.is_empty() {
            return vec![String::from("(empty)")];
        }

        let mut lines = Vec::new();
        for (index, item) in entries.iter().enumerate() {
            let prefix = if item.is_dir { "[DIR]" } else if item.is_file { "[FILE]" } else { "[ITEM]" };
            let body = format!("{} {}", prefix, item.name);
            let should_highlight = selected_index.is_some_and(|selected| selected == index)
                || highlight_path.is_some_and(|path| path == &item.path);

            let indicator = if should_highlight { ">" } else { " " };
            lines.push(fit_text(&format!("{} {}", indicator, body), width));
        }

        lines
    }
}

fn three_column_widths(total_width: u16) -> (u16, u16, u16) {
    // Two separators: " | " and " | " => 6 columns.
    let separators = 6;
    // Keep one spare column to avoid terminal auto-wrap flicker/blank lines.
    let safety_margin = 1;

    if total_width <= separators + safety_margin + 3 {
        return (1, 1, 1);
    }

    let available = total_width - separators - safety_margin;
    let min_left = 10;
    let min_middle = 12;
    let min_right = 12;

    if available <= min_left + min_middle + min_right {
        let left = (available / 3).max(1);
        let middle = ((available - left) / 2).max(1);
        let right = available.saturating_sub(left + middle).max(1);
        return (left, middle, right);
    }

    let mut left = ((available as f32) * 0.25) as u16;
    let mut middle = ((available as f32) * 0.30) as u16;

    left = left.max(min_left);
    middle = middle.max(min_middle);

    if left + middle + min_right > available {
        let overflow = left + middle + min_right - available;
        let cut_left = (overflow / 2).min(left.saturating_sub(min_left));
        left -= cut_left;
        let remaining = overflow - cut_left;
        let cut_middle = remaining.min(middle.saturating_sub(min_middle));
        middle -= cut_middle;
    }

    let right = available.saturating_sub(left + middle).max(min_right);
    (left, middle, right)
}

fn fit_text(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let mut result = String::new();
    for (count, ch) in text.chars().enumerate() {
        if count >= width {
            if width >= 1 {
                result.pop();
                result.push('…');
            }
            return pad_to_width(result, width);
        }
        result.push(ch);
    }

    pad_to_width(result, width)
}

fn clip_text(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let mut result = String::new();
    for (count, ch) in text.chars().enumerate() {
        if count >= width {
            if width >= 1 {
                result.pop();
                result.push('.');
            }
            return result;
        }
        result.push(ch);
    }
    result
}

fn pad_to_width(mut text: String, width: usize) -> String {
    let current_width = text.chars().count();
    if current_width < width {
        text.push_str(&" ".repeat(width - current_width));
    }
    text
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = Vec::new();

    for raw_line in text.lines() {
        if raw_line.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current = String::new();
        for word in raw_line.split_whitespace() {
            let candidate = if current.is_empty() {
                word.to_owned()
            } else {
                format!("{} {}", current, word)
            };

            if candidate.chars().count() > width {
                if !current.is_empty() {
                    lines.push(current);
                }
                current = word.chars().take(width).collect();
            } else {
                current = candidate;
            }
        }

        if !current.is_empty() {
            lines.push(current);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}