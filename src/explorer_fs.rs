use std::cmp::Ordering;
use std::fs::File;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use zip::read::ZipArchive;

#[derive(Clone, Debug)]
pub struct EntryItem {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_file: bool,
}

pub fn list_entries(current_dir: &Path) -> io::Result<Vec<EntryItem>> {
    let mut entries: Vec<_> = fs::read_dir(current_dir)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|entry| {
            let path = entry.path();
            let file_type = entry.file_type().ok();

            EntryItem {
                name: entry.file_name().to_string_lossy().into_owned(),
                path,
                is_dir: file_type.as_ref().is_some_and(|kind| kind.is_dir()),
                is_file: file_type.as_ref().is_some_and(|kind| kind.is_file()),
            }
        })
        .collect();

    entries.sort_by(|left, right| match (left.is_dir, right.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
    });

    Ok(entries)
}

pub fn drive_root(letter: char) -> PathBuf {
    PathBuf::from(format!("{}:\\", letter.to_ascii_uppercase()))
}

pub fn available_volumes() -> Vec<PathBuf> {
    let mut volumes = Vec::new();

    for letter in 'C'..='Z' {
        let root = drive_root(letter);
        if root.exists() {
            volumes.push(root);
        }
    }

    volumes
}

pub fn volume_label(index: usize, path: &Path) -> String {
    let display = path.display().to_string();
    format!("{}: {}", index + 1, display)
}

pub fn read_preview(path: &Path, max_bytes: usize) -> io::Result<String> {
    if is_zip_file(path) {
        return read_zip_preview(path);
    }

    let bytes = fs::read(path)?;
    let slice = if bytes.len() > max_bytes {
        &bytes[..max_bytes]
    } else {
        &bytes
    };

    let mut text = String::from_utf8_lossy(slice).into_owned();
    if bytes.len() > max_bytes {
        text.push_str("\n... (zkráceno)");
    }

    Ok(text)
}

fn is_zip_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

fn read_zip_preview(path: &Path) -> io::Result<String> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;

    if archive.len() == 0 {
        return Ok(String::from("ZIP archive is empty."));
    }

    let mut lines = Vec::new();
    lines.push(format!("ZIP archive: {}", path.display()));
    lines.push(String::new());

    for index in 0..archive.len().min(40) {
        let file = archive
            .by_index(index)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;

        if file.encrypted() {
            return Ok(String::from("Archive is encrypted."));
        }

        let kind = if file.is_dir() { "DIR" } else { "FILE" };
        lines.push(format!("[{kind}] {}", file.name()));
    }

    if archive.len() > 40 {
        lines.push(String::from("... (more entries)"));
    }

    Ok(lines.join("\n"))
}
