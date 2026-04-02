use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug)]
pub enum ClipboardMode {
    Copy,
    Cut,
}

#[derive(Clone, Debug)]
pub struct ClipboardItem {
    pub source: PathBuf,
    pub mode: ClipboardMode,
}

pub fn open_in_notepad(path: &Path) -> io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("notepad.exe").arg(path).spawn().map(|_| ())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("open").arg(path).spawn().map(|_| ())
    }
}

pub fn delete_path(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

pub fn paste_clipboard(item: &ClipboardItem, target_dir: &Path) -> io::Result<PathBuf> {
    let file_name = item
        .source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("item");

    let destination = unique_destination(target_dir, file_name);

    match item.mode {
        ClipboardMode::Copy => {
            copy_recursively(&item.source, &destination)?;
        }
        ClipboardMode::Cut => {
            if fs::rename(&item.source, &destination).is_err() {
                copy_recursively(&item.source, &destination)?;
                delete_path(&item.source)?;
            }
        }
    }

    Ok(destination)
}

fn copy_recursively(source: &Path, destination: &Path) -> io::Result<()> {
    if source.is_file() {
        fs::copy(source, destination)?;
        return Ok(());
    }

    if source.is_dir() {
        fs::create_dir_all(destination)?;

        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let child_source = entry.path();
            let child_destination = destination.join(entry.file_name());
            copy_recursively(&child_source, &child_destination)?;
        }
    }

    Ok(())
}

fn unique_destination(target_dir: &Path, file_name: &str) -> PathBuf {
    let mut candidate = target_dir.join(file_name);

    if !candidate.exists() {
        return candidate;
    }

    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(file_name);
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");

    for index in 1..1000 {
        let name = if extension.is_empty() {
            format!("{}_copy{}", stem, index)
        } else {
            format!("{}_copy{}.{}", stem, index, extension)
        };
        candidate = target_dir.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }

    target_dir.join(format!("{}_copy_final", stem))
}