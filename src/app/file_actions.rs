use std::io;
use std::path::PathBuf;

use crate::actions::{delete_path, paste_clipboard, ClipboardItem, ClipboardMode};

use super::*;

impl App {
    pub(crate) fn copy_selected(&mut self) {
        if let Some(item) = self.selected_item().cloned() {
            self.clipboard = Some(ClipboardItem {
                source: item.path.clone(),
                mode: ClipboardMode::Copy,
            });
            self.status = format!("Copied {}.", item.name);
        }
    }

    pub(crate) fn cut_selected(&mut self) {
        if let Some(item) = self.selected_item().cloned() {
            self.clipboard = Some(ClipboardItem {
                source: item.path.clone(),
                mode: ClipboardMode::Cut,
            });
            self.status = format!("Cut {}.", item.name);
        }
    }

    pub(crate) fn paste_selected(&mut self) -> io::Result<()> {
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

    pub(crate) fn request_delete(&mut self) {
        if let Some(item) = self.selected_item() {
            self.modal = Some(Modal::ConfirmDelete {
                path: item.path.clone(),
                label: item.name.clone(),
            });
        }
    }

    pub(crate) fn delete_confirmed(&mut self, path: PathBuf, label: String) -> io::Result<()> {
        delete_path(&path)?;
        self.modal = None;
        self.reload()?;
        self.status = format!("Deleted {}.", label);
        Ok(())
    }
}
