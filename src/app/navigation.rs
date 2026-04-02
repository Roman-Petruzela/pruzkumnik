use std::io;
use std::path::PathBuf;

use crate::explorer_fs::{available_volumes, list_entries, read_preview};

use super::*;

impl App {
    pub(crate) fn reload(&mut self) -> io::Result<()> {
        self.volumes = available_volumes();
        self.parent_dir = self.current_dir.parent().map(|path| path.to_path_buf());
        self.parent_entries = match &self.parent_dir {
            Some(path) => list_entries(path)?,
            None => Vec::new(),
        };
        self.entries = list_entries(&self.current_dir)?;

        if self.selected >= self.entries.len() {
            self.selected = self.entries.len().saturating_sub(1);
        }

        self.clamp_scroll_to_selection(self.current_list_rows().unwrap_or(1));
        self.update_preview();
        Ok(())
    }

    pub(crate) fn selected_item(&self) -> Option<&EntryItem> {
        self.entries.get(self.selected)
    }

    fn save_navigation_state(&mut self) {
        self.navigation_history.insert(
            self.current_dir.clone(),
            NavigationState {
                selected: self.selected,
                scroll_offset: self.scroll_offset,
            },
        );
    }

    fn restore_navigation_state(&mut self) {
        if let Some(state) = self.navigation_history.get(&self.current_dir) {
            self.selected = state.selected;
            self.scroll_offset = state.scroll_offset;
        } else {
            self.selected = 0;
            self.scroll_offset = 0;
        }
    }

    pub(crate) fn enter_directory(&mut self, path: PathBuf) -> io::Result<()> {
        self.save_navigation_state();
        self.current_dir = path;
        self.restore_navigation_state();
        self.reload()?;
        self.status = String::from("Folder opened.");
        Ok(())
    }

    pub(crate) fn go_parent(&mut self) -> io::Result<()> {
        if let Some(parent) = self.parent_dir.clone() {
            self.save_navigation_state();
            self.current_dir = parent;
            self.restore_navigation_state();
            self.reload()?;
            self.status = String::from("Moved to parent folder.");
        } else {
            self.status = String::from("You are already at a root folder.");
        }

        Ok(())
    }

    pub(crate) fn switch_root(&mut self, index: usize) -> io::Result<()> {
        if let Some(root) = self.volumes.get(index).cloned() {
            self.current_dir = root;
            self.selected = 0;
            self.scroll_offset = 0;
            self.reload()?;
            self.status = format!("Switched to volume {}.", index + 1);
        } else {
            self.status = format!("Volume {} is not available.", index + 1);
        }

        Ok(())
    }

    pub(crate) fn update_preview(&mut self) {
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
}
