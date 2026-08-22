use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};

use crate::types::Modal;

/// Výsledek zpracování klávesy v modálu
#[derive(Debug)]
pub enum ModalResult {
    /// Zůstanu v modálu
    Continue,
    /// Zavřu modál
    Close,
    /// Vykonám akci
    Action(ModalAction),
}

/// Akce, kterou modál spustí
#[derive(Debug, Clone)]
pub enum ModalAction {
    ConfirmDelete { path: PathBuf, label: String },
}

/// Zpracování klávesy v modálu Help
pub fn handle_help_key(key: KeyEvent) -> ModalResult {
    match key.code {
        KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Enter => {
            ModalResult::Close
        }
        _ => ModalResult::Continue,
    }
}

/// Zpracování klávesy v modálu ConfirmDelete
pub fn handle_confirm_delete_key(
    key: KeyEvent,
    path: PathBuf,
    label: String,
) -> ModalResult {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            ModalResult::Action(ModalAction::ConfirmDelete { path, label })
        }
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => ModalResult::Close,
        _ => ModalResult::Continue,
    }
}

/// Jednotný handler pro všechny modály
pub fn handle_modal_key(key: KeyEvent, modal: &Modal) -> ModalResult {
    match modal {
        Modal::Help => handle_help_key(key),
        Modal::ConfirmDelete { path, label } => {
            handle_confirm_delete_key(key, path.clone(), label.clone())
        }
    }
}
