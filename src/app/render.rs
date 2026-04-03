use std::io::{self, stdout, Write};
use std::path::PathBuf;

use crossterm::{
    cursor,
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};

use crate::actions::ClipboardMode;
use crate::explorer_fs::volume_label;
use crate::help::help_lines;

use super::*;

impl App {
    pub(crate) fn render(&self) -> io::Result<()> {
        let mut out = stdout();
        execute!(out, Clear(ClearType::All), cursor::MoveTo(0, 0))?;

        if matches!(self.modal, Some(Modal::Help)) {
            return self.render_help_overlay(&mut out);
        }

        let (width, height) = terminal::size().unwrap_or((80, 24));
        let (left_width, middle_width, right_width) = three_column_widths(width);
        let content_height = self.current_list_rows_from_height(height).max(1);
        let preview_width = right_width as usize;

        execute!(
            out,
            SetForegroundColor(Color::Cyan),
            Print(format!("File Explorer - {}\n", self.current_dir.display())),
            ResetColor,
            Print("Controls: arrows move | Right enters folders | Enter opens files in Notepad | H help | Q quit\n\n"),
        )?;

        execute!(out, SetForegroundColor(Color::DarkGrey), Print(self.volume_summary(width as usize)), ResetColor, Print("\n"))?;

        execute!(out, SetForegroundColor(Color::Yellow), Print(self.panel_title("Parent", self.parent_dir.as_ref())), ResetColor)?;
        execute!(out, Print(" | "))?;
        execute!(out, SetForegroundColor(Color::Yellow), Print(self.panel_title("Current", Some(&self.current_dir))), ResetColor)?;
        execute!(out, Print(" | "))?;
        execute!(out, SetForegroundColor(Color::Yellow), Print(self.panel_title("Preview", None)), ResetColor, Print("\n"))?;

        let left_lines = self.render_directory_lines(&self.parent_entries, left_width as usize, Some(&self.current_dir), None);
        let middle_lines = self.render_scrolled_directory_lines(&self.entries, middle_width as usize, content_height);
        let right_lines = wrap_text(&self.preview, preview_width.max(20));
        let preview_max_offset = right_lines.len().saturating_sub(content_height);
        let preview_start = self.preview_scroll_offset.min(preview_max_offset);

        for row in 0..content_height {
            let left = left_lines.get(row).cloned().unwrap_or_default();
            let middle = middle_lines.get(row).cloned().unwrap_or_default();
            let right = right_lines
                .get(preview_start + row)
                .cloned()
                .unwrap_or_default();

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

    fn volume_summary(&self, width: usize) -> String {
        if self.volumes.is_empty() {
            return fit_text("Volumes: none found", width);
        }

        let mut summary = String::from("Volumes: ");
        for (index, volume) in self.volumes.iter().enumerate().take(9) {
            if index > 0 {
                summary.push_str(" | ");
            }
            summary.push_str(&volume_label(index, volume));
        }

        fit_text(&summary, width)
    }

    fn current_list_rows_from_height(&self, height: u16) -> usize {
        height.saturating_sub(7) as usize
    }

    pub(crate) fn current_list_rows(&self) -> io::Result<usize> {
        let (_, height) = terminal::size().unwrap_or((80, 24));
        Ok(self.current_list_rows_from_height(height).max(1))
    }

    pub(crate) fn max_scroll_start(&self, rows: usize) -> usize {
        let rows = rows.max(1);
        let max_visible_files = rows.saturating_sub(1).max(1);
        self.entries.len().saturating_sub(max_visible_files)
    }

    fn visible_entry_range(&self, scroll_offset: usize, rows: usize) -> (usize, usize) {
        let rows = rows.max(1);
        if self.entries.is_empty() {
            return (0, 0);
        }

        let start = scroll_offset.min(self.entries.len());
        let more_above = start > 0;

        let (file_rows, _more_below) = if more_above {
            let available_for_files = rows.saturating_sub(1);
            let tentative_end = (start + available_for_files).min(self.entries.len());
            let has_more_below = tentative_end < self.entries.len();

            if has_more_below {
                let adjusted_file_rows = available_for_files.saturating_sub(1).max(1);
                (adjusted_file_rows, true)
            } else {
                (available_for_files, false)
            }
        } else {
            let tentative_end = (start + rows).min(self.entries.len());
            let has_more_below = tentative_end < self.entries.len();

            if has_more_below {
                (rows.saturating_sub(1).max(1), true)
            } else {    
                (rows, false)
            }
        };

        let end = (start + file_rows).min(self.entries.len());
        (start, end)
    }

    pub(crate) fn ensure_selected_visible(&mut self, rows: usize) {
        let rows = rows.max(1);
        let (vis_start, vis_end) = self.visible_entry_range(self.scroll_offset, rows);

        if self.selected < vis_start {
            self.scroll_offset = self.selected;
        } else if self.selected >= vis_end {
            let visible_items = vis_end.saturating_sub(vis_start).max(1);
            self.scroll_offset = self.selected + 1 - visible_items;
        }

        self.scroll_offset = self.scroll_offset.min(self.max_scroll_start(rows));
    }

    pub(crate) fn scroll_preview_by(&mut self, delta: isize) {
        let (width, height) = terminal::size().unwrap_or((80, 24));
        let (_, _, right_width) = three_column_widths(width);
        let rows = self.current_list_rows_from_height(height).max(1);
        let preview_width = right_width as usize;
        let wrapped = wrap_text(&self.preview, preview_width.max(20));
        let max_offset = wrapped.len().saturating_sub(rows);

        let next = if delta.is_negative() {
            self.preview_scroll_offset.saturating_sub(delta.unsigned_abs())
        } else {
            self.preview_scroll_offset.saturating_add(delta as usize)
        };

        self.preview_scroll_offset = next.min(max_offset);
        self.status = format!(
            "Preview scroll: {}/{}",
            self.preview_scroll_offset,
            max_offset
        );
    }

    pub(crate) fn clamp_scroll_to_selection(&mut self, rows: usize) {
        let rows = rows.max(1);

        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + rows {
            self.scroll_offset = self.selected + 1 - rows;
        }

        self.scroll_offset = self.scroll_offset.min(self.max_scroll_start(rows));
    }

    fn render_scrolled_directory_lines(
        &self,
        entries: &[EntryItem],
        width: usize,
        rows: usize,
    ) -> Vec<String> {
        let rows = rows.max(1);
        if entries.is_empty() {
            let mut lines = vec![fit_text("empty", width)];
            while lines.len() < rows {
                lines.push(String::new());
            }
            return lines;
        }

        let start = self.scroll_offset.min(entries.len());
        let more_above = start > 0;

        let (file_rows, more_below) = if more_above {
            let available_for_files = rows.saturating_sub(1);
            let tentative_end = (start + available_for_files).min(entries.len());
            let has_more_below = tentative_end < entries.len();

            if has_more_below {
                let adjusted_file_rows = available_for_files.saturating_sub(1).max(1);
                (adjusted_file_rows, true)
            } else {
                (available_for_files, false)
            }
        } else {
            let tentative_end = (start + rows).min(entries.len());
            let has_more_below = tentative_end < entries.len();

            if has_more_below {
                (rows.saturating_sub(1).max(1), true)
            } else {
                (rows, false)
            }
        };

        let end = (start + file_rows).min(entries.len());

        let mut lines = Vec::new();
        if more_above {
            lines.push(fit_text("^ more above", width));
        }

        for (index, item) in entries[start..end].iter().enumerate() {
            let absolute_index = start + index;
            let prefix = if item.is_dir { "[/]" } else if item.is_file { "[.]" } else { "[?]" };
            let body = format!("{} {}", prefix, item.name);
            let indicator = if absolute_index == self.selected { ">" } else { " " };
            lines.push(fit_text(&format!("{} {}", indicator, body), width));
        }

        if more_below {
            lines.push(fit_text("v more below", width));
        }

        while lines.len() < rows {
            lines.push(String::new());
        }

        lines.truncate(rows);
        lines
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
            let prefix = if item.is_dir { "[/]" } else if item.is_file { "[.]" } else { "[?]" };
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
    let separators = 6;
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
