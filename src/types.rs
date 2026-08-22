use std::collections::HashMap;
use std::path::PathBuf;

/// Položka v adresáři - soit soubor nebo složka
#[derive(Clone, Debug)]
pub struct EntryItem {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_file: bool,
}

/// Režim schránky - kopírování nebo stříhání
#[derive(Clone, Debug)]
pub enum ClipboardMode {
    Copy,
    Cut,
}

/// Položka ve schránce s cestou a režimem
#[derive(Clone, Debug)]
pub struct ClipboardItem {
    pub source: PathBuf,
    pub mode: ClipboardMode,
}

/// Stav navigace - kam se koukat a co je vybráno
#[derive(Clone, Copy)]
pub struct NavigationState {
    pub selected: usize,
    pub scroll_offset: usize,
}

/// Modální dialogy - Help nebo potvrzení mazání
#[derive(Clone)]
pub enum Modal {
    Help,
    ConfirmDelete { path: PathBuf, label: String },
}

/// Hlavní stav aplikace
pub struct App {
    pub current_dir: PathBuf,
    pub parent_dir: Option<PathBuf>,
    pub parent_entries: Vec<EntryItem>,
    pub entries: Vec<EntryItem>,
    pub volumes: Vec<PathBuf>,
    pub selected: usize,
    pub status: String,
    pub preview: String,
    pub preview_scroll_offset: usize,
    pub clipboard: Option<ClipboardItem>,
    pub modal: Option<Modal>,
    pub scroll_offset: usize,
    pub navigation_history: HashMap<PathBuf, NavigationState>,
    pub shift_filter_query: String,
}
