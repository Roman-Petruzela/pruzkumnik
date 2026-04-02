use std::cmp::Ordering;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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

pub fn read_preview(path: &Path, max_bytes: usize) -> io::Result<String> {
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
