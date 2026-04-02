use std::io::{self, stdout, Write};
use std::path::PathBuf;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::explorer_fs::{list_entries, read_preview, EntryItem};

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
            status: String::from("Šipky: pohyb | Enter: otevřít | Backspace: o úroveň výš | q: konec | r: obnovit"),
            preview: String::from("Vyber složku nebo soubor."),
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

    fn update_preview(&mut self) {
        if let Some(item) = self.entries.get(self.selected) {
            if item.is_dir {
                self.preview = format!(
                    "Složka: {}\nCesta: {}\n\nEnter otevře složku.",
                    item.name,
                    item.path.display()
                );
            } else if item.is_file {
                match read_preview(&item.path, 8000) {
                    Ok(contents) => {
                        self.preview = format!(
                            "Soubor: {}\nCesta: {}\n\n{}",
                            item.name,
                            item.path.display(),
                            contents
                        );
                    }
                    Err(error) => {
                        self.preview = format!("Soubor: {}\n\nNelze načíst obsah: {error}", item.name);
                    }
                }
            } else {
                self.preview = format!("Položka: {}\n\nTyp není rozpoznán.", item.name);
            }
        } else {
            self.preview = String::from("Ve složce nejsou žádné položky.");
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> io::Result<bool> {
        match key.code {
            KeyCode::Char('q') => return Ok(false),
            KeyCode::Char('r') => {
                self.status = String::from("Obnovení obsahu.");
                self.reload()?;
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
            KeyCode::Enter => {
                if let Some(item) = self.entries.get(self.selected).cloned() {
                    if item.is_dir {
                        self.current_dir = item.path;
                        self.selected = 0;
                        self.reload()?;
                        self.status = String::from("Otevřena složka.");
                    } else if item.is_file {
                        self.status = String::from("Soubor zobrazen v náhledu.");
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(parent) = self.parent_dir.clone() {
                    self.current_dir = parent;
                    self.selected = 0;
                    self.reload()?;
                    self.status = String::from("Návrat o úroveň výš.");
                }
            }
            KeyCode::Left => {
                if let Some(parent) = self.parent_dir.clone() {
                    self.current_dir = parent;
                    self.selected = 0;
                    self.reload()?;
                    self.status = String::from("Návrat o úroveň výš.");
                }
            }
            _ => {}
        }

        Ok(true)
    }

    fn render(&self) -> io::Result<()> {
        let mut out = stdout();
        execute!(out, Clear(ClearType::All), cursor::MoveTo(0, 0))?;

        let (width, height) = terminal::size().unwrap_or((80, 24));
        let left_width = ((width as f32) * 0.25).max(18.0) as u16;
        let middle_width = ((width as f32) * 0.28).max(22.0) as u16;
        let right_width = width.saturating_sub(left_width + middle_width + 4);
        let content_height = height.saturating_sub(5) as usize;

        execute!(
            out,
            SetForegroundColor(Color::Cyan),
            Print(format!("Pruzkumnik souboru - {}\n", self.current_dir.display())),
            ResetColor,
            Print("Ovládání: šipky | Enter | Backspace | r | q\n\n"),
        )?;

        execute!(out, SetForegroundColor(Color::Yellow), Print(self.panel_title("Nadřazená složka", self.parent_dir.as_ref())), ResetColor)?;
        execute!(out, Print(" | "))?;
        execute!(out, SetForegroundColor(Color::Yellow), Print(self.panel_title("Aktuální složka", Some(&self.current_dir))), ResetColor)?;
        execute!(out, Print(" | "))?;
        execute!(out, SetForegroundColor(Color::Yellow), Print(self.panel_title("Náhled", None)), ResetColor, Print("\n"))?;

        let left_lines = self.render_directory_lines(&self.parent_entries, left_width as usize, self.current_dir.file_name().and_then(|name| name.to_str()), false);
        let middle_lines = self.render_directory_lines(&self.entries, middle_width as usize, None, true);
        let right_lines = wrap_text(&self.preview, right_width.max(20) as usize);

        for row in 0..content_height {
            let left = left_lines.get(row).cloned().unwrap_or_default();
            let middle = middle_lines.get(row).cloned().unwrap_or_default();
            let right = right_lines.get(row).cloned().unwrap_or_default();

            execute!(
                out,
                Print(format!("{:left_width$}", left, left_width = left_width as usize)),
                Print(" | "),
                Print(format!("{:middle_width$}", middle, middle_width = middle_width as usize)),
                Print(" | "),
                Print(truncate_text(&right, right_width.max(20) as usize)),
                Print("\n"),
            )?;
        }

        let footer_y = height.saturating_sub(2);
        execute!(
            out,
            cursor::MoveTo(0, footer_y),
            SetForegroundColor(Color::DarkGrey),
            Print(truncate_text(&self.status, width as usize)),
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
        highlight_name: Option<&str>,
        highlight_selected: bool,
    ) -> Vec<String> {
        if entries.is_empty() {
            return vec![String::from("(prázdná složka)")];
        }

        let mut lines = Vec::new();
        for (index, item) in entries.iter().enumerate() {
            let prefix = if item.is_dir { "[D]" } else if item.is_file { "[F]" } else { "[?]" };
            let mut line = format!("{} {}", prefix, item.name);

            if line.chars().count() > width {
                line = truncate_text(&line, width);
            }

            let should_highlight = if highlight_selected {
                index == self.selected
            } else if let Some(name) = highlight_name {
                item.name == name
            } else {
                false
            };

            if should_highlight {
                lines.push(format!("> {}", line));
            } else {
                lines.push(format!("  {}", line));
            }
        }

        lines
    }
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let mut result = String::new();
    for (count, ch) in text.chars().enumerate() {
        if count >= max_chars {
            result.push('…');
            return result;
        }
        result.push(ch);
    }
    result
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